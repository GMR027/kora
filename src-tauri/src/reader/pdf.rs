use std::path::Path;

use async_trait::async_trait;

use super::DocumentParser;
use super::model::Document;
use crate::error::{KoraError, Result};

/// Parser PDF basado en `lopdf` para texto y metadatos. El renderizado de
/// páginas queda detrás de `PageRenderer` y se decide en la fase de lector
/// (pdfium embebido o pdf.js en el WebView).
pub struct PdfParser;

/// Documento PDF cargado en memoria (fase 4).
pub type LopdfDocument = lopdf::Document;

impl PdfParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PdfParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DocumentParser for PdfParser {
    async fn open(&self, path: &Path) -> Result<Document> {
        let _ = path;
        Err(KoraError::not_implemented(
            "reader::PdfParser::open (lopdf)",
        ))
    }
}
