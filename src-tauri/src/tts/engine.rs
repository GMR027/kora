use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub id: String,
    pub name: String,
    pub offline: bool,
    pub voice_cloning: bool,
    pub streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceDescriptor {
    pub id: String,
    pub name: String,
    pub language: String,
    pub gender: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthesisRequest {
    pub text: String,
    pub voice_id: String,
    pub rate: f32,
    pub pitch: f32,
}

/// Bloque de audio PCM listo para el sink. Se mantiene independiente del
/// motor para que Android TTS, Piper/ONNX o un servidor remoto compartan tipo.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub pcm: Vec<i16>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Motor de síntesis. La reproducción se hace con `audio::AudioSink`.
#[async_trait]
pub trait TtsEngine: Send + Sync {
    fn info(&self) -> EngineInfo;
    async fn voices(&self) -> Result<Vec<VoiceDescriptor>>;
    async fn synthesize(&self, request: &SynthesisRequest) -> Result<AudioChunk>;
}
