#![cfg(target_os = "android")]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use tauri::{AppHandle, Runtime};
use tauri_plugin_tts::{GetVoicesRequest, QueueMode, SpeakRequest as PluginSpeakRequest, TtsExt};

use super::engine::{
    AudioChunk, EngineInfo, SpeakRequest, SynthesisRequest, TtsEngine, VoiceDescriptor,
};
use crate::error::{KoraError, Result};

/// Motor TTS nativo de Android. El plugin reproduce con `TextToSpeech`; este
/// motor espera a que termine la locución sondeando `isSpeaking`.
pub struct AndroidTtsEngine<R: Runtime> {
    app: AppHandle<R>,
    interrupted: Arc<AtomicBool>,
}

impl<R: Runtime> AndroidTtsEngine<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            interrupted: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl<R: Runtime> TtsEngine for AndroidTtsEngine<R> {
    fn info(&self) -> EngineInfo {
        EngineInfo {
            id: "android".to_string(),
            name: "Android TextToSpeech".to_string(),
            offline: true,
            voice_cloning: false,
            streaming: false,
            available: true,
            supports_pause: false,
        }
    }

    async fn voices(&self) -> Result<Vec<VoiceDescriptor>> {
        let app = self.app.clone();
        let voices = tokio::task::spawn_blocking(move || {
            // El motor nativo inicializa de forma asíncrona; sondeamos un poco
            // para que el selector no aparezca vacío al abrir el lector.
            for _ in 0..10 {
                let response = app
                    .tts()
                    .get_voices(GetVoicesRequest { language: None })
                    .map_err(|error| KoraError::Internal(format!("TTS Android: {error}")))?;
                if !response.voices.is_empty() {
                    return Ok(response.voices);
                }
                std::thread::sleep(Duration::from_millis(200));
            }

            app.tts()
                .get_voices(GetVoicesRequest { language: None })
                .map(|response| response.voices)
                .map_err(|error| KoraError::Internal(format!("TTS Android: {error}")))
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
        let app = self.app.clone();
        let interrupted = self.interrupted.clone();
        let voice_id = request.voice_id.trim().to_string();
        let language = request.language.clone();
        let rate = request.rate.clamp(0.5, 3.0);
        let pitch = request.pitch.clamp(0.5, 2.0);
        let volume = request.volume.clamp(0.0, 1.0);

        tokio::task::spawn_blocking(move || -> Result<()> {
            app.tts()
                .speak(PluginSpeakRequest {
                    text,
                    language,
                    voice_id: (!voice_id.is_empty()).then_some(voice_id),
                    rate,
                    pitch,
                    volume,
                    queue_mode: QueueMode::Flush,
                })
                .map_err(|error| KoraError::Internal(format!("TTS Android: {error}")))?;

            // `speak` vuelve cuando empieza la locución; esperamos a que el
            // motor nativo termine. Si se pide pausa/stop, interrumpimos.
            std::thread::sleep(Duration::from_millis(120));
            loop {
                if interrupted.load(Ordering::SeqCst) {
                    let _ = app.tts().stop();
                    return Err(KoraError::Conflict("locución interrumpida".into()));
                }

                match app.tts().is_speaking() {
                    Ok(status) if status.speaking => {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Ok(_) => return Ok(()),
                    Err(error) => {
                        return Err(KoraError::Internal(format!("TTS Android: {error}")));
                    }
                }
            }
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de TTS Android: {error}")))?
    }

    async fn pause(&self) -> Result<()> {
        self.interrupted.store(true, Ordering::SeqCst);
        let app = self.app.clone();
        tokio::task::spawn_blocking(move || {
            let _ = app.tts().stop();
        })
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
        let app = self.app.clone();
        tokio::task::spawn_blocking(move || {
            let _ = app.tts().stop();
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de stop Android: {error}")))?;
        Ok(())
    }
}
