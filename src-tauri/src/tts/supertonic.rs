//! Supertonic 3: pack opcional de voces neuronales de alta calidad.
//!
//! El modelo se descarga desde Hugging Face y se ejecuta con ONNX Runtime en
//! CPU. No forma parte del APK ni del paquete de escritorio: el usuario lo
//! activa desde Ajustes. Las sesiones se cargan de forma perezosa y se liberan
//! tras un periodo de inactividad para no acercarse al límite de memoria del
//! teléfono.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use ndarray::{Array, Array3};
use ort::session::Session;
use ort::value::Tensor;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::audio::AudioPlayer;
use super::engine::{
    AudioChunk, EngineInfo, SpeakRequest, SynthesisRequest, TtsEngine, VoiceDescriptor,
};
use crate::error::{KoraError, Result};

pub const ENGINE_ID: &str = "supertonic";
/// Prefijo de los identificadores: `supertonic:<idioma>:<estilo>`.
pub const PREFIX: &str = "supertonic:";
pub const LICENSE_NAME: &str = "OpenRAIL-M";

const BASE_URL: &str = "https://huggingface.co/Supertone/supertonic-3/resolve/main";
const IDLE_UNLOAD: Duration = Duration::from_secs(90);
const MAX_DOWNLOAD_BYTES: u64 = 600 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct ModelFile {
    pub path: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

/// Archivos del modelo. Los ONNX y los JSON pequeños se verifican con SHA-256.
pub const MODEL_FILES: &[ModelFile] = &[
    ModelFile {
        path: "onnx/duration_predictor.onnx",
        size: 3_700_147,
        sha256: "c3eb91414d5ff8a7a239b7fe9e34e7e2bf8a8140d8375ffb14718b1c639325db",
    },
    ModelFile {
        path: "onnx/text_encoder.onnx",
        size: 36_416_150,
        sha256: "c7befd5ea8c3119769e8a6c1486c4edc6a3bc8365c67621c881bbb774b9902ff",
    },
    ModelFile {
        path: "onnx/tts.json",
        size: 8_253,
        sha256: "42078d3aef1cd43ab43021f3c54f47d2d75ceb4e75f627f118890128b06a0d09",
    },
    ModelFile {
        path: "onnx/unicode_indexer.json",
        size: 277_676,
        sha256: "9bf7346e43883a81f8645c81224f786d43c5b57f3641f6e7671a7d6c493cb24f",
    },
    ModelFile {
        path: "onnx/vector_estimator.onnx",
        size: 256_534_781,
        sha256: "883ac868ea0275ef0e991524dc64f16b3c0376efd7c320af6b53f5b780d7c61c",
    },
    ModelFile {
        path: "onnx/vocoder.onnx",
        size: 101_424_195,
        sha256: "085de76dd8e8d5836d6ca66826601f615939218f90e519f70ee8a36ed2a4c4ba",
    },
    ModelFile {
        path: "voice_styles/F1.json",
        size: 292_046,
        sha256: "bbdec6ee00231c2c742ad05483df5334cab3b52fda3ba38e6a07059c4563dbc2",
    },
    ModelFile {
        path: "voice_styles/F2.json",
        size: 292_423,
        sha256: "7c722c6a72707b1a77f035d67f0d1351ba187738e06f7683e8c72b1df3477fc6",
    },
    ModelFile {
        path: "voice_styles/F3.json",
        size: 290_794,
        sha256: "12f6ef2573baa2defa1128069cb59f203e3ab67c92af77b42df8a0e3a2f7c6ab",
    },
    ModelFile {
        path: "voice_styles/F4.json",
        size: 291_808,
        sha256: "c2fa764c1225a76dfc3e2c73e8aa4f70d9ee48793860eb34c295fff01c2e032b",
    },
    ModelFile {
        path: "voice_styles/F5.json",
        size: 291_479,
        sha256: "45966e73316415626cf41a7d1c6f3b4c70dbc1ba2bee5c1978ef0ce33244fc8d",
    },
    ModelFile {
        path: "voice_styles/M1.json",
        size: 291_748,
        sha256: "e35604687f5d23694b8e91593a93eec0e4eca6c0b02bb8ed69139ab2ea6b0a5b",
    },
    ModelFile {
        path: "voice_styles/M2.json",
        size: 292_055,
        sha256: "b76cbf62bac707c710cf0ae5aba5e31eea1a6339a9734bfae33ab98499534a50",
    },
    ModelFile {
        path: "voice_styles/M3.json",
        size: 290_198,
        sha256: "ea1ac35ccb91b0d7ecad533a2fbd0eec10c91513d8951e3b25fbba99954e159b",
    },
    ModelFile {
        path: "voice_styles/M4.json",
        size: 291_522,
        sha256: "ca8eefad4fcd989c9379032ff3e50738adc547eeb5e221b82593a6d7b3bac303",
    },
    ModelFile {
        path: "voice_styles/M5.json",
        size: 291_469,
        sha256: "dd22b92740314321f8ae11c5e87f8dd60d060f15dd3a632b5adf77f471f77af2",
    },
];

#[derive(Debug, Clone, Copy)]
struct Preset {
    style: &'static str,
    language: &'static str,
    name: &'static str,
    gender: &'static str,
}

/// Seis voces curadas de las diez incluidas. Se evitan duplicados tímbricos y
/// se mantiene equilibrio femenino/masculino en cada idioma.
const CURATED: &[Preset] = &[
    Preset {
        style: "F1",
        language: "es",
        name: "Español (Supertonic F1)",
        gender: "female",
    },
    Preset {
        style: "F3",
        language: "es",
        name: "Español (Supertonic F3)",
        gender: "female",
    },
    Preset {
        style: "M2",
        language: "es",
        name: "Español (Supertonic M2)",
        gender: "male",
    },
    Preset {
        style: "F2",
        language: "en",
        name: "English (Supertonic F2)",
        gender: "female",
    },
    Preset {
        style: "F4",
        language: "en",
        name: "English (Supertonic F4)",
        gender: "female",
    },
    Preset {
        style: "M1",
        language: "en",
        name: "English (Supertonic M1)",
        gender: "male",
    },
];

pub fn total_bytes() -> u64 {
    MODEL_FILES.iter().map(|file| file.size).sum()
}

pub fn root_for(assets: &Path) -> PathBuf {
    assets.join("tts").join("supertonic")
}

fn url_for(path: &str) -> String {
    format!("{BASE_URL}/{path}")
}

// ---------------------------------------------------------------------------
// Estado persistente
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackState {
    license_accepted: bool,
    enabled: bool,
}

impl Default for PackState {
    fn default() -> Self {
        Self {
            license_accepted: false,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupertonicStatus {
    pub installed: bool,
    pub enabled: bool,
    pub license_accepted: bool,
    pub license: &'static str,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
}

impl SupertonicStatus {
    pub fn active(&self) -> bool {
        self.installed && self.enabled
    }
}

// ---------------------------------------------------------------------------
// Servicio
// ---------------------------------------------------------------------------

pub struct SupertonicService {
    root: PathBuf,
    player: Arc<dyn AudioPlayer>,
    state: Mutex<PackState>,
    engine: Mutex<EngineState>,
    downloading: AtomicBool,
}

#[derive(Default)]
struct EngineState {
    loaded: Option<LoadedModel>,
    last_used: Option<Instant>,
    styles: HashMap<String, Style>,
}

pub struct LoadedModel {
    /// Mantener el entorno vivo mientras existan sesiones.
    _environment: ort::environment::Environment,
    config: Config,
    indexer: Vec<i64>,
    duration: Session,
    text_encoder: Session,
    vector_estimator: Session,
    vocoder: Session,
    sample_rate: u32,
}

impl SupertonicService {
    pub fn new(root: PathBuf, player: Arc<dyn AudioPlayer>) -> Self {
        let state = load_state(&root).unwrap_or_default();
        Self {
            root,
            player,
            state: Mutex::new(state),
            engine: Mutex::new(EngineState::default()),
            downloading: AtomicBool::new(false),
        }
    }

    /// Hilo que libera las sesiones ONNX tras 90 s sin uso. Se usa `Weak`
    /// para que el hilo no impida cerrar la aplicación.
    pub fn spawn_reaper(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(15));
                let Some(service) = Weak::upgrade(&weak) else {
                    break;
                };
                if service.unload_if_idle() {
                    tracing::info!("Supertonic: sesiones liberadas por inactividad");
                }
            }
        });
    }

    fn unload_if_idle(&self) -> bool {
        let Ok(mut engine) = self.engine.lock() else {
            return false;
        };
        let idle = engine
            .last_used
            .map(|instant| instant.elapsed() >= IDLE_UNLOAD)
            .unwrap_or(false);
        if idle && engine.loaded.take().is_some() {
            engine.styles.clear();
            return true;
        }
        false
    }

    pub fn status(&self) -> SupertonicStatus {
        let state = self
            .state
            .lock()
            .map(|state| state.clone())
            .unwrap_or_default();
        let downloaded_bytes = MODEL_FILES
            .iter()
            .filter(|file| file_present(&self.root, file))
            .map(|file| file.size)
            .sum();
        SupertonicStatus {
            installed: is_installed(&self.root),
            enabled: state.enabled,
            license_accepted: state.license_accepted,
            license: LICENSE_NAME,
            total_bytes: total_bytes(),
            downloaded_bytes,
        }
    }

    pub fn accept_license(&self) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| KoraError::Internal("estado de Supertonic envenenado".into()))?;
        state.license_accepted = true;
        state.enabled = true;
        save_state(&self.root, &state)
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| KoraError::Internal("estado de Supertonic envenenado".into()))?;
        state.enabled = enabled;
        save_state(&self.root, &state)?;
        if !enabled {
            self.unload();
        }
        Ok(())
    }

    pub fn unload(&self) {
        if let Ok(mut engine) = self.engine.lock() {
            engine.loaded = None;
            engine.styles.clear();
            engine.last_used = None;
        }
    }

    /// Descarga los archivos que falten. Los ya verificados se conservan, por
    /// lo que una descarga interrumpida puede reanudarse.
    pub fn download(&self, progress: &mut dyn FnMut(u64, u64)) -> Result<()> {
        if !self
            .state
            .lock()
            .map(|state| state.license_accepted)
            .unwrap_or(false)
        {
            return Err(KoraError::Conflict(
                "acepta la licencia OpenRAIL-M antes de descargar las voces".into(),
            ));
        }
        if self
            .downloading
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(KoraError::Conflict(
                "ya hay una descarga de Supertonic en curso".into(),
            ));
        }

        let result = (|| -> Result<()> {
            fs::create_dir_all(&self.root)?;
            let total = total_bytes();
            let mut done: u64 = 0;
            for file in MODEL_FILES {
                let destination = self.root.join(file.path);
                if file_present(&self.root, file) {
                    done += file.size;
                    progress(done, total);
                    continue;
                }
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }
                let partial = destination.with_extension("part");
                let _ = fs::remove_file(&partial);
                let base = done;
                download_one(&url_for(file.path), &partial, file, &mut |read| {
                    progress(base + read, total)
                })?;
                fs::rename(&partial, &destination)?;
                done += file.size;
                progress(done, total);
            }
            Ok(())
        })();

        self.downloading.store(false, Ordering::SeqCst);
        if result.is_err() {
            let _ = fs::remove_file(self.root.join("onnx/vector_estimator.part"));
        }
        result
    }

    pub fn delete_pack(&self) -> Result<()> {
        self.unload();
        if self.root.exists() {
            fs::remove_dir_all(&self.root)?;
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| KoraError::Internal("estado de Supertonic envenenado".into()))?;
        state.enabled = true;
        save_state(&self.root, &state)
    }

    fn voices(&self) -> Vec<VoiceDescriptor> {
        let status = self.status();
        if !status.active() {
            return Vec::new();
        }
        CURATED
            .iter()
            .map(|preset| {
                let language = preset.language;
                VoiceDescriptor {
                    id: format!("{PREFIX}{language}:{}", preset.style),
                    name: preset.name.to_string(),
                    language: language.to_string(),
                    gender: Some(preset.gender.to_string()),
                    engine: ENGINE_ID.to_string(),
                    installed: true,
                    size_bytes: 0,
                    cloned: false,
                    enhanced: true,
                    quality: Some(500),
                    latency: None,
                    requires_network: false,
                    locale: Some(language.to_string()),
                }
            })
            .collect()
    }

    fn load(&self) -> Result<()> {
        let mut engine = self
            .engine
            .lock()
            .map_err(|_| KoraError::Internal("estado de Supertonic envenenado".into()))?;
        if engine.loaded.is_some() {
            engine.last_used = Some(Instant::now());
            return Ok(());
        }
        if !is_installed(&self.root) {
            return Err(KoraError::NotFound(
                "el pack de voces mejoradas no está descargado".into(),
            ));
        }

        let started = Instant::now();
        let config = load_config(&self.root)?;
        let indexer = load_indexer(&self.root)?;
        let environment = ort::init()
            .with_name("kora-supertonic")
            .with_telemetry(false)
            .build()
            .map_err(|error| {
                KoraError::Internal(format!("no se pudo iniciar ONNX Runtime: {error}"))
            })?;

        let session = |name: &str| -> Result<Session> {
            let path = self.root.join("onnx").join(name);
            Session::builder(&environment)
                .and_then(|builder| builder.with_intra_threads(threads()).map_err(Into::into))
                .and_then(|mut builder| builder.commit_from_file(&path))
                .map_err(|error| KoraError::Internal(format!("no se pudo cargar {name}: {error}")))
        };

        let loaded = LoadedModel {
            duration: session("duration_predictor.onnx")?,
            text_encoder: session("text_encoder.onnx")?,
            vector_estimator: session("vector_estimator.onnx")?,
            vocoder: session("vocoder.onnx")?,
            sample_rate: config.ae.sample_rate.max(8_000) as u32,
            config,
            indexer,
            _environment: environment,
        };

        tracing::info!(
            elapsed_ms = started.elapsed().as_millis() as u64,
            "Supertonic: modelo cargado"
        );
        engine.loaded = Some(loaded);
        engine.styles.clear();
        engine.last_used = Some(Instant::now());
        Ok(())
    }

    fn synthesize_blocking(&self, voice_id: &str, text: &str, rate: f32) -> Result<AudioChunk> {
        let (language, style_id) = parse_voice_id(voice_id)?;
        self.load()?;

        let mut engine = self
            .engine
            .lock()
            .map_err(|_| KoraError::Internal("estado de Supertonic envenenado".into()))?;

        if !engine.styles.contains_key(style_id) {
            let style = load_style(&self.root, style_id)?;
            engine.styles.insert(style_id.to_string(), style);
        }
        let style = engine
            .styles
            .get(style_id)
            .cloned()
            .ok_or_else(|| KoraError::Internal("estilo Supertonic no encontrado".into()))?;
        let model = engine
            .loaded
            .as_mut()
            .ok_or_else(|| KoraError::Internal("el modelo Supertonic no está cargado".into()))?;

        let chunks = chunk_text(text, 300);
        let mut pcm = Vec::new();
        let mut sample_rate = model.sample_rate;

        for (index, chunk) in chunks.iter().enumerate() {
            if chunk.trim().is_empty() {
                continue;
            }
            let (samples, _) = infer(model, chunk, language, &style, 8, rate)?;
            sample_rate = model.sample_rate;
            if index > 0 {
                pcm.extend(std::iter::repeat_n(
                    0i16,
                    (sample_rate as usize * 120) / 1000,
                ));
            }
            pcm.extend(
                samples
                    .into_iter()
                    .map(|sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16),
            );
        }

        engine.last_used = Some(Instant::now());
        if pcm.is_empty() {
            return Err(KoraError::Internal(
                "Supertonic devolvió audio vacío".into(),
            ));
        }
        normalize_peak(&mut pcm);
        Ok(AudioChunk {
            pcm,
            sample_rate,
            channels: 1,
        })
    }
}

fn threads() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get().clamp(2, 4))
        .unwrap_or(2)
}

fn load_state(root: &Path) -> Result<PackState> {
    let path = root.join("state.json");
    if !path.exists() {
        return Ok(PackState::default());
    }
    let file = File::open(path)?;
    let state = serde_json::from_reader(BufReader::new(file)).unwrap_or_default();
    Ok(state)
}

fn save_state(root: &Path, state: &PackState) -> Result<()> {
    fs::create_dir_all(root)?;
    let path = root.join("state.json");
    let partial = root.join("state.json.part");
    let mut file = File::create(&partial)?;
    serde_json::to_writer_pretty(&mut file, state)
        .map_err(|error| KoraError::Storage(error.to_string()))?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(partial, path)?;
    Ok(())
}

fn file_present(root: &Path, file: &ModelFile) -> bool {
    fs::metadata(root.join(file.path))
        .map(|metadata| metadata.is_file() && metadata.len() == file.size)
        .unwrap_or(false)
}

pub fn is_installed(root: &Path) -> bool {
    MODEL_FILES.iter().all(|file| file_present(root, file))
}

fn download_one(
    url: &str,
    destination: &Path,
    file: &ModelFile,
    progress: &mut dyn FnMut(u64),
) -> Result<()> {
    let response = ureq::get(url).call().map_err(|error| {
        KoraError::Internal(format!("no se pudo descargar {}: {error}", file.path))
    })?;
    let mut reader = response
        .into_body()
        .into_with_config()
        .limit(MAX_DOWNLOAD_BYTES)
        .reader();

    let mut output = File::create(destination)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 128 * 1024];
    let mut read_total = 0u64;

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| KoraError::Io(std::io::Error::other(error)))?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read])?;
        hasher.update(&buffer[..read]);
        read_total += read as u64;
        progress(read_total);
    }
    output.sync_all()?;

    if read_total != file.size {
        let _ = fs::remove_file(destination);
        return Err(KoraError::Storage(format!(
            "{} llegó incompleto ({} de {} bytes)",
            file.path, read_total, file.size
        )));
    }
    let digest = hex::encode(hasher.finalize());
    if digest != file.sha256 {
        let _ = fs::remove_file(destination);
        return Err(KoraError::Storage(format!(
            "la verificación SHA-256 de {} no coincide",
            file.path
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Formato del modelo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
struct Config {
    ae: AeConfig,
    ttl: TtlConfig,
}

#[derive(Debug, Clone, Deserialize)]
struct AeConfig {
    sample_rate: i32,
    base_chunk_size: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct TtlConfig {
    chunk_compress_factor: i32,
    latent_dim: i32,
}

fn load_config(root: &Path) -> Result<Config> {
    let file = File::open(root.join("onnx/tts.json"))?;
    serde_json::from_reader(BufReader::new(file))
        .map_err(|error| KoraError::Storage(format!("tts.json inválido: {error}")))
}

fn load_indexer(root: &Path) -> Result<Vec<i64>> {
    let file = File::open(root.join("onnx/unicode_indexer.json"))?;
    serde_json::from_reader(BufReader::new(file))
        .map_err(|error| KoraError::Storage(format!("unicode_indexer.json inválido: {error}")))
}

#[derive(Debug, Clone, Deserialize)]
struct VoiceStyleData {
    style_ttl: StyleComponent,
    style_dp: StyleComponent,
}

#[derive(Debug, Clone, Deserialize)]
struct StyleComponent {
    data: Vec<Vec<Vec<f32>>>,
    dims: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct Style {
    ttl: Array3<f32>,
    dp: Array3<f32>,
}

fn load_style(root: &Path, id: &str) -> Result<Style> {
    let file = File::open(root.join("voice_styles").join(format!("{id}.json")))?;
    let data: VoiceStyleData = serde_json::from_reader(BufReader::new(file))
        .map_err(|error| KoraError::Storage(format!("estilo {id} inválido: {error}")))?;

    let dims = |dims: &[usize]| -> Result<(usize, usize)> {
        if dims.len() >= 3 {
            Ok((dims[1], dims[2]))
        } else {
            Err(KoraError::Storage(format!(
                "el estilo {id} tiene dimensiones inesperadas"
            )))
        }
    };
    let (ttl_rows, ttl_cols) = dims(&data.style_ttl.dims)?;
    let (dp_rows, dp_cols) = dims(&data.style_dp.dims)?;

    let flatten = |source: &[Vec<Vec<f32>>]| -> Vec<f32> {
        source
            .iter()
            .flat_map(|batch| batch.iter().flat_map(|row| row.iter().copied()))
            .collect()
    };

    let ttl = Array3::from_shape_vec((1, ttl_rows, ttl_cols), flatten(&data.style_ttl.data))
        .map_err(|error| KoraError::Storage(format!("estilo {id}: {error}")))?;
    let dp = Array3::from_shape_vec((1, dp_rows, dp_cols), flatten(&data.style_dp.data))
        .map_err(|error| KoraError::Storage(format!("estilo {id}: {error}")))?;
    Ok(Style { ttl, dp })
}

// ---------------------------------------------------------------------------
// Inferencia (porte del ejemplo oficial, MIT)
// ---------------------------------------------------------------------------

fn parse_voice_id(voice_id: &str) -> Result<(&str, &str)> {
    let rest = voice_id.strip_prefix(PREFIX).ok_or_else(|| {
        KoraError::InvalidInput(format!("voz Supertonic desconocida: {voice_id}"))
    })?;
    let (language, style) = rest.split_once(':').ok_or_else(|| {
        KoraError::InvalidInput(format!("voz Supertonic desconocida: {voice_id}"))
    })?;
    if language != "es" && language != "en" {
        return Err(KoraError::InvalidInput(format!(
            "idioma no soportado para Supertonic: {language}"
        )));
    }
    if !CURATED.iter().any(|preset| preset.style == style) {
        return Err(KoraError::NotFound(format!(
            "la voz Supertonic {style} no está en el catálogo"
        )));
    }
    Ok((language, style))
}

fn infer(
    model: &mut LoadedModel,
    text: &str,
    language: &str,
    style: &Style,
    total_step: usize,
    speed: f32,
) -> Result<(Vec<f32>, f32)> {
    let prepared = preprocess_text(text, language)?;
    let text_ids = encode_text(&prepared, &model.indexer);
    let length = text_ids.len().max(1);
    if text_ids.is_empty() {
        return Err(KoraError::InvalidInput(
            "texto vacío para Supertonic".into(),
        ));
    }

    let text_ids_array = Array::from_shape_vec((1, length), text_ids)
        .map_err(|error| KoraError::Internal(error.to_string()))?;
    let text_mask = Array3::from_shape_vec(
        (1, 1, length),
        std::iter::repeat_n(1.0f32, length).collect::<Vec<_>>(),
    )
    .map_err(|error| KoraError::Internal(error.to_string()))?;

    let text_ids_tensor = Tensor::from_array(text_ids_array)
        .map_err(|error| KoraError::Internal(format!("tensor text_ids: {error}")))?;
    let text_mask_tensor = Tensor::from_array(text_mask.clone())
        .map_err(|error| KoraError::Internal(format!("tensor text_mask: {error}")))?;
    let style_dp_tensor = Tensor::from_array(style.dp.clone())
        .map_err(|error| KoraError::Internal(format!("tensor style_dp: {error}")))?;
    let style_ttl_tensor = Tensor::from_array(style.ttl.clone())
        .map_err(|error| KoraError::Internal(format!("tensor style_ttl: {error}")))?;

    let duration_outputs = model
        .duration
        .run(ort::inputs![
            "text_ids" => &text_ids_tensor,
            "style_dp" => &style_dp_tensor,
            "text_mask" => &text_mask_tensor,
        ])
        .map_err(|error| KoraError::Internal(format!("Supertonic (duración): {error}")))?;
    let (_, duration_data) = duration_outputs["duration"]
        .try_extract_tensor::<f32>()
        .map_err(|error| KoraError::Internal(format!("salida duration: {error}")))?;
    let duration: Vec<f32> = duration_data
        .iter()
        .map(|value| (*value / speed.max(0.25)).max(0.05))
        .collect();

    let text_outputs = model
        .text_encoder
        .run(ort::inputs![
            "text_ids" => &text_ids_tensor,
            "style_ttl" => &style_ttl_tensor,
            "text_mask" => &text_mask_tensor,
        ])
        .map_err(|error| KoraError::Internal(format!("Supertonic (texto): {error}")))?;
    let (text_shape, text_data) = text_outputs["text_emb"]
        .try_extract_tensor::<f32>()
        .map_err(|error| KoraError::Internal(format!("salida text_emb: {error}")))?;
    let text_emb = Array3::from_shape_vec(
        (
            text_shape[0] as usize,
            text_shape[1] as usize,
            text_shape[2] as usize,
        ),
        text_data.to_vec(),
    )
    .map_err(|error| KoraError::Internal(format!("text_emb inválido: {error}")))?;

    let (mut latent, latent_mask) = sample_noisy_latent(
        &duration,
        model.sample_rate as i32,
        model.config.ae.base_chunk_size,
        model.config.ttl.chunk_compress_factor,
        model.config.ttl.latent_dim,
    );

    let total_step_array = Array::from_elem(1, total_step as f32);
    for step in 0..total_step {
        let latent_tensor = Tensor::from_array(latent.clone())
            .map_err(|error| KoraError::Internal(format!("tensor latent: {error}")))?;
        let emb_tensor = Tensor::from_array(text_emb.clone())
            .map_err(|error| KoraError::Internal(format!("tensor text_emb: {error}")))?;
        let mask_tensor = Tensor::from_array(latent_mask.clone())
            .map_err(|error| KoraError::Internal(format!("tensor latent_mask: {error}")))?;
        let text_mask_tensor = Tensor::from_array(text_mask.clone())
            .map_err(|error| KoraError::Internal(format!("tensor text_mask: {error}")))?;
        let step_tensor = Tensor::from_array(Array::from_elem(1, step as f32))
            .map_err(|error| KoraError::Internal(format!("tensor current_step: {error}")))?;
        let total_tensor = Tensor::from_array(total_step_array.clone())
            .map_err(|error| KoraError::Internal(format!("tensor total_step: {error}")))?;

        let outputs = model
            .vector_estimator
            .run(ort::inputs![
                "noisy_latent" => &latent_tensor,
                "text_emb" => &emb_tensor,
                "style_ttl" => &style_ttl_tensor,
                "latent_mask" => &mask_tensor,
                "text_mask" => &text_mask_tensor,
                "current_step" => &step_tensor,
                "total_step" => &total_tensor,
            ])
            .map_err(|error| {
                KoraError::Internal(format!("Supertonic (paso {step}/{total_step}): {error}"))
            })?;
        let (shape, data) = outputs["denoised_latent"]
            .try_extract_tensor::<f32>()
            .map_err(|error| KoraError::Internal(format!("salida denoised_latent: {error}")))?;
        latent = Array3::from_shape_vec(
            (shape[0] as usize, shape[1] as usize, shape[2] as usize),
            data.to_vec(),
        )
        .map_err(|error| KoraError::Internal(format!("latente inválido: {error}")))?;
    }

    let latent_tensor = Tensor::from_array(latent)
        .map_err(|error| KoraError::Internal(format!("tensor latent final: {error}")))?;
    let outputs = model
        .vocoder
        .run(ort::inputs!["latent" => &latent_tensor])
        .map_err(|error| KoraError::Internal(format!("Supertonic (vocoder): {error}")))?;
    let (_, wav_data) = outputs["wav_tts"]
        .try_extract_tensor::<f32>()
        .map_err(|error| KoraError::Internal(format!("salida wav_tts: {error}")))?;

    let duration_seconds = duration.first().copied().unwrap_or(0.0);
    let sample_count = (model.sample_rate as f32 * duration_seconds) as usize;
    let mut wav = wav_data.to_vec();
    wav.truncate(sample_count.min(wav.len()));
    Ok((wav, duration_seconds))
}

fn encode_text(text: &str, indexer: &[i64]) -> Vec<i64> {
    text.chars()
        .map(|character| {
            let code = character as usize;
            indexer.get(code).copied().unwrap_or(-1)
        })
        .collect()
}

fn sample_noisy_latent(
    duration: &[f32],
    sample_rate: i32,
    base_chunk_size: i32,
    chunk_compress: i32,
    latent_dim: i32,
) -> (Array3<f32>, Array3<f32>) {
    let max_duration = duration.iter().fold(0.0f32, |max, value| max.max(*value));
    let wav_len_max = (max_duration * sample_rate as f32) as usize;
    let chunk_size = (base_chunk_size * chunk_compress).max(1) as usize;
    let latent_len = wav_len_max.div_ceil(chunk_size);
    let latent_channels = (latent_dim * chunk_compress) as usize;

    let mut rng = rand::thread_rng();
    let mut latent = Array3::<f32>::zeros((duration.len(), latent_channels, latent_len));
    for batch in 0..duration.len() {
        for channel in 0..latent_channels {
            for position in 0..latent_len {
                latent[[batch, channel, position]] = gaussian(&mut rng);
            }
        }
    }

    let lengths: Vec<usize> = duration
        .iter()
        .map(|value| {
            let wav_len = (*value * sample_rate as f32) as usize;
            wav_len.div_ceil(chunk_size)
        })
        .collect();
    let mask = length_mask(&lengths, latent_len);
    for batch in 0..duration.len() {
        for channel in 0..latent_channels {
            for position in 0..latent_len {
                latent[[batch, channel, position]] *= mask[[batch, 0, position]];
            }
        }
    }
    (latent, mask)
}

fn gaussian(rng: &mut impl Rng) -> f32 {
    // Box-Muller: evita añadir otra dependencia solo para el ruido latente.
    let u1: f32 = rng.gen_range(1e-7..1.0);
    let u2: f32 = rng.gen_range(0.0..1.0);
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
}

fn length_mask(lengths: &[usize], max_len: usize) -> Array3<f32> {
    let mut mask = Array3::<f32>::zeros((lengths.len(), 1, max_len));
    for (index, length) in lengths.iter().enumerate() {
        for position in 0..(*length).min(max_len) {
            mask[[index, 0, position]] = 1.0;
        }
    }
    mask
}

// ---------------------------------------------------------------------------
// Preprocesado de texto
// ---------------------------------------------------------------------------

fn preprocess_text(text: &str, language: &str) -> Result<String> {
    use unicode_normalization::UnicodeNormalization;

    let mut normalized: String = text.nfkd().collect();

    // Quita emojis y pictogramas que el indexador no conoce.
    normalized = normalized
        .chars()
        .filter(|character| {
            let code = *character as u32;
            !matches!(code, 0x1F000..=0x1FAFF | 0x2600..=0x27BF)
        })
        .collect();

    for (from, to) in [
        ('\u{2013}', '-'),
        ('\u{2011}', '-'),
        ('\u{2014}', '-'),
        ('_', ' '),
        ('\u{201C}', '"'),
        ('\u{201D}', '"'),
        ('\u{2018}', '\''),
        ('\u{2019}', '\''),
        ('´', '\''),
        ('`', '\''),
        ('[', ' '),
        (']', ' '),
        ('|', ' '),
        ('/', ' '),
        ('#', ' '),
        ('→', ' '),
        ('←', ' '),
    ] {
        normalized = normalized.replace(from, &to.to_string());
    }
    for symbol in ["♥", "☆", "♡", "©", "\\"] {
        normalized = normalized.replace(symbol, "");
    }
    normalized = normalized
        .replace('@', " at ")
        .replace("e.g.,", "for example, ")
        .replace("i.e.,", "that is, ");

    for (from, to) in [
        (" ,", ","),
        (" .", "."),
        (" !", "!"),
        (" ?", "?"),
        (" ;", ";"),
        (" :", ":"),
        (" '", "'"),
    ] {
        normalized = normalized.replace(from, to);
    }
    while normalized.contains("\"\"") {
        normalized = normalized.replace("\"\"", "\"");
    }
    while normalized.contains("''") {
        normalized = normalized.replace("''", "'");
    }
    while normalized.contains("``") {
        normalized = normalized.replace("``", "`");
    }

    normalized = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return Err(KoraError::InvalidInput(
            "el texto no contiene nada pronunciable".into(),
        ));
    }
    let ends_with_punctuation = normalized
        .chars()
        .last()
        .is_some_and(|character| ".!?;:,'\")]}…".contains(character));
    if !ends_with_punctuation {
        normalized.push('.');
    }
    Ok(format!("<{language}>{normalized}</{language}>"))
}

fn chunk_text(text: &str, max_len: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for sentence in text.split_inclusive(['.', '!', '?', '\n']) {
        let sentence = sentence.trim();
        if sentence.is_empty() {
            continue;
        }
        if !current.is_empty() && current.chars().count() + 1 + sentence.chars().count() > max_len {
            chunks.push(std::mem::take(&mut current));
        }
        if sentence.chars().count() > max_len {
            for word in sentence.split_whitespace() {
                if current.chars().count() + word.chars().count() + 1 > max_len
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
    chunks
}

/// Supertonic sale a un nivel bajo (~-26 dBFS). Se sube el pico a -0.7 dBFS
/// sin comprimir la dinámica ni amplificar el silencio.
fn normalize_peak(pcm: &mut [i16]) {
    let peak = pcm
        .iter()
        .map(|sample| sample.unsigned_abs())
        .max()
        .unwrap_or(0);
    if peak < 256 {
        return;
    }
    let target = (i16::MAX as f32 * 0.92) as u32;
    let gain = (target as f32 / peak as f32).min(4.0);
    if gain <= 1.05 {
        return;
    }
    for sample in pcm.iter_mut() {
        *sample = (*sample as f32 * gain)
            .clamp(i16::MIN as f32, i16::MAX as f32)
            .round() as i16;
    }
}

fn resample_pitch(chunk: AudioChunk, pitch: f32) -> AudioChunk {
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

// ---------------------------------------------------------------------------
// Motor TTS
// ---------------------------------------------------------------------------

pub struct SupertonicEngine {
    service: Arc<SupertonicService>,
}

impl SupertonicEngine {
    pub fn new(service: Arc<SupertonicService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl TtsEngine for SupertonicEngine {
    fn info(&self) -> EngineInfo {
        let status = self.service.status();
        EngineInfo {
            id: ENGINE_ID.to_string(),
            name: "Supertonic 3 (voces mejoradas)".to_string(),
            offline: true,
            voice_cloning: false,
            streaming: false,
            available: status.active(),
            supports_pause: true,
        }
    }

    async fn voices(&self) -> Result<Vec<VoiceDescriptor>> {
        Ok(self.service.voices())
    }

    async fn synthesize(&self, request: &SynthesisRequest) -> Result<AudioChunk> {
        let text = request.text.trim().to_string();
        if text.is_empty() {
            return Err(KoraError::InvalidInput(
                "la vista previa necesita texto".into(),
            ));
        }
        let voice_id = request.voice_id.clone();
        let rate = request.rate.clamp(0.5, 2.0);
        let pitch = request.pitch.clamp(0.5, 1.5);
        let service = self.service.clone();
        let chunk = tokio::task::spawn_blocking(move || {
            service.synthesize_blocking(&voice_id, &text, rate)
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea Supertonic: {error}")))??;
        Ok(resample_pitch(chunk, pitch))
    }

    async fn speak(&self, request: &SpeakRequest) -> Result<()> {
        let text = request.text.trim().to_string();
        if text.is_empty() {
            return Ok(());
        }
        self.service.player.reset().await?;
        let voice_id = request.voice_id.clone();
        let rate = request.rate.clamp(0.5, 2.0);
        let pitch = request.pitch.clamp(0.5, 1.5);
        let volume = request.volume.clamp(0.0, 1.0);
        let service = self.service.clone();
        let mut chunk = tokio::task::spawn_blocking(move || {
            service.synthesize_blocking(&voice_id, &text, rate)
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea Supertonic: {error}")))??;

        chunk = resample_pitch(chunk, pitch);
        if volume < 0.999 {
            for sample in &mut chunk.pcm {
                *sample = (*sample as f32 * volume).round() as i16;
            }
        }
        self.service.player.play(chunk).await
    }

    async fn pause(&self) -> Result<()> {
        self.service.player.pause().await
    }

    async fn resume(&self) -> Result<()> {
        self.service.player.resume().await
    }

    async fn stop(&self) -> Result<()> {
        self.service.player.stop().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_ids_encode_language_and_style() {
        assert_eq!(parse_voice_id("supertonic:es:F1").unwrap(), ("es", "F1"));
        assert!(parse_voice_id("supertonic:fr:F1").is_err());
        assert!(parse_voice_id("supertonic:es:F5").is_err());
        assert!(parse_voice_id("es_ES-davefx-medium").is_err());
    }

    #[test]
    fn preprocess_wraps_language_and_punctuation() {
        let text = preprocess_text("Hola   mundo", "es").unwrap();
        assert_eq!(text, "<es>Hola mundo.</es>");
    }

    #[test]
    fn chunker_respects_limit() {
        let text = "Uno. Dos. Tres. Cuatro. Cinco. Seis.";
        let chunks = chunk_text(text, 14);
        assert!(chunks.iter().all(|chunk| chunk.chars().count() <= 14));
        assert!(!chunks.is_empty());
    }

    #[test]
    fn catalog_sizes_match_manifest() {
        let total = total_bytes();
        assert!(total > 400_000_000 && total < 410_000_000);
    }
}
