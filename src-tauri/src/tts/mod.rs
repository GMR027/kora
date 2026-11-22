#[cfg(target_os = "android")]
pub mod android;
pub mod audio;
#[cfg(not(target_os = "android"))]
pub mod download;
pub mod engine;
pub mod expressive;
pub mod neutts;
#[cfg(not(target_os = "android"))]
pub mod piper;
pub mod practice;
pub mod recording;
pub mod session;
pub mod voice_cloner;
pub mod wav;

#[cfg(not(target_os = "android"))]
pub use audio::RodioPlayer;
pub use audio::{AudioPlayer, NullAudioPlayer};
pub use engine::{
    AudioChunk, EngineInfo, SpeakRequest, SynthesisRequest, TtsEngine, VoiceDescriptor,
};
pub use expressive::{
    CharacterInfo, DIALOGUE, ExpressiveSettings, NARRATOR, SpeechChunk, SpeechSequence,
    build_from_text, build_from_words, content_hash,
};
pub use neutts::{EncodeResult, NeuTtsEngine, phonemize};
#[cfg(not(target_os = "android"))]
pub use piper::PiperEngine;
pub use practice::{
    DiffKind, PracticeInput, PracticeManager, StoppedPractice, WHISPER_CATALOG, WordDiff,
    diff_words, find_model as find_whisper_model, model_path as whisper_model_path, whisper_dir,
};
pub use session::{PhraseEvent, SessionEvent, SessionPlan, TtsSessionManager, TtsState, TtsStatus};
pub use voice_cloner::{
    CLONE_PREFIX, ModelKind, ProfileFile, VOICE_MODELS, VoiceModelSpec, VoiceProfile,
    VoiceProfileService, clone_voice_id, models_for_language, strip_clone_prefix,
};
pub use wav::{encode_wav, encode_wav_with_watermark, read_watermark};

use std::path::PathBuf;
use std::sync::Arc;

use crate::error::{KoraError, Result};

/// Registro de motores de voz. En escritorio se registra Piper; en Android el
/// motor nativo se añadirá cuando se empaqueten las dependencias de la ABI.
#[derive(Default)]
pub struct TtsService {
    engines: Vec<Arc<dyn TtsEngine>>,
}

impl TtsService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Servicio de escritorio con Piper y sus voces en `assets/tts`.
    pub fn with_piper(root: PathBuf) -> Self {
        let mut service = Self::new();
        service.register(Arc::new(PiperEngine::new(root)));
        service
    }

    /// Elige el motor nativo de la plataforma.
    pub fn with_platform<R: tauri::Runtime>(app: &tauri::AppHandle<R>, root: PathBuf) -> Self {
        #[cfg(not(target_os = "android"))]
        {
            let _ = app;
            Self::with_piper(root)
        }

        #[cfg(target_os = "android")]
        {
            let _ = root;
            let mut service = Self::new();
            service.register(Arc::new(android::AndroidTtsEngine::new(app.clone())));
            service
        }
    }

    fn engine(&self) -> Result<&Arc<dyn TtsEngine>> {
        self.engines.first().ok_or_else(|| {
            KoraError::Unsupported("no hay ningún motor TTS disponible en esta plataforma".into())
        })
    }

    /// Enruta las voces clonadas al motor NeuTTS y las del catálogo al motor
    /// nativo (Piper en escritorio, TextToSpeech en Android).
    fn engine_for(&self, voice_id: &str) -> Result<&Arc<dyn TtsEngine>> {
        if voice_id.starts_with(CLONE_PREFIX) {
            return self
                .engines
                .iter()
                .find(|engine| engine.info().id == "neutts")
                .ok_or_else(|| {
                    KoraError::Unsupported(
                        "la clonación de voz no está disponible en esta plataforma".into(),
                    )
                });
        }
        self.engine()
    }

    pub fn register(&mut self, engine: Arc<dyn TtsEngine>) {
        self.engines.push(engine);
    }

    pub fn engines(&self) -> Vec<EngineInfo> {
        self.engines.iter().map(|engine| engine.info()).collect()
    }

    pub fn has_engines(&self) -> bool {
        !self.engines.is_empty()
    }

    pub async fn voices(&self) -> Result<Vec<VoiceDescriptor>> {
        let mut voices = Vec::new();
        for engine in &self.engines {
            voices.extend(engine.voices().await?);
        }
        Ok(voices)
    }

    pub async fn synthesize(&self, request: &SynthesisRequest) -> Result<AudioChunk> {
        self.engine_for(&request.voice_id)?
            .synthesize(request)
            .await
    }

    pub async fn speak(&self, request: &SpeakRequest) -> Result<()> {
        self.engine_for(&request.voice_id)?.speak(request).await
    }

    pub async fn pause(&self) -> Result<()> {
        for engine in &self.engines {
            let _ = engine.pause().await;
        }
        Ok(())
    }

    pub async fn resume(&self) -> Result<()> {
        for engine in &self.engines {
            let _ = engine.resume().await;
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        for engine in &self.engines {
            let _ = engine.stop().await;
        }
        Ok(())
    }
}
