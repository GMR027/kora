use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::error::{KoraError, Result};

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
    /// `true` si el motor permite sintetizar una frase por delante y
    /// reproducir después el PCM ya preparado. Lo usa Supertonic para evitar
    /// silencios entre frases.
    #[serde(default)]
    pub prefetch: bool,
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
    /// `true` para las voces del pack opcional Supertonic.
    #[serde(default)]
    pub enhanced: bool,
    /// Calidad declarada por el motor nativo (100–500). Solo se usa para
    /// curar el catálogo; no se muestra como número.
    #[serde(default)]
    pub quality: Option<u16>,
    /// Latencia declarada por el motor nativo (100–500). Menor es mejor.
    #[serde(default)]
    pub latency: Option<u16>,
    /// `true` si la voz necesita red (Google TTS la marca así).
    #[serde(default)]
    pub requires_network: bool,
    /// Etiqueta BCP-47 del motor (`es-ES`, `en-US`). Ayuda a elegir una voz
    /// por acento sin mostrar los códigos internos.
    #[serde(default)]
    pub locale: Option<String>,
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

/// Audio preparado que viaja por la cola de reproducción continua.
#[derive(Debug, Clone)]
pub struct QueuedAudio {
    pub index: usize,
    pub voice_id: String,
    pub audio: AudioChunk,
}

/// Eventos que publica el reproductor al encadenar audios preparados.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPlaybackEvent {
    Started(usize),
    Finished(usize),
}

/// Motor de síntesis y reproducción.
#[async_trait]
pub trait TtsEngine: Send + Sync {
    fn info(&self) -> EngineInfo;

    /// Consulta barata para el pipeline de prefetch. No debe recorrer el
    /// catálogo ni comprobar archivos en disco.
    fn supports_prefetch(&self) -> bool {
        false
    }

    async fn voices(&self) -> Result<Vec<VoiceDescriptor>>;

    /// Síntesis sin reproducir, para previsualizaciones y tests.
    async fn synthesize(&self, request: &SynthesisRequest) -> Result<AudioChunk>;

    /// Reproduce un bloque que ya se sintetizó con [`Self::synthesize`]. Los
    /// motores con `prefetch` lo usan para encadenar frases sin volver a
    /// sintetizar entre una y otra.
    async fn play_audio(&self, _audio: AudioChunk) -> Result<()> {
        Err(KoraError::Unsupported(
            "este motor no permite reproducir audio preparado".into(),
        ))
    }

    /// Reproduce una cola de audios sin recrear el reproductor entre frases.
    /// Lo usa Android para eliminar los huecos entre frases.
    fn supports_stream(&self) -> bool {
        false
    }

    async fn stream_audio(
        &self,
        _items: mpsc::Receiver<QueuedAudio>,
        _events: mpsc::Sender<AudioPlaybackEvent>,
    ) -> Result<()> {
        Err(KoraError::Unsupported(
            "este motor no permite reproducir una cola continua".into(),
        ))
    }

    /// Sintetiza y reproduce; vuelve cuando termina, se pausa o se detiene.
    async fn speak(&self, request: &SpeakRequest) -> Result<()>;

    async fn pause(&self) -> Result<()>;
    async fn resume(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
}
