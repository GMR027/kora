use serde::Serialize;

use super::manifest::AudiobookManifest;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSnapshot {
    pub audiobook_id: String,
    pub title: String,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub chapter_index: usize,
    pub chapter_title: String,
    pub section: usize,
    pub offset_start: usize,
    pub offset_end: usize,
    pub playing: bool,
    /// Portada del libro para el mini-reproductor; se resuelve una vez al
    /// iniciar la reproducción para no consultar SQLite en cada sondeo.
    pub cover_path: Option<String>,
}

impl PlaybackSnapshot {
    pub fn from_manifest(
        audiobook_id: String,
        manifest: &AudiobookManifest,
        position_ms: i64,
        playing: bool,
    ) -> Self {
        let index = manifest.chapter_at(position_ms);
        let chapter = manifest.chapter(index);
        Self {
            audiobook_id,
            title: manifest.book_title.clone(),
            position_ms,
            duration_ms: manifest.duration_ms,
            chapter_index: index,
            chapter_title: chapter
                .map(|chapter| chapter.title.clone())
                .unwrap_or_else(|| "Audiolibro".to_string()),
            section: chapter.map(|chapter| chapter.section).unwrap_or(0),
            offset_start: chapter.map(|chapter| chapter.offset_start).unwrap_or(0),
            offset_end: chapter.map(|chapter| chapter.offset_end).unwrap_or(0),
            playing,
            cover_path: None,
        }
    }
}

#[cfg(test)]
mod decode_probe {
    use std::fs::File;
    use std::io::BufReader;

    use rodio::{Decoder, Source};

    #[test]
    #[ignore]
    fn counts_decoded_audio_samples() {
        let Ok(path) = std::env::var("KORA_TEST_AUDIOBOOK") else {
            return;
        };
        let file = File::open(path).expect("abrir audio");
        let decoder = Decoder::new(BufReader::new(file)).expect("decodificar");
        let channels = decoder.channels();
        let rate = decoder.sample_rate();
        let samples = decoder.count();
        eprintln!("decoded samples={samples} channels={channels} rate={rate}");
        assert!(samples > 10_000);
    }
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use std::fs::File;
    use std::io::BufReader;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use anyhow::{Context, Result};
    use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

    use super::PlaybackSnapshot;
    use crate::audiobooks::manifest::AudiobookManifest;

    struct Output {
        _sink: Arc<MixerDeviceSink>,
        player: Arc<Player>,
    }

    struct Current {
        audiobook_id: String,
        manifest: AudiobookManifest,
        cover_path: Option<String>,
    }

    /// Reproductor de M4B/MP3 con búsqueda. Comparte un único dispositivo de
    /// salida para no abrir cientos de sinks durante la sesión.
    #[derive(Default)]
    pub struct AudiobookPlayback {
        output: Mutex<Option<Output>>,
        current: Mutex<Option<Current>>,
        /// Marca un final natural para que la capa de comandos guarde el
        /// progreso una sola vez, aunque la UI sondee varias veces.
        finished: AtomicBool,
    }

    impl AudiobookPlayback {
        fn output(&self) -> Result<(Arc<MixerDeviceSink>, Arc<Player>)> {
            let mut slot = self
                .output
                .lock()
                .map_err(|_| anyhow::anyhow!("reproductor de audiolibros bloqueado"))?;
            if slot.is_none() {
                let mut sink = DeviceSinkBuilder::open_default_sink()
                    .with_context(|| "no se pudo abrir el dispositivo de audio para audiolibros")?;
                sink.log_on_drop(false);
                let player = Arc::new(Player::connect_new(sink.mixer()));
                *slot = Some(Output {
                    _sink: Arc::new(sink),
                    player,
                });
            }
            let output = slot.as_ref().expect("salida inicializada");
            Ok((output._sink.clone(), output.player.clone()))
        }

        pub fn play(
            &self,
            audiobook_id: String,
            path: &Path,
            manifest: AudiobookManifest,
            start_ms: i64,
            cover_path: Option<String>,
        ) -> Result<PlaybackSnapshot> {
            let (_, player) = self.output()?;
            player.stop();
            self.finished.store(false, Ordering::SeqCst);

            let file =
                File::open(path).with_context(|| format!("no se pudo abrir {}", path.display()))?;
            let decoder = Decoder::new(BufReader::new(file))
                .with_context(|| "el audiolibro no se pudo decodificar")?;
            player.append(decoder);
            player.play();

            if start_ms > 0 {
                let target = start_ms.clamp(0, manifest.duration_ms.max(0)) as u64;
                let _ = player.try_seek(Duration::from_millis(target));
            }

            let position = player.get_pos().as_millis() as i64;
            let mut snapshot =
                PlaybackSnapshot::from_manifest(audiobook_id.clone(), &manifest, position, true);
            snapshot.cover_path = cover_path.clone();
            *self
                .current
                .lock()
                .map_err(|_| anyhow::anyhow!("reproductor de audiolibros bloqueado"))? =
                Some(Current {
                    audiobook_id,
                    manifest,
                    cover_path,
                });
            Ok(snapshot)
        }

        pub fn pause(&self) -> Result<Option<PlaybackSnapshot>> {
            let (_, player) = self.output()?;
            player.pause();
            self.snapshot()
        }

        pub fn resume(&self) -> Result<Option<PlaybackSnapshot>> {
            let (_, player) = self.output()?;
            player.play();
            self.snapshot()
        }

        pub fn stop(&self) -> Result<()> {
            let (_, player) = self.output()?;
            player.stop();
            self.finished.store(false, Ordering::SeqCst);
            *self
                .current
                .lock()
                .map_err(|_| anyhow::anyhow!("reproductor de audiolibros bloqueado"))? = None;
            Ok(())
        }

        pub fn set_speed(&self, speed: f32) -> Result<Option<PlaybackSnapshot>> {
            let (_, player) = self.output()?;
            player.set_speed(speed.clamp(0.5, 2.5));
            self.snapshot()
        }

        pub fn seek(&self, position_ms: i64) -> Result<Option<PlaybackSnapshot>> {
            let (_, player) = self.output()?;
            let current = self
                .current
                .lock()
                .map_err(|_| anyhow::anyhow!("reproductor de audiolibros bloqueado"))?
                .as_ref()
                .map(|current| (current.audiobook_id.clone(), current.manifest.duration_ms));
            let Some((_, duration)) = current else {
                return Ok(None);
            };
            let target = position_ms.clamp(0, duration.max(0)) as u64;
            self.finished.store(false, Ordering::SeqCst);
            player
                .try_seek(Duration::from_millis(target))
                .map_err(|error| anyhow::anyhow!("no se pudo buscar en el audiolibro: {error}"))?;
            self.snapshot()
        }

        pub fn snapshot(&self) -> Result<Option<PlaybackSnapshot>> {
            let (_, player) = self.output()?;
            let current = self
                .current
                .lock()
                .map_err(|_| anyhow::anyhow!("reproductor de audiolibros bloqueado"))?;
            let Some(current) = current.as_ref() else {
                return Ok(None);
            };
            let duration = current.manifest.duration_ms.max(0);
            let finished = player.empty() && !player.is_paused();
            let position = if finished {
                duration
            } else {
                player.get_pos().as_millis() as i64
            };
            if finished {
                self.finished.store(true, Ordering::SeqCst);
            }
            let playing = !player.is_paused() && !finished;
            let mut snapshot = PlaybackSnapshot::from_manifest(
                current.audiobook_id.clone(),
                &current.manifest,
                position,
                playing,
            );
            snapshot.cover_path = current.cover_path.clone();
            Ok(Some(snapshot))
        }

        /// Indica si el audio terminó de forma natural. El comando que sondea
        /// guarda el progreso y después llama a `clear_finished`.
        pub fn finished(&self) -> bool {
            self.finished.load(Ordering::SeqCst)
        }

        pub fn clear_finished(&self) {
            self.finished.store(false, Ordering::SeqCst);
        }

        pub fn position_ms(&self) -> i64 {
            self.output
                .lock()
                .ok()
                .and_then(|slot| slot.as_ref().map(|output| output.player.get_pos()))
                .map(|position| position.as_millis() as i64)
                .unwrap_or(0)
        }
    }
}

#[cfg(target_os = "android")]
mod android {
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use anyhow::{Context, Result, anyhow};
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use tauri::ipc::{Channel, InvokeResponseBody};
    use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
    use tauri::{AppHandle, Manager, Wry};

    use super::PlaybackSnapshot;
    use crate::audiobooks::manifest::AudiobookManifest;

    const PLUGIN_NAME: &str = "audiobook-audio";
    const PLUGIN_PACKAGE: &str = "dev.kora.reader.audio";
    const PLUGIN_CLASS: &str = "AudiobookAudioPlugin";
    const PROGRESS_WRITE_INTERVAL_MS: i64 = 5_000;
    const PROGRESS_MIN_DELTA_MS: i64 = 3_000;

    static HANDLE: OnceLock<PluginHandle<Wry>> = OnceLock::new();
    static EVENT_CHANNEL: OnceLock<Channel<NativePlaybackState>> = OnceLock::new();
    static RELAY_READY: AtomicBool = AtomicBool::new(false);
    static FINISHED: AtomicBool = AtomicBool::new(false);
    static KEEP_SCREEN_ON: AtomicBool = AtomicBool::new(false);
    static LAST_PERSIST_AT: AtomicI64 = AtomicI64::new(0);
    static LAST_PERSIST_POSITION: AtomicI64 = AtomicI64::new(-1);

    fn current() -> &'static Mutex<Option<Current>> {
        static CURRENT: OnceLock<Mutex<Option<Current>>> = OnceLock::new();
        CURRENT.get_or_init(|| Mutex::new(None))
    }

    fn speed_slot() -> &'static Mutex<f32> {
        static SPEED: OnceLock<Mutex<f32>> = OnceLock::new();
        SPEED.get_or_init(|| Mutex::new(1.0))
    }

    #[derive(Debug, Clone)]
    struct Current {
        audiobook_id: String,
        manifest: AudiobookManifest,
        cover_path: Option<String>,
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct PlayArgs<'a> {
        audiobook_id: &'a str,
        src: String,
        title: &'a str,
        artist: Option<&'a str>,
        cover_path: Option<&'a str>,
        start_ms: i64,
        speed: f32,
        chapters_json: String,
        keep_screen_on: bool,
    }

    /// Estado que envía el servicio Android. Se usa tanto para las respuestas de
    /// los comandos como para el canal de eventos periódicos.
    #[derive(Debug, Clone, Default, Serialize, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct NativePlaybackState {
        #[serde(default)]
        pub event: String,
        #[serde(default)]
        pub audiobook_id: Option<String>,
        #[serde(default)]
        pub position_ms: i64,
        #[serde(default)]
        pub duration_ms: i64,
        #[serde(default)]
        pub playing: bool,
        #[serde(default)]
        pub ended: bool,
        #[serde(default)]
        pub speed: f32,
        #[serde(default)]
        pub chapter_index: i64,
        #[serde(default)]
        pub chapter_title: Option<String>,
        #[serde(default)]
        pub sleep_remaining_ms: i64,
        #[serde(default)]
        pub error: Option<String>,
    }

    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0)
    }

    fn ensure_event_relay() -> Result<()> {
        if RELAY_READY.load(Ordering::SeqCst) {
            return Ok(());
        }
        let handle = HANDLE
            .get()
            .ok_or_else(|| anyhow!("el plugin de audio de Android no está inicializado"))?;
        let channel = EVENT_CHANNEL
            .get()
            .ok_or_else(|| anyhow!("el canal de eventos del reproductor no está inicializado"))?;

        #[derive(Serialize)]
        struct RelayArgs<'a> {
            channel: &'a Channel<NativePlaybackState>,
        }

        handle
            .run_mobile_plugin::<serde_json::Value>("setupEventRelay", RelayArgs { channel })
            .map_err(|error| anyhow!("reproductor Android: {error}"))?;
        RELAY_READY.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn call<T: DeserializeOwned>(command: &str, payload: impl Serialize) -> Result<T> {
        ensure_event_relay()?;
        let handle = HANDLE
            .get()
            .ok_or_else(|| anyhow!("el plugin de audio de Android no está inicializado"))?;
        handle
            .run_mobile_plugin(command, payload)
            .map_err(|error| anyhow!("reproductor Android: {error}"))
    }

    /// Decodifica un fragmento del audiolibro a un WAV mono de 16 kHz usando
    /// `MediaExtractor` + `MediaCodec`. Bloquea hasta que el plugin nativo
    /// termina; debe llamarse desde una tarea bloqueante.
    pub fn decode_audio_range(
        input: &Path,
        output: &Path,
        start_ms: i64,
        duration_ms: i64,
    ) -> Result<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            input_path: String,
            output_path: String,
            start_ms: i64,
            duration_ms: i64,
        }

        let handle = HANDLE
            .get()
            .ok_or_else(|| anyhow!("el plugin de audio de Android no está inicializado"))?;
        let args = Args {
            input_path: input.to_string_lossy().to_string(),
            output_path: output.to_string_lossy().to_string(),
            start_ms,
            duration_ms,
        };
        let _: serde_json::Value = handle
            .run_mobile_plugin("decodeAudioRange", args)
            .map_err(|error| anyhow!("decodificador Android: {error}"))?;
        Ok(())
    }

    pub fn start_transcription_service(title: &str) -> Result<()> {
        let handle = HANDLE
            .get()
            .ok_or_else(|| anyhow!("el plugin de audio de Android no está inicializado"))?;
        let _: serde_json::Value = handle
            .run_mobile_plugin("startTranscription", serde_json::json!({ "title": title }))
            .map_err(|error| anyhow!("servicio de transcripción: {error}"))?;
        Ok(())
    }

    pub fn update_transcription_service(percent: f64, message: &str) -> Result<()> {
        let handle = HANDLE
            .get()
            .ok_or_else(|| anyhow!("el plugin de audio de Android no está inicializado"))?;
        let _: serde_json::Value = handle
            .run_mobile_plugin(
                "updateTranscription",
                serde_json::json!({ "percent": percent, "message": message }),
            )
            .map_err(|error| anyhow!("servicio de transcripción: {error}"))?;
        Ok(())
    }

    pub fn stop_transcription_service() -> Result<()> {
        let handle = HANDLE
            .get()
            .ok_or_else(|| anyhow!("el plugin de audio de Android no está inicializado"))?;
        let _: serde_json::Value = handle
            .run_mobile_plugin("stopTranscription", serde_json::json!({}))
            .map_err(|error| anyhow!("servicio de transcripción: {error}"))?;
        Ok(())
    }

    fn chapters_json(manifest: &AudiobookManifest) -> Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Chapter<'a> {
            index: usize,
            title: &'a str,
            start_ms: i64,
            end_ms: i64,
        }

        let chapters: Vec<Chapter<'_>> = manifest
            .chapters
            .iter()
            .map(|chapter| Chapter {
                index: chapter.index,
                title: &chapter.title,
                start_ms: chapter.start_ms,
                end_ms: chapter.end_ms,
            })
            .collect();
        Ok(serde_json::to_string(&chapters)?)
    }

    fn snapshot_from_state(
        audiobook_id: &str,
        manifest: &AudiobookManifest,
        cover_path: Option<String>,
        state: &NativePlaybackState,
    ) -> Result<PlaybackSnapshot> {
        let position = state.position_ms.clamp(0, manifest.duration_ms.max(0));
        let mut snapshot = PlaybackSnapshot::from_manifest(
            audiobook_id.to_string(),
            manifest,
            position,
            state.playing && !state.ended,
        );
        snapshot.cover_path = cover_path;
        if state.ended {
            FINISHED.store(true, Ordering::SeqCst);
        }
        Ok(snapshot)
    }

    fn snapshot_from_native(state: NativePlaybackState) -> Result<Option<PlaybackSnapshot>> {
        if let Some(error) = state
            .error
            .as_deref()
            .map(str::trim)
            .filter(|error| !error.is_empty())
        {
            anyhow::bail!("reproductor Android: {error}");
        }

        let guard = current()
            .lock()
            .map_err(|_| anyhow!("estado del reproductor de audiolibros bloqueado"))?;
        let Some(current) = guard.as_ref() else {
            return Ok(None);
        };
        if state.audiobook_id.as_deref() != Some(current.audiobook_id.as_str()) {
            return Ok(None);
        }
        snapshot_from_state(
            &current.audiobook_id,
            &current.manifest,
            current.cover_path.clone(),
            &state,
        )
        .map(Some)
    }

    fn decode_state(body: InvokeResponseBody) -> Option<NativePlaybackState> {
        match body {
            InvokeResponseBody::Json(json) => serde_json::from_str(&json).ok(),
            InvokeResponseBody::Raw(bytes) => serde_json::from_slice(&bytes).ok(),
        }
    }

    fn handle_native_event(app: &AppHandle<Wry>, state: NativePlaybackState) {
        if state.ended {
            FINISHED.store(true, Ordering::SeqCst);
        }

        let Some(audiobook_id) = state.audiobook_id.as_deref() else {
            return;
        };

        let position = state.position_ms.max(0);
        let (current_id, chapter) = {
            let guard = match current().lock() {
                Ok(guard) => guard,
                Err(_) => return,
            };
            let Some(current) = guard.as_ref() else {
                return;
            };
            if audiobook_id != current.audiobook_id {
                return;
            }
            (
                current.audiobook_id.clone(),
                current.manifest.chapter_at(position) as i64,
            )
        };

        let now = now_ms();
        let force = state.event != "tick";
        let last_at = LAST_PERSIST_AT.load(Ordering::SeqCst);
        let last_position = LAST_PERSIST_POSITION.load(Ordering::SeqCst);
        if !force
            && now.saturating_sub(last_at) < PROGRESS_WRITE_INTERVAL_MS
            && (position - last_position).abs() < PROGRESS_MIN_DELTA_MS
        {
            return;
        }

        LAST_PERSIST_AT.store(now, Ordering::SeqCst);
        LAST_PERSIST_POSITION.store(position, Ordering::SeqCst);

        let Some(app_state) = app.try_state::<crate::state::AppState>() else {
            return;
        };
        let storage = app_state.storage.clone();
        let owner = app_state.identity.account.id.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = storage
                .audiobooks
                .save_progress(&current_id, &owner, position, chapter)
                .await
            {
                eprintln!("no se pudo guardar el progreso del audiolibro: {error}");
            }
        });
    }

    pub fn init_plugin() -> TauriPlugin<Wry> {
        Builder::new(PLUGIN_NAME)
            .setup(|app, api| {
                let handle = match api.register_android_plugin(PLUGIN_PACKAGE, PLUGIN_CLASS) {
                    Ok(handle) => handle,
                    Err(error) => {
                        eprintln!("Kora: no se pudo registrar el audio Android: {error}");
                        return Ok(());
                    }
                };
                let _ = HANDLE.set(handle);
                RELAY_READY.store(false, Ordering::SeqCst);

                let app = app.clone();
                let channel = Channel::<NativePlaybackState>::new(move |body| {
                    if let Some(state) = decode_state(body) {
                        handle_native_event(&app, state);
                    }
                    Ok(())
                });
                let _ = EVENT_CHANNEL.set(channel);
                Ok(())
            })
            .build()
    }

    pub fn set_keep_screen_on(enabled: bool) -> Result<()> {
        KEEP_SCREEN_ON.store(enabled, Ordering::SeqCst);
        let _: serde_json::Value =
            call("setKeepScreenOn", serde_json::json!({ "enabled": enabled }))?;
        Ok(())
    }

    pub fn set_sleep_timer(minutes: u32) -> Result<()> {
        let _: serde_json::Value =
            call("setSleepTimer", serde_json::json!({ "minutes": minutes }))?;
        Ok(())
    }

    #[derive(Default)]
    pub struct AudiobookPlayback;

    impl AudiobookPlayback {
        pub fn play(
            &self,
            audiobook_id: String,
            path: &Path,
            manifest: AudiobookManifest,
            start_ms: i64,
            cover_path: Option<String>,
        ) -> Result<PlaybackSnapshot> {
            let existing: NativePlaybackState = call("getState", ()).unwrap_or_default();
            let can_resume = existing.audiobook_id.as_deref() == Some(audiobook_id.as_str())
                && !existing.ended
                && existing
                    .error
                    .as_deref()
                    .map(str::trim)
                    .is_none_or(str::is_empty);
            let state = if can_resume {
                if existing.playing {
                    existing
                } else {
                    call("resume", ())?
                }
            } else {
                let chapters = chapters_json(&manifest)?;
                let chapter_title = manifest
                    .chapter(manifest.chapter_at(start_ms))
                    .map(|chapter| chapter.title.clone());
                let speed = *speed_slot()
                    .lock()
                    .map_err(|_| anyhow!("velocidad del reproductor bloqueada"))?;
                let args = PlayArgs {
                    audiobook_id: &audiobook_id,
                    src: path.to_string_lossy().to_string(),
                    title: &manifest.book_title,
                    artist: chapter_title.as_deref(),
                    cover_path: cover_path.as_deref(),
                    start_ms: start_ms.max(0),
                    speed,
                    chapters_json: chapters,
                    keep_screen_on: KEEP_SCREEN_ON.load(Ordering::SeqCst),
                };
                call("play", &args)
                    .with_context(|| "no se pudo iniciar el audiolibro en Android")?
            };

            if let Some(error) = state
                .error
                .as_deref()
                .map(str::trim)
                .filter(|error| !error.is_empty())
            {
                anyhow::bail!("reproductor Android: {error}");
            }

            *current()
                .lock()
                .map_err(|_| anyhow!("reproductor de audiolibros bloqueado"))? = Some(Current {
                audiobook_id: audiobook_id.clone(),
                manifest: manifest.clone(),
                cover_path: cover_path.clone(),
            });
            FINISHED.store(false, Ordering::SeqCst);
            let snapshot = snapshot_from_state(&audiobook_id, &manifest, cover_path, &state)?;
            if state.speed > 0.0
                && let Ok(mut speed) = speed_slot().lock()
            {
                *speed = state.speed.clamp(0.5, 2.5);
            }
            Ok(snapshot)
        }

        pub fn pause(&self) -> Result<Option<PlaybackSnapshot>> {
            let state: NativePlaybackState = call("pause", ())?;
            snapshot_from_native(state)
        }

        pub fn resume(&self) -> Result<Option<PlaybackSnapshot>> {
            let state: NativePlaybackState = call("resume", ())?;
            snapshot_from_native(state)
        }

        pub fn stop(&self) -> Result<()> {
            let result = call::<NativePlaybackState>("stop", ());
            *current()
                .lock()
                .map_err(|_| anyhow!("reproductor de audiolibros bloqueado"))? = None;
            FINISHED.store(false, Ordering::SeqCst);
            LAST_PERSIST_AT.store(0, Ordering::SeqCst);
            LAST_PERSIST_POSITION.store(-1, Ordering::SeqCst);
            result.map(|_| ())
        }

        pub fn set_speed(&self, speed: f32) -> Result<Option<PlaybackSnapshot>> {
            let speed = speed.clamp(0.5, 2.5);
            if let Ok(mut slot) = speed_slot().lock() {
                *slot = speed;
            }
            let state: NativePlaybackState =
                call("setSpeed", serde_json::json!({ "speed": speed }))?;
            snapshot_from_native(state)
        }

        pub fn seek(&self, position_ms: i64) -> Result<Option<PlaybackSnapshot>> {
            let state: NativePlaybackState = call(
                "seek",
                serde_json::json!({ "positionMs": position_ms.max(0) }),
            )?;
            LAST_PERSIST_AT.store(0, Ordering::SeqCst);
            snapshot_from_native(state)
        }

        pub fn snapshot(&self) -> Result<Option<PlaybackSnapshot>> {
            let state = match call::<NativePlaybackState>("getState", ()) {
                Ok(state) => state,
                Err(_) => return Ok(None),
            };
            snapshot_from_native(state)
        }

        pub fn position_ms(&self) -> i64 {
            call::<NativePlaybackState>("getState", ())
                .map(|state| state.position_ms.max(0))
                .unwrap_or(0)
        }

        pub fn finished(&self) -> bool {
            FINISHED.load(Ordering::SeqCst)
        }

        pub fn clear_finished(&self) {
            FINISHED.store(false, Ordering::SeqCst);
        }
    }
}

#[cfg(target_os = "android")]
pub use android::{
    AudiobookPlayback, decode_audio_range, init_plugin as init_mobile_plugin, set_keep_screen_on,
    set_sleep_timer, start_transcription_service, stop_transcription_service,
    update_transcription_service,
};
#[cfg(not(target_os = "android"))]
pub use desktop::AudiobookPlayback;

#[cfg(not(target_os = "android"))]
pub fn init_mobile_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    unreachable!("el plugin de audio móvil solo se registra en Android")
}

#[cfg(not(target_os = "android"))]
pub fn set_keep_screen_on(_enabled: bool) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub fn set_sleep_timer(_minutes: u32) -> anyhow::Result<()> {
    Ok(())
}
