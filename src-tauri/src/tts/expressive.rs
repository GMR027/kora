use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::reader::{SourceText, TextWord};
use crate::storage::OffsetUnit;

/// Personaje reservado para la narración.
pub const NARRATOR: &str = "Narrador";
/// Personaje reservado para diálogos de los que no se deduce el hablante.
pub const DIALOGUE: &str = "Diálogo";

const MAX_CHUNK_CHARS: usize = 240;
// v2: el texto hablado pasa por `sanitize_speech` (símbolos y puntuación
// repetida ya no se verbalizan). Las secuencias v1 cacheadas se regeneran.
const SEQUENCE_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpressiveSettings {
    pub enabled: bool,
    pub pause_scale: f32,
    pub emphasis: f32,
}

impl Default for ExpressiveSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            pause_scale: 1.0,
            emphasis: 1.0,
        }
    }
}

impl ExpressiveSettings {
    pub fn clamped(self) -> Self {
        Self {
            enabled: self.enabled,
            pause_scale: self.pause_scale.clamp(0.0, 3.0),
            emphasis: self.emphasis.clamp(0.0, 2.0),
        }
    }
}

/// Un fragmento listo para el motor. `offset_end` es exclusivo en ambas
/// unidades: en EPUB son unidades UTF-16 y en PDF índices de palabra.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechChunk {
    pub index: usize,
    pub section: usize,
    pub offset_start: usize,
    pub offset_end: usize,
    pub text: String,
    pub speech: String,
    pub character: Option<String>,
    pub pause_before_ms: u32,
    pub pause_after_ms: u32,
    pub rate: f32,
    pub pitch: f32,
    pub emphasis: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterInfo {
    pub name: String,
    pub lines: usize,
    pub sample: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechSequence {
    pub version: u32,
    pub section: usize,
    pub unit: OffsetUnit,
    pub chunks: Vec<SpeechChunk>,
    pub characters: Vec<CharacterInfo>,
}

impl SpeechSequence {
    pub fn new(section: usize, unit: OffsetUnit, chunks: Vec<SpeechChunk>) -> Self {
        let characters = detect_characters(&chunks);
        Self {
            version: SEQUENCE_VERSION,
            section,
            unit,
            chunks,
            characters,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }
}

pub fn content_hash(text: &str, settings: &ExpressiveSettings) -> String {
    let settings = settings.clamped();
    let mut hasher = Sha256::new();
    hasher.update(SEQUENCE_VERSION.to_le_bytes());
    hasher.update([settings.enabled as u8]);
    hasher.update(settings.pause_scale.to_le_bytes());
    hasher.update(settings.emphasis.to_le_bytes());
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn build_from_text(
    section: usize,
    source: &SourceText,
    settings: &ExpressiveSettings,
) -> SpeechSequence {
    let settings = settings.clamped();
    let mut chunks = Vec::new();
    let breaks: Vec<usize> = source.breaks.clone();
    let chars: Vec<(usize, char)> = source.text.char_indices().collect();
    let mut utf16 = Vec::with_capacity(chars.len() + 1);
    utf16.push(0);
    for (_, ch) in &chars {
        let last = *utf16.last().unwrap_or(&0);
        utf16.push(last + ch.len_utf16());
    }

    let mut start: Option<usize> = None;
    let mut index = 0usize;

    while index < chars.len() {
        let (byte, ch) = chars[index];

        if start.is_none() && !ch.is_whitespace() {
            start = Some(index);
        }

        let mut consumed = 1usize;
        let mut close = false;

        if start.is_some() {
            let ends_sentence = is_terminator(ch)
                && is_sentence_end(
                    &source.text,
                    byte,
                    ch,
                    chars.get(index + 1).map(|(_, c)| *c),
                );
            if ends_sentence || ch == '\n' {
                close = true;
            }
        }

        // Un bloque (`p`, `div`, `li`…) también cierra la frase.
        let after_utf16 = utf16[index + 1];
        let block_break = breaks.binary_search(&after_utf16).is_ok();

        if (close || block_break)
            && let Some(start_index) = start.take()
        {
            let mut end_index = index + 1;
            if close {
                while end_index < chars.len() && is_closing(chars[end_index].1) {
                    end_index += 1;
                }
            }
            let end_byte = if end_index < chars.len() {
                chars[end_index].0
            } else {
                source.text.len()
            };
            let start_byte = chars[start_index].0;
            let start_utf16 = utf16[start_index];
            let end_utf16 = if end_index < chars.len() {
                utf16[end_index]
            } else {
                source.utf16_len
            };
            push_with_limit(
                &mut chunks,
                section,
                &source.text,
                start_byte,
                end_byte,
                start_utf16,
                end_utf16,
                &settings,
                block_break,
            );
            consumed = end_index.saturating_sub(index).max(1);
        }

        index += consumed;
    }

    if let Some(start_index) = start {
        push_with_limit(
            &mut chunks,
            section,
            &source.text,
            chars[start_index].0,
            source.text.len(),
            utf16[start_index],
            source.utf16_len,
            &settings,
            false,
        );
    }

    for (i, chunk) in chunks.iter_mut().enumerate() {
        chunk.index = i;
    }

    SpeechSequence::new(section, OffsetUnit::Utf16, chunks)
}

pub fn build_from_words(
    section: usize,
    words: &[TextWord],
    settings: &ExpressiveSettings,
) -> SpeechSequence {
    let settings = settings.clamped();
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut text = String::new();

    for (index, word) in words.iter().enumerate() {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(word.text.trim());

        let ends = ends_sentence(&word.text);
        if ends || text.chars().count() >= MAX_CHUNK_CHARS {
            push_word_chunk(&mut chunks, section, start, index + 1, &text, &settings);
            start = index + 1;
            text.clear();
        }
    }

    if start < words.len() {
        push_word_chunk(&mut chunks, section, start, words.len(), &text, &settings);
    }

    for (i, chunk) in chunks.iter_mut().enumerate() {
        chunk.index = i;
    }

    SpeechSequence::new(section, OffsetUnit::Word, chunks)
}

fn push_word_chunk(
    chunks: &mut Vec<SpeechChunk>,
    section: usize,
    start: usize,
    end: usize,
    raw: &str,
    settings: &ExpressiveSettings,
) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }

    let chunk = make_chunk(chunks.len(), section, start, end, trimmed, settings, false);
    chunks.push(chunk);
}

#[allow(clippy::too_many_arguments)]
fn push_with_limit(
    chunks: &mut Vec<SpeechChunk>,
    section: usize,
    text: &str,
    start_byte: usize,
    end_byte: usize,
    start_utf16: usize,
    end_utf16: usize,
    settings: &ExpressiveSettings,
    paragraph: bool,
) {
    let mut cursor = start_byte;
    let mut cursor_utf16 = start_utf16;

    while cursor < end_byte {
        let remaining = &text[cursor..end_byte];
        if remaining.encode_utf16().count() <= MAX_CHUNK_CHARS {
            push_text_piece(
                chunks,
                section,
                text,
                cursor,
                end_byte,
                cursor_utf16,
                end_utf16,
                settings,
                paragraph,
            );
            break;
        }

        let mut last_space = None;
        let mut last_punct = None;
        let mut split = None;

        for (count, (offset, ch)) in remaining.char_indices().enumerate() {
            if count >= MAX_CHUNK_CHARS {
                split = Some(
                    last_punct
                        .filter(|position| *position > cursor + (offset / 3).max(1))
                        .or(last_space)
                        .unwrap_or(cursor + offset),
                );
                break;
            }

            let absolute = cursor + offset;
            if ch.is_whitespace() {
                last_space = Some(absolute + ch.len_utf8());
            }
            if matches!(ch, ',' | ';' | ':' | '—') {
                last_punct = Some(absolute + ch.len_utf8());
            }
        }

        let split = split.unwrap_or(end_byte);
        let split_utf16 = cursor_utf16 + text[cursor..split].encode_utf16().count();
        push_text_piece(
            chunks,
            section,
            text,
            cursor,
            split,
            cursor_utf16,
            split_utf16,
            settings,
            paragraph,
        );
        cursor = split;
        cursor_utf16 = split_utf16;
    }
}

#[allow(clippy::too_many_arguments)]
fn push_text_piece(
    chunks: &mut Vec<SpeechChunk>,
    section: usize,
    text: &str,
    start_byte: usize,
    end_byte: usize,
    start_utf16: usize,
    end_utf16: usize,
    settings: &ExpressiveSettings,
    paragraph: bool,
) {
    let raw = &text[start_byte..end_byte];
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }

    let leading = &raw[..raw.len() - raw.trim_start().len()];
    let start = start_utf16 + leading.encode_utf16().count();
    let end = start + trimmed.encode_utf16().count();
    let _ = end_utf16;

    let chunk = make_chunk(
        chunks.len(),
        section,
        start,
        end,
        trimmed,
        settings,
        paragraph,
    );
    chunks.push(chunk);
}

fn make_chunk(
    index: usize,
    section: usize,
    offset_start: usize,
    offset_end: usize,
    text: &str,
    settings: &ExpressiveSettings,
    paragraph: bool,
) -> SpeechChunk {
    let (character, dialogue) = classify(text, settings.enabled);
    let (rate, pitch, emphasis) = prosody(text, dialogue, settings);
    let pause_after = pause_after(text, paragraph, dialogue, settings);
    let pause_before = 0;

    SpeechChunk {
        index,
        section,
        offset_start,
        offset_end,
        text: text.to_string(),
        speech: normalize_speech(text),
        character,
        pause_before_ms: pause_before,
        pause_after_ms: pause_after,
        rate,
        pitch,
        emphasis,
    }
}

fn classify(text: &str, expressive: bool) -> (Option<String>, bool) {
    if !expressive {
        return (None, false);
    }

    let trimmed = text.trim();
    let dialogue = looks_like_dialogue(trimmed);
    if !dialogue {
        if let Some(name) = script_speaker(trimmed) {
            return (Some(name), true);
        }
        return (None, false);
    }

    match speaker_name(trimmed) {
        Some(name) => (Some(name), true),
        None => (Some(DIALOGUE.to_string()), true),
    }
}

fn looks_like_dialogue(text: &str) -> bool {
    let start = text.trim_start();
    if start.starts_with(['—', '–', '«', '“', '"']) {
        return true;
    }

    let quotes = start.matches('"').count();
    start.contains('«') || start.contains('“') || start.contains('”') || quotes >= 2
}

fn speaker_name(text: &str) -> Option<String> {
    static AFTER: OnceLock<Regex> = OnceLock::new();
    static BEFORE: OnceLock<Regex> = OnceLock::new();

    let after = AFTER.get_or_init(|| {
        Regex::new(
            r"(?i:\b(?:dijo|preguntó|respondió|exclamó|murmuró|gritó|añadió|contestó|pensó|susurró|replicó|ordenó|advirtió|comentó|protestó|suspiró|rio|ríe))\s+([A-ZÁÉÍÓÚÜÑ][\p{L}'-]+(?:\s+[A-ZÁÉÍÓÚÜÑ][\p{L}'-]+)?)",
        )
        .expect("regex de diálogo válida")
    });
    if let Some(captures) = after.captures(text)
        && let Some(name) = captures.get(1)
        && let Some(name) = clean_name(name.as_str())
    {
        return Some(name);
    }

    let before = BEFORE.get_or_init(|| {
        Regex::new(
            r"([A-ZÁÉÍÓÚÜÑ][\p{L}'-]+(?:\s+[A-ZÁÉÍÓÚÜÑ][\p{L}'-]+)?)\s+(?i:(?:dijo|preguntó|respondió|exclamó|murmuró|gritó|añadió|contestó|pensó|susurró|replicó|ordenó|advirtió|comentó|protestó|suspiró))",
        )
        .expect("regex de diálogo válida")
    });
    if let Some(captures) = before.captures(text)
        && let Some(name) = captures.get(1)
        && let Some(name) = clean_name(name.as_str())
    {
        return Some(name);
    }

    None
}

fn script_speaker(text: &str) -> Option<String> {
    static SCRIPT: OnceLock<Regex> = OnceLock::new();
    let script = SCRIPT.get_or_init(|| {
        Regex::new(r#"^([A-ZÁÉÍÓÚÜÑ][\p{L}'-]+(?:\s+[A-ZÁÉÍÓÚÜÑ][\p{L}'-]+)?)\s*:\s*[—–"«“]"#)
            .expect("regex de guion válida")
    });

    let captures = script.captures(text)?;
    clean_name(captures.get(1)?.as_str())
}

fn clean_name(raw: &str) -> Option<String> {
    const STOPWORDS: &[&str] = &[
        "el",
        "la",
        "los",
        "las",
        "un",
        "una",
        "unos",
        "unas",
        "él",
        "ella",
        "ellos",
        "ellas",
        "yo",
        "tú",
        "usted",
        "nosotros",
        "vosotros",
        "esto",
        "eso",
        "aquello",
        "alguien",
        "nadie",
        "todos",
        "todo",
        "nada",
        "que",
        "quien",
        "quién",
        "como",
        "cuando",
        "donde",
        "the",
        "a",
        "an",
        "he",
        "she",
        "they",
        "i",
        "we",
        "you",
        "it",
        "this",
        "that",
        "someone",
        "nobody",
        "everyone",
        "something",
        "nothing",
        "who",
        "what",
        "when",
        "where",
        "and",
        "but",
        "then",
    ];

    let name = raw
        .trim_matches(|c: char| !c.is_alphabetic() && c != '\'' && c != '-')
        .trim();
    let name = name
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");

    if name.chars().count() < 2 {
        return None;
    }
    if STOPWORDS.contains(&name.to_lowercase().as_str()) {
        return None;
    }

    Some(name)
}

fn prosody(text: &str, dialogue: bool, settings: &ExpressiveSettings) -> (f32, f32, f32) {
    if !settings.enabled {
        return (1.0, 1.0, 0.0);
    }

    let trimmed = text.trim_end();
    let mut rate = 1.0;
    let mut pitch = 1.0;
    let mut emphasis: f32 = 0.0;

    if dialogue {
        rate *= 1.02;
        pitch *= 1.04;
        emphasis = 0.35;
    }
    if trimmed.ends_with('!') {
        rate *= 1.04;
        pitch *= 1.03;
        emphasis = emphasis.max(0.7);
    }
    if trimmed.ends_with('?') {
        rate *= 0.98;
        pitch *= 1.05;
        emphasis = emphasis.max(0.3);
    }
    if trimmed.ends_with('…') {
        rate *= 0.96;
    }
    if is_shouting(trimmed) {
        emphasis = emphasis.max(0.65);
        rate *= 0.97;
    }

    (
        rate,
        pitch,
        1.0 + (emphasis * settings.emphasis).clamp(0.0, 1.5),
    )
}

fn is_shouting(text: &str) -> bool {
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    if letters < 4 {
        return false;
    }
    let upper = text.chars().filter(|c| c.is_uppercase()).count();
    upper as f32 / letters as f32 > 0.7
}

fn pause_after(text: &str, paragraph: bool, dialogue: bool, settings: &ExpressiveSettings) -> u32 {
    let trimmed = text.trim_end();
    let mut ms = if trimmed.ends_with('…') {
        380
    } else if trimmed.ends_with('!') || trimmed.ends_with('?') {
        230
    } else {
        180
    };

    if paragraph {
        ms += 220;
    }
    if dialogue {
        ms += 40;
    }

    scaled(ms, settings)
}

fn scaled(value: u32, settings: &ExpressiveSettings) -> u32 {
    let scale = if settings.enabled {
        settings.pause_scale
    } else {
        1.0
    };
    ((value as f32) * scale).clamp(0.0, 1500.0) as u32
}

fn normalize_speech(text: &str) -> String {
    sanitize_speech(text)
}

/// Limpia el texto que se envía al motor de voz sin tocar el texto visible.
///
/// Los motores TTS tienden a verbalizar símbolos como `??`, `____`, `$`, `%`
/// o `^`. Aquí se conservan letras, números, acentos y la puntuación que aporta
/// prosodia; el resto se convierte en separación y las repeticiones de
/// puntuación se reducen a una sola.
pub fn sanitize_speech(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;

    let chars: Vec<char> = text.nfc().collect();
    let mut output = String::with_capacity(text.len());
    let mut last_was_space = true;

    for (index, ch) in chars.iter().copied().enumerate() {
        let previous = index
            .checked_sub(1)
            .and_then(|value| chars.get(value))
            .copied();
        let next = chars.get(index + 1).copied();

        let mapped = match ch {
            '\u{00A0}' | '\u{2007}' | '\u{202F}' => Some(' '),
            '\n' | '\r' | '\t' => Some(' '),
            '…' => Some('.'),
            '.' | ',' | ';' | ':' | '!' | '?' => Some(ch),
            '¿' | '¡' => None,
            '\'' | '’' => {
                if previous.is_some_and(|value| value.is_alphanumeric())
                    && next.is_some_and(|value| value.is_alphanumeric())
                {
                    Some('\'')
                } else {
                    None
                }
            }
            value if value.is_alphanumeric() => Some(value),
            value if value.is_whitespace() => Some(' '),
            _ => Some(' '),
        };

        let Some(mapped) = mapped else {
            continue;
        };

        if mapped == ' ' {
            if last_was_space {
                continue;
            }
            last_was_space = true;
            output.push(' ');
            continue;
        }

        if is_speech_punctuation(mapped) {
            while output.ends_with(' ') {
                output.pop();
            }
            if output.is_empty() {
                continue;
            }
            match output.chars().next_back() {
                Some(previous) if is_speech_punctuation(previous) => {
                    if matches!(mapped, '?' | '!') && !matches!(previous, '?' | '!') {
                        output.pop();
                        output.push(mapped);
                    }
                }
                _ => output.push(mapped),
            }
        } else {
            output.push(mapped);
        }
        last_was_space = false;
    }

    output.trim().to_string()
}

fn is_speech_punctuation(ch: char) -> bool {
    matches!(ch, '.' | ',' | ';' | ':' | '!' | '?')
}

fn ends_sentence(word: &str) -> bool {
    matches!(
        word.trim_end()
            .trim_end_matches(['"', '\'', '»', '”', ')', ']'])
            .chars()
            .last(),
        Some('.' | '!' | '?' | '…')
    )
}

fn is_terminator(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?' | '…')
}

fn is_closing(ch: char) -> bool {
    matches!(ch, '"' | '»' | '”' | '’' | '\'' | ')' | ']')
}

fn is_sentence_end(text: &str, byte: usize, ch: char, next: Option<char>) -> bool {
    if let Some(next) = next {
        if next.is_alphanumeric() && !next.is_uppercase() {
            return false;
        }
        if ch == '.' && next.is_ascii_digit() {
            let previous = text[..byte].chars().next_back();
            if previous.is_some_and(|c| c.is_ascii_digit()) {
                return false;
            }
        }
    }

    if ch != '.' {
        return true;
    }

    let previous = text[..byte].split_whitespace().next_back().unwrap_or("");
    let token = previous
        .trim_matches(|c: char| !c.is_alphabetic() && c != '.')
        .trim_end_matches('.')
        .to_lowercase();
    const ABBREVIATIONS: &[&str] = &[
        "sr", "sra", "srta", "dr", "dra", "lic", "ud", "uds", "etc", "ej", "p", "n", "vol", "cap",
        "fig", "núm", "num", "no", "mr", "mrs", "ms", "st", "av", "aprox", "pág", "pag", "ee",
        "uu", "s", "a", "c", "d",
    ];

    !ABBREVIATIONS.contains(&token.as_str())
}

pub fn detect_characters(chunks: &[SpeechChunk]) -> Vec<CharacterInfo> {
    let mut counts: HashMap<String, (usize, String)> = HashMap::new();
    for chunk in chunks {
        let name = chunk
            .character
            .clone()
            .unwrap_or_else(|| NARRATOR.to_string());
        let entry = counts.entry(name).or_insert_with(|| (0, String::new()));
        entry.0 += 1;
        if entry.1.is_empty() {
            entry.1 = chunk.text.chars().take(120).collect();
        }
    }

    let mut characters: Vec<CharacterInfo> = counts
        .into_iter()
        .map(|(name, (lines, sample))| CharacterInfo {
            name,
            lines,
            sample,
        })
        .collect();

    characters.sort_by(|a, b| {
        let a_reserved = a.name == NARRATOR || a.name == DIALOGUE;
        let b_reserved = b.name == NARRATOR || b.name == DIALOGUE;
        b_reserved
            .cmp(&a_reserved)
            .then_with(|| b.lines.cmp(&a.lines))
            .then_with(|| a.name.cmp(&b.name))
    });
    characters
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::extract_text;

    fn build(html: &str) -> SpeechSequence {
        let source = extract_text(html);
        build_from_text(0, &source, &ExpressiveSettings::default())
    }

    #[test]
    fn sanitize_removes_symbol_noise() {
        assert_eq!(sanitize_speech("Hola?? ____ $%^ mundo"), "Hola? mundo");
        assert_eq!(sanitize_speech("?? ____ $%^"), "");
    }

    #[test]
    fn sanitize_keeps_accents_and_prosody() {
        assert_eq!(sanitize_speech("¿Qué tal? ¡Bien!"), "Qué tal? Bien!");
        assert_eq!(sanitize_speech("—Hola —dijo."), "Hola dijo.");
    }

    #[test]
    fn sanitize_splits_words_joined_by_symbols() {
        assert_eq!(sanitize_speech("foo_bar/baz"), "foo bar baz");
        assert_eq!(sanitize_speech("COVID-19 y 50%"), "COVID 19 y 50");
    }

    #[test]
    fn chunks_use_sanitized_speech_but_visible_text() {
        let sequence = build("<p>Hola ____ $%^ mundo.</p>");
        assert_eq!(sequence.chunks.len(), 1);
        assert!(sequence.chunks[0].text.contains("____"));
        assert_eq!(sequence.chunks[0].speech, "Hola mundo.");
    }

    #[test]
    fn splits_sentences_and_keeps_offsets() {
        let source = extract_text("<p>Hola. Mundo.</p>");
        let sequence = build_from_text(0, &source, &ExpressiveSettings::default());
        assert_eq!(sequence.chunks.len(), 2);
        assert_eq!(sequence.chunks[0].text, "Hola.");
        assert_eq!(sequence.chunks[1].text, "Mundo.");
        assert_eq!(sequence.unit, OffsetUnit::Utf16);
        assert_eq!(sequence.chunks[0].offset_start, 0);
        assert_eq!(sequence.chunks[1].offset_start, 6);
    }

    #[test]
    fn does_not_split_abbreviations() {
        let sequence = build("<p>El Sr. García llegó. Después se fue.</p>");
        assert_eq!(sequence.chunks.len(), 2);
        assert!(sequence.chunks[0].text.contains("Sr. García"));
    }

    #[test]
    fn detects_dialogue_and_speaker() {
        let sequence = build("<p>—Hola —dijo Juan.</p><p>«Adiós» dijo Ana.</p>");
        assert_eq!(sequence.chunks[0].character.as_deref(), Some("Juan"));
        assert_eq!(sequence.chunks[1].character.as_deref(), Some("Ana"));
        assert!(sequence.chunks[0].pause_after_ms >= 200);
    }

    #[test]
    fn generic_dialogue_gets_its_own_character() {
        let sequence = build("<p>—¿Hay alguien?</p>");
        assert_eq!(sequence.chunks[0].character.as_deref(), Some(DIALOGUE));
    }

    #[test]
    fn paragraphs_add_a_pause() {
        let sequence = build("<p>Uno.</p><p>Dos.</p>");
        assert!(sequence.chunks[0].pause_after_ms >= 300);
    }

    #[test]
    fn word_chunks_cover_all_words() {
        let words: Vec<TextWord> = ["Hola.", "Mundo.", "Otra", "frase."]
            .iter()
            .map(|text| TextWord {
                text: text.to_string(),
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
                font_size: 12.0,
                confidence: None,
            })
            .collect();
        let sequence = build_from_words(3, &words, &ExpressiveSettings::default());
        assert_eq!(sequence.unit, OffsetUnit::Word);
        assert_eq!(sequence.chunks[0].offset_start, 0);
        assert_eq!(sequence.chunks[0].offset_end, 1);
        assert_eq!(sequence.chunks[1].offset_end, 2);
        assert_eq!(sequence.chunks[2].offset_end, 4);
    }

    #[test]
    fn cache_hash_changes_with_settings() {
        let a = content_hash("hola", &ExpressiveSettings::default());
        let b = content_hash(
            "hola",
            &ExpressiveSettings {
                enabled: false,
                ..ExpressiveSettings::default()
            },
        );
        assert_ne!(a, b);
    }
}
