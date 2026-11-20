use async_trait::async_trait;

use crate::error::{KoraError, Result};

/// Embeddings para búsqueda semántica y recomendaciones dentro de la
/// biblioteca. El modelo se cargará en la fase de features.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn dimensions(&self) -> usize;
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

pub struct OnnxEmbeddingProvider;

#[async_trait]
impl EmbeddingProvider for OnnxEmbeddingProvider {
    fn id(&self) -> &'static str {
        "onnx"
    }

    fn dimensions(&self) -> usize {
        384
    }

    async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Err(KoraError::not_implemented(
            "features::OnnxEmbeddingProvider::embed (fase 7)",
        ))
    }
}
