use serde::{Deserialize, Serialize};

/// Método con el que se obtuvieron los capítulos de un audiolibro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChapterSource {
    /// Capítulos embebidos en los metadatos del M4B/M4A/MP3.
    Embedded,
    /// Pausas largas detectadas con `silencedetect`.
    Silence,
    /// Marcas acústicas (tonos) seguidas de silencio.
    Tone,
    /// Bloques fijos de diez minutos cuando no hay otra señal.
    Fixed,
    /// Secciones del libro de origen en audiolibros generados.
    Sections,
    /// Agrupación y títulos propuestos por un modelo de IA local.
    Ai,
    /// Capítulos corregidos manualmente por el usuario.
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedChapter {
    pub start_ms: i64,
    pub end_ms: i64,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterDetection {
    pub chapters: Vec<DetectedChapter>,
    pub source: ChapterSource,
}

pub const MIN_SILENCE_MS: i64 = 2_500;
pub const MIN_SILENCE_CHAPTER_GAP_MS: i64 = 300_000;
pub const MIN_TONE_CHAPTER_GAP_MS: i64 = 45_000;
pub const FIXED_CHAPTER_MS: i64 = 600_000;
const EDGE_GUARD_MS: i64 = 45_000;
const MAX_CHAPTERS: usize = 300;
const TONE_BLOCK_MS: u32 = 50;
const TONE_SAMPLE_RATE: u32 = 8_000;

/// Convierte capítulos embebidos en un conjunto contiguo. Devuelve `None` si
/// hay menos de dos, porque un único capítulo no aporta estructura.
pub fn from_embedded(items: &[(i64, i64, String)], duration_ms: i64) -> Option<ChapterDetection> {
    let duration = duration_ms.max(1);
    let mut items: Vec<(i64, i64, String)> = items
        .iter()
        .filter(|(start, end, _)| *end > *start && *end > 0)
        .cloned()
        .collect();
    if items.len() < 2 {
        return None;
    }
    items.sort_by_key(|(start, _, _)| *start);

    let mut chapters: Vec<DetectedChapter> = Vec::new();
    let mut cursor = 0;
    for (start, end, title) in items {
        let start = start.clamp(0, duration).max(cursor);
        let end = end.clamp(0, duration);
        if end <= start {
            continue;
        }
        chapters.push(DetectedChapter {
            start_ms: start,
            end_ms: end,
            title: normal_title(&title, chapters.len() + 1),
        });
        cursor = end;
    }

    if chapters.len() < 2 {
        return None;
    }

    chapters[0].start_ms = 0;
    for index in 0..chapters.len() {
        let next_start = chapters.get(index + 1).map(|chapter| chapter.start_ms);
        chapters[index].end_ms = match next_start {
            Some(next) => next.max(chapters[index].start_ms + 1),
            None => duration,
        };
        let end = chapters[index].end_ms;
        if let Some(next) = chapters.get_mut(index + 1) {
            next.start_ms = end;
        }
    }

    Some(ChapterDetection {
        chapters,
        source: ChapterSource::Embedded,
    })
}

/// Combina marcas de tono y silencios. Los tonos mandan; los silencios rellenan
/// cortes intermedios. Si no hay señales, cae a bloques fijos de diez minutos.
pub fn from_signals(
    silences: &[(i64, i64)],
    tone_ends: &[i64],
    duration_ms: i64,
) -> ChapterDetection {
    let duration = duration_ms.max(1);
    let mut tone_boundaries: Vec<i64> = tone_ends
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect();
    tone_boundaries.sort_unstable();
    tone_boundaries = enforce_gap(tone_boundaries, duration, MIN_TONE_CHAPTER_GAP_MS);

    if !tone_boundaries.is_empty() {
        let mut merged = tone_boundaries.clone();
        for boundary in silence_candidates(silences) {
            if merged
                .iter()
                .all(|existing| (existing - boundary).abs() >= MIN_TONE_CHAPTER_GAP_MS)
            {
                merged.push(boundary);
            }
        }
        merged.sort_unstable();
        merged = enforce_gap(merged, duration, MIN_TONE_CHAPTER_GAP_MS);
        if let Some(chapters) = chapters_from_boundaries(&merged, duration) {
            return ChapterDetection {
                chapters,
                source: ChapterSource::Tone,
            };
        }
    }

    let silence_boundaries = enforce_gap(
        silence_candidates(silences),
        duration,
        MIN_SILENCE_CHAPTER_GAP_MS,
    );
    if let Some(chapters) = chapters_from_boundaries(&silence_boundaries, duration) {
        return ChapterDetection {
            chapters,
            source: ChapterSource::Silence,
        };
    }

    ChapterDetection {
        chapters: fixed_chapters(duration),
        source: ChapterSource::Fixed,
    }
}

pub fn fixed_chapters(duration_ms: i64) -> Vec<DetectedChapter> {
    let duration = duration_ms.max(1);
    let mut chapters = Vec::new();
    let mut start = 0;
    while start < duration {
        let end = (start + FIXED_CHAPTER_MS).min(duration);
        chapters.push(DetectedChapter {
            start_ms: start,
            end_ms: end,
            title: format!("Parte {}", chapters.len() + 1),
        });
        start = end;
    }
    if chapters.is_empty() {
        chapters.push(DetectedChapter {
            start_ms: 0,
            end_ms: duration,
            title: "Parte 1".to_string(),
        });
    }
    chapters
}

/// Extrae las pausas del stderr de `ffmpeg -af silencedetect`.
pub fn parse_silences(stderr: &str, duration_ms: i64) -> Vec<(i64, i64)> {
    let duration = duration_ms.max(1);
    let mut start: Option<i64> = None;
    let mut spans = Vec::new();

    for line in stderr.lines() {
        if let Some((_, value)) = line.split_once("silence_start:") {
            start = parse_seconds(value);
        } else if let Some((_, value)) = line.split_once("silence_end:") {
            let end = parse_seconds(value);
            if let (Some(begin), Some(end)) = (start.take(), end) {
                spans.push((begin, end));
            }
        }
    }
    if let Some(begin) = start {
        spans.push((begin, duration));
    }

    spans.retain(|(begin, end)| *end > *begin);
    spans
        .into_iter()
        .map(|(begin, end)| (begin.clamp(0, duration), end.clamp(0, duration)))
        .filter(|(begin, end)| end > begin)
        .collect()
}

fn parse_seconds(value: &str) -> Option<i64> {
    let token = value.split_whitespace().next()?;
    let seconds = token.parse::<f64>().ok()?;
    if !seconds.is_finite() {
        return None;
    }
    Some((seconds * 1000.0).round() as i64)
}

fn silence_candidates(silences: &[(i64, i64)]) -> Vec<i64> {
    let mut candidates: Vec<(i64, i64)> = silences
        .iter()
        .filter(|(begin, end)| end - begin >= MIN_SILENCE_MS)
        .map(|(begin, end)| ((begin + end) / 2, end - begin))
        .collect();
    candidates.sort_by_key(|left| std::cmp::Reverse(left.1));
    candidates.truncate(MAX_CHAPTERS);
    let mut boundaries: Vec<i64> = candidates.into_iter().map(|(middle, _)| middle).collect();
    boundaries.sort_unstable();
    boundaries
}

fn enforce_gap(boundaries: Vec<i64>, duration_ms: i64, minimum_gap_ms: i64) -> Vec<i64> {
    let edge = EDGE_GUARD_MS.min(duration_ms / 10).max(0);
    let mut result = Vec::new();
    for boundary in boundaries {
        if boundary <= edge || boundary >= duration_ms - edge {
            continue;
        }
        let far_enough = result
            .last()
            .map(|last| boundary - last >= minimum_gap_ms)
            .unwrap_or(true);
        if far_enough && result.len() < MAX_CHAPTERS - 1 {
            result.push(boundary);
        }
    }
    result
}

fn chapters_from_boundaries(boundaries: &[i64], duration_ms: i64) -> Option<Vec<DetectedChapter>> {
    if boundaries.is_empty() {
        return None;
    }
    let mut chapters = Vec::with_capacity(boundaries.len() + 1);
    let mut start = 0;
    for boundary in boundaries {
        if *boundary <= start || *boundary >= duration_ms {
            continue;
        }
        chapters.push(DetectedChapter {
            start_ms: start,
            end_ms: *boundary,
            title: format!("Capítulo {}", chapters.len() + 1),
        });
        start = *boundary;
    }
    if start < duration_ms {
        chapters.push(DetectedChapter {
            start_ms: start,
            end_ms: duration_ms,
            title: format!("Capítulo {}", chapters.len() + 1),
        });
    }
    if chapters.len() < 2 {
        return None;
    }
    Some(chapters)
}

fn normal_title(title: &str, index: usize) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        format!("Capítulo {index}")
    } else {
        trimmed.to_string()
    }
}

/// Detector incremental de marcas acústicas. Consume bloques de 50 ms a 8 kHz
/// y busca tonos puros de 440 Hz o 1 kHz seguidos de una caída de energía.
pub struct ToneDetector {
    block_samples: usize,
    block_ms: i64,
    block_index: i64,
    pending: Vec<f32>,
    run_start_ms: Option<i64>,
    run_last_ms: i64,
    boundaries: Vec<i64>,
}

impl ToneDetector {
    pub fn new() -> Self {
        Self {
            block_samples: (TONE_SAMPLE_RATE * TONE_BLOCK_MS / 1000) as usize,
            block_ms: TONE_BLOCK_MS as i64,
            block_index: 0,
            pending: Vec::new(),
            run_start_ms: None,
            run_last_ms: 0,
            boundaries: Vec::new(),
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        self.pending.extend_from_slice(samples);
        while self.pending.len() >= self.block_samples {
            let block: Vec<f32> = self.pending.drain(..self.block_samples).collect();
            self.process(&block);
        }
    }

    pub fn finish(mut self) -> Vec<i64> {
        if let Some(start) = self.run_start_ms.take() {
            let duration = self.run_last_ms - start;
            if (120..=2_500).contains(&duration) {
                self.boundaries.push(self.run_last_ms + 40);
            }
        }
        self.boundaries.sort_unstable();
        self.boundaries
    }

    fn process(&mut self, block: &[f32]) {
        if block.len() < self.block_samples {
            return;
        }
        let start_ms = self.block_index * self.block_ms;
        self.block_index += 1;

        let energy: f32 = block.iter().map(|sample| sample * sample).sum();
        let count = block.len() as f32;
        let rms = (energy / count).sqrt();
        let ratio = if energy > 1e-7 {
            let tone = goertzel_power(block, TONE_SAMPLE_RATE, 1_000.0).max(goertzel_power(
                block,
                TONE_SAMPLE_RATE,
                440.0,
            ));
            (2.0 * tone / (count * energy)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let is_tone = rms > 0.008 && ratio > 0.55;
        if is_tone {
            if self.run_start_ms.is_none() {
                self.run_start_ms = Some(start_ms);
            }
            self.run_last_ms = start_ms + self.block_ms;
            return;
        }

        let Some(run_start) = self.run_start_ms else {
            return;
        };
        let duration = self.run_last_ms - run_start;
        let quiet = rms < 0.012 || ratio < 0.45;
        if quiet && (120..=2_500).contains(&duration) {
            self.boundaries.push(self.run_last_ms + 40);
            self.run_start_ms = None;
        } else if start_ms - run_start > 2_600 {
            self.run_start_ms = None;
        }
    }
}

impl Default for ToneDetector {
    fn default() -> Self {
        Self::new()
    }
}

fn goertzel_power(samples: &[f32], sample_rate: u32, frequency: f32) -> f32 {
    let count = samples.len();
    if count == 0 {
        return 0.0;
    }
    let normalized = frequency * count as f32 / sample_rate as f32;
    let omega = 2.0 * std::f32::consts::PI * normalized / count as f32;
    let coefficient = 2.0 * omega.cos();
    let mut previous = 0.0f32;
    let mut before_previous = 0.0f32;
    for sample in samples {
        let current = sample + coefficient * previous - before_previous;
        before_previous = previous;
        previous = current;
    }
    (previous * previous + before_previous * before_previous
        - coefficient * previous * before_previous)
        .max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(frequency: f32, seconds: f32, amplitude: f32) -> Vec<f32> {
        let sample_rate = TONE_SAMPLE_RATE as f32;
        let count = (sample_rate * seconds) as usize;
        (0..count)
            .map(|index| {
                amplitude
                    * (2.0 * std::f32::consts::PI * frequency * index as f32 / sample_rate).sin()
            })
            .collect()
    }

    #[test]
    fn parses_silencedetect_output() {
        let stderr = r#"
[silencedetect @ 0x1] silence_start: 12.5
[silencedetect @ 0x1] silence_end: 15.25 | silence_duration: 2.75
[silencedetect @ 0x1] silence_start: 99.5
"#;
        let spans = parse_silences(stderr, 120_000);
        assert_eq!(spans, vec![(12_500, 15_250), (99_500, 120_000)]);
    }

    #[test]
    fn detects_a_one_khz_marker_after_silence() {
        let mut detector = ToneDetector::new();
        detector.push(&vec![0.0; 8_000 / 2]);
        detector.push(&sine(1_000.0, 0.45, 0.4));
        detector.push(&vec![0.0; 8_000 / 2]);
        let boundaries = detector.finish();
        assert_eq!(boundaries.len(), 1, "{boundaries:?}");
        assert!((boundaries[0] - 940).abs() < 120, "{boundaries:?}");
    }

    #[test]
    fn detects_a_440_hz_marker() {
        let mut detector = ToneDetector::new();
        detector.push(&vec![0.0; 4_000]);
        detector.push(&sine(440.0, 0.6, 0.35));
        detector.push(&vec![0.0; 4_000]);
        assert_eq!(detector.finish().len(), 1);
    }

    #[test]
    fn speech_like_noise_is_not_a_tone() {
        let mut detector = ToneDetector::new();
        let mut seed = 7u32;
        let noise: Vec<f32> = (0..24_000)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((seed as f32 / u32::MAX as f32) - 0.5) * 0.2
            })
            .collect();
        detector.push(&noise);
        assert!(detector.finish().is_empty());
    }

    #[test]
    fn embedded_chapters_are_normalized_and_contiguous() {
        let items = vec![
            (0, 60_000, "Intro".to_string()),
            (90_000, 120_000, "".to_string()),
            (120_000, 200_000, "Cierre".to_string()),
        ];
        let detection = from_embedded(&items, 240_000).expect("capítulos");
        assert_eq!(detection.source, ChapterSource::Embedded);
        assert_eq!(detection.chapters.len(), 3);
        assert_eq!(detection.chapters[0].start_ms, 0);
        assert_eq!(detection.chapters[0].end_ms, 90_000);
        assert_eq!(detection.chapters[1].start_ms, 90_000);
        assert_eq!(detection.chapters[1].title, "Capítulo 2");
        assert_eq!(detection.chapters[2].end_ms, 240_000);
    }

    #[test]
    fn signal_detection_prefers_tones_and_falls_back_to_silence() {
        let tone = from_signals(&[(30_000, 34_000)], &[120_000], 600_000);
        assert_eq!(tone.source, ChapterSource::Tone);
        assert!(tone.chapters.len() >= 2);

        let silence = from_signals(&[(200_000, 205_000)], &[], 600_000);
        assert_eq!(silence.source, ChapterSource::Silence);
        assert!(silence.chapters.len() >= 2);

        let fixed = from_signals(&[], &[], 600_000);
        assert_eq!(fixed.source, ChapterSource::Fixed);
        assert_eq!(fixed.chapters.len(), 1);
    }

    #[test]
    fn fixed_chapters_cover_the_whole_duration() {
        let chapters = fixed_chapters(FIXED_CHAPTER_MS * 2 + 1_000);
        assert_eq!(chapters.len(), 3);
        assert_eq!(chapters[0].start_ms, 0);
        assert_eq!(chapters[2].end_ms, FIXED_CHAPTER_MS * 2 + 1_000);
    }
}
