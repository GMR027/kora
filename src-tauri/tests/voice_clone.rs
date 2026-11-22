use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

fn first_existing(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().find(|path| path.is_file()).cloned()
}

fn helper_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("KORA_NEUTTS_HELPER") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/debug/kora-neutts");
    candidate.is_file().then_some(candidate)
}

fn request(stdin: &mut ChildStdin, stdout: &mut BufReader<ChildStdout>, value: Value) -> Value {
    writeln!(stdin, "{value}").expect("escribir petición");
    stdin.flush().expect("flush");
    let mut line = String::new();
    stdout.read_line(&mut line).expect("leer respuesta");
    serde_json::from_str(line.trim()).expect("respuesta JSON")
}

fn wav_duration(path: &Path) -> Option<f64> {
    let reader = hound::WavReader::open(path).ok()?;
    let spec = reader.spec();
    if spec.sample_rate == 0 {
        return None;
    }
    Some(reader.len() as f64 / spec.sample_rate as f64 / spec.channels.max(1) as f64)
}

/// Prueba real del pipeline de clonación. Necesita los modelos descargados:
///
/// ```bash
/// KORA_NEUTTS_ROOT=/ruta/a/modelos \
/// KORA_NEUTTS_SAMPLE=/ruta/mateo.wav \
/// cargo test -p kora --test voice_clone
/// ```
#[test]
fn clones_a_known_sample_and_produces_valid_audio() {
    let Some(root) = std::env::var_os("KORA_NEUTTS_ROOT").map(PathBuf::from) else {
        eprintln!("KORA_NEUTTS_ROOT no definido; se omite la prueba de clonación");
        return;
    };

    let Some(backbone) = first_existing(&[
        root.join("neutts-nano-es-q4.gguf"),
        root.join("neutts-nano-spanish-q4.gguf"),
    ]) else {
        eprintln!("no hay backbone español en {}; se omite", root.display());
        return;
    };
    let Some(encoder) = first_existing(&[root.join("encoder.onnx")]) else {
        eprintln!("no hay codificador en {}; se omite", root.display());
        return;
    };
    let Some(decoder) = first_existing(&[root.join("decoder.onnx")]) else {
        eprintln!("no hay decodificador en {}; se omite", root.display());
        return;
    };
    let Some(helper) = helper_path() else {
        eprintln!("no se encontró kora-neutts; se omite");
        return;
    };

    let sample = std::env::var_os("KORA_NEUTTS_SAMPLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("mateo.wav"));
    if !sample.is_file() {
        eprintln!("no hay sample de referencia; se omite");
        return;
    }
    let reference_text = std::fs::read_to_string(root.join("mateo.txt"))
        .unwrap_or_else(|_| {
            "Además su eficiencia depende del clima. En días nublados o durante la noche producen menos energía.".to_string()
        });

    let workspace = tempfile::tempdir().expect("tmp");
    let codes = workspace.path().join("embedding.bin");
    let reference_wav = workspace.path().join("reference.wav");
    let reference_opus = workspace.path().join("reference.opus");
    let output = workspace.path().join("cloned.wav");

    let mut child = Command::new(&helper)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("iniciar kora-neutts");
    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));

    let encoded = request(
        &mut stdin,
        &mut stdout,
        json!({
            "id": 1,
            "cmd": "encode",
            "audio_path": sample.to_string_lossy(),
            "encoder": encoder.to_string_lossy(),
            "decoder": decoder.to_string_lossy(),
            "codes_path": codes.to_string_lossy(),
            "out_path": reference_wav.to_string_lossy(),
            "opus_path": reference_opus.to_string_lossy(),
        }),
    );
    assert_eq!(encoded["ok"], Value::Bool(true), "{encoded}");
    assert!(codes.is_file());
    assert!(reference_wav.is_file());
    assert!(reference_opus.is_file());
    assert_eq!(
        &std::fs::read(&reference_opus).expect("opus")[0..4],
        b"OggS"
    );

    let spoken = request(
        &mut stdin,
        &mut stdout,
        json!({
            "id": 2,
            "cmd": "speak",
            "backbone": backbone.to_string_lossy(),
            "encoder": encoder.to_string_lossy(),
            "decoder": decoder.to_string_lossy(),
            "codes_path": codes.to_string_lossy(),
            "reference_text": reference_text.trim(),
            "text": "Hola, esta es una prueba de clonación de voz con Kora.",
            "input_format": "text",
            "out_path": output.to_string_lossy(),
            "seed": 7,
        }),
    );
    assert_eq!(spoken["ok"], Value::Bool(true), "{spoken}");
    assert!(
        spoken["frames"].as_u64().unwrap_or(0) >= 25,
        "audio demasiado corto: {spoken}"
    );

    let bytes = std::fs::read(&output).expect("wav");
    assert_eq!(&bytes[0..4], b"RIFF");
    let duration = wav_duration(&output).expect("duración");
    assert!(duration > 0.5, "duración inesperada: {duration}");

    let _ = request(&mut stdin, &mut stdout, json!({"id": 3, "cmd": "shutdown"}));
    let _ = child.kill();
    let _ = child.wait();
}
