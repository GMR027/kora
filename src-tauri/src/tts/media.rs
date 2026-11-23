use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Metadatos que se muestran en la tarjeta de reproducción del sistema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsMediaMetadata {
    /// Título del libro.
    pub title: String,
    /// Capítulo o página actual.
    pub artist: String,
    /// Autor u otro subtítulo secundario.
    pub album: Option<String>,
    /// Ruta local de la portada, si existe.
    pub cover_path: Option<String>,
}

/// Estado que se publica en la sesión multimedia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "error")]
pub enum TtsMediaState {
    Playing,
    Paused,
    Stopped,
    Finished,
    Error(String),
}

impl TtsMediaState {
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Error(message) => Some(message),
            _ => None,
        }
    }
}

/// Puente opcional entre la sesión TTS y el reproductor multimedia del sistema.
///
/// En escritorio no hay implementación y todos los métodos son no-ops. En
/// Android lo implementa el motor nativo, que publica metadatos y estado en un
/// `MediaSessionService`.
#[async_trait]
pub trait TtsMediaSession: Send + Sync {
    async fn start(&self, metadata: &TtsMediaMetadata) -> Result<()>;
    async fn set_state(&self, state: TtsMediaState) -> Result<()>;
    async fn stop(&self) -> Result<()>;
}
