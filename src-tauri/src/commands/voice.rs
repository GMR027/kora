use std::path::{Path, PathBuf};

use base64::Engine as _;
use chrono::Utc;
use serde::Serialize;
#[cfg(not(target_os = "android"))]
use tauri::Emitter;
use tauri::{AppHandle, State};

use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::storage::{NewAuditEntry, ResourceType};
use crate::tts::recording::{MAX_SAMPLE_SECONDS, discard_recording};
use crate::tts::voice_cloner::{
    CONSENT_TEXT, PROFILE_VERSION, ProfileFile, VoiceProfile, engine_for_language,
    input_format_for_language, read_archive, strip_clone_prefix, validate_id, write_imported,
};
use crate::tts::{ModelKind, SynthesisRequest, TtsEngine, encode_wav_with_watermark};
#[cfg(not(target_os = "android"))]
use crate::tts::{VOICE_MODELS, VoiceModelSpec, models_for_language};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceModelInfo {
    pub id: String,
    pub file: String,
    pub label: String,
    pub kind: String,
    pub size_bytes: u64,
    pub installed: bool,
    pub license: String,
    pub languages: Vec<String>,
}

#[cfg(not(target_os = "android"))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceModelProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoicePreview {
    pub audio_base64: String,
    pub sample_rate: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSample {
    pub path: String,
    pub duration_ms: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceCapabilities {
    pub recording: bool,
    pub cloning: bool,
    pub model_download: bool,
    pub voice_download: bool,
}

#[tauri::command]
pub fn voice_capabilities(state: State<'_, AppState>) -> VoiceCapabilities {
    VoiceCapabilities {
        recording: cfg!(not(target_os = "android")),
        cloning: state.neutts.is_some(),
        model_download: cfg!(not(target_os = "android")),
        voice_download: cfg!(not(target_os = "android")),
    }
}

#[tauri::command]
pub fn voice_consent_text() -> &'static str {
    CONSENT_TEXT
}

#[tauri::command]
pub async fn voice_sample_start(
    state: State<'_, AppState>,
    device_name: Option<String>,
) -> Result<()> {
    let directory = state.layout.assets.join("tts").join("recordings");
    state
        .voice_sample
        .start(directory, MAX_SAMPLE_SECONDS, device_name)
}

#[tauri::command]
pub fn voice_sample_level(state: State<'_, AppState>) -> f32 {
    state.voice_sample.level()
}

#[tauri::command]
pub fn voice_input_devices() -> Result<Vec<String>> {
    crate::tts::recording::input_devices()
}

#[tauri::command]
pub async fn voice_sample_stop(state: State<'_, AppState>) -> Result<VoiceSample> {
    let recording = state.voice_sample.stop()?;
    Ok(VoiceSample {
        path: recording.wav_path.to_string_lossy().into_owned(),
        duration_ms: recording.duration_ms,
        warnings: recording.warnings,
    })
}

#[tauri::command]
pub async fn voice_sample_cancel(state: State<'_, AppState>) -> Result<()> {
    state.voice_sample.cancel()
}

#[tauri::command]
pub async fn voice_sample_discard(state: State<'_, AppState>, path: String) -> Result<()> {
    let directory = state.layout.assets.join("tts").join("recordings");
    discard_recording(&directory, Path::new(&path))
}

#[tauri::command]
pub fn voice_models(state: State<'_, AppState>) -> Result<Vec<VoiceModelInfo>> {
    Ok(crate::tts::VOICE_MODELS
        .iter()
        .map(|model| VoiceModelInfo {
            id: model.id.to_string(),
            file: model.file.to_string(),
            label: model.label.to_string(),
            kind: kind_label(model.kind),
            size_bytes: model.size_bytes,
            installed: state.voice_profiles.model_path(model.file).is_file(),
            license: model.license.to_string(),
            languages: model
                .languages
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        })
        .collect())
}

fn kind_label(kind: ModelKind) -> String {
    match kind {
        ModelKind::Backbone => "backbone",
        ModelKind::Encoder => "encoder",
        ModelKind::Decoder => "decoder",
    }
    .to_string()
}

#[tauri::command]
pub async fn voice_download_models(
    state: State<'_, AppState>,
    app: AppHandle,
    language: String,
) -> Result<()> {
    if language != "es" && language != "en" {
        return Err(KoraError::InvalidInput("el idioma debe ser es o en".into()));
    }

    #[cfg(target_os = "android")]
    {
        let _ = (&state, &app, &language);
        return Err(KoraError::Unsupported(
            "la descarga de modelos de clonación aún no está disponible en Android".into(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        let root = state.voice_profiles.models_dir();
        let models: Vec<&'static VoiceModelSpec> = models_for_language(&language);
        let app_clone = app.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            std::fs::create_dir_all(&root)?;
            for model in models {
                download_one_model(&root, model, &app_clone)?;
            }
            Ok(())
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de descarga: {error}")))??;

        Ok(())
    }
}

#[tauri::command]
pub async fn voice_download_model(
    state: State<'_, AppState>,
    app: AppHandle,
    model_id: String,
) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        let _ = (&state, &app, &model_id);
        return Err(KoraError::Unsupported(
            "la descarga de modelos de clonación aún no está disponible en Android".into(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        let model = VOICE_MODELS
            .iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| KoraError::NotFound(format!("el modelo {model_id} no existe")))?;
        let root = state.voice_profiles.models_dir();

        tokio::task::spawn_blocking(move || -> Result<()> {
            std::fs::create_dir_all(&root)?;
            download_one_model(&root, model, &app)
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de descarga: {error}")))??;

        Ok(())
    }
}

#[cfg(not(target_os = "android"))]
fn download_one_model(root: &Path, model: &'static VoiceModelSpec, app: &AppHandle) -> Result<()> {
    let destination = root.join(model.file);
    if destination.is_file()
        && destination.metadata().map(|meta| meta.len()).unwrap_or(0) == model.size_bytes
    {
        return Ok(());
    }

    let temporary = root.join(format!("{}.tmp", model.file));
    let model_id = model.id.to_string();
    let app_progress = app.clone();
    let mut last_emit = std::time::Instant::now() - std::time::Duration::from_secs(1);

    let result = crate::tts::download::download_file(
        model.url,
        &temporary,
        model.size_bytes,
        &mut |downloaded, total| {
            if last_emit.elapsed() >= std::time::Duration::from_millis(160) || downloaded >= total {
                last_emit = std::time::Instant::now();
                let _ = app_progress.emit(
                    "voice-model-progress",
                    VoiceModelProgress {
                        model_id: model_id.clone(),
                        downloaded,
                        total,
                        done: false,
                        error: None,
                    },
                );
            }
        },
    );

    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        let _ = app.emit(
            "voice-model-progress",
            VoiceModelProgress {
                model_id,
                downloaded: 0,
                total: 0,
                done: true,
                error: Some(error.to_string()),
            },
        );
        return Err(error);
    }

    if let Err(error) = verify_file(&temporary, model) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }

    std::fs::rename(&temporary, &destination)?;
    let _ = app.emit(
        "voice-model-progress",
        VoiceModelProgress {
            model_id,
            downloaded: 1,
            total: 1,
            done: true,
            error: None,
        },
    );
    Ok(())
}

#[tauri::command]
pub async fn delete_voice_model(state: State<'_, AppState>, model_id: String) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        let _ = (&state, &model_id);
        return Err(KoraError::Unsupported(
            "la gestión de modelos de clonación aún no está disponible en Android".into(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        let model = VOICE_MODELS
            .iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| KoraError::NotFound(format!("el modelo {model_id} no existe")))?;
        let path = state.voice_profiles.model_path(model.file);
        if path.is_file() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}

#[cfg(not(target_os = "android"))]
fn verify_file(path: &Path, model: &VoiceModelSpec) -> Result<()> {
    let size = std::fs::metadata(path)?.len();
    if size != model.size_bytes {
        return Err(KoraError::Storage(format!(
            "{} llegó incompleto ({} de {} bytes)",
            model.file, size, model.size_bytes
        )));
    }

    let digest = sha256_file(path)?;
    if digest != model.sha256 {
        return Err(KoraError::Storage(format!(
            "{} no coincide con la huella esperada",
            model.file
        )));
    }
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_voice_profile(
    state: State<'_, AppState>,
    audio_path: String,
    name: String,
    language: String,
    reference_text: String,
    consent: bool,
) -> Result<VoiceProfile> {
    let owner = state.identity.account.id.clone();
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(KoraError::InvalidInput(
            "el nombre del perfil debe tener entre 1 y 80 caracteres".into(),
        ));
    }
    if !consent {
        return Err(KoraError::InvalidInput(
            "debes aceptar que solo clonas voces propias o con permiso".into(),
        ));
    }

    let language = language.trim().to_lowercase();
    engine_for_language(&language)?;
    let reference_text = reference_text.trim().to_string();
    if reference_text.is_empty() {
        return Err(KoraError::InvalidInput(
            "escribe lo que dice el clip de referencia".into(),
        ));
    }
    if reference_text.chars().count() > 2000 {
        return Err(KoraError::InvalidInput(
            "la transcripción de referencia es demasiado larga".into(),
        ));
    }

    let audio = PathBuf::from(audio_path);
    if !audio.is_file() {
        return Err(KoraError::NotFound(format!(
            "no se encontró el audio {}",
            audio.display()
        )));
    }

    let engine = state.neutts.as_ref().ok_or_else(|| {
        KoraError::Unsupported("la clonación de voz no está disponible en esta plataforma".into())
    })?;

    let id = uuid::Uuid::new_v4().to_string();
    let directory = state.voice_profiles.profile_dir(&id);
    std::fs::create_dir_all(&directory)?;

    let result = async {
        let encoded = engine
            .encode_reference(audio, language.clone(), reference_text, directory.clone())
            .await?;

        if !(2_500..=30_500).contains(&encoded.duration_ms) {
            return Err(KoraError::InvalidInput(format!(
                "el clip debe durar entre 3 y 30 segundos; dura {:.1} s",
                encoded.duration_ms as f64 / 1000.0
            )));
        }

        let profile = ProfileFile {
            version: PROFILE_VERSION,
            id: id.clone(),
            name: name.clone(),
            engine: engine_for_language(&language)?.to_string(),
            language: language.clone(),
            input_format: input_format_for_language(&language).to_string(),
            reference_duration_ms: encoded.duration_ms,
            frames: encoded.frames,
            created_at: Utc::now(),
        };

        std::fs::write(
            directory.join("reference.txt"),
            encoded.reference_text.trim(),
        )?;
        std::fs::write(
            directory.join("profile.json"),
            serde_json::to_vec_pretty(&profile)?,
        )?;

        state.voice_profiles.insert(&profile, &owner).await
    }
    .await;

    match result {
        Ok(profile) => {
            append_consent(
                &state,
                &owner,
                &profile.id,
                "create",
                &profile.name,
                &profile.language,
            )
            .await?;
            Ok(profile)
        }
        Err(error) => {
            let _ = crate::storage::trash::move_to_trash(&state.layout, &directory);
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn list_voice_profiles(state: State<'_, AppState>) -> Result<Vec<VoiceProfile>> {
    state.voice_profiles.list(&state.identity.account.id).await
}

#[tauri::command]
pub async fn delete_voice_profile(state: State<'_, AppState>, voice_id: String) -> Result<()> {
    let id = strip_clone_prefix(&voice_id).unwrap_or(&voice_id);
    state
        .voice_profiles
        .delete(id, &state.identity.account.id)
        .await
}

#[tauri::command]
pub async fn update_voice_profile(
    state: State<'_, AppState>,
    voice_id: String,
    name: String,
) -> Result<VoiceProfile> {
    let id = strip_clone_prefix(&voice_id).unwrap_or(&voice_id);
    validate_id(id)?;
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(KoraError::InvalidInput(
            "el nombre del perfil debe tener entre 1 y 80 caracteres".into(),
        ));
    }
    state
        .voice_profiles
        .rename(id, &state.identity.account.id, &name)
        .await
}

#[tauri::command]
pub async fn voice_profile_sample(state: State<'_, AppState>, voice_id: String) -> Result<String> {
    let id = strip_clone_prefix(&voice_id).unwrap_or(&voice_id);
    validate_id(id)?;
    let _ = state
        .voice_profiles
        .get(id, &state.identity.account.id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("el perfil {id} no existe")))?;
    let path = state.voice_profiles.wav_path(id);
    if !path.is_file() {
        return Err(KoraError::NotFound(
            "el perfil no tiene una muestra de audio".into(),
        ));
    }
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn replace_voice_sample(
    state: State<'_, AppState>,
    voice_id: String,
    audio_path: String,
    reference_text: String,
    consent: bool,
) -> Result<VoiceProfile> {
    let id = strip_clone_prefix(&voice_id).unwrap_or(&voice_id);
    validate_id(id)?;
    if !consent {
        return Err(KoraError::InvalidInput(
            "debes aceptar que solo clonas voces propias o con permiso".into(),
        ));
    }
    let reference_text = reference_text.trim().to_string();
    if reference_text.is_empty() || reference_text.chars().count() > 2000 {
        return Err(KoraError::InvalidInput(
            "escribe lo que dice el nuevo clip de referencia".into(),
        ));
    }
    let audio = PathBuf::from(audio_path);
    if !audio.is_file() {
        return Err(KoraError::NotFound(format!(
            "no se encontró el audio {}",
            audio.display()
        )));
    }

    let owner = state.identity.account.id.clone();
    let profile = state
        .voice_profiles
        .get(id, &owner)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("el perfil {id} no existe")))?;
    let engine = state.neutts.as_ref().ok_or_else(|| {
        KoraError::Unsupported("la clonación de voz no está disponible en esta plataforma".into())
    })?;

    let temporary = state
        .voice_profiles
        .profile_dir(&uuid::Uuid::new_v4().to_string());
    let result = async {
        let encoded = engine
            .encode_reference(
                audio,
                profile.language.clone(),
                reference_text,
                temporary.clone(),
            )
            .await?;
        if !(2_500..=30_500).contains(&encoded.duration_ms) {
            return Err(KoraError::InvalidInput(format!(
                "el clip debe durar entre 3 y 30 segundos; dura {:.1} s",
                encoded.duration_ms as f64 / 1000.0
            )));
        }
        state
            .voice_profiles
            .replace_sample(
                id,
                &owner,
                &temporary,
                &encoded.reference_text,
                encoded.duration_ms,
                encoded.frames,
            )
            .await
    }
    .await;

    let _ = std::fs::remove_dir_all(&temporary);
    result
}

#[tauri::command]
pub async fn export_voice_profile(
    state: State<'_, AppState>,
    voice_id: String,
    destination: String,
) -> Result<()> {
    let id = strip_clone_prefix(&voice_id).unwrap_or(&voice_id);
    let _ = state
        .voice_profiles
        .get(id, &state.identity.account.id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("el perfil {id} no existe")))?;
    state.voice_profiles.export(id, Path::new(&destination))
}

#[tauri::command]
pub async fn import_voice_profile(
    state: State<'_, AppState>,
    source: String,
    consent: bool,
) -> Result<VoiceProfile> {
    if !consent {
        return Err(KoraError::InvalidInput(
            "debes aceptar que solo clonas voces propias o con permiso".into(),
        ));
    }

    let imported = read_archive(Path::new(&source))?;
    engine_for_language(&imported.profile.language)?;
    if imported.profile.name.trim().is_empty() || imported.profile.name.chars().count() > 80 {
        return Err(KoraError::InvalidInput(
            "el perfil importado no tiene un nombre válido".into(),
        ));
    }
    if imported.codes.len() > 5000 * 4 {
        return Err(KoraError::InvalidInput(
            "el perfil importado tiene demasiados códigos".into(),
        ));
    }

    let owner = state.identity.account.id.clone();
    let id = uuid::Uuid::new_v4().to_string();
    let mut profile = imported.profile.clone();
    profile.id = id.clone();
    profile.created_at = Utc::now();

    let directory = state.voice_profiles.profile_dir(&id);
    if directory.exists() {
        return Err(KoraError::Conflict("el perfil ya existe".into()));
    }

    let result = async {
        write_imported(&directory, &imported)?;
        std::fs::write(
            directory.join("profile.json"),
            serde_json::to_vec_pretty(&profile)?,
        )?;
        state.voice_profiles.insert(&profile, &owner).await
    }
    .await;

    match result {
        Ok(saved) => {
            append_consent(
                &state,
                &owner,
                &saved.id,
                "import",
                &saved.name,
                &saved.language,
            )
            .await?;
            Ok(saved)
        }
        Err(error) => {
            let _ = crate::storage::trash::move_to_trash(&state.layout, &directory);
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn voice_clone_preview(
    state: State<'_, AppState>,
    voice_id: String,
    text: String,
) -> Result<VoicePreview> {
    let id = strip_clone_prefix(&voice_id).unwrap_or(&voice_id);
    validate_id(id)?;
    let profile = state
        .voice_profiles
        .get(id, &state.identity.account.id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("el perfil {id} no existe")))?;

    let text = crate::tts::sanitize_speech(&text);
    if text.is_empty() || text.chars().count() > 400 {
        return Err(KoraError::InvalidInput(
            "la vista previa debe tener entre 1 y 400 caracteres".into(),
        ));
    }

    let engine = state.neutts.as_ref().ok_or_else(|| {
        KoraError::Unsupported("la clonación de voz no está disponible en esta plataforma".into())
    })?;

    let chunk = engine
        .synthesize(&SynthesisRequest {
            text,
            voice_id: format!("clone:{id}"),
            rate: 1.0,
            pitch: 1.0,
        })
        .await?;

    let wav = encode_wav_with_watermark(
        &chunk,
        &crate::tts::wav::Watermark {
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
        },
    )?;
    let duration_ms =
        chunk.pcm.len() as u64 * 1000 / (chunk.sample_rate as u64 * chunk.channels.max(1) as u64);

    Ok(VoicePreview {
        audio_base64: base64::engine::general_purpose::STANDARD.encode(wav),
        sample_rate: chunk.sample_rate,
        duration_ms,
    })
}

async fn append_consent(
    state: &AppState,
    owner: &str,
    profile_id: &str,
    source: &str,
    name: &str,
    language: &str,
) -> Result<()> {
    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(owner.to_string()),
            action: "voice.clone.consent".to_string(),
            resource_type: Some(ResourceType::Voice),
            resource_id: Some(profile_id.to_string()),
            metadata: serde_json::json!({
                "consent": CONSENT_TEXT,
                "version": 1,
                "source": source,
                "profile": name,
                "language": language,
            })
            .to_string(),
        })
        .await?;
    Ok(())
}
