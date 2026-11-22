use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use super::audio::AudioPlayer;
use super::engine::{
    AudioChunk, EngineInfo, SpeakRequest, SynthesisRequest, TtsEngine, VoiceDescriptor,
};
use super::voice_cloner::{
    CLONE_PREFIX, VoiceProfileService, backbone_for, engine_for_language,
    input_format_for_language, strip_clone_prefix,
};
use crate::error::{KoraError, Result};

const ENGINE_ID: &str = "neutts";

#[derive(Debug, Deserialize)]
struct HelperResponse {
    #[allow(dead_code)]
    id: u64,
    ok: bool,
    error: Option<String>,
    duration_ms: Option<u128>,
    frames: Option<usize>,
    sample_rate: Option<u32>,
    #[allow(dead_code)]
    elapsed_ms: Option<u128>,
}

struct Helper {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Helper {
    fn spawn() -> Result<Self> {
        let path = helper_path()?;
        let mut child = Command::new(&path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                KoraError::Unsupported(format!(
                    "no se pudo iniciar el motor de clonación ({}): {error}",
                    path.display()
                ))
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| KoraError::Internal("el motor de clonación no tiene stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| KoraError::Internal("el motor de clonación no tiene stdout".into()))?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 0,
        })
    }

    fn request(&mut self, mut command: Value) -> Result<HelperResponse> {
        self.next_id += 1;
        command["id"] = json!(self.next_id);

        let encoded = serde_json::to_string(&command)?;
        writeln!(self.stdin, "{encoded}")?;
        self.stdin.flush()?;

        let mut line = String::new();
        let read = self.stdout.read_line(&mut line)?;
        if read == 0 {
            return Err(KoraError::Internal(
                "el motor de clonación se cerró inesperadamente".into(),
            ));
        }

        let response: HelperResponse = serde_json::from_str(line.trim())?;
        if !response.ok {
            return Err(KoraError::Internal(response.error.unwrap_or_else(|| {
                "el motor de clonación devolvió un error sin detalle".into()
            })));
        }
        Ok(response)
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "{{\"cmd\":\"shutdown\"}}");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn helper_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("KORA_NEUTTS_HELPER") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(KoraError::Unsupported(format!(
            "KORA_NEUTTS_HELPER apunta a {} y no existe",
            path.display()
        )));
    }

    let name = if cfg!(windows) {
        "kora-neutts.exe"
    } else {
        "kora-neutts"
    };
    let exe = std::env::current_exe().map_err(|error| {
        KoraError::Internal(format!("no se pudo resolver el ejecutable: {error}"))
    })?;

    let mut candidates = vec![exe.with_file_name(name)];

    // `cargo test` vive en target/debug/deps/, mientras que el helper está en
    // target/debug/. También cubre el layout de `tauri dev`.
    let Some(parent) = exe.parent() else {
        return Err(KoraError::Unsupported(
            "no se pudo resolver el directorio de Kora".into(),
        ));
    };
    candidates.push(parent.join(name));
    if let Some(grandparent) = parent.parent() {
        candidates.push(grandparent.join(name));
        // Tauri coloca los sidecars de Linux en /usr/lib/<app>/.
        candidates.push(grandparent.join("lib").join("kora").join(name));
        candidates.push(grandparent.join("lib").join("Kora").join(name));
    }
    if let Ok(appdir) = std::env::var("APPDIR") {
        let appdir = PathBuf::from(appdir);
        candidates.push(appdir.join("usr/lib/kora").join(name));
        candidates.push(appdir.join("usr/bin").join(name));
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err(KoraError::Unsupported(
        "no se encontró el binario `kora-neutts`; compílalo con `cargo build -p kora-neutts`"
            .into(),
    ))
}

#[derive(Debug, Clone)]
pub struct EncodeResult {
    pub duration_ms: i64,
    pub frames: usize,
    pub reference_text: String,
}

pub struct NeuTtsEngine {
    profiles: Arc<VoiceProfileService>,
    owner: String,
    player: Arc<dyn AudioPlayer>,
    helper: Arc<Mutex<Option<Helper>>>,
}

impl NeuTtsEngine {
    pub fn new(
        profiles: Arc<VoiceProfileService>,
        owner: String,
        player: Arc<dyn AudioPlayer>,
    ) -> Self {
        Self {
            profiles,
            owner,
            player,
            helper: Arc::new(Mutex::new(None)),
        }
    }

    pub fn profiles(&self) -> &Arc<VoiceProfileService> {
        &self.profiles
    }

    fn model_paths(&self, language: &str) -> Result<(PathBuf, PathBuf, PathBuf)> {
        if !self.profiles.models_installed(language) {
            let missing = self
                .profiles
                .missing_models(language)
                .iter()
                .map(|model| model.label)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(KoraError::Unsupported(format!(
                "faltan modelos de clonación: {missing}"
            )));
        }

        let backbone = backbone_for(language)?;
        Ok((
            self.profiles.model_path(backbone.file),
            self.profiles.model_path("encoder.onnx"),
            self.profiles.model_path("decoder.onnx"),
        ))
    }

    /// Codifica el clip de referencia y escribe `embedding.bin`,
    /// `reference.wav` y `reference.opus` en `output_dir`.
    pub async fn encode_reference(
        &self,
        audio_path: PathBuf,
        language: String,
        reference_text: String,
        output_dir: PathBuf,
    ) -> Result<EncodeResult> {
        let (_, encoder, decoder) = self.model_paths(&language)?;
        let phonemized = if input_format_for_language(&language) == "phonemes" {
            phonemize(&reference_text, &language)?
        } else {
            reference_text.trim().to_string()
        };

        std::fs::create_dir_all(&output_dir)?;
        let codes_path = output_dir.join("embedding.bin");
        let wav_path = output_dir.join("reference.wav");
        let opus_path = output_dir.join("reference.opus");
        let helper = self.helper.clone();
        let audio = audio_path.clone();

        let response = tokio::task::spawn_blocking(move || -> Result<HelperResponse> {
            let mut slot = helper
                .lock()
                .map_err(|_| KoraError::Internal("motor de clonación bloqueado".into()))?;
            if slot.is_none() {
                *slot = Some(Helper::spawn()?);
            }
            let result = slot.as_mut().expect("helper inicializado").request(json!({
                "cmd": "encode",
                "audio_path": audio,
                "encoder": encoder,
                "decoder": decoder,
                "codes_path": codes_path,
                "out_path": wav_path,
                "opus_path": opus_path,
            }));
            if result.is_err() {
                *slot = None;
            }
            result
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de clonación: {error}")))??;

        Ok(EncodeResult {
            duration_ms: response.duration_ms.unwrap_or(0) as i64,
            frames: response.frames.unwrap_or(0),
            reference_text: phonemized,
        })
    }

    async fn synthesize_inner(&self, voice_id: &str, text: &str) -> Result<AudioChunk> {
        let id = strip_clone_prefix(voice_id)
            .ok_or_else(|| KoraError::InvalidInput("voz clonada no válida".into()))?;
        let profile = self
            .profiles
            .get(id, &self.owner)
            .await?
            .ok_or_else(|| KoraError::NotFound(format!("el perfil {id} no existe")))?;

        let (backbone, encoder, decoder) = self.model_paths(&profile.language)?;
        let codes_path = self.profiles.codes_path(id);
        if !codes_path.is_file() {
            return Err(KoraError::NotFound(
                "el perfil no tiene códigos de referencia".into(),
            ));
        }
        let reference_text = self.profiles.read_reference_text(id)?;

        let input_text = if profile.input_format == "phonemes" {
            phonemize(text, &profile.language)?
        } else {
            text.to_string()
        };

        let output = std::env::temp_dir().join(format!("kora-neutts-{}.wav", uuid::Uuid::new_v4()));
        let output_for_helper = output.clone();
        let helper = self.helper.clone();
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos() as u64)
            .unwrap_or(42);

        let response = tokio::task::spawn_blocking(move || -> Result<HelperResponse> {
            let mut slot = helper
                .lock()
                .map_err(|_| KoraError::Internal("motor de clonación bloqueado".into()))?;
            if slot.is_none() {
                *slot = Some(Helper::spawn()?);
            }
            let result = slot.as_mut().expect("helper inicializado").request(json!({
                "cmd": "speak",
                "backbone": backbone,
                "encoder": encoder,
                "decoder": decoder,
                "codes_path": codes_path,
                "reference_text": reference_text,
                "text": input_text,
                "input_format": profile.input_format,
                "out_path": output_for_helper,
                "seed": seed,
                "temperature": 1.0,
                "top_k": 50,
            }));
            if result.is_err() {
                *slot = None;
            }
            result
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de clonación: {error}")))??;

        let chunk = read_wav(&output, response.sample_rate.unwrap_or(24_000))?;
        let _ = std::fs::remove_file(&output);
        Ok(chunk)
    }
}

fn read_wav(path: &Path, fallback_rate: u32) -> Result<AudioChunk> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|error| KoraError::Internal(format!("WAV clonado inválido: {error}")))?;
    let spec = reader.spec();
    let pcm: Vec<i16> = reader
        .samples::<i16>()
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| KoraError::Internal(format!("WAV clonado inválido: {error}")))?;

    if pcm.is_empty() {
        return Err(KoraError::Internal(
            "el motor de clonación devolvió audio vacío".into(),
        ));
    }

    Ok(AudioChunk {
        pcm,
        sample_rate: if spec.sample_rate == 0 {
            fallback_rate
        } else {
            spec.sample_rate
        },
        channels: spec.channels.max(1),
    })
}

#[cfg(target_os = "android")]
pub fn phonemize(_text: &str, _language: &str) -> Result<String> {
    Err(KoraError::Unsupported(
        "la clonación de voz aún no está disponible en Android".into(),
    ))
}

#[cfg(not(target_os = "android"))]
static ESPEAK_LOCK: Mutex<()> = Mutex::new(());

#[cfg(not(target_os = "android"))]
pub fn phonemize(text: &str, language: &str) -> Result<String> {
    let _guard = ESPEAK_LOCK
        .lock()
        .map_err(|_| KoraError::Internal("fonemizador bloqueado".into()))?;
    let voice = if language == "en" { "en-us" } else { "es" };
    let sentences = espeak_rs::text_to_phonemes(text, voice, None)
        .map_err(|error| KoraError::Internal(format!("espeak-ng: {error:?}")))?;
    let joined = sentences
        .into_iter()
        .map(|sentence| sentence.trim().to_string())
        .filter(|sentence| !sentence.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if joined.is_empty() {
        return Err(KoraError::InvalidInput(
            "no se pudo convertir el texto a fonemas".into(),
        ));
    }
    Ok(joined)
}

#[async_trait]
impl TtsEngine for NeuTtsEngine {
    fn info(&self) -> EngineInfo {
        let available = ["es", "en"]
            .iter()
            .any(|language| self.profiles.models_installed(language));
        EngineInfo {
            id: ENGINE_ID.to_string(),
            name: "NeuTTS (voz clonada)".to_string(),
            offline: true,
            voice_cloning: true,
            streaming: false,
            available,
            supports_pause: true,
        }
    }

    async fn voices(&self) -> Result<Vec<VoiceDescriptor>> {
        Ok(self
            .profiles
            .list(&self.owner)
            .await?
            .into_iter()
            .map(|profile| VoiceDescriptor {
                id: format!("{CLONE_PREFIX}{}", profile.id),
                name: profile.name,
                language: profile.language,
                gender: None,
                engine: ENGINE_ID.to_string(),
                installed: true,
                size_bytes: 0,
                cloned: true,
            })
            .collect())
    }

    async fn synthesize(&self, request: &SynthesisRequest) -> Result<AudioChunk> {
        self.synthesize_inner(&request.voice_id, &request.text)
            .await
    }

    async fn speak(&self, request: &SpeakRequest) -> Result<()> {
        let chunk = self
            .synthesize_inner(&request.voice_id, &request.text)
            .await?;
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

/// Nombre del motor para un idioma, expuesto para la UI de Ajustes.
pub fn engine_label(language: &str) -> Result<&'static str> {
    engine_for_language(language)
}
