use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::error::{KoraError, Result};
use crate::reader::{PageTextLayer, TextWord};
use crate::storage::AutoIndexEntry;
use crate::tejido::epub::EpubChapter;

use super::auto_index;
use super::epub::EpubParser;
use super::pdf::IndexLine;
use super::text::extract_text;

/// Cómo se dividen los capítulos del EPUB. `Auto` prefiere el índice nativo
/// del PDF, después los encabezados detectados y por último bloques de páginas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChapterMode {
    Auto,
    Outline,
    Heuristic,
    Pages,
}

impl ChapterMode {
    pub fn parse(value: Option<&str>) -> Self {
        match value.unwrap_or("auto").trim().to_ascii_lowercase().as_str() {
            "outline" | "indice" | "índice" => Self::Outline,
            "heuristic" | "headings" | "encabezados" => Self::Heuristic,
            "pages" | "bloques" => Self::Pages,
            _ => Self::Auto,
        }
    }
}

/// Rango de páginas originales que alimenta cada capítulo. Se usa para
/// reubicar anotaciones y para conservar una idea del progreso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct ConversionOutput {
    pub chapters: Vec<EpubChapter>,
    pub page_ranges: Vec<PageRange>,
    pub chars: usize,
    pub warnings: Vec<String>,
    pub empty_pages: usize,
    pub native_pages: usize,
    pub ocr_pages: usize,
    pub used_outline: bool,
}

/// Informe que se muestra al terminar y que también viaja en el evento de
/// finalización.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionReport {
    pub book_id: String,
    pub chapters: usize,
    pub chars: usize,
    pub pages: usize,
    pub ocr_used: bool,
    pub anchored: usize,
    pub approximate: usize,
    pub unmapped: usize,
    pub warnings: Vec<String>,
    pub replaced: bool,
}

/// Estado en memoria de una conversión. El mapa vive en `AppState`; no toca
/// SQLite para no añadir una migración por un trabajo efímero.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfConversionState {
    pub book_id: String,
    pub phase: String,
    pub percent: f32,
    pub message: String,
    pub pages_done: usize,
    pub pages_total: usize,
    pub error: Option<String>,
    pub done: bool,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub report: Option<ConversionReport>,
}

impl PdfConversionState {
    pub fn new(book_id: impl Into<String>) -> Self {
        Self {
            book_id: book_id.into(),
            phase: "preparing".to_string(),
            percent: 0.0,
            message: String::new(),
            pages_done: 0,
            pages_total: 0,
            error: None,
            done: false,
            started_at: Utc::now(),
            finished_at: None,
            report: None,
        }
    }
}

/// Texto plano de un capítulo ya generado. Los desplazamientos son UTF-16,
/// igual que los que usa el lector para las anotaciones.
#[derive(Debug, Clone)]
pub struct ChapterText {
    pub text: String,
    pub utf16_len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationStatus {
    Anchored,
    Approximate,
}

#[derive(Debug, Clone)]
pub struct RelocationInput {
    pub id: String,
    /// Página original del PDF (0-based). Puede faltar en notas globales.
    pub page: Option<usize>,
    pub quote: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RelocationOutput {
    pub id: String,
    pub chapter_index: i64,
    pub offset_start: i64,
    pub offset_end: i64,
    pub status: RelocationStatus,
}

/// Convierte las capas de texto de un PDF en capítulos EPUB reflowables.
pub fn convert_pages(
    pages: &[PageTextLayer],
    outline: &[AutoIndexEntry],
    mode: ChapterMode,
    language: &str,
) -> Result<ConversionOutput> {
    if pages.is_empty() {
        return Err(KoraError::InvalidInput(
            "el PDF no tiene páginas para convertir".into(),
        ));
    }

    let page_count = pages.len();
    let empty_pages = pages.iter().filter(|page| page.words.is_empty()).count();
    let native_pages = pages
        .iter()
        .filter(|page| page.source == "native" && !page.words.is_empty())
        .count();
    let ocr_pages = pages
        .iter()
        .filter(|page| page.source == "ocr" && !page.words.is_empty())
        .count();

    let mut lines = Vec::new();
    for page in pages {
        lines.extend(page_lines(page));
    }
    remove_running_lines(&mut lines, page_count);
    lines.retain(|line| !line.text.trim().is_empty());

    if lines.is_empty() {
        return Err(KoraError::Unsupported(
            "no se pudo extraer texto suficiente del PDF; puede ser un documento de imágenes o una maquetación fija".into(),
        ));
    }

    let mut warnings = Vec::new();
    if empty_pages > 0 {
        warnings.push(format!(
            "{empty_pages} páginas no tenían texto reconocible y se omitieron."
        ));
    }
    if ocr_pages > 0 {
        warnings.push(
            "Parte del texto vino del OCR; revisa posibles errores de reconocimiento.".into(),
        );
    }

    let (starts, used_outline) = chapter_starts(outline, &lines, page_count, mode, language);
    let page_ranges = page_ranges(&starts, page_count);
    let paragraphs = build_paragraphs(&lines);
    let (chapters, page_ranges) = build_chapters(&starts, &page_ranges, &paragraphs, language);

    if chapters.is_empty() {
        return Err(KoraError::Unsupported(
            "no se generó ningún capítulo con texto utilizable".into(),
        ));
    }

    if chapters.len() == 1 && page_count > 40 {
        warnings.push(
            "No se detectaron capítulos claros; el libro se generó como un único bloque de texto."
                .into(),
        );
    }

    let chars = chapters
        .iter()
        .map(|chapter| chapter.body.chars().count())
        .sum();

    Ok(ConversionOutput {
        chapters,
        page_ranges,
        chars,
        warnings,
        empty_pages,
        native_pages,
        ocr_pages,
        used_outline,
    })
}

/// Lee un EPUB ya escrito y devuelve el texto de cada capítulo en el mismo
/// orden del spine. Es la base para reanclar citas y subrayados.
pub fn chapter_texts(path: &Path) -> Result<Vec<ChapterText>> {
    let parser = EpubParser::new();
    let inspection = parser.inspect(path)?;
    let mut texts = Vec::with_capacity(inspection.sections.len());

    for index in 0..inspection.sections.len() {
        let chapter = parser.chapter(path, index)?;
        let source = extract_text(&chapter.html);
        texts.push(ChapterText {
            text: source.text,
            utf16_len: source.utf16_len,
        });
    }

    Ok(texts)
}

/// Busca la cita de cada anotación en los capítulos nuevos. Si no aparece,
/// deja la anotación al inicio del capítulo que corresponde a su página
/// original y la marca como aproximada.
pub fn relocate_items(
    items: &[RelocationInput],
    chapters: &[ChapterText],
    page_ranges: &[PageRange],
) -> Vec<RelocationOutput> {
    if chapters.is_empty() {
        return items
            .iter()
            .map(|item| RelocationOutput {
                id: item.id.clone(),
                chapter_index: 0,
                offset_start: 0,
                offset_end: 0,
                status: RelocationStatus::Approximate,
            })
            .collect();
    }

    items
        .iter()
        .map(|item| {
            let page_chapter = item
                .page
                .and_then(|page| chapter_for_page(page, page_ranges));

            if let Some(quote) = item
                .quote
                .as_deref()
                .map(str::trim)
                .filter(|quote| quote.chars().count() >= 6)
            {
                let order = search_order(page_chapter, chapters.len());
                for index in order {
                    if let Some((start, end)) = find_quote(&chapters[index], quote) {
                        return RelocationOutput {
                            id: item.id.clone(),
                            chapter_index: index as i64,
                            offset_start: start as i64,
                            offset_end: end as i64,
                            status: RelocationStatus::Anchored,
                        };
                    }
                }
            }

            let index = page_chapter
                .unwrap_or(0)
                .min(chapters.len().saturating_sub(1));
            RelocationOutput {
                id: item.id.clone(),
                chapter_index: index as i64,
                offset_start: 0,
                offset_end: 0,
                status: RelocationStatus::Approximate,
            }
        })
        .collect()
}

#[derive(Debug, Clone)]
struct LayoutLine {
    text: String,
    page: usize,
    column: usize,
    x: f32,
    y: f32,
    width: f32,
    font_size: f32,
    page_height: f32,
}

#[derive(Debug, Clone)]
struct Paragraph {
    text: String,
    start_page: usize,
    end_page: usize,
}

#[derive(Debug, Clone)]
struct ChapterStart {
    page: usize,
    title: String,
}

fn page_lines(page: &PageTextLayer) -> Vec<LayoutLine> {
    if page.words.is_empty() {
        return Vec::new();
    }

    let page_height = page.height.max(1.0);
    let page_width = page.width.max(1.0);
    let groups = column_groups(&page.words, 0.0, page_width, page_height, 0);
    let mut lines = Vec::new();

    for (column, words) in groups.into_iter().enumerate() {
        let mut words = words;
        words.sort_by(|left, right| {
            left.y
                .partial_cmp(&right.y)
                .unwrap_or(Ordering::Equal)
                .then(left.x.partial_cmp(&right.x).unwrap_or(Ordering::Equal))
        });

        // El OCR devuelve líneas con pequeñas variaciones de baseline. Se
        // agrupan por la media de la línea y un margen amplio, y después se
        // ordenan las palabras por x. Si se usa la altura de cada palabra por
        // separado, unas pocas palabras se escapan a otra línea y el texto
        // sale desordenado.
        let mut clusters: Vec<Vec<TextWord>> = Vec::new();
        for word in words {
            if word.text.trim().is_empty() {
                continue;
            }
            let height = word.height.max(word.font_size).max(1.0);
            let joins = clusters.last().is_some_and(|cluster| {
                let count = cluster.len().max(1) as f32;
                let average_y = cluster.iter().map(|word| word.y).sum::<f32>() / count;
                let average_height = cluster
                    .iter()
                    .map(|word| word.height.max(word.font_size).max(1.0))
                    .sum::<f32>()
                    / count;
                (word.y - average_y).abs() <= (average_height.max(height) * 0.85).max(3.0)
            });

            if joins {
                if let Some(cluster) = clusters.last_mut() {
                    cluster.push(word);
                }
            } else {
                clusters.push(vec![word]);
            }
        }

        for mut cluster in clusters {
            cluster.sort_by(|left, right| {
                left.x
                    .partial_cmp(&right.x)
                    .unwrap_or(Ordering::Equal)
                    .then(left.y.partial_cmp(&right.y).unwrap_or(Ordering::Equal))
            });

            let mut line = LayoutLine {
                text: String::new(),
                page: page.page,
                column,
                x: f32::MAX,
                y: 0.0,
                width: 0.0,
                font_size: 0.0,
                page_height,
            };
            let mut right = f32::MIN;
            let mut y_sum = 0.0f32;
            let mut size_sum = 0.0f32;
            let mut count = 0usize;

            for word in cluster {
                let text = word.text.trim();
                if text.is_empty() {
                    continue;
                }
                if !line.text.is_empty() {
                    let gap = word.x - right;
                    if gap > (word.font_size * 0.12).max(0.8) {
                        line.text.push(' ');
                    }
                }
                line.text.push_str(text);
                line.x = line.x.min(word.x);
                right = right.max(word.x + word.width);
                y_sum += word.y;
                size_sum += word.font_size.max(1.0);
                count += 1;
            }

            if line.text.trim().is_empty() || count == 0 {
                continue;
            }
            line.width = (right - line.x).max(0.1);
            line.y = y_sum / count as f32;
            line.font_size = size_sum / count as f32;
            lines.push(line);
        }
    }

    lines.sort_by(|left, right| {
        left.page
            .cmp(&right.page)
            .then(left.column.cmp(&right.column))
            .then(left.y.partial_cmp(&right.y).unwrap_or(Ordering::Equal))
            .then(left.x.partial_cmp(&right.x).unwrap_or(Ordering::Equal))
    });
    lines
}

/// Detecta uno o dos canales verticales de lectura. Ignora la cabecera y el
/// pie para que un título a todo el ancho no impida ver las columnas.
fn column_groups(
    words: &[TextWord],
    x_min: f32,
    x_max: f32,
    page_height: f32,
    depth: usize,
) -> Vec<Vec<TextWord>> {
    if words.len() < 24 || depth >= 2 || x_max - x_min < 220.0 {
        return vec![words.to_vec()];
    }

    let candidates: Vec<TextWord> = words
        .iter()
        .filter(|word| word.y >= page_height * 0.12 && word.y <= page_height * 0.90)
        .cloned()
        .collect();
    if candidates.len() < 18 {
        return vec![words.to_vec()];
    }

    let span = x_max - x_min;
    let start = x_min + span * 0.28;
    let end = x_min + span * 0.72;
    let min_side = ((candidates.len() as f32) * 0.14).ceil() as usize;
    let mut best: Option<(usize, f32)> = None;
    let mut gutter = start;
    while gutter <= end {
        let mut left = 0usize;
        let mut right = 0usize;
        let mut crossing = 0usize;

        for word in &candidates {
            let center = word.x + word.width * 0.5;
            if center < gutter {
                left += 1;
            } else {
                right += 1;
            }
            if word.x < gutter && word.x + word.width > gutter {
                crossing += 1;
            }
        }

        if left >= min_side && right >= min_side && crossing == 0 {
            let score = left.min(right);
            if best.is_none_or(|(best_score, _)| score > best_score) {
                best = Some((score, gutter));
            }
        }
        gutter += 2.0;
    }

    let Some((_, gutter)) = best else {
        return vec![words.to_vec()];
    };

    let mut left = Vec::new();
    let mut right = Vec::new();
    for word in words {
        if word.x + word.width * 0.5 < gutter {
            left.push(word.clone());
        } else {
            right.push(word.clone());
        }
    }

    if left.len() < 4 || right.len() < 4 {
        return vec![words.to_vec()];
    }

    let mut groups = column_groups(&left, x_min, gutter, page_height, depth + 1);
    groups.extend(column_groups(&right, gutter, x_max, page_height, depth + 1));
    groups
}

fn remove_running_lines(lines: &mut Vec<LayoutLine>, page_count: usize) {
    let mut pages_by_key: HashMap<String, HashSet<usize>> = HashMap::new();

    for line in lines.iter() {
        if !in_margin(line) {
            continue;
        }
        let key = normalize_repeated(&line.text);
        if key.is_empty() || key.chars().count() > 90 {
            continue;
        }
        pages_by_key.entry(key).or_default().insert(line.page);
    }

    let threshold = (page_count / 5).max(3);
    let repeated: HashSet<String> = pages_by_key
        .into_iter()
        .filter(|(_, pages)| pages.len() >= threshold)
        .map(|(key, _)| key)
        .collect();
    let median = median_font_size(lines);

    lines.retain(|line| {
        // Cabeceras de OCR que varían una letra entre páginas no coinciden
        // con la clave repetida, pero sí son más pequeñas que el cuerpo y
        // viven en el margen superior.
        if line.y < line.page_height * 0.12 && line.font_size < median * 0.90 {
            return false;
        }
        if in_margin(line) && is_page_number(&line.text) {
            return false;
        }
        if in_margin(line) && repeated.contains(&normalize_repeated(&line.text)) {
            return false;
        }
        true
    });
}

fn in_margin(line: &LayoutLine) -> bool {
    // Los PDF escaneados suelen colocar la cabecera en torno al 9-10 % de
    // la altura; el cuerpo empieza después del 12 %. Un umbral mayor evita
    // que el título del libro y el número de página se cuelen en cada párrafo.
    line.y < line.page_height * 0.12 || line.y > line.page_height * 0.91
}

fn build_paragraphs(lines: &[LayoutLine]) -> Vec<Paragraph> {
    if lines.is_empty() {
        return Vec::new();
    }

    let median = median_font_size(lines);
    let mut column_left: HashMap<(usize, usize), f32> = HashMap::new();
    for line in lines {
        let entry = column_left
            .entry((line.page, line.column))
            .or_insert(line.x);
        *entry = entry.min(line.x);
    }

    let mut paragraphs = Vec::new();
    let mut current: Option<Paragraph> = None;
    let mut previous: Option<&LayoutLine> = None;

    for line in lines {
        let starts_new = previous.is_none_or(|prev| should_break(prev, line, median, &column_left));

        if starts_new {
            if let Some(paragraph) = current.take()
                && !paragraph.text.trim().is_empty()
            {
                paragraphs.push(paragraph);
            }
            current = Some(Paragraph {
                text: line.text.trim().to_string(),
                start_page: line.page,
                end_page: line.page,
            });
        } else if let Some(paragraph) = current.as_mut() {
            append_line(&mut paragraph.text, &line.text);
            paragraph.end_page = line.page;
        }

        previous = Some(line);
    }

    if let Some(paragraph) = current.take()
        && !paragraph.text.trim().is_empty()
    {
        paragraphs.push(paragraph);
    }

    paragraphs
}

fn should_break(
    previous: &LayoutLine,
    current: &LayoutLine,
    median: f32,
    column_left: &HashMap<(usize, usize), f32>,
) -> bool {
    if previous.page != current.page {
        if previous.column != current.column {
            return true;
        }
        if ends_sentence(&previous.text) || starts_upper(&current.text) {
            return true;
        }
        return false;
    }

    if previous.column != current.column {
        return true;
    }

    if current.font_size >= median * 1.20 {
        return true;
    }

    let line_gap = (current.y - previous.y).abs();
    if line_gap > previous.font_size.max(current.font_size) * 1.80 {
        return true;
    }

    let left = column_left
        .get(&(current.page, current.column))
        .copied()
        .unwrap_or(current.x);
    let current_indent = current.x - left;
    let previous_indent = previous.x - left;
    if current_indent > median * 0.90 && previous_indent < median * 0.55 {
        return true;
    }

    if ends_sentence(&previous.text) && starts_upper(&current.text) {
        return true;
    }

    if is_mostly_upper(&current.text)
        && current.text.chars().count() <= 90
        && current.font_size >= median * 1.05
    {
        return true;
    }

    false
}

fn append_line(paragraph: &mut String, line: &str) {
    let line = line.trim();
    if line.is_empty() {
        return;
    }

    let ends_with_hyphen = paragraph.ends_with('-')
        || paragraph.ends_with('‐')
        || paragraph.ends_with('‑')
        || paragraph.ends_with('\u{00ad}');

    if ends_with_hyphen {
        paragraph.pop();
        paragraph.push_str(line);
    } else {
        if !paragraph.ends_with(char::is_whitespace) {
            paragraph.push(' ');
        }
        paragraph.push_str(line);
    }
}

fn ends_sentence(text: &str) -> bool {
    text.trim_end()
        .chars()
        .last()
        .is_some_and(|last| matches!(last, '.' | '!' | '?' | '…' | ':' | ';'))
}

fn starts_upper(text: &str) -> bool {
    text.trim_start()
        .chars()
        .next()
        .is_some_and(|first| first.is_uppercase() || first.is_ascii_digit())
}

fn is_mostly_upper(text: &str) -> bool {
    let letters: Vec<char> = text
        .chars()
        .filter(|character| character.is_alphabetic())
        .collect();
    if letters.len() < 4 {
        return false;
    }
    let upper = letters
        .iter()
        .filter(|character| character.is_uppercase())
        .count();
    upper * 100 / letters.len() >= 80
}

fn median_font_size(lines: &[LayoutLine]) -> f32 {
    let mut sizes: Vec<f32> = lines
        .iter()
        .map(|line| line.font_size)
        .filter(|size| size.is_finite() && *size > 0.0)
        .collect();
    if sizes.is_empty() {
        return 10.0;
    }
    sizes.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let middle = sizes.len() / 2;
    if sizes.len().is_multiple_of(2) {
        (sizes[middle - 1] + sizes[middle]) / 2.0
    } else {
        sizes[middle]
    }
}

fn chapter_starts(
    outline: &[AutoIndexEntry],
    lines: &[LayoutLine],
    page_count: usize,
    mode: ChapterMode,
    language: &str,
) -> (Vec<ChapterStart>, bool) {
    let outline: Vec<AutoIndexEntry> = outline
        .iter()
        .filter(|entry| entry.page < page_count && !entry.title.trim().is_empty())
        .cloned()
        .collect();

    let mut entries = match mode {
        ChapterMode::Outline => {
            if outline.len() >= 2 {
                outline.clone()
            } else {
                heuristic_entries(lines)
            }
        }
        ChapterMode::Heuristic => heuristic_entries(lines),
        ChapterMode::Pages => Vec::new(),
        ChapterMode::Auto => {
            if outline.len() >= 2 {
                outline.clone()
            } else {
                heuristic_entries(lines)
            }
        }
    };

    entries.retain(|entry| entry.page < page_count && !entry.title.trim().is_empty());
    entries.sort_by_key(|entry| (entry.page, entry.level));

    let mut seen = HashSet::new();
    entries.retain(|entry| seen.insert(entry.page));

    let used_outline = entries.len() >= 2
        && (mode == ChapterMode::Outline || (mode == ChapterMode::Auto && outline.len() >= 2));

    let mut starts: Vec<ChapterStart> = if entries.len() >= 2 && entries.len() <= 240 {
        entries
            .into_iter()
            .map(|entry| ChapterStart {
                page: entry.page,
                title: clean_title(&entry.title),
            })
            .collect()
    } else {
        page_starts(lines, page_count, language)
    };

    if starts.is_empty() {
        starts.push(ChapterStart {
            page: 0,
            title: first_part_title(language, 1, page_count),
        });
    }

    starts.sort_by_key(|start| start.page);
    let mut seen = HashSet::new();
    starts.retain(|start| seen.insert(start.page));

    if let Some(first_page) = starts.first().map(|start| start.page)
        && first_page > 0
    {
        if lines.iter().any(|line| line.page < first_page) {
            starts.insert(
                0,
                ChapterStart {
                    page: 0,
                    title: start_title(language),
                },
            );
        } else if let Some(first) = starts.first_mut() {
            first.page = 0;
        }
    }

    (starts, used_outline)
}

fn heuristic_entries(lines: &[LayoutLine]) -> Vec<AutoIndexEntry> {
    let index_lines: Vec<IndexLine> = lines
        .iter()
        .map(|line| IndexLine {
            text: line.text.clone(),
            page: line.page,
            font_size: line.font_size,
            y: line.y,
            page_height: line.page_height,
            bold: false,
        })
        .collect();

    auto_index::build_from_lines(&index_lines)
}

fn page_starts(lines: &[LayoutLine], page_count: usize, language: &str) -> Vec<ChapterStart> {
    if page_count == 0 {
        return Vec::new();
    }

    let mut chars_by_page = vec![0usize; page_count];
    for line in lines {
        if let Some(count) = chars_by_page.get_mut(line.page) {
            *count += line.text.chars().count();
        }
    }

    let mut starts = Vec::new();
    let mut current_start = 0usize;
    let mut chars = 0usize;
    let mut pages = 0usize;
    let mut part = 1usize;

    for (page, count) in chars_by_page.iter().enumerate() {
        if pages > 0 && (chars >= 18_000 || pages >= 25) {
            starts.push(ChapterStart {
                page: current_start,
                title: first_part_title(language, part, current_start),
            });
            part += 1;
            current_start = page;
            chars = 0;
            pages = 0;
        }
        chars += *count;
        pages += 1;
    }

    if starts.is_empty()
        || starts
            .last()
            .is_none_or(|start| start.page != current_start)
    {
        starts.push(ChapterStart {
            page: current_start,
            title: first_part_title(language, part, current_start),
        });
    }

    starts
}

fn first_part_title(language: &str, part: usize, _page: usize) -> String {
    if language.to_ascii_lowercase().starts_with("en") {
        format!("Part {part}")
    } else {
        format!("Parte {part}")
    }
}

fn start_title(language: &str) -> String {
    if language.to_ascii_lowercase().starts_with("en") {
        "Start".to_string()
    } else {
        "Inicio".to_string()
    }
}

fn page_ranges(starts: &[ChapterStart], page_count: usize) -> Vec<PageRange> {
    let mut ranges = Vec::with_capacity(starts.len());
    for (index, start) in starts.iter().enumerate() {
        let end = starts
            .get(index + 1)
            .map(|next| next.page.saturating_sub(1))
            .unwrap_or_else(|| page_count.saturating_sub(1));
        if start.page <= end {
            ranges.push(PageRange {
                start: start.page,
                end,
            });
        }
    }
    ranges
}

fn build_chapters(
    starts: &[ChapterStart],
    ranges: &[PageRange],
    paragraphs: &[Paragraph],
    _language: &str,
) -> (Vec<EpubChapter>, Vec<PageRange>) {
    let mut chapters = Vec::new();
    let mut used_ranges = Vec::new();
    let mut paragraph_index = 0usize;

    for (index, start) in starts.iter().enumerate() {
        let Some(range) = ranges.get(index).copied() else {
            continue;
        };
        let mut body = String::new();

        while paragraph_index < paragraphs.len()
            && paragraphs[paragraph_index].start_page <= range.end
        {
            let paragraph = &paragraphs[paragraph_index];
            if paragraph.start_page >= range.start {
                let duplicate_title =
                    normalize_repeated(&paragraph.text) == normalize_repeated(&start.title);
                if !duplicate_title {
                    if !body.is_empty() {
                        body.push('\n');
                    }
                    body.push_str(paragraph.text.trim());
                }
            }
            paragraph_index += 1;
        }

        let body = body.trim().to_string();
        if body.is_empty() {
            continue;
        }

        chapters.push(EpubChapter {
            title: start.title.clone(),
            body,
            source: None,
        });
        used_ranges.push(range);
    }

    (chapters, used_ranges)
}

fn chapter_for_page(page: usize, ranges: &[PageRange]) -> Option<usize> {
    ranges
        .iter()
        .position(|range| page >= range.start && page <= range.end)
        .or_else(|| ranges.iter().rposition(|range| range.start <= page))
}

fn search_order(expected: Option<usize>, len: usize) -> Vec<usize> {
    let mut order = Vec::with_capacity(len);
    if let Some(expected) = expected
        && expected < len
    {
        order.push(expected);
    }
    for index in 0..len {
        if !order.contains(&index) {
            order.push(index);
        }
    }
    order
}

fn find_quote(chapter: &ChapterText, quote: &str) -> Option<(usize, usize)> {
    let (haystack, map) = normalize_with_map(&chapter.text);
    let (needle, _) = normalize_with_map(quote);
    let needle = needle.trim();
    if needle.chars().count() < 6 {
        return None;
    }

    let chars: Vec<char> = needle.chars().collect();
    let mut candidates = vec![needle.to_string()];
    for length in [160usize, 100, 60, 30] {
        if chars.len() > length {
            candidates.push(chars[..length].iter().collect());
        }
    }

    for candidate in candidates {
        let Some(start) = haystack.find(&candidate) else {
            continue;
        };
        let end = start + candidate.len();
        let start_byte = *map.get(start)?;
        let end_byte = map.get(end).copied().unwrap_or(chapter.text.len());
        let start_utf16 = utf16_offset(&chapter.text, start_byte);
        let end_utf16 = utf16_offset(&chapter.text, end_byte);
        if end_utf16 > start_utf16 {
            return Some((start_utf16, end_utf16));
        }
    }

    None
}

fn normalize_with_map(text: &str) -> (String, Vec<usize>) {
    let mut normalized = String::with_capacity(text.len());
    let mut map = Vec::with_capacity(text.len());
    let mut last_space = true;

    for (byte, character) in text.char_indices() {
        if character.is_whitespace() || character == '\u{00ad}' {
            if character == '\u{00ad}' {
                continue;
            }
            if !last_space {
                normalized.push(' ');
                map.push(byte);
                last_space = true;
            }
            continue;
        }

        for lower in character.to_lowercase() {
            let start = normalized.len();
            normalized.push(lower);
            let end = normalized.len();
            // `find` devuelve índices de byte; el mapa debe tener una entrada
            // por byte, no por carácter, para no romper con acentos.
            for _ in start..end {
                map.push(byte);
            }
        }
        last_space = false;
    }

    while normalized.ends_with(' ') {
        normalized.pop();
        map.pop();
    }

    (normalized, map)
}

fn utf16_offset(text: &str, byte: usize) -> usize {
    text.get(..byte)
        .map(|prefix| prefix.encode_utf16().count())
        .unwrap_or_else(|| text.encode_utf16().count())
}

fn clean_title(text: &str) -> String {
    let title = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|character: char| {
            character.is_whitespace() || matches!(character, '•' | '·' | '-' | '—' | '|')
        })
        .to_string();
    if title.is_empty() {
        "Capítulo".to_string()
    } else {
        title.chars().take(140).collect()
    }
}

fn normalize_repeated(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_ascii_digit() {
                '#'
            } else {
                character.to_ascii_lowercase()
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_page_number(text: &str) -> bool {
    let compact: String = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    if compact.is_empty() {
        return true;
    }

    if compact.chars().all(|character| character.is_ascii_digit()) {
        return true;
    }

    if compact
        .chars()
        .all(|character| character.is_ascii_digit() || matches!(character, '/' | '-' | '–' | '|'))
    {
        return true;
    }

    let roman = compact.trim_end_matches('.').chars().all(|character| {
        matches!(
            character.to_ascii_uppercase(),
            'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'
        )
    });

    roman && compact.len() <= 8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, x: f32, y: f32, width: f32) -> TextWord {
        TextWord {
            text: text.to_string(),
            x,
            y,
            width,
            height: 10.0,
            font_size: 10.0,
            confidence: None,
        }
    }

    fn page(page: usize, words: Vec<TextWord>) -> PageTextLayer {
        PageTextLayer {
            book_id: "book".into(),
            page,
            width: 600.0,
            height: 800.0,
            source: "native".into(),
            words,
        }
    }

    #[test]
    fn keeps_words_with_varying_baseline_in_one_line() {
        let mut first = word("alfa", 40.0, 100.0, 30.0);
        let mut second = word("beta", 75.0, 106.0, 30.0);
        second.height = 6.0;
        second.font_size = 6.0;
        let mut third = word("gamma", 110.0, 107.0, 35.0);
        third.height = 7.0;
        third.font_size = 7.0;
        let mut fourth = word("delta", 150.0, 101.0, 32.0);
        fourth.height = 11.0;
        fourth.font_size = 11.0;
        first.height = 10.0;

        let output = convert_pages(
            &[page(0, vec![first, second, third, fourth])],
            &[],
            ChapterMode::Pages,
            "es",
        )
        .expect("convertir");
        let body = &output.chapters[0].body;
        assert!(body.contains("alfa beta gamma delta"), "{body}");
    }

    #[test]
    fn joins_lines_and_removes_hyphenation() {
        let pages = vec![page(
            0,
            vec![
                word("extra-", 50.0, 100.0, 40.0),
                word("ordinario", 50.0, 112.0, 55.0),
                word("y", 110.0, 112.0, 8.0),
                word("sigue", 123.0, 112.0, 30.0),
            ],
        )];

        let output = convert_pages(&pages, &[], ChapterMode::Pages, "es").expect("convertir");
        let body = &output.chapters[0].body;
        assert!(body.contains("extraordinario y sigue"), "{body}");
    }

    #[test]
    fn reads_columns_in_order() {
        let mut words = Vec::new();
        for (row, line) in ["izquierda uno", "izquierda dos"].iter().enumerate() {
            words.push(word("izquierda", 40.0, 100.0 + row as f32 * 20.0, 55.0));
            words.push(word(
                line.split(' ').nth(1).unwrap_or("uno"),
                100.0,
                100.0 + row as f32 * 20.0,
                25.0,
            ));
        }
        for (row, line) in ["derecha uno", "derecha dos"].iter().enumerate() {
            words.push(word("derecha", 340.0, 100.0 + row as f32 * 20.0, 50.0));
            words.push(word(
                line.split(' ').nth(1).unwrap_or("uno"),
                395.0,
                100.0 + row as f32 * 20.0,
                25.0,
            ));
        }

        let output =
            convert_pages(&[page(0, words)], &[], ChapterMode::Pages, "es").expect("convertir");
        let body = &output.chapters[0].body;
        let left = body.find("izquierda").expect("columna izquierda");
        let right = body.find("derecha").expect("columna derecha");
        assert!(left < right, "{body}");
    }

    #[test]
    fn removes_small_headers_even_when_text_varies() {
        let mut pages = Vec::new();
        for index in 0..4 {
            let mut header = word(&format!("Encabezado variable {index}"), 40.0, 40.0, 140.0);
            header.height = 6.0;
            header.font_size = 6.0;
            pages.push(page(
                index,
                vec![header, word("Cuerpo del libro", 40.0, 200.0, 110.0)],
            ));
        }

        let output = convert_pages(&pages, &[], ChapterMode::Pages, "es").expect("convertir");
        let body = output
            .chapters
            .iter()
            .map(|chapter| chapter.body.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!body.contains("Encabezado variable"), "{body}");
        assert!(body.contains("Cuerpo del libro"), "{body}");
    }

    #[test]
    fn removes_running_headers_and_page_numbers() {
        let mut pages = Vec::new();
        for index in 0..5 {
            pages.push(page(
                index,
                vec![
                    word("Cabecera repetida", 40.0, 10.0, 120.0),
                    word("Texto del cuerpo", 40.0, 200.0, 110.0),
                    word(&format!("{}", index + 1), 300.0, 780.0, 10.0),
                ],
            ));
        }

        let output = convert_pages(&pages, &[], ChapterMode::Pages, "es").expect("convertir");
        let body = output
            .chapters
            .iter()
            .map(|chapter| chapter.body.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!body.contains("Cabecera repetida"), "{body}");
        assert!(!body.contains(" 1 "), "{body}");
    }

    #[test]
    fn uses_outline_for_chapters() {
        let pages = vec![
            page(0, vec![word("Primer capítulo", 40.0, 100.0, 120.0)]),
            page(1, vec![word("Segundo capítulo", 40.0, 100.0, 130.0)]),
        ];
        let outline = vec![
            AutoIndexEntry {
                title: "Uno".into(),
                page: 0,
                level: 0,
            },
            AutoIndexEntry {
                title: "Dos".into(),
                page: 1,
                level: 0,
            },
        ];

        let output = convert_pages(&pages, &outline, ChapterMode::Auto, "es").expect("convertir");
        assert_eq!(output.chapters.len(), 2);
        assert_eq!(output.chapters[1].title, "Dos");
        assert!(output.used_outline);
    }

    #[test]
    fn converted_chapters_round_trip_through_epub() {
        let pages = vec![
            page(0, vec![word("Capítulo uno", 40.0, 100.0, 100.0)]),
            page(
                1,
                vec![word("Texto del segundo capítulo.", 40.0, 100.0, 180.0)],
            ),
        ];
        let outline = vec![
            AutoIndexEntry {
                title: "Uno".into(),
                page: 0,
                level: 0,
            },
            AutoIndexEntry {
                title: "Dos".into(),
                page: 1,
                level: 0,
            },
        ];
        let output = convert_pages(&pages, &outline, ChapterMode::Auto, "es").expect("convertir");
        let workspace = tempfile::tempdir().expect("tmp");
        let path = workspace.path().join("convertido.epub");
        crate::tejido::epub::build_epub(&path, "Convertido", "Kora", "es", &output.chapters, None)
            .expect("epub");

        let texts = chapter_texts(&path).expect("leer epub");
        assert_eq!(texts.len(), output.chapters.len());
        assert!(
            texts[0].text.contains("Capítulo uno"),
            "{:?}",
            texts[0].text
        );
        assert!(
            texts[1].text.contains("Texto del segundo capítulo"),
            "{:?}",
            texts[1].text
        );
    }

    #[test]
    fn relocates_quotes_with_accents() {
        let text = "El zorro saltó sobre el perro.";
        let chapters = vec![ChapterText {
            text: text.into(),
            utf16_len: text.encode_utf16().count(),
        }];
        let ranges = vec![PageRange { start: 0, end: 0 }];
        let items = vec![RelocationInput {
            id: "acento".into(),
            page: Some(0),
            quote: Some("saltó sobre".into()),
        }];

        let relocated = relocate_items(&items, &chapters, &ranges);
        assert_eq!(relocated[0].status, RelocationStatus::Anchored);
        assert_eq!(relocated[0].offset_start, 9);
        assert_eq!(relocated[0].offset_end, 20);
    }

    #[test]
    fn relocates_quotes_and_falls_back_to_pages() {
        let chapters = vec![
            ChapterText {
                text: "El zorro salta sobre el perro.".into(),
                utf16_len: "El zorro salta sobre el perro.".encode_utf16().count(),
            },
            ChapterText {
                text: "Un segundo capítulo distinto.".into(),
                utf16_len: "Un segundo capítulo distinto.".encode_utf16().count(),
            },
        ];
        let ranges = vec![
            PageRange { start: 0, end: 4 },
            PageRange { start: 5, end: 9 },
        ];
        let items = vec![
            RelocationInput {
                id: "a".into(),
                page: Some(1),
                quote: Some("zorro salta".into()),
            },
            RelocationInput {
                id: "b".into(),
                page: Some(7),
                quote: Some("no aparece".into()),
            },
        ];

        let relocated = relocate_items(&items, &chapters, &ranges);
        assert_eq!(relocated[0].status, RelocationStatus::Anchored);
        assert_eq!(relocated[0].chapter_index, 0);
        assert!(relocated[0].offset_start > 0);
        assert_eq!(relocated[1].status, RelocationStatus::Approximate);
        assert_eq!(relocated[1].chapter_index, 1);
    }
}
