pub mod audio;
pub mod engine;
pub mod profiles;

pub use audio::{AudioSink, NullAudioSink};
pub use engine::{AudioChunk, EngineInfo, SynthesisRequest, TtsEngine, VoiceDescriptor};
pub use profiles::{FileVoiceProfileStore, VoiceCloner, VoiceProfile, VoiceProfileStore};

use std::sync::Arc;

use crate::error::{KoraError, Result};

/// Registro de motores de voz. En esta fase arranca vacío: los motores
/// concretos (Android TTS, Piper, ONNX, remoto) se añaden en la fase de TTS.
#[derive(Default)]
pub struct TtsService {
    engines: Vec<Arc<dyn TtsEngine>>,
}

impl TtsService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, engine: Arc<dyn TtsEngine>) {
        self.engines.push(engine);
    }

    pub fn engines(&self) -> Vec<EngineInfo> {
        self.engines.iter().map(|engine| engine.info()).collect()
    }

    pub async fn voices(&self) -> Result<Vec<VoiceDescriptor>> {
        let mut voices = Vec::new();
        for engine in &self.engines {
            voices.extend(engine.voices().await?);
        }
        Ok(voices)
    }

    pub async fn synthesize(&self, request: &SynthesisRequest) -> Result<AudioChunk> {
        match self.engines.first() {
            Some(engine) => engine.synthesize(request).await,
            None => Err(KoraError::not_implemented(
                "tts::TtsService::synthesize sin motores registrados",
            )),
        }
    }
}
