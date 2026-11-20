use super::model::{Document, Locator, PageBitmap, Viewport};
use crate::error::{KoraError, Result};

/// Convierte una posición del documento en píxeles para pintar en el WebView.
pub trait PageRenderer: Send + Sync {
    fn render(
        &self,
        document: &Document,
        locator: &Locator,
        viewport: &Viewport,
    ) -> Result<PageBitmap>;
}

/// Renderizado delegado al WebView (EPUB en HTML y PDF con pdf.js).
pub struct WebViewRenderer;

impl PageRenderer for WebViewRenderer {
    fn render(
        &self,
        _document: &Document,
        _locator: &Locator,
        _viewport: &Viewport,
    ) -> Result<PageBitmap> {
        Err(KoraError::not_implemented(
            "reader::WebViewRenderer::render (fase 4)",
        ))
    }
}

/// Renderizado nativo con pdfium. Se activará solo si el binario está
/// disponible para la ABI de Android correspondiente.
pub struct PdfiumRenderer;

impl PageRenderer for PdfiumRenderer {
    fn render(
        &self,
        _document: &Document,
        _locator: &Locator,
        _viewport: &Viewport,
    ) -> Result<PageBitmap> {
        Err(KoraError::not_implemented(
            "reader::PdfiumRenderer::render (fase 4)",
        ))
    }
}
