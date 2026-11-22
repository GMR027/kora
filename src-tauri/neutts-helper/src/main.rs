use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

mod audio;
mod codec;
mod engine;

use engine::NeuttsEngine;

#[derive(Debug, Deserialize)]
struct Request {
    #[serde(default)]
    id: u64,
    cmd: String,
    #[serde(default)]
    backbone: Option<String>,
    #[serde(default)]
    encoder: Option<String>,
    #[serde(default)]
    decoder: Option<String>,
    #[serde(default)]
    audio_path: Option<String>,
    #[serde(default)]
    codes_path: Option<String>,
    #[serde(default)]
    out_path: Option<String>,
    #[serde(default)]
    opus_path: Option<String>,
    #[serde(default)]
    reference_text: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    input_format: Option<String>,
    #[serde(default)]
    seed: Option<u64>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    top_k: Option<usize>,
}

#[derive(Debug, Serialize)]
struct Response {
    id: u64,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frames: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elapsed_ms: Option<u128>,
}

impl Response {
    fn ok(id: u64) -> Self {
        Self {
            id,
            ok: true,
            error: None,
            duration_ms: None,
            frames: None,
            sample_rate: None,
            elapsed_ms: None,
        }
    }

    fn error(id: u64, error: impl ToString) -> Self {
        Self {
            id,
            ok: false,
            error: Some(error.to_string()),
            duration_ms: None,
            frames: None,
            sample_rate: None,
            elapsed_ms: None,
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("kora-neutts: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|value| value == "--selftest") {
        return selftest(&args[1..]);
    }

    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout());
    let mut engine = NeuttsEngine::new()?;

    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(line) {
            Ok(request) => request,
            Err(error) => {
                let response = Response::error(0, format!("petición inválida: {error}"));
                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                stdout.flush()?;
                continue;
            }
        };

        if request.cmd == "shutdown" {
            let response = Response::ok(request.id);
            writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
            stdout.flush()?;
            break;
        }

        let response = handle(&mut engine, request);
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }

    Ok(())
}

fn handle(engine: &mut NeuttsEngine, request: Request) -> Response {
    let id = request.id;
    match request.cmd.as_str() {
        "ping" => Response::ok(id),
        "encode" => encode(engine, &request)
            .unwrap_or_else(|error| Response::error(id, format!("{error:#}"))),
        "speak" => speak(engine, &request)
            .unwrap_or_else(|error| Response::error(id, format!("{error:#}"))),
        other => Response::error(id, format!("comando desconocido: {other}")),
    }
}

fn encode(engine: &mut NeuttsEngine, request: &Request) -> Result<Response> {
    let audio_path = required(&request.audio_path, "audio_path")?;
    let encoder = required(&request.encoder, "encoder")?;
    let decoder = required(&request.decoder, "decoder")?;
    let codes_path = required(&request.codes_path, "codes_path")?;
    let out_path = required(&request.out_path, "out_path")?;

    let pcm = audio::prepare_reference(Path::new(audio_path))?;
    let duration_ms = pcm.len() as u128 * 1000 / 16_000;
    let codes = engine.encode_reference(Path::new(encoder), Path::new(decoder), &pcm)?;

    write_codes(Path::new(codes_path), &codes)?;
    audio::write_wav(Path::new(out_path), &pcm, 16_000)?;
    if let Some(opus_path) = request
        .opus_path
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        audio::write_opus(Path::new(opus_path), &pcm, 16_000)?;
    }

    let mut response = Response::ok(request.id);
    response.duration_ms = Some(duration_ms);
    response.frames = Some(codes.len());
    response.sample_rate = Some(16_000);
    Ok(response)
}

fn speak(engine: &mut NeuttsEngine, request: &Request) -> Result<Response> {
    let backbone = required(&request.backbone, "backbone")?;
    let encoder = required(&request.encoder, "encoder")?;
    let decoder = required(&request.decoder, "decoder")?;
    let codes_path = required(&request.codes_path, "codes_path")?;
    let out_path = required(&request.out_path, "out_path")?;
    let text = required(&request.text, "text")?;
    let reference_text = request.reference_text.as_deref().unwrap_or("");
    let input_format = request.input_format.as_deref().unwrap_or("phonemes");

    let codes = read_codes(Path::new(codes_path))?;
    let synthesis = engine.speak(
        Path::new(backbone),
        Path::new(encoder),
        Path::new(decoder),
        &codes,
        reference_text,
        text,
        input_format,
        request.seed.unwrap_or(42),
        request.temperature.unwrap_or(1.0),
        request.top_k.unwrap_or(50),
    )?;

    audio::write_wav(
        Path::new(out_path),
        &synthesis.samples,
        synthesis.sample_rate,
    )?;

    let mut response = Response::ok(request.id);
    response.duration_ms =
        Some(synthesis.samples.len() as u128 * 1000 / synthesis.sample_rate.max(1) as u128);
    response.frames = Some(synthesis.frames);
    response.sample_rate = Some(synthesis.sample_rate);
    response.elapsed_ms = Some(synthesis.elapsed_ms);
    Ok(response)
}

fn required<'a>(value: &'a Option<String>, name: &str) -> Result<&'a str> {
    value
        .as_deref()
        .filter(|value| !value.is_empty())
        .with_context(|| format!("falta {name}"))
}

fn read_codes(path: &Path) -> Result<Vec<i32>> {
    let bytes =
        std::fs::read(path).with_context(|| format!("no se pudo leer {}", path.display()))?;
    if bytes.len() % 4 != 0 {
        bail!("el archivo de códigos tiene un tamaño inválido");
    }
    let (chunks, _) = bytes.as_chunks::<4>();
    Ok(chunks
        .iter()
        .map(|chunk| i32::from_le_bytes(*chunk))
        .collect())
}

fn write_codes(path: &Path, codes: &[i32]) -> Result<()> {
    let mut bytes = Vec::with_capacity(codes.len() * 4);
    for code in codes {
        bytes.extend_from_slice(&code.to_le_bytes());
    }
    std::fs::write(path, bytes)
        .with_context(|| format!("no se pudo escribir {}", path.display()))?;
    Ok(())
}

/// Prueba de extremo a extremo desde la línea de comandos:
/// `kora-neutts --selftest <backbone> <encoder> <decoder> <ref.wav> <ref.txt> <formato> <texto> <salida.wav>`
fn selftest(args: &[String]) -> Result<()> {
    if args.len() < 8 {
        bail!(
            "uso: kora-neutts --selftest <backbone> <encoder> <decoder> <ref.wav> <ref.txt> <input_format> <texto> <salida.wav>"
        );
    }

    let backbone = PathBuf::from(&args[0]);
    let encoder = PathBuf::from(&args[1]);
    let decoder = PathBuf::from(&args[2]);
    let reference_audio = PathBuf::from(&args[3]);
    let reference_text = std::fs::read_to_string(&args[4])?;
    let input_format = &args[5];
    let text = &args[6];
    let output = PathBuf::from(&args[7]);

    let mut engine = NeuttsEngine::new()?;
    let started = std::time::Instant::now();
    let pcm = audio::prepare_reference(&reference_audio)?;
    println!("referencia: {:.2} s a 16 kHz", pcm.len() as f32 / 16_000.0);
    let codes = engine.encode_reference(&encoder, &decoder, &pcm)?;
    println!(
        "códigos: {} ({:.0} Hz)",
        codes.len(),
        codes.len() as f32 / (pcm.len() as f32 / 16_000.0)
    );

    let synthesis = engine.speak(
        &backbone,
        &encoder,
        &decoder,
        &codes,
        reference_text.trim(),
        text,
        input_format,
        42,
        1.0,
        50,
    )?;
    audio::write_wav(&output, &synthesis.samples, synthesis.sample_rate)?;
    println!(
        "audio: {:.2} s a {} Hz ({} frames, {:.1} s de reloj)",
        synthesis.samples.len() as f32 / synthesis.sample_rate as f32,
        synthesis.sample_rate,
        synthesis.frames,
        started.elapsed().as_secs_f32()
    );
    println!("guardado en {}", output.display());

    Ok(())
}
