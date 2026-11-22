use std::path::PathBuf;

use std::sync::{Arc, Mutex};

/// Piper/ONNX no tolera crear dos modelos a la vez en el mismo proceso.
static TTS_LOCK: Mutex<()> = Mutex::new(());

use kora_lib::tts::{NullAudioPlayer, PiperEngine, SpeakRequest, SynthesisRequest, TtsEngine};

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn synthesizes_speech_with_an_installed_voice() {
    let _guard = TTS_LOCK.lock().unwrap();
    let Ok(root) = std::env::var("KORA_TTS_ROOT") else {
        eprintln!("KORA_TTS_ROOT no definido; se omite la prueba de Piper");
        return;
    };

    let engine = PiperEngine::new(PathBuf::from(root));
    let voices = engine.voices().await.expect("catálogo de voces");
    let voice = voices
        .iter()
        .find(|voice| voice.installed)
        .expect("debe haber una voz instalada");

    let chunk = engine
        .synthesize(&SynthesisRequest {
            text: "Hola, esto es Kora leyendo un libro.".to_string(),
            voice_id: voice.id.clone(),
            rate: 1.0,
            pitch: 1.0,
        })
        .await
        .expect("sintetizar");

    assert!(!chunk.pcm.is_empty());
    assert!(chunk.sample_rate >= 8_000);
    assert_eq!(chunk.channels, 1);

    let wav = kora_lib::tts::encode_wav(&chunk).expect("wav");
    assert_eq!(&wav[0..4], b"RIFF");
    assert!(wav.len() > 44);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn speaks_with_the_same_pipeline_without_an_audio_device() {
    let _guard = TTS_LOCK.lock().unwrap();
    let Ok(root) = std::env::var("KORA_TTS_ROOT") else {
        eprintln!("KORA_TTS_ROOT no definido; se omite la prueba de Piper");
        return;
    };

    let engine = PiperEngine::with_player(PathBuf::from(root), Arc::new(NullAudioPlayer));
    let voices = engine.voices().await.expect("catálogo de voces");
    let voice = voices
        .iter()
        .find(|voice| voice.installed)
        .expect("debe haber una voz instalada");

    engine
        .speak(&SpeakRequest {
            text: "Hola, esto es una prueba de la sesión.".to_string(),
            voice_id: voice.id.clone(),
            language: Some("es".to_string()),
            rate: 1.2,
            pitch: 1.05,
            volume: 1.0,
            emphasis: 1.0,
        })
        .await
        .expect("hablar");

    engine.pause().await.expect("pausar");
    engine.resume().await.expect("reanudar");
    engine.stop().await.expect("detener");
}
