use chrono::{DateTime, Utc};
use serde::Serialize;
#[cfg(not(target_os = "android"))]
use tauri::Emitter;
use tauri::{AppHandle, State};

use crate::commands::reader::owned_book;
use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::storage::{NewPracticeAttempt, PracticeAttempt};
use crate::tts::practice::{
    PracticeInput, WHISPER_CATALOG, WordDiff, diff_words, find_model, model_path, transcribe,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeResult {
    pub attempt_id: String,
    pub transcript: String,
    pub accuracy: f64,
    pub diff: Vec<WordDiff>,
    pub duration_ms: u64,
    pub sample_rate: u32,
    pub model: String,
    pub recording_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperModelInfo {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeHistoryEntry {
    pub id: String,
    pub section: usize,
    pub offset_start: i64,
    pub offset_end: i64,
    pub expected: String,
    pub transcript: String,
    pub accuracy: f64,
    pub diff: Vec<WordDiff>,
    pub created_at: DateTime<Utc>,
}

#[cfg(not(target_os = "android"))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub done: bool,
    pub error: Option<String>,
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn practice_start(
    state: State<'_, AppState>,
    book_id: String,
    section: usize,
    offset_start: i64,
    offset_end: i64,
    expected: String,
    language: Option<String>,
) -> Result<()> {
    let book = owned_book(&state, &book_id).await?;
    let expected = expected.trim().to_string();
    if expected.is_empty() {
        return Err(KoraError::InvalidInput(
            "selecciona un pasaje para leer".into(),
        ));
    }
    if expected.chars().count() > 4000 {
        return Err(KoraError::InvalidInput(
            "el pasaje es demasiado largo; elige menos de 4000 caracteres".into(),
        ));
    }
    if offset_end < offset_start {
        return Err(KoraError::InvalidInput(
            "el rango del pasaje no es válido".into(),
        ));
    }

    let language = language
        .filter(|value| matches!(value.as_str(), "es" | "en"))
        .or_else(|| {
            book.language.as_ref().and_then(|value| {
                let value = value.to_lowercase();
                if value.starts_with("es") {
                    Some("es".to_string())
                } else if value.starts_with("en") {
                    Some("en".to_string())
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| "es".to_string());

    let output_dir = state
        .layout
        .assets
        .join("tts")
        .join("practice")
        .join(&book_id);
    state.practice.start(
        PracticeInput {
            book_id,
            section,
            offset_start,
            offset_end,
            expected,
            language,
        },
        output_dir,
    )
}

#[tauri::command]
pub async fn practice_stop(state: State<'_, AppState>, model_id: String) -> Result<PracticeResult> {
    let stopped = state.practice.stop()?;
    if stopped.duration_ms < 300 {
        return Err(KoraError::InvalidInput(
            "la grabación es demasiado corta; vuelve a intentarlo".into(),
        ));
    }

    let model_id = model_id.trim().to_string();
    if find_model(&model_id).is_none() {
        return Err(KoraError::InvalidInput(format!(
            "modelo Whisper desconocido: {model_id}"
        )));
    }

    let model = model_path(&state.layout.assets, &model_id);
    if !model.is_file() {
        return Err(KoraError::Unsupported(
            "el modelo Whisper no está descargado; descárgalo en el panel de ensayo".into(),
        ));
    }

    let samples = stopped.samples.clone();
    let language = stopped.language.clone();
    let transcript = tokio::task::spawn_blocking(move || transcribe(&model, &samples, &language))
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de Whisper: {error}")))??;

    let (diff, accuracy) = diff_words(&stopped.expected, &transcript);
    let attempt_id = uuid::Uuid::new_v4().to_string();

    state
        .storage
        .tts
        .save_practice_attempt(NewPracticeAttempt {
            id: attempt_id.clone(),
            book_id: stopped.book_id,
            section: stopped.section as i64,
            offset_start: stopped.offset_start,
            offset_end: stopped.offset_end,
            expected: stopped.expected,
            transcript: transcript.clone(),
            accuracy,
            diff: serde_json::to_string(&diff)?,
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await?;

    Ok(PracticeResult {
        attempt_id,
        transcript,
        accuracy,
        diff,
        duration_ms: stopped.duration_ms,
        sample_rate: stopped.sample_rate,
        model: model_id,
        recording_path: stopped.wav_path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub fn practice_cancel(state: State<'_, AppState>) -> Result<()> {
    state.practice.cancel()
}

#[tauri::command]
pub fn practice_models(state: State<'_, AppState>) -> Result<Vec<WhisperModelInfo>> {
    Ok(WHISPER_CATALOG
        .iter()
        .map(|model| WhisperModelInfo {
            id: model.id.to_string(),
            name: model.name.to_string(),
            installed: model_path(&state.layout.assets, model.id).is_file(),
            size_bytes: model.size_bytes,
        })
        .collect())
}

#[tauri::command]
pub async fn practice_history(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<Vec<PracticeHistoryEntry>> {
    let _ = owned_book(&state, &book_id).await?;
    let attempts = state.storage.tts.practice_history(&book_id, 50).await?;
    Ok(attempts.into_iter().map(history_entry).collect())
}

fn history_entry(attempt: PracticeAttempt) -> PracticeHistoryEntry {
    PracticeHistoryEntry {
        id: attempt.id,
        section: attempt.section.max(0) as usize,
        offset_start: attempt.offset_start,
        offset_end: attempt.offset_end,
        expected: attempt.expected,
        transcript: attempt.transcript,
        accuracy: attempt.accuracy,
        diff: serde_json::from_str(&attempt.diff).unwrap_or_default(),
        created_at: attempt.created_at,
    }
}

#[tauri::command]
pub async fn practice_download_model(
    state: State<'_, AppState>,
    app: AppHandle,
    model_id: String,
) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        let _ = (&state, &app, &model_id);
        return Err(KoraError::Unsupported(
            "la descarga de modelos Whisper no está disponible en Android".into(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        let spec = find_model(&model_id)
            .ok_or_else(|| KoraError::NotFound(format!("modelo {model_id} desconocido")))?;
        let root = state.layout.assets.clone();
        let destination = model_path(&root, spec.id);
        if destination.is_file() {
            return Ok(());
        }

        std::fs::create_dir_all(crate::tts::whisper_dir(&root))?;
        let temp = destination.with_extension("bin.tmp");
        let app_clone = app.clone();
        let id = spec.id.to_string();
        let url = spec.url;
        let size = spec.size_bytes;

        tokio::task::spawn_blocking(move || {
            let mut last_emit = std::time::Instant::now() - std::time::Duration::from_secs(1);
            let result =
                crate::tts::download::download_file(url, &temp, size, &mut |downloaded, total| {
                    if last_emit.elapsed() >= std::time::Duration::from_millis(150)
                        || downloaded >= total
                    {
                        last_emit = std::time::Instant::now();
                        let _ = app_clone.emit(
                            "tts-practice-download-progress",
                            ModelDownloadProgress {
                                model_id: id.clone(),
                                downloaded,
                                total,
                                done: false,
                                error: None,
                            },
                        );
                    }
                });

            let result = result.and_then(|downloaded| {
                if downloaded < size / 2 {
                    return Err(KoraError::Storage(
                        "el modelo Whisper llegó incompleto".into(),
                    ));
                }
                std::fs::rename(&temp, &destination)?;
                Ok(())
            });

            if result.is_err() {
                let _ = std::fs::remove_file(&temp);
            }

            match &result {
                Ok(()) => {
                    let _ = app_clone.emit(
                        "tts-practice-download-progress",
                        ModelDownloadProgress {
                            model_id: id.clone(),
                            downloaded: 1,
                            total: 1,
                            done: true,
                            error: None,
                        },
                    );
                }
                Err(error) => {
                    let _ = app_clone.emit(
                        "tts-practice-download-progress",
                        ModelDownloadProgress {
                            model_id: id.clone(),
                            downloaded: 0,
                            total: 0,
                            done: true,
                            error: Some(error.to_string()),
                        },
                    );
                }
            }

            result
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de descarga Whisper: {error}")))??;

        Ok(())
    }
}
