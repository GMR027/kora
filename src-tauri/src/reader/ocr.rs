use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use crate::error::{KoraError, Result};
use crate::features::ocr::{OcrEngine, OcrResult, default_engine};
use crate::reader::auto_index::build_from_ocr_pages;
use crate::reader::model::{PageTextLayer, TextWord};
use crate::reader::pdf::PdfParser;
use crate::storage::{AutoIndex, OcrStatus, PdfOcrState, Storage};

const OCR_LANGUAGE: &str = "spa+eng";
const OCR_DPI: f32 = 300.0;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrProgress {
    pub book_id: String,
    pub page: Option<usize>,
    pub pages_done: usize,
    pub pages_total: usize,
    pub status: String,
    pub error: Option<String>,
}

/// Ejecuta el OCR de un PDF en segundo plano. Nunca modifica el PDF: escribe
/// una capa JSON por página y actualiza el estado en SQLite.
pub async fn run_ocr_job(
    app: AppHandle,
    storage: Storage,
    jobs: Arc<Mutex<HashSet<String>>>,
    book_id: String,
    path: PathBuf,
    pages_total: usize,
    owner_account_id: String,
) {
    let result = execute(
        &app,
        &storage,
        &book_id,
        &path,
        pages_total,
        &owner_account_id,
    )
    .await;

    if let Err(error) = result {
        let message = error.to_string();
        let mut state = PdfOcrState::pending(
            book_id.clone(),
            pages_total as i64,
            owner_account_id.clone(),
        );
        state.status = OcrStatus::Failed;
        state.error = Some(message.clone());
        state.pages_done = count_ocr_pages(&storage, &book_id, pages_total).unwrap_or(0) as i64;
        let _ = storage.pdf.upsert_ocr_state(&state).await;

        let _ = app.emit(
            "ocr-progress",
            OcrProgress {
                book_id: book_id.clone(),
                page: None,
                pages_done: state.pages_done as usize,
                pages_total,
                status: "failed".to_string(),
                error: Some(message),
            },
        );
    }

    jobs.lock().await.remove(&book_id);
}

async fn execute(
    app: &AppHandle,
    storage: &Storage,
    book_id: &str,
    path: &Path,
    pages_total: usize,
    owner_account_id: &str,
) -> Result<()> {
    let engine: Arc<dyn OcrEngine> = Arc::from(default_engine());
    if !engine.available() {
        return Err(KoraError::Unsupported(
            "el OCR no está disponible en esta plataforma".into(),
        ));
    }

    let mut state = storage
        .pdf
        .get_ocr_state(book_id)
        .await?
        .unwrap_or_else(|| {
            PdfOcrState::pending(
                book_id.to_string(),
                pages_total as i64,
                owner_account_id.to_string(),
            )
        });

    state.status = OcrStatus::Running;
    state.pages_total = pages_total as i64;
    state.language = OCR_LANGUAGE.to_string();
    state.error = None;
    storage.pdf.upsert_ocr_state(&state).await?;

    emit(
        app,
        book_id,
        None,
        state.pages_done as usize,
        pages_total,
        "running",
        None,
    );

    let parser = PdfParser::new();
    let mut done = 0usize;

    for page in 0..pages_total {
        if let Some(existing) = read_ocr_page(storage, book_id, page)?
            && existing.source == "ocr"
            && !existing.words.is_empty()
        {
            done += 1;
            state.pages_done = done as i64;
            state.updated_at = chrono::Utc::now();
            storage.pdf.upsert_ocr_state(&state).await?;
            emit(app, book_id, Some(page), done, pages_total, "running", None);
            continue;
        }

        let parser = parser.clone();
        let path = path.to_path_buf();
        let rendered =
            tokio::task::spawn_blocking(move || parser.render_page(&path, page, OCR_DPI))
                .await
                .map_err(|error| KoraError::Internal(format!("tarea de rasterizado: {error}")))??;

        let engine = engine.clone();
        let png = rendered.png.clone();
        let ocr = tokio::task::spawn_blocking(move || engine.recognize(&png, OCR_LANGUAGE))
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de OCR: {error}")))??;

        let layer = layer_from_ocr(
            book_id,
            page,
            rendered.width,
            rendered.height,
            rendered.point_per_pixel,
            ocr,
        );

        write_ocr_page(storage, book_id, page, &layer)?;

        done += 1;
        state.pages_done = done as i64;
        state.updated_at = chrono::Utc::now();
        storage.pdf.upsert_ocr_state(&state).await?;
        emit(app, book_id, Some(page), done, pages_total, "running", None);
    }

    state.status = OcrStatus::Done;
    state.pages_done = done as i64;
    state.updated_at = chrono::Utc::now();
    storage.pdf.upsert_ocr_state(&state).await?;

    // Aprovecha el OCR para generar el índice si el PDF no traía bookmarks.
    if let Ok(info) = storage.pdf.get_info(book_id).await
        && info.is_some_and(|info| info.is_scanned)
        && storage.pdf.get_auto_index(book_id).await?.is_none()
    {
        let _ = build_and_save_index(storage, &parser, book_id, path, owner_account_id).await;
    }

    emit(app, book_id, None, done, pages_total, "done", None);
    Ok(())
}

fn emit(
    app: &AppHandle,
    book_id: &str,
    page: Option<usize>,
    pages_done: usize,
    pages_total: usize,
    status: &str,
    error: Option<String>,
) {
    let _ = app.emit(
        "ocr-progress",
        OcrProgress {
            book_id: book_id.to_string(),
            page,
            pages_done,
            pages_total,
            status: status.to_string(),
            error,
        },
    );
}

fn layer_from_ocr(
    book_id: &str,
    page: usize,
    width_px: u32,
    height_px: u32,
    point_per_pixel: f32,
    ocr: OcrResult,
) -> PageTextLayer {
    let words = ocr
        .words
        .into_iter()
        .map(|word| TextWord {
            text: word.text,
            x: word.x * point_per_pixel,
            y: word.y * point_per_pixel,
            width: word.width * point_per_pixel,
            height: word.height * point_per_pixel,
            font_size: (word.height * point_per_pixel).max(1.0),
            confidence: Some(word.confidence),
        })
        .collect::<Vec<_>>();

    PageTextLayer {
        book_id: book_id.to_string(),
        page,
        width: width_px as f32 * point_per_pixel,
        height: height_px as f32 * point_per_pixel,
        source: "ocr".to_string(),
        words,
    }
}

pub fn ocr_dir(storage: &Storage, book_id: &str) -> Result<PathBuf> {
    crate::storage::paths::validate_id(book_id)?;
    Ok(storage.layout().assets.join("ocr").join(book_id))
}

pub fn ocr_page_path(storage: &Storage, book_id: &str, page: usize) -> Result<PathBuf> {
    Ok(ocr_dir(storage, book_id)?.join(format!("page_{page}.json")))
}

pub fn read_ocr_page(
    storage: &Storage,
    book_id: &str,
    page: usize,
) -> Result<Option<PageTextLayer>> {
    let path = ocr_page_path(storage, book_id, page)?;
    if !path.is_file() {
        return Ok(None);
    }

    let data = std::fs::read(&path)?;
    let layer: PageTextLayer = serde_json::from_slice(&data).map_err(|error| {
        KoraError::Storage(format!("capa OCR corrupta en {}: {error}", path.display()))
    })?;

    Ok(Some(layer))
}

pub fn write_ocr_page(
    storage: &Storage,
    book_id: &str,
    page: usize,
    layer: &PageTextLayer,
) -> Result<()> {
    let directory = ocr_dir(storage, book_id)?;
    std::fs::create_dir_all(&directory)?;

    let path = ocr_page_path(storage, book_id, page)?;
    let temporary = directory.join(format!(".tmp-{}-{page}", uuid::Uuid::new_v4()));
    let data = serde_json::to_vec(layer)?;

    if let Err(error) = std::fs::write(&temporary, data) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.into());
    }

    if let Err(error) = std::fs::rename(&temporary, &path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.into());
    }

    Ok(())
}

pub fn count_ocr_pages(storage: &Storage, book_id: &str, pages_total: usize) -> Result<usize> {
    let mut count = 0;
    for page in 0..pages_total {
        if read_ocr_page(storage, book_id, page)?.is_some_and(|layer| !layer.words.is_empty()) {
            count += 1;
        }
    }
    Ok(count)
}

pub fn list_ocr_pages(
    storage: &Storage,
    book_id: &str,
    pages_total: usize,
) -> Result<Vec<PageTextLayer>> {
    let mut pages = Vec::new();
    for page in 0..pages_total {
        if let Some(layer) = read_ocr_page(storage, book_id, page)?
            && !layer.words.is_empty()
        {
            pages.push(layer);
        }
    }
    Ok(pages)
}

/// Genera el índice del PDF: bookmarks nativos, OCR o heurística de texto.
pub async fn build_and_save_index(
    storage: &Storage,
    parser: &PdfParser,
    book_id: &str,
    path: &Path,
    owner_account_id: &str,
) -> Result<AutoIndex> {
    let inspection = {
        let parser = parser.clone();
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || parser.inspect(&path))
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de inspección: {error}")))??
    };

    let (source, entries) = if inspection.has_native_outline {
        ("outline", inspection.entries.clone())
    } else {
        let info = storage.pdf.get_info(book_id).await?;
        let scanned = info
            .as_ref()
            .map(|info| info.is_scanned)
            .unwrap_or(inspection.scanned);
        let pages_total = info
            .as_ref()
            .map(|info| info.page_count.max(0) as usize)
            .unwrap_or(inspection.page_count);

        if scanned {
            let storage = storage.clone();
            let book_id = book_id.to_string();
            let pages = tokio::task::spawn_blocking(move || {
                list_ocr_pages(&storage, &book_id, pages_total)
            })
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de lectura OCR: {error}")))??;
            ("ocr", build_from_ocr_pages(&pages))
        } else {
            let parser = parser.clone();
            let path = path.to_path_buf();
            let lines = tokio::task::spawn_blocking(move || parser.index_lines(&path, 800))
                .await
                .map_err(|error| KoraError::Internal(format!("tarea de índice: {error}")))??;
            (
                "heuristic",
                crate::reader::auto_index::build_from_lines(&lines),
            )
        }
    };

    storage
        .pdf
        .save_auto_index(book_id, source, &entries, owner_account_id, None)
        .await
}
