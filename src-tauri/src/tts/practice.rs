use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[cfg(not(target_os = "android"))]
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
#[cfg(not(target_os = "android"))]
use std::sync::{Arc, Mutex};
use unicode_normalization::UnicodeNormalization;

use crate::error::{KoraError, Result};

#[derive(Debug, Clone, Copy)]
pub struct WhisperSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    pub size_bytes: u64,
}

pub const WHISPER_CATALOG: &[WhisperSpec] = &[
    WhisperSpec {
        id: "tiny",
        name: "Whisper Tiny (rápido)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
        size_bytes: 77_691_713,
    },
    WhisperSpec {
        id: "base",
        name: "Whisper Base (preciso)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
        size_bytes: 147_951_465,
    },
];

pub fn find_model(id: &str) -> Option<&'static WhisperSpec> {
    WHISPER_CATALOG.iter().find(|model| model.id == id)
}

pub fn whisper_dir(root: &Path) -> PathBuf {
    root.join("whisper")
}

pub fn model_path(root: &Path, id: &str) -> PathBuf {
    whisper_dir(root).join(format!("ggml-{id}.bin"))
}

pub fn installed(root: &Path, id: &str) -> bool {
    model_path(root, id)
        .metadata()
        .map(|meta| meta.len() > 10 * 1024 * 1024)
        .unwrap_or(false)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Match,
    Substitution,
    Missing,
    Extra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordDiff {
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub kind: DiffKind,
    pub expected_index: Option<usize>,
    pub actual_index: Option<usize>,
}

/// Compara la lectura con el texto original usando alineamiento de
/// Levenshtein sobre palabras normalizadas (minúsculas y sin acentos).
pub fn diff_words(expected: &str, actual: &str) -> (Vec<WordDiff>, f64) {
    let expected_tokens = tokenize(expected);
    let actual_tokens = tokenize(actual);

    if expected_tokens.is_empty() {
        return (Vec::new(), if actual_tokens.is_empty() { 1.0 } else { 0.0 });
    }

    let n = expected_tokens.len();
    let m = actual_tokens.len();
    let mut costs = vec![vec![0u32; m + 1]; n + 1];

    for (i, row) in costs.iter_mut().enumerate() {
        row[0] = i as u32;
    }
    for (j, cell) in costs[0].iter_mut().enumerate() {
        *cell = j as u32;
    }

    for i in 1..=n {
        for j in 1..=m {
            let substitution = if expected_tokens[i - 1].1 == actual_tokens[j - 1].1 {
                0
            } else {
                1
            };
            costs[i][j] = (costs[i - 1][j - 1] + substitution)
                .min(costs[i - 1][j] + 1)
                .min(costs[i][j - 1] + 1);
        }
    }

    let mut diff = Vec::new();
    let mut i = n;
    let mut j = m;

    while i > 0 || j > 0 {
        if i > 0
            && j > 0
            && expected_tokens[i - 1].1 == actual_tokens[j - 1].1
            && costs[i][j] == costs[i - 1][j - 1]
        {
            diff.push(WordDiff {
                expected: Some(expected_tokens[i - 1].0.clone()),
                actual: Some(actual_tokens[j - 1].0.clone()),
                kind: DiffKind::Match,
                expected_index: Some(i - 1),
                actual_index: Some(j - 1),
            });
            i -= 1;
            j -= 1;
        } else if i > 0 && j > 0 && costs[i][j] == costs[i - 1][j - 1] + 1 {
            diff.push(WordDiff {
                expected: Some(expected_tokens[i - 1].0.clone()),
                actual: Some(actual_tokens[j - 1].0.clone()),
                kind: DiffKind::Substitution,
                expected_index: Some(i - 1),
                actual_index: Some(j - 1),
            });
            i -= 1;
            j -= 1;
        } else if i > 0 && costs[i][j] == costs[i - 1][j] + 1 {
            diff.push(WordDiff {
                expected: Some(expected_tokens[i - 1].0.clone()),
                actual: None,
                kind: DiffKind::Missing,
                expected_index: Some(i - 1),
                actual_index: None,
            });
            i -= 1;
        } else {
            diff.push(WordDiff {
                expected: None,
                actual: Some(actual_tokens[j - 1].0.clone()),
                kind: DiffKind::Extra,
                expected_index: None,
                actual_index: Some(j - 1),
            });
            j -= 1;
        }
    }

    diff.reverse();
    let matches = diff
        .iter()
        .filter(|entry| matches!(entry.kind, DiffKind::Match))
        .count();
    let accuracy = (matches as f64 / n as f64).clamp(0.0, 1.0);
    (diff, accuracy)
}

fn tokenize(text: &str) -> Vec<(String, String)> {
    text.split_whitespace()
        .filter_map(|word| {
            let original = word
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-')
                .to_string();
            let normalized = normalize_word(&original);
            (!normalized.is_empty()).then_some((original, normalized))
        })
        .collect()
}

fn normalize_word(word: &str) -> String {
    word.nfkd()
        .filter(|ch| !unicode_normalization::char::is_combining_mark(*ch))
        .flat_map(char::to_lowercase)
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

pub struct PracticeInput {
    pub book_id: String,
    pub section: usize,
    pub offset_start: i64,
    pub offset_end: i64,
    pub expected: String,
    pub language: String,
}

pub struct StoppedPractice {
    pub book_id: String,
    pub section: usize,
    pub offset_start: i64,
    pub offset_end: i64,
    pub expected: String,
    pub language: String,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration_ms: u64,
    pub wav_path: PathBuf,
}

#[derive(Default)]
pub struct PracticeManager {
    #[cfg(not(target_os = "android"))]
    inner: Mutex<Option<ActivePractice>>,
}

#[cfg(not(target_os = "android"))]
struct ActivePractice {
    input: PracticeInput,
    recorder: Recorder,
}

impl PracticeManager {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(not(target_os = "android"))]
    pub fn start(&self, input: PracticeInput, output_dir: PathBuf) -> Result<()> {
        let mut slot = self
            .inner
            .lock()
            .map_err(|_| KoraError::Internal("grabadora envenenada".into()))?;
        if slot.is_some() {
            return Err(KoraError::Conflict(
                "ya hay una grabación de ensayo en curso".into(),
            ));
        }

        let id = uuid::Uuid::new_v4().to_string();
        let recorder = Recorder::start(output_dir, id)?;
        *slot = Some(ActivePractice { input, recorder });
        Ok(())
    }

    #[cfg(target_os = "android")]
    pub fn start(&self, _input: PracticeInput, _output_dir: PathBuf) -> Result<()> {
        Err(KoraError::Unsupported(
            "el modo ensayo con micrófono aún no está disponible en Android".into(),
        ))
    }

    #[cfg(not(target_os = "android"))]
    pub fn stop(&self) -> Result<StoppedPractice> {
        let active = self
            .inner
            .lock()
            .map_err(|_| KoraError::Internal("grabadora envenenada".into()))?
            .take()
            .ok_or_else(|| KoraError::InvalidInput("no hay una grabación en curso".into()))?;

        let recording = active.recorder.stop()?;
        Ok(StoppedPractice {
            book_id: active.input.book_id,
            section: active.input.section,
            offset_start: active.input.offset_start,
            offset_end: active.input.offset_end,
            expected: active.input.expected,
            language: active.input.language,
            samples: recording.samples,
            sample_rate: recording.sample_rate,
            duration_ms: recording.duration_ms,
            wav_path: recording.wav_path,
        })
    }

    #[cfg(target_os = "android")]
    pub fn stop(&self) -> Result<StoppedPractice> {
        Err(KoraError::Unsupported(
            "el modo ensayo con micrófono aún no está disponible en Android".into(),
        ))
    }

    #[cfg(not(target_os = "android"))]
    pub fn cancel(&self) -> Result<()> {
        let mut slot = self
            .inner
            .lock()
            .map_err(|_| KoraError::Internal("grabadora envenenada".into()))?;
        if let Some(active) = slot.take() {
            active.recorder.cancel();
        }
        Ok(())
    }

    #[cfg(target_os = "android")]
    pub fn cancel(&self) -> Result<()> {
        Ok(())
    }

    #[cfg(not(target_os = "android"))]
    pub fn is_recording(&self) -> bool {
        self.inner
            .lock()
            .map(|slot| slot.is_some())
            .unwrap_or(false)
    }

    #[cfg(target_os = "android")]
    pub fn is_recording(&self) -> bool {
        false
    }
}

#[cfg(not(target_os = "android"))]
struct Recorder {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<Result<crate::tts::recording::Recording>>>,
}

#[cfg(not(target_os = "android"))]
impl Recorder {
    fn start(directory: PathBuf, id: String) -> Result<Self> {
        std::fs::create_dir_all(&directory)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = stop.clone();
        let handle = std::thread::Builder::new()
            .name("kora-practice".into())
            .spawn(move || {
                let level = Arc::new(AtomicU32::new(0));
                crate::tts::recording::record_to_wav(
                    &directory,
                    &id,
                    stop_for_thread,
                    600,
                    None,
                    &level,
                    false,
                )
            })
            .map_err(|error| {
                KoraError::Internal(format!("no se pudo iniciar el micrófono: {error}"))
            })?;
        Ok(Self {
            stop,
            handle: Some(handle),
        })
    }

    fn stop(mut self) -> Result<crate::tts::recording::Recording> {
        self.stop.store(true, Ordering::SeqCst);
        self.handle
            .take()
            .ok_or_else(|| KoraError::Internal("grabadora sin hilo".into()))?
            .join()
            .map_err(|_| KoraError::Internal("la grabación terminó de forma inesperada".into()))?
    }

    fn cancel(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimedWord {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimedSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub words: Vec<TimedWord>,
}

/// Carga el modelo una sola vez para transcribir varios fragmentos seguidos.
/// En móvil esto evita releer cientos de megas entre capítulo y capítulo.
pub struct WhisperEngine {
    context: whisper_rs::WhisperContext,
}

impl WhisperEngine {
    pub fn load(model: &Path) -> Result<Self> {
        use whisper_rs::{WhisperContext, WhisperContextParameters};

        let path = model
            .to_str()
            .ok_or_else(|| KoraError::InvalidInput("ruta de modelo Whisper no válida".into()))?;
        let context = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .map_err(|error| KoraError::Internal(format!("no se pudo cargar Whisper: {error}")))?;
        Ok(Self { context })
    }

    fn params(language: &str, timed: bool) -> whisper_rs::FullParams<'_, '_> {
        use whisper_rs::{FullParams, SamplingStrategy};

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        let language = matches!(language, "es" | "en").then_some(language);
        params.set_language(language);
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_n_threads(
            std::thread::available_parallelism()
                .map(|value| value.get() as i32)
                .unwrap_or(4)
                .clamp(1, 8),
        );
        if timed {
            params.set_token_timestamps(true);
            params.set_split_on_word(true);
        }
        params
    }

    pub fn transcribe(&self, samples: &[f32], language: &str) -> Result<String> {
        let mut state = self
            .context
            .create_state()
            .map_err(|error| KoraError::Internal(format!("estado de Whisper: {error}")))?;
        state
            .full(Self::params(language, false), samples)
            .map_err(|error| {
                KoraError::Internal(format!("Whisper no pudo transcribir: {error}"))
            })?;

        let mut text = String::new();
        for index in 0..state.full_n_segments() {
            if let Some(segment) = state.get_segment(index) {
                text.push_str(
                    segment
                        .to_str_lossy()
                        .map_err(|error| KoraError::Internal(format!("texto de Whisper: {error}")))?
                        .as_ref(),
                );
            }
        }
        Ok(text.trim().to_string())
    }

    /// Transcribe con timestamps por token para alinear palabra a palabra.
    /// Los timestamps son experimentales en whisper.cpp: si el modelo no los
    /// devuelve con suficiente calidad, se omiten las palabras y la interfaz
    /// reparte la línea por longitud.
    pub fn transcribe_timed(&self, samples: &[f32], language: &str) -> Result<Vec<TimedSegment>> {
        self.transcribe_timed_with_progress(samples, language, |_| {})
    }

    /// Variante con callback de progreso (0–100) para tareas largas.
    pub fn transcribe_timed_with_progress<F>(
        &self,
        samples: &[f32],
        language: &str,
        progress: F,
    ) -> Result<Vec<TimedSegment>>
    where
        F: FnMut(i32) + 'static,
    {
        let mut state = self
            .context
            .create_state()
            .map_err(|error| KoraError::Internal(format!("estado de Whisper: {error}")))?;
        let mut params = Self::params(language, true);
        params.set_progress_callback_safe(progress);
        state.full(params, samples).map_err(|error| {
            KoraError::Internal(format!("Whisper no pudo transcribir: {error}"))
        })?;

        let mut output = Vec::new();
        for index in 0..state.full_n_segments() {
            let Some(segment) = state.get_segment(index) else {
                continue;
            };
            let text = segment
                .to_str_lossy()
                .map_err(|error| KoraError::Internal(format!("texto de Whisper: {error}")))?
                .trim()
                .to_string();
            if text.is_empty() {
                continue;
            }
            let start_ms = segment.start_timestamp().max(0) * 10;
            let end_ms = (segment.end_timestamp().max(0) * 10).max(start_ms + 1);
            let words = words_from_segment(&segment, start_ms, end_ms);
            output.push(TimedSegment {
                start_ms,
                end_ms,
                text,
                words,
            });
        }
        Ok(output)
    }
}

pub fn transcribe(model: &Path, samples: &[f32], language: &str) -> Result<String> {
    WhisperEngine::load(model)?.transcribe(samples, language)
}

pub fn transcribe_timed(
    model: &Path,
    samples: &[f32],
    language: &str,
) -> Result<Vec<TimedSegment>> {
    WhisperEngine::load(model)?.transcribe_timed(samples, language)
}

fn words_from_segment(
    segment: &whisper_rs::WhisperSegment<'_>,
    start_ms: i64,
    end_ms: i64,
) -> Vec<TimedWord> {
    struct Span {
        start: usize,
        end: usize,
        t0: i64,
        t1: i64,
    }

    let mut raw = String::new();
    let mut spans: Vec<Span> = Vec::new();
    let mut valid_tokens = 0usize;

    for index in 0..segment.n_tokens() {
        let Some(token) = segment.get_token(index) else {
            continue;
        };
        let Ok(token_text) = token.to_str_lossy() else {
            continue;
        };
        if token_text.starts_with("<|") || token_text.starts_with("[_") {
            continue;
        }
        let start = raw.len();
        raw.push_str(&token_text);
        let data = token.token_data();
        if data.t1 > data.t0 && data.t0 >= 0 {
            valid_tokens += 1;
        }
        spans.push(Span {
            start,
            end: raw.len(),
            t0: data.t0,
            t1: data.t1,
        });
    }

    if raw.trim().is_empty() || spans.is_empty() || valid_tokens * 2 < spans.len() {
        return Vec::new();
    }

    let chars: Vec<(usize, char)> = raw.char_indices().collect();
    let mut words: Vec<TimedWord> = Vec::new();
    let mut index = 0usize;

    while index < chars.len() {
        while index < chars.len() && chars[index].1.is_whitespace() {
            index += 1;
        }
        if index >= chars.len() {
            break;
        }
        let start = chars[index].0;
        while index < chars.len() && !chars[index].1.is_whitespace() {
            index += 1;
        }
        let end = chars.get(index).map(|(byte, _)| *byte).unwrap_or(raw.len());
        let text = raw[start..end].to_string();

        let mut t0 = i64::MAX;
        let mut t1 = i64::MIN;
        for span in &spans {
            if span.start < end && span.end > start && span.t1 > span.t0 && span.t0 >= 0 {
                t0 = t0.min(span.t0);
                t1 = t1.max(span.t1);
            }
        }

        let (word_start, word_end) = if t0 == i64::MAX {
            (0, 0)
        } else {
            (
                (t0 * 10).clamp(start_ms, end_ms),
                (t1 * 10).clamp(start_ms, end_ms),
            )
        };
        words.push(TimedWord {
            text,
            start_ms: word_start,
            end_ms: word_end,
        });
    }

    if words.is_empty() {
        return Vec::new();
    }

    // Rellena palabras sin tiempos válidos repartiendo el tramo del segmento.
    let total = words.len() as i64;
    let span = (end_ms - start_ms).max(1);
    for (index, word) in words.iter_mut().enumerate() {
        if word.end_ms <= word.start_ms {
            word.start_ms = start_ms + span * index as i64 / total;
            word.end_ms = start_ms + span * (index as i64 + 1) / total;
        }
    }

    // Corrige solapamientos y limita al segmento.
    let mut cursor = start_ms;
    for word in &mut words {
        word.start_ms = word.start_ms.max(cursor).clamp(start_ms, end_ms);
        word.end_ms = word.end_ms.max(word.start_ms + 1).min(end_ms);
        cursor = word.end_ms;
    }
    words.retain(|word| word.end_ms > word.start_ms);
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_text_scores_one() {
        let (diff, accuracy) = diff_words("El gato duerme.", "el gato duerme");
        assert!((accuracy - 1.0).abs() < f64::EPSILON);
        assert_eq!(
            diff.iter()
                .filter(|entry| matches!(entry.kind, DiffKind::Match))
                .count(),
            3
        );
    }

    #[test]
    fn substitutions_and_missing_words_are_reported() {
        let (diff, accuracy) = diff_words("El gato duerme", "El perro");
        assert!(accuracy < 1.0);
        assert!(
            diff.iter()
                .any(|entry| matches!(entry.kind, DiffKind::Substitution))
        );
        assert!(
            diff.iter()
                .any(|entry| matches!(entry.kind, DiffKind::Missing))
        );
    }

    #[test]
    fn accents_are_ignored_for_comparison() {
        let (_, accuracy) = diff_words("canción", "cancion");
        assert!((accuracy - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn catalog_has_tiny_and_base() {
        assert!(find_model("tiny").is_some());
        assert!(find_model("base").is_some());
    }
}
