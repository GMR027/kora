use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::chapters::{ToneDetector, parse_silences};
use super::dsp::{
    AudioTreatment, AudioWaveform, LoudnessMeasurement, WaveformBuilder, parse_loudnorm_json,
};

/// Ruta a ffmpeg. Se puede forzar con `KORA_FFMPEG`; si no, se usa el del PATH.
pub fn ffmpeg() -> PathBuf {
    ffmpeg_command()
}

pub fn ffprobe() -> PathBuf {
    std::env::var_os("KORA_FFPROBE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("ffprobe"))
}

fn ffmpeg_command() -> PathBuf {
    std::env::var_os("KORA_FFMPEG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("ffmpeg"))
}

pub fn ensure_available() -> Result<()> {
    let output = Command::new(ffmpeg_command())
        .arg("-version")
        .output()
        .map_err(|error| {
            anyhow::anyhow!(
                "no se encontró ffmpeg ({}); instálalo o define KORA_FFMPEG: {error}",
                ffmpeg_command().display()
            )
        })?;
    if !output.status.success() {
        bail!("ffmpeg no respondió correctamente");
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct MuxChapter {
    pub title: String,
    pub path: PathBuf,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Default)]
pub struct ProbeInfo {
    pub duration_ms: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub language: Option<String>,
    pub genre: Option<String>,
    pub comment: Option<String>,
    pub chapters: Vec<ProbeChapter>,
}

#[derive(Debug, Clone)]
pub struct ProbeChapter {
    pub start_ms: i64,
    pub end_ms: i64,
    pub title: String,
}

pub fn mux(
    chapters: &[MuxChapter],
    output: &Path,
    title: &str,
    author: &str,
    metadata: &[(&str, &str)],
) -> Result<()> {
    if chapters.is_empty() {
        bail!("no hay capítulos para codificar");
    }
    ensure_available()?;

    let workspace = output
        .parent()
        .ok_or_else(|| anyhow::anyhow!("la salida no tiene directorio"))?;
    let list_path = workspace.join("concat.txt");
    let metadata_path = workspace.join("chapters.txt");

    let mut list = String::new();
    for chapter in chapters {
        list.push_str(&format!("file '{}'\n", escape_concat(&chapter.path)));
    }
    std::fs::write(&list_path, list)
        .with_context(|| format!("no se pudo escribir {}", list_path.display()))?;

    let mut tags = String::from(";FFMETADATA1\n");
    tags.push_str(&format!("title={}\n", escape_metadata(title)));
    tags.push_str(&format!("album={}\n", escape_metadata(title)));
    tags.push_str(&format!("artist={}\n", escape_metadata(author)));
    tags.push_str("genre=Audiobook\n");
    for (key, value) in metadata {
        tags.push_str(&format!("{key}={}\n", escape_metadata(value)));
    }

    let mut cursor = 0i64;
    for chapter in chapters {
        let end = cursor + chapter.duration_ms.max(1);
        tags.push_str("[CHAPTER]\nTIMEBASE=1/1000\n");
        tags.push_str(&format!("START={cursor}\nEND={end}\n"));
        tags.push_str(&format!("title={}\n", escape_metadata(&chapter.title)));
        cursor = end;
    }
    std::fs::write(&metadata_path, tags)
        .with_context(|| format!("no se pudo escribir {}", metadata_path.display()))?;

    let output_result = Command::new(ffmpeg_command())
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(&list_path)
        .arg("-i")
        .arg(&metadata_path)
        .args([
            "-map",
            "0:a:0",
            "-map_metadata",
            "1",
            "-map_chapters",
            "1",
            "-c:a",
            "aac",
            "-b:a",
            "96k",
            "-ac",
            "1",
            "-ar",
            "24000",
            "-movflags",
            "+faststart",
            "-f",
            "mp4",
        ])
        .arg(output)
        .output()
        .with_context(|| "no se pudo ejecutar ffmpeg")?;

    let _ = std::fs::remove_file(&list_path);
    let _ = std::fs::remove_file(&metadata_path);

    if !output_result.status.success() {
        let stderr = String::from_utf8_lossy(&output_result.stderr);
        bail!("ffmpeg falló al crear el M4B: {}", stderr.trim());
    }
    if !output.is_file() || output.metadata().map(|meta| meta.len()).unwrap_or(0) == 0 {
        bail!("ffmpeg no generó el archivo M4B");
    }
    Ok(())
}

fn escape_concat(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "'\\''")
}

fn escape_metadata(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\\n")
        .replace('=', "\\=")
        .replace(';', "\\;")
        .replace('#', "\\#")
}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    format: FfprobeFormat,
    #[serde(default)]
    streams: Vec<FfprobeStream>,
    #[serde(default)]
    chapters: Vec<FfprobeChapter>,
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeStream {
    #[serde(default)]
    duration: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeFormat {
    #[serde(default)]
    duration: Option<String>,
    #[serde(default)]
    tags: Option<FfprobeTags>,
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeTags {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    album_artist: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    genre: Option<String>,
    #[serde(default)]
    comment: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeChapter {
    #[serde(default)]
    start_time: Option<String>,
    #[serde(default)]
    end_time: Option<String>,
    #[serde(default)]
    tags: Option<FfprobeTags>,
}

pub fn probe(path: &Path) -> Result<ProbeInfo> {
    let output = Command::new(ffprobe())
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            "-show_chapters",
        ])
        .arg(path)
        .output()
        .with_context(|| "no se pudo ejecutar ffprobe; instálalo o define KORA_FFPROBE")?;

    if !output.status.success() {
        bail!(
            "ffprobe no pudo leer {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let parsed: FfprobeOutput =
        serde_json::from_slice(&output.stdout).context("ffprobe devolvió un JSON inesperado")?;

    let tags = parsed.format.tags.unwrap_or_default();
    let duration_seconds = parsed
        .format
        .duration
        .as_deref()
        .and_then(|value| value.parse::<f64>().ok())
        .or_else(|| {
            parsed
                .streams
                .iter()
                .filter_map(|stream| stream.duration.as_deref())
                .filter_map(|value| value.parse::<f64>().ok())
                .reduce(f64::max)
        });
    let mut duration_ms = duration_seconds
        .map(|seconds| (seconds * 1000.0).round() as i64)
        .unwrap_or(0);

    let chapters: Vec<ProbeChapter> = parsed
        .chapters
        .into_iter()
        .map(|chapter| {
            let chapter_tags = chapter.tags.unwrap_or_default();
            ProbeChapter {
                start_ms: parse_seconds(chapter.start_time.as_deref()).unwrap_or(0),
                end_ms: parse_seconds(chapter.end_time.as_deref()).unwrap_or(0),
                title: chapter_tags
                    .title
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "Capítulo".to_string()),
            }
        })
        .filter(|chapter| chapter.end_ms > chapter.start_ms)
        .collect();

    if duration_ms == 0
        && let Some(last) = chapters.last()
    {
        duration_ms = last.end_ms;
    }

    Ok(ProbeInfo {
        duration_ms,
        title: tags.title.filter(|value| !value.trim().is_empty()),
        artist: tags
            .artist
            .or(tags.album_artist)
            .filter(|value| !value.trim().is_empty()),
        language: tags.language.filter(|value| !value.trim().is_empty()),
        genre: tags.genre.filter(|value| !value.trim().is_empty()),
        comment: tags
            .comment
            .or(tags.description)
            .filter(|value| !value.trim().is_empty()),
        chapters,
    })
}

fn parse_seconds(value: Option<&str>) -> Option<i64> {
    value
        .and_then(|value| value.parse::<f64>().ok())
        .map(|seconds| (seconds * 1000.0).round() as i64)
}

/// Busca pausas largas con el filtro `silencedetect` de ffmpeg.
pub fn detect_silences(input: &Path, duration_ms: i64) -> Result<Vec<(i64, i64)>> {
    ensure_available()?;
    let output = Command::new(ffmpeg_command())
        .args(["-hide_banner", "-nostats", "-i"])
        .arg(input)
        .args(["-af", "silencedetect=noise=-32dB:d=2.0", "-f", "null", "-"])
        .output()
        .with_context(|| "no se pudo ejecutar ffmpeg para detectar silencios")?;

    if !output.status.success() {
        bail!(
            "ffmpeg no pudo analizar los silencios: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(parse_silences(
        &String::from_utf8_lossy(&output.stderr),
        duration_ms,
    ))
}

/// Decodifica el audio a PCM mono por tubería y entrega bloques de `i16`.
/// Evita ocupar disco con audiolibros largos y lo comparten el detector de
/// tonos y el generador de waveforms.
pub fn stream_pcm<F>(input: &Path, sample_rate: u32, mut on_samples: F) -> Result<()>
where
    F: FnMut(&[i16]),
{
    ensure_available()?;
    let rate = sample_rate.to_string();
    let mut child = Command::new(ffmpeg_command())
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(input)
        .args(["-ac", "1", "-ar", rate.as_str(), "-f", "s16le", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| "no se pudo iniciar el análisis de audio")?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("ffmpeg no devolvió el flujo de audio"))?;
    let mut bytes = vec![0u8; 64 * 1024];
    let mut pending = Vec::with_capacity(bytes.len() + 1);
    let mut samples = Vec::with_capacity(bytes.len() / 2);

    loop {
        let read = stdout
            .read(&mut bytes)
            .with_context(|| "no se pudo leer el audio decodificado")?;
        if read == 0 {
            break;
        }
        pending.extend_from_slice(&bytes[..read]);
        let usable = pending.len() / 2 * 2;
        samples.clear();
        samples.extend(
            pending[..usable]
                .chunks_exact(2)
                .map(|pair| i16::from_le_bytes([pair[0], pair[1]])),
        );
        on_samples(&samples);
        let _ = pending.drain(..usable);
    }

    let status = child
        .wait()
        .with_context(|| "no se pudo esperar al análisis de audio")?;
    if !status.success() {
        bail!("ffmpeg no pudo analizar el audio");
    }
    Ok(())
}

/// Detecta marcas acústicas de 440 Hz o 1 kHz.
pub fn tone_boundaries(input: &Path) -> Result<Vec<i64>> {
    let mut detector = ToneDetector::new();
    stream_pcm(input, 8_000, |samples| {
        let values: Vec<f32> = samples
            .iter()
            .map(|sample| *sample as f32 / i16::MAX as f32)
            .collect();
        detector.push(&values);
    })?;
    Ok(detector.finish())
}

pub fn compute_waveform(input: &Path, buckets: usize) -> Result<AudioWaveform> {
    let mut builder = WaveformBuilder::new(8_000);
    stream_pcm(input, 8_000, |samples| builder.push(samples))?;
    let duration_ms = builder.duration_ms().max(1);
    Ok(AudioWaveform {
        buckets: builder.finish(buckets),
        duration_ms,
    })
}

/// Primera pasada de `loudnorm`. Devuelve `None` si el audio no se puede medir
/// (por ejemplo, silencio absoluto) para caer a una normalización de una pasada.
pub fn measure_loudness(input: &Path, target_lufs: f32) -> Result<Option<LoudnessMeasurement>> {
    ensure_available()?;
    let filter = format!("loudnorm=I={target_lufs}:TP=-1.5:LRA=11:print_format=json");
    let output = Command::new(ffmpeg_command())
        .args(["-hide_banner", "-nostats", "-i"])
        .arg(input)
        .args(["-af", &filter, "-f", "null", "-"])
        .output()
        .with_context(|| "no se pudo medir el volumen del audiolibro")?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(parse_loudnorm_json(&String::from_utf8_lossy(
        &output.stderr,
    )))
}

/// Aplica normalización LUFS y ecualización. Re-codifica a AAC mono 24 kHz y
/// conserva capítulos y metadatos del archivo original.
pub fn apply_treatment<F>(
    input: &Path,
    output: &Path,
    treatment: AudioTreatment,
    duration_ms: i64,
    mut on_progress: F,
) -> Result<()>
where
    F: FnMut(f64),
{
    ensure_available()?;
    let treatment = treatment.clamped();
    let measurement = if treatment.normalize {
        measure_loudness(input, treatment.target_lufs)?
    } else {
        None
    };
    let chain = treatment
        .filter_chain_with(measurement.as_ref())
        .ok_or_else(|| anyhow::anyhow!("el tratamiento no contiene cambios"))?;
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }

    let mut child = Command::new(ffmpeg_command())
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(input)
        .args([
            "-map",
            "0:a:0",
            "-map_metadata",
            "0",
            "-map_chapters",
            "0",
            "-af",
            chain.as_str(),
            "-c:a",
            "aac",
            "-b:a",
            "96k",
            "-ac",
            "1",
            "-ar",
            "24000",
            "-movflags",
            "+faststart",
            "-f",
            "mp4",
            "-progress",
            "pipe:1",
            "-nostats",
        ])
        .arg(output)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| "no se pudo iniciar el tratamiento de audio")?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("ffmpeg no devolvió el progreso"))?;
    for line in BufReader::new(stdout).lines() {
        let line = line.with_context(|| "no se pudo leer el progreso de ffmpeg")?;
        if let Some(value) = line.strip_prefix("out_time_us=")
            && let Ok(microseconds) = value.trim().parse::<i64>()
        {
            let percent = if duration_ms > 0 {
                (microseconds as f64 / 1000.0 / duration_ms as f64 * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };
            on_progress(percent);
        }
    }

    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }
    let status = child
        .wait()
        .with_context(|| "no se pudo esperar al tratamiento de audio")?;
    if !status.success() {
        bail!("ffmpeg no pudo aplicar el tratamiento: {}", stderr.trim());
    }
    if !output.is_file() || output.metadata().map(|meta| meta.len()).unwrap_or(0) == 0 {
        bail!("ffmpeg no generó el audio tratado");
    }
    on_progress(100.0);
    Ok(())
}

/// Extrae la portada embebida del audio, si existe, como PNG.
/// Devuelve `false` cuando el archivo no tiene pista de vídeo/arte.
pub fn extract_cover(input: &Path, output: &Path) -> Result<bool> {
    ensure_available()?;

    let probe = Command::new(ffprobe())
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(input)
        .output()
        .with_context(|| "no se pudo ejecutar ffprobe")?;
    if !probe.status.success() {
        return Ok(false);
    }
    let has_video = String::from_utf8_lossy(&probe.stdout)
        .lines()
        .any(|line| line.trim() == "video");
    if !has_video {
        return Ok(false);
    }

    let result = Command::new(ffmpeg_command())
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(input)
        .args(["-map", "0:v:0", "-frames:v", "1", "-c:v", "png"])
        .arg(output)
        .output()
        .with_context(|| "no se pudo ejecutar ffmpeg")?;
    if !result.status.success() {
        return Ok(false);
    }

    Ok(output.is_file() && output.metadata().map(|meta| meta.len()).unwrap_or(0) > 0)
}

/// Extrae un tramo a WAV mono 16 kHz para Whisper.
pub fn extract_wav(
    input: &Path,
    output: &Path,
    start: Option<f64>,
    duration: Option<f64>,
) -> Result<()> {
    ensure_available()?;
    let mut command = Command::new(ffmpeg_command());
    command.args(["-hide_banner", "-loglevel", "error", "-y"]);
    if let Some(start) = start {
        command.arg("-ss").arg(format!("{start:.3}"));
    }
    command.arg("-i").arg(input);
    if let Some(duration) = duration {
        command.arg("-t").arg(format!("{duration:.3}"));
    }
    command.args(["-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le"]);
    command.arg(output);

    let result = command
        .output()
        .with_context(|| "no se pudo ejecutar ffmpeg")?;
    if !result.status.success() {
        bail!(
            "ffmpeg no pudo extraer audio: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        );
    }
    if !output.is_file() {
        bail!("ffmpeg no generó el WAV intermedio");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_escaping_keeps_the_format_intact() {
        let escaped = escape_metadata("Título = raro; #1");
        assert!(escaped.contains("\\="));
        assert!(escaped.contains("\\;"));
        assert!(escaped.contains("\\#"));
    }

    #[test]
    fn concat_escaping_wraps_single_quotes() {
        let escaped = escape_concat(Path::new("/tmp/mi'libro.wav"));
        assert!(escaped.contains("'\\''"));
    }
}
