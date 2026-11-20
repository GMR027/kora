use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::State;

use crate::error::{KoraError, Result};
use crate::reader::{
    ChapterContent, Document, DocumentFormat, Locator, ReaderService, ReaderSession,
};
use crate::state::AppState;
use crate::storage::{
    Annotation, AnnotationKind, AssetKind, AssetRef, Book, BookFormat, NewAnnotation,
    NewReaderSettings, NewReadingProgress, ReaderSettings,
};

const MAX_ANNOTATION_TEXT: usize = 20_000;

#[tauri::command]
pub async fn reader_open(state: State<'_, AppState>, book_id: String) -> Result<ReaderSession> {
    let mut book = owned_book(&state, &book_id).await?;
    ensure_epub(&book)?;

    let path = book_asset_path(&state, &book)?;
    let task_path = path.clone();
    let inspection =
        tokio::task::spawn_blocking(move || ReaderService::new().inspect_epub(&task_path))
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de lectura: {error}")))??;

    let mut changed = false;

    if book.cover_hash.is_none()
        && let Some(cover) = &inspection.cover
    {
        let asset =
            state
                .storage
                .assets
                .put_bytes(AssetKind::Cover, &cover.bytes, cover.extension)?;
        book.cover_hash = Some(asset.hash);
        book.cover_ext = Some(asset.extension);
        changed = true;
    }

    let file_stem = Path::new(&book.original_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim);

    let title_is_placeholder =
        book.title == "Sin título" || file_stem.is_some_and(|stem| stem == book.title.as_str());

    if title_is_placeholder && let Some(title) = inspection.title.clone() {
        book.title = title;
        changed = true;
    }

    if book.author.is_none()
        && let Some(author) = inspection.author.clone()
    {
        book.author = Some(author);
        changed = true;
    }

    if book.language.is_none()
        && let Some(language) = inspection.language.clone()
    {
        book.language = Some(language);
        changed = true;
    }

    if book.publisher.is_none()
        && let Some(publisher) = inspection.publisher.clone()
    {
        book.publisher = Some(publisher);
        changed = true;
    }

    if book.description.is_none()
        && let Some(description) = inspection.description.clone()
    {
        book.description = Some(description);
        changed = true;
    }

    if changed {
        state.storage.books.update(&book).await?;
    }

    let stored_progress = state.storage.progress.get(&book_id).await?;
    let mut locator = stored_progress
        .as_ref()
        .and_then(|progress| serde_json::from_str::<Locator>(&progress.locator).ok())
        .unwrap_or(Locator::START);

    if locator.section >= inspection.sections.len() {
        locator = Locator::START;
    }

    let percent = stored_progress
        .as_ref()
        .map(|progress| progress.percent)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);

    let settings = state
        .storage
        .reader_settings
        .get(&book_id)
        .await?
        .unwrap_or_else(|| {
            ReaderSettings::defaults(book_id.clone(), state.identity.account.id.clone())
        });

    let document = Document {
        id: book.id.clone(),
        format: DocumentFormat::Epub,
        title: inspection.title.unwrap_or_else(|| book.title.clone()),
        author: inspection.author.or_else(|| book.author.clone()),
        language: inspection.language.or_else(|| book.language.clone()),
        publisher: inspection.publisher.or_else(|| book.publisher.clone()),
        description: inspection.description.or_else(|| book.description.clone()),
        sections: inspection.sections,
    };

    Ok(ReaderSession {
        book_id,
        document,
        locator,
        cover_path: cover_path(&state, &book)?,
        progress: percent,
        settings,
    })
}

#[tauri::command]
pub async fn reader_get_chapter(
    state: State<'_, AppState>,
    book_id: String,
    index: usize,
) -> Result<ChapterContent> {
    let book = owned_book(&state, &book_id).await?;
    ensure_epub(&book)?;

    let path = book_asset_path(&state, &book)?;
    let mut chapter =
        tokio::task::spawn_blocking(move || ReaderService::new().epub_chapter(&path, index))
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de lectura: {error}")))??;

    chapter.book_id = book_id;
    Ok(chapter)
}

#[tauri::command]
pub async fn reader_update_progress(
    state: State<'_, AppState>,
    book_id: String,
    chapter: usize,
    offset: usize,
    percent: f64,
) -> Result<()> {
    owned_book(&state, &book_id).await?;

    if !percent.is_finite() || !(0.0..=1.0).contains(&percent) {
        return Err(KoraError::InvalidInput(
            "el porcentaje debe estar entre 0 y 1".into(),
        ));
    }

    let locator = serde_json::to_string(&Locator {
        section: chapter,
        offset,
    })?;

    state
        .storage
        .progress
        .upsert(NewReadingProgress {
            book_id,
            locator,
            percent,
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await?;

    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn reader_add_annotation(
    state: State<'_, AppState>,
    book_id: String,
    chapter_index: i64,
    offset_start: i64,
    offset_end: i64,
    kind: AnnotationKind,
    color: Option<String>,
    text: Option<String>,
    note: Option<String>,
) -> Result<Annotation> {
    owned_book(&state, &book_id).await?;

    if chapter_index < 0 || offset_start < 0 || offset_end < offset_start {
        return Err(KoraError::InvalidInput(
            "la posición de la anotación no es válida".into(),
        ));
    }

    let text = clean_optional(text);
    let note = clean_optional(note);

    if text
        .as_ref()
        .is_some_and(|text| text.len() > MAX_ANNOTATION_TEXT)
        || note
            .as_ref()
            .is_some_and(|note| note.len() > MAX_ANNOTATION_TEXT)
    {
        return Err(KoraError::InvalidInput(
            "la anotación supera el tamaño máximo".into(),
        ));
    }

    let locator = format!("chapter:{chapter_index}:{offset_start}-{offset_end}");

    state
        .storage
        .annotations
        .create(NewAnnotation {
            book_id,
            kind,
            locator,
            chapter_index: Some(chapter_index),
            offset_start: Some(offset_start),
            offset_end: Some(offset_end),
            text,
            note,
            color: clean_optional(color),
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await
}

#[tauri::command]
pub async fn reader_update_annotation(
    state: State<'_, AppState>,
    annotation_id: String,
    color: Option<String>,
    note: Option<String>,
) -> Result<Annotation> {
    let mut annotation = owned_annotation(&state, &annotation_id).await?;
    annotation.color = clean_optional(color);
    annotation.note = clean_optional(note);

    if annotation
        .note
        .as_ref()
        .is_some_and(|note| note.len() > MAX_ANNOTATION_TEXT)
    {
        return Err(KoraError::InvalidInput(
            "la nota supera el tamaño máximo".into(),
        ));
    }

    state.storage.annotations.update(&annotation).await?;
    Ok(annotation)
}

#[tauri::command]
pub async fn reader_list_annotations(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<Vec<Annotation>> {
    owned_book(&state, &book_id).await?;
    state.storage.annotations.list_by_book(&book_id).await
}

#[tauri::command]
pub async fn reader_delete_annotation(
    state: State<'_, AppState>,
    annotation_id: String,
) -> Result<()> {
    owned_annotation(&state, &annotation_id).await?;
    state.storage.annotations.delete(&annotation_id).await
}

#[tauri::command]
pub async fn reader_get_settings(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<ReaderSettings> {
    owned_book(&state, &book_id).await?;

    Ok(state
        .storage
        .reader_settings
        .get(&book_id)
        .await?
        .unwrap_or_else(|| ReaderSettings::defaults(book_id, state.identity.account.id.clone())))
}

#[tauri::command]
pub async fn reader_save_settings(
    state: State<'_, AppState>,
    book_id: String,
    settings: ReaderSettingsInput,
) -> Result<ReaderSettings> {
    owned_book(&state, &book_id).await?;
    settings.validate()?;

    state
        .storage
        .reader_settings
        .save(NewReaderSettings {
            book_id,
            font_size: settings.font_size,
            line_height: settings.line_height,
            margin: settings.margin,
            font_family: settings.font_family,
            theme: settings.theme,
            texture: settings.texture,
            mode: settings.mode,
            zoom: settings.zoom,
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderSettingsInput {
    pub font_size: f64,
    pub line_height: f64,
    pub margin: i64,
    pub font_family: String,
    pub theme: String,
    pub texture: String,
    pub mode: String,
    pub zoom: f64,
}

impl ReaderSettingsInput {
    fn validate(&self) -> Result<()> {
        if !(12.0..=36.0).contains(&self.font_size) {
            return Err(KoraError::InvalidInput(
                "el tamaño de fuente debe estar entre 12 y 36".into(),
            ));
        }
        if !(1.0..=2.5).contains(&self.line_height) {
            return Err(KoraError::InvalidInput(
                "el interlineado debe estar entre 1.0 y 2.5".into(),
            ));
        }
        if !(0..=160).contains(&self.margin) {
            return Err(KoraError::InvalidInput(
                "el margen debe estar entre 0 y 160".into(),
            ));
        }
        if !matches!(self.font_family.as_str(), "serif" | "sans" | "mono") {
            return Err(KoraError::InvalidInput(
                "la tipografía debe ser serif, sans o mono".into(),
            ));
        }
        if !matches!(self.theme.as_str(), "light" | "dark" | "sepia") {
            return Err(KoraError::InvalidInput(
                "el tema debe ser light, dark o sepia".into(),
            ));
        }
        if !matches!(self.texture.as_str(), "none" | "paper" | "linen" | "grid") {
            return Err(KoraError::InvalidInput(
                "la textura debe ser none, paper, linen o grid".into(),
            ));
        }
        if !matches!(self.mode.as_str(), "scroll" | "paged") {
            return Err(KoraError::InvalidInput(
                "el modo debe ser scroll o paged".into(),
            ));
        }
        if !(0.6..=2.5).contains(&self.zoom) {
            return Err(KoraError::InvalidInput(
                "el zoom debe estar entre 0.6 y 2.5".into(),
            ));
        }

        Ok(())
    }
}

async fn owned_book(state: &AppState, book_id: &str) -> Result<Book> {
    let book = state
        .storage
        .books
        .get(book_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("libro {book_id}")))?;

    if book.owner_account_id != state.identity.account.id {
        return Err(KoraError::NotFound(format!("libro {book_id}")));
    }

    Ok(book)
}

async fn owned_annotation(state: &AppState, annotation_id: &str) -> Result<Annotation> {
    let annotation = state
        .storage
        .annotations
        .get(annotation_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("anotación {annotation_id}")))?;

    if annotation.owner_account_id != state.identity.account.id {
        return Err(KoraError::NotFound(format!("anotación {annotation_id}")));
    }

    Ok(annotation)
}

fn ensure_epub(book: &Book) -> Result<()> {
    match book.format {
        BookFormat::Epub => Ok(()),
        BookFormat::Pdf => Err(KoraError::Unsupported(
            "la lectura de PDF llegará en la fase 4".into(),
        )),
    }
}

fn book_asset_path(state: &AppState, book: &Book) -> Result<PathBuf> {
    let reference = AssetRef {
        kind: AssetKind::Book,
        hash: book.content_hash.clone(),
        extension: book.content_ext.clone(),
    };

    let path = state.storage.assets.path(&reference)?;
    if !path.is_file() {
        return Err(KoraError::NotFound(format!(
            "archivo del libro {}",
            book.id
        )));
    }

    Ok(path)
}

fn cover_path(state: &AppState, book: &Book) -> Result<Option<String>> {
    let (Some(hash), Some(extension)) = (&book.cover_hash, &book.cover_ext) else {
        return Ok(None);
    };

    let reference = AssetRef {
        kind: AssetKind::Cover,
        hash: hash.clone(),
        extension: extension.clone(),
    };

    Ok(Some(
        state.storage.assets.path(&reference)?.display().to_string(),
    ))
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
