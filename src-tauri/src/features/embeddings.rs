use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::error::Result;

pub const EMBEDDING_MODEL_ID: &str = "multilingual-e5-small";
pub const EMBEDDING_DIMENSIONS: usize = 384;

/// Proveedor de embeddings. La implementación real usa `fastembed` en
/// escritorio; Android devuelve un error claro hasta empaquetar onnxruntime.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn dimensions(&self) -> usize;
    fn ready(&self) -> bool;
    async fn prepare(&self) -> Result<()>;
    async fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>>;
}

pub fn normalize(mut vector: Vec<f32>) -> Vec<f32> {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

#[cfg(not(target_os = "android"))]
mod platform {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use async_trait::async_trait;
    use tokio::sync::Mutex;

    use super::{EMBEDDING_DIMENSIONS, EMBEDDING_MODEL_ID, EmbeddingProvider, normalize};
    use crate::error::{KoraError, Result};

    const BATCH_SIZE: usize = 32;
    const READY_MARKER: &str = ".kora-model-ready";

    pub struct FastEmbedProvider {
        cache_dir: PathBuf,
        ready: AtomicBool,
        model: Mutex<Option<Arc<std::sync::Mutex<fastembed::TextEmbedding>>>>,
    }

    impl FastEmbedProvider {
        pub fn new(cache_dir: PathBuf) -> Self {
            let ready = cache_dir.join(READY_MARKER).is_file();
            Self {
                cache_dir,
                ready: AtomicBool::new(ready),
                model: Mutex::new(None),
            }
        }

        async fn model(&self) -> Result<Arc<std::sync::Mutex<fastembed::TextEmbedding>>> {
            let mut slot = self.model.lock().await;
            if let Some(model) = slot.as_ref().cloned() {
                return Ok(model);
            }

            let cache_dir = self.cache_dir.clone();
            let model = tokio::task::spawn_blocking(move || {
                std::fs::create_dir_all(&cache_dir)?;
                let options =
                    fastembed::TextInitOptions::new(fastembed::EmbeddingModel::MultilingualE5Small)
                        .with_cache_dir(cache_dir.clone())
                        .with_environment(piper_rs::environment().clone())
                        .with_show_download_progress(false);

                let model = fastembed::TextEmbedding::try_new(options).map_err(|error| {
                    KoraError::Storage(format!(
                        "no se pudo cargar el modelo de embeddings: {error}"
                    ))
                })?;
                std::fs::write(cache_dir.join(READY_MARKER), b"ready")?;
                Ok::<_, KoraError>(model)
            })
            .await
            .map_err(|error| KoraError::Internal(format!("tarea del modelo: {error}")))??;

            let model = Arc::new(std::sync::Mutex::new(model));
            *slot = Some(model.clone());
            self.ready.store(true, Ordering::Release);
            Ok(model)
        }

        async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
            if texts.is_empty() {
                return Ok(Vec::new());
            }
            let model = self.model().await?;
            tokio::task::spawn_blocking(move || {
                let mut model = model
                    .lock()
                    .map_err(|_| KoraError::Internal("modelo de embeddings bloqueado".into()))?;
                let embeddings = model
                    .embed(&texts, Some(BATCH_SIZE))
                    .map_err(|error| KoraError::Internal(format!("embeddings: {error}")))?;

                if embeddings
                    .iter()
                    .any(|embedding| embedding.len() != EMBEDDING_DIMENSIONS)
                {
                    return Err(KoraError::Internal(
                        "el modelo devolvió una dimensión inesperada".into(),
                    ));
                }

                Ok(embeddings.into_iter().map(normalize).collect())
            })
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de embeddings: {error}")))?
        }
    }

    #[async_trait]
    impl EmbeddingProvider for FastEmbedProvider {
        fn id(&self) -> &'static str {
            EMBEDDING_MODEL_ID
        }

        fn dimensions(&self) -> usize {
            EMBEDDING_DIMENSIONS
        }

        fn ready(&self) -> bool {
            self.ready.load(Ordering::Acquire)
        }

        async fn prepare(&self) -> Result<()> {
            if self.ready() && self.model.lock().await.is_some() {
                return Ok(());
            }
            self.model().await?;
            Ok(())
        }

        async fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            let texts = texts
                .iter()
                .map(|text| format!("passage: {text}"))
                .collect();
            self.embed(texts).await
        }

        async fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
            let mut embeddings = self.embed(vec![format!("query: {text}")]).await?;
            embeddings
                .pop()
                .ok_or_else(|| KoraError::Internal("el modelo no devolvió la consulta".into()))
        }
    }
}

#[cfg(target_os = "android")]
mod platform {
    use async_trait::async_trait;

    use super::{EMBEDDING_DIMENSIONS, EMBEDDING_MODEL_ID, EmbeddingProvider};
    use crate::error::{KoraError, Result};

    pub struct FastEmbedProvider;

    impl FastEmbedProvider {
        pub fn new(_cache_dir: std::path::PathBuf) -> Self {
            Self
        }
    }

    #[async_trait]
    impl EmbeddingProvider for FastEmbedProvider {
        fn id(&self) -> &'static str {
            EMBEDDING_MODEL_ID
        }

        fn dimensions(&self) -> usize {
            EMBEDDING_DIMENSIONS
        }

        fn ready(&self) -> bool {
            false
        }

        async fn prepare(&self) -> Result<()> {
            Err(KoraError::Unsupported(
                "la búsqueda semántica aún no está disponible en Android".into(),
            ))
        }

        async fn embed_passages(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Err(KoraError::Unsupported(
                "la búsqueda semántica aún no está disponible en Android".into(),
            ))
        }

        async fn embed_query(&self, _text: &str) -> Result<Vec<f32>> {
            Err(KoraError::Unsupported(
                "la búsqueda semántica aún no está disponible en Android".into(),
            ))
        }
    }
}

pub fn platform_provider(cache_dir: PathBuf) -> Arc<dyn EmbeddingProvider> {
    Arc::new(platform::FastEmbedProvider::new(cache_dir))
}

/// Proveedor determinista para tests. No descarga nada y permite probar el
/// indexador y el ranking sin red.
pub struct TestEmbeddingProvider;

#[async_trait]
impl EmbeddingProvider for TestEmbeddingProvider {
    fn id(&self) -> &'static str {
        EMBEDDING_MODEL_ID
    }

    fn dimensions(&self) -> usize {
        EMBEDDING_DIMENSIONS
    }

    fn ready(&self) -> bool {
        true
    }

    async fn prepare(&self) -> Result<()> {
        Ok(())
    }

    async fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|text| test_vector(text)).collect())
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        Ok(test_vector(text))
    }
}

/// Vector bag-of-words muy simple: suficiente para comprobar que el ranking
/// ordena primero los textos que comparten términos con la consulta.
pub fn test_vector(text: &str) -> Vec<f32> {
    let mut vector = vec![0.0; EMBEDDING_DIMENSIONS];
    for word in text.split_whitespace() {
        let hash = word
            .chars()
            .fold(0usize, |acc, ch| acc.wrapping_mul(31) + ch as usize);
        vector[hash % EMBEDDING_DIMENSIONS] += 1.0;
    }
    normalize(vector)
}
