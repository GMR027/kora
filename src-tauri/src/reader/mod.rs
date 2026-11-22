pub mod auto_index;
pub mod convert;
pub mod epub;
pub mod model;
pub mod ocr;
pub mod pdf;
pub mod renderer;
pub mod sanitize;
pub mod text;

pub use epub::{CoverImage, EpubInspection, EpubParser};
pub use model::{
    ChapterContent, Document, DocumentFormat, Locator, PageTextLayer, PdfSessionInfo,
    ReaderSession, Section, TextRange, TextWord, Viewport,
};
pub use pdf::PdfParser;
pub use renderer::{PageRenderer, PdfiumRenderer, WebViewRenderer};
pub use text::{SourceText, extract_text};

use std::path::Path;

use async_trait::async_trait;

use crate::error::Result;

#[async_trait]
pub trait DocumentParser: Send + Sync {
    async fn open(&self, path: &Path) -> Result<Document>;
}

pub trait TextExtractor: Send + Sync {
    fn extract(&self, document: &Document, range: &TextRange) -> Result<String>;
}

/// Fachada que elige el parser según la extensión del archivo. El EPUB se
/// inspecciona y se lee capítulo a capítulo; el PDF se renderiza con pdf.js y
/// el texto/OCR se resuelve con PDFium y Tesseract.
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

    pub fn inspect_epub(&self, path: &Path) -> Result<EpubInspection> {
        self.epub.inspect(path)
    }

    pub fn epub_chapter(&self, path: &Path, index: usize) -> Result<ChapterContent> {
        self.epub.chapter(path, index)
    }

    pub fn pdf(&self) -> &PdfParser {
        &self.pdf
    }
}

impl Default for ReaderService {
    fn default() -> Self {
        Self::new()
    }
}
