use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
#[cfg(not(target_os = "android"))]
use tauri::Emitter;
use tauri::{AppHandle, State};

use crate::audiobooks::engine::audio_path;
use crate::audiobooks::{
    ActiveGenerationStatus, AudioImportOptions, AudioTreatment, AudioWaveform, AudiobookScope,
    CastInput, CastMember, ChapterKaraoke, ChapterSource, KaraokePrecision, ManifestChapter,
    NarrationSettings, PlaybackSnapshot, lines_from_text, m4b, marker_anchor, queue, read_manifest,
    resolve_sections,
};
use crate::audiobooks::{AudiobookProgressEvent, emit_progress, transcribe_existing};
#[cfg(not(target_os = "android"))]
use crate::audiobooks::{
    DEFAULT_WAVEFORM_BUCKETS, MAX_WAVEFORM_BUCKETS, MIN_WAVEFORM_BUCKETS, generate, import,
    import_audio as import_audio_only, reanalyze_audio_chapters, regenerate,
};
use crate::commands::reader::owned_book;
use crate::error::{KoraError, Result};
use crate::reader::SourceText;
use crate::state::AppState;
#[cfg(not(target_os = "android"))]
use crate::storage::NewAudiobookQueueItem;
use crate::storage::{
    AssetKind, AssetRef, Audiobook, AudiobookOrigin, AudiobookProgress, AudiobookStatus, Book,
};
use crate::tts::practice;
use crate::tts::{CharacterInfo, DIALOGUE, ExpressiveSettings, NARRATOR, build_from_text};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookInfo {
    pub id: String,
    pub book_id: String,
    pub book_title: String,
    pub author: Option<String>,
    pub voice_id: Option<String>,
    pub voice_name: Option<String>,
    pub status: AudiobookStatus,
    pub origin: AudiobookOrigin,
    pub duration_ms: i64,
    pub audio_hash: Option<String>,
    pub audio_ext: Option<String>,
    pub scope: Option<AudiobookScope>,
    pub narration: Option<NarrationSettings>,
    pub cast: Vec<CastMember>,
    pub chapter_source: Option<ChapterSource>,
    pub treatment: Option<AudioTreatment>,
    pub chapter_count: usize,
    pub watermark: bool,
    pub cover_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub position_ms: i64,
    pub last_listened_at: Option<DateTime<Utc>>,
    pub percent: f64,
    pub progress_chapter_index: i64,
    pub progress_chapter_title: Option<String>,
    pub progress_section: usize,
    pub progress_offset: usize,
    pub favorite: bool,
    pub audio_only: bool,
    pub category_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookCapabilities {
    pub platform_supported: bool,
    pub audio_import_supported: bool,
    pub native_audio_import: bool,
    pub transcription_supported: bool,
    pub playback_supported: bool,
    pub background_playback: bool,
    pub ffmpeg: bool,
    pub whisper_base: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookQueueView {
    pub id: String,
    pub book_id: String,
    pub book_title: String,
    pub author: Option<String>,
    pub cover_path: Option<String>,
    pub scope: AudiobookScope,
    pub voice_id: String,
    pub voice_name: Option<String>,
    pub narration: Option<NarrationSettings>,
    pub cast: CastInput,
    pub origin: AudiobookOrigin,
    pub position: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookGenerationView {
    pub active: Option<ActiveGenerationStatus>,
    pub queue: Vec<AudiobookQueueView>,
}

#[tauri::command]
pub async fn list_audiobooks(state: State<'_, AppState>) -> Result<Vec<AudiobookInfo>> {
    let owner = state.identity.account.id.clone();
    let rows = state.storage.audiobooks.list_by_owner(&owner).await?;
    let progress_by_audiobook: HashMap<String, AudiobookProgress> = state
        .storage
        .audiobooks
        .list_progress_by_owner(&owner)
        .await?
        .into_iter()
        .map(|progress| (progress.audiobook_id.clone(), progress))
        .collect();
    let mut infos = Vec::with_capacity(rows.len());
    for audiobook in rows {
        if let Some(book) = state.storage.books.get(&audiobook.book_id).await?
            && book.deleted_at.is_some()
        {
            continue;
        }
        infos.push(info_for(&state, &audiobook, progress_by_audiobook.get(&audiobook.id)).await?);
    }
    Ok(infos)
}

#[tauri::command]
pub async fn audiobook_set_favorite(
    state: State<'_, AppState>,
    audiobook_id: String,
    favorite: bool,
) -> Result<AudiobookInfo> {
    let audiobook = owned_audiobook(&state, &audiobook_id).await?;
    state
        .storage
        .audiobooks
        .set_favorite(&state.identity.account.id, &audiobook.id, favorite)
        .await?;
    let progress = state.storage.audiobooks.progress(&audiobook.id).await?;
    info_for(&state, &audiobook, progress.as_ref()).await
}

#[tauri::command]
pub fn audiobook_capabilities(state: State<'_, AppState>) -> AudiobookCapabilities {
    let platform_supported = cfg!(not(target_os = "android"));
    AudiobookCapabilities {
        platform_supported,
        audio_import_supported: true,
        native_audio_import: cfg!(target_os = "android"),
        transcription_supported: true,
        playback_supported: true,
        background_playback: cfg!(target_os = "android"),
        ffmpeg: platform_supported && m4b::ensure_available().is_ok(),
        whisper_base: practice::installed(&state.layout.assets, "base"),
    }
}

#[tauri::command]
pub async fn generate_audiobook(
    state: State<'_, AppState>,
    app: AppHandle,
    book_id: String,
    scope: AudiobookScope,
    voice_id: String,
    narration: Option<NarrationSettings>,
    cast: Option<CastInput>,
) -> Result<AudiobookInfo> {
    #[cfg(target_os = "android")]
    {
        let _ = (&state, &app, &book_id, &scope, &voice_id, &narration, &cast);
        return Err(KoraError::Unsupported(
            "la generación de audiolibros aún no está disponible en Android".into(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        let _guard = state.audiobook_jobs.run_lock.lock().await;
        let title = state
            .storage
            .books
            .get(&book_id)
            .await?
            .map(|book| book.title)
            .unwrap_or_else(|| "Audiolibro".to_string());
        let voice_name = state.tts.voices().await.ok().and_then(|voices| {
            voices
                .into_iter()
                .find(|voice| voice.id == voice_id)
                .map(|voice| voice.name)
        });
        let cast = cast.unwrap_or_default().normalized();
        let narration = narration.unwrap_or_default().clamped();
        let narration = NarrationSettings {
            multi_voice: cast.enabled,
            ..narration
        };
        let control = state.audiobook_jobs.begin(&title, voice_name.as_deref());
        queue::notify_queue(&app);
        let result = generate(
            &state,
            &app,
            &book_id,
            scope,
            &voice_id,
            AudiobookOrigin::Generated,
            narration,
            cast,
            Some(control.as_ref()),
        )
        .await;
        state.audiobook_jobs.finish();
        queue::notify_queue(&app);
        drop(_guard);
        queue::start_worker(app.clone());
        let audiobook = result?;
        info_for(&state, &audiobook, None).await
    }
}

#[tauri::command]
pub async fn enqueue_audiobook(
    state: State<'_, AppState>,
    app: AppHandle,
    book_id: String,
    scope: AudiobookScope,
    voice_id: String,
    narration: Option<NarrationSettings>,
    cast: Option<CastInput>,
) -> Result<AudiobookGenerationView> {
    #[cfg(target_os = "android")]
    {
        let _ = (&state, &app, &book_id, &scope, &voice_id, &narration, &cast);
        return Err(KoraError::Unsupported(
            "la generación de audiolibros aún no está disponible en Android".into(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        if voice_id.trim().is_empty() {
            return Err(KoraError::InvalidInput(
                "elige una voz para generar el audiolibro".into(),
            ));
        }
        let book = state
            .storage
            .books
            .get(&book_id)
            .await?
            .ok_or_else(|| KoraError::NotFound(format!("libro {book_id}")))?;
        if book.owner_account_id != state.identity.account.id {
            return Err(KoraError::NotFound(format!("libro {book_id}")));
        }
        m4b::ensure_available()?;

        let cast = cast.unwrap_or_default().normalized();
        let narration = narration.unwrap_or_default().clamped();
        let narration = NarrationSettings {
            multi_voice: cast.enabled,
            ..narration
        };
        let owner = state.identity.account.id.clone();
        state
            .storage
            .audiobooks
            .enqueue(NewAudiobookQueueItem {
                book_id,
                scope: serde_json::to_string(&scope)?,
                voice_id,
                narration: Some(serde_json::to_string(&narration)?),
                cast: Some(serde_json::to_string(&cast)?),
                origin: AudiobookOrigin::Generated,
                owner_account_id: owner,
            })
            .await?;
        queue::notify_queue(&app);
        queue::start_worker(app.clone());
        generation_view(&state).await
    }
}

#[tauri::command]
pub async fn retry_audiobook(
    state: State<'_, AppState>,
    app: AppHandle,
    audiobook_id: String,
) -> Result<AudiobookInfo> {
    #[cfg(target_os = "android")]
    {
        let _ = (&state, &app, &audiobook_id);
        return Err(KoraError::Unsupported(
            "la generación de audiolibros aún no está disponible en Android".into(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        let audiobook = owned_audiobook(&state, &audiobook_id).await?;
        if !matches!(
            audiobook.status,
            AudiobookStatus::Failed | AudiobookStatus::Cancelled
        ) {
            return Err(KoraError::Conflict(
                "solo se pueden reintentar audiolibros fallidos o cancelados".into(),
            ));
        }
        let title = state
            .storage
            .books
            .get(&audiobook.book_id)
            .await?
            .map(|book| book.title)
            .unwrap_or_else(|| "Audiolibro".to_string());
        let voice_name = if let Some(voice_id) = audiobook.voice_id.as_deref() {
            state
                .tts
                .voices()
                .await
                .ok()
                .and_then(|voices| voices.into_iter().find(|voice| voice.id == voice_id))
                .map(|voice| voice.name)
        } else {
            None
        };
        let narration = audiobook
            .narration
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok())
            .unwrap_or_default();
        let cast = audiobook
            .cast
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok())
            .unwrap_or_default();

        let _guard = state.audiobook_jobs.run_lock.lock().await;
        let control = state.audiobook_jobs.begin(&title, voice_name.as_deref());
        queue::notify_queue(&app);
        let result = regenerate(
            &state,
            &app,
            &audiobook_id,
            narration,
            cast,
            Some(control.as_ref()),
        )
        .await;
        state.audiobook_jobs.finish();
        queue::notify_queue(&app);
        drop(_guard);
        queue::start_worker(app.clone());
        let audiobook = result?;
        info_for(&state, &audiobook, None).await
    }
}

#[tauri::command]
pub async fn audiobook_queue_status(state: State<'_, AppState>) -> Result<AudiobookGenerationView> {
    generation_view(&state).await
}

#[tauri::command]
pub async fn remove_queued_audiobook(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
) -> Result<AudiobookGenerationView> {
    let owner = state.identity.account.id.clone();
    state
        .storage
        .audiobooks
        .remove_queue_item(&id, &owner)
        .await?;
    queue::notify_queue(&app);
    generation_view(&state).await
}

#[tauri::command]
pub async fn move_queued_audiobook(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
    direction: i64,
) -> Result<AudiobookGenerationView> {
    let owner = state.identity.account.id.clone();
    let mut queue = state.storage.audiobooks.list_queue(&owner).await?;
    let Some(index) = queue.iter().position(|item| item.id == id) else {
        return generation_view(&state).await;
    };
    let target = if direction < 0 {
        index.saturating_sub(1)
    } else {
        (index + 1).min(queue.len().saturating_sub(1))
    };
    if target != index && target < queue.len() {
        queue.swap(index, target);
        let ids: Vec<String> = queue.into_iter().map(|item| item.id).collect();
        state.storage.audiobooks.reorder_queue(&owner, &ids).await?;
        queue::notify_queue(&app);
    }
    generation_view(&state).await
}

#[tauri::command]
pub async fn clear_audiobook_queue(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<AudiobookGenerationView> {
    let owner = state.identity.account.id.clone();
    state.storage.audiobooks.clear_queue(&owner).await?;
    queue::notify_queue(&app);
    generation_view(&state).await
}

#[tauri::command]
pub async fn pause_audiobook_generation(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<AudiobookGenerationView> {
    state.audiobook_jobs.pause()?;
    queue::notify_queue(&app);
    generation_view(&state).await
}

#[tauri::command]
pub async fn resume_audiobook_generation(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<AudiobookGenerationView> {
    state.audiobook_jobs.resume()?;
    queue::notify_queue(&app);
    generation_view(&state).await
}

#[tauri::command]
pub async fn cancel_audiobook_generation(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<AudiobookGenerationView> {
    state.audiobook_jobs.cancel()?;
    queue::notify_queue(&app);
    generation_view(&state).await
}

#[tauri::command]
pub async fn import_external_audiobook(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
) -> Result<AudiobookInfo> {
    #[cfg(target_os = "android")]
    {
        let _ = (&state, &app, &path);
        return Err(KoraError::Unsupported(
            "la importación de audiolibros aún no está disponible en Android".into(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        if state.audiobook_jobs.status().is_some() {
            return Err(KoraError::Conflict(
                "espera a que termine la generación actual antes de importar".into(),
            ));
        }
        let _guard = state.audiobook_jobs.run_lock.lock().await;
        if state.audiobook_jobs.status().is_some() {
            return Err(KoraError::Conflict(
                "espera a que termine la generación actual antes de importar".into(),
            ));
        }
        let title = PathBuf::from(&path)
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| "Audiolibro importado".to_string());
        let control = state.audiobook_jobs.begin(&title, Some("Whisper Base"));
        queue::notify_queue(&app);

        let result = import(&state, &app, &path, Some(control.as_ref())).await;
        state.audiobook_jobs.finish();
        queue::notify_queue(&app);
        drop(_guard);

        let audiobook = match result {
            Ok(audiobook) => audiobook,
            Err(error) => {
                let cancelled = matches!(error, KoraError::Cancelled(_));
                emit_progress(
                    &app,
                    AudiobookProgressEvent {
                        audiobook_id: None,
                        phase: if cancelled { "cancelled" } else { "error" }.to_string(),
                        chapter: 0,
                        total_chapters: 0,
                        percent: 0.0,
                        message: if cancelled {
                            "Importación cancelada".to_string()
                        } else {
                            format!("{error:#}")
                        },
                    },
                );
                return Err(error);
            }
        };
        info_for(&state, &audiobook, None).await
    }
}

#[tauri::command]
pub async fn import_audio_audiobook(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
    options: AudioImportOptions,
    metadata: Option<crate::audiobooks::import_audio::AndroidAudioMetadata>,
    transcribe: Option<bool>,
) -> Result<AudiobookInfo> {
    #[cfg(target_os = "android")]
    {
        let transcribe = transcribe.unwrap_or(false);
        let model_id = options.model_id.clone();
        let Some(metadata) = metadata else {
            return Err(KoraError::InvalidInput(
                "faltan los metadatos de audio de Android".into(),
            ));
        };
        if state.audiobook_jobs.status().is_some() {
            return Err(KoraError::Conflict(
                "espera a que termine la tarea actual antes de importar".into(),
            ));
        }
        let _guard = state.audiobook_jobs.run_lock.lock().await;
        if state.audiobook_jobs.status().is_some() {
            return Err(KoraError::Conflict(
                "espera a que termine la tarea actual antes de importar".into(),
            ));
        }
        let title = metadata
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Audiolibro importado")
            .to_string();
        let control = state
            .audiobook_jobs
            .begin(&title, Some("Importación nativa"));
        queue::notify_queue(&app);

        let result = crate::audiobooks::import_audio::import_android(
            &state,
            &app,
            &path,
            options,
            metadata,
            Some(control.as_ref()),
            !transcribe,
        )
        .await;
        let mut audiobook = match result {
            Ok(audiobook) => audiobook,
            Err(error) => {
                state.audiobook_jobs.finish();
                queue::notify_queue(&app);
                drop(_guard);
                eprintln!("Kora: falló la importación de audio en Android: {error:#}");
                return Err(error);
            }
        };
        if transcribe {
            match transcribe_existing(
                &state,
                &app,
                &audiobook.id,
                model_id.as_deref(),
                Some(control.as_ref()),
            )
            .await
            {
                Ok(updated) => audiobook = updated,
                Err(error) => {
                    state.audiobook_jobs.finish();
                    queue::notify_queue(&app);
                    drop(_guard);
                    eprintln!("Kora: falló la transcripción del audio importado: {error:#}");
                    emit_task_error(&app, Some(&audiobook.id), &error, "Transcripción cancelada");
                    return Err(error);
                }
            }
        }
        state.audiobook_jobs.finish();
        queue::notify_queue(&app);
        drop(_guard);
        return info_for(&state, &audiobook, None).await;
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = (metadata, transcribe);
        if state.audiobook_jobs.status().is_some() {
            return Err(KoraError::Conflict(
                "espera a que termine la tarea actual antes de importar".into(),
            ));
        }
        let _guard = state.audiobook_jobs.run_lock.lock().await;
        if state.audiobook_jobs.status().is_some() {
            return Err(KoraError::Conflict(
                "espera a que termine la tarea actual antes de importar".into(),
            ));
        }
        let title = PathBuf::from(&path)
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| "Audiolibro importado".to_string());
        let control = state
            .audiobook_jobs
            .begin(&title, Some("Análisis de capítulos"));
        queue::notify_queue(&app);

        let result = import_audio_only(&state, &app, &path, options, Some(control.as_ref())).await;
        state.audiobook_jobs.finish();
        queue::notify_queue(&app);
        drop(_guard);

        let audiobook = match result {
            Ok(audiobook) => audiobook,
            Err(error) => {
                emit_task_error(&app, None, &error, "Importación cancelada");
                return Err(error);
            }
        };
        info_for(&state, &audiobook, None).await
    }
}

#[tauri::command]
pub async fn reanalyze_audiobook_chapters(
    state: State<'_, AppState>,
    app: AppHandle,
    audiobook_id: String,
    options: AudioImportOptions,
) -> Result<AudiobookInfo> {
    #[cfg(target_os = "android")]
    {
        let _ = (&state, &app, &audiobook_id, &options);
        return Err(KoraError::Unsupported(
            "el análisis de capítulos aún no está disponible en Android".into(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        let audiobook = owned_audiobook(&state, &audiobook_id).await?;
        let book = state
            .storage
            .books
            .get(&audiobook.book_id)
            .await?
            .ok_or_else(|| KoraError::NotFound("libro del audiolibro".into()))?;
        if !book.audio_only {
            return Err(KoraError::Unsupported(
                "solo se pueden reanalizar audiolibros importados sin transcripción".into(),
            ));
        }
        if state.audiobook_jobs.status().is_some() {
            return Err(KoraError::Conflict(
                "espera a que termine la tarea actual antes de reanalizar".into(),
            ));
        }
        let _guard = state.audiobook_jobs.run_lock.lock().await;
        if state.audiobook_jobs.status().is_some() {
            return Err(KoraError::Conflict(
                "espera a que termine la tarea actual antes de reanalizar".into(),
            ));
        }
        let title = format!("Reanalizando «{}»", book.title);
        let control = state
            .audiobook_jobs
            .begin(&title, Some("Análisis de capítulos"));
        queue::notify_queue(&app);
        let result =
            reanalyze_audio_chapters(&state, &app, &audiobook_id, options, Some(control.as_ref()))
                .await;
        state.audiobook_jobs.finish();
        queue::notify_queue(&app);
        drop(_guard);

        let audiobook = match result {
            Ok(audiobook) => audiobook,
            Err(error) => {
                emit_task_error(&app, Some(&audiobook_id), &error, "Análisis cancelado");
                return Err(error);
            }
        };
        let progress = state.storage.audiobooks.progress(&audiobook.id).await?;
        info_for(&state, &audiobook, progress.as_ref()).await
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterEditInput {
    pub start_ms: i64,
    pub end_ms: i64,
    pub title: String,
}

#[tauri::command]
pub async fn update_audiobook_chapters(
    state: State<'_, AppState>,
    audiobook_id: String,
    chapters: Vec<ChapterEditInput>,
) -> Result<AudiobookInfo> {
    if chapters.is_empty() {
        return Err(KoraError::InvalidInput(
            "el audiolibro necesita al menos un capítulo".into(),
        ));
    }
    if chapters.len() > 500 {
        return Err(KoraError::InvalidInput(
            "el audiolibro no puede tener más de 500 capítulos".into(),
        ));
    }
    let mut audiobook = owned_audiobook(&state, &audiobook_id).await?;
    let book = state
        .storage
        .books
        .get(&audiobook.book_id)
        .await?
        .ok_or_else(|| KoraError::NotFound("libro del audiolibro".into()))?;
    if !book.audio_only {
        return Err(KoraError::Unsupported(
            "solo se pueden editar los capítulos de audiolibros sin transcripción".into(),
        ));
    }
    let mut manifest = read_manifest(&state, &audiobook)?;
    let duration = manifest
        .duration_ms
        .max(audiobook.duration_ms.unwrap_or(0))
        .max(1);
    let mut ordered = chapters;
    ordered.sort_by_key(|chapter| chapter.start_ms);

    let mut cursor = 0i64;
    let mut updated = Vec::with_capacity(ordered.len());
    for (index, chapter) in ordered.into_iter().enumerate() {
        let title = chapter.title.trim();
        if title.is_empty() || title.chars().count() > 200 {
            return Err(KoraError::InvalidInput(format!(
                "el título del capítulo {} debe tener entre 1 y 200 caracteres",
                index + 1
            )));
        }
        if cursor >= duration {
            return Err(KoraError::InvalidInput(
                "hay capítulos fuera de la duración del audio".into(),
            ));
        }
        let end = chapter.end_ms.clamp(cursor + 1, duration);
        if end <= cursor {
            return Err(KoraError::InvalidInput(format!(
                "el capítulo {} no tiene una duración válida",
                index + 1
            )));
        }
        updated.push(ManifestChapter {
            index,
            title: title.to_string(),
            start_ms: cursor,
            end_ms: end,
            section: 0,
            offset_start: 0,
            offset_end: 0,
            karaoke: Vec::new(),
        });
        cursor = end;
    }
    if let Some(last) = updated.last_mut() {
        last.end_ms = duration;
    }
    if updated.first().map(|chapter| chapter.start_ms) != Some(0)
        && let Some(first) = updated.first_mut()
    {
        first.start_ms = 0;
    }

    let previous_manifest = (
        audiobook.manifest_hash.clone(),
        audiobook.manifest_ext.clone(),
    );
    manifest.chapters = updated;
    manifest.chapter_source = Some(ChapterSource::Manual);
    manifest.duration_ms = duration;
    let manifest_asset = state.storage.assets.put_bytes(
        AssetKind::Audiobook,
        &serde_json::to_vec_pretty(&manifest)?,
        "json",
    )?;
    audiobook.manifest_hash = Some(manifest_asset.hash);
    audiobook.manifest_ext = Some(manifest_asset.extension);
    audiobook.duration_ms = Some(duration);
    state.storage.audiobooks.update(&audiobook).await?;
    if let (Some(hash), Some(extension)) = previous_manifest {
        discard_audiobook_asset(&state, Some(&hash), Some(&extension)).await?;
    }
    let progress = state.storage.audiobooks.progress(&audiobook.id).await?;
    info_for(&state, &audiobook, progress.as_ref()).await
}

#[tauri::command]
pub async fn transcribe_audiobook_later(
    state: State<'_, AppState>,
    app: AppHandle,
    audiobook_id: String,
    model_id: Option<String>,
) -> Result<AudiobookInfo> {
    let audiobook = owned_audiobook(&state, &audiobook_id).await?;
    let book = state
        .storage
        .books
        .get(&audiobook.book_id)
        .await?
        .ok_or_else(|| KoraError::NotFound("libro del audiolibro".into()))?;
    if !book.audio_only {
        return Err(KoraError::Unsupported(
            "este audiolibro ya tiene una transcripción".into(),
        ));
    }
    if state.audiobook_jobs.status().is_some() {
        return Err(KoraError::Conflict(
            "espera a que termine la tarea actual antes de transcribir".into(),
        ));
    }
    let _guard = state.audiobook_jobs.run_lock.lock().await;
    if state.audiobook_jobs.status().is_some() {
        return Err(KoraError::Conflict(
            "espera a que termine la tarea actual antes de transcribir".into(),
        ));
    }
    let title = format!("Transcribiendo «{}»", book.title);
    let control = state.audiobook_jobs.begin(&title, Some("Whisper"));
    queue::notify_queue(&app);
    let result = transcribe_existing(
        &state,
        &app,
        &audiobook_id,
        model_id.as_deref(),
        Some(control.as_ref()),
    )
    .await;
    state.audiobook_jobs.finish();
    queue::notify_queue(&app);
    drop(_guard);

    let audiobook = match result {
        Ok(audiobook) => audiobook,
        Err(error) => {
            eprintln!("Kora: falló la transcripción: {error:#}");
            emit_task_error(&app, Some(&audiobook_id), &error, "Transcripción cancelada");
            return Err(error);
        }
    };
    let progress = state.storage.audiobooks.progress(&audiobook.id).await?;
    info_for(&state, &audiobook, progress.as_ref()).await
}

fn emit_task_error(
    app: &AppHandle,
    audiobook_id: Option<&str>,
    error: &KoraError,
    cancelled_message: &str,
) {
    let cancelled = matches!(error, KoraError::Cancelled(_));
    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: audiobook_id.map(str::to_string),
            phase: if cancelled { "cancelled" } else { "error" }.to_string(),
            chapter: 0,
            total_chapters: 0,
            percent: 0.0,
            message: if cancelled {
                cancelled_message.to_string()
            } else {
                format!("{error:#}")
            },
        },
    );
}

#[tauri::command]
pub async fn play_audiobook(
    state: State<'_, AppState>,
    audiobook_id: String,
) -> Result<PlaybackSnapshot> {
    let audiobook = owned_audiobook(&state, &audiobook_id).await?;
    if audiobook.status != AudiobookStatus::Done {
        return Err(KoraError::Unsupported(
            "el audiolibro todavía no está listo".into(),
        ));
    }

    let manifest = read_manifest(&state, &audiobook)?;
    let path = audio_path(&state, &audiobook)?;
    let cover = state
        .storage
        .books
        .get(&audiobook.book_id)
        .await?
        .and_then(|book| cover_path(&state, &book));
    let start_ms = state
        .storage
        .audiobooks
        .progress(&audiobook.id)
        .await?
        .map(|progress| progress.position_ms)
        .unwrap_or(0);
    // Si el audiolibro ya se había terminado, «Escuchar» vuelve a empezar.
    let start_ms =
        if manifest.duration_ms > 0 && start_ms >= manifest.duration_ms.saturating_sub(1_000) {
            0
        } else {
            start_ms
        };

    state
        .audiobook_player
        .play(audiobook.id.clone(), &path, manifest, start_ms, cover)
        .map_err(|error| KoraError::Internal(format!("{error:#}")))
}

#[tauri::command]
pub async fn pause_audiobook(state: State<'_, AppState>) -> Result<Option<PlaybackSnapshot>> {
    let snapshot = state
        .audiobook_player
        .pause()
        .map_err(|error| KoraError::Internal(format!("{error:#}")))?;
    persist(&state, &snapshot).await?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn seek_audiobook(
    state: State<'_, AppState>,
    position_ms: i64,
) -> Result<Option<PlaybackSnapshot>> {
    let snapshot = state
        .audiobook_player
        .seek(position_ms)
        .map_err(|error| KoraError::Internal(format!("{error:#}")))?;
    persist(&state, &snapshot).await?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn stop_audiobook(state: State<'_, AppState>) -> Result<Option<PlaybackSnapshot>> {
    let snapshot = state
        .audiobook_player
        .snapshot()
        .map_err(|error| KoraError::Internal(format!("{error:#}")))?;
    persist(&state, &snapshot).await?;
    state
        .audiobook_player
        .stop()
        .map_err(|error| KoraError::Internal(format!("{error:#}")))?;
    Ok(snapshot)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookChapterInfo {
    pub index: usize,
    pub title: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub section: usize,
    pub offset_start: usize,
    pub offset_end: usize,
    pub karaoke: bool,
}

#[tauri::command]
pub async fn audiobook_chapters(
    state: State<'_, AppState>,
    audiobook_id: String,
) -> Result<Vec<AudiobookChapterInfo>> {
    let audiobook = owned_audiobook(&state, &audiobook_id).await?;
    let manifest = read_manifest(&state, &audiobook)?;
    Ok(manifest
        .chapters
        .into_iter()
        .map(|chapter| AudiobookChapterInfo {
            index: chapter.index,
            title: chapter.title,
            start_ms: chapter.start_ms,
            end_ms: chapter.end_ms,
            section: chapter.section,
            offset_start: chapter.offset_start,
            offset_end: chapter.offset_end,
            karaoke: !chapter.karaoke.is_empty(),
        })
        .collect())
}

/// Detecta los personajes del alcance elegido sin sintetizar audio. Se usa en
/// el generador para ofrecer el reparto de voces antes de crear el audiolibro.
#[tauri::command]
pub async fn audiobook_characters(
    state: State<'_, AppState>,
    book_id: String,
    scope: AudiobookScope,
) -> Result<Vec<CharacterInfo>> {
    let book = owned_book(&state, &book_id).await?;
    let sections = resolve_sections(&state, &book, &scope).await?;
    let characters = tokio::task::spawn_blocking(move || {
        let settings = ExpressiveSettings::default();
        let mut counts: HashMap<String, (usize, String)> = HashMap::new();
        for section in sections {
            let utf16_len = section.text.encode_utf16().count();
            let source = SourceText {
                text: section.text,
                utf16_len,
                breaks: Vec::new(),
            };
            let sequence = build_from_text(section.section, &source, &settings);
            for info in sequence.characters {
                let entry = counts
                    .entry(info.name)
                    .or_insert_with(|| (0, String::new()));
                entry.0 += info.lines;
                if entry.1.is_empty() {
                    entry.1 = info.sample;
                }
            }
        }

        let mut characters: Vec<CharacterInfo> = counts
            .into_iter()
            .map(|(name, (lines, sample))| CharacterInfo {
                name,
                lines,
                sample,
            })
            .collect();
        characters.sort_by(|left, right| {
            let reserved = |name: &str| name == NARRATOR || name == DIALOGUE;
            reserved(&left.name)
                .cmp(&reserved(&right.name))
                .then_with(|| right.lines.cmp(&left.lines))
                .then_with(|| left.name.cmp(&right.name))
        });
        characters
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de personajes: {error}")))?;
    Ok(characters)
}

/// Devuelve la línea de tiempo karaoke del capítulo indicado. Los tiempos de
/// las líneas son relativos al inicio del capítulo; `startMs` y `endMs` de la
/// respuesta son absolutos dentro del audiolibro.
#[tauri::command]
pub async fn audiobook_karaoke(
    state: State<'_, AppState>,
    audiobook_id: String,
    chapter_index: usize,
) -> Result<ChapterKaraoke> {
    let audiobook = owned_audiobook(&state, &audiobook_id).await?;
    let manifest = read_manifest(&state, &audiobook)?;
    let Some(chapter) = manifest.chapters.get(chapter_index) else {
        return Err(KoraError::NotFound(format!(
            "el capítulo {chapter_index} no existe"
        )));
    };

    if !chapter.karaoke.is_empty() {
        let precision = if chapter.karaoke.iter().any(|line| !line.words.is_empty()) {
            KaraokePrecision::Aligned
        } else {
            KaraokePrecision::Timed
        };
        return Ok(ChapterKaraoke {
            chapter_index,
            title: chapter.title.clone(),
            start_ms: chapter.start_ms,
            end_ms: chapter.end_ms,
            precision,
            lines: chapter.karaoke.clone(),
        });
    }

    let relative_end = (chapter.end_ms - chapter.start_ms).max(1);
    let text = chapter_text(&state, &audiobook, chapter).await?;
    let lines = text
        .as_deref()
        .map(|text| lines_from_text(text, 0, relative_end))
        .unwrap_or_default();
    Ok(ChapterKaraoke {
        chapter_index,
        title: chapter.title.clone(),
        start_ms: chapter.start_ms,
        end_ms: chapter.end_ms,
        precision: KaraokePrecision::Estimated,
        lines,
    })
}

async fn chapter_text(
    state: &AppState,
    audiobook: &Audiobook,
    chapter: &crate::audiobooks::ManifestChapter,
) -> Result<Option<String>> {
    let Some(book) = state.storage.books.get(&audiobook.book_id).await? else {
        return Ok(None);
    };
    let scope = AudiobookScope::Chapter {
        section: chapter.section,
    };
    let sections = resolve_sections(state, &book, &scope).await?;
    let Some(source) = sections
        .into_iter()
        .find(|section| section.section == chapter.section)
    else {
        return Ok(None);
    };
    let utf16: Vec<u16> = source.text.encode_utf16().collect();
    let start = chapter.offset_start.min(utf16.len());
    let end = chapter.offset_end.clamp(start, utf16.len());
    Ok(Some(String::from_utf16_lossy(&utf16[start..end])))
}

/// Devuelve el punto exacto del texto que corresponde a una posición del
/// audio. El reproductor usa estos offsets para «Abrir texto» y saltar a la
/// frase que está sonando en vez de al inicio del capítulo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookTextLocator {
    pub chapter_index: usize,
    pub section: usize,
    pub offset_start: i64,
    pub offset_end: i64,
    pub quote: String,
}

#[tauri::command]
pub async fn audiobook_locator(
    state: State<'_, AppState>,
    audiobook_id: String,
    position_ms: i64,
) -> Result<AudiobookTextLocator> {
    let audiobook = owned_audiobook(&state, &audiobook_id).await?;
    let manifest = read_manifest(&state, &audiobook)?;
    let anchor = marker_anchor(&manifest, position_ms)
        .ok_or_else(|| KoraError::NotFound("el audiolibro no tiene capítulos".into()))?;
    let mut locator = AudiobookTextLocator {
        chapter_index: anchor.chapter_index,
        section: anchor.section,
        offset_start: anchor.offset_start,
        offset_end: anchor.offset_end,
        quote: anchor.quote.clone(),
    };
    let Some(chapter) = manifest.chapters.get(anchor.chapter_index) else {
        return Ok(locator);
    };
    if chapter.karaoke.is_empty() {
        return Ok(locator);
    }
    let relative = position_ms.saturating_sub(chapter.start_ms).max(0);
    let Ok(Some(text)) = chapter_text(&state, &audiobook, chapter).await else {
        return Ok(locator);
    };
    if let Some((line_quote, start, end)) = locate_karaoke_text(&text, chapter, relative) {
        locator.quote = line_quote;
        locator.offset_start = chapter.offset_start as i64 + start as i64;
        locator.offset_end = chapter.offset_start as i64 + end as i64;
    }
    Ok(locator)
}

/// Busca la línea activa y sus offsets dentro del texto del capítulo. Recorre
/// las líneas en orden para no confundir una frase repetida con una anterior.
fn locate_karaoke_text(
    text: &str,
    chapter: &crate::audiobooks::ManifestChapter,
    relative_ms: i64,
) -> Option<(String, usize, usize)> {
    let active = chapter
        .karaoke
        .iter()
        .position(|line| relative_ms >= line.start_ms && relative_ms < line.end_ms)
        .or_else(|| {
            chapter
                .karaoke
                .iter()
                .rposition(|line| line.start_ms <= relative_ms)
        })
        .unwrap_or(0);
    let mut cursor = 0usize;
    let mut found_active = None;
    for (index, line) in chapter.karaoke.iter().enumerate() {
        let quote: String = line.text.trim().chars().take(280).collect();
        if quote.is_empty() {
            continue;
        }
        let Some(offset) = text[cursor..].find(&quote) else {
            break;
        };
        let start = cursor + offset;
        let end = start + quote.len();
        if index == active {
            found_active = Some((
                quote,
                text[..start].encode_utf16().count(),
                text[..end].encode_utf16().count(),
            ));
            break;
        }
        cursor = end;
    }
    found_active.or_else(|| {
        let line = chapter.karaoke.get(active)?;
        let quote: String = line.text.trim().chars().take(280).collect();
        if quote.is_empty() {
            return None;
        }
        let (byte_start, byte_end) = find_text_offsets(text, &quote)?;
        let start = text[..byte_start].encode_utf16().count();
        let end = text[..byte_end].encode_utf16().count();
        Some((quote, start, end))
    })
}

fn find_text_offsets(text: &str, needle: &str) -> Option<(usize, usize)> {
    let start = text.find(needle)?;
    Some((start, start + needle.len()))
}

#[tauri::command]
pub async fn set_audiobook_speed(
    state: State<'_, AppState>,
    speed: f32,
) -> Result<Option<PlaybackSnapshot>> {
    if !(0.5..=2.5).contains(&speed) {
        return Err(KoraError::InvalidInput(
            "la velocidad debe estar entre 0.5 y 2.5".into(),
        ));
    }
    state
        .audiobook_player
        .set_speed(speed)
        .map_err(|error| KoraError::Internal(format!("{error:#}")))
}

/// Mantiene la pantalla encendida mientras suena el audiolibro. En escritorio
/// es una operación sin efecto; en Android la aplica el plugin nativo.
#[tauri::command]
pub async fn set_audiobook_keep_screen_on(enabled: bool) -> Result<()> {
    crate::audiobooks::player::set_keep_screen_on(enabled)
        .map_err(|error| KoraError::Internal(format!("{error:#}")))
}

/// Programa el temporizador de sueño en el servicio de reproducción. En
/// escritorio la vista mantiene su propio temporizador; en Android debe vivir
/// en el servicio para seguir funcionando con la pantalla apagada.
#[tauri::command]
pub async fn set_audiobook_sleep_timer(minutes: u32) -> Result<()> {
    crate::audiobooks::player::set_sleep_timer(minutes.min(600))
        .map_err(|error| KoraError::Internal(format!("{error:#}")))
}

#[tauri::command]
pub async fn get_audiobook_progress(
    state: State<'_, AppState>,
) -> Result<Option<PlaybackSnapshot>> {
    if let Some(snapshot) = state
        .audiobook_player
        .snapshot()
        .map_err(|error| KoraError::Internal(format!("{error:#}")))?
    {
        if state.audiobook_player.finished() {
            persist(&state, &Some(snapshot.clone())).await?;
            state.audiobook_player.clear_finished();
        }
        return Ok(Some(snapshot));
    }

    let Some(progress) = state
        .storage
        .audiobooks
        .latest_progress_by_owner(&state.identity.account.id)
        .await?
    else {
        return Ok(None);
    };
    let Some(audiobook) = state.storage.audiobooks.get(&progress.audiobook_id).await? else {
        return Ok(None);
    };
    if audiobook.owner_account_id != state.identity.account.id {
        return Ok(None);
    }
    let Ok(manifest) = read_manifest(&state, &audiobook) else {
        return Ok(None);
    };
    let mut snapshot = PlaybackSnapshot::from_manifest(
        audiobook.id.clone(),
        &manifest,
        progress.position_ms,
        false,
    );
    if let Some(book) = state.storage.books.get(&audiobook.book_id).await? {
        snapshot.cover_path = cover_path(&state, &book);
    }
    Ok(Some(snapshot))
}

#[tauri::command]
pub async fn delete_audiobook(state: State<'_, AppState>, audiobook_id: String) -> Result<()> {
    let audiobook = owned_audiobook(&state, &audiobook_id).await?;
    let placeholder_book = state
        .storage
        .books
        .get(&audiobook.book_id)
        .await?
        .filter(|book| book.audio_only);
    if state
        .audiobook_player
        .snapshot()
        .ok()
        .flatten()
        .is_some_and(|snapshot| snapshot.audiobook_id == audiobook.id)
    {
        let _ = state.audiobook_player.stop();
    }
    let voice_markers = state
        .storage
        .voice_notes
        .list_by_audiobook_including_deleted(&audiobook.id)
        .await?;
    let listener = state.voice_marks.clone();
    let _ = tokio::task::spawn_blocking(move || listener.stop()).await;
    state.storage.audiobooks.delete(&audiobook.id).await?;

    for marker in &voice_markers {
        let path = crate::annotations::note_audio_path(state.storage.layout(), &marker.id);
        if path.is_file() {
            crate::storage::trash::move_to_trash(state.storage.layout(), &path)?;
        }
    }

    for (hash, extension) in [
        (
            audiobook.audio_hash.as_deref(),
            audiobook.audio_ext.as_deref(),
        ),
        (
            audiobook.manifest_hash.as_deref(),
            audiobook.manifest_ext.as_deref(),
        ),
    ] {
        let (Some(hash), Some(extension)) = (hash, extension) else {
            continue;
        };
        let reference = AssetRef {
            kind: AssetKind::Audiobook,
            hash: hash.to_string(),
            extension: extension.to_string(),
        };
        if !state
            .storage
            .asset_is_referenced(AssetKind::Audiobook, hash)
            .await?
            && state.storage.assets.exists(&reference)?
        {
            let path = state.storage.assets.path(&reference)?;
            crate::storage::trash::move_to_trash(state.storage.layout(), &path)?;
        }
    }

    if let Some(book) = placeholder_book
        && state
            .storage
            .audiobooks
            .list_by_book(&book.id)
            .await?
            .is_empty()
    {
        state.storage.books.delete(&book.id).await?;
        if state
            .storage
            .books
            .count_by_content_hash(&book.content_hash)
            .await?
            == 0
        {
            let reference = AssetRef {
                kind: AssetKind::GeneratedBook,
                hash: book.content_hash.clone(),
                extension: book.content_ext.clone(),
            };
            if state.storage.assets.exists(&reference)? {
                let path = state.storage.assets.path(&reference)?;
                crate::storage::trash::move_to_trash(state.storage.layout(), &path)?;
            }
        }
        if let (Some(hash), Some(extension)) = (&book.cover_hash, &book.cover_ext)
            && state.storage.books.count_by_cover_hash(hash).await? == 0
        {
            let reference = AssetRef {
                kind: AssetKind::Cover,
                hash: hash.clone(),
                extension: extension.clone(),
            };
            if state.storage.assets.exists(&reference)? {
                let path = state.storage.assets.path(&reference)?;
                crate::storage::trash::move_to_trash(state.storage.layout(), &path)?;
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn export_audiobook(
    state: State<'_, AppState>,
    audiobook_id: String,
    destination: String,
) -> Result<String> {
    let audiobook = owned_audiobook(&state, &audiobook_id).await?;
    if audiobook.status != AudiobookStatus::Done {
        return Err(KoraError::Unsupported(
            "el audiolibro todavía no está listo para exportarse".into(),
        ));
    }

    let source = audio_path(&state, &audiobook)?;
    let destination = PathBuf::from(destination);
    if destination.as_os_str().is_empty() {
        return Err(KoraError::InvalidInput(
            "elige una ruta para guardar el audiolibro".into(),
        ));
    }
    if let Some(parent) = destination.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&source, &destination)?;
    Ok(destination.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn apply_audiobook_treatment(
    state: State<'_, AppState>,
    app: AppHandle,
    audiobook_id: String,
    treatment: AudioTreatment,
) -> Result<AudiobookInfo> {
    #[cfg(target_os = "android")]
    {
        let _ = (&state, &app, &audiobook_id, &treatment);
        return Err(KoraError::Unsupported(
            "el tratamiento de audio aún no está disponible en Android".into(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        let treatment = treatment.clamped();
        if treatment.is_noop() {
            return Err(KoraError::InvalidInput(
                "elige normalización o un timbre distinto".into(),
            ));
        }

        let mut audiobook = owned_audiobook(&state, &audiobook_id).await?;
        if audiobook.status != AudiobookStatus::Done {
            return Err(KoraError::Unsupported(
                "el audiolibro todavía no está listo para tratarse".into(),
            ));
        }
        let _guard = state.audiobook_jobs.run_lock.lock().await;
        let previous_audio = (audiobook.audio_hash.clone(), audiobook.audio_ext.clone());
        let previous_manifest = (
            audiobook.manifest_hash.clone(),
            audiobook.manifest_ext.clone(),
        );
        let mut manifest = read_manifest(&state, &audiobook)?;
        let source = audio_path(&state, &audiobook)?;
        let output = state
            .layout
            .root
            .join("tmp")
            .join(format!("treatment-{}.m4b", uuid::Uuid::new_v4()));
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let duration_ms = audiobook.duration_ms.unwrap_or(0);

        if state
            .audiobook_player
            .snapshot()
            .ok()
            .flatten()
            .map(|snapshot| snapshot.audiobook_id == audiobook.id)
            .unwrap_or(false)
        {
            let _ = state.audiobook_player.stop();
        }

        emit_progress(
            &app,
            AudiobookProgressEvent {
                audiobook_id: Some(audiobook.id.clone()),
                phase: "processing".to_string(),
                chapter: 0,
                total_chapters: 0,
                percent: 70.0,
                message: "Midiendo volumen y aplicando tratamiento…".to_string(),
            },
        );

        let app_handle = app.clone();
        let event_id = audiobook.id.clone();
        let source_task = source.clone();
        let output_task = output.clone();
        let result = tokio::task::spawn_blocking(move || {
            m4b::apply_treatment(
                &source_task,
                &output_task,
                treatment,
                duration_ms,
                |percent| {
                    let _ = app_handle.emit(
                        "audiobook::progress",
                        AudiobookProgressEvent {
                            audiobook_id: Some(event_id.clone()),
                            phase: "processing".to_string(),
                            chapter: 0,
                            total_chapters: 0,
                            percent: 70.0 + percent * 0.28,
                            message: format!("Aplicando tratamiento… {percent:.0}%"),
                        },
                    );
                },
            )
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de tratamiento: {error}")))?;

        if let Err(error) = result {
            let _ = std::fs::remove_file(&output);
            return Err(error.into());
        }

        let probe = m4b::probe(&output).unwrap_or_default();
        let audio_asset = state
            .storage
            .assets
            .put_file(AssetKind::Audiobook, &output, "m4b")?;
        manifest.treatment = Some(treatment);
        if probe.duration_ms > 0 {
            manifest.duration_ms = probe.duration_ms;
        }
        let manifest_asset = state.storage.assets.put_bytes(
            AssetKind::Audiobook,
            &serde_json::to_vec_pretty(&manifest)?,
            "json",
        )?;

        audiobook.audio_hash = Some(audio_asset.hash);
        audiobook.audio_ext = Some(audio_asset.extension);
        audiobook.manifest_hash = Some(manifest_asset.hash);
        audiobook.manifest_ext = Some(manifest_asset.extension);
        if probe.duration_ms > 0 {
            audiobook.duration_ms = Some(probe.duration_ms);
        }
        state.storage.audiobooks.update(&audiobook).await?;
        for (hash, extension) in [previous_audio, previous_manifest] {
            discard_audiobook_asset(&state, hash.as_deref(), extension.as_deref()).await?;
        }

        let _ = std::fs::remove_file(&output);
        emit_progress(
            &app,
            AudiobookProgressEvent {
                audiobook_id: Some(audiobook.id.clone()),
                phase: "done".to_string(),
                chapter: 0,
                total_chapters: 0,
                percent: 100.0,
                message: format!("Tratamiento aplicado a «{}»", manifest.book_title),
            },
        );
        info_for(&state, &audiobook, None).await
    }
}

#[tauri::command]
pub async fn audiobook_waveform(
    state: State<'_, AppState>,
    audiobook_id: String,
    buckets: Option<usize>,
) -> Result<AudioWaveform> {
    #[cfg(target_os = "android")]
    {
        let _ = (&state, &audiobook_id, &buckets);
        return Err(KoraError::Unsupported(
            "la forma de onda aún no está disponible en Android".into(),
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
        let hash = audiobook
            .audio_hash
            .clone()
            .ok_or_else(|| KoraError::NotFound("el audiolibro no tiene audio".into()))?;
        let buckets = buckets
            .unwrap_or(DEFAULT_WAVEFORM_BUCKETS)
            .clamp(MIN_WAVEFORM_BUCKETS, MAX_WAVEFORM_BUCKETS);
        let cache_dir = state.layout.root.join("cache").join("waveforms");
        std::fs::create_dir_all(&cache_dir)?;
        let cache = cache_dir.join(format!("{hash}-{buckets}.json"));
        if let Ok(bytes) = std::fs::read(&cache)
            && let Ok(waveform) = serde_json::from_slice(&bytes)
        {
            return Ok(waveform);
        }

        let path = audio_path(&state, &audiobook)?;
        let waveform = tokio::task::spawn_blocking(move || m4b::compute_waveform(&path, buckets))
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de waveform: {error}")))??;
        if let Ok(bytes) = serde_json::to_vec(&waveform) {
            let temporary =
                cache.with_file_name(format!("{hash}-{buckets}-{}.tmp", uuid::Uuid::new_v4()));
            if std::fs::write(&temporary, bytes).is_ok() {
                let _ = std::fs::rename(&temporary, &cache);
            }
        }
        Ok(waveform)
    }
}

async fn persist(state: &AppState, snapshot: &Option<PlaybackSnapshot>) -> Result<()> {
    let Some(snapshot) = snapshot else {
        return Ok(());
    };
    state
        .storage
        .audiobooks
        .save_progress(
            &snapshot.audiobook_id,
            &state.identity.account.id,
            snapshot.position_ms,
            snapshot.chapter_index as i64,
        )
        .await?;
    Ok(())
}

/// Mueve a la papelera un asset de audiolibro que ya no referencia ninguna
/// fila. Se usa al reemplazar el audio con un tratamiento o al borrar.
pub(crate) async fn discard_audiobook_asset(
    state: &AppState,
    hash: Option<&str>,
    extension: Option<&str>,
) -> Result<()> {
    let (Some(hash), Some(extension)) = (hash, extension) else {
        return Ok(());
    };
    if state
        .storage
        .asset_is_referenced(AssetKind::Audiobook, hash)
        .await?
    {
        return Ok(());
    }
    let reference = AssetRef {
        kind: AssetKind::Audiobook,
        hash: hash.to_string(),
        extension: extension.to_string(),
    };
    if state.storage.assets.exists(&reference)? {
        let path = state.storage.assets.path(&reference)?;
        crate::storage::trash::move_to_trash(state.storage.layout(), &path)?;
    }
    Ok(())
}

async fn owned_audiobook(state: &AppState, id: &str) -> Result<Audiobook> {
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

async fn info_for(
    state: &AppState,
    audiobook: &Audiobook,
    progress: Option<&AudiobookProgress>,
) -> Result<AudiobookInfo> {
    let book = state.storage.books.get(&audiobook.book_id).await?;
    let manifest = read_manifest(state, audiobook).ok();
    let duration_ms = audiobook.duration_ms.unwrap_or(0);
    let position_ms = progress
        .map(|progress| progress.position_ms)
        .unwrap_or(0)
        .max(0);
    let percent = if duration_ms > 0 {
        (position_ms as f64 / duration_ms as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let progress_chapter_index = progress
        .map(|progress| progress.chapter_index.max(0))
        .unwrap_or(0);
    let progress_chapter = progress.and_then(|progress| {
        manifest
            .as_ref()
            .and_then(|manifest| manifest.chapter(progress.chapter_index.max(0) as usize))
    });
    let category_ids = match &book {
        Some(book) => state
            .storage
            .book_categories
            .list_by_book(&book.id)
            .await?
            .into_iter()
            .map(|category| category.id)
            .collect(),
        None => Vec::new(),
    };

    Ok(AudiobookInfo {
        id: audiobook.id.clone(),
        book_id: audiobook.book_id.clone(),
        book_title: book
            .as_ref()
            .map(|book| book.title.clone())
            .or_else(|| {
                manifest
                    .as_ref()
                    .map(|manifest| manifest.book_title.clone())
            })
            .unwrap_or_else(|| "Audiolibro".to_string()),
        author: book.as_ref().and_then(|book| book.author.clone()),
        voice_id: audiobook.voice_id.clone(),
        voice_name: manifest
            .as_ref()
            .map(|manifest| manifest.voice.name.clone()),
        status: audiobook.status,
        origin: audiobook.origin,
        duration_ms,
        audio_hash: audiobook.audio_hash.clone(),
        audio_ext: audiobook.audio_ext.clone(),
        scope: manifest.as_ref().map(|manifest| manifest.scope.clone()),
        narration: manifest.as_ref().and_then(|manifest| manifest.narration),
        cast: manifest
            .as_ref()
            .map(|manifest| manifest.cast.clone())
            .unwrap_or_default(),
        chapter_source: manifest
            .as_ref()
            .and_then(|manifest| manifest.chapter_source),
        treatment: manifest.as_ref().and_then(|manifest| manifest.treatment),
        chapter_count: manifest
            .as_ref()
            .map(|manifest| manifest.chapters.len())
            .unwrap_or(0),
        watermark: manifest
            .as_ref()
            .map(|manifest| manifest.watermark)
            .unwrap_or(false),
        cover_path: book.as_ref().and_then(|book| cover_path(state, book)),
        created_at: audiobook.created_at,
        position_ms,
        last_listened_at: progress.map(|progress| progress.updated_at),
        percent,
        progress_chapter_index,
        progress_chapter_title: progress_chapter.map(|chapter| chapter.title.clone()),
        progress_section: progress_chapter.map(|chapter| chapter.section).unwrap_or(0),
        progress_offset: progress_chapter
            .map(|chapter| chapter.offset_start)
            .unwrap_or(0),
        favorite: state.storage.audiobooks.is_favorite(&audiobook.id).await?,
        audio_only: book.as_ref().map(|book| book.audio_only).unwrap_or(false),
        category_ids,
    })
}

async fn generation_view(state: &AppState) -> Result<AudiobookGenerationView> {
    let owner = state.identity.account.id.clone();
    let rows = state.storage.audiobooks.list_queue(&owner).await?;
    let voices = state.tts.voices().await.unwrap_or_default();
    let voice_names: HashMap<String, String> = voices
        .into_iter()
        .map(|voice| (voice.id, voice.name))
        .collect();

    let mut queue = Vec::with_capacity(rows.len());
    for row in rows {
        let book = state.storage.books.get(&row.book_id).await?;
        let scope: AudiobookScope =
            serde_json::from_str(&row.scope).unwrap_or(AudiobookScope::FullBook);
        let narration = row
            .narration
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok());
        let cast: CastInput = row
            .cast
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok())
            .unwrap_or_default();
        queue.push(AudiobookQueueView {
            id: row.id,
            book_id: row.book_id.clone(),
            book_title: book
                .as_ref()
                .map(|book| book.title.clone())
                .unwrap_or_else(|| "Audiolibro".to_string()),
            author: book.as_ref().and_then(|book| book.author.clone()),
            cover_path: book.as_ref().and_then(|book| cover_path(state, book)),
            scope,
            voice_name: voice_names.get(&row.voice_id).cloned(),
            voice_id: row.voice_id,
            narration,
            cast,
            origin: row.origin,
            position: row.position,
            created_at: row.created_at,
        });
    }

    Ok(AudiobookGenerationView {
        active: state.audiobook_jobs.status(),
        queue,
    })
}

pub(crate) fn cover_path(state: &AppState, book: &Book) -> Option<String> {
    let hash = book.cover_hash.as_deref()?;
    let extension = book.cover_ext.as_deref()?;
    let reference = AssetRef {
        kind: AssetKind::Cover,
        hash: hash.to_string(),
        extension: extension.to_string(),
    };
    state
        .storage
        .assets
        .path(&reference)
        .ok()
        .filter(|path| path.is_file())
        .map(|path| path.to_string_lossy().to_string())
}
