#![cfg(target_os = "android")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{AppHandle, Manager, Runtime, Wry};

use super::engine::{
    AudioChunk, EngineInfo, SpeakRequest, SynthesisRequest, TtsEngine, VoiceDescriptor,
};
use super::media::{TtsMediaMetadata, TtsMediaSession, TtsMediaState};
use crate::error::{KoraError, Result};

static HANDLE: OnceLock<PluginHandle<Wry>> = OnceLock::new();
static CONTROL_CHANNEL: OnceLock<Channel<TtsControl>> = OnceLock::new();

fn plugin_handle() -> Result<&'static PluginHandle<Wry>> {
    HANDLE
        .get()
        .ok_or_else(|| KoraError::Internal("el plugin TTS de Android no está inicializado".into()))
}

/// Acción que llega desde la notificación o la pantalla de bloqueo.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TtsControl {
    action: String,
}

fn decode_control(body: InvokeResponseBody) -> Option<TtsControl> {
    match body {
        InvokeResponseBody::Json(json) => serde_json::from_str(&json).ok(),
        InvokeResponseBody::Raw(bytes) => serde_json::from_slice(&bytes).ok(),
    }
}

pub fn init_plugin() -> TauriPlugin<Wry> {
    Builder::new("kora-tts")
        .setup(|app, api| {
            let handle = api.register_android_plugin("dev.kora.reader.tts", "KoraTtsPlugin")?;
            let _ = HANDLE.set(handle);

            let app_for_channel = app.clone();
            let channel = Channel::<TtsControl>::new(move |body| {
                let Some(control) = decode_control(body) else {
                    return Ok(());
                };
                let app = app_for_channel.clone();
                tauri::async_runtime::spawn(async move {
                    let Some(state) = app.try_state::<crate::state::AppState>() else {
                        return;
                    };
                    let session = state.tts_session.clone();
                    let result = match control.action.as_str() {
                        "play" => session.resume(&app).await,
                        "pause" => session.pause(&app).await,
                        "stop" => session.stop(&app).await,
                        other => {
                            tracing::warn!(action = other, "acción TTS desconocida");
                            return;
                        }
                    };
                    if let Err(error) = result {
                        tracing::warn!(%error, "no se pudo aplicar la acción del reproductor TTS");
                    }
                });
                Ok(())
            });

            let _ = CONTROL_CHANNEL.set(channel);
            Ok(())
        })
        .build()
}

/// Conecta el canal de control nativo. Se llama después del setup de Tauri,
/// cuando el plugin Kotlin ya está registrado.
pub fn install_control_relay() -> Result<()> {
    let handle = plugin_handle()?;
    let channel = CONTROL_CHANNEL.get().ok_or_else(|| {
        KoraError::Internal("el canal de control TTS no está inicializado".into())
    })?;

    #[derive(Serialize)]
    struct RelayArgs<'a> {
        channel: &'a Channel<TtsControl>,
    }

    handle
        .run_mobile_plugin::<serde_json::Value>("setupControlRelay", RelayArgs { channel })
        .map_err(|error| KoraError::Internal(format!("TTS Android: {error}")))?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpeakArgs<'a> {
    text: &'a str,
    language: Option<&'a str>,
    voice_id: Option<&'a str>,
    rate: f32,
    pitch: f32,
    volume: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeVoice {
    id: String,
    name: String,
    language: String,
}

#[derive(Debug, Clone, Deserialize)]
struct VoicesResponse {
    voices: Vec<NativeVoice>,
}

#[derive(Debug, Clone, Deserialize)]
struct UtteranceStatus {
    state: String,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionArgs<'a> {
    title: &'a str,
    artist: &'a str,
    album: Option<&'a str>,
    cover_path: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionStateArgs<'a> {
    state: &'a str,
    error: Option<&'a str>,
}

/// Motor TTS nativo de Android. El plugin Kotlin conserva `TextToSpeech` en un
/// singleton de proceso y publica una sesión Media3 para la pantalla de bloqueo.
pub struct AndroidTtsEngine {
    interrupted: Arc<AtomicBool>,
}

impl AndroidTtsEngine {
    pub fn new<R: Runtime>(_app: AppHandle<R>) -> Self {
        Self {
            interrupted: Arc::new(AtomicBool::new(false)),
        }
    }

    fn stop_native() {
        if let Ok(handle) = plugin_handle() {
            let _: serde_json::Value = handle
                .run_mobile_plugin("stop", json!({}))
                .unwrap_or(serde_json::Value::Null);
        }
    }
}

#[async_trait]
impl TtsEngine for AndroidTtsEngine {
    fn info(&self) -> EngineInfo {
        EngineInfo {
            id: "android".to_string(),
            name: "Android TextToSpeech".to_string(),
            offline: true,
            voice_cloning: false,
            streaming: false,
            available: plugin_handle().is_ok(),
            supports_pause: false,
        }
    }

    async fn voices(&self) -> Result<Vec<VoiceDescriptor>> {
        let handle = plugin_handle()?;
        let voices = tokio::task::spawn_blocking(move || {
            // El motor nativo inicializa de forma asíncrona; sondeamos hasta
            // que el catálogo de voces esté disponible.
            for _ in 0..75 {
                let response: VoicesResponse = handle
                    .run_mobile_plugin("getVoices", json!({}))
                    .map_err(|error| KoraError::Internal(format!("TTS Android: {error}")))?;
                if !response.voices.is_empty() {
                    return Ok::<Vec<NativeVoice>, KoraError>(response.voices);
                }
                std::thread::sleep(Duration::from_millis(200));
            }

            let response: VoicesResponse = handle
                .run_mobile_plugin("getVoices", json!({}))
                .map_err(|error| KoraError::Internal(format!("TTS Android: {error}")))?;
            Ok::<Vec<NativeVoice>, KoraError>(response.voices)
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de voces Android: {error}")))??;

        Ok(voices
            .into_iter()
            .map(|voice| VoiceDescriptor {
                id: voice.id,
                name: voice.name,
                language: voice
                    .language
                    .split(['-', '_'])
                    .next()
                    .unwrap_or("und")
                    .to_lowercase(),
                gender: None,
                engine: "android".to_string(),
                installed: true,
                size_bytes: 0,
                cloned: false,
            })
            .collect())
    }

    async fn synthesize(&self, _request: &SynthesisRequest) -> Result<AudioChunk> {
        Err(KoraError::Unsupported(
            "Android TextToSpeech no expone audio PCM para previsualizar".into(),
        ))
    }

    async fn speak(&self, request: &SpeakRequest) -> Result<()> {
        let text = request.text.trim().to_string();
        if text.is_empty() {
            return Ok(());
        }

        self.interrupted.store(false, Ordering::SeqCst);
        let handle = plugin_handle()?;
        let voice_id = request.voice_id.trim().to_string();
        let language = request.language.clone();
        let rate = request.rate.clamp(0.5, 3.0);
        let pitch = request.pitch.clamp(0.5, 2.0);
        let volume = request.volume.clamp(0.0, 1.0);
        let interrupted = self.interrupted.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let _: serde_json::Value = handle
                .run_mobile_plugin(
                    "speak",
                    SpeakArgs {
                        text: &text,
                        language: language.as_deref(),
                        voice_id: (!voice_id.is_empty()).then_some(voice_id.as_str()),
                        rate,
                        pitch,
                        volume,
                    },
                )
                .map_err(|error| KoraError::Internal(format!("TTS Android: {error}")))?;

            let started = std::time::Instant::now();
            loop {
                if interrupted.load(Ordering::SeqCst) {
                    if let Ok(handle) = plugin_handle() {
                        let _: serde_json::Value = handle
                            .run_mobile_plugin("stop", json!({}))
                            .unwrap_or(serde_json::Value::Null);
                    }
                    return Err(KoraError::Conflict("locución interrumpida".into()));
                }

                let status: UtteranceStatus = handle
                    .run_mobile_plugin("utteranceStatus", json!({}))
                    .map_err(|error| KoraError::Internal(format!("TTS Android: {error}")))?;

                match status.state.as_str() {
                    "starting" | "idle" => {
                        if started.elapsed() > Duration::from_secs(15) {
                            let _: serde_json::Value = handle
                                .run_mobile_plugin("stop", json!({}))
                                .unwrap_or(serde_json::Value::Null);
                            return Err(KoraError::Internal(
                                "el motor TTS de Android no comenzó la locución".into(),
                            ));
                        }
                    }
                    "speaking" => {}
                    "finished" => return Ok(()),
                    "interrupted" => {
                        return Err(KoraError::Conflict("locución interrumpida".into()));
                    }
                    "error" => {
                        return Err(KoraError::Internal(
                            status
                                .error
                                .unwrap_or_else(|| "error del motor TTS de Android".into()),
                        ));
                    }
                    other => {
                        tracing::warn!(state = other, "estado TTS desconocido");
                    }
                }

                std::thread::sleep(Duration::from_millis(80));
            }
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de TTS Android: {error}")))?
    }

    async fn pause(&self) -> Result<()> {
        self.interrupted.store(true, Ordering::SeqCst);
        tokio::task::spawn_blocking(Self::stop_native)
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de pausa Android: {error}")))?;
        Ok(())
    }

    async fn resume(&self) -> Result<()> {
        self.interrupted.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.interrupted.store(true, Ordering::SeqCst);
        tokio::task::spawn_blocking(Self::stop_native)
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de stop Android: {error}")))?;
        Ok(())
    }
}

#[async_trait]
impl TtsMediaSession for AndroidTtsEngine {
    async fn start(&self, metadata: &TtsMediaMetadata) -> Result<()> {
        let handle = plugin_handle()?;
        let args = SessionArgs {
            title: &metadata.title,
            artist: &metadata.artist,
            album: metadata.album.as_deref(),
            cover_path: metadata.cover_path.as_deref(),
        };
        handle
            .run_mobile_plugin::<serde_json::Value>("startSession", args)
            .map_err(|error| KoraError::Internal(format!("TTS Android: {error}")))?;
        Ok(())
    }

    async fn set_state(&self, state: TtsMediaState) -> Result<()> {
        let handle = plugin_handle()?;
        let (name, error) = match &state {
            TtsMediaState::Playing => ("playing", None),
            TtsMediaState::Paused => ("paused", None),
            TtsMediaState::Stopped => ("stopped", None),
            TtsMediaState::Finished => ("finished", None),
            TtsMediaState::Error(message) => ("error", Some(message.as_str())),
        };
        handle
            .run_mobile_plugin::<serde_json::Value>(
                "setSessionState",
                SessionStateArgs { state: name, error },
            )
            .map_err(|error| KoraError::Internal(format!("TTS Android: {error}")))?;
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let handle = plugin_handle()?;
        handle
            .run_mobile_plugin::<serde_json::Value>("stopSession", json!({}))
            .map_err(|error| KoraError::Internal(format!("TTS Android: {error}")))?;
        Ok(())
    }
}
