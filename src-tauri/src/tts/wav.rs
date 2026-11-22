use std::collections::HashMap;

use crate::error::Result;
use crate::tts::engine::AudioChunk;

/// Codifica PCM mono de 16 bits en un WAV listo para `<audio>` o Web Audio.
pub fn encode_wav(chunk: &AudioChunk) -> Result<Vec<u8>> {
    let channels = chunk.channels.max(1);
    let sample_rate = chunk.sample_rate;
    let bits_per_sample = 16u16;
    let block_align = channels * bits_per_sample / 8;
    let byte_rate = sample_rate * block_align as u32;
    let data_len = (chunk.pcm.len() * 2) as u32;
    let riff_len = 36 + data_len;

    let mut wav = Vec::with_capacity(44 + chunk.pcm.len() * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_len.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());

    for sample in &chunk.pcm {
        wav.extend_from_slice(&sample.to_le_bytes());
    }

    Ok(wav)
}

#[derive(Debug, Clone, Default)]
pub struct Watermark {
    pub profile_id: String,
    pub profile_name: String,
}

/// Añade un chunk `id3 ` al WAV. Es la marca mínima que pide la fase: WAV no
/// lleva ID3 de forma nativa, pero muchos reproductores y `ffmpeg` leen este
/// chunk, y la app muestra además un aviso visible.
pub fn encode_wav_with_watermark(chunk: &AudioChunk, watermark: &Watermark) -> Result<Vec<u8>> {
    let base = encode_wav(chunk)?;
    let id3 = build_id3(watermark);

    let fmt = &base[12..36];
    let data_header = &base[36..44];
    let pcm = &base[44..];

    let mut out = Vec::with_capacity(base.len() + 8 + id3.len());
    out.extend_from_slice(b"RIFF");
    let riff_len = 4 + fmt.len() + 8 + id3.len() + 8 + pcm.len();
    out.extend_from_slice(&(riff_len as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(fmt);
    out.extend_from_slice(b"id3 ");
    out.extend_from_slice(&(id3.len() as u32).to_le_bytes());
    out.extend_from_slice(&id3);
    out.extend_from_slice(data_header);
    out.extend_from_slice(pcm);
    Ok(out)
}

fn build_id3(watermark: &Watermark) -> Vec<u8> {
    let frames = [
        ("KORA_CLONED_VOICE", "1".to_string()),
        ("KORA_VOICE_PROFILE", watermark.profile_id.clone()),
        ("KORA_VOICE_NAME", watermark.profile_name.clone()),
        ("KORA_WATERMARK", "cloned-voice-v1".to_string()),
    ];

    let mut body = Vec::new();
    for (description, value) in frames {
        let mut content = Vec::new();
        content.push(3u8); // UTF-8
        content.extend_from_slice(description.as_bytes());
        content.push(0);
        content.extend_from_slice(value.as_bytes());
        content.push(0);

        body.extend_from_slice(b"TXXX");
        body.extend_from_slice(&(content.len() as u32).to_be_bytes());
        body.extend_from_slice(&0u16.to_be_bytes());
        body.extend_from_slice(&content);
    }

    let mut tag = Vec::with_capacity(10 + body.len());
    tag.extend_from_slice(b"ID3");
    tag.push(4); // ID3v2.4
    tag.push(0);
    tag.push(0);
    tag.extend_from_slice(&synchsafe(body.len() as u32));
    tag.extend_from_slice(&body);
    tag
}

fn synchsafe(value: u32) -> [u8; 4] {
    [
        ((value >> 21) & 0x7f) as u8,
        ((value >> 14) & 0x7f) as u8,
        ((value >> 7) & 0x7f) as u8,
        (value & 0x7f) as u8,
    ]
}

fn unsynchsafe(bytes: [u8; 4]) -> u32 {
    ((bytes[0] as u32 & 0x7f) << 21)
        | ((bytes[1] as u32 & 0x7f) << 14)
        | ((bytes[2] as u32 & 0x7f) << 7)
        | (bytes[3] as u32 & 0x7f)
}

/// Lee las etiquetas TXXX del chunk `id3 ` de un WAV. Devuelve un mapa vacío
/// si el archivo no tiene marca.
pub fn read_watermark(wav: &[u8]) -> HashMap<String, String> {
    let mut tags = HashMap::new();
    if wav.len() < 12 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return tags;
    }

    let mut offset = 12usize;
    while offset + 8 <= wav.len() {
        let id = &wav[offset..offset + 4];
        let size =
            u32::from_le_bytes(wav[offset + 4..offset + 8].try_into().unwrap_or([0; 4])) as usize;
        let start = offset + 8;
        let end = start.saturating_add(size);
        if end > wav.len() {
            break;
        }

        if id == b"id3 " {
            parse_id3(&wav[start..end], &mut tags);
            break;
        }

        // Los chunks se alinean a un número par de bytes.
        offset = end + (size % 2);
    }

    tags
}

fn parse_id3(tag: &[u8], tags: &mut HashMap<String, String>) {
    if tag.len() < 10 || &tag[0..3] != b"ID3" {
        return;
    }
    let size = unsynchsafe([tag[6], tag[7], tag[8], tag[9]]) as usize;
    let end = (10 + size).min(tag.len());
    let mut cursor = 10usize;

    while cursor + 10 <= end {
        let frame_id = &tag[cursor..cursor + 4];
        if frame_id == b"\0\0\0\0" {
            break;
        }
        let frame_size =
            u32::from_be_bytes(tag[cursor + 4..cursor + 8].try_into().unwrap_or([0; 4])) as usize;
        let start = cursor + 10;
        let finish = start.saturating_add(frame_size);
        if finish > end {
            break;
        }

        if frame_id == b"TXXX" {
            let content = &tag[start..finish];
            if content.len() > 1 {
                let encoding = content[0];
                let rest = &content[1..];
                let split = rest.iter().position(|value| *value == 0);
                if let Some(split) = split {
                    let description = decode_text(encoding, &rest[..split]);
                    let value_bytes = &rest[split + 1..];
                    let value_end = value_bytes
                        .iter()
                        .position(|value| *value == 0)
                        .unwrap_or(value_bytes.len());
                    let value = decode_text(encoding, &value_bytes[..value_end]);
                    tags.insert(description, value);
                }
            }
        }

        cursor = finish;
    }
}

fn decode_text(encoding: u8, bytes: &[u8]) -> String {
    match encoding {
        1 | 2 => {
            let mut units = Vec::with_capacity(bytes.len() / 2);
            let (chunks, _) = bytes.as_chunks::<2>();
            for chunk in chunks {
                units.push(u16::from_be_bytes(*chunk));
            }
            String::from_utf16_lossy(&units)
        }
        3 => String::from_utf8_lossy(bytes).to_string(),
        _ => bytes.iter().map(|value| *value as char).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_riff_header_and_samples() {
        let chunk = AudioChunk {
            pcm: vec![0, 32767, -32768],
            sample_rate: 22_050,
            channels: 1,
        };

        let wav = encode_wav(&chunk).expect("wav");
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), 44 + 3 * 2);
        assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()), 42);
    }

    #[test]
    fn watermark_round_trips_inside_a_wav() {
        let chunk = AudioChunk {
            pcm: vec![100, -100, 200, -200],
            sample_rate: 24_000,
            channels: 1,
        };
        let watermark = Watermark {
            profile_id: "abc-123".to_string(),
            profile_name: "Mi voz".to_string(),
        };

        let wav = encode_wav_with_watermark(&chunk, &watermark).expect("wav");
        assert_eq!(&wav[0..4], b"RIFF");
        let tags = read_watermark(&wav);
        assert_eq!(tags.get("KORA_CLONED_VOICE").map(String::as_str), Some("1"));
        assert_eq!(
            tags.get("KORA_VOICE_PROFILE").map(String::as_str),
            Some("abc-123")
        );
        assert_eq!(
            tags.get("KORA_VOICE_NAME").map(String::as_str),
            Some("Mi voz")
        );
        assert!(tags.contains_key("KORA_WATERMARK"));
    }
}
