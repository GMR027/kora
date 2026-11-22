use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use async_trait::async_trait;
use pdfium_render::prelude::*;

use super::DocumentParser;
use super::model::{
    Document, DocumentFormat, Locator, PageTextLayer, PdfSessionInfo, Section, TextWord,
};
use crate::error::{KoraError, Result};

/// Variables de entorno para localizar PDFium. Si no se define, se intenta la
/// librería del sistema (`libpdfium.so` en Linux, `pdfium.dll` en Windows).
const PDFIUM_PATH_ENV: &str = "KORA_PDFIUM_PATH";
/// Lado máximo del rasterizado que se envía al OCR.
const MAX_RENDER_SIDE: u32 = 3500;
/// Páginas que se muestrean para decidir si un PDF es escaneado.
const SCAN_SAMPLE_PAGES: usize = 6;

/// Parser PDF basado en PDFium. Si la librería nativa no está disponible, la
/// inspección básica sigue funcionando con `lopdf` y el visor usa pdf.js.
#[derive(Clone)]
pub struct PdfParser;

#[derive(Debug, Clone)]
pub struct RenderedPage {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
    /// Cuántos puntos PDF hay por píxel de la imagen.
    pub point_per_pixel: f32,
}

#[derive(Debug, Clone)]
pub struct PdfInspection {
    pub page_count: usize,
    pub title: Option<String>,
    pub author: Option<String>,
    /// `Subject` del diccionario de información del PDF.
    pub subjects: Vec<String>,
    /// `Keywords` separadas por comas o punto y coma.
    pub keywords: Vec<String>,
    pub scanned: bool,
    pub has_native_outline: bool,
    pub sections: Vec<Section>,
    pub entries: Vec<crate::storage::AutoIndexEntry>,
}

/// Línea reconstruida para las heurísticas de índice.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexLine {
    pub text: String,
    pub page: usize,
    pub font_size: f32,
    pub y: f32,
    pub page_height: f32,
    pub bold: bool,
}

#[derive(Debug, Clone)]
struct RawChar {
    text: char,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    font_size: f32,
}

impl PdfParser {
    pub fn new() -> Self {
        Self
    }

    /// Inspección completa: páginas, metadatos, bookmarks y detección de OCR.
    pub fn inspect(&self, path: &Path) -> Result<PdfInspection> {
        match self.inspect_with_pdfium(path) {
            Ok(inspection) => Ok(inspection),
            Err(pdfium_error) => self.inspect_with_lopdf(path, pdfium_error),
        }
    }

    pub fn page_text(&self, path: &Path, page: usize) -> Result<PageTextLayer> {
        let pdfium = bind_pdfium()?;
        let document = pdfium.load_pdf_from_file(path, None).map_err(|error| {
            KoraError::InvalidInput(format!("no se pudo abrir el PDF: {error}"))
        })?;

        page_text_with_document(&document, page)
    }

    /// Extrae varias páginas abriendo el documento una sola vez. Se usa para
    /// la búsqueda en todo el PDF.
    pub fn page_texts(&self, path: &Path, pages: &[usize]) -> Result<Vec<(usize, PageTextLayer)>> {
        let pdfium = bind_pdfium()?;
        let document = pdfium.load_pdf_from_file(path, None).map_err(|error| {
            KoraError::InvalidInput(format!("no se pudo abrir el PDF: {error}"))
        })?;

        let mut layers = Vec::with_capacity(pages.len());
        for page in pages {
            if let Ok(layer) = page_text_with_document(&document, *page) {
                layers.push((*page, layer));
            }
        }
        Ok(layers)
    }

    /// Rasteriza una página a PNG para el OCR.
    pub fn render_page(&self, path: &Path, page: usize, dpi: f32) -> Result<RenderedPage> {
        let pdfium = bind_pdfium()?;
        let document = pdfium.load_pdf_from_file(path, None).map_err(|error| {
            KoraError::InvalidInput(format!("no se pudo abrir el PDF: {error}"))
        })?;

        let pdf_page = document
            .pages()
            .get(pdf_page_index(page)?)
            .map_err(|error| {
                KoraError::InvalidInput(format!("página {page} fuera de rango: {error}"))
            })?;

        let width_pt = pdf_page.width().value.max(1.0);
        let height_pt = pdf_page.height().value.max(1.0);
        let dpi = dpi.clamp(96.0, 600.0);
        let mut scale = dpi / 72.0;
        let longest = width_pt.max(height_pt) * scale;
        if longest > MAX_RENDER_SIDE as f32 {
            scale *= MAX_RENDER_SIDE as f32 / longest;
        }

        let target_width = (width_pt * scale).round().max(1.0) as i32;
        let target_height = (height_pt * scale).round().max(1.0) as i32;

        let config = PdfRenderConfig::new()
            .set_target_width(target_width)
            .set_target_height(target_height);

        let bitmap = pdf_page.render_with_config(&config).map_err(|error| {
            KoraError::Internal(format!("PDFium no pudo rasterizar la página: {error}"))
        })?;

        let width = bitmap.width().max(0) as u32;
        let height = bitmap.height().max(0) as u32;
        let rgba = bitmap.as_rgba_bytes();
        let image = image::RgbaImage::from_raw(width, height, rgba).ok_or_else(|| {
            KoraError::Internal("PDFium devolvió un bitmap con tamaño inconsistente".into())
        })?;

        let mut png = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|error| KoraError::Internal(format!("no se pudo codificar PNG: {error}")))?;

        Ok(RenderedPage {
            width,
            height,
            png,
            point_per_pixel: width_pt / width.max(1) as f32,
        })
    }

    /// Extrae líneas de texto nativo para el auto-índice.
    pub fn index_lines(&self, path: &Path, max_pages: usize) -> Result<Vec<IndexLine>> {
        let pdfium = bind_pdfium()?;
        let document = pdfium.load_pdf_from_file(path, None).map_err(|error| {
            KoraError::InvalidInput(format!("no se pudo abrir el PDF: {error}"))
        })?;

        let pages = usize::try_from(document.pages().len()).unwrap_or(0);
        let limit = pages.min(max_pages.max(1));

        let mut lines = Vec::new();
        for index in 0..limit {
            let Ok(page) = document.pages().get(pdf_page_index(index)?) else {
                continue;
            };
            let page_height = page.height().value.max(1.0);
            let Ok(text) = page.text() else {
                continue;
            };

            let chars = collect_chars(&text, page_height);
            let words = group_chars(&chars);
            lines.extend(words_to_lines(&words, index, page_height));
        }

        Ok(lines)
    }

    fn inspect_with_pdfium(&self, path: &Path) -> Result<PdfInspection> {
        let pdfium = bind_pdfium()?;
        let document = pdfium.load_pdf_from_file(path, None).map_err(|error| {
            KoraError::InvalidInput(format!("no se pudo abrir el PDF: {error}"))
        })?;

        let page_count = usize::try_from(document.pages().len()).unwrap_or(0);
        let metadata = document.metadata();
        let title = metadata
            .get(PdfDocumentMetadataTagType::Title)
            .map(|tag| tag.value().trim().to_string())
            .filter(|value| !value.is_empty());
        let author = metadata
            .get(PdfDocumentMetadataTagType::Author)
            .map(|tag| tag.value().trim().to_string())
            .filter(|value| !value.is_empty());
        let subjects = metadata
            .get(PdfDocumentMetadataTagType::Subject)
            .map(|tag| tag.value().trim().to_string())
            .filter(|value| !value.is_empty())
            .into_iter()
            .collect();
        let keywords = metadata
            .get(PdfDocumentMetadataTagType::Keywords)
            .map(|tag| split_keywords(tag.value()))
            .unwrap_or_default();

        let entries = outline_entries(&document);
        let has_native_outline = !entries.is_empty();

        let sample = sampled_pages(page_count, SCAN_SAMPLE_PAGES);
        let mut text_chars = 0usize;
        for index in &sample {
            if let Ok(page) = document.pages().get(pdf_page_index(*index)?)
                && let Ok(text) = page.text()
            {
                text_chars += text.all().chars().filter(|c| !c.is_whitespace()).count();
            }
        }
        let scanned = sample.is_empty() || text_chars / sample.len().max(1) < 12;

        let sections = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| Section {
                id: format!("pdf-section-{index}"),
                title: Some(entry.title.clone()),
                locator: Locator {
                    section: entry.page,
                    offset: 0,
                },
            })
            .collect();

        Ok(PdfInspection {
            page_count,
            title,
            author,
            subjects,
            keywords,
            scanned,
            has_native_outline,
            sections,
            entries,
        })
    }

    /// Plan B sin PDFium: cuenta operadores de texto para detectar escaneados.
    fn inspect_with_lopdf(&self, path: &Path, pdfium_error: KoraError) -> Result<PdfInspection> {
        let document = lopdf::Document::load(path).map_err(|error| {
            KoraError::InvalidInput(format!("no se pudo abrir el PDF: {error}"))
        })?;

        let page_count = document.get_pages().len();
        let pages: Vec<_> = document.get_pages().values().copied().collect();
        let sample = sampled_pages(page_count, SCAN_SAMPLE_PAGES);

        let mut operators = 0usize;
        for index in &sample {
            let Some(page_id) = pages.get(*index) else {
                continue;
            };
            let content = document.get_page_content(*page_id);
            operators += count_text_operators(&content);
        }

        let scanned = sample.is_empty() || operators / sample.len().max(1) < 3;

        tracing::warn!(%pdfium_error, "PDFium no disponible; se usa la inspección básica con lopdf");

        Ok(PdfInspection {
            page_count,
            title: None,
            author: None,
            subjects: Vec::new(),
            keywords: Vec::new(),
            scanned,
            has_native_outline: false,
            sections: Vec::new(),
            entries: Vec::new(),
        })
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
        let inspection = self.inspect(path)?;

        Ok(Document {
            id: path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("pdf")
                .to_string(),
            format: DocumentFormat::Pdf,
            title: inspection.title.unwrap_or_else(|| "PDF".to_string()),
            author: inspection.author,
            language: None,
            publisher: None,
            description: None,
            sections: inspection.sections,
        })
    }
}

pub fn session_info(
    inspection: &PdfInspection,
    source_path: &Path,
    ocr_status: &str,
    ocr_done: usize,
    ocr_total: usize,
) -> PdfSessionInfo {
    PdfSessionInfo {
        page_count: inspection.page_count,
        scanned: inspection.scanned,
        has_native_outline: inspection.has_native_outline,
        source_path: source_path.display().to_string(),
        ocr_status: ocr_status.to_string(),
        ocr_pages_done: ocr_done,
        ocr_pages_total: ocr_total,
    }
}

/// Enlaza con PDFium una sola vez por proceso. `pdfium-render` no permite
/// reinicializar los bindings, así que se comparten entre todas las tareas.
pub fn bind_pdfium() -> Result<&'static Pdfium> {
    static PDFIUM: OnceLock<Pdfium> = OnceLock::new();

    if let Some(pdfium) = PDFIUM.get() {
        return Ok(pdfium);
    }

    let pdfium = if let Ok(configured) = std::env::var(PDFIUM_PATH_ENV) {
        let configured = Path::new(&configured);
        let library = if configured.is_dir() {
            Pdfium::pdfium_platform_library_name_at_path(configured)
        } else {
            configured.to_path_buf()
        };

        let bindings = Pdfium::bind_to_library(&library).map_err(|error| {
            KoraError::Unsupported(format!(
                "no se pudo cargar PDFium desde {}: {error}. Instala libpdfium o define {PDFIUM_PATH_ENV}",
                library.display()
            ))
        })?;

        Pdfium::new(bindings)
    } else {
        Pdfium::bind_to_system_library()
            .map(Pdfium::new)
            .map_err(|error| {
                KoraError::Unsupported(format!(
                    "PDFium no está disponible: {error}. Instala libpdfium o define {PDFIUM_PATH_ENV} para OCR, texto nativo y auto-índice"
                ))
            })?
    };

    let _ = PDFIUM.set(pdfium);
    PDFIUM
        .get()
        .ok_or_else(|| KoraError::Internal("PDFium no se pudo inicializar en este proceso".into()))
}

fn page_text_with_document(document: &PdfDocument<'_>, page: usize) -> Result<PageTextLayer> {
    let pdf_page = document
        .pages()
        .get(pdf_page_index(page)?)
        .map_err(|error| {
            KoraError::InvalidInput(format!("página {page} fuera de rango: {error}"))
        })?;

    let width = pdf_page.width().value.max(1.0);
    let height = pdf_page.height().value.max(1.0);
    let text = pdf_page
        .text()
        .map_err(|error| KoraError::Internal(format!("PDFium no pudo extraer texto: {error}")))?;

    let chars = collect_chars(&text, height);
    let words = group_chars(&chars);
    let source = if words.is_empty() { "empty" } else { "native" };

    Ok(PageTextLayer {
        book_id: String::new(),
        page,
        width,
        height,
        source: source.to_string(),
        words,
    })
}

fn collect_chars(text: &PdfPageText<'_>, page_height: f32) -> Vec<RawChar> {
    let mut chars = Vec::new();

    for char in text.chars().iter() {
        let Some(unicode) = char.unicode_char() else {
            continue;
        };
        if unicode.is_control() {
            continue;
        }

        let bounds = char
            .tight_bounds()
            .or_else(|_| char.loose_bounds())
            .unwrap_or(PdfRect::ZERO);

        let font_size = char.scaled_font_size().value;

        chars.push(RawChar {
            text: unicode,
            x: bounds.left().value,
            y: page_height - bounds.top().value,
            width: bounds.width().value.max(0.0),
            height: bounds.height().value.max(font_size * 0.6),
            font_size: font_size.max(1.0),
        });
    }

    chars
}

fn pdf_page_index(page: usize) -> Result<i32> {
    i32::try_from(page)
        .map_err(|_| KoraError::InvalidInput(format!("la página {page} excede el rango de PDFium")))
}

fn split_keywords(value: &str) -> Vec<String> {
    let mut keywords: Vec<String> = value
        .split([',', ';'])
        .map(str::trim)
        .filter(|keyword| !keyword.is_empty())
        .map(str::to_string)
        .collect();
    keywords.sort_unstable();
    keywords.dedup();
    keywords
}

/// Agrupa caracteres en palabras. Es una función pura para poder probar las
/// heurísticas sin cargar PDFium.
fn group_chars(chars: &[RawChar]) -> Vec<TextWord> {
    let mut words = Vec::new();
    let mut current: Option<TextWord> = None;
    let mut current_end_x = 0.0f32;
    let mut current_baseline = 0.0f32;

    let flush = |current: &mut Option<TextWord>, words: &mut Vec<TextWord>| {
        if let Some(word) = current.take() {
            let text = word.text.trim().to_string();
            if !text.is_empty() {
                words.push(TextWord { text, ..word });
            }
        }
    };

    for ch in chars {
        if ch.text.is_whitespace() {
            flush(&mut current, &mut words);
            continue;
        }

        let same_line = (ch.y - current_baseline).abs() <= ch.height.max(2.0) * 0.75;
        let gap = ch.x - current_end_x;
        let max_gap = (ch.font_size * 0.28).max(1.2);

        if current.is_some() && (!same_line || gap > max_gap) {
            flush(&mut current, &mut words);
        }

        match &mut current {
            Some(word) => {
                word.text.push(ch.text);
                let right = (ch.x + ch.width).max(word.x + word.width);
                let bottom = (ch.y + ch.height).max(word.y + word.height);
                let left = word.x.min(ch.x);
                let top = word.y.min(ch.y);
                word.x = left;
                word.y = top;
                word.width = right - left;
                word.height = bottom - top;
                word.font_size = (word.font_size + ch.font_size) / 2.0;
                word.confidence = None;
                current_end_x = (ch.x + ch.width).max(current_end_x);
            }
            None => {
                current = Some(TextWord {
                    text: ch.text.to_string(),
                    x: ch.x,
                    y: ch.y,
                    width: ch.width.max(0.1),
                    height: ch.height.max(0.1),
                    font_size: ch.font_size,
                    confidence: None,
                });
                current_baseline = ch.y;
                current_end_x = ch.x + ch.width;
            }
        }
    }

    flush(&mut current, &mut words);
    words
}

pub(crate) fn words_to_lines(words: &[TextWord], page: usize, page_height: f32) -> Vec<IndexLine> {
    let mut sorted = words.to_vec();
    sorted.sort_by(|a, b| {
        a.y.partial_cmp(&b.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut lines: Vec<IndexLine> = Vec::new();
    for word in sorted {
        let joins = lines
            .last()
            .is_some_and(|line| (word.y - line.y).abs() <= word.height.max(4.0) * 0.6);

        if joins {
            let line = lines.last_mut().expect("línea recién comprobada");
            line.text.push(' ');
            line.text.push_str(&word.text);
            line.font_size = (line.font_size + word.font_size) / 2.0;
        } else {
            lines.push(IndexLine {
                text: word.text.clone(),
                page,
                font_size: word.font_size,
                y: word.y,
                page_height,
                bold: false,
            });
        }
    }

    lines
}

fn outline_entries(document: &PdfDocument<'_>) -> Vec<crate::storage::AutoIndexEntry> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();

    for bookmark in document.bookmarks().iter() {
        let Some(title) = bookmark.title() else {
            continue;
        };
        let title = title.trim().to_string();
        if title.is_empty() {
            continue;
        }

        let Some(destination) = bookmark.destination() else {
            continue;
        };
        let Ok(page) = destination.page_index() else {
            continue;
        };

        let mut level = 0usize;
        let mut parent = bookmark.parent();
        while let Some(bookmark) = parent {
            level += 1;
            parent = bookmark.parent();
        }

        if seen.insert((page, title.clone())) {
            entries.push(crate::storage::AutoIndexEntry {
                title,
                page: usize::try_from(page).unwrap_or(0),
                level,
            });
        }
    }

    entries
}

fn sampled_pages(page_count: usize, sample: usize) -> Vec<usize> {
    if page_count == 0 {
        return Vec::new();
    }

    let mut pages: Vec<usize> = (0..page_count.min(sample)).collect();
    if page_count > sample {
        pages.push(page_count - 1);
        pages.push(page_count / 2);
    }
    pages.sort_unstable();
    pages.dedup();
    pages
}

fn count_text_operators(content: &[u8]) -> usize {
    let mut count = 0usize;
    let mut index = 0usize;

    while index < content.len() {
        let byte = content[index];
        if matches!(byte, b'T') {
            if content.get(index + 1) == Some(&b'j') {
                count += 1;
                index += 2;
                continue;
            }
            if content.get(index + 1) == Some(&b'J') {
                count += 1;
                index += 2;
                continue;
            }
        }
        if byte == b'\'' || byte == b'"' {
            count += 1;
        }
        index += 1;
    }

    count
}

/// Construye una capa vacía cuando no hay texto ni PDFium disponible.
pub fn empty_layer(book_id: &str, page: usize, width: f32, height: f32) -> PageTextLayer {
    PageTextLayer {
        book_id: book_id.to_string(),
        page,
        width,
        height,
        source: "empty".to_string(),
        words: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(text: char, x: f32, y: f32, width: f32) -> RawChar {
        RawChar {
            text,
            x,
            y,
            width,
            height: 10.0,
            font_size: 10.0,
        }
    }

    #[test]
    fn groups_words_and_splits_on_gaps() {
        let chars = vec![
            ch('H', 0.0, 10.0, 6.0),
            ch('o', 6.0, 10.0, 5.0),
            ch('l', 11.0, 10.0, 3.0),
            ch('a', 14.0, 10.0, 5.0),
            ch(' ', 19.0, 10.0, 2.0),
            ch('m', 24.0, 10.0, 8.0),
            ch('u', 32.0, 10.0, 6.0),
            ch('n', 38.0, 10.0, 6.0),
            ch('d', 44.0, 10.0, 6.0),
            ch('o', 50.0, 10.0, 6.0),
        ];

        let words = group_chars(&chars);
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hola");
        assert_eq!(words[1].text, "mundo");
    }

    #[test]
    fn splits_words_on_large_gap_without_space() {
        let chars = vec![ch('A', 0.0, 10.0, 5.0), ch('B', 20.0, 10.0, 5.0)];
        let words = group_chars(&chars);
        assert_eq!(words.len(), 2);
    }

    #[test]
    fn keeps_line_words_separate_after_a_long_previous_line() {
        let chars = vec![
            ch('A', 0.0, 10.0, 5.0),
            ch('B', 300.0, 10.0, 5.0),
            ch('C', 0.0, 30.0, 5.0),
            ch('D', 30.0, 30.0, 5.0),
        ];
        let words = group_chars(&chars);
        assert_eq!(words.len(), 4);
        assert_eq!(words[2].text, "C");
        assert_eq!(words[3].text, "D");
    }

    #[test]
    fn counts_text_operators() {
        assert_eq!(count_text_operators(b"BT /F1 12 Tf (Hola) Tj ET"), 1);
        assert_eq!(count_text_operators(b"[(A) -2 (B)] TJ"), 1);
        assert_eq!(count_text_operators(b"q 1 0 0 1 0 0 cm /Im0 Do Q"), 0);
    }

    #[test]
    fn samples_first_middle_and_last_pages() {
        assert_eq!(sampled_pages(0, 6), Vec::<usize>::new());
        assert_eq!(sampled_pages(3, 6), vec![0, 1, 2]);
        assert_eq!(sampled_pages(20, 4), vec![0, 1, 2, 3, 10, 19]);
    }
}
