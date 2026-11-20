use async_trait::async_trait;

use crate::error::{KoraError, Result};

pub type DocId = String;

/// Documentos CRDT replicados (progreso, anotaciones, ajustes compartidos).
/// Los updates son binarios para viajar por QUIC sin re-serializar JSON.
#[async_trait]
pub trait CrdtEngine: Send + Sync {
    async fn apply_update(&self, document: &DocId, update: &[u8]) -> Result<()>;
    async fn snapshot(&self, document: &DocId) -> Result<Vec<u8>>;
    async fn merge(&self, left: &[u8], right: &[u8]) -> Result<Vec<u8>>;
}

/// Envoltorio previsto para `yrs` (Yjs en Rust). El almacenamiento interno
/// pasará de bytes opacos a `yrs::Doc` en la fase de sincronización.
pub struct YrsEngine;

/// Documento CRDT de `yrs` (fase 6).
pub type YrsDocument = yrs::Doc;

impl YrsEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for YrsEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CrdtEngine for YrsEngine {
    async fn apply_update(&self, _document: &DocId, _update: &[u8]) -> Result<()> {
        Err(KoraError::not_implemented(
            "sync::YrsEngine::apply_update (fase 6)",
        ))
    }

    async fn snapshot(&self, _document: &DocId) -> Result<Vec<u8>> {
        Err(KoraError::not_implemented(
            "sync::YrsEngine::snapshot (fase 6)",
        ))
    }

    async fn merge(&self, _left: &[u8], _right: &[u8]) -> Result<Vec<u8>> {
        Err(KoraError::not_implemented("sync::YrsEngine::merge"))
    }
}
