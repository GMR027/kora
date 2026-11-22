use std::collections::VecDeque;

use super::manifest::AudiobookManifest;

/// Resultado de reconocer el comando de voz dentro de una transcripción.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceCommandMatch {
    /// Texto dicho después del comando; vacío si solo se pronunció la frase.
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WordToken {
    /// Índice de byte donde empieza la palabra en el texto original.
    start: usize,
    /// Índice de byte donde termina (exclusivo).
    end: usize,
    normalized: String,
}

/// Ancla de un marcador dentro de un audiolibro. `offset_start` y `offset_end`
/// apuntan al texto del libro en unidades UTF-16, como el resto del lector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceMarkerAnchor {
    pub chapter_index: usize,
    pub section: usize,
    pub chapter_title: String,
    pub position_ms: i64,
    pub quote: String,
    pub offset_start: i64,
    pub offset_end: i64,
}

/// Normaliza una palabra para comparar transcripciones: minúsculas, sin
/// acentos, sin signos y con las confusiones habituales de Whisper (k/c).
fn normalize_word(word: &str) -> String {
    let mut normalized = String::with_capacity(word.len());
    for character in word.chars() {
        let folded = match character {
            'á' | 'à' | 'ä' | 'â' | 'ã' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            other => other,
        };
        if folded.is_alphanumeric() {
            // Whisper alterna «Kora» y «Cora» según el idioma del modelo.
            normalized.push(if folded == 'k' { 'c' } else { folded });
        }
    }
    normalized.to_lowercase()
}

fn tokenize(text: &str) -> Vec<WordToken> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;

    for (index, character) in text.char_indices() {
        if character.is_alphanumeric() {
            if start.is_none() {
                start = Some(index);
            }
        } else if let Some(begin) = start.take() {
            let word = &text[begin..index];
            let normalized = normalize_word(word);
            if !normalized.is_empty() {
                tokens.push(WordToken {
                    start: begin,
                    end: index,
                    normalized,
                });
            }
        }
    }

    if let Some(begin) = start {
        let word = &text[begin..];
        let normalized = normalize_word(word);
        if !normalized.is_empty() {
            tokens.push(WordToken {
                start: begin,
                end: text.len(),
                normalized,
            });
        }
    }

    tokens
}

fn edit_distance(left: &str, right: &str) -> usize {
    if left == right {
        return 0;
    }
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    if left.is_empty() {
        return right.len();
    }
    if right.is_empty() {
        return left.len();
    }

    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0usize; right.len() + 1];
    for (row, left_char) in left.iter().enumerate() {
        current[0] = row + 1;
        for (column, right_char) in right.iter().enumerate() {
            let cost = usize::from(left_char != right_char);
            current[column + 1] = (current[column] + 1)
                .min(previous[column + 1] + 1)
                .min(previous[column] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

fn tolerance(word: &str) -> usize {
    if word.chars().count() >= 4 { 1 } else { 0 }
}

/// Busca la frase de activación dentro de una transcripción. Admite hasta dos
/// palabras ajenas intercaladas («Kora, por favor, marca esto») y errores de
/// una letra en palabras de cuatro letras o más. Devuelve lo dicho después del
/// comando para usarlo como comentario del marcador.
pub fn match_voice_command(transcript: &str, phrase: &str) -> Option<VoiceCommandMatch> {
    let phrase_tokens = tokenize(phrase);
    if phrase_tokens.len() < 2 {
        return None;
    }
    let transcript_tokens = tokenize(transcript);
    if transcript_tokens.len() < phrase_tokens.len() {
        return None;
    }

    const MAX_SKIPS: usize = 2;
    let mut best: Option<(usize, usize, usize)> = None; // (coste, inicio, fin)

    for start in 0..transcript_tokens.len() {
        let mut cursor = start;
        let mut cost = 0usize;
        let mut skipped = 0usize;
        let mut matched_end = None;
        let mut ok = true;

        for expected in &phrase_tokens {
            let mut found = false;
            while cursor < transcript_tokens.len() {
                let actual = &transcript_tokens[cursor];
                let distance = edit_distance(&expected.normalized, &actual.normalized);
                if distance <= tolerance(&expected.normalized) {
                    cost += distance;
                    cursor += 1;
                    matched_end = Some(cursor);
                    found = true;
                    break;
                }
                cursor += 1;
                skipped += 1;
                if skipped > MAX_SKIPS {
                    break;
                }
            }
            if !found {
                ok = false;
                break;
            }
        }

        let Some(end) = matched_end else {
            continue;
        };
        if !ok {
            continue;
        }
        let candidate = (cost + skipped, start, end);
        if best
            .as_ref()
            .map(|current| candidate.0 < current.0)
            .unwrap_or(true)
        {
            best = Some(candidate);
        }
    }

    let (_, _, end) = best?;
    let last = transcript_tokens.get(end - 1)?;
    let comment = transcript[last.end..]
        .trim_start_matches(|character: char| {
            character.is_whitespace()
                || matches!(
                    character,
                    ',' | '.'
                        | ';'
                        | ':'
                        | '—'
                        | '–'
                        | '-'
                        | '!'
                        | '?'
                        | '¿'
                        | '¡'
                        | '"'
                        | '«'
                        | '»'
                        | '“'
                        | '”'
                )
        })
        .trim()
        .to_string();

    Some(VoiceCommandMatch { comment })
}

/// Devuelve el ancla de un marcador a partir de la posición de reproducción.
/// Si el capítulo tiene karaoke, la cita es la frase que está sonando.
pub fn marker_anchor(manifest: &AudiobookManifest, position_ms: i64) -> Option<VoiceMarkerAnchor> {
    let chapter_index = manifest.chapter_at(position_ms);
    let chapter = manifest.chapter(chapter_index)?;
    let relative = position_ms.saturating_sub(chapter.start_ms).max(0);
    let line = chapter
        .karaoke
        .iter()
        .find(|line| relative >= line.start_ms && relative < line.end_ms)
        .or_else(|| {
            chapter
                .karaoke
                .iter()
                .rev()
                .find(|line| line.start_ms <= relative)
        })
        .or_else(|| chapter.karaoke.first());
    let quote = line
        .map(|line| line.text.trim())
        .filter(|text| !text.is_empty())
        .map(|text| text.chars().take(280).collect())
        .unwrap_or_default();

    Some(VoiceMarkerAnchor {
        chapter_index,
        section: chapter.section,
        chapter_title: chapter.title.clone(),
        position_ms: position_ms.max(0),
        quote,
        offset_start: chapter.offset_start as i64,
        offset_end: chapter.offset_end as i64,
    })
}

/// Localiza la cita dentro del texto completo del capítulo y devuelve sus
/// offsets UTF-16. Si no aparece, el llamador conserva los del capítulo.
pub fn quote_offsets(section_text: &str, quote: &str) -> Option<(i64, i64)> {
    let quote = quote.trim();
    if quote.is_empty() {
        return None;
    }
    let byte_start = section_text.find(quote)?;
    let start = section_text[..byte_start].encode_utf16().count() as i64;
    let end = start + quote.encode_utf16().count() as i64;
    Some((start, end))
}

/// Configuración del detector de locuciones. Los valores están pensados para
/// micrófono de escritorio: 20 ms por trama, 350 ms mínimos de voz y 650 ms de
/// silencio para cerrar una frase.
#[derive(Debug, Clone, Copy)]
pub struct SegmenterConfig {
    pub sample_rate: u32,
    pub frame_ms: u32,
    pub min_speech_ms: u32,
    pub end_silence_ms: u32,
    pub pre_roll_ms: u32,
    pub max_utterance_ms: u32,
}

impl Default for SegmenterConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16_000,
            frame_ms: 20,
            min_speech_ms: 350,
            end_silence_ms: 650,
            pre_roll_ms: 320,
            max_utterance_ms: 15_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmenterState {
    Idle,
    Speech,
}

/// Detecta locuciones por energía. Mantiene un pre-roll para no perder la
/// primera sílaba y cierra la frase tras un silencio sostenido. El ruido de
/// fondo se adapta despacio; con el audiolibro por altavoces conviene usar
/// auriculares para que el micrófono no reciba la narración.
pub struct UtteranceSegmenter {
    config: SegmenterConfig,
    frame_samples: usize,
    pending: Vec<f32>,
    pre_roll: VecDeque<f32>,
    pre_roll_len: usize,
    noise_floor: f32,
    state: SegmenterState,
    utterance: Vec<f32>,
    voiced_run: u32,
    voiced_frames: u32,
    silence_frames: u32,
}

impl UtteranceSegmenter {
    pub fn new(config: SegmenterConfig) -> Self {
        let frame_samples =
            (config.sample_rate as usize * config.frame_ms.max(1) as usize / 1000).max(1);
        let pre_roll_len =
            (config.sample_rate as usize * config.pre_roll_ms as usize / 1000).max(frame_samples);
        Self {
            config,
            frame_samples,
            pending: Vec::new(),
            pre_roll: VecDeque::with_capacity(pre_roll_len + frame_samples),
            pre_roll_len,
            noise_floor: 0.004,
            state: SegmenterState::Idle,
            utterance: Vec::new(),
            voiced_run: 0,
            voiced_frames: 0,
            silence_frames: 0,
        }
    }

    /// Añade muestras mono y devuelve las locuciones completas.
    pub fn push(&mut self, samples: &[f32]) -> Vec<Vec<f32>> {
        let mut output = Vec::new();
        self.pending.extend_from_slice(samples);
        while self.pending.len() >= self.frame_samples {
            let frame: Vec<f32> = self.pending.drain(..self.frame_samples).collect();
            self.process_frame(&frame, &mut output);
        }
        output
    }

    fn process_frame(&mut self, frame: &[f32], output: &mut Vec<Vec<f32>>) {
        let rms = (frame.iter().map(|sample| sample * sample).sum::<f32>()
            / frame.len().max(1) as f32)
            .sqrt();
        let start_threshold = (self.noise_floor * 2.0).clamp(0.012, 0.08);
        let end_threshold = start_threshold * 0.7;

        match self.state {
            SegmenterState::Idle => {
                if rms < start_threshold {
                    self.noise_floor = (self.noise_floor * 0.95 + rms * 0.05).max(0.0008);
                }

                // El pre-roll incluye la trama actual; así la primera sílaba
                // queda dentro de la locución cuando se confirma la voz.
                self.push_pre_roll(frame);

                if rms > start_threshold {
                    self.voiced_run += 1;
                    if self.voiced_run >= 3 {
                        self.state = SegmenterState::Speech;
                        self.utterance = self.pre_roll.iter().copied().collect();
                        self.voiced_frames = self.voiced_run;
                        self.silence_frames = 0;
                        self.voiced_run = 0;
                    }
                } else {
                    self.voiced_run = 0;
                }
            }
            SegmenterState::Speech => {
                self.utterance.extend_from_slice(frame);
                if rms > end_threshold {
                    self.voiced_frames += 1;
                    self.silence_frames = 0;
                } else {
                    self.silence_frames += 1;
                }

                let utterance_ms =
                    self.utterance.len() as u64 * 1000 / self.config.sample_rate.max(1) as u64;
                if utterance_ms >= self.config.max_utterance_ms as u64
                    || self.silence_frames * self.config.frame_ms >= self.config.end_silence_ms
                {
                    self.finish(output);
                }
            }
        }
    }

    fn push_pre_roll(&mut self, frame: &[f32]) {
        self.pre_roll.extend(frame.iter().copied());
        while self.pre_roll.len() > self.pre_roll_len {
            self.pre_roll.pop_front();
        }
    }

    fn finish(&mut self, output: &mut Vec<Vec<f32>>) {
        let speech_ms = self.voiced_frames as u64 * self.config.frame_ms as u64;
        let mut utterance = std::mem::take(&mut self.utterance);
        // Recorta el silencio final conservando un margen de 220 ms.
        let keep_silence = (self.config.sample_rate as usize * 220 / 1000).min(utterance.len());
        let trailing = self.silence_frames as usize * self.frame_samples;
        if trailing > keep_silence {
            let keep = utterance.len().saturating_sub(trailing - keep_silence);
            utterance.truncate(keep);
        }
        if speech_ms >= self.config.min_speech_ms as u64 && !utterance.is_empty() {
            output.push(utterance);
        }
        self.state = SegmenterState::Idle;
        self.pre_roll.clear();
        self.voiced_run = 0;
        self.voiced_frames = 0;
        self.silence_frames = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::super::karaoke::KaraokeLine;
    use super::super::manifest::{
        AudiobookManifest, MANIFEST_VERSION, ManifestChapter, ManifestVoice,
    };
    use super::super::scope::AudiobookScope;
    use super::*;

    fn tone(rate: u32, milliseconds: u32, amplitude: f32) -> Vec<f32> {
        let count = rate as usize * milliseconds as usize / 1000;
        (0..count)
            .map(|index| {
                let phase = index as f32 * 220.0 * std::f32::consts::TAU / rate as f32;
                phase.sin() * amplitude
            })
            .collect()
    }

    #[test]
    fn matches_the_default_command_with_accents_and_punctuation() {
        let matched =
            match_voice_command("Kóra, marcá esto.", "Kora, marca esto").expect("comando");
        assert_eq!(matched.comment, "");
    }

    #[test]
    fn accepts_the_whisper_confusion_between_k_and_c() {
        let matched =
            match_voice_command("Oye, Cora, marca esto", "Kora, marca esto").expect("comando");
        assert_eq!(matched.comment, "");
    }

    #[test]
    fn keeps_the_comment_after_the_command() {
        let matched = match_voice_command(
            "Kora, marca esto: esto es clave para el final.",
            "Kora, marca esto",
        )
        .expect("comando");
        assert_eq!(matched.comment, "esto es clave para el final.");
    }

    #[test]
    fn allows_a_couple_of_filler_words() {
        let matched = match_voice_command("Kora, por favor, marca esto ahora", "Kora, marca esto")
            .expect("comando");
        assert_eq!(matched.comment, "ahora");
    }

    #[test]
    fn ignores_plain_narration_without_the_wake_word() {
        assert!(match_voice_command("El narrador dijo marca esto", "Kora, marca esto").is_none());
        assert!(match_voice_command("Kora, marca", "Kora, marca esto").is_none());
    }

    #[test]
    fn supports_custom_phrases() {
        let matched = match_voice_command("Lumina, guarda cita: el río.", "Lumina, guarda cita")
            .expect("comando");
        assert_eq!(matched.comment, "el río.");
    }

    #[test]
    fn marker_anchor_uses_the_sentence_that_is_playing() {
        let manifest = AudiobookManifest {
            version: MANIFEST_VERSION,
            book_id: "book".into(),
            book_title: "Libro".into(),
            language: None,
            voice: ManifestVoice {
                id: "v".into(),
                name: "Voz".into(),
                cloned: false,
            },
            watermark: false,
            scope: AudiobookScope::FullBook,
            duration_ms: 10_000,
            narration: None,
            cast: Vec::new(),
            chapter_source: None,
            treatment: None,
            chapters: vec![ManifestChapter {
                index: 0,
                title: "Uno".into(),
                start_ms: 1_000,
                end_ms: 10_000,
                section: 3,
                offset_start: 10,
                offset_end: 90,
                karaoke: vec![
                    KaraokeLine::timed("Primera frase.", 1_000, 3_000).expect("línea"),
                    KaraokeLine::timed("Segunda frase.", 3_000, 6_000).expect("línea"),
                ],
            }],
        };

        let anchor = marker_anchor(&manifest, 4_200).expect("ancla");
        assert_eq!(anchor.chapter_index, 0);
        assert_eq!(anchor.section, 3);
        assert_eq!(anchor.position_ms, 4_200);
        assert_eq!(anchor.quote, "Segunda frase.");
        assert_eq!(anchor.offset_start, 10);
        assert_eq!(anchor.offset_end, 90);
    }

    #[test]
    fn quote_offsets_are_utf16_indices() {
        let text = "Prefacio 😀. La cita empieza aquí.";
        let (start, end) = quote_offsets(text, "La cita empieza aquí.").expect("cita");
        assert_eq!(start, 13);
        assert_eq!(
            end,
            start + "La cita empieza aquí.".encode_utf16().count() as i64
        );
    }

    #[test]
    fn finds_an_utterance_between_silences() {
        let mut segmenter = UtteranceSegmenter::new(SegmenterConfig::default());
        let mut output = segmenter.push(&tone(16_000, 400, 0.0001));
        output.extend(segmenter.push(&tone(16_000, 700, 0.18)));
        output.extend(segmenter.push(&tone(16_000, 800, 0.0001)));
        assert_eq!(output.len(), 1);
        assert!(output[0].len() >= 16_000 * 600 / 1000);
    }

    #[test]
    fn ignores_continuous_silence_and_short_clicks() {
        let mut segmenter = UtteranceSegmenter::new(SegmenterConfig::default());
        assert!(segmenter.push(&tone(16_000, 2_000, 0.0001)).is_empty());
        assert!(segmenter.push(&tone(16_000, 120, 0.2)).is_empty());
    }

    #[test]
    fn cuts_very_long_utterances() {
        let config = SegmenterConfig {
            max_utterance_ms: 900,
            ..SegmenterConfig::default()
        };
        let mut segmenter = UtteranceSegmenter::new(config);
        let output = segmenter.push(&tone(16_000, 2_400, 0.2));
        assert!(output.len() >= 2);
    }
}
