use std::path::Path;

use anyhow::{Context, Result};

use crate::tts::AudioChunk;

/// Frecuencia única de los WAV intermedios. AAC se codifica después a partir de
/// ellos, así que todo el libro comparte el mismo formato.
pub const OUTPUT_RATE: u32 = 24_000;

/// Mezcla a mono y remuestrea a 24 kHz. El audio de Piper suele venir a
/// 22.05 kHz y el de NeuTTS a 24 kHz.
pub fn chunk_to_samples(chunk: &AudioChunk) -> Vec<i16> {
    let channels = chunk.channels.max(1) as usize;
    let mono: Vec<f32> = if channels == 1 {
        chunk
            .pcm
            .iter()
            .map(|sample| *sample as f32 / i16::MAX as f32)
            .collect()
    } else {
        chunk
            .pcm
            .chunks(channels)
            .map(|frame| {
                let sum: f32 = frame
                    .iter()
                    .map(|sample| *sample as f32 / i16::MAX as f32)
                    .sum();
                sum / frame.len().max(1) as f32
            })
            .collect()
    };

    let resampled = resample(&mono, chunk.sample_rate.max(1), OUTPUT_RATE);
    resampled
        .into_iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16)
        .collect()
}

pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if input.is_empty() || from == 0 {
        return Vec::new();
    }
    if from == to {
        return input.to_vec();
    }

    let ratio = from as f64 / to as f64;
    let output_len = ((input.len() as f64) / ratio).round().max(1.0) as usize;
    let mut output = Vec::with_capacity(output_len);

    for index in 0..output_len {
        let source = index as f64 * ratio;
        let left = source.floor() as usize;
        let fraction = (source - left as f64) as f32;
        let a = input.get(left).copied().unwrap_or(0.0);
        let b = input.get(left + 1).copied().unwrap_or(a);
        output.push(a + (b - a) * fraction);
    }

    output
}

pub fn write_wav(path: &Path, samples: &[i16]) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: OUTPUT_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .with_context(|| format!("no se pudo crear {}", path.display()))?;
    for sample in samples {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;
    Ok(())
}

pub fn duration_ms(samples: usize) -> i64 {
    (samples as i64).saturating_mul(1000) / OUTPUT_RATE as i64
}

pub fn push_silence(samples: &mut Vec<i16>, milliseconds: u32) {
    let count = OUTPUT_RATE as usize * milliseconds as usize / 1000;
    samples.extend(std::iter::repeat_n(0, count));
}

/// Convierte un WAV mono a f32. Lo usa la transcripción inversa.
pub fn read_wav_mono(path: &Path) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)
        .with_context(|| format!("no se pudo abrir {}", path.display()))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;

    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()?,
        hound::SampleFormat::Int => reader
            .samples::<i16>()
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(|sample| sample as f32 / i16::MAX as f32)
            .collect(),
    };

    if channels <= 1 {
        return Ok(interleaved);
    }

    Ok(interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len().max(1) as f32)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_chunks_become_mono_at_the_shared_rate() {
        let chunk = AudioChunk {
            pcm: vec![1000, 3000, -1000, -3000, 2000, 4000, -2000, -4000],
            sample_rate: 48_000,
            channels: 2,
        };
        let samples = chunk_to_samples(&chunk);
        assert!(samples.len() >= 2);
        assert!(!samples.is_empty());
    }

    #[test]
    fn duration_uses_the_output_rate() {
        assert_eq!(duration_ms(24_000), 1000);
    }
}
