use serde::{Deserialize, Serialize};

/// Presets de ecualización pensados para voz hablada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum EqPreset {
    #[default]
    Neutral,
    ClearVoice,
    Warm,
    Bright,
}

/// Tratamiento que se aplica al audio de un audiolibro. La normalización usa
/// `loudnorm`; los presets de EQ usan filtros `equalizer` de ffmpeg.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTreatment {
    #[serde(default)]
    pub normalize: bool,
    #[serde(default = "default_lufs")]
    pub target_lufs: f32,
    #[serde(default)]
    pub eq: EqPreset,
}

pub const DEFAULT_LUFS: f32 = -18.0;

/// Medidas de la primera pasada de `loudnorm`. La segunda pasada las usa para
/// alcanzar el objetivo sin limitar la dinámica de forma agresiva.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessMeasurement {
    pub input_i: f64,
    pub input_tp: f64,
    pub input_lra: f64,
    pub input_thresh: f64,
    pub target_offset: f64,
}

pub fn parse_loudnorm_json(stderr: &str) -> Option<LoudnessMeasurement> {
    let start = stderr.find('{')?;
    let end = stderr.rfind('}')?;
    let value: serde_json::Value = serde_json::from_str(&stderr[start..=end]).ok()?;
    let read = |key: &str| -> Option<f64> {
        value
            .get(key)
            .and_then(|entry| entry.as_str())
            .and_then(|entry| entry.parse::<f64>().ok())
            .filter(|entry| entry.is_finite())
    };
    Some(LoudnessMeasurement {
        input_i: read("input_i")?,
        input_tp: read("input_tp")?,
        input_lra: read("input_lra")?,
        input_thresh: read("input_thresh")?,
        target_offset: read("target_offset")?,
    })
}

fn default_lufs() -> f32 {
    DEFAULT_LUFS
}

impl Default for AudioTreatment {
    fn default() -> Self {
        Self {
            normalize: false,
            target_lufs: DEFAULT_LUFS,
            eq: EqPreset::Neutral,
        }
    }
}

impl AudioTreatment {
    pub fn clamped(self) -> Self {
        let target_lufs = if self.target_lufs.is_finite() {
            self.target_lufs.clamp(-24.0, -12.0)
        } else {
            DEFAULT_LUFS
        };
        Self {
            normalize: self.normalize,
            target_lufs,
            eq: self.eq,
        }
    }

    pub fn is_noop(self) -> bool {
        let settings = self.clamped();
        !settings.normalize && settings.eq == EqPreset::Neutral
    }

    /// Cadena de filtros para `ffmpeg -af`. Devuelve `None` si no hay cambios.
    pub fn filter_chain(self) -> Option<String> {
        self.filter_chain_with(None)
    }

    /// Variante de dos pasadas: si hay medidas, `loudnorm` las usa para
    /// corregir el volumen con ganancia lineal y evitar bombeo.
    pub fn filter_chain_with(self, measurement: Option<&LoudnessMeasurement>) -> Option<String> {
        let settings = self.clamped();
        let mut filters = Vec::new();
        if settings.normalize {
            filters.push(match measurement {
                Some(measured) => format!(
                    "loudnorm=I={}:TP=-1.5:LRA=11:measured_I={}:measured_TP={}:measured_LRA={}:measured_thresh={}:offset={}:linear=true",
                    settings.target_lufs,
                    measured.input_i,
                    measured.input_tp,
                    measured.input_lra,
                    measured.input_thresh,
                    measured.target_offset
                ),
                None => format!(
                    "loudnorm=I={}:TP=-1.5:LRA=11",
                    settings.target_lufs
                ),
            });
        }
        match settings.eq {
            EqPreset::Neutral => {}
            EqPreset::ClearVoice => filters.push(
                "equalizer=f=250:t=q:w=1:g=-1.5,equalizer=f=3000:t=q:w=1:g=2.5,equalizer=f=8000:t=q:w=1:g=1.5"
                    .to_string(),
            ),
            EqPreset::Warm => filters.push(
                "equalizer=f=120:t=q:w=1:g=2.0,equalizer=f=500:t=q:w=1:g=1.0,equalizer=f=6000:t=q:w=1:g=-1.5"
                    .to_string(),
            ),
            EqPreset::Bright => filters.push(
                "equalizer=f=2000:t=q:w=1:g=1.5,equalizer=f=8000:t=q:w=1:g=3.0".to_string(),
            ),
        }
        if filters.is_empty() {
            None
        } else {
            Some(filters.join(","))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioWaveform {
    /// Amplitudes normalizadas de 0 a 1, una por barra.
    pub buckets: Vec<f32>,
    pub duration_ms: i64,
}

pub const DEFAULT_WAVEFORM_BUCKETS: usize = 600;
pub const MIN_WAVEFORM_BUCKETS: usize = 64;
pub const MAX_WAVEFORM_BUCKETS: usize = 2_000;

/// Acumula picos por segundo mientras llega el PCM y los reduce a un número
/// fijo de barras al terminar. Funciona igual con audiolibros de 20 horas o de
/// diez segundos.
pub struct WaveformBuilder {
    samples_per_second: usize,
    current_peak: f32,
    current_samples: usize,
    seconds: Vec<f32>,
}

impl WaveformBuilder {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            samples_per_second: sample_rate.max(1) as usize,
            current_peak: 0.0,
            current_samples: 0,
            seconds: Vec::new(),
        }
    }

    pub fn push(&mut self, samples: &[i16]) {
        for sample in samples {
            let value = (*sample as f32 / i16::MAX as f32).abs();
            self.current_peak = self.current_peak.max(value);
            self.current_samples += 1;
            if self.current_samples >= self.samples_per_second {
                self.seconds.push(self.current_peak);
                self.current_peak = 0.0;
                self.current_samples = 0;
            }
        }
    }

    pub fn duration_ms(&self) -> i64 {
        let partial = if self.current_samples > 0 { 1_000 } else { 0 };
        self.seconds.len() as i64 * 1_000 + partial
    }

    pub fn finish(mut self, buckets: usize) -> Vec<f32> {
        if self.current_samples > 0 {
            self.seconds.push(self.current_peak);
        }
        let buckets = buckets.clamp(MIN_WAVEFORM_BUCKETS, MAX_WAVEFORM_BUCKETS);
        let total = self.seconds.len();
        if total == 0 {
            return vec![0.0; buckets];
        }

        let mut peaks = Vec::with_capacity(buckets);
        for index in 0..buckets {
            let start = index * total / buckets;
            let end = (((index + 1) * total) / buckets).max(start + 1).min(total);
            let peak = self.seconds[start..end]
                .iter()
                .copied()
                .fold(0.0f32, f32::max);
            peaks.push(peak);
        }

        let maximum = peaks.iter().copied().fold(0.0f32, f32::max);
        if maximum > 0.02 {
            for peak in &mut peaks {
                *peak = (*peak / maximum).sqrt().clamp(0.0, 1.0);
            }
        }
        peaks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_treatment_is_a_noop() {
        assert!(AudioTreatment::default().is_noop());
        assert_eq!(AudioTreatment::default().filter_chain(), None);
    }

    #[test]
    fn builds_loudnorm_and_eq_chain() {
        let treatment = AudioTreatment {
            normalize: true,
            target_lufs: -20.0,
            eq: EqPreset::ClearVoice,
        };
        let chain = treatment.filter_chain().expect("cadena");
        assert!(chain.contains("loudnorm=I=-20"));
        assert!(chain.contains("equalizer=f=3000"));
    }

    #[test]
    fn parses_the_first_loudnorm_pass() {
        let stderr = r#"
[Parsed_loudnorm_0 @ 0x1] 
{
    "input_i" : "-13.03",
    "input_tp" : "-7.90",
    "input_lra" : "0.00",
    "input_thresh" : "-24.00",
    "output_i" : "-14.22",
    "target_offset" : "-3.78"
}
"#;
        let measured = parse_loudnorm_json(stderr).expect("medidas");
        assert!((measured.input_i + 13.03).abs() < 0.001);
        assert!((measured.target_offset + 3.78).abs() < 0.001);
    }

    #[test]
    fn builds_a_two_pass_loudnorm_chain() {
        let measurement = LoudnessMeasurement {
            input_i: -23.0,
            input_tp: -4.0,
            input_lra: 6.0,
            input_thresh: -33.0,
            target_offset: 2.0,
        };
        let treatment = AudioTreatment {
            normalize: true,
            target_lufs: -18.0,
            eq: EqPreset::Warm,
        };
        let chain = treatment
            .filter_chain_with(Some(&measurement))
            .expect("cadena");
        assert!(chain.contains("measured_I=-23"));
        assert!(chain.contains("linear=true"));
        assert!(chain.contains("equalizer=f=120"));
    }

    #[test]
    fn clamps_lufs_to_a_safe_range() {
        let low = AudioTreatment {
            normalize: true,
            target_lufs: -99.0,
            eq: EqPreset::Neutral,
        }
        .clamped();
        assert_eq!(low.target_lufs, -24.0);

        let invalid = AudioTreatment {
            normalize: true,
            target_lufs: f32::NAN,
            eq: EqPreset::Neutral,
        }
        .clamped();
        assert_eq!(invalid.target_lufs, DEFAULT_LUFS);
    }

    #[test]
    fn waveform_has_the_requested_size_and_normalizes() {
        let mut builder = WaveformBuilder::new(8_000);
        let loud: Vec<i16> = (0..8_000).map(|i| ((i % 100) as i16) * 100).collect();
        builder.push(&loud);
        let quiet = vec![1_000i16; 8_000];
        builder.push(&quiet);
        let peaks = builder.finish(100);
        assert_eq!(peaks.len(), 100);
        assert!(peaks.iter().all(|peak| (0.0..=1.0).contains(peak)));
        assert!(peaks.iter().any(|peak| *peak > 0.9));
    }

    #[test]
    fn short_waveforms_are_stretched_to_the_bucket_count() {
        let mut builder = WaveformBuilder::new(8_000);
        builder.push(&vec![20_000i16; 8_000]);
        let peaks = builder.finish(MIN_WAVEFORM_BUCKETS);
        assert_eq!(peaks.len(), MIN_WAVEFORM_BUCKETS);
        assert!(peaks.iter().all(|peak| *peak > 0.9));
    }
}
