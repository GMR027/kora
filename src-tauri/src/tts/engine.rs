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
    pub available: bool,
    /// `true` si el motor puede pausar y continuar la frase actual sin
    /// reiniciarla (Piper). Android TTS no lo permite.
    pub supports_pause: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceDescriptor {
    pub id: String,
    pub name: String,
    pub language: String,
    pub gender: Option<String>,
    pub engine: String,
    pub installed: bool,
    pub size_bytes: u64,
    /// `true` para perfiles clonados; la UI los marca y aplica la marca de agua.
    pub cloned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthesisRequest {
    pub text: String,
    pub voice_id: String,
    pub rate: f32,
    pub pitch: f32,
}

/// Petición de locución completa. El motor sintetiza y reproduce; la sesión
/// solo se encarga de encadenar frases y emitir eventos.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakRequest {
    pub text: String,
    pub voice_id: String,
    pub language: Option<String>,
    pub rate: f32,
    pub pitch: f32,
    pub volume: f32,
    /// Multiplicador de expresividad (1.0 = neutro).
    pub emphasis: f32,
}

/// Bloque de audio PCM listo para el sink.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub pcm: Vec<i16>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Motor de síntesis y reproducción.
#[async_trait]
pub trait TtsEngine: Send + Sync {
    fn info(&self) -> EngineInfo;
    async fn voices(&self) -> Result<Vec<VoiceDescriptor>>;

    /// Síntesis sin reproducir, para previsualizaciones y tests.
    async fn synthesize(&self, request: &SynthesisRequest) -> Result<AudioChunk>;

    /// Sintetiza y reproduce; vuelve cuando termina, se pausa o se detiene.
    async fn speak(&self, request: &SpeakRequest) -> Result<()>;

    async fn pause(&self) -> Result<()>;
    async fn resume(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
}
