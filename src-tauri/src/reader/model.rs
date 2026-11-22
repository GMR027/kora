use serde::{Deserialize, Serialize};

use crate::storage::ReaderSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentFormat {
    Epub,
    Pdf,
}

impl DocumentFormat {
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "epub" => Some(Self::Epub),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }
}

/// Posición estable dentro de un documento: sección (capítulo o página) y
/// desplazamiento en caracteres desde el inicio de esa sección.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Locator {
    pub section: usize,
    pub offset: usize,
}

impl Locator {
    pub const START: Self = Self {
        section: 0,
        offset: 0,
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: String,
    pub title: Option<String>,
    pub locator: Locator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub format: DocumentFormat,
    pub title: String,
    pub author: Option<String>,
    pub language: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub sections: Vec<Section>,
}

/// Estado completo que el frontend necesita al abrir un libro.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderSession {
    pub book_id: String,
    pub document: Document,
    pub locator: Locator,
    pub cover_path: Option<String>,
    pub progress: f64,
    pub settings: ReaderSettings,
    pub pdf: Option<PdfSessionInfo>,
}

/// Datos específicos de un PDF que el visor necesita además del documento.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfSessionInfo {
    pub page_count: usize,
    pub scanned: bool,
    pub has_native_outline: bool,
    pub source_path: String,
    pub ocr_status: String,
    pub ocr_pages_done: usize,
    pub ocr_pages_total: usize,
}

/// Una palabra (o fragmento) con su caja en puntos PDF y origen arriba-izquierda.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextWord {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub font_size: f32,
    pub confidence: Option<f32>,
}

/// Capa de texto de una página, ya sea extraída del PDF o generada por OCR.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageTextLayer {
    pub book_id: String,
    pub page: usize,
    pub width: f32,
    pub height: f32,
    /// `native`, `ocr` o `empty`.
    pub source: String,
    pub words: Vec<TextWord>,
}

impl PageTextLayer {
    pub fn text(&self) -> String {
        self.words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Un capítulo ya saneado y listo para inyectar con `{@html}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterContent {
    pub book_id: String,
    pub index: usize,
    pub title: String,
    pub html: String,
    pub char_count: usize,
    pub has_previous: bool,
    pub has_next: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct TextRange {
    pub start: Locator,
    pub end: Locator,
}

#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
}

#[derive(Debug, Clone)]
pub struct PageBitmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
