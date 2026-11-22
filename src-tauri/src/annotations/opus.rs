use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use ogg::writing::{PacketWriteEndInfo, PacketWriter};
use opus::{Application, Channels, Encoder};

/// Opus trabaja de forma nativa a 48 kHz. Cualquier otra frecuencia se
/// remuestrea antes de codificar.
pub const OPUS_RATE: u32 = 48_000;

pub fn write_opus(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let samples = if sample_rate == OPUS_RATE {
        samples.to_vec()
    } else {
        resample_linear(samples, sample_rate, OPUS_RATE)
    };
    if samples.is_empty() {
        anyhow::bail!("la nota de voz quedó vacía");
    }

    let mut encoder = Encoder::new(OPUS_RATE, Channels::Mono, Application::Audio)
        .map_err(|error| anyhow::anyhow!("Opus: {error}"))?;
    let pre_skip = encoder
        .get_lookahead()
        .map_err(|error| anyhow::anyhow!("Opus: {error}"))? as u16;

    let file = std::fs::File::create(path)
        .with_context(|| format!("no se pudo crear {}", path.display()))?;
    let mut writer = PacketWriter::new(BufWriter::new(file));
    let serial = 0x4b4f_5241u32; // "KORA"

    let mut head = Vec::with_capacity(19);
    head.extend_from_slice(b"OpusHead");
    head.push(1); // versión
    head.push(1); // canales
    head.extend_from_slice(&pre_skip.to_le_bytes());
    head.extend_from_slice(&OPUS_RATE.to_le_bytes());
    head.extend_from_slice(&0i16.to_le_bytes()); // ganancia
    head.push(0); // familia de mapeo
    writer
        .write_packet(head, serial, PacketWriteEndInfo::EndPage, 0)
        .map_err(|error| anyhow::anyhow!("Ogg: {error}"))?;

    let vendor = b"Kora";
    let mut tags = Vec::with_capacity(8 + vendor.len() + 4);
    tags.extend_from_slice(b"OpusTags");
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor);
    tags.extend_from_slice(&0u32.to_le_bytes());
    writer
        .write_packet(tags, serial, PacketWriteEndInfo::EndPage, 0)
        .map_err(|error| anyhow::anyhow!("Ogg: {error}"))?;

    let frame = (OPUS_RATE / 50) as usize;
    let chunks: Vec<&[f32]> = samples.chunks(frame).collect();
    let mut granule = 0u64;
    let mut output = vec![0u8; 4000];

    for (index, chunk) in chunks.iter().enumerate() {
        let mut padded = (*chunk).to_vec();
        if padded.len() < frame {
            padded.resize(frame, 0.0);
        }
        let length = encoder
            .encode_float(&padded, &mut output)
            .map_err(|error| anyhow::anyhow!("Opus: {error}"))?;
        granule += 960; // 20 ms a 48 kHz
        let end = if index + 1 == chunks.len() {
            PacketWriteEndInfo::EndStream
        } else {
            PacketWriteEndInfo::NormalPacket
        };
        writer
            .write_packet(output[..length].to_vec(), serial, end, granule)
            .map_err(|error| anyhow::anyhow!("Ogg: {error}"))?;
    }

    writer
        .into_inner()
        .into_inner()
        .map_err(|error| anyhow::anyhow!("Ogg: {error}"))?
        .flush()
        .context("no se pudo vaciar el Opus")?;

    Ok(())
}

fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if input.is_empty() || from == 0 {
        return Vec::new();
    }
    if from == to {
        return input.to_vec();
    }

    let ratio = from as f64 / to as f64;
    let output_len = ((input.len() as f64) / ratio).round() as usize;
    let mut output = Vec::with_capacity(output_len.max(1));

    for index in 0..output_len.max(1) {
        let source = index as f64 * ratio;
        let left = source.floor() as usize;
        let fraction = (source - left as f64) as f32;
        let a = input.get(left).copied().unwrap_or(0.0);
        let b = input.get(left + 1).copied().unwrap_or(a);
        output.push(a + (b - a) * fraction);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_an_ogg_opus_file() {
        let workspace = tempfile::tempdir().expect("tmp");
        let path = workspace.path().join("nota.opus");
        let samples: Vec<f32> = (0..24_000)
            .map(|index| {
                (index as f32 * 440.0 * std::f32::consts::TAU / OPUS_RATE as f32).sin() * 0.2
            })
            .collect();

        write_opus(&path, &samples, OPUS_RATE).expect("opus");
        let bytes = std::fs::read(&path).expect("leer");
        assert_eq!(&bytes[0..4], b"OggS");
        assert!(bytes.windows(8).any(|window| window == b"OpusHead"));
        assert!(bytes.windows(8).any(|window| window == b"OpusTags"));
        assert!(bytes.len() > 100);
    }
}
