use serde::Serialize;
use tauri::{AppHandle, State};
#[cfg(not(target_os = "android"))]
use tauri::{Emitter, Manager};

#[cfg(not(target_os = "android"))]
use crate::audiobooks::{
    AudiobookScope, marker_anchor, match_voice_command, quote_offsets, read_manifest,
    resolve_sections,
};
#[cfg(not(target_os = "android"))]
use crate::commands::annotations::unlock_achievements;
use crate::commands::annotations::{VoiceNoteView, voice_note_view};
#[cfg(not(target_os = "android"))]
use crate::commands::reader::owned_book;
use crate::error::{KoraError, Result};
use crate::state::AppState;
#[cfg(not(target_os = "android"))]
use crate::storage::{AudiobookStatus, NewVoiceNote, ResourceType};
use crate::tts::practice::{self, model_path};

const PHRASE_KEY: &str = "audiobook.voice_marker.phrase";
#[cfg(not(target_os = "android"))]
const DEFAULT_LANGUAGE: &str = "es";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceMarkerStatus {
    pub supported: bool,
    pub listening: bool,
    pub model_id: Option<String>,
    pub model_ready: bool,
    pub phrase: Option<String>,
}

/// Preferimos Whisper Tiny para el comando porque responde antes; Base sirve
/// como respaldo y ya se usa para importar y para el modo ensayo.
fn preferred_model(assets: &std::path::Path) -> Option<(&'static str, std::path::PathBuf)> {
    ["tiny", "base"].into_iter().find_map(|id| {
        practice::installed(assets, id).then(|| {
            let path = model_path(assets, id);
            (id, path)
        })
    })
}

fn validated_phrase(phrase: &str) -> Result<String> {
    let phrase = phrase.trim();
    let words = phrase.split_whitespace().count();
    if words < 2 {
        return Err(KoraError::InvalidInput(
            "el comando de voz necesita al menos dos palabras".into(),
        ));
    }
    if phrase.chars().count() > 80 {
        return Err(KoraError::InvalidInput(
            "el comando de voz no puede superar 80 caracteres".into(),
        ));
    }
    Ok(phrase.to_string())
}

async fn status_for(state: &AppState) -> Result<VoiceMarkerStatus> {
    let phrase = state.storage.settings.get(PHRASE_KEY).await?;
    let model = if cfg!(not(target_os = "android")) {
        preferred_model(&state.layout.assets)
    } else {
        None
    };
    Ok(VoiceMarkerStatus {
        supported: cfg!(not(target_os = "android")),
        listening: state.voice_marks.is_listening(),
        model_id: model.as_ref().map(|(id, _)| (*id).to_string()),
        model_ready: model.is_some(),
        phrase,
    })
}

async fn owned_audiobook(state: &AppState, id: &str) -> Result<crate::storage::Audiobook> {
    let audiobook = state
        .storage
        .audiobooks
        .get(id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("audiolibro {id}")))?;
    if audiobook.owner_account_id != state.identity.account.id {
        return Err(KoraError::NotFound(format!("audiolibro {id}")));
    }
    Ok(audiobook)
}

#[tauri::command]
pub async fn voice_marker_status(state: State<'_, AppState>) -> Result<VoiceMarkerStatus> {
    status_for(&state).await
}

#[tauri::command]
pub async fn save_voice_marker_phrase(
    state: State<'_, AppState>,
    phrase: String,
) -> Result<VoiceMarkerStatus> {
    let phrase = phrase.trim();
    if phrase.is_empty() {
        state.storage.settings.delete(PHRASE_KEY).await?;
    } else {
        let phrase = validated_phrase(phrase)?;
        state.storage.settings.set(PHRASE_KEY, &phrase).await?;
    }
    status_for(&state).await
}

#[tauri::command]
pub async fn audiobook_voice_markers(
    state: State<'_, AppState>,
    audiobook_id: String,
) -> Result<Vec<VoiceNoteView>> {
    let _ = owned_audiobook(&state, &audiobook_id).await?;
    let notes = state
        .storage
        .voice_notes
        .list_by_audiobook(&audiobook_id)
        .await?;
    Ok(notes
        .into_iter()
        .map(|note| voice_note_view(&state, note))
        .collect())
}

#[tauri::command]
pub async fn start_voice_marker_listening(
    state: State<'_, AppState>,
    app: AppHandle,
    audiobook_id: String,
    phrase: String,
    language: Option<String>,
) -> Result<VoiceMarkerStatus> {
    #[cfg(target_os = "android")]
    {
        let _ = (&state, &app, &audiobook_id, &phrase, &language);
        return Err(KoraError::Unsupported(
            "los marcadores de voz aún no están disponibles en Android".into(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        let audiobook = owned_audiobook(&state, &audiobook_id).await?;
        if audiobook.status != AudiobookStatus::Done {
            return Err(KoraError::Unsupported(
                "el audiolibro todavía no está listo".into(),
            ));
        }
        let phrase = validated_phrase(&phrase)?;
        if state.voice_notes.is_recording() {
            return Err(KoraError::Conflict(
                "detén la nota de voz antes de activar los marcadores".into(),
            ));
        }
        if state.practice.is_recording() {
            return Err(KoraError::Conflict(
                "detén la grabación de ensayo antes de activar los marcadores".into(),
            ));
        }
        let (model_id, whisper_path) = preferred_model(&state.layout.assets).ok_or_else(|| {
            KoraError::Unsupported(
                "descarga Whisper Tiny o Base para usar los marcadores de voz".into(),
            )
        })?;
        let snapshot = state
            .audiobook_player
            .snapshot()
            .map_err(|error| KoraError::Internal(format!("{error:#}")))?
            .ok_or_else(|| {
                KoraError::Unsupported(
                    "reproduce el audiolibro antes de activar los marcadores de voz".into(),
                )
            })?;
        if snapshot.audiobook_id != audiobook.id {
            return Err(KoraError::Unsupported(
                "el audiolibro en reproducción no coincide con el marcador".into(),
            ));
        }

        let book = owned_book(&state, &audiobook.book_id).await?;
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
            .unwrap_or_else(|| DEFAULT_LANGUAGE.to_string());

        let listener = state.voice_marks.clone();
        let generation = tokio::task::spawn_blocking(move || listener.start())
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de micrófono: {error}")))??;

        let worker = VoiceMarkerWorker {
            generation,
            audiobook_id: audiobook.id.clone(),
            phrase,
            language,
            model_id: model_id.to_string(),
            model_path: whisper_path,
        };
        tauri::async_runtime::spawn(worker.run(app));
        status_for(&state).await
    }
}

#[tauri::command]
pub async fn stop_voice_marker_listening(state: State<'_, AppState>) -> Result<VoiceMarkerStatus> {
    let listener = state.voice_marks.clone();
    let _ = tokio::task::spawn_blocking(move || listener.stop()).await;
    status_for(&state).await
}

#[cfg(not(target_os = "android"))]
struct VoiceMarkerWorker {
    generation: u64,
    audiobook_id: String,
    phrase: String,
    language: String,
    model_id: String,
    model_path: std::path::PathBuf,
}

#[cfg(not(target_os = "android"))]
impl VoiceMarkerWorker {
    async fn run(self, app: AppHandle) {
        loop {
            let listening = {
                let state = app.state::<AppState>();
                state.voice_marks.is_current(self.generation)
            };
            if !listening {
                break;
            }

            if let Some(error) = {
                let state = app.state::<AppState>();
                state.voice_marks.take_error()
            } {
                let listener = {
                    let state = app.state::<AppState>();
                    state.voice_marks.clone()
                };
                let _ = tokio::task::spawn_blocking(move || listener.stop()).await;
                let _ = app.emit("audiobook::voice_marker_error", error);
                break;
            }

            let utterance = {
                let state = app.state::<AppState>();
                state.voice_marks.try_next(self.generation)
            };
            let Some(utterance) = utterance else {
                tokio::time::sleep(std::time::Duration::from_millis(120)).await;
                continue;
            };

            let position_ms = {
                let state = app.state::<AppState>();
                match state.audiobook_player.snapshot() {
                    Ok(Some(snapshot)) if snapshot.audiobook_id == self.audiobook_id => {
                        if snapshot.playing {
                            (snapshot.position_ms - utterance.duration_ms - 200).max(0)
                        } else {
                            snapshot.position_ms.max(0)
                        }
                    }
                    _ => continue,
                }
            };

            let samples = utterance.samples.clone();
            let model_path = self.model_path.clone();
            let language = self.language.clone();
            let transcript = tokio::task::spawn_blocking(move || {
                practice::transcribe(&model_path, &samples, &language)
            })
            .await;

            let transcript = match transcript {
                Ok(Ok(text)) => text,
                Ok(Err(error)) => {
                    tracing::warn!(%error, "Whisper no pudo revisar el marcador de voz");
                    continue;
                }
                Err(error) => {
                    tracing::warn!(%error, "tarea de Whisper para el marcador de voz");
                    continue;
                }
            };

            let Some(command) = match_voice_command(&transcript, &self.phrase) else {
                continue;
            };

            let still_current = {
                let state = app.state::<AppState>();
                state.voice_marks.is_current(self.generation)
            };
            if !still_current {
                break;
            }

            match create_marker(
                &app,
                &self.audiobook_id,
                position_ms,
                &command.comment,
                &utterance.samples,
                &self.model_id,
            )
            .await
            {
                Ok(view) => {
                    let _ = app.emit("audiobook::voice_marker", view);
                }
                Err(error) => {
                    tracing::warn!(%error, "no se pudo guardar el marcador de voz");
                    let _ = app.emit(
                        "audiobook::voice_marker_error",
                        format!("No se pudo guardar el marcador: {error}"),
                    );
                }
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
async fn create_marker(
    app: &AppHandle,
    audiobook_id: &str,
    position_ms: i64,
    comment: &str,
    samples: &[f32],
    model_id: &str,
) -> Result<VoiceNoteView> {
    let state = app.state::<AppState>();
    let audiobook = owned_audiobook(&state, audiobook_id).await?;
    let book = owned_book(&state, &audiobook.book_id).await?;
    let manifest = read_manifest(&state, &audiobook)?;
    let mut anchor = marker_anchor(&manifest, position_ms)
        .ok_or_else(|| KoraError::Internal("el audiolibro no tiene capítulos".into()))?;

    if let Ok(sections) = resolve_sections(
        &state,
        &book,
        &AudiobookScope::Chapter {
            section: anchor.section,
        },
    )
    .await
        && let Some(section) = sections
            .iter()
            .find(|section| section.section == anchor.section)
        && let Some((start, end)) = quote_offsets(&section.text, &anchor.quote)
    {
        anchor.offset_start = start;
        anchor.offset_end = end;
    }

    let quote = if anchor.quote.trim().is_empty() {
        anchor.chapter_title.clone()
    } else {
        anchor.quote.clone()
    };
    let id = crate::storage::repos::new_id();
    let path = crate::annotations::note_audio_path(&state.layout, &id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Err(error) = crate::annotations::opus::write_opus(&path, samples, 16_000) {
        let _ = std::fs::remove_file(&path);
        return Err(KoraError::Internal(format!(
            "no se pudo codificar el marcador de voz: {error:#}"
        )));
    }
    let audio_hash = std::fs::read(&path)
        .ok()
        .map(|bytes| crate::storage::hash::sha256_bytes(&bytes));
    let duration_ms = samples.len() as i64 * 1000 / 16_000;
    let transcript: String = comment.trim().chars().take(1_000).collect();

    let result = state
        .storage
        .voice_notes
        .create(NewVoiceNote {
            id: Some(id),
            book_id: audiobook.book_id.clone(),
            chapter_index: anchor.section as i64,
            offset_start: anchor.offset_start.max(0),
            offset_end: anchor.offset_end.max(anchor.offset_start.max(0)),
            text: quote.clone(),
            duration_ms,
            audio_hash,
            audiobook_id: Some(audiobook.id.clone()),
            position_ms: position_ms.max(0),
            quote: Some(quote),
            transcript: (!transcript.is_empty()).then(|| transcript.clone()),
            source: "voice_mark".to_string(),
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await;

    let note = match result {
        Ok(note) => note,
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
    };

    state.sync.touch(ResourceType::VoiceNote, &note.id).await;
    let _ = unlock_achievements(&state, app).await;
    tracing::debug!(
        model = %model_id,
        position_ms,
        "marcador de voz guardado"
    );
    Ok(voice_note_view(&state, note))
}
