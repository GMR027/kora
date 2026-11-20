pub mod epub;
pub mod model;
pub mod pdf;
pub mod renderer;

pub use epub::EpubParser;
pub use model::{
    Document, DocumentFormat, Locator, PageBitmap, ReaderSession, Section, TextRange, Viewport,
};
pub use pdf::PdfParser;
pub use renderer::{PageRenderer, PdfiumRenderer, WebViewRenderer};

use std::path::Path;

use async_trait::async_trait;

use crate::error::{KoraError, Result};

#[async_trait]
pub trait DocumentParser: Send + Sync {
    async fn open(&self, path: &Path) -> Result<Document>;
}

pub trait TextExtractor: Send + Sync {
    fn extract(&self, document: &Document, range: &TextRange) -> Result<String>;
}

/// Fachada que elige el parser según la extensión del archivo.
pub struct ReaderService {
    epub: EpubParser,
    pdf: PdfParser,
}

impl ReaderService {
    pub fn new() -> Self {
        Self {
            epub: EpubParser::new(),
            pdf: PdfParser::new(),
        }
    }

    pub async fn open(&self, book_id: &str, path: &Path) -> Result<ReaderSession> {
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .ok_or_else(|| KoraError::InvalidInput("el archivo no tiene extensión".into()))?;

        let format = DocumentFormat::from_extension(extension).ok_or_else(|| {
            KoraError::InvalidInput(format!("formato no soportado: .{extension}"))
        })?;

        let document = match format {
            DocumentFormat::Epub => self.epub.open(path).await?,
            DocumentFormat::Pdf => self.pdf.open(path).await?,
        };

        let locator = document
            .sections
            .first()
            .map(|section| section.locator)
            .unwrap_or(Locator::START);

        Ok(ReaderSession {
            book_id: book_id.to_string(),
            document,
            locator,
        })
    }
}

impl Default for ReaderService {
    fn default() -> Self {
        Self::new()
    }
}
