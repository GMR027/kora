#[cfg(target_os = "android")]
pub mod android;
pub mod audio;
pub mod download;
pub mod engine;
pub mod expressive;
pub mod media;
pub mod neutts;
#[cfg(not(target_os = "android"))]
pub mod piper;
pub mod practice;
pub mod recording;
pub mod session;
pub mod supertonic;
pub mod voice_cloner;
pub mod wav;

#[cfg(target_os = "android")]
pub use audio::AndroidAudioPlayer;
#[cfg(not(target_os = "android"))]
pub use audio::RodioPlayer;
pub use audio::{AudioPlayer, NullAudioPlayer};
pub use engine::{
    AudioChunk, AudioPlaybackEvent, EngineInfo, QueuedAudio, SpeakRequest, SynthesisRequest,
    TtsEngine, VoiceDescriptor,
};
pub use expressive::{
    CharacterInfo, DIALOGUE, ExpressiveSettings, NARRATOR, SpeechChunk, SpeechSequence,
    build_from_text, build_from_words, content_hash, sanitize_speech,
};
pub use media::{TtsMediaMetadata, TtsMediaSession, TtsMediaState};
pub use neutts::{EncodeResult, NeuTtsEngine, phonemize};
#[cfg(not(target_os = "android"))]
pub use piper::PiperEngine;
pub use practice::{
    DiffKind, PracticeInput, PracticeManager, StoppedPractice, WHISPER_CATALOG, WordDiff,
    diff_words, find_model as find_whisper_model, model_path as whisper_model_path, whisper_dir,
};
pub use session::{PhraseEvent, SessionEvent, SessionPlan, TtsSessionManager, TtsState, TtsStatus};
pub use supertonic::{SupertonicEngine, SupertonicService, SupertonicStatus};
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
    media: Option<Arc<dyn TtsMediaSession>>,
}

impl TtsService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Servicio de escritorio con Piper y sus voces en `assets/tts`.
    #[cfg(not(target_os = "android"))]
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
            let engine = Arc::new(android::AndroidTtsEngine::new(app.clone()));
            service.register(engine.clone());
            service.media = Some(engine);
            if let Err(error) = android::install_control_relay() {
                tracing::warn!(%error, "no se pudo registrar el canal de control TTS");
            }
            service
        }
    }

    fn engine(&self) -> Result<&Arc<dyn TtsEngine>> {
        self.engines.first().ok_or_else(|| {
            KoraError::Unsupported("no hay ningún motor TTS disponible en esta plataforma".into())
        })
    }

    /// Enruta las voces clonadas a NeuTTS, las mejoradas a Supertonic y el
    /// resto al motor nativo (Piper en escritorio, TextToSpeech en Android).
    fn engine_for(&self, voice_id: &str) -> Result<&Arc<dyn TtsEngine>> {
        let required = if voice_id.starts_with(CLONE_PREFIX) {
            Some("neutts")
        } else if voice_id.starts_with(supertonic::PREFIX) {
            Some(supertonic::ENGINE_ID)
        } else {
            None
        };

        if let Some(id) = required {
            return self
                .engines
                .iter()
                .find(|engine| engine.info().id == id)
                .ok_or_else(|| {
                    KoraError::Unsupported(if id == "neutts" {
                        "la clonación de voz no está disponible en esta plataforma".into()
                    } else {
                        "las voces mejoradas no están disponibles; descarga el pack en Ajustes"
                            .into()
                    })
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
        // Kora solo ofrece lectura en español e inglés. El motor nativo de
        // Android devuelve todas las voces instaladas del sistema (coreano,
        // japonés, etc.), así que se descartan aquí para todas las pantallas.
        voices.retain(|voice| supported_voice_language(&voice.language));
        Ok(voices)
    }

    pub async fn synthesize(&self, request: &SynthesisRequest) -> Result<AudioChunk> {
        let text = sanitize_speech(&request.text);
        if text.is_empty() {
            return Err(KoraError::InvalidInput(
                "el texto no contiene nada pronunciable".into(),
            ));
        }
        let mut request = request.clone();
        request.text = text;
        self.engine_for(&request.voice_id)?
            .synthesize(&request)
            .await
    }

    /// Reproduce un bloque ya sintetizado. Solo lo implementan los motores
    /// con `prefetch`; el resto devuelve `Unsupported`.
    pub async fn play_audio(&self, voice_id: &str, audio: AudioChunk) -> Result<()> {
        self.engine_for(voice_id)?.play_audio(audio).await
    }

    /// Indica si la voz puede usar el pipeline de síntesis por delante.
    pub fn supports_prefetch(&self, voice_id: &str) -> bool {
        self.engine_for(voice_id)
            .map(|engine| engine.supports_prefetch())
            .unwrap_or(false)
    }

    /// Indica si la voz puede reproducir una cola continua sin recrear el
    /// reproductor entre frases.
    pub fn supports_stream(&self, voice_id: &str) -> bool {
        self.engine_for(voice_id)
            .map(|engine| engine.supports_stream())
            .unwrap_or(false)
    }

    pub async fn stream_audio(
        &self,
        voice_id: &str,
        items: tokio::sync::mpsc::Receiver<QueuedAudio>,
        events: tokio::sync::mpsc::Sender<AudioPlaybackEvent>,
    ) -> Result<()> {
        self.engine_for(voice_id)?.stream_audio(items, events).await
    }

    pub async fn speak(&self, request: &SpeakRequest) -> Result<()> {
        let text = sanitize_speech(&request.text);
        if text.is_empty() {
            return Ok(());
        }
        let mut request = request.clone();
        request.text = text;
        self.engine_for(&request.voice_id)?.speak(&request).await
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

    pub async fn media_start(&self, metadata: &TtsMediaMetadata) -> Result<()> {
        match self.media.as_ref() {
            Some(media) => media.start(metadata).await,
            None => Ok(()),
        }
    }

    pub async fn media_state(&self, state: TtsMediaState) -> Result<()> {
        match self.media.as_ref() {
            Some(media) => media.set_state(state).await,
            None => Ok(()),
        }
    }

    pub async fn media_stop(&self) -> Result<()> {
        match self.media.as_ref() {
            Some(media) => media.stop().await,
            None => Ok(()),
        }
    }
}

/// Idiomas de lectura soportados por Kora. El catálogo Piper, los perfiles
/// clonados y el motor nativo de Android se limitan a estos dos idiomas.
pub fn supported_voice_language(language: &str) -> bool {
    matches!(language, "es" | "en")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_spanish_and_english_are_supported() {
        assert!(supported_voice_language("es"));
        assert!(supported_voice_language("en"));
        for language in ["ko", "ja", "zh", "fr", "de", "it", "pt", "und"] {
            assert!(!supported_voice_language(language), "{language}");
        }
    }
}
