use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use crate::commands::library::book_summary_for;
use crate::commands::reader::{book_asset_path_for, ensure_pdf, owned_book, prepare_pdf};
use crate::error::{KoraError, Result};
use crate::features::SemanticService;
use crate::features::ocr::default_engine;
use crate::reader::Locator;
use crate::reader::PageTextLayer;
use crate::reader::convert::{
    self, ChapterMode, ChapterText, ConversionOutput, ConversionReport, PageRange,
    PdfConversionState, RelocationInput, RelocationOutput, RelocationStatus,
};
use crate::reader::ocr::{ocr_dir, read_ocr_page, run_ocr_job};
use crate::reader::pdf::{PdfInspection, PdfParser};
use crate::state::AppState;
use crate::storage::conversion::{self, AnchorUpdate, EpubReplacement, ProgressUpdate};
use crate::storage::{
    AssetKind, AssetRef, AutoIndexEntry, Book, BookSummary, NewAuditEntry, OcrStatus, ResourceType,
    Storage,
};
use crate::tejido::epub::build_epub;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfConversionPreview {
    pub book_id: String,
    pub title: String,
    pub author: Option<String>,
    pub language: Option<String>,
    pub pages: usize,
    pub scanned: bool,
    pub ocr_status: String,
    pub ocr_pages_done: usize,
    pub ocr_pages_total: usize,
    pub has_native_outline: bool,
    pub estimated_chapters: usize,
    /// `backend` usa PDFium/OCR, `frontend` extrae texto con pdf.js y `none`
    /// indica que este dispositivo no puede convertir el documento.
    pub extraction: String,
    pub sample: String,
    pub annotations: usize,
    pub extracts: usize,
    pub notes: usize,
    pub voice_notes: usize,
    pub reactions: usize,
    pub audiobooks: usize,
    pub can_convert: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PdfConversionOptions {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub chapter_mode: Option<String>,
    pub replace_original: bool,
}

impl Default for PdfConversionOptions {
    fn default() -> Self {
        Self {
            title: None,
            author: None,
            language: None,
            chapter_mode: None,
            replace_original: true,
        }
    }
}

struct ProvidedPages {
    pages: Vec<PageTextLayer>,
    outline: Vec<AutoIndexEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfConversionResult {
    /// Libro PDF del que partió la conversión. Puede ser distinto de
    /// `book.id` cuando se elige conservar el PDF y crear un EPUB nuevo.
    pub source_book_id: String,
    pub book: BookSummary,
    pub report: ConversionReport,
}

#[tauri::command]
pub async fn pdf_conversion_preview(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<PdfConversionPreview> {
    let book = owned_book(&state, &book_id).await?;
    validate_pdf_book(&book)?;

    let prepared = prepare_pdf(&state, None, &book).await?;
    let inspection = prepared.inspection;
    let pages = inspection.page_count;
    let ocr = prepared.ocr;
    let ocr_status = ocr
        .as_ref()
        .map(|state| ocr_status_name(state.status))
        .unwrap_or("pending")
        .to_string();
    let ocr_pages_done = ocr
        .as_ref()
        .map(|state| state.pages_done.max(0) as usize)
        .unwrap_or(0);
    let ocr_pages_total = ocr
        .as_ref()
        .map(|state| state.pages_total.max(0) as usize)
        .unwrap_or(pages);

    let auto_index = state.storage.pdf.get_auto_index(&book.id).await?;
    let estimated_chapters = if inspection.entries.len() >= 2 {
        inspection.entries.len()
    } else if let Some(index) = auto_index.as_ref() {
        index.entries.len().max(1)
    } else {
        (pages / 20).max(1)
    };

    let asset_path = book_asset_path_for(&state.storage, &book)?;
    let sample = preview_sample(&state.storage, &book.id, &asset_path, inspection.scanned).await?;
    let annotations = state
        .storage
        .annotations
        .list_by_book(&book.id)
        .await?
        .len();
    let extracts = state.storage.extracts.list_by_book(&book.id).await?.len();
    let notes = state.storage.notes.list_by_book(&book.id).await?.len();
    let voice_notes = state
        .storage
        .voice_notes
        .list_by_book(&book.id)
        .await?
        .len();
    let reactions = state.storage.reactions.list_by_book(&book.id).await?.len();
    let audiobooks = state.storage.audiobooks.list_by_book(&book.id).await?.len();

    let pdfium_ready = crate::reader::pdf::bind_pdfium().is_ok();
    let ocr_ready = default_engine().available();
    let ocr_text_ready = ocr
        .as_ref()
        .is_some_and(|state| state.status == OcrStatus::Done);
    let ocr_failed = ocr
        .as_ref()
        .is_some_and(|state| state.status == OcrStatus::Failed);
    let extraction = if pdfium_ready || ocr_text_ready {
        "backend"
    } else if !inspection.scanned {
        "frontend"
    } else {
        "none"
    };

    let mut warnings = Vec::new();
    if book.read_only || book.received_share_id.is_some() {
        warnings.push(
            "Los libros compartidos de solo lectura no se pueden convertir en este dispositivo."
                .into(),
        );
    }
    if inspection.scanned {
        if ocr_text_ready {
            warnings.push("PDF escaneado: se reutilizará el texto ya reconocido por OCR.".into());
        } else if pdfium_ready && ocr_ready {
            warnings.push(
                "PDF escaneado: se ejecutará OCR automáticamente antes de generar el EPUB.".into(),
            );
        } else if !ocr_ready {
            warnings.push(
                "PDF escaneado sin OCR disponible; no se puede convertir en este dispositivo."
                    .into(),
            );
        } else {
            warnings.push(
                "PDF escaneado sin PDFium para rasterizar páginas; no se puede convertir en este dispositivo."
                    .into(),
            );
        }
    }
    if ocr_failed && !ocr_text_ready {
        warnings.push("El OCR falló anteriormente; se reintentará al convertir.".into());
    }
    if pages == 0 {
        warnings.push("El PDF no declara páginas utilizables.".into());
    } else {
        warnings.push(
            "La conversión conserva el texto; las imágenes, tablas y fórmulas no se incluyen."
                .into(),
        );
    }
    if extraction == "frontend" && !inspection.scanned {
        warnings.push(
            "El texto se extraerá con el visor PDF; el orden de lectura puede variar en páginas con columnas o tablas."
                .into(),
        );
    }

    let title = if book.title.trim().eq_ignore_ascii_case("sin título") {
        inspection
            .title
            .clone()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| book.title.clone())
    } else {
        book.title.clone()
    };

    Ok(PdfConversionPreview {
        book_id: book.id.clone(),
        title,
        author: book.author.clone().or(inspection.author.clone()),
        language: book.language.clone().or_else(|| Some("es".into())),
        pages,
        scanned: inspection.scanned,
        ocr_status,
        ocr_pages_done,
        ocr_pages_total,
        has_native_outline: inspection.has_native_outline,
        estimated_chapters,
        extraction: extraction.to_string(),
        sample,
        annotations,
        extracts,
        notes,
        voice_notes,
        reactions,
        audiobooks,
        can_convert: !book.read_only
            && book.received_share_id.is_none()
            && pages > 0
            && extraction != "none",
        warnings,
    })
}

#[tauri::command]
pub async fn convert_pdf_to_epub(
    state: State<'_, AppState>,
    app: AppHandle,
    book_id: String,
    options: PdfConversionOptions,
    pages: Option<Vec<PageTextLayer>>,
    outline: Option<Vec<AutoIndexEntry>>,
) -> Result<PdfConversionState> {
    let book = owned_book(&state, &book_id).await?;
    validate_pdf_book(&book)?;

    let provided = match pages {
        Some(pages) => Some(validate_provided_pages(
            &book.id,
            pages,
            outline.unwrap_or_default(),
        )?),
        None => None,
    };

    let mut jobs = state.pdf_conversions.lock().await;
    if jobs.values().any(|existing| !existing.done) {
        return Err(KoraError::Conflict(
            "ya hay una conversión en curso; espera a que termine antes de iniciar otra".into(),
        ));
    }
    let status = PdfConversionState::new(book_id.clone());
    jobs.insert(book_id.clone(), status.clone());
    drop(jobs);

    let storage = state.storage.clone();
    let layout = storage.layout().clone();
    let ocr_jobs = state.ocr_jobs.clone();
    let conversions = state.pdf_conversions.clone();
    let semantic = state.semantic.clone();
    let owner = state.identity.account.id.clone();
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        run_conversion(
            app_handle,
            storage,
            layout.root.join("tmp"),
            ocr_jobs,
            conversions,
            semantic,
            owner,
            book,
            options,
            provided,
        )
        .await;
    });

    Ok(status)
}

fn validate_provided_pages(
    book_id: &str,
    mut pages: Vec<PageTextLayer>,
    outline: Vec<AutoIndexEntry>,
) -> Result<ProvidedPages> {
    if pages.is_empty() {
        return Err(KoraError::InvalidInput(
            "el visor no envió páginas para convertir".into(),
        ));
    }
    if pages.len() > 5_000 {
        return Err(KoraError::Unsupported(
            "el PDF tiene demasiadas páginas para convertirlo en una sola operación".into(),
        ));
    }

    let total_words: usize = pages.iter().map(|page| page.words.len()).sum();
    if total_words > 1_000_000 {
        return Err(KoraError::Unsupported(
            "el texto extraído es demasiado grande para convertirlo en una sola operación".into(),
        ));
    }

    for (index, page) in pages.iter_mut().enumerate() {
        page.book_id = book_id.to_string();
        page.page = index;
        page.source = if page.source == "ocr" {
            "ocr".to_string()
        } else {
            "native".to_string()
        };
        if page.width <= 0.0 || !page.width.is_finite() {
            page.width = 595.0;
        }
        if page.height <= 0.0 || !page.height.is_finite() {
            page.height = 842.0;
        }
        page.words.retain(|word| !word.text.trim().is_empty());
        for word in &mut page.words {
            if !word.x.is_finite() {
                word.x = 0.0;
            }
            if !word.y.is_finite() {
                word.y = 0.0;
            }
            if word.width <= 0.0 || !word.width.is_finite() {
                word.width = 1.0;
            }
            if word.height <= 0.0 || !word.height.is_finite() {
                word.height = 1.0;
            }
            if word.font_size <= 0.0 || !word.font_size.is_finite() {
                word.font_size = word.height.max(1.0);
            }
        }
    }

    let outline = outline
        .into_iter()
        .filter(|entry| entry.page < pages.len() && !entry.title.trim().is_empty())
        .take(1_000)
        .collect();

    Ok(ProvidedPages { pages, outline })
}

#[tauri::command]
pub async fn pdf_conversion_status(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<Option<PdfConversionState>> {
    owned_book(&state, &book_id).await?;
    Ok(state.pdf_conversions.lock().await.get(&book_id).cloned())
}

fn validate_pdf_book(book: &Book) -> Result<()> {
    ensure_pdf(book)?;
    if book.read_only || book.received_share_id.is_some() {
        return Err(KoraError::Unsupported(
            "los libros compartidos de solo lectura no se pueden convertir".into(),
        ));
    }
    Ok(())
}

fn ocr_status_name(status: OcrStatus) -> &'static str {
    match status {
        OcrStatus::Pending => "pending",
        OcrStatus::Running => "running",
        OcrStatus::Done => "done",
        OcrStatus::Failed => "failed",
        OcrStatus::Skipped => "skipped",
    }
}

async fn preview_sample(
    storage: &Storage,
    book_id: &str,
    path: &Path,
    scanned: bool,
) -> Result<String> {
    let storage = storage.clone();
    let book_id = book_id.to_string();
    let path = path.to_path_buf();

    tokio::task::spawn_blocking(move || -> Result<String> {
        let mut parts = Vec::new();
        if scanned {
            for page in 0..2 {
                if let Ok(Some(layer)) = read_ocr_page(&storage, &book_id, page)
                    && !layer.words.is_empty()
                {
                    parts.push(layer.text());
                }
            }
        }

        if parts.is_empty() {
            let parser = PdfParser::new();
            if let Ok(layers) = parser.page_texts(&path, &[0, 1]) {
                for (_, layer) in layers {
                    if !layer.words.is_empty() {
                        parts.push(layer.text());
                    }
                }
            }
        }

        let text = parts.join("\n\n");
        let total = text.chars().count();
        let mut preview: String = text.chars().take(900).collect();
        if total > 900 {
            preview.push('…');
        }
        Ok(preview)
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de vista previa: {error}")))?
}

#[allow(clippy::too_many_arguments)]
async fn run_conversion(
    app: AppHandle,
    storage: Storage,
    tmp_root: PathBuf,
    ocr_jobs: Arc<Mutex<HashSet<String>>>,
    conversions: Arc<Mutex<HashMap<String, PdfConversionState>>>,
    semantic: Arc<SemanticService>,
    owner: String,
    book: Book,
    options: PdfConversionOptions,
    provided: Option<ProvidedPages>,
) {
    let workspace = tmp_root.join(format!("pdf-convert-{}", uuid::Uuid::new_v4()));
    if let Err(error) = std::fs::create_dir_all(&workspace) {
        fail_conversion(&app, &conversions, &book.id, KoraError::Io(error)).await;
        return;
    }

    let result = run_conversion_inner(
        &app,
        &storage,
        &ocr_jobs,
        &conversions,
        &semantic,
        &owner,
        &book,
        &options,
        provided,
        &workspace,
    )
    .await;

    let _ = std::fs::remove_dir_all(&workspace);

    match result {
        Ok(result) => finish_conversion(&app, &conversions, &book.id, &result).await,
        Err(error) => fail_conversion(&app, &conversions, &book.id, error).await,
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_conversion_inner(
    app: &AppHandle,
    storage: &Storage,
    ocr_jobs: &Arc<Mutex<HashSet<String>>>,
    conversions: &Arc<Mutex<HashMap<String, PdfConversionState>>>,
    semantic: &Arc<SemanticService>,
    owner: &str,
    book: &Book,
    options: &PdfConversionOptions,
    provided: Option<ProvidedPages>,
    workspace: &Path,
) -> Result<PdfConversionResult> {
    emit_status(
        app,
        conversions,
        &book.id,
        "preparing",
        2.0,
        "Preparando la conversión…",
        0,
        0,
    )
    .await;

    let asset_path = book_asset_path_for(storage, book)?;
    let (pages, pages_total, pdf_author, outline) = if let Some(provided) = provided {
        let pages_total = provided.pages.len();
        emit_status(
            app,
            conversions,
            &book.id,
            "extracting",
            40.0,
            "Usando el texto extraído por el visor…",
            pages_total,
            pages_total,
        )
        .await;
        let outline = if provided.outline.is_empty() {
            stored_outline(storage, book).await?
        } else {
            provided.outline
        };
        (provided.pages, pages_total, None, outline)
    } else {
        let inspection_path = asset_path.clone();
        let inspection =
            tokio::task::spawn_blocking(move || PdfParser::new().inspect(&inspection_path))
                .await
                .map_err(|error| KoraError::Internal(format!("tarea de inspección: {error}")))??;
        let pages_total = inspection.page_count;

        emit_status(
            app,
            conversions,
            &book.id,
            "preparing",
            5.0,
            format!("{pages_total} páginas detectadas."),
            0,
            pages_total,
        )
        .await;

        ensure_ocr(
            app,
            storage,
            ocr_jobs,
            conversions,
            book,
            &asset_path,
            &inspection,
        )
        .await?;

        emit_status(
            app,
            conversions,
            &book.id,
            "extracting",
            45.0,
            "Extrayendo texto por página…",
            0,
            pages_total,
        )
        .await;

        let pages = extract_pages(storage, book, &asset_path, &inspection).await?;
        let outline = conversion_outline(storage, book, &inspection).await?;
        (pages, pages_total, inspection.author.clone(), outline)
    };

    let title = options
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&book.title)
        .chars()
        .take(200)
        .collect::<String>();
    let author = options
        .author
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| book.author.clone())
        .or(pdf_author);
    let language = options
        .language
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| book.language.clone())
        .unwrap_or_else(|| "es".to_string());
    let language = if language.len() <= 16
        && language
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        language
    } else {
        "es".to_string()
    };
    let mode = ChapterMode::parse(options.chapter_mode.as_deref());

    emit_status(
        app,
        conversions,
        &book.id,
        "chapters",
        58.0,
        "Reconstruyendo párrafos y capítulos…",
        pages_total,
        pages_total,
    )
    .await;

    let language_for_layout = language.clone();
    let conversion = tokio::task::spawn_blocking(move || {
        convert::convert_pages(&pages, &outline, mode, &language_for_layout)
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de conversión: {error}")))??;

    ensure_text_quality(&conversion)?;

    emit_status(
        app,
        conversions,
        &book.id,
        "building",
        70.0,
        "Generando el EPUB…",
        pages_total,
        pages_total,
    )
    .await;

    let cover_path = prepare_cover(
        storage,
        book,
        &asset_path,
        workspace,
        &title,
        author.as_deref(),
    )
    .await?;
    let epub_path = workspace.join("converted.epub");
    let chapters = conversion.chapters.clone();
    let title_for_build = title.clone();
    let author_for_build = author.clone().unwrap_or_else(|| "Kora".to_string());
    let language_for_build = language.clone();
    let cover_for_build = cover_path.clone();
    let epub_for_build = epub_path.clone();
    tokio::task::spawn_blocking(move || {
        build_epub(
            &epub_for_build,
            &title_for_build,
            &author_for_build,
            &language_for_build,
            &chapters,
            cover_for_build.as_deref(),
        )
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de EPUB: {error}")))?
    .map_err(|error| KoraError::Internal(format!("no se pudo generar el EPUB: {error:#}")))?;

    emit_status(
        app,
        conversions,
        &book.id,
        "verifying",
        82.0,
        "Verificando el EPUB generado…",
        pages_total,
        pages_total,
    )
    .await;

    let chapter_texts = verify_epub(&epub_path, &conversion)?;
    let relocation = if options.replace_original {
        relocate_book_data(storage, &book.id, &chapter_texts, &conversion.page_ranges).await?
    } else {
        RelocationPlan::default()
    };

    let current = storage
        .books
        .get(&book.id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("libro {}", book.id)))?;
    if current.deleted_at.is_some() {
        return Err(KoraError::Conflict(
            "el PDF se movió a la papelera durante la conversión".into(),
        ));
    }

    emit_status(
        app,
        conversions,
        &book.id,
        "migrating",
        90.0,
        "Migrando progreso y anotaciones…",
        pages_total,
        pages_total,
    )
    .await;

    let epub_size = std::fs::metadata(&epub_path).map_err(KoraError::Io)?.len() as i64;
    let cover_asset = match cover_path.as_deref() {
        Some(path) => {
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("png");
            Some(storage.assets.put_file(AssetKind::Cover, path, extension)?)
        }
        None => None,
    };
    let (cover_hash, cover_ext) = if let Some(asset) = cover_asset.as_ref() {
        (Some(asset.hash.clone()), Some(asset.extension.clone()))
    } else {
        (book.cover_hash.clone(), book.cover_ext.clone())
    };

    let original_pdf_asset = AssetRef {
        kind: AssetKind::Book,
        hash: book.content_hash.clone(),
        extension: book.content_ext.clone(),
    };
    let old_cover = book.cover_hash.clone().zip(book.cover_ext.clone());
    let slug = slug(&title);
    let original_name = format!("{slug}.epub");
    let description = book
        .description
        .clone()
        .or_else(|| Some("Convertido desde PDF.".to_string()));

    let mut shares_revoked = false;
    let final_book = if options.replace_original {
        let asset = storage
            .assets
            .put_file(AssetKind::Book, &epub_path, "epub")?;
        let old_progress = storage.progress.get(&book.id).await?;
        let progress = old_progress.as_ref().map(|progress| {
            let sections = conversion.chapters.len().max(1);
            let section = ((progress.percent.clamp(0.0, 1.0) * sections as f64).floor() as usize)
                .min(sections - 1);
            let locator = serde_json::to_string(&Locator { section, offset: 0 })
                .unwrap_or_else(|_| "{\"section\":0,\"offset\":0}".to_string());
            ProgressUpdate {
                locator,
                percent: progress.percent,
            }
        });

        conversion::replace_book_with_epub(
            storage,
            &EpubReplacement {
                book_id: &book.id,
                title: &title,
                author: author.as_deref(),
                language: Some(language.as_str()),
                description: description.as_deref(),
                content_hash: &asset.hash,
                content_ext: "epub",
                original_name: &original_name,
                size_bytes: epub_size,
                cover_hash: cover_hash.as_deref(),
                cover_ext: cover_ext.as_deref(),
                progress,
                annotations: relocation.annotations,
                extracts: relocation.extracts,
                notes: relocation.notes,
                voice_notes: relocation.voice_notes,
                reactions: relocation.reactions,
            },
        )
        .await?;

        let final_book = storage
            .books
            .get(&book.id)
            .await?
            .ok_or_else(|| KoraError::NotFound(format!("libro {}", book.id)))?;

        cleanup_replaced_assets(
            storage,
            &book.id,
            &original_pdf_asset,
            old_cover,
            cover_hash.clone().zip(cover_ext.clone()),
        )
        .await;

        // El contenido del recurso cambió de PDF a EPUB. Un enlace de
        // compartición antiguo serviría un archivo distinto al que se compartió.
        let shares = storage
            .shares
            .list_by_resource(ResourceType::Book, &book.id)
            .await
            .unwrap_or_default();
        if shares.iter().any(|share| share.revoked_at.is_none())
            && storage
                .shares
                .revoke_by_resource(ResourceType::Book, &book.id)
                .await
                .is_ok()
        {
            shares_revoked = true;
        }

        final_book
    } else {
        let asset = storage
            .assets
            .put_file(AssetKind::GeneratedBook, &epub_path, "epub")?;
        let created = conversion::create_converted_book(
            storage,
            &title,
            author.as_deref(),
            Some(language.as_str()),
            description.as_deref(),
            &asset.hash,
            "epub",
            &original_name,
            epub_size,
            cover_hash.as_deref(),
            cover_ext.as_deref(),
            owner,
        )
        .await?;

        for category in storage.book_categories.list_by_book(&book.id).await? {
            let _ = storage
                .book_categories
                .assign(&created.id, &category.id, owner, None)
                .await;
        }
        if storage.books.is_favorite(&book.id).await? {
            let _ = storage.books.set_favorite(owner, &created.id, true).await;
        }
        created
    };

    let mut report = ConversionReport {
        book_id: final_book.id.clone(),
        chapters: conversion.chapters.len(),
        chars: conversion.chars,
        pages: pages_total,
        ocr_used: conversion.ocr_pages > 0,
        anchored: if options.replace_original {
            relocation.anchored
        } else {
            0
        },
        approximate: if options.replace_original {
            relocation.approximate
        } else {
            0
        },
        unmapped: if options.replace_original {
            relocation.unmapped
        } else {
            0
        },
        warnings: conversion.warnings.clone(),
        replaced: options.replace_original,
    };
    if !options.replace_original {
        report
            .warnings
            .push("El PDF original se conserva porque elegiste crear un EPUB nuevo.".into());
    }
    if shares_revoked {
        report.warnings.push(
            "Se revocaron los enlaces de compartición del PDF porque su contenido cambió.".into(),
        );
    }

    let metadata = serde_json::json!({
        "title": final_book.title.clone(),
        "pages": pages_total,
        "chapters": report.chapters,
        "ocrUsed": report.ocr_used,
        "anchored": report.anchored,
        "approximate": report.approximate,
        "replaced": report.replaced,
    });
    if let Err(error) = storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(owner.to_string()),
            action: "book.convert_epub".to_string(),
            resource_type: Some(ResourceType::Book),
            resource_id: Some(final_book.id.clone()),
            metadata: metadata.to_string(),
        })
        .await
    {
        tracing::warn!(%error, book = %final_book.id, "no se pudo registrar la conversión");
    }

    if semantic.provider().ready() {
        let semantic = semantic.clone();
        let app = app.clone();
        let indexed_book = final_book.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = semantic.index_book(Some(&app), &indexed_book).await {
                tracing::warn!(book = %indexed_book.id, %error, "no se pudo reindexar el EPUB convertido");
            }
        });
    }

    emit_status(
        app,
        conversions,
        &book.id,
        "cleanup",
        97.0,
        "Limpiando datos temporales…",
        pages_total,
        pages_total,
    )
    .await;

    let summary = book_summary_for(storage, &final_book).await?;
    Ok(PdfConversionResult {
        source_book_id: book.id.clone(),
        book: summary,
        report,
    })
}

async fn ensure_ocr(
    app: &AppHandle,
    storage: &Storage,
    ocr_jobs: &Arc<Mutex<HashSet<String>>>,
    conversions: &Arc<Mutex<HashMap<String, PdfConversionState>>>,
    book: &Book,
    path: &Path,
    inspection: &PdfInspection,
) -> Result<()> {
    if !inspection.scanned {
        return Ok(());
    }

    let current = storage.pdf.get_ocr_state(&book.id).await?;
    if current
        .as_ref()
        .is_some_and(|state| matches!(state.status, OcrStatus::Done | OcrStatus::Skipped))
    {
        return Ok(());
    }
    if !default_engine().available() {
        return Err(KoraError::Unsupported(
            "el OCR no está disponible en esta plataforma".into(),
        ));
    }

    let running = ocr_jobs.lock().await.contains(&book.id);
    if !running {
        ocr_jobs.lock().await.insert(book.id.clone());
        let app = app.clone();
        let storage = storage.clone();
        let jobs = ocr_jobs.clone();
        let book_id = book.id.clone();
        let path = path.to_path_buf();
        let pages_total = inspection.page_count;
        let owner = book.owner_account_id.clone();
        tauri::async_runtime::spawn(async move {
            run_ocr_job(app, storage, jobs, book_id, path, pages_total, owner).await;
        });
    }

    let started = Instant::now();
    loop {
        let state = storage.pdf.get_ocr_state(&book.id).await?;
        let status = state
            .as_ref()
            .map(|state| state.status)
            .unwrap_or(OcrStatus::Pending);
        let done = state
            .as_ref()
            .map(|state| state.pages_done.max(0) as usize)
            .unwrap_or(0);
        let total = state
            .as_ref()
            .map(|state| state.pages_total.max(0) as usize)
            .unwrap_or(inspection.page_count)
            .max(1);
        let percent = 8.0 + (done.min(total) as f32 / total as f32) * 35.0;

        emit_status(
            app,
            conversions,
            &book.id,
            "ocr",
            percent,
            format!("Reconociendo texto… {done}/{total} páginas"),
            done,
            total,
        )
        .await;

        match status {
            OcrStatus::Done | OcrStatus::Skipped => return Ok(()),
            OcrStatus::Failed => {
                return Err(KoraError::Unsupported(
                    state
                        .and_then(|state| state.error)
                        .unwrap_or_else(|| "el OCR no pudo completarse".to_string()),
                ));
            }
            OcrStatus::Pending | OcrStatus::Running => {}
        }

        if started.elapsed() > Duration::from_secs(3 * 60 * 60) {
            return Err(KoraError::Internal(
                "el OCR tardó demasiado; puedes reintentarlo desde el lector".into(),
            ));
        }
        tokio::time::sleep(Duration::from_millis(700)).await;
    }
}

async fn extract_pages(
    storage: &Storage,
    book: &Book,
    path: &Path,
    inspection: &PdfInspection,
) -> Result<Vec<PageTextLayer>> {
    let total = inspection.page_count;
    let mut ocr_by_page: HashMap<usize, PageTextLayer> = HashMap::new();
    for page in 0..total {
        match read_ocr_page(storage, &book.id, page) {
            Ok(Some(mut layer)) if !layer.words.is_empty() => {
                layer.page = page;
                ocr_by_page.insert(page, layer);
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, book = %book.id, page, "capa OCR ilegible; se omite");
            }
        }
    }

    let pages: Vec<usize> = (0..total).collect();
    let path = path.to_path_buf();
    let native = tokio::task::spawn_blocking(move || PdfParser::new().page_texts(&path, &pages))
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de extracción: {error}")));

    let native_layers = match native {
        Ok(Ok(layers)) => layers,
        Ok(Err(error)) => {
            tracing::warn!(%error, book = %book.id, "no se pudo extraer texto nativo");
            Vec::new()
        }
        Err(error) => {
            tracing::warn!(%error, book = %book.id, "la tarea de texto nativo falló");
            Vec::new()
        }
    };
    let mut native_by_page: HashMap<usize, PageTextLayer> = native_layers.into_iter().collect();

    let mut result = Vec::with_capacity(total);
    for page in 0..total {
        let native_layer = native_by_page.remove(&page);
        let ocr_layer = ocr_by_page.remove(&page);

        let chosen = if inspection.scanned {
            ocr_layer
                .filter(|layer| !layer.words.is_empty())
                .or(native_layer)
        } else {
            native_layer
                .filter(|layer| !layer.words.is_empty())
                .or(ocr_layer)
        };

        result.push(chosen.unwrap_or_else(|| PageTextLayer {
            book_id: book.id.clone(),
            page,
            width: 595.0,
            height: 842.0,
            source: "empty".to_string(),
            words: Vec::new(),
        }));
    }

    Ok(result)
}

async fn stored_outline(storage: &Storage, book: &Book) -> Result<Vec<AutoIndexEntry>> {
    Ok(storage
        .pdf
        .get_auto_index(&book.id)
        .await?
        .map(|index| index.entries)
        .unwrap_or_default())
}

async fn conversion_outline(
    storage: &Storage,
    book: &Book,
    inspection: &PdfInspection,
) -> Result<Vec<AutoIndexEntry>> {
    if inspection.has_native_outline && inspection.entries.len() >= 2 {
        return Ok(inspection.entries.clone());
    }
    stored_outline(storage, book).await
}

async fn prepare_cover(
    storage: &Storage,
    book: &Book,
    pdf_path: &Path,
    workspace: &Path,
    title: &str,
    author: Option<&str>,
) -> Result<Option<PathBuf>> {
    if let (Some(hash), Some(extension)) = (&book.cover_hash, &book.cover_ext) {
        let reference = AssetRef {
            kind: AssetKind::Cover,
            hash: hash.clone(),
            extension: extension.clone(),
        };
        let normalized = extension.to_ascii_lowercase();
        if matches!(normalized.as_str(), "png" | "jpg" | "jpeg" | "webp")
            && let Ok(path) = storage.assets.path(&reference)
            && path.is_file()
        {
            let cover_path = workspace.join(format!("cover.{normalized}"));
            std::fs::copy(&path, &cover_path)?;
            return Ok(Some(cover_path));
        }
    }

    let cover_path = workspace.join("cover.png");
    let pdf_path = pdf_path.to_path_buf();
    let rendered =
        tokio::task::spawn_blocking(move || PdfParser::new().render_page(&pdf_path, 0, 150.0))
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de portada: {error}")));

    if let Ok(Ok(rendered)) = rendered {
        std::fs::write(&cover_path, rendered.png)?;
        return Ok(Some(cover_path));
    }

    if crate::tejido::cover::render_cover(title, author.unwrap_or("Kora"), &cover_path).is_ok() {
        return Ok(Some(cover_path));
    }

    Ok(None)
}

fn ensure_text_quality(conversion: &ConversionOutput) -> Result<()> {
    if conversion.chars < 200 {
        return Err(KoraError::Unsupported(
            "se extrajo muy poco texto para construir un EPUB legible".into(),
        ));
    }
    if conversion.empty_pages > 0 && conversion.native_pages + conversion.ocr_pages == 0 {
        return Err(KoraError::Unsupported(
            "el PDF no contiene texto utilizable; parece un documento de imágenes".into(),
        ));
    }
    Ok(())
}

fn verify_epub(path: &Path, conversion: &ConversionOutput) -> Result<Vec<ChapterText>> {
    let texts = convert::chapter_texts(path)?;
    if texts.is_empty() {
        return Err(KoraError::Internal(
            "el EPUB generado no contiene capítulos legibles".into(),
        ));
    }
    if texts.len() < conversion.chapters.len() {
        return Err(KoraError::Internal(
            "el EPUB generado perdió capítulos durante la verificación".into(),
        ));
    }
    let chars: usize = texts
        .iter()
        .map(|chapter| chapter.text.chars().count())
        .sum();
    if chars == 0 {
        return Err(KoraError::Internal(
            "el EPUB generado no contiene texto".into(),
        ));
    }
    if chars * 2 < conversion.chars {
        return Err(KoraError::Internal(
            "el texto del EPUB es demasiado corto respecto al PDF; se canceló la sustitución"
                .into(),
        ));
    }
    Ok(texts)
}

#[derive(Default)]
struct RelocationPlan {
    annotations: Vec<AnchorUpdate>,
    extracts: Vec<AnchorUpdate>,
    notes: Vec<AnchorUpdate>,
    voice_notes: Vec<AnchorUpdate>,
    reactions: Vec<AnchorUpdate>,
    anchored: usize,
    approximate: usize,
    unmapped: usize,
}

async fn relocate_book_data(
    storage: &Storage,
    book_id: &str,
    chapters: &[ChapterText],
    page_ranges: &[PageRange],
) -> Result<RelocationPlan> {
    let annotations = storage.annotations.list_by_book(book_id).await?;
    let annotation_inputs: Vec<RelocationInput> = annotations
        .iter()
        .map(|annotation| RelocationInput {
            id: annotation.id.clone(),
            page: annotation
                .chapter_index
                .and_then(|page| usize::try_from(page.max(0)).ok()),
            quote: if annotation.kind == crate::storage::AnnotationKind::Bookmark {
                None
            } else {
                annotation.text.clone()
            },
        })
        .collect();
    let annotation_outputs = convert::relocate_items(&annotation_inputs, chapters, page_ranges);

    let extracts = storage.extracts.list_by_book(book_id).await?;
    let extract_inputs: Vec<RelocationInput> = extracts
        .iter()
        .map(|extract| RelocationInput {
            id: extract.id.clone(),
            page: extract
                .chapter_index
                .and_then(|page| usize::try_from(page.max(0)).ok()),
            quote: Some(extract.text.clone()),
        })
        .collect();
    let extract_outputs = convert::relocate_items(&extract_inputs, chapters, page_ranges);

    let notes = storage.notes.list_by_book(book_id).await?;
    let note_inputs: Vec<RelocationInput> = notes
        .iter()
        .map(|note| RelocationInput {
            id: note.id.clone(),
            page: note
                .chapter_index
                .and_then(|page| usize::try_from(page.max(0)).ok()),
            quote: note.text.clone(),
        })
        .collect();
    let note_outputs = convert::relocate_items(&note_inputs, chapters, page_ranges);

    let voice_notes = storage.voice_notes.list_by_book(book_id).await?;
    let voice_inputs: Vec<RelocationInput> = voice_notes
        .iter()
        // Los marcadores de audiolibro usan el capítulo del M4B, no la página
        // del PDF; se dejan intactos para no romper su reproducción.
        .filter(|note| note.audiobook_id.is_none())
        .map(|note| RelocationInput {
            id: note.id.clone(),
            page: Some(note.chapter_index.max(0) as usize),
            quote: note
                .quote
                .clone()
                .filter(|quote| !quote.trim().is_empty())
                .or_else(|| Some(note.text.clone())),
        })
        .collect();
    let voice_outputs = convert::relocate_items(&voice_inputs, chapters, page_ranges);

    let reactions = storage.reactions.list_by_book(book_id).await?;
    let reaction_inputs: Vec<RelocationInput> = reactions
        .iter()
        .map(|reaction| RelocationInput {
            id: reaction.id.clone(),
            page: Some(reaction.chapter_index.max(0) as usize),
            quote: reaction.text.clone(),
        })
        .collect();
    let reaction_outputs = convert::relocate_items(&reaction_inputs, chapters, page_ranges);

    let all_inputs: Vec<&RelocationInput> = annotation_inputs
        .iter()
        .chain(extract_inputs.iter())
        .chain(note_inputs.iter())
        .chain(voice_inputs.iter())
        .chain(reaction_inputs.iter())
        .collect();
    let all_outputs: Vec<&RelocationOutput> = annotation_outputs
        .iter()
        .chain(extract_outputs.iter())
        .chain(note_outputs.iter())
        .chain(voice_outputs.iter())
        .chain(reaction_outputs.iter())
        .collect();

    let anchored = all_outputs
        .iter()
        .filter(|output| output.status == RelocationStatus::Anchored)
        .count();
    let approximate = all_outputs.len().saturating_sub(anchored);
    let unmapped = all_inputs
        .iter()
        .filter(|input| {
            input.page.is_none()
                && input
                    .quote
                    .as_deref()
                    .map(str::trim)
                    .is_none_or(|quote| quote.chars().count() < 6)
        })
        .count();

    Ok(RelocationPlan {
        annotations: anchor_updates(annotation_outputs),
        extracts: anchor_updates(extract_outputs),
        notes: anchor_updates(note_outputs),
        voice_notes: anchor_updates(voice_outputs),
        reactions: anchor_updates(reaction_outputs),
        anchored,
        approximate,
        unmapped,
    })
}

fn anchor_updates(outputs: Vec<RelocationOutput>) -> Vec<AnchorUpdate> {
    outputs
        .into_iter()
        .map(|output| AnchorUpdate {
            id: output.id,
            chapter_index: output.chapter_index,
            offset_start: output.offset_start,
            offset_end: output.offset_end,
        })
        .collect()
}

async fn cleanup_replaced_assets(
    storage: &Storage,
    book_id: &str,
    pdf_asset: &AssetRef,
    old_cover: Option<(String, String)>,
    new_cover: Option<(String, String)>,
) {
    if !storage
        .asset_is_referenced(AssetKind::Book, &pdf_asset.hash)
        .await
        .unwrap_or(true)
        && let Ok(path) = storage.assets.path(pdf_asset)
        && path.is_file()
        && let Err(error) = crate::storage::trash::move_to_trash(storage.layout(), &path)
    {
        tracing::warn!(%error, book = book_id, "no se pudo mover el PDF original a la papelera");
    }

    if let Ok(directory) = ocr_dir(storage, book_id)
        && directory.is_dir()
        && let Err(error) = crate::storage::trash::move_to_trash(storage.layout(), &directory)
    {
        tracing::warn!(%error, book = book_id, "no se pudo mover el OCR a la papelera");
    }

    let practice = storage
        .layout()
        .assets
        .join("tts")
        .join("practice")
        .join(book_id);
    if practice.is_dir()
        && let Err(error) = crate::storage::trash::move_to_trash(storage.layout(), &practice)
    {
        tracing::warn!(%error, book = book_id, "no se pudo mover la práctica a la papelera");
    }

    if let Some((hash, extension)) = old_cover {
        let same_cover = new_cover
            .as_ref()
            .is_some_and(|(new_hash, new_ext)| new_hash == &hash && new_ext == &extension);
        if !same_cover
            && !storage
                .asset_is_referenced(AssetKind::Cover, &hash)
                .await
                .unwrap_or(true)
        {
            let reference = AssetRef {
                kind: AssetKind::Cover,
                hash,
                extension,
            };
            if let Ok(path) = storage.assets.path(&reference)
                && path.is_file()
                && let Err(error) = crate::storage::trash::move_to_trash(storage.layout(), &path)
            {
                tracing::warn!(%error, book = book_id, "no se pudo mover la portada antigua a la papelera");
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn emit_status(
    app: &AppHandle,
    conversions: &Arc<Mutex<HashMap<String, PdfConversionState>>>,
    book_id: &str,
    phase: &str,
    percent: f32,
    message: impl Into<String>,
    pages_done: usize,
    pages_total: usize,
) {
    let state = {
        let mut jobs = conversions.lock().await;
        let state = jobs
            .entry(book_id.to_string())
            .or_insert_with(|| PdfConversionState::new(book_id));
        state.phase = phase.to_string();
        state.percent = percent.clamp(0.0, 100.0);
        state.message = message.into();
        state.pages_done = pages_done;
        state.pages_total = pages_total;
        state.clone()
    };
    let _ = app.emit("pdf-convert-progress", state);
}

async fn finish_conversion(
    app: &AppHandle,
    conversions: &Arc<Mutex<HashMap<String, PdfConversionState>>>,
    source_book_id: &str,
    result: &PdfConversionResult,
) {
    let state = {
        let mut jobs = conversions.lock().await;
        let state = jobs
            .entry(source_book_id.to_string())
            .or_insert_with(|| PdfConversionState::new(source_book_id));
        state.phase = "done".to_string();
        state.percent = 100.0;
        state.message = "Conversión completada.".to_string();
        state.done = true;
        state.finished_at = Some(chrono::Utc::now());
        state.report = Some(result.report.clone());
        state.clone()
    };
    let _ = app.emit("pdf-convert-progress", state);
    let _ = app.emit("pdf-convert-done", result);
}

async fn fail_conversion(
    app: &AppHandle,
    conversions: &Arc<Mutex<HashMap<String, PdfConversionState>>>,
    book_id: &str,
    error: KoraError,
) {
    let message = error.to_string();
    let state = {
        let mut jobs = conversions.lock().await;
        let state = jobs
            .entry(book_id.to_string())
            .or_insert_with(|| PdfConversionState::new(book_id));
        state.phase = "failed".to_string();
        state.done = true;
        state.error = Some(message.clone());
        state.message = message;
        state.finished_at = Some(chrono::Utc::now());
        state.clone()
    };
    let _ = app.emit("pdf-convert-failed", state);
}

fn slug(title: &str) -> String {
    let mut slug = String::new();
    for character in title.chars() {
        if character.is_alphanumeric() {
            for lower in character.to_lowercase() {
                slug.push(lower);
            }
        } else if (character.is_whitespace() || matches!(character, '-' | '_' | '.'))
            && !slug.ends_with('-')
            && !slug.is_empty()
        {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-').chars().take(80).collect::<String>();
    if slug.is_empty() {
        "libro".to_string()
    } else {
        slug
    }
}
