//! Sonda manual para comprobar el motor Supertonic sin abrir la aplicación.
//!
//! Uso:
//! `cargo run --example supertonic_probe -- <directorio-del-modelo> [texto] [voz] [salida.wav]`

use std::sync::Arc;

use kora_lib::tts::{
    NullAudioPlayer, SupertonicEngine, SupertonicService, SynthesisRequest, TtsEngine, encode_wav,
};

#[tokio::main]
async fn main() {
    let root = std::env::args()
        .nth(1)
        .expect("indica el directorio con onnx/ y voice_styles/");
    let text = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "Hola, esta es una prueba de las voces mejoradas de Kora.".into());
    let voice = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "supertonic:es:F1".into());

    let service = Arc::new(SupertonicService::new(
        root.into(),
        Arc::new(NullAudioPlayer),
    ));
    let engine = SupertonicEngine::new(service);
    let started = std::time::Instant::now();
    let chunk = engine
        .synthesize(&SynthesisRequest {
            text,
            voice_id: voice,
            rate: 1.0,
            pitch: 1.0,
        })
        .await
        .expect("sintetizar");
    let seconds = chunk.pcm.len() as f32 / chunk.sample_rate.max(1) as f32;
    println!(
        "audio: {:.2}s a {} Hz en {:.2}s",
        seconds,
        chunk.sample_rate,
        started.elapsed().as_secs_f32()
    );
    let output = std::env::args()
        .nth(4)
        .unwrap_or_else(|| "supertonic-test.wav".to_string());
    let wav = encode_wav(&chunk).expect("WAV");
    std::fs::write(&output, wav).expect("escribir WAV");
    println!("WAV: {output}");
}
