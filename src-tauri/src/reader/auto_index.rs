use std::collections::{HashMap, HashSet};

use crate::storage::AutoIndexEntry;

use super::model::PageTextLayer;
use super::pdf::{IndexLine, words_to_lines};

const MAX_ENTRIES: usize = 400;
const MAX_TITLE_CHARS: usize = 140;

/// Genera un índice a partir de líneas ya reconstruidas. La función es pura
/// para poder probar las heurísticas sin PDFium ni Tesseract.
pub fn build_from_lines(lines: &[IndexLine]) -> Vec<AutoIndexEntry> {
    if lines.is_empty() {
        return Vec::new();
    }

    let median = median_font_size(lines);
    let page_count = lines.iter().map(|line| line.page).max().unwrap_or(0) + 1;
    let repeated = repeated_margin_lines(lines, page_count);

    let mut entries: Vec<AutoIndexEntry> = Vec::new();
    let mut seen = HashSet::new();

    for line in lines {
        let title = clean_title(&line.text);
        if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
            continue;
        }
        if is_page_number(&title) {
            continue;
        }
        if repeated.contains(&normalize_repeated(&title)) {
            continue;
        }

        let pattern = pattern_level(&title);
        let in_margin = line.y < line.page_height * 0.055 || line.y > line.page_height * 0.945;
        let large = line.font_size >= median * 1.16;
        let very_large = line.font_size >= median * 1.35;
        let short = title.chars().count() <= 90;
        let upper = is_mostly_upper(&title);

        let candidate = pattern.is_some()
            || (large && short && !in_margin)
            || (upper && large && short)
            || (very_large && short);

        if !candidate {
            continue;
        }

        // Los encabezados de capítulo suelen ser cortos; una línea larga que
        // solo destaca por tamaño probablemente sea una portada o un aviso.
        if pattern.is_none() && title.chars().count() > 70 {
            continue;
        }

        let level = pattern.unwrap_or(if very_large { 0 } else { 1 });

        if seen.insert((line.page, title.clone())) {
            entries.push(AutoIndexEntry {
                title,
                page: line.page,
                level,
            });
        }

        if entries.len() >= MAX_ENTRIES {
            break;
        }
    }

    entries.sort_by_key(|entry| (entry.page, entry.level));
    entries
}

/// Índice a partir de las capas OCR ya guardadas (PDFs escaneados).
pub fn build_from_ocr_pages(pages: &[PageTextLayer]) -> Vec<AutoIndexEntry> {
    let mut lines = Vec::new();

    for page in pages {
        if page.words.is_empty() {
            continue;
        }
        lines.extend(words_to_lines(&page.words, page.page, page.height.max(1.0)));
    }

    build_from_lines(&lines)
}

fn median_font_size(lines: &[IndexLine]) -> f32 {
    let mut sizes: Vec<f32> = lines
        .iter()
        .map(|line| line.font_size)
        .filter(|size| size.is_finite() && *size > 0.0)
        .collect();

    if sizes.is_empty() {
        return 10.0;
    }

    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let middle = sizes.len() / 2;
    if sizes.len().is_multiple_of(2) {
        (sizes[middle - 1] + sizes[middle]) / 2.0
    } else {
        sizes[middle]
    }
}

fn repeated_margin_lines(lines: &[IndexLine], page_count: usize) -> HashSet<String> {
    if page_count < 4 {
        return HashSet::new();
    }

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut pages_seen: HashMap<String, HashSet<usize>> = HashMap::new();

    for line in lines {
        let in_margin = line.y < line.page_height * 0.09 || line.y > line.page_height * 0.91;
        if !in_margin {
            continue;
        }

        let key = normalize_repeated(&line.text);
        if key.is_empty() || key.chars().count() > 80 {
            continue;
        }

        let pages = pages_seen.entry(key.clone()).or_default();
        if pages.insert(line.page) {
            *counts.entry(key).or_default() += 1;
        }
    }

    let threshold = (page_count / 5).max(3);
    counts
        .into_iter()
        .filter(|(_, count)| *count >= threshold)
        .map(|(key, _)| key)
        .collect()
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

fn clean_title(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|character: char| {
            character.is_whitespace() || matches!(character, '•' | '·' | '-' | '—' | '|')
        })
        .to_string()
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

    let roman = compact.trim_end_matches('.').chars().all(|character| {
        matches!(
            character.to_ascii_uppercase(),
            'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'
        )
    });

    roman && compact.len() <= 8
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

fn pattern_level(text: &str) -> Option<usize> {
    let lower = text.to_lowercase();
    let trimmed = lower.trim_start();

    const KEYWORDS: &[&str] = &[
        "capítulo",
        "capitulo",
        "chapter",
        "parte",
        "part ",
        "sección",
        "seccion",
        "section",
        "libro",
        "book ",
        "prólogo",
        "prologo",
        "prologue",
        "epílogo",
        "epilogo",
        "epilogue",
        "introducción",
        "introduccion",
        "introduction",
        "apéndice",
        "apendice",
        "appendix",
        "índice",
        "indice",
        "contents",
        "prefacio",
        "preface",
    ];

    if KEYWORDS.iter().any(|keyword| trimmed.starts_with(keyword)) {
        return Some(0);
    }

    // "1.2 Título" o "3 Título": nivel según la profundidad numérica.
    let numeric: String = trimmed
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '.')
        .collect();
    if !numeric.is_empty()
        && numeric.chars().any(|character| character.is_ascii_digit())
        && trimmed
            .chars()
            .nth(numeric.chars().count())
            .is_some_and(|character| character.is_whitespace())
    {
        let depth = numeric.matches('.').count();
        return Some(depth.min(3));
    }

    // "IV. Título" o "XII Título".
    let roman: String = trimmed
        .chars()
        .take_while(|character| {
            matches!(
                character.to_ascii_uppercase(),
                'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'
            )
        })
        .collect();
    if roman.len() >= 2
        && trimmed
            .chars()
            .nth(roman.chars().count())
            .is_some_and(|character| character == '.' || character.is_whitespace())
    {
        return Some(1);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, page: usize, font_size: f32, y: f32) -> IndexLine {
        IndexLine {
            text: text.to_string(),
            page,
            font_size,
            y,
            page_height: 800.0,
            bold: false,
        }
    }

    #[test]
    fn detects_chapter_patterns_and_levels() {
        let lines = vec![
            line("Capítulo 1", 0, 18.0, 120.0),
            line("1.2 Una sección", 3, 14.0, 200.0),
            line("Texto normal del cuerpo", 3, 10.0, 240.0),
            line("Chapter 2", 10, 18.0, 120.0),
        ];

        let index = build_from_lines(&lines);
        assert_eq!(index.len(), 3);
        assert_eq!(index[0].title, "Capítulo 1");
        assert_eq!(index[0].level, 0);
        assert_eq!(index[1].title, "1.2 Una sección");
        assert_eq!(index[1].level, 1);
        assert_eq!(index[2].title, "Chapter 2");
    }

    #[test]
    fn ignores_body_text_and_page_numbers() {
        let lines = vec![
            line(
                "Esta es una frase normal del cuerpo del libro.",
                1,
                10.0,
                300.0,
            ),
            line("42", 2, 10.0, 780.0),
            line("xii", 3, 10.0, 20.0),
        ];

        assert!(build_from_lines(&lines).is_empty());
    }

    #[test]
    fn ignores_repeated_headers() {
        let mut lines = Vec::new();
        for page in 0..10 {
            lines.push(line("Kora Reader", page, 9.0, 15.0));
            lines.push(line("Texto del cuerpo", page, 10.0, 300.0));
        }
        lines.push(line("Capítulo 1", 0, 20.0, 120.0));

        let index = build_from_lines(&lines);
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].title, "Capítulo 1");
    }

    #[test]
    fn large_heading_without_keyword_is_detected() {
        let lines = vec![
            line("La ciudad y los perros", 0, 24.0, 100.0),
            line("Texto normal", 0, 10.0, 200.0),
        ];

        let index = build_from_lines(&lines);
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].level, 0);
    }
}
