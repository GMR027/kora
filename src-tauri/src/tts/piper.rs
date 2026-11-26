#![cfg(not(target_os = "android"))]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::audio::{AudioPlayer, RodioPlayer};
use super::engine::{
    AudioChunk, EngineInfo, SpeakRequest, SynthesisRequest, TtsEngine, VoiceDescriptor,
};
use crate::error::{KoraError, Result};

/// Catálogo curado de voces Piper para los dos idiomas de Kora.
#[derive(Debug, Clone)]
pub struct VoiceSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub language: &'static str,
    pub gender: Option<&'static str>,
    pub model_url: &'static str,
    pub config_url: &'static str,
    pub size_bytes: u64,
}

pub const CATALOG: &[VoiceSpec] = &[
    VoiceSpec {
        id: "es_ES-davefx-medium",
        name: "Español (DaveFX)",
        language: "es",
        gender: Some("male"),
        model_url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/es/es_ES/davefx/medium/es_ES-davefx-medium.onnx",
        config_url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/es/es_ES/davefx/medium/es_ES-davefx-medium.onnx.json",
        size_bytes: 63_221_984,
    },
    VoiceSpec {
        id: "es_MX-ald-medium",
        name: "Español México (Ald)",
        language: "es",
        gender: Some("male"),
        model_url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/es/es_MX/ald/medium/es_MX-ald-medium.onnx",
        config_url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/es/es_MX/ald/medium/es_MX-ald-medium.onnx.json",
        size_bytes: 63_221_984,
    },
    VoiceSpec {
        id: "en_US-lessac-medium",
        name: "English (Lessac)",
        language: "en",
        gender: Some("female"),
        model_url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/lessac/medium/en_US-lessac-medium.onnx",
        config_url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/lessac/medium/en_US-lessac-medium.onnx.json",
        size_bytes: 63_201_294,
    },
    VoiceSpec {
        id: "en_US-ryan-medium",
        name: "English (Ryan)",
        language: "en",
        gender: Some("male"),
        model_url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ryan/medium/en_US-ryan-medium.onnx",
        config_url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ryan/medium/en_US-ryan-medium.onnx.json",
        size_bytes: 63_221_984,
    },
];

pub fn find_voice(id: &str) -> Option<&'static VoiceSpec> {
    CATALOG.iter().find(|voice| voice.id == id)
}

pub fn voice_dir(root: &Path, id: &str) -> PathBuf {
    root.join(id)
}

pub fn installed(root: &Path, id: &str) -> bool {
    let directory = voice_dir(root, id);
    directory.join("model.onnx").is_file() && directory.join("config.json").is_file()
}

pub struct PiperEngine {
    root: PathBuf,
    cache: Arc<Mutex<HashMap<String, piper_rs::Piper>>>,
    player: Arc<dyn AudioPlayer>,
}

impl PiperEngine {
    pub fn new(root: PathBuf) -> Self {
        Self::with_player(root, Arc::new(RodioPlayer::default()))
    }

    pub fn with_player(root: PathBuf, player: Arc<dyn AudioPlayer>) -> Self {
        ensure_espeak_data();
        Self {
            root,
            cache: Arc::new(Mutex::new(HashMap::new())),
            player,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[async_trait]
impl TtsEngine for PiperEngine {
    fn info(&self) -> EngineInfo {
        EngineInfo {
            id: "piper".to_string(),
            name: "Piper (offline)".to_string(),
            offline: true,
            voice_cloning: false,
            streaming: false,
            available: true,
            supports_pause: true,
            prefetch: false,
        }
    }

    async fn voices(&self) -> Result<Vec<VoiceDescriptor>> {
        Ok(CATALOG
            .iter()
            .map(|voice| VoiceDescriptor {
                id: voice.id.to_string(),
                name: voice.name.to_string(),
                language: voice.language.to_string(),
                gender: voice.gender.map(str::to_string),
                engine: "piper".to_string(),
                installed: installed(&self.root, voice.id),
                size_bytes: voice.size_bytes,
                cloned: false,
                enhanced: false,
                quality: None,
                latency: None,
                requires_network: false,
                locale: None,
            })
            .collect())
    }

    async fn synthesize(&self, request: &SynthesisRequest) -> Result<AudioChunk> {
        let text = request.text.trim().to_string();
        if text.is_empty() {
            return Err(KoraError::InvalidInput(
                "no hay texto para sintetizar".into(),
            ));
        }

        let voice_id = resolve_voice(&self.root, &request.voice_id)?;
        let root = self.root.clone();
        let cache = self.cache.clone();
        let rate = request.rate.clamp(0.5, 3.0);
        let pitch = request.pitch.clamp(0.5, 1.5);

        let chunk = tokio::task::spawn_blocking(move || {
            synthesize_blocking(&root, &cache, &voice_id, &text, rate, pitch, 1.0)
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de TTS: {error}")))??;

        Ok(resample_pitch(chunk, pitch))
    }

    async fn speak(&self, request: &SpeakRequest) -> Result<()> {
        let text = request.text.trim().to_string();
        if text.is_empty() {
            return Ok(());
        }

        let voice_id = resolve_voice(&self.root, &request.voice_id)?;
        let root = self.root.clone();
        let cache = self.cache.clone();
        let rate = request.rate.clamp(0.5, 3.0);
        let pitch = request.pitch.clamp(0.5, 1.5);
        let emphasis = request.emphasis.clamp(0.5, 2.0);

        let chunk = tokio::task::spawn_blocking(move || {
            synthesize_blocking(&root, &cache, &voice_id, &text, rate, pitch, emphasis)
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de TTS: {error}")))??;

        let chunk = resample_pitch(chunk, pitch);
        self.player.play(chunk).await
    }

    async fn pause(&self) -> Result<()> {
        self.player.pause().await
    }

    async fn resume(&self) -> Result<()> {
        self.player.resume().await
    }

    async fn stop(&self) -> Result<()> {
        self.player.stop().await
    }
}

fn resolve_voice(root: &Path, voice_id: &str) -> Result<String> {
    let voice_id = if voice_id.trim().is_empty() {
        default_voice(root).ok_or_else(|| {
            KoraError::Unsupported(
                "no hay ninguna voz Piper instalada; descarga una voz en Ajustes".into(),
            )
        })?
    } else {
        voice_id.to_string()
    };

    if !installed(root, &voice_id) {
        return Err(KoraError::NotFound(format!(
            "la voz {voice_id} no está instalada"
        )));
    }

    Ok(voice_id)
}

#[allow(clippy::too_many_arguments)]
fn synthesize_blocking(
    root: &Path,
    cache: &Arc<Mutex<HashMap<String, piper_rs::Piper>>>,
    voice_id: &str,
    text: &str,
    rate: f32,
    pitch: f32,
    emphasis: f32,
) -> Result<AudioChunk> {
    let mut cache = cache
        .lock()
        .map_err(|_| KoraError::Internal("caché de voces envenenada".into()))?;

    if !cache.contains_key(voice_id) {
        let directory = voice_dir(root, voice_id);
        let model = directory.join("model.onnx");
        let config = directory.join("config.json");

        let piper = piper_rs::Piper::new(&model, &config).map_err(|error| {
            KoraError::Internal(format!("no se pudo cargar la voz {voice_id}: {error}"))
        })?;

        cache.insert(voice_id.to_string(), piper);
    }

    let piper = cache
        .get_mut(voice_id)
        .ok_or_else(|| KoraError::Internal("voz Piper no encontrada en caché".into()))?;

    // La síntesis se ralentiza por el pitch para que el remuestreo posterior
    // suba el tono sin cambiar la duración final.
    let length_scale = (1.0 / rate) * pitch;
    let noise_scale = (0.667 * emphasis).clamp(0.35, 1.2);
    let noise_w = (0.8 * emphasis).clamp(0.4, 1.4);

    let mut pcm = Vec::new();
    let mut sample_rate = 22_050u32;

    for chunk in split_text(text) {
        let (samples, rate) = piper
            .create(
                &chunk,
                false,
                None,
                Some(length_scale),
                Some(noise_scale),
                Some(noise_w),
            )
            .map_err(|error| KoraError::Internal(format!("Piper no pudo sintetizar: {error}")))?;

        sample_rate = rate;
        pcm.extend(
            samples
                .into_iter()
                .map(|sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16),
        );
    }

    if pcm.is_empty() {
        return Err(KoraError::Internal(
            "Piper devolvió audio vacío".to_string(),
        ));
    }

    Ok(AudioChunk {
        pcm,
        sample_rate,
        channels: 1,
    })
}

/// Cambia el tono remuestreando. La duración final se conserva porque la
/// síntesis previa se compensó con `length_scale * pitch`.
pub fn resample_pitch(chunk: AudioChunk, pitch: f32) -> AudioChunk {
    if (pitch - 1.0).abs() < 0.01 || chunk.pcm.len() < 2 {
        return chunk;
    }

    let pitch = pitch.clamp(0.5, 1.5);
    let output_len = ((chunk.pcm.len() as f32) / pitch).round() as usize;
    let mut pcm = Vec::with_capacity(output_len.max(1));

    for index in 0..output_len.max(1) {
        let source = index as f32 * pitch;
        let left = source.floor() as usize;
        let right = (left + 1).min(chunk.pcm.len() - 1);
        let fraction = source - left as f32;
        let a = chunk.pcm[left.min(chunk.pcm.len() - 1)] as f32;
        let b = chunk.pcm[right] as f32;
        pcm.push((a + (b - a) * fraction).round() as i16);
    }

    AudioChunk { pcm, ..chunk }
}

/// Divide el texto en fragmentos de frase para que el modelo no reciba
/// secuencias enormes y la reproducción empiece antes.
fn split_text(text: &str) -> Vec<String> {
    const MAX_CHARS: usize = 360;
    let mut chunks = Vec::new();
    let mut current = String::new();

    for sentence in text.split_inclusive(['.', '!', '?', '\n']) {
        let sentence = sentence.trim();
        if sentence.is_empty() {
            continue;
        }

        if current.chars().count() + sentence.chars().count() > MAX_CHARS && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }

        if sentence.chars().count() > MAX_CHARS {
            for word in sentence.split_whitespace() {
                if current.chars().count() + word.chars().count() + 1 > MAX_CHARS
                    && !current.is_empty()
                {
                    chunks.push(std::mem::take(&mut current));
                }
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(word);
            }
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(sentence);
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    if chunks.is_empty() {
        chunks.push(text.to_string());
    }

    chunks
}

/// `espeak-rs` solo busca `espeak-ng-data` en el directorio actual y junto al
/// ejecutable. En Linux el paquete del sistema lo instala en `/usr/share`, y el
/// binario compilado puede llevar una ruta de build que no existe en el equipo
/// del usuario. Detectamos las ubicaciones habituales antes de inicializar.
fn ensure_espeak_data() {
    if let Ok(current) = std::env::var("PIPER_ESPEAKNG_DATA_DIRECTORY")
        && Path::new(&current).join("espeak-ng-data").is_dir()
    {
        return;
    }

    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.to_path_buf());
        candidates.push(dir.join("share"));
        if let Some(parent) = dir.parent() {
            candidates.push(parent.join("share"));
            candidates.push(parent.join("lib").join("kora"));
            candidates.push(parent.join("lib").join("Kora"));
        }
    }
    candidates.extend(
        [
            "/usr/share",
            "/usr/local/share",
            "/opt/homebrew/share",
            "/opt/local/share",
        ]
        .into_iter()
        .map(PathBuf::from),
    );

    if let Some(directory) = candidates
        .into_iter()
        .find(|directory| directory.join("espeak-ng-data").is_dir())
    {
        // SAFETY: se llama al construir el motor, antes de que eSpeak-ng
        // inicialice su OnceLock y antes de que la app use hilos de síntesis.
        unsafe {
            std::env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", directory);
        }
    }
}

pub fn default_voice(root: &Path) -> Option<String> {
    CATALOG
        .iter()
        .find(|voice| installed(root, voice.id))
        .map(|voice| voice.id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_long_text_into_sentences() {
        let text = "Primera frase. Segunda frase. Tercera frase.";
        let chunks = split_text(text);
        assert!(!chunks.is_empty());
        assert_eq!(chunks.join(" "), text);
    }

    #[test]
    fn catalog_has_two_voices_per_language() {
        assert_eq!(
            CATALOG
                .iter()
                .filter(|voice| voice.language == "es")
                .count(),
            2
        );
        assert_eq!(
            CATALOG
                .iter()
                .filter(|voice| voice.language == "en")
                .count(),
            2
        );
    }

    /// Prueba manual de humo: sintetiza una frase con la voz real instalada.
    /// Se ejecuta solo cuando KORA_TEST_TTS_ROOT apunta a assets/tts.
    #[tokio::test]
    #[ignore]
    async fn synthesizes_installed_voice_from_env() {
        let Ok(root) = std::env::var("KORA_TEST_TTS_ROOT") else {
            return;
        };
        let engine = PiperEngine::new(PathBuf::from(root));
        let chunk = engine
            .synthesize(&SynthesisRequest {
                text: "Kora puede leer este capítulo en voz alta.".to_string(),
                voice_id: "es_ES-davefx-medium".to_string(),
                rate: 1.0,
                pitch: 1.0,
            })
            .await
            .expect("sintetizar voz Piper");
        eprintln!(
            "TTS OK: {} muestras a {} Hz, {} canales",
            chunk.pcm.len(),
            chunk.sample_rate,
            chunk.channels
        );
        assert!(chunk.pcm.len() > 10_000);
    }

    #[test]
    fn pitch_resampling_keeps_the_duration() {
        let chunk = AudioChunk {
            pcm: vec![0; 1000],
            sample_rate: 22_050,
            channels: 1,
        };
        let shifted = resample_pitch(chunk, 1.1);
        assert!((shifted.pcm.len() as f32 - 909.0).abs() < 2.0);
    }
}
