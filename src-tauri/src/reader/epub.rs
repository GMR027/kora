use std::path::Path;

use async_trait::async_trait;

use super::DocumentParser;
use super::model::Document;
use crate::error::{KoraError, Result};

/// Parser EPUB basado en el crate `epub` 2.x. La extracción de capítulos,
/// metadatos y portada se implementa en la fase de lector.
pub struct EpubParser;

/// Tipo concreto que devolverá el parser cuando se implemente.
pub type EpubDocument = epub::doc::EpubDoc<std::io::BufReader<std::fs::File>>;

impl EpubParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EpubParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DocumentParser for EpubParser {
    async fn open(&self, path: &Path) -> Result<Document> {
        let _ = path;
        Err(KoraError::not_implemented(
            "reader::EpubParser::open (epub 2.x)",
        ))
    }
}
