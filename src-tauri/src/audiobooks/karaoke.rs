use serde::{Deserialize, Serialize};

/// Tiempo de una palabra dentro de un capítulo. Los milisegundos son
/// relativos al inicio del capítulo para que el reproductor no dependa de
/// offsets globales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KaraokeWord {
    pub start_ms: i64,
    pub end_ms: i64,
}

/// Una línea legible del audiolibro con su tramo de audio. `words` solo se
/// rellena cuando hay una alineación real (Whisper); en el resto de casos la
/// interfaz reparte la duración de la línea entre sus palabras.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KaraokeLine {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<KaraokeWord>,
}

impl KaraokeLine {
    /// Línea de una síntesis: el tramo de audio es exacto y la interfaz
    /// estima las palabras proporcionalmente.
    pub fn timed(text: &str, start_ms: i64, end_ms: i64) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() || end_ms <= start_ms {
            return None;
        }
        Some(Self {
            start_ms,
            end_ms,
            text: text.to_string(),
            words: Vec::new(),
        })
    }

    /// Línea alineada por Whisper. Se descartan palabras sin tiempo válido.
    pub fn aligned(
        text: &str,
        start_ms: i64,
        end_ms: i64,
        words: Vec<KaraokeWord>,
    ) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() || end_ms <= start_ms {
            return None;
        }
        let words = words
            .into_iter()
            .filter(|word| word.end_ms > word.start_ms)
            .collect();
        Some(Self {
            start_ms,
            end_ms,
            text: text.to_string(),
            words,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KaraokePrecision {
    /// Palabras alineadas por reconocimiento (Whisper).
    Aligned,
    /// Límites de frase exactos; palabras repartidas por la interfaz.
    Timed,
    /// Capítulo repartido por longitud porque no hay datos sincronizados.
    Estimated,
}

/// Datos que consume el reproductor para una línea de tiempo concreta.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterKaraoke {
    pub chapter_index: usize,
    pub title: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub precision: KaraokePrecision,
    pub lines: Vec<KaraokeLine>,
}

/// Peso orientativo de una palabra para repartir la duración de una línea.
/// Incluye una pausa breve en comas y una más larga al cerrar la frase.
pub fn word_weight(text: &str) -> u64 {
    let mut weight = 0u64;
    let mut punctuation = 0u64;
    for character in text.chars() {
        if character.is_alphanumeric() {
            weight += 1;
        } else if matches!(character, ',' | ';' | ':' | '—' | '–' | '-') {
            punctuation += 2;
        } else if matches!(character, '.' | '!' | '?' | '…') {
            punctuation += 4;
        }
    }
    (weight + punctuation).max(1)
}

pub fn line_weight(text: &str) -> u64 {
    text.split_whitespace().map(word_weight).sum::<u64>().max(1)
}

/// Reparte un capítulo sin datos de síntesis entre frases proporcionales a su
/// longitud. Es la red de seguridad para audiolibros creados antes del modo
/// karaoke; los nuevos guardan sus propios límites de frase.
pub fn lines_from_text(text: &str, start_ms: i64, end_ms: i64) -> Vec<KaraokeLine> {
    if end_ms <= start_ms {
        return Vec::new();
    }
    let pieces = split_lines(text);
    if pieces.is_empty() {
        return Vec::new();
    }
    let weights: Vec<u64> = pieces.iter().map(|piece| line_weight(piece)).collect();
    let total: u64 = weights.iter().sum::<u64>().max(1);
    let span = end_ms - start_ms;
    let mut lines = Vec::with_capacity(pieces.len());
    let mut accumulated = 0u64;

    for (index, piece) in pieces.iter().enumerate() {
        accumulated += weights[index];
        let line_start = start_ms
            + span.saturating_mul(accumulated.saturating_sub(weights[index]) as i64) / total as i64;
        let line_end = if index + 1 == pieces.len() {
            end_ms
        } else {
            start_ms + span.saturating_mul(accumulated as i64) / total as i64
        };
        if line_end <= line_start {
            continue;
        }
        lines.push(KaraokeLine {
            start_ms: line_start,
            end_ms: line_end,
            text: piece.to_string(),
            words: Vec::new(),
        });
    }

    if let Some(last) = lines.last_mut() {
        last.end_ms = end_ms;
    }
    lines
}

/// Divide en frases y párrafos, con un límite de longitud para que una línea
/// nunca desborde el reproductor.
fn split_lines(text: &str) -> Vec<String> {
    const MAX_CHARS: usize = 240;
    let mut lines = Vec::new();
    let chars: Vec<char> = text
        .chars()
        .filter(|character| *character != '\r')
        .collect();
    let mut start = 0usize;
    let mut index = 0usize;

    while index < chars.len() {
        if chars[index] == '\n' {
            push_trimmed(&chars[start..index], &mut lines);
            index += 1;
            start = index;
            continue;
        }

        if matches!(chars[index], '.' | '!' | '?' | '…') {
            let mut end = index + 1;
            while end < chars.len() && is_closing(chars[end]) {
                end += 1;
            }
            if end == chars.len() || chars[end].is_whitespace() {
                push_trimmed(&chars[start..end], &mut lines);
                index = end;
                start = end;
                continue;
            }
        }
        index += 1;
    }
    push_trimmed(&chars[start..], &mut lines);

    // Un párrafo sin puntuación puede superar el límite: se parte por espacios.
    let mut bounded = Vec::with_capacity(lines.len());
    for line in lines {
        if line.chars().count() <= MAX_CHARS {
            bounded.push(line);
            continue;
        }
        let mut buffer = String::new();
        for word in line.split_whitespace() {
            if !buffer.is_empty() && buffer.chars().count() + word.chars().count() + 1 > MAX_CHARS {
                bounded.push(std::mem::take(&mut buffer));
            }
            if !buffer.is_empty() {
                buffer.push(' ');
            }
            buffer.push_str(word);
        }
        if !buffer.is_empty() {
            bounded.push(buffer);
        }
    }
    bounded
}

fn push_trimmed(chars: &[char], lines: &mut Vec<String>) {
    let text: String = chars.iter().collect();
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        lines.push(trimmed.to_string());
    }
}

fn is_closing(character: char) -> bool {
    matches!(
        character,
        '"' | '\'' | '»' | '”' | '’' | ')' | ']' | '}' | '*'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sentences_and_covers_the_whole_span() {
        let lines = lines_from_text(
            "Primera frase. Segunda frase más larga, con calma.\n\nTercera.",
            1_000,
            10_000,
        );
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].start_ms, 1_000);
        assert_eq!(lines.last().expect("última").end_ms, 10_000);
        assert!(lines[1].end_ms > lines[1].start_ms);
    }

    #[test]
    fn longer_sentences_receive_more_time() {
        let lines = lines_from_text(
            "Corta. Una frase bastante más larga que la anterior.",
            0,
            9_000,
        );
        assert_eq!(lines.len(), 2);
        let first = lines[0].end_ms - lines[0].start_ms;
        let second = lines[1].end_ms - lines[1].start_ms;
        assert!(second > first, "{second} debería superar a {first}");
    }

    #[test]
    fn caps_very_long_paragraphs() {
        let text = "palabra ".repeat(200);
        let lines = lines_from_text(&text, 0, 60_000);
        assert!(!lines.is_empty());
        assert!(lines.iter().all(|line| line.text.chars().count() <= 240));
    }

    #[test]
    fn aligned_lines_drop_words_without_a_valid_span() {
        let line = KaraokeLine::aligned(
            "Hola mundo",
            0,
            1_000,
            vec![
                KaraokeWord {
                    start_ms: 0,
                    end_ms: 400,
                },
                KaraokeWord {
                    start_ms: 500,
                    end_ms: 500,
                },
            ],
        )
        .expect("línea");
        assert_eq!(line.words.len(), 1);
        assert_eq!(line.words[0].end_ms, 400);
    }

    #[test]
    fn timed_lines_ignore_empty_text() {
        assert!(KaraokeLine::timed("   ", 0, 100).is_none());
        assert!(KaraokeLine::timed("Hola", 100, 100).is_none());
        let line = KaraokeLine::timed(" Hola ", 0, 500).expect("línea");
        assert_eq!(line.text, "Hola");
    }

    #[test]
    fn punctuation_adds_weight_without_losing_the_minimum() {
        assert!(word_weight("hola,") > word_weight("hola"));
        assert!(word_weight(".") >= 1);
        assert!(word_weight("fin.") > word_weight("fin"));
    }
}
