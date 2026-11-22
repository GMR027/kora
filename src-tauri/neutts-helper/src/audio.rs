use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result, bail};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Decodifica cualquier formato soportado por Symphonia y lo mezcla a mono.
pub fn decode(path: &Path) -> Result<DecodedAudio> {
    let file = File::open(path)
        .with_context(|| format!("no se pudo abrir el audio {}", path.display()))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("formato de audio no reconocido")?;
    let mut format = probed.format;

    let track = format
        .default_track()
        .context("el audio no tiene una pista utilizable")?
        .clone();
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .context("códec de audio no soportado")?;

    let mut samples = Vec::new();
    let mut sample_rate = codec_params.sample_rate.unwrap_or(0);
    let mut buffer: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(error))
                if error.kind() == ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => break,
            Err(error) => return Err(error).context("error leyendo el audio"),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                sample_rate = spec.rate;
                let duration = decoded.capacity() as u64;
                let slot = buffer.get_or_insert_with(|| SampleBuffer::new(duration, spec));
                slot.copy_interleaved_ref(decoded);
                append_mono(slot.samples(), spec.channels.count(), &mut samples);
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(error) => return Err(error).context("error decodificando el audio"),
        }
    }

    if samples.is_empty() {
        bail!("el audio no contiene muestras");
    }
    if sample_rate == 0 {
        bail!("el audio no declara una frecuencia de muestreo");
    }

    Ok(DecodedAudio {
        samples,
        sample_rate,
    })
}

fn append_mono(interleaved: &[f32], channels: usize, out: &mut Vec<f32>) {
    if channels <= 1 {
        out.extend_from_slice(interleaved);
        return;
    }
    for frame in interleaved.chunks(channels) {
        let sum: f32 = frame.iter().sum();
        out.push(sum / frame.len() as f32);
    }
}

pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
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

pub fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .with_context(|| format!("no se pudo crear {}", path.display()))?;
    for sample in samples {
        writer.write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16)?;
    }
    writer.finalize()?;
    Ok(())
}

/// Deja el audio en 16 kHz mono, que es lo que espera el codificador.
pub fn prepare_reference(path: &Path) -> Result<Vec<f32>> {
    let decoded = decode(path)?;
    Ok(resample(&decoded.samples, decoded.sample_rate, 16_000))
}

/// Escribe Ogg/Opus mono. El perfil guarda esta copia para poder exportarlo
/// sin depender del formato original del usuario.
pub fn write_opus(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    use ogg::writing::{PacketWriteEndInfo, PacketWriter};
    use opus::{Application, Channels, Encoder};
    use std::fs::File;
    use std::io::BufWriter;

    let (rate, samples) = match sample_rate {
        8_000 | 12_000 | 16_000 | 24_000 | 48_000 => (sample_rate, samples.to_vec()),
        _ => (16_000u32, resample(samples, sample_rate, 16_000)),
    };

    let mut encoder = Encoder::new(rate, Channels::Mono, Application::Audio)
        .map_err(|error| anyhow::anyhow!("Opus: {error}"))?;
    let pre_skip = encoder
        .get_lookahead()
        .map_err(|error| anyhow::anyhow!("Opus: {error}"))? as u16;

    let file =
        File::create(path).with_context(|| format!("no se pudo crear {}", path.display()))?;
    let mut writer = PacketWriter::new(BufWriter::new(file));
    let serial = 0x4b4f_5241u32; // "KORA"

    let mut head = Vec::with_capacity(19);
    head.extend_from_slice(b"OpusHead");
    head.push(1); // versión
    head.push(1); // canales
    head.extend_from_slice(&pre_skip.to_le_bytes());
    head.extend_from_slice(&sample_rate.max(1).to_le_bytes());
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

    let frame = (rate / 50).max(1) as usize;
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
        .sync_all()?;
    Ok(())
}
