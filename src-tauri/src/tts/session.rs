use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::{Mutex, Notify};

use super::TtsService;
use super::engine::SpeakRequest;
use super::expressive::{NARRATOR, SpeechChunk, SpeechSequence};
use crate::error::{KoraError, Result};
use crate::sync::MirrorHub;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsState {
    Idle,
    Playing,
    Paused,
    Finished,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsStatus {
    pub book_id: Option<String>,
    pub section: Option<usize>,
    pub state: TtsState,
    pub rate: f32,
    pub pitch: f32,
    pub chunk_index: Option<usize>,
    pub error: Option<String>,
    /// La sesión reproduce solo un extracto seleccionado.
    pub excerpt: bool,
}

impl TtsStatus {
    fn idle() -> Self {
        Self {
            book_id: None,
            section: None,
            state: TtsState::Idle,
            rate: 1.0,
            pitch: 1.0,
            chunk_index: None,
            error: None,
            excerpt: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhraseEvent {
    pub book_id: String,
    pub section: usize,
    pub offset_start: usize,
    pub offset_end: usize,
    pub chunk_index: usize,
    pub unit: String,
    pub character: Option<String>,
    pub voice_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEvent {
    pub book_id: String,
    pub section: usize,
    pub state: TtsState,
    pub error: Option<String>,
    pub excerpt: bool,
}

pub struct SessionPlan {
    pub book_id: String,
    pub section: usize,
    pub sequence: Arc<SpeechSequence>,
    pub start_index: usize,
    pub language: Option<String>,
    pub default_voice: String,
    pub character_voices: HashMap<String, String>,
    pub rate: f32,
    pub pitch: f32,
    /// Fin exclusivo del extracto. `None` significa leer hasta el final de la
    /// sección.
    pub excerpt_end: Option<usize>,
}

struct RuntimeState {
    state: TtsState,
    chunk_index: Option<usize>,
    error: Option<String>,
}

struct Control {
    runtime: Mutex<RuntimeState>,
    cancelled: AtomicBool,
    paused: AtomicBool,
    rate: AtomicU32,
    pitch: AtomicU32,
    voices: RwLock<HashMap<String, String>>,
    notify: Notify,
}

impl Control {
    fn new(rate: f32, pitch: f32, voices: HashMap<String, String>) -> Self {
        Self {
            runtime: Mutex::new(RuntimeState {
                state: TtsState::Playing,
                chunk_index: None,
                error: None,
            }),
            cancelled: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            rate: AtomicU32::new(rate.to_bits()),
            pitch: AtomicU32::new(pitch.to_bits()),
            voices: RwLock::new(voices),
            notify: Notify::new(),
        }
    }

    fn rate(&self) -> f32 {
        f32::from_bits(self.rate.load(Ordering::Relaxed))
    }

    fn pitch(&self) -> f32 {
        f32::from_bits(self.pitch.load(Ordering::Relaxed))
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.notify.notify_one();
    }

    fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    async fn set_state(&self, state: TtsState) {
        self.paused
            .store(state == TtsState::Paused, Ordering::SeqCst);
        self.runtime.lock().await.state = state;
    }

    async fn set_error(&self, error: String) {
        let mut runtime = self.runtime.lock().await;
        runtime.state = TtsState::Error;
        runtime.error = Some(error);
    }

    async fn set_chunk(&self, index: usize) {
        self.runtime.lock().await.chunk_index = Some(index);
    }

    async fn wait_while_paused(&self) {
        loop {
            if !self.is_paused() {
                return;
            }
            self.notify.notified().await;
        }
    }

    async fn sleep_pausable(&self, milliseconds: u32) -> bool {
        if milliseconds == 0 {
            return !self.is_cancelled();
        }

        let sleep = tokio::time::sleep(Duration::from_millis(milliseconds as u64));
        tokio::pin!(sleep);

        loop {
            tokio::select! {
                _ = sleep.as_mut() => return !self.is_cancelled(),
                _ = self.notify.notified() => {
                    if self.is_cancelled() {
                        return false;
                    }
                    if self.is_paused() {
                        self.wait_while_paused().await;
                    }
                }
            }
        }
    }

    fn voice_for(&self, character: Option<&str>, default_voice: &str) -> String {
        let key = character.unwrap_or(NARRATOR);
        self.voices
            .read()
            .ok()
            .and_then(|voices| voices.get(key).cloned())
            .unwrap_or_else(|| default_voice.to_string())
    }

    async fn set_voice(&self, character: &str, voice_id: &str) {
        if let Ok(mut voices) = self.voices.write() {
            voices.insert(character.to_string(), voice_id.to_string());
        }
    }

    async fn status(&self, book_id: &str, section: usize, excerpt: bool) -> TtsStatus {
        let runtime = self.runtime.lock().await;
        TtsStatus {
            book_id: Some(book_id.to_string()),
            section: Some(section),
            state: runtime.state,
            rate: self.rate(),
            pitch: self.pitch(),
            chunk_index: runtime.chunk_index,
            error: runtime.error.clone(),
            excerpt,
        }
    }
}

struct ActiveSession {
    book_id: String,
    section: usize,
    excerpt_end: Option<usize>,
    control: Arc<Control>,
    handle: tauri::async_runtime::JoinHandle<()>,
}

pub struct TtsSessionManager {
    service: Arc<TtsService>,
    active: Mutex<Option<ActiveSession>>,
    mirror: Option<MirrorHub>,
}

impl TtsSessionManager {
    pub fn new(service: Arc<TtsService>) -> Self {
        Self {
            service,
            active: Mutex::new(None),
            mirror: None,
        }
    }

    pub fn with_mirror(service: Arc<TtsService>, mirror: MirrorHub) -> Self {
        Self {
            service,
            active: Mutex::new(None),
            mirror: Some(mirror),
        }
    }

    pub async fn start<R: Runtime>(
        &self,
        app: AppHandle<R>,
        plan: SessionPlan,
    ) -> Result<TtsStatus> {
        self.stop_active().await;

        if plan.default_voice.trim().is_empty() {
            return Err(KoraError::Unsupported(
                "no hay una voz instalada para leer este libro".into(),
            ));
        }

        let control = Arc::new(Control::new(
            plan.rate.clamp(0.5, 3.0),
            plan.pitch.clamp(0.5, 1.5),
            plan.character_voices.clone(),
        ));

        let book_id = plan.book_id.clone();
        let section = plan.section;
        let excerpt_end = plan.excerpt_end;
        let start_offset = plan
            .sequence
            .chunks
            .get(plan.start_index)
            .map(|chunk| chunk.offset_start)
            .unwrap_or(0);
        let status = control
            .status(&book_id, section, excerpt_end.is_some())
            .await;

        emit_session(
            &app,
            &book_id,
            section,
            TtsState::Playing,
            None,
            start_offset,
            excerpt_end,
            self.mirror.as_ref(),
        );

        let service = self.service.clone();
        let mirror = self.mirror.clone();
        let control_for_task = control.clone();
        let plan_for_task = plan;
        let app_for_task = app.clone();
        let handle = tauri::async_runtime::spawn(async move {
            run_session(
                app_for_task,
                service,
                plan_for_task,
                control_for_task,
                mirror,
            )
            .await;
        });

        *self.active.lock().await = Some(ActiveSession {
            book_id,
            section,
            excerpt_end,
            control,
            handle,
        });

        Ok(status)
    }

    pub async fn pause<R: Runtime>(&self, app: &AppHandle<R>) -> Result<TtsStatus> {
        let active = self.active.lock().await;
        let Some(active) = active.as_ref() else {
            return Ok(TtsStatus::idle());
        };

        active.control.set_state(TtsState::Paused).await;
        active.control.notify.notify_one();
        self.service.pause().await?;
        emit_session(
            app,
            &active.book_id,
            active.section,
            TtsState::Paused,
            None,
            0,
            active.excerpt_end,
            self.mirror.as_ref(),
        );
        Ok(active
            .control
            .status(
                &active.book_id,
                active.section,
                active.excerpt_end.is_some(),
            )
            .await)
    }

    pub async fn resume<R: Runtime>(&self, app: &AppHandle<R>) -> Result<TtsStatus> {
        let active = self.active.lock().await;
        let Some(active) = active.as_ref() else {
            return Ok(TtsStatus::idle());
        };

        active.control.set_state(TtsState::Playing).await;
        active.control.notify.notify_one();
        self.service.resume().await?;
        emit_session(
            app,
            &active.book_id,
            active.section,
            TtsState::Playing,
            None,
            0,
            active.excerpt_end,
            self.mirror.as_ref(),
        );
        Ok(active
            .control
            .status(
                &active.book_id,
                active.section,
                active.excerpt_end.is_some(),
            )
            .await)
    }

    pub async fn stop<R: Runtime>(&self, app: &AppHandle<R>) -> Result<TtsStatus> {
        let active = self.stop_active().await;
        if let Some(active) = active {
            emit_session(
                app,
                &active.book_id,
                active.section,
                TtsState::Stopped,
                None,
                0,
                active.excerpt_end,
                self.mirror.as_ref(),
            );
        }
        Ok(TtsStatus::idle())
    }

    pub async fn set_rate(&self, rate: f32) -> Result<TtsStatus> {
        if !(0.5..=3.0).contains(&rate) || !rate.is_finite() {
            return Err(KoraError::InvalidInput(
                "la velocidad debe estar entre 0.5 y 3.0".into(),
            ));
        }

        let active = self.active.lock().await;
        let Some(active) = active.as_ref() else {
            return Ok(TtsStatus::idle());
        };
        active.control.rate.store(rate.to_bits(), Ordering::Relaxed);
        Ok(active
            .control
            .status(
                &active.book_id,
                active.section,
                active.excerpt_end.is_some(),
            )
            .await)
    }

    pub async fn set_pitch(&self, pitch: f32) -> Result<TtsStatus> {
        if !(0.5..=1.5).contains(&pitch) || !pitch.is_finite() {
            return Err(KoraError::InvalidInput(
                "el tono debe estar entre 0.5 y 1.5".into(),
            ));
        }

        let active = self.active.lock().await;
        let Some(active) = active.as_ref() else {
            return Ok(TtsStatus::idle());
        };
        active
            .control
            .pitch
            .store(pitch.to_bits(), Ordering::Relaxed);
        Ok(active
            .control
            .status(
                &active.book_id,
                active.section,
                active.excerpt_end.is_some(),
            )
            .await)
    }

    pub async fn set_voice(&self, character: &str, voice_id: &str) {
        let active = self.active.lock().await;
        if let Some(active) = active.as_ref() {
            active.control.set_voice(character, voice_id).await;
        }
    }

    pub async fn status(&self) -> TtsStatus {
        let active = self.active.lock().await;
        match active.as_ref() {
            Some(active) => {
                active
                    .control
                    .status(
                        &active.book_id,
                        active.section,
                        active.excerpt_end.is_some(),
                    )
                    .await
            }
            None => TtsStatus::idle(),
        }
    }

    async fn stop_active(&self) -> Option<ActiveSession> {
        let active = self.active.lock().await.take();
        if let Some(active) = active.as_ref() {
            active.control.cancel();
            active.handle.abort();
        }
        if active.is_some() {
            let _ = self.service.stop().await;
        }
        active
    }
}

async fn run_session<R: Runtime>(
    app: AppHandle<R>,
    service: Arc<TtsService>,
    plan: SessionPlan,
    control: Arc<Control>,
    mirror: Option<MirrorHub>,
) {
    let chunks = &plan.sequence.chunks;
    let mut index = plan.start_index.min(chunks.len());

    while index < chunks.len() {
        if control.is_cancelled() {
            break;
        }
        control.wait_while_paused().await;
        if control.is_cancelled() {
            break;
        }

        let chunk = &chunks[index];
        control.set_chunk(index).await;
        let voice_id = control.voice_for(chunk.character.as_deref(), &plan.default_voice);
        let request = SpeakRequest {
            text: chunk.speech.clone(),
            voice_id: voice_id.clone(),
            language: plan.language.clone(),
            rate: (control.rate() * chunk.rate).clamp(0.5, 3.0),
            pitch: (control.pitch() * chunk.pitch).clamp(0.5, 1.5),
            volume: 1.0,
            emphasis: chunk.emphasis,
        };

        emit_phrase_start(&app, &plan, chunk, index, &voice_id, mirror.as_ref());

        match service.speak(&request).await {
            Ok(()) => {
                if control.is_cancelled() {
                    break;
                }
                emit_phrase_end(&app, &plan, chunk, index);
                index += 1;
                if !control.sleep_pausable(chunk.pause_after_ms).await {
                    break;
                }
            }
            Err(error) => {
                if control.is_cancelled() {
                    break;
                }
                if matches!(error, KoraError::Conflict(_)) {
                    // Motor sin pausa nativa (Android): repetir la frase al
                    // reanudar. `wait_while_paused` evita el bucle ocupado.
                    continue;
                }
                let message = error.to_string();
                control.set_error(message.clone()).await;
                emit_session(
                    &app,
                    &plan.book_id,
                    plan.section,
                    TtsState::Error,
                    Some(message),
                    chunk.offset_start,
                    plan.excerpt_end,
                    mirror.as_ref(),
                );
                return;
            }
        }
    }

    if control.is_cancelled() {
        control.set_state(TtsState::Stopped).await;
        emit_session(
            &app,
            &plan.book_id,
            plan.section,
            TtsState::Stopped,
            None,
            0,
            plan.excerpt_end,
            mirror.as_ref(),
        );
    } else {
        control.set_state(TtsState::Finished).await;
        emit_session(
            &app,
            &plan.book_id,
            plan.section,
            TtsState::Finished,
            None,
            0,
            plan.excerpt_end,
            mirror.as_ref(),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_session<R: Runtime>(
    app: &AppHandle<R>,
    book_id: &str,
    section: usize,
    state: TtsState,
    error: Option<String>,
    offset: usize,
    excerpt_end: Option<usize>,
    mirror: Option<&MirrorHub>,
) {
    let _ = app.emit(
        "tts::state",
        SessionEvent {
            book_id: book_id.to_string(),
            section,
            state,
            error,
            excerpt: excerpt_end.is_some(),
        },
    );
    if let Some(mirror) = mirror {
        mirror.tts(book_id, section, tts_state_str(state), offset, excerpt_end);
    }
}

fn tts_state_str(state: TtsState) -> &'static str {
    match state {
        TtsState::Idle => "idle",
        TtsState::Playing => "playing",
        TtsState::Paused => "paused",
        TtsState::Finished => "finished",
        TtsState::Stopped => "stopped",
        TtsState::Error => "error",
    }
}

fn emit_phrase_start<R: Runtime>(
    app: &AppHandle<R>,
    plan: &SessionPlan,
    chunk: &SpeechChunk,
    index: usize,
    voice_id: &str,
    mirror: Option<&MirrorHub>,
) {
    let _ = app.emit(
        "tts::phrase_start",
        PhraseEvent {
            book_id: plan.book_id.clone(),
            section: plan.section,
            offset_start: chunk.offset_start,
            offset_end: chunk.offset_end,
            chunk_index: index,
            unit: plan.sequence.unit.as_str().to_string(),
            character: chunk.character.clone(),
            voice_id: voice_id.to_string(),
            text: chunk.text.clone(),
        },
    );
    if let Some(mirror) = mirror {
        mirror.tts(
            &plan.book_id,
            plan.section,
            "playing",
            chunk.offset_start,
            plan.excerpt_end,
        );
    }
}

fn emit_phrase_end<R: Runtime>(
    app: &AppHandle<R>,
    plan: &SessionPlan,
    chunk: &SpeechChunk,
    index: usize,
) {
    let _ = app.emit(
        "tts::phrase_end",
        PhraseEvent {
            book_id: plan.book_id.clone(),
            section: plan.section,
            offset_start: chunk.offset_start,
            offset_end: chunk.offset_end,
            chunk_index: index,
            unit: plan.sequence.unit.as_str().to_string(),
            character: chunk.character.clone(),
            voice_id: String::new(),
            text: String::new(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pause_and_resume_toggle_the_gate() {
        let control = Control::new(1.0, 1.0, HashMap::new());
        control.set_state(TtsState::Paused).await;
        assert!(control.is_paused());
        control.set_state(TtsState::Playing).await;
        assert!(!control.is_paused());
    }

    #[tokio::test]
    async fn cancellation_interrupts_a_pausable_sleep() {
        let control = Arc::new(Control::new(1.0, 1.0, HashMap::new()));
        let other = control.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            other.cancel();
        });

        assert!(!control.sleep_pausable(5_000).await);
    }

    #[tokio::test]
    async fn status_marks_excerpt_sessions() {
        let control = Control::new(1.0, 1.0, HashMap::new());
        let status = control.status("libro", 3, true).await;
        assert!(status.excerpt);
        assert_eq!(status.section, Some(3));
    }

    #[test]
    fn resolves_character_voice_and_falls_back_to_narrator() {
        let control = Control::new(
            1.0,
            1.0,
            HashMap::from([("Ana".to_string(), "voz-ana".to_string())]),
        );
        assert_eq!(control.voice_for(Some("Ana"), "narrador"), "voz-ana");
        assert_eq!(control.voice_for(Some("Luis"), "narrador"), "narrador");
        assert_eq!(control.voice_for(None, "narrador"), "narrador");
    }
}
