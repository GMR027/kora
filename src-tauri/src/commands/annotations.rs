use std::collections::HashMap;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::annotations::note_audio_path;
use crate::commands::reader::owned_book;
use crate::error::{KoraError, Result};
use crate::state::AppState;
#[cfg(not(target_os = "android"))]
use crate::storage::NewVoiceNote;
use crate::storage::{
    Annotation, AnnotationKind, Extract, NewAnnotation, NewExtract, NewNote, NewReaction, Note,
    Reaction, ReactionKind, ResourceType, VoiceNote,
};

const MAX_TEXT: usize = 20_000;
const MAX_NOTE_JSON: usize = 256_000;
const MAX_CITATION: usize = 500;

pub(crate) async fn unlock_achievements(state: &AppState, app: &AppHandle) -> Result<()> {
    let owner = state.identity.account.id.clone();
    for achievement in state.achievements.evaluate(&owner).await? {
        let _ = app.emit("achievement::unlocked", achievement);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceNoteView {
    #[serde(flatten)]
    pub note: VoiceNote,
    pub audio_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum AnnotationEntry {
    Marker(Annotation),
    Note(Note),
    VoiceNote(VoiceNoteView),
    Extract(Extract),
    Reaction(Reaction),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryAnnotationEntry {
    pub book_id: String,
    pub book_title: String,
    pub entry: AnnotationEntry,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionBucket {
    pub chapter_index: i64,
    pub total: i64,
    pub like: i64,
    pub surprise: i64,
    pub tension: i64,
    pub boredom: i64,
    pub emotion: i64,
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn add_marker(
    state: State<'_, AppState>,
    book_id: String,
    chapter_index: i64,
    offset_start: i64,
    offset_end: i64,
    kind: AnnotationKind,
    color: String,
    text: Option<String>,
    note: Option<String>,
) -> Result<Annotation> {
    owned_book(&state, &book_id).await?;
    validate_range(chapter_index, offset_start, offset_end)?;
    let color = validate_color(&color)?;
    let text = clean_optional(text);
    let note = clean_optional(note);
    validate_text(text.as_deref(), "el texto del marcador")?;
    validate_text(note.as_deref(), "la nota del marcador")?;

    let annotation = state
        .storage
        .annotations
        .create(NewAnnotation {
            book_id,
            kind,
            locator: format!("chapter:{chapter_index}:{offset_start}-{offset_end}"),
            chapter_index: Some(chapter_index),
            offset_start: Some(offset_start),
            offset_end: Some(offset_end),
            text,
            note,
            color: Some(color),
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await?;
    state
        .sync
        .touch(ResourceType::Annotation, &annotation.id)
        .await;
    Ok(annotation)
}

#[tauri::command]
pub async fn update_marker(
    state: State<'_, AppState>,
    annotation_id: String,
    color: String,
    note: Option<String>,
) -> Result<Annotation> {
    let mut annotation = owned_marker(&state, &annotation_id).await?;
    annotation.color = Some(validate_color(&color)?);
    annotation.note = clean_optional(note);
    validate_text(annotation.note.as_deref(), "la nota del marcador")?;
    state.storage.annotations.update(&annotation).await?;
    state
        .sync
        .touch(ResourceType::Annotation, &annotation.id)
        .await;
    Ok(annotation)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn add_note(
    state: State<'_, AppState>,
    app: AppHandle,
    book_id: Option<String>,
    chapter_index: Option<i64>,
    offset_start: Option<i64>,
    offset_end: Option<i64>,
    text: Option<String>,
    title: Option<String>,
    content_json: String,
    plain_text: Option<String>,
) -> Result<Note> {
    if let Some(book_id) = book_id.as_deref() {
        owned_book(&state, book_id).await?;
    }
    if let (Some(chapter), Some(start), Some(end)) = (chapter_index, offset_start, offset_end) {
        validate_range(chapter, start, end)?;
    }
    let body = validate_note(&content_json, plain_text.as_deref())?;
    let title = clean_optional(title);
    validate_text(title.as_deref(), "el título de la nota")?;
    let text = clean_optional(text);
    validate_text(text.as_deref(), "la cita de la nota")?;

    let note = state
        .storage
        .notes
        .create(NewNote {
            book_id,
            title,
            body,
            content_json: Some(content_json),
            chapter_index,
            offset_start,
            offset_end,
            text,
            audio_hash: None,
            audio_ext: None,
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await?;
    state.sync.touch(ResourceType::Note, &note.id).await;
    unlock_achievements(&state, &app).await?;
    Ok(note)
}

#[tauri::command]
pub async fn update_note(
    state: State<'_, AppState>,
    note_id: String,
    title: Option<String>,
    content_json: String,
    plain_text: Option<String>,
) -> Result<Note> {
    let mut note = owned_note(&state, &note_id).await?;
    note.body = validate_note(&content_json, plain_text.as_deref())?;
    note.title = clean_optional(title);
    validate_text(note.title.as_deref(), "el título de la nota")?;
    note.content_json = Some(content_json);
    state.storage.notes.update(&note).await?;
    state.sync.touch(ResourceType::Note, &note.id).await;
    Ok(note)
}

#[tauri::command]
pub async fn start_voice_note_recording(state: State<'_, AppState>) -> Result<()> {
    state.voice_notes.start()
}

#[tauri::command]
pub async fn cancel_voice_note_recording(state: State<'_, AppState>) -> Result<()> {
    state.voice_notes.cancel();
    Ok(())
}

#[tauri::command]
pub async fn add_voice_note(
    state: State<'_, AppState>,
    app: AppHandle,
    book_id: String,
    chapter_index: i64,
    offset_start: i64,
    offset_end: i64,
    text: Option<String>,
) -> Result<VoiceNoteView> {
    owned_book(&state, &book_id).await?;
    validate_range(chapter_index, offset_start, offset_end)?;
    let text = clean_optional(text).unwrap_or_default();
    validate_text(Some(&text), "la cita de la nota de voz")?;

    #[cfg(target_os = "android")]
    {
        let _ = (
            &state,
            &app,
            &book_id,
            chapter_index,
            offset_start,
            offset_end,
            &text,
        );
        return Err(KoraError::Unsupported(
            "las notas de voz aún no están disponibles en Android".into(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        let recording = state.voice_notes.stop()?;
        let id = crate::storage::repos::new_id();
        let path = note_audio_path(&state.layout, &id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if let Err(error) =
            crate::annotations::opus::write_opus(&path, &recording.samples, recording.sample_rate)
        {
            let _ = std::fs::remove_file(&path);
            return Err(KoraError::Internal(format!(
                "no se pudo codificar el Opus: {error:#}"
            )));
        }

        let result = state
            .storage
            .voice_notes
            .create(NewVoiceNote {
                id: Some(id),
                book_id,
                chapter_index,
                offset_start,
                offset_end,
                text,
                duration_ms: recording.duration_ms,
                audio_hash: std::fs::read(&path)
                    .ok()
                    .map(|bytes| crate::storage::hash::sha256_bytes(&bytes)),
                audiobook_id: None,
                position_ms: 0,
                quote: None,
                transcript: None,
                source: "manual".to_string(),
                owner_account_id: state.identity.account.id.clone(),
                origin_share_id: None,
            })
            .await;

        match result {
            Ok(note) => {
                state.sync.touch(ResourceType::VoiceNote, &note.id).await;
                unlock_achievements(&state, &app).await?;
                Ok(voice_note_view(&state, note))
            }
            Err(error) => {
                let _ = std::fs::remove_file(&path);
                Err(error)
            }
        }
    }
}

/// Edita el texto, la transcripción o la cita de una nota de voz. El audio
/// original se conserva.
#[tauri::command]
pub async fn update_voice_note(
    state: State<'_, AppState>,
    note_id: String,
    text: String,
    transcript: Option<String>,
    quote: Option<String>,
) -> Result<VoiceNoteView> {
    let mut note = owned_voice_note(&state, &note_id).await?;
    let text = text.trim().to_string();
    validate_text(Some(&text), "la nota de voz")?;
    note.text = text;
    note.transcript = clean_optional(transcript);
    note.quote = clean_optional(quote);
    validate_text(note.transcript.as_deref(), "la transcripción")?;
    validate_text(note.quote.as_deref(), "la cita")?;

    state.storage.voice_notes.update(&note).await?;
    state.sync.touch(ResourceType::VoiceNote, &note.id).await;
    Ok(voice_note_view(&state, note))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn add_extract(
    state: State<'_, AppState>,
    app: AppHandle,
    book_id: String,
    chapter_index: i64,
    offset_start: i64,
    offset_end: i64,
    text: String,
    note: Option<String>,
    citation: Option<String>,
) -> Result<Extract> {
    let book = owned_book(&state, &book_id).await?;
    validate_range(chapter_index, offset_start, offset_end)?;

    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(KoraError::InvalidInput("el extracto no tiene texto".into()));
    }
    validate_text(Some(&text), "el extracto")?;
    let note = clean_optional(note);
    validate_text(note.as_deref(), "la nota del extracto")?;
    let citation = clean_optional(citation).unwrap_or_else(|| {
        format!(
            "{} · {} {} · {}-{}",
            book.title,
            if book.format == crate::storage::BookFormat::Pdf {
                "página"
            } else {
                "capítulo"
            },
            chapter_index + 1,
            offset_start,
            offset_end
        )
    });
    if citation.chars().count() > MAX_CITATION {
        return Err(KoraError::InvalidInput(
            "la cita del extracto es demasiado larga".into(),
        ));
    }

    let extract = state
        .storage
        .extracts
        .create(NewExtract {
            book_id,
            locator: format!("chapter:{chapter_index}:{offset_start}-{offset_end}"),
            text,
            note,
            chapter_index: Some(chapter_index),
            offset_start: Some(offset_start),
            offset_end: Some(offset_end),
            citation: Some(citation),
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await?;
    state.sync.touch(ResourceType::Extract, &extract.id).await;
    unlock_achievements(&state, &app).await?;
    Ok(extract)
}

#[tauri::command]
pub async fn update_extract(
    state: State<'_, AppState>,
    extract_id: String,
    note: Option<String>,
    citation: Option<String>,
) -> Result<Extract> {
    let mut extract = owned_extract(&state, &extract_id).await?;
    extract.note = clean_optional(note);
    extract.citation = clean_optional(citation);
    validate_text(extract.note.as_deref(), "la nota del extracto")?;
    if extract
        .citation
        .as_deref()
        .is_some_and(|value| value.chars().count() > MAX_CITATION)
    {
        return Err(KoraError::InvalidInput(
            "la cita del extracto es demasiado larga".into(),
        ));
    }
    state.storage.extracts.update(&extract).await?;
    state.sync.touch(ResourceType::Extract, &extract.id).await;
    Ok(extract)
}

#[tauri::command]
pub async fn add_reaction(
    state: State<'_, AppState>,
    book_id: String,
    chapter_index: i64,
    offset_start: i64,
    offset_end: i64,
    kind: ReactionKind,
    text: Option<String>,
) -> Result<Reaction> {
    owned_book(&state, &book_id).await?;
    validate_range(chapter_index, offset_start, offset_end)?;
    let text = clean_optional(text);
    validate_text(text.as_deref(), "la reacción")?;

    let reaction = state
        .storage
        .reactions
        .create(NewReaction {
            book_id,
            chapter_index,
            offset_start,
            offset_end,
            kind,
            text,
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await?;
    state.sync.touch(ResourceType::Reaction, &reaction.id).await;
    Ok(reaction)
}

#[tauri::command]
pub async fn update_reaction(
    state: State<'_, AppState>,
    reaction_id: String,
    kind: ReactionKind,
    text: Option<String>,
) -> Result<Reaction> {
    let mut reaction = owned_reaction(&state, &reaction_id).await?;
    let text = clean_optional(text);
    validate_text(text.as_deref(), "la reacción")?;
    reaction.kind = kind;
    reaction.text = text;
    state.storage.reactions.update(&reaction).await?;
    state.sync.touch(ResourceType::Reaction, &reaction.id).await;
    Ok(reaction)
}

#[tauri::command]
pub async fn list_annotations(
    state: State<'_, AppState>,
    book_id: String,
    kind: Option<String>,
) -> Result<Vec<AnnotationEntry>> {
    owned_book(&state, &book_id).await?;
    let kind = kind.unwrap_or_else(|| "all".to_string());
    let mut entries = Vec::new();

    if kind == "all" || kind == "marker" {
        for marker in state.storage.annotations.list_by_book(&book_id).await? {
            entries.push(AnnotationEntry::Marker(marker));
        }
    }
    if kind == "all" || kind == "note" {
        for note in state.storage.notes.list_by_book(&book_id).await? {
            entries.push(AnnotationEntry::Note(note));
        }
    }
    if kind == "all" || kind == "voice_note" {
        for note in state.storage.voice_notes.list_by_book(&book_id).await? {
            entries.push(AnnotationEntry::VoiceNote(voice_note_view(&state, note)));
        }
    }
    if kind == "all" || kind == "extract" {
        for extract in state.storage.extracts.list_by_book(&book_id).await? {
            entries.push(AnnotationEntry::Extract(extract));
        }
    }
    if kind == "all" || kind == "reaction" {
        for reaction in state.storage.reactions.list_by_book(&book_id).await? {
            entries.push(AnnotationEntry::Reaction(reaction));
        }
    }

    if !matches!(
        kind.as_str(),
        "all" | "marker" | "note" | "voice_note" | "extract" | "reaction"
    ) {
        return Err(KoraError::InvalidInput(format!(
            "tipo de anotación desconocido: {kind}"
        )));
    }

    Ok(entries)
}

/// Devuelve notas, extractos, marcadores y notas de voz de toda la
/// biblioteca, con el título del libro para la vista de libreta global.
#[tauri::command]
pub async fn list_library_annotations(
    state: State<'_, AppState>,
) -> Result<Vec<LibraryAnnotationEntry>> {
    let owner = state.identity.account.id.clone();
    let books = state.storage.books.list_by_owner(&owner).await?;
    let titles: HashMap<String, String> = books
        .into_iter()
        .map(|book| (book.id, book.title))
        .collect();
    let mut entries = Vec::new();

    for marker in state.storage.annotations.list_by_owner(&owner).await? {
        if let Some(title) = titles.get(&marker.book_id) {
            entries.push(LibraryAnnotationEntry {
                book_id: marker.book_id.clone(),
                book_title: title.clone(),
                entry: AnnotationEntry::Marker(marker),
            });
        }
    }
    for note in state.storage.notes.list_by_owner(&owner).await? {
        let Some(book_id) = note.book_id.clone() else {
            continue;
        };
        if let Some(title) = titles.get(&book_id) {
            entries.push(LibraryAnnotationEntry {
                book_id,
                book_title: title.clone(),
                entry: AnnotationEntry::Note(note),
            });
        }
    }
    for extract in state.storage.extracts.list_by_owner(&owner).await? {
        if let Some(title) = titles.get(&extract.book_id) {
            entries.push(LibraryAnnotationEntry {
                book_id: extract.book_id.clone(),
                book_title: title.clone(),
                entry: AnnotationEntry::Extract(extract),
            });
        }
    }
    for note in state.storage.voice_notes.list_by_owner(&owner).await? {
        if let Some(title) = titles.get(&note.book_id) {
            entries.push(LibraryAnnotationEntry {
                book_id: note.book_id.clone(),
                book_title: title.clone(),
                entry: AnnotationEntry::VoiceNote(voice_note_view(&state, note)),
            });
        }
    }
    for reaction in state.storage.reactions.list_by_owner(&owner).await? {
        if let Some(title) = titles.get(&reaction.book_id) {
            entries.push(LibraryAnnotationEntry {
                book_id: reaction.book_id.clone(),
                book_title: title.clone(),
                entry: AnnotationEntry::Reaction(reaction),
            });
        }
    }

    entries.sort_by_key(|item| std::cmp::Reverse(entry_timestamp(&item.entry)));
    Ok(entries)
}

fn entry_timestamp(entry: &AnnotationEntry) -> i64 {
    match entry {
        AnnotationEntry::Marker(value) => value.created_at.timestamp_millis(),
        AnnotationEntry::Note(value) => value.created_at.timestamp_millis(),
        AnnotationEntry::VoiceNote(value) => value.note.created_at.timestamp_millis(),
        AnnotationEntry::Extract(value) => value.created_at.timestamp_millis(),
        AnnotationEntry::Reaction(value) => value.created_at.timestamp_millis(),
    }
}

#[tauri::command]
pub async fn search_extracts(
    state: State<'_, AppState>,
    book_id: Option<String>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<Extract>> {
    if let Some(book_id) = book_id.as_deref() {
        owned_book(&state, book_id).await?;
    }
    state
        .storage
        .extracts
        .search(
            &state.identity.account.id,
            book_id.as_deref(),
            &query,
            limit.unwrap_or(100),
        )
        .await
}

#[tauri::command]
pub async fn reaction_map(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<Vec<ReactionBucket>> {
    owned_book(&state, &book_id).await?;
    let rows = state.storage.reactions.counts_by_book(&book_id).await?;
    let mut buckets: Vec<ReactionBucket> = Vec::new();

    for (chapter_index, kind, total) in rows {
        let bucket = match buckets
            .iter_mut()
            .find(|bucket| bucket.chapter_index == chapter_index)
        {
            Some(bucket) => bucket,
            None => {
                buckets.push(ReactionBucket {
                    chapter_index,
                    total: 0,
                    like: 0,
                    surprise: 0,
                    tension: 0,
                    boredom: 0,
                    emotion: 0,
                });
                buckets.last_mut().expect("bucket recién insertado")
            }
        };
        bucket.total += total;
        match kind {
            ReactionKind::Like => bucket.like += total,
            ReactionKind::Surprise => bucket.surprise += total,
            ReactionKind::Tension => bucket.tension += total,
            ReactionKind::Boredom => bucket.boredom += total,
            ReactionKind::Emotion => bucket.emotion += total,
        }
    }

    buckets.sort_by_key(|bucket| bucket.chapter_index);
    Ok(buckets)
}

#[tauri::command]
pub async fn delete_annotation(state: State<'_, AppState>, kind: String, id: String) -> Result<()> {
    match kind.as_str() {
        "marker" => {
            owned_marker(&state, &id).await?;
            state.storage.annotations.delete(&id).await?;
            state.sync.touch(ResourceType::Annotation, &id).await;
        }
        "note" => {
            owned_note(&state, &id).await?;
            state.storage.notes.delete(&id).await?;
            state.sync.touch(ResourceType::Note, &id).await;
        }
        "voice_note" => {
            owned_voice_note(&state, &id).await?;
            state.storage.voice_notes.delete(&id).await?;
            let path = note_audio_path(&state.layout, &id);
            if path.is_file() {
                crate::storage::trash::move_to_trash(state.storage.layout(), &path)?;
            }
            state.sync.touch(ResourceType::VoiceNote, &id).await;
        }
        "extract" => {
            owned_extract(&state, &id).await?;
            state.storage.extracts.delete(&id).await?;
            state.sync.touch(ResourceType::Extract, &id).await;
        }
        "reaction" => {
            owned_reaction(&state, &id).await?;
            state.storage.reactions.delete(&id).await?;
            state.sync.touch(ResourceType::Reaction, &id).await;
        }
        _ => {
            return Err(KoraError::InvalidInput(format!(
                "tipo de anotación desconocido: {kind}"
            )));
        }
    }
    Ok(())
}

pub(crate) fn voice_note_view(state: &AppState, note: VoiceNote) -> VoiceNoteView {
    let path = note_audio_path(&state.layout, &note.id);
    VoiceNoteView {
        note,
        audio_path: path.is_file().then(|| path.to_string_lossy().to_string()),
    }
}

async fn owned_marker(state: &AppState, id: &str) -> Result<Annotation> {
    let marker = state
        .storage
        .annotations
        .get(id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("anotación {id}")))?;
    if marker.owner_account_id != state.identity.account.id {
        return Err(KoraError::NotFound(format!("anotación {id}")));
    }
    Ok(marker)
}

async fn owned_note(state: &AppState, id: &str) -> Result<Note> {
    let note = state
        .storage
        .notes
        .get(id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("nota {id}")))?;
    if note.owner_account_id != state.identity.account.id {
        return Err(KoraError::NotFound(format!("nota {id}")));
    }
    Ok(note)
}

async fn owned_extract(state: &AppState, id: &str) -> Result<Extract> {
    let extract = state
        .storage
        .extracts
        .get(id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("extracto {id}")))?;
    if extract.owner_account_id != state.identity.account.id {
        return Err(KoraError::NotFound(format!("extracto {id}")));
    }
    Ok(extract)
}

async fn owned_voice_note(state: &AppState, id: &str) -> Result<VoiceNote> {
    let note = state
        .storage
        .voice_notes
        .get(id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("nota de voz {id}")))?;
    if note.owner_account_id != state.identity.account.id {
        return Err(KoraError::NotFound(format!("nota de voz {id}")));
    }
    Ok(note)
}

async fn owned_reaction(state: &AppState, id: &str) -> Result<Reaction> {
    let reaction = state
        .storage
        .reactions
        .get(id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("reacción {id}")))?;
    if reaction.owner_account_id != state.identity.account.id {
        return Err(KoraError::NotFound(format!("reacción {id}")));
    }
    Ok(reaction)
}

fn validate_range(chapter_index: i64, offset_start: i64, offset_end: i64) -> Result<()> {
    if chapter_index < 0 || offset_start < 0 || offset_end < offset_start {
        return Err(KoraError::InvalidInput(
            "la posición de la anotación no es válida".into(),
        ));
    }
    Ok(())
}

fn validate_color(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    let valid = value.len() == 7
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit());
    if valid {
        Ok(value)
    } else {
        Err(KoraError::InvalidInput(
            "el color debe tener el formato #rrggbb".into(),
        ))
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn validate_text(value: Option<&str>, label: &str) -> Result<()> {
    if value.is_some_and(|value| value.len() > MAX_TEXT) {
        return Err(KoraError::InvalidInput(format!(
            "{label} supera el tamaño máximo"
        )));
    }
    Ok(())
}

/// Valida el JSON de Quill y devuelve su texto plano. Si el frontend ya calculó
/// el texto, se usa ese; si no, se extraen los `insert` del Delta.
fn validate_note(content_json: &str, plain_text: Option<&str>) -> Result<String> {
    if content_json.len() > MAX_NOTE_JSON {
        return Err(KoraError::InvalidInput(
            "la nota supera el tamaño máximo".into(),
        ));
    }
    let value: serde_json::Value = serde_json::from_str(content_json)
        .map_err(|_| KoraError::InvalidInput("el contenido de la nota no es JSON válido".into()))?;
    if !value.is_object() {
        return Err(KoraError::InvalidInput(
            "el contenido de la nota debe ser un objeto JSON".into(),
        ));
    }

    let body = plain_text
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| delta_plain_text(&value))
        .unwrap_or_default();

    if body.is_empty() {
        return Err(KoraError::InvalidInput("la nota no tiene contenido".into()));
    }
    validate_text(Some(&body), "la nota")?;
    Ok(body)
}

fn delta_plain_text(value: &serde_json::Value) -> Option<String> {
    let ops = value.get("ops")?.as_array()?;
    let mut text = String::new();
    for op in ops {
        if let Some(insert) = op.get("insert").and_then(|insert| insert.as_str()) {
            text.push_str(insert);
        }
    }
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_six_digit_hex_colors() {
        assert_eq!(validate_color("#FFD166").expect("color"), "#ffd166");
        assert!(validate_color("#fff").is_err());
        assert!(validate_color("red").is_err());
    }

    #[test]
    fn extracts_plain_text_from_a_quill_delta() {
        let delta = r#"{"ops":[{"insert":"Hola "},{"insert":"mundo","attributes":{"bold":true}},{"insert":"\n"}]}"#;
        assert_eq!(validate_note(delta, None).expect("nota"), "Hola mundo");
        assert_eq!(
            validate_note(delta, Some("texto del frontend")).expect("nota"),
            "texto del frontend"
        );
    }

    #[test]
    fn rejects_invalid_or_empty_notes() {
        assert!(validate_note("no es json", None).is_err());
        assert!(validate_note("[]", None).is_err());
        assert!(validate_note(r#"{"ops":[]}"#, None).is_err());
    }

    #[test]
    fn validates_ranges() {
        assert!(validate_range(0, 0, 0).is_ok());
        assert!(validate_range(0, 5, 5).is_ok());
        assert!(validate_range(-1, 0, 0).is_err());
        assert!(validate_range(0, 6, 5).is_err());
    }
}
