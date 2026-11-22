use std::path::Path;

use crate::reader::{PdfParser, ReaderService, extract_text};
use crate::storage::hash::sha256_bytes;
use crate::storage::{Book, BookFormat};

use super::normalize::{normalize_key, truncate_chars, truncate_words};

const MAX_DESCRIPTION_CHARS: usize = 2_400;
const MAX_SAMPLE_WORDS: usize = 1_200;
const MAX_EMBEDDING_CHARS: usize = 2_400;

/// Ficha de un libro construida a partir de metadatos y una muestra de texto.
/// La clasificación nunca depende de un único campo: cuantas más señales, mejor.
#[derive(Debug, Clone, Default)]
pub struct BookProfile {
    pub title: String,
    pub author: Option<String>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub description: Option<String>,
    pub subjects: Vec<String>,
    pub keywords: Vec<String>,
    pub sample_text: String,
}

impl BookProfile {
    pub fn embedding_text(&self) -> String {
        let mut parts = Vec::new();
        parts.push(format!("Título: {}", self.title));
        if let Some(author) = &self.author {
            parts.push(format!("Autor: {author}"));
        }
        if let Some(publisher) = &self.publisher {
            parts.push(format!("Editorial: {publisher}"));
        }
        if let Some(language) = &self.language {
            parts.push(format!("Idioma: {language}"));
        }
        if !self.subjects.is_empty() {
            parts.push(format!("Materias: {}", self.subjects.join("; ")));
        }
        if !self.keywords.is_empty() {
            parts.push(format!("Palabras clave: {}", self.keywords.join("; ")));
        }
        if let Some(description) = &self.description {
            parts.push(format!("Descripción: {description}"));
        }
        if !self.sample_text.is_empty() {
            parts.push(format!("Fragmento: {}", self.sample_text));
        }
        truncate_chars(&parts.join("\n"), MAX_EMBEDDING_CHARS)
    }

    /// Huella estable de las señales de entrada. Si no cambia, no hace falta
    /// repetir la clasificación automática.
    pub fn input_hash(&self) -> String {
        let mut subjects = self.subjects.clone();
        subjects.sort_unstable();
        let mut keywords = self.keywords.clone();
        keywords.sort_unstable();
        let joined = format!(
            "{}\u{0}{}\u{0}{}\u{0}{}\u{0}{}\u{0}{}\u{0}{}\u{0}{}",
            self.title,
            self.author.as_deref().unwrap_or_default(),
            self.publisher.as_deref().unwrap_or_default(),
            self.language.as_deref().unwrap_or_default(),
            self.description.as_deref().unwrap_or_default(),
            subjects.join("\u{1}"),
            keywords.join("\u{1}"),
            self.sample_text,
        );
        sha256_bytes(joined.as_bytes())
    }

    pub fn metadata_terms(&self) -> impl Iterator<Item = &str> {
        self.subjects
            .iter()
            .chain(self.keywords.iter())
            .map(String::as_str)
    }

    /// Términos que merecen convertirse en alias cuando el usuario acepta una
    /// categoría nueva. Las materias BISAC se separan por niveles para quedarse
    /// con los términos específicos.
    pub fn learning_aliases(&self) -> Vec<String> {
        let mut aliases = Vec::new();
        'terms: for term in self.metadata_terms() {
            for segment in term.split(['/', ';', ',', '·']) {
                let trimmed = segment.trim();
                let normalized = normalize_key(trimmed);
                if normalized.len() < 4 || normalized.len() > 80 {
                    continue;
                }
                if matches!(
                    normalized.as_str(),
                    "fiction"
                        | "general"
                        | "novel"
                        | "books"
                        | "book"
                        | "literature"
                        | "unknown"
                        | "other"
                ) {
                    continue;
                }
                if aliases
                    .iter()
                    .any(|alias: &String| normalize_key(alias) == normalized)
                {
                    continue;
                }
                aliases.push(trimmed.to_string());
                if aliases.len() >= 6 {
                    break 'terms;
                }
            }
        }
        aliases
    }
}

/// Construye la ficha. `book_path` apunta al EPUB/PDF y `audio_path` al audio
/// de un audiolibro importado sin transcripción. Ninguna lectura es crítica:
/// si falla, se conserva lo que haya en la base de datos.
pub fn build_profile(
    book: &Book,
    book_path: Option<&Path>,
    audio_path: Option<&Path>,
) -> BookProfile {
    let mut profile = BookProfile {
        title: book.title.clone(),
        author: book.author.clone(),
        publisher: book.publisher.clone(),
        language: book.language.clone(),
        description: book.description.clone(),
        ..BookProfile::default()
    };

    if book.audio_only {
        enrich_from_audio(&mut profile, audio_path);
    } else {
        match book.format {
            BookFormat::Epub => enrich_from_epub(&mut profile, book_path),
            BookFormat::Pdf => enrich_from_pdf(&mut profile, book_path),
        }
    }

    profile.subjects.sort_unstable();
    profile.subjects.dedup();
    profile.keywords.sort_unstable();
    profile.keywords.dedup();
    profile.description = profile
        .description
        .as_deref()
        .map(|description| truncate_chars(description.trim(), MAX_DESCRIPTION_CHARS))
        .filter(|description| !description.is_empty());
    profile.sample_text = truncate_words(profile.sample_text.trim(), MAX_SAMPLE_WORDS);
    profile
}

fn enrich_from_epub(profile: &mut BookProfile, book_path: Option<&Path>) {
    let Some(path) = book_path else {
        return;
    };
    let reader = ReaderService::new();
    let inspection = match reader.inspect_epub(path) {
        Ok(inspection) => inspection,
        Err(error) => {
            tracing::warn!(%error, "no se pudo inspeccionar el EPUB para clasificarlo");
            return;
        }
    };

    if let Some(title) = inspection.title
        && !title.trim().is_empty()
    {
        profile.title = title;
    }
    profile.author = inspection.author.or_else(|| profile.author.take());
    profile.publisher = inspection.publisher.or_else(|| profile.publisher.take());
    profile.language = inspection.language.or_else(|| profile.language.take());
    profile.description = inspection
        .description
        .or_else(|| profile.description.take());
    profile.subjects.extend(inspection.subjects);

    let mut sample = String::new();
    for index in 0..inspection.sections.len().min(2) {
        let Ok(chapter) = reader.epub_chapter(path, index) else {
            continue;
        };
        let text = extract_text(&chapter.html);
        if text.is_empty() {
            continue;
        }
        if !sample.is_empty() {
            sample.push('\n');
        }
        sample.push_str(&text.text);
        if sample.split_whitespace().count() >= MAX_SAMPLE_WORDS {
            break;
        }
    }
    profile.sample_text = sample;
}

fn enrich_from_pdf(profile: &mut BookProfile, book_path: Option<&Path>) {
    let Some(path) = book_path else {
        return;
    };
    let parser = PdfParser::new();
    if let Ok(inspection) = parser.inspect(path) {
        if let Some(title) = inspection.title
            && !title.trim().is_empty()
        {
            profile.title = title;
        }
        profile.author = inspection.author.or_else(|| profile.author.take());
        profile.subjects.extend(inspection.subjects);
        profile.keywords.extend(inspection.keywords);
    }

    let mut sample = String::new();
    if let Ok(layer) = parser.page_text(path, 0) {
        sample = layer.text();
    }
    profile.sample_text = sample;
}

fn enrich_from_audio(profile: &mut BookProfile, audio_path: Option<&Path>) {
    let Some(path) = audio_path else {
        return;
    };
    let probe = match crate::audiobooks::m4b::probe(path) {
        Ok(probe) => probe,
        Err(error) => {
            tracing::warn!(%error, "no se pudo leer el audio para clasificarlo");
            return;
        }
    };

    if profile.title.trim().is_empty() {
        profile.title = probe.title.unwrap_or_else(|| "Audiolibro".to_string());
    }
    profile.author = profile.author.take().or(probe.artist);
    profile.language = profile.language.take().or(probe.language);
    if let Some(genre) = probe.genre {
        profile.subjects.push(genre);
    }
    if let Some(comment) = probe.comment {
        profile.keywords.extend(
            comment
                .split([',', ';', '.'])
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(str::to_string),
        );
    }
    profile.sample_text = probe
        .chapters
        .iter()
        .take(24)
        .map(|chapter| chapter.title.as_str())
        .collect::<Vec<_>>()
        .join(" · ");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learns_specific_bisac_segments() {
        let profile = BookProfile {
            subjects: vec!["Fiction / Fantasy / Epic".to_string()],
            ..BookProfile::default()
        };
        let aliases = profile.learning_aliases();
        assert!(aliases.iter().any(|alias| alias == "Fantasy"));
        assert!(aliases.iter().any(|alias| alias == "Epic"));
        assert!(!aliases.iter().any(|alias| alias == "Fiction"));
    }
}
