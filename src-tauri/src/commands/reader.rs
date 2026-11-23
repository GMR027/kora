use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::error::{KoraError, Result};
use crate::html::clean_description;
use crate::reader::ocr::{build_and_save_index, read_ocr_page};
use crate::reader::pdf::PdfParser;
use crate::reader::{
    ChapterContent, Document, DocumentFormat, Locator, PageTextLayer, PdfSessionInfo,
    ReaderService, ReaderSession, Section,
};
use crate::state::AppState;
use crate::storage::{
    Annotation, AnnotationKind, AssetKind, AssetRef, AutoIndex, Book, BookFormat, Extract,
    NewAnnotation, NewExtract, NewReaderSettings, NewReadingProgress, OcrStatus, PdfDocumentInfo,
    PdfOcrState, ReaderSettings, ResourceType, Storage,
};

const MAX_ANNOTATION_TEXT: usize = 20_000;
const PDF_COVER_DPI: f32 = 96.0;

#[tauri::command]
pub async fn reader_open(
    state: State<'_, AppState>,
    app: AppHandle,
    book_id: String,
    locator: Option<Locator>,
) -> Result<ReaderSession> {
    let book = owned_book(&state, &book_id).await?;

    let classification = state.classification.clone();
    let classification_config = classification.config().await.unwrap_or_default();
    if classification_config.enabled
        && state
            .storage
            .book_categories
            .list_by_book(&book.id)
            .await?
            .is_empty()
    {
        let handle = app.clone();
        let target = book.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = classification
                .classify_book(Some(&handle), &target, false, false)
                .await
            {
                tracing::warn!(
                    book = %target.id,
                    %error,
                    "no se pudo clasificar el libro al abrirlo"
                );
            }
        });
    }

    let session = match book.format {
        BookFormat::Epub => open_epub(&state, book_id, book, locator).await?,
        BookFormat::Pdf => open_pdf(&state, &app, book_id, book, locator).await?,
    };
    state.sync.handoff(
        &session.book_id,
        session.locator.section,
        session.locator.offset,
        session.progress,
    );
    Ok(session)
}

async fn open_epub(
    state: &AppState,
    book_id: String,
    mut book: Book,
    requested_locator: Option<Locator>,
) -> Result<ReaderSession> {
    let path = book_asset_path(state, &book)?;
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

    changed |= apply_metadata(&mut book, &inspection);

    if changed {
        state.storage.books.update(&book).await?;
    }

    let stored_progress = state.storage.progress.get(&book_id).await?;
    let mut locator = requested_locator
        .or_else(|| {
            stored_progress
                .as_ref()
                .and_then(|progress| serde_json::from_str::<Locator>(&progress.locator).ok())
        })
        .unwrap_or(Locator::START);

    if locator.section >= inspection.sections.len() {
        locator = Locator::START;
    }

    let percent = stored_progress
        .as_ref()
        .map(|progress| progress.percent)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);

    let settings = match state.storage.reader_settings.get(&book_id).await? {
        Some(settings) => settings,
        None => default_reader_settings(state, &book_id).await,
    };

    let document = Document {
        id: book.id.clone(),
        format: DocumentFormat::Epub,
        title: inspection.title.unwrap_or_else(|| book.title.clone()),
        author: inspection.author.or_else(|| book.author.clone()),
        language: inspection.language.or_else(|| book.language.clone()),
        publisher: inspection.publisher.or_else(|| book.publisher.clone()),
        description: inspection
            .description
            .or_else(|| clean_description(book.description.clone())),
        sections: inspection.sections,
    };

    Ok(ReaderSession {
        book_id,
        document,
        locator,
        cover_path: cover_path(state, &book)?,
        progress: percent,
        settings,
        pdf: None,
    })
}

async fn open_pdf(
    state: &AppState,
    app: &AppHandle,
    book_id: String,
    mut book: Book,
    requested_locator: Option<Locator>,
) -> Result<ReaderSession> {
    let prepared = prepare_pdf(state, Some(app.clone()), &book).await?;
    let inspection = &prepared.inspection;

    let mut changed = false;

    if book.cover_hash.is_none() {
        let cover_path = prepared.path.clone();
        if let Ok(Some((bytes, extension))) =
            tokio::task::spawn_blocking(move || render_cover(&cover_path)).await
        {
            let asset = state
                .storage
                .assets
                .put_bytes(AssetKind::Cover, &bytes, extension)?;
            book.cover_hash = Some(asset.hash);
            book.cover_ext = Some(asset.extension);
            changed = true;
        }
    }

    changed |= apply_pdf_metadata(&mut book, inspection);

    if changed {
        state.storage.books.update(&book).await?;
    }

    let stored_progress = state.storage.progress.get(&book_id).await?;
    let mut locator = requested_locator
        .or_else(|| {
            stored_progress
                .as_ref()
                .and_then(|progress| serde_json::from_str::<Locator>(&progress.locator).ok())
        })
        .unwrap_or(Locator::START);

    if locator.section >= inspection.page_count {
        locator = Locator::START;
    }

    let percent = stored_progress
        .as_ref()
        .map(|progress| progress.percent)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);

    let settings = match state.storage.reader_settings.get(&book_id).await? {
        Some(settings) => settings,
        None => default_reader_settings(state, &book_id).await,
    };

    let mut sections = inspection.sections.clone();
    if sections.is_empty()
        && let Some(index) = state.storage.pdf.get_auto_index(&book_id).await?
    {
        sections = index
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| Section {
                id: format!("pdf-auto-{index}"),
                title: Some(entry.title.clone()),
                locator: Locator {
                    section: entry.page,
                    offset: 0,
                },
            })
            .collect();
    }

    let ocr = prepared
        .ocr
        .as_ref()
        .map(|state| {
            (
                status_name(state.status).to_string(),
                state.pages_done.max(0) as usize,
                state.pages_total.max(0) as usize,
            )
        })
        .unwrap_or(("pending".to_string(), 0, inspection.page_count));

    let document = Document {
        id: book.id.clone(),
        format: DocumentFormat::Pdf,
        title: inspection
            .title
            .clone()
            .unwrap_or_else(|| book.title.clone()),
        author: inspection.author.clone().or_else(|| book.author.clone()),
        language: book.language.clone(),
        publisher: book.publisher.clone(),
        description: clean_description(book.description.clone()),
        sections,
    };

    Ok(ReaderSession {
        book_id,
        document,
        locator,
        cover_path: cover_path(state, &book)?,
        progress: percent,
        settings,
        pdf: Some(PdfSessionInfo {
            page_count: inspection.page_count,
            scanned: inspection.scanned,
            has_native_outline: inspection.has_native_outline,
            source_path: prepared.path.display().to_string(),
            ocr_status: ocr.0,
            ocr_pages_done: ocr.1,
            ocr_pages_total: ocr.2,
        }),
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
            book_id: book_id.clone(),
            locator,
            percent,
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await?;

    state
        .sessions
        .track_progress(&book_id, chapter as i64, percent);
    state.sync.position(&book_id, chapter, offset);
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

    let annotation = state
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
        .await?;
    state
        .sync
        .touch(ResourceType::Annotation, &annotation.id)
        .await;
    Ok(annotation)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn reader_add_extract(
    state: State<'_, AppState>,
    app: AppHandle,
    book_id: String,
    chapter_index: i64,
    offset_start: i64,
    offset_end: i64,
    text: String,
    note: Option<String>,
) -> Result<Extract> {
    owned_book(&state, &book_id).await?;

    if chapter_index < 0 || offset_start < 0 || offset_end < offset_start {
        return Err(KoraError::InvalidInput(
            "la posición del extracto no es válida".into(),
        ));
    }

    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(KoraError::InvalidInput("el extracto no tiene texto".into()));
    }
    if text.len() > MAX_ANNOTATION_TEXT {
        return Err(KoraError::InvalidInput(
            "el extracto supera el tamaño máximo".into(),
        ));
    }

    let note = clean_optional(note);
    if note
        .as_ref()
        .is_some_and(|note| note.len() > MAX_ANNOTATION_TEXT)
    {
        return Err(KoraError::InvalidInput(
            "la nota del extracto supera el tamaño máximo".into(),
        ));
    }

    let locator = format!("chapter:{chapter_index}:{offset_start}-{offset_end}");

    let extract = state
        .storage
        .extracts
        .create(NewExtract {
            book_id,
            locator,
            text,
            note,
            chapter_index: Some(chapter_index),
            offset_start: Some(offset_start),
            offset_end: Some(offset_end),
            citation: None,
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await?;
    state.sync.touch(ResourceType::Extract, &extract.id).await;
    crate::commands::annotations::unlock_achievements(&state, &app).await?;
    Ok(extract)
}

#[tauri::command]
pub async fn reader_list_extracts(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<Vec<Extract>> {
    owned_book(&state, &book_id).await?;
    state.storage.extracts.list_by_book(&book_id).await
}

#[tauri::command]
pub async fn list_all_extracts(state: State<'_, AppState>) -> Result<Vec<Extract>> {
    state
        .storage
        .extracts
        .list_by_owner(&state.identity.account.id)
        .await
}

#[tauri::command]
pub async fn reader_delete_extract(state: State<'_, AppState>, extract_id: String) -> Result<()> {
    let extract = state
        .storage
        .extracts
        .get(&extract_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("extracto {extract_id}")))?;
    if extract.owner_account_id != state.identity.account.id {
        return Err(KoraError::NotFound(format!("extracto {extract_id}")));
    }
    state.storage.extracts.delete(&extract_id).await?;
    state.sync.touch(ResourceType::Extract, &extract_id).await;
    Ok(())
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
    state
        .sync
        .touch(ResourceType::Annotation, &annotation.id)
        .await;
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
    state.storage.annotations.delete(&annotation_id).await?;
    state
        .sync
        .touch(ResourceType::Annotation, &annotation_id)
        .await;
    Ok(())
}

#[tauri::command]
pub async fn reader_get_settings(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<ReaderSettings> {
    owned_book(&state, &book_id).await?;

    match state.storage.reader_settings.get(&book_id).await? {
        Some(settings) => Ok(settings),
        None => Ok(default_reader_settings(&state, &book_id).await),
    }
}

#[tauri::command]
pub async fn reader_get_defaults(state: State<'_, AppState>) -> Result<ReaderSettings> {
    Ok(default_reader_settings(&state, "").await)
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
            font_face: settings.font_face,
            column_width: settings.column_width,
            text_align: settings.text_align,
            paragraph_indent: settings.paragraph_indent,
            hyphenate: settings.hyphenate,
            theme: settings.theme,
            texture: settings.texture,
            mode: settings.mode,
            zoom: settings.zoom,
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await
}

#[tauri::command]
pub async fn reader_apply_to_all(
    state: State<'_, AppState>,
    settings: ReaderSettingsInput,
) -> Result<usize> {
    settings.validate()?;
    let encoded = serde_json::to_string(&settings)?;
    state
        .storage
        .settings
        .set("reader.defaults", &encoded)
        .await?;
    let owner = state.identity.account.id.clone();
    let books = state.storage.books.list_by_owner(&owner).await?;
    let mut applied = 0usize;
    for book in books {
        state
            .storage
            .reader_settings
            .save(NewReaderSettings {
                book_id: book.id,
                font_size: settings.font_size,
                line_height: settings.line_height,
                margin: settings.margin,
                font_family: settings.font_family.clone(),
                font_face: settings.font_face.clone(),
                column_width: settings.column_width,
                text_align: settings.text_align.clone(),
                paragraph_indent: settings.paragraph_indent,
                hyphenate: settings.hyphenate,
                theme: settings.theme.clone(),
                texture: settings.texture.clone(),
                mode: settings.mode.clone(),
                zoom: settings.zoom,
                owner_account_id: owner.clone(),
                origin_share_id: None,
            })
            .await?;
        applied += 1;
    }
    Ok(applied)
}

// --- PDF, OCR y auto-índice -------------------------------------------------

#[tauri::command]
pub async fn ocr_pdf(
    state: State<'_, AppState>,
    app: AppHandle,
    book_id: String,
) -> Result<PdfOcrState> {
    let book = owned_book(&state, &book_id).await?;
    ensure_pdf(&book)?;

    let prepared = prepare_pdf(&state, None, &book).await?;

    // El OCR manual funciona también en PDFs con texto (por ejemplo, para
    // mejorar una capa nativa defectuosa). Se reanuda desde cero si estaba
    // marcado como omitido.
    let owner = state.identity.account.id.clone();
    let mut fresh = PdfOcrState::pending(
        book_id.clone(),
        prepared.inspection.page_count as i64,
        owner,
    );
    fresh.status = OcrStatus::Running;
    state.storage.pdf.upsert_ocr_state(&fresh).await?;

    spawn_ocr(
        &state,
        app,
        book_id,
        prepared.path,
        prepared.inspection.page_count,
    )
    .await?;

    Ok(fresh)
}

#[tauri::command]
pub async fn get_ocr_page(
    state: State<'_, AppState>,
    book_id: String,
    page: usize,
) -> Result<PageTextLayer> {
    let book = owned_book(&state, &book_id).await?;
    ensure_pdf(&book)?;

    let info = state.storage.pdf.get_info(&book_id).await?;
    let page_count = info
        .as_ref()
        .map(|info| info.page_count.max(0) as usize)
        .unwrap_or(usize::MAX);

    if page >= page_count {
        return Err(KoraError::InvalidInput(format!(
            "la página {page} está fuera del documento"
        )));
    }

    if let Some(mut layer) = read_ocr_page(&state.storage, &book_id, page)? {
        layer.book_id = book_id;
        return Ok(layer);
    }

    let path = book_asset_path(&state, &book)?;
    let parser = PdfParser::new();
    let task_path = path.clone();
    match tokio::task::spawn_blocking(move || parser.page_text(&task_path, page)).await {
        Ok(Ok(mut layer)) => {
            layer.book_id = book_id;
            Ok(layer)
        }
        _ => {
            // Sin PDFium el visor sigue funcionando con pdf.js; la capa vacía
            // indica que no hay coordenadas disponibles en el backend.
            let (width, height) = page_size(&path, page).unwrap_or((595.0, 842.0));
            Ok(crate::reader::pdf::empty_layer(
                &book_id, page, width, height,
            ))
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfSearchHit {
    pub page: usize,
    pub snippet: String,
}

/// Busca texto en todas las páginas del PDF usando la capa OCR guardada o el
/// texto nativo de PDFium. Devuelve coincidencias con su página para que el
/// visor pueda saltar a ellas.
#[tauri::command]
pub async fn search_pdf(
    state: State<'_, AppState>,
    book_id: String,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<PdfSearchHit>> {
    let book = owned_book(&state, &book_id).await?;
    ensure_pdf(&book)?;

    let query = query.trim().to_lowercase();
    if query.chars().count() < 2 {
        return Ok(Vec::new());
    }

    let path = book_asset_path(&state, &book)?;
    let page_count = match state.storage.pdf.get_info(&book_id).await? {
        Some(info) => info.page_count.max(0) as usize,
        None => {
            let path = path.clone();
            tokio::task::spawn_blocking(move || PdfParser::new().inspect(&path))
                .await
                .map_err(|error| KoraError::Internal(format!("tarea de inspección PDF: {error}")))??
                .page_count
        }
    };
    if page_count == 0 {
        return Ok(Vec::new());
    }

    let storage = state.storage.clone();
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let hits = tokio::task::spawn_blocking(move || -> Result<Vec<PdfSearchHit>> {
        let parser = PdfParser::new();
        let mut texts: Vec<Option<String>> = vec![None; page_count];
        let mut missing = Vec::new();

        for (page, slot) in texts.iter_mut().enumerate() {
            match read_ocr_page(&storage, &book_id, page) {
                Ok(Some(layer)) if !layer.words.is_empty() => {
                    *slot = Some(layer.text());
                }
                _ => missing.push(page),
            }
        }

        if !missing.is_empty()
            && let Ok(layers) = parser.page_texts(&path, &missing)
        {
            for (page, layer) in layers {
                if !layer.words.is_empty() {
                    texts[page] = Some(layer.text());
                }
            }
        }

        let mut hits = Vec::new();
        for (page, text) in texts.into_iter().enumerate() {
            let Some(text) = text else {
                continue;
            };
            let lower = text.to_lowercase();
            for (position, _) in lower.match_indices(&query) {
                let char_position = lower[..position].chars().count();
                let snippet: String = text
                    .chars()
                    .skip(char_position.saturating_sub(60))
                    .take(240)
                    .collect::<String>()
                    .replace(['\n', '\r'], " ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                hits.push(PdfSearchHit { page, snippet });
                if hits.len() >= limit {
                    return Ok(hits);
                }
            }
        }

        Ok(hits)
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de búsqueda PDF: {error}")))??;

    Ok(hits)
}

#[tauri::command]
pub async fn build_auto_index(state: State<'_, AppState>, book_id: String) -> Result<AutoIndex> {
    let book = owned_book(&state, &book_id).await?;
    ensure_pdf(&book)?;

    let path = book_asset_path(&state, &book)?;
    let parser = PdfParser::new();
    build_and_save_index(
        &state.storage,
        &parser,
        &book_id,
        &path,
        &state.identity.account.id,
    )
    .await
}

#[tauri::command]
pub async fn get_auto_index(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<Option<AutoIndex>> {
    owned_book(&state, &book_id).await?;
    state.storage.pdf.get_auto_index(&book_id).await
}

/// Devuelve los bytes del PDF por IPC. Es un plan B por si el protocolo
/// `asset:` no está disponible en alguna plataforma.
#[tauri::command]
pub async fn reader_pdf_bytes(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<tauri::ipc::Response> {
    let book = owned_book(&state, &book_id).await?;
    ensure_pdf(&book)?;

    let path = book_asset_path(&state, &book)?;
    let bytes = std::fs::read(path)?;
    Ok(tauri::ipc::Response::new(bytes))
}

pub(crate) struct PreparedPdf {
    pub path: PathBuf,
    pub inspection: crate::reader::pdf::PdfInspection,
    pub ocr: Option<PdfOcrState>,
}

/// Inspecciona un PDF, guarda sus metadatos y lanza OCR si está escaneado.
/// Se usa al abrir el libro y al importarlo; es idempotente.
pub(crate) async fn prepare_pdf(
    state: &AppState,
    app: Option<AppHandle>,
    book: &Book,
) -> Result<PreparedPdf> {
    let path = book_asset_path(state, book)?;
    let task_path = path.clone();
    let inspection = tokio::task::spawn_blocking(move || PdfParser::new().inspect(&task_path))
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de inspección PDF: {error}")))??;

    let owner = state.identity.account.id.clone();
    let info = PdfDocumentInfo {
        book_id: book.id.clone(),
        page_count: inspection.page_count as i64,
        is_scanned: inspection.scanned,
        has_native_outline: inspection.has_native_outline,
        inspected_at: chrono::Utc::now(),
        owner_account_id: owner.clone(),
        origin_share_id: None,
    };
    state.storage.pdf.upsert_info(&info).await?;

    let mut ocr = state.storage.pdf.get_ocr_state(&book.id).await?;
    if ocr.is_none() {
        let mut fresh =
            PdfOcrState::pending(book.id.clone(), inspection.page_count as i64, owner.clone());
        if !inspection.scanned {
            fresh.status = OcrStatus::Skipped;
            fresh.pages_done = inspection.page_count as i64;
        }
        state.storage.pdf.upsert_ocr_state(&fresh).await?;
        ocr = Some(fresh);
    }

    if let Some(app) = app
        && inspection.scanned
        && ocr.as_ref().is_none_or(|state| !state.status.is_terminal())
    {
        spawn_ocr(
            state,
            app,
            book.id.clone(),
            path.clone(),
            inspection.page_count,
        )
        .await?;
    }

    Ok(PreparedPdf {
        path,
        inspection,
        ocr,
    })
}

pub(crate) async fn spawn_ocr(
    state: &AppState,
    app: AppHandle,
    book_id: String,
    path: PathBuf,
    pages_total: usize,
) -> Result<()> {
    let mut jobs = state.ocr_jobs.lock().await;
    if jobs.contains(&book_id) {
        return Ok(());
    }
    jobs.insert(book_id.clone());
    drop(jobs);

    let storage = state.storage.clone();
    let jobs = state.ocr_jobs.clone();
    let owner = state.identity.account.id.clone();

    tauri::async_runtime::spawn(async move {
        crate::reader::ocr::run_ocr_job(app, storage, jobs, book_id, path, pages_total, owner)
            .await;
    });

    Ok(())
}

pub(crate) async fn owned_book(state: &AppState, book_id: &str) -> Result<Book> {
    let book = state
        .storage
        .books
        .get(book_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("libro {book_id}")))?;

    if book.owner_account_id != state.identity.account.id || book.deleted_at.is_some() {
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

pub(crate) fn ensure_epub(book: &Book) -> Result<()> {
    match book.format {
        BookFormat::Epub => Ok(()),
        BookFormat::Pdf => Err(KoraError::Unsupported(
            "este comando solo aplica a EPUB".into(),
        )),
    }
}

pub(crate) fn ensure_pdf(book: &Book) -> Result<()> {
    match book.format {
        BookFormat::Pdf => Ok(()),
        BookFormat::Epub => Err(KoraError::Unsupported(
            "este comando solo aplica a PDF".into(),
        )),
    }
}

pub(crate) fn book_asset_path(state: &AppState, book: &Book) -> Result<PathBuf> {
    book_asset_path_for(&state.storage, book)
}

/// Variante para tareas en segundo plano que solo tienen `Storage`.
pub(crate) fn book_asset_path_for(storage: &Storage, book: &Book) -> Result<PathBuf> {
    let reference = AssetRef {
        kind: AssetKind::Book,
        hash: book.content_hash.clone(),
        extension: book.content_ext.clone(),
    };

    let path = storage.assets.path(&reference)?;
    if path.is_file() {
        return Ok(path);
    }

    // Los libros generados (Tejido y transcripciones) viven en
    // assets/generated_books/ y no duplican el archivo en assets/books/.
    let generated = storage.assets.path(&AssetRef {
        kind: AssetKind::GeneratedBook,
        hash: book.content_hash.clone(),
        extension: book.content_ext.clone(),
    })?;
    if generated.is_file() {
        return Ok(generated);
    }

    Err(KoraError::NotFound(format!(
        "archivo del libro {}",
        book.id
    )))
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

pub(crate) fn apply_metadata(book: &mut Book, inspection: &crate::reader::EpubInspection) -> bool {
    let mut changed = false;
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

    changed
}

pub(crate) fn apply_pdf_metadata(
    book: &mut Book,
    inspection: &crate::reader::pdf::PdfInspection,
) -> bool {
    let mut changed = false;
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

    changed
}

pub(crate) fn render_cover(path: &Path) -> Option<(Vec<u8>, &'static str)> {
    let parser = PdfParser::new();
    let rendered = parser.render_page(path, 0, PDF_COVER_DPI).ok()?;
    Some((rendered.png, "png"))
}

pub(crate) fn page_size(path: &Path, page: usize) -> Option<(f32, f32)> {
    let document = lopdf::Document::load(path).ok()?;
    let pages = document.get_pages();
    let page_id = pages.get(&(page as u32))?;
    let media_box = document
        .get_dictionary(*page_id)
        .ok()?
        .get(b"MediaBox")
        .ok()?
        .as_array()
        .ok()?;

    let value = |index: usize| -> Option<f32> {
        media_box
            .get(index)
            .and_then(|object| object.as_float().ok())
    };

    let left = value(0).unwrap_or(0.0);
    let bottom = value(1).unwrap_or(0.0);
    let right = value(2).unwrap_or(595.0);
    let top = value(3).unwrap_or(842.0);

    Some(((right - left).abs().max(1.0), (top - bottom).abs().max(1.0)))
}

fn status_name(status: OcrStatus) -> &'static str {
    match status {
        OcrStatus::Pending => "pending",
        OcrStatus::Running => "running",
        OcrStatus::Done => "done",
        OcrStatus::Failed => "failed",
        OcrStatus::Skipped => "skipped",
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn default_reader_settings(state: &AppState, book_id: &str) -> ReaderSettings {
    let mut settings =
        ReaderSettings::defaults(book_id.to_string(), state.identity.account.id.clone());
    if let Ok(Some(raw)) = state.storage.settings.get("reader.defaults").await
        && let Ok(value) = serde_json::from_str::<ReaderSettingsInput>(&raw)
    {
        settings.font_size = value.font_size;
        settings.line_height = value.line_height;
        settings.margin = value.margin;
        settings.font_family = value.font_family;
        settings.font_face = value.font_face;
        settings.column_width = value.column_width;
        settings.text_align = value.text_align;
        settings.paragraph_indent = value.paragraph_indent;
        settings.hyphenate = value.hyphenate;
        settings.theme = value.theme;
        settings.texture = value.texture;
        settings.mode = value.mode;
        settings.zoom = value.zoom;
    }
    settings
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderSettingsInput {
    pub font_size: f64,
    pub line_height: f64,
    pub margin: i64,
    pub font_family: String,
    pub font_face: String,
    pub column_width: i64,
    pub text_align: String,
    pub paragraph_indent: i64,
    pub hyphenate: bool,
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
        if !matches!(
            self.font_face.as_str(),
            "newsreader"
                | "literata"
                | "garamond"
                | "atkinson"
                | "jakarta"
                | "mono"
                | "opendyslexic"
        ) {
            return Err(KoraError::InvalidInput(
                "la familia tipográfica no es válida".into(),
            ));
        }
        if !(520..=1000).contains(&self.column_width) {
            return Err(KoraError::InvalidInput(
                "el ancho de columna debe estar entre 520 y 1000".into(),
            ));
        }
        if !matches!(self.text_align.as_str(), "left" | "justify") {
            return Err(KoraError::InvalidInput(
                "la alineación debe ser left o justify".into(),
            ));
        }
        if !(0..=2).contains(&self.paragraph_indent) {
            return Err(KoraError::InvalidInput(
                "la sangría debe estar entre 0 y 2".into(),
            ));
        }
        if !matches!(
            self.theme.as_str(),
            "light" | "dark" | "sepia" | "contrast" | "inherit"
        ) {
            return Err(KoraError::InvalidInput(
                "el tema debe ser light, dark, sepia, contrast o inherit".into(),
            ));
        }
        if !matches!(
            self.texture.as_str(),
            "none"
                | "paper"
                | "parchment"
                | "soft-dark"
                | "grid"
                | "linen"
                | "dots"
                | "waves"
                | "stripes"
                | "grain"
                | "gingham"
                | "inherit"
        ) {
            return Err(KoraError::InvalidInput(
                "la textura debe ser none, paper, parchment, soft-dark, grid, linen, dots, waves, stripes, grain, gingham o inherit"
                    .into(),
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
