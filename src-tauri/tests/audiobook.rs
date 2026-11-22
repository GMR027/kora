use std::path::Path;

use kora_lib::audiobooks::audio;
use kora_lib::audiobooks::m4b::{self, MuxChapter};

fn write_tone(path: &Path, seconds: f32) {
    let total = (audio::OUTPUT_RATE as f32 * seconds) as usize;
    let mut samples = Vec::with_capacity(total);
    for index in 0..total {
        let value =
            (index as f32 * 440.0 * std::f32::consts::TAU / audio::OUTPUT_RATE as f32).sin();
        samples.push((value * 0.2 * i16::MAX as f32) as i16);
    }
    audio::write_wav(path, &samples).expect("wav");
}

fn write_marker_audio(path: &Path) {
    let rate = audio::OUTPUT_RATE as f32;
    let mut samples = Vec::new();
    samples.extend(std::iter::repeat_n(0i16, (rate * 1.5) as usize));
    let tone_samples = (rate * 0.5) as usize;
    for index in 0..tone_samples {
        let value = (index as f32 * 1_000.0 * std::f32::consts::TAU / rate).sin();
        samples.push((value * 0.4 * i16::MAX as f32) as i16);
    }
    samples.extend(std::iter::repeat_n(0i16, (rate * 3.0) as usize));
    audio::write_wav(path, &samples).expect("wav");
}

#[test]
fn detects_silences_and_tone_markers_with_ffmpeg() {
    if m4b::ensure_available().is_err() {
        eprintln!("ffmpeg no disponible; se omiten las pruebas de detección");
        return;
    }

    let workspace = tempfile::tempdir().expect("tmp");
    let source = workspace.path().join("markers.wav");
    write_marker_audio(&source);
    let duration_ms = 5_000;

    let silences = m4b::detect_silences(&source, duration_ms).expect("silencios");
    assert!(
        silences
            .iter()
            .any(|(start, end)| { *start >= 1_800 && *start <= 2_500 && *end >= 4_500 }),
        "silencios inesperados: {silences:?}"
    );

    let tones = m4b::tone_boundaries(&source).expect("tonos");
    assert!(
        tones
            .iter()
            .any(|boundary| *boundary >= 1_400 && *boundary <= 2_300),
        "tonos inesperados: {tones:?}"
    );
}

#[test]
fn normalizes_loudness_and_builds_a_waveform_with_ffmpeg() {
    if m4b::ensure_available().is_err() {
        eprintln!("ffmpeg no disponible; se omite la prueba de tratamiento");
        return;
    }

    let workspace = tempfile::tempdir().expect("tmp");
    let source = workspace.path().join("markers.wav");
    let output = workspace.path().join("treated.m4b");
    write_marker_audio(&source);

    let treatment = kora_lib::audiobooks::AudioTreatment {
        normalize: true,
        target_lufs: -18.0,
        eq: kora_lib::audiobooks::EqPreset::ClearVoice,
    };
    m4b::apply_treatment(&source, &output, treatment, 5_000, |_| {}).expect("tratamiento");

    let probe = m4b::probe(&output).expect("probe");
    assert!(
        (probe.duration_ms - 5_000).abs() < 500,
        "duración inesperada: {}",
        probe.duration_ms
    );

    let measured = m4b::measure_loudness(&output, -18.0)
        .expect("medir")
        .expect("medidas");
    assert!(
        (measured.input_i + 18.0).abs() < 4.0,
        "LUFS inesperado: {}",
        measured.input_i
    );

    let waveform = m4b::compute_waveform(&output, 100).expect("waveform");
    assert_eq!(waveform.buckets.len(), 100);
    assert!(waveform.buckets.iter().any(|peak| *peak > 0.5));
}

#[test]
fn muxes_a_two_chapter_m4b_with_ffmpeg() {
    if m4b::ensure_available().is_err() {
        eprintln!("ffmpeg no disponible; se omite la prueba de M4B");
        return;
    }

    let workspace = tempfile::tempdir().expect("tmp");
    let first = workspace.path().join("one.wav");
    let second = workspace.path().join("two.wav");
    write_tone(&first, 1.0);
    write_tone(&second, 2.0);

    let output = workspace.path().join("book.m4b");
    let chapters = vec![
        MuxChapter {
            title: "Primero".to_string(),
            path: first,
            duration_ms: 1000,
        },
        MuxChapter {
            title: "Segundo".to_string(),
            path: second,
            duration_ms: 2000,
        },
    ];

    m4b::mux(&chapters, &output, "Prueba", "Kora", &[("book_id", "test")]).expect("mux");

    let probe = m4b::probe(&output).expect("probe");
    assert!(
        (probe.duration_ms - 3000).abs() < 250,
        "duración inesperada: {}",
        probe.duration_ms
    );
    assert_eq!(probe.chapters.len(), 2);
    assert_eq!(probe.chapters[0].title, "Primero");
    assert_eq!(probe.chapters[1].title, "Segundo");
    assert!(probe.chapters[1].start_ms >= 900);

    // El mismo archivo debe poder abrirse con Symphonia, que es lo que usa el
    // reproductor de audiolibros.
    let file = std::fs::File::open(&output).expect("abrir m4b");
    let decoder = rodio::Decoder::new(std::io::BufReader::new(file)).expect("decodificar m4b");
    let decoded = decoder.take(2_000).count();
    assert!(decoded > 0, "el decodificador no devolvió muestras");
}

#[test]
fn plain_m4b_has_no_embedded_cover() {
    if m4b::ensure_available().is_err() {
        eprintln!("ffmpeg no disponible; se omite la prueba de portada");
        return;
    }

    let workspace = tempfile::tempdir().expect("tmp");
    let tone = workspace.path().join("tone.wav");
    write_tone(&tone, 1.0);

    let output = workspace.path().join("book.m4b");
    let chapters = vec![MuxChapter {
        title: "Único".to_string(),
        path: tone,
        duration_ms: 1000,
    }];
    m4b::mux(&chapters, &output, "Prueba", "Kora", &[]).expect("mux");

    let cover = workspace.path().join("cover.png");
    let extracted = m4b::extract_cover(&output, &cover).expect("portada");
    assert!(!extracted);
    assert!(!cover.exists());
}

#[tokio::test]
async fn builds_an_m4b_from_piper_audio() {
    let Ok(root) = std::env::var("KORA_TTS_ROOT") else {
        eprintln!("KORA_TTS_ROOT no definido; se omite la prueba con Piper");
        return;
    };
    if m4b::ensure_available().is_err() {
        eprintln!("ffmpeg no disponible; se omite la prueba con Piper");
        return;
    }

    use kora_lib::tts::{PiperEngine, SynthesisRequest, TtsEngine};

    let engine = PiperEngine::new(std::path::PathBuf::from(root));
    let voices = engine.voices().await.expect("voces");
    let voice = voices
        .iter()
        .find(|voice| voice.installed)
        .expect("voz instalada");

    let workspace = tempfile::tempdir().expect("tmp");
    let mut chapters = Vec::new();
    for (index, text) in ["Primer capítulo de prueba.", "Segundo capítulo."]
        .iter()
        .enumerate()
    {
        let chunk = engine
            .synthesize(&SynthesisRequest {
                text: (*text).to_string(),
                voice_id: voice.id.clone(),
                rate: 1.0,
                pitch: 1.0,
            })
            .await
            .expect("sintetizar");
        let samples = audio::chunk_to_samples(&chunk);
        let path = workspace.path().join(format!("chapter-{index}.wav"));
        audio::write_wav(&path, &samples).expect("wav");
        chapters.push(MuxChapter {
            title: format!("Capítulo {}", index + 1),
            path,
            duration_ms: audio::duration_ms(samples.len()),
        });
    }

    let output = workspace.path().join("piper.m4b");
    m4b::mux(&chapters, &output, "Piper", "Kora", &[]).expect("mux");
    let probe = m4b::probe(&output).expect("probe");
    assert!(probe.duration_ms > 500);
    assert_eq!(probe.chapters.len(), 2);
}
