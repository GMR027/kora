use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter, State};

use crate::commands::reader::{apply_metadata, apply_pdf_metadata, owned_book};
use crate::error::{KoraError, Result};
use crate::html::clean_description;
use crate::reader::ReaderService;
use crate::state::AppState;
use crate::storage::{
    AssetKind, AssetRef, Book, BookFormat, BookOrigin, BookSummary, Category, NewAuditEntry,
    NewBook, NewCategory, ResourceType, Storage,
};

#[tauri::command]
pub async fn library_list(state: State<'_, AppState>) -> Result<Vec<BookSummary>> {
    let owner = state.identity.account.id.clone();
    let rows = state.storage.books.list_summaries(&owner).await?;
    let mut categories_by_book: HashMap<String, Vec<String>> = HashMap::new();
    for (book_id, category_id) in state
        .storage
        .book_categories
        .list_pairs_by_owner(&owner)
        .await?
    {
        categories_by_book
            .entry(book_id)
            .or_default()
            .push(category_id);
    }

    let mut books = Vec::with_capacity(rows.len());
    for row in rows {
        let cover_path = cover_path(&state, row.cover_hash.as_deref(), row.cover_ext.as_deref())?;
        let category_ids = categories_by_book.remove(&row.id).unwrap_or_default();

        books.push(BookSummary {
            id: row.id,
            title: row.title,
            author: row.author,
            format: row.format,
            cover_path,
            size_bytes: row.size_bytes.max(0) as u64,
            added_at: row.added_at,
            language: row.language,
            publisher: row.publisher,
            description: clean_description(row.description),
            last_read_at: row.last_read_at,
            progress: row.progress as f32,
            origin: row.origin,
            favorite: row.favorite,
            read_only: row.read_only,
            received_share_id: row.received_share_id,
            category_ids,
        });
    }

    Ok(books)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueBook {
    pub id: String,
    pub title: String,
    pub format: BookFormat,
    pub cover_path: Option<String>,
    pub progress: f32,
    pub added_at: chrono::DateTime<chrono::Utc>,
    pub last_read_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryOverview {
    pub current: Option<ContinueBook>,
    pub in_progress: u32,
}

/// Resumen mínimo para la barra superior: un solo libro y un contador, sin
/// serializar toda la biblioteca en cada navegación.
#[tauri::command]
pub async fn library_overview(state: State<'_, AppState>) -> Result<LibraryOverview> {
    let owner = state.identity.account.id.clone();
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            BookFormat,
            Option<String>,
            Option<String>,
            f64,
            chrono::DateTime<chrono::Utc>,
            Option<chrono::DateTime<chrono::Utc>>,
        ),
    >(
        "SELECT b.id, b.title, b.format, b.cover_hash, b.cover_ext,
                COALESCE(rp.percent, 0.0), b.added_at, rp.updated_at
         FROM books b
         LEFT JOIN reading_progress rp ON rp.book_id = b.id
         WHERE b.owner_account_id = ? AND b.audio_only = 0 AND b.deleted_at IS NULL
         ORDER BY
            CASE WHEN COALESCE(rp.percent, 0.0) > 0.001
                   AND COALESCE(rp.percent, 0.0) < 0.995 THEN 0 ELSE 1 END,
            COALESCE(rp.updated_at, b.added_at) DESC
         LIMIT 1",
    )
    .bind(&owner)
    .fetch_optional(state.storage.books.pool())
    .await?;

    let current = match row {
        Some((id, title, format, cover_hash, cover_ext, progress, added_at, last_read_at)) => {
            Some(ContinueBook {
                id,
                title,
                format,
                cover_path: cover_path(&state, cover_hash.as_deref(), cover_ext.as_deref())?,
                progress: progress as f32,
                added_at,
                last_read_at,
            })
        }
        None => None,
    };

    let in_progress: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM books b
         LEFT JOIN reading_progress rp ON rp.book_id = b.id
         WHERE b.owner_account_id = ? AND b.audio_only = 0 AND b.deleted_at IS NULL
           AND COALESCE(rp.percent, 0.0) > 0.001
           AND COALESCE(rp.percent, 0.0) < 0.995",
    )
    .bind(&owner)
    .fetch_one(state.storage.books.pool())
    .await?;

    Ok(LibraryOverview {
        current,
        in_progress: in_progress.0.max(0) as u32,
    })
}

#[tauri::command]
pub async fn library_audiobook_book_ids(state: State<'_, AppState>) -> Result<Vec<String>> {
    let owner = state.identity.account.id.clone();
    state
        .storage
        .audiobooks
        .list_book_ids_by_owner(&owner)
        .await
}

#[tauri::command]
pub async fn library_import(
    state: State<'_, AppState>,
    app: AppHandle,
    source: String,
) -> Result<BookSummary> {
    let source_path = PathBuf::from(&source);
    let format = detect_book_format(&source_path)?;

    let metadata = std::fs::metadata(&source_path)?;
    if !metadata.is_file() {
        return Err(KoraError::InvalidInput(format!(
            "la ruta no es un archivo: {source}"
        )));
    }
    if metadata.len() == 0 {
        return Err(KoraError::InvalidInput("el archivo está vacío".into()));
    }

    let owner = state.identity.account.id.clone();
    let asset = state
        .storage
        .assets
        .put_file(AssetKind::Book, &source_path, format.extension())?;

    let existing = match state
        .storage
        .books
        .find_by_hash(&owner, &asset.hash)
        .await?
    {
        Some(book) => Some(book),
        None => {
            // Reimportar un libro que está en la papelera lo restaura en lugar
            // de crear un duplicado (el hash de contenido es único por cuenta).
            match state
                .storage
                .books
                .find_trashed_by_hash(&owner, &asset.hash)
                .await?
            {
                Some(mut book) => {
                    state.storage.books.set_deleted(&book.id, false).await?;
                    book.deleted_at = None;
                    Some(book)
                }
                None => None,
            }
        }
    };

    if let Some(mut existing) = existing {
        if let Err(error) = attach_generated_cover(&state, &mut existing).await {
            tracing::warn!(%error, book = %existing.id, "no se pudo generar la portada existente");
        }
        if existing.format == BookFormat::Pdf
            && let Err(error) =
                crate::commands::reader::prepare_pdf(&state, Some(app.clone()), &existing).await
        {
            tracing::warn!(%error, book = %existing.id, "no se pudo preparar el PDF existente");
        }
        if state.semantic.provider().ready() {
            let semantic = state.semantic.clone();
            let handle = app.clone();
            let indexed_book = existing.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = semantic.index_book(Some(&handle), &indexed_book).await {
                    tracing::warn!(book = %indexed_book.id, %error, "no se pudo indexar el libro existente");
                }
            });
        }
        return book_summary(&state, &existing).await;
    }

    let title = source_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .unwrap_or("Sin título")
        .to_string();

    let original_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("libro")
        .to_string();

    let mut book = state
        .storage
        .books
        .create(NewBook {
            title,
            author: None,
            format,
            content_hash: asset.hash.clone(),
            content_ext: format.extension().to_string(),
            original_name,
            cover_hash: None,
            cover_ext: None,
            size_bytes: metadata.len() as i64,
            language: None,
            publisher: None,
            description: None,
            origin: BookOrigin::Imported,
            owner_account_id: owner.clone(),
            origin_share_id: None,
            audio_only: false,
        })
        .await?;

    // Los metadatos del EPUB se leen al importar para que la biblioteca no
    // dependa de abrir el libro una vez. En PDF se reutiliza la inspección de
    // `prepare_pdf` más abajo.
    if book.format == BookFormat::Epub {
        let asset_path = state.storage.assets.path(&AssetRef {
            kind: AssetKind::Book,
            hash: book.content_hash.clone(),
            extension: book.content_ext.clone(),
        })?;
        let inspection =
            tokio::task::spawn_blocking(move || ReaderService::new().inspect_epub(&asset_path))
                .await;
        if let Ok(Ok(inspection)) = inspection
            && apply_metadata(&mut book, &inspection)
        {
            state.storage.books.update(&book).await?;
        }
    }

    if let Err(error) = attach_generated_cover(&state, &mut book).await {
        tracing::warn!(%error, book = %book.id, "no se pudo generar la portada");
    }

    if book.format == BookFormat::Pdf {
        match crate::commands::reader::prepare_pdf(&state, Some(app.clone()), &book).await {
            Ok(prepared) => {
                if apply_pdf_metadata(&mut book, &prepared.inspection) {
                    state.storage.books.update(&book).await?;
                }
            }
            Err(error) => {
                tracing::warn!(%error, book = %book.id, "no se pudo preparar el PDF importado");
            }
        }
    }

    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(owner),
            action: "book.import".to_string(),
            resource_type: Some(ResourceType::Book),
            resource_id: Some(book.id.clone()),
            metadata: serde_json::json!({
                "contentHash": book.content_hash,
                "source": source,
            })
            .to_string(),
        })
        .await?;

    for achievement in state
        .achievements
        .evaluate(&state.identity.account.id)
        .await?
    {
        let _ = app.emit("achievement::unlocked", achievement);
    }

    if state.semantic.provider().ready() {
        let semantic = state.semantic.clone();
        let handle = app.clone();
        let indexed_book = book.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = semantic.index_book(Some(&handle), &indexed_book).await {
                tracing::warn!(book = %indexed_book.id, %error, "no se pudo indexar el libro importado");
            }
        });
    }

    let classification = state.classification.clone();
    let classification_config = classification.config().await.unwrap_or_default();
    if classification_config.enabled && classification_config.classify_on_import {
        let handle = app.clone();
        let target = book.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = classification
                .classify_book(Some(&handle), &target, false, false)
                .await
            {
                tracing::warn!(book = %target.id, %error, "no se pudo clasificar el libro importado");
            }
        });
    }

    book_summary(&state, &book).await
}

/// Mueve un libro a la papelera. El registro y sus anotaciones se conservan,
/// junto con los assets, para que la restauración sea completa.
#[tauri::command]
pub async fn library_delete(state: State<'_, AppState>, book_id: String) -> Result<()> {
    let book = crate::commands::reader::owned_book(&state, &book_id).await?;
    state.storage.books.set_deleted(&book.id, true).await?;
    // Un libro en la papelera no debe seguir compartido con otros equipos.
    state
        .storage
        .shares
        .revoke_by_resource(ResourceType::Book, &book.id)
        .await?;
    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(state.identity.account.id.clone()),
            action: "book.trash".to_string(),
            resource_type: Some(ResourceType::Book),
            resource_id: Some(book.id.clone()),
            metadata: serde_json::json!({ "title": book.title }).to_string(),
        })
        .await?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashedBookSummary {
    #[serde(flatten)]
    pub book: BookSummary,
    pub deleted_at: chrono::DateTime<chrono::Utc>,
}

#[tauri::command]
pub async fn library_trash(state: State<'_, AppState>) -> Result<Vec<TrashedBookSummary>> {
    let owner = state.identity.account.id.clone();
    let mut trash = Vec::new();
    for book in state.storage.books.list_trash(&owner).await? {
        let deleted_at = book.deleted_at.unwrap_or_else(chrono::Utc::now);
        trash.push(TrashedBookSummary {
            book: book_summary(&state, &book).await?,
            deleted_at,
        });
    }
    Ok(trash)
}

#[tauri::command]
pub async fn library_restore(state: State<'_, AppState>, book_id: String) -> Result<BookSummary> {
    let mut book = state
        .storage
        .books
        .get(&book_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("libro {book_id}")))?;
    if book.owner_account_id != state.identity.account.id || book.deleted_at.is_none() {
        return Err(KoraError::NotFound(format!("libro {book_id}")));
    }

    state.storage.books.set_deleted(&book.id, false).await?;
    book.deleted_at = None;
    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(state.identity.account.id.clone()),
            action: "book.restore".to_string(),
            resource_type: Some(ResourceType::Book),
            resource_id: Some(book.id.clone()),
            metadata: serde_json::json!({ "title": book.title }).to_string(),
        })
        .await?;
    book_summary(&state, &book).await
}

#[tauri::command]
pub async fn library_purge_book(state: State<'_, AppState>, book_id: String) -> Result<()> {
    let book = state
        .storage
        .books
        .get(&book_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("libro {book_id}")))?;
    if book.owner_account_id != state.identity.account.id || book.deleted_at.is_none() {
        return Err(KoraError::NotFound(format!("libro {book_id}")));
    }

    purge_book(&state, book).await
}

#[tauri::command]
pub async fn library_empty_trash(state: State<'_, AppState>) -> Result<u64> {
    let owner = state.identity.account.id.clone();
    let books = state.storage.books.list_trash(&owner).await?;
    let mut purged = 0u64;
    for book in books {
        purge_book(&state, book).await?;
        purged += 1;
    }
    Ok(purged)
}

/// Borra definitivamente un libro y mueve sus assets huérfanos a `trash/`.
async fn purge_book(state: &AppState, book: Book) -> Result<()> {
    let book_id = book.id.clone();
    let audiobook_assets = state.storage.audiobooks.list_by_book(&book_id).await?;
    let voice_notes = state
        .storage
        .voice_notes
        .list_by_book_including_deleted(&book_id)
        .await?;

    state.storage.books.delete(&book_id).await?;

    // Las notas de voz no usan assets con hash: su archivo se deriva del id.
    for note in &voice_notes {
        let path = crate::annotations::note_audio_path(state.storage.layout(), &note.id);
        if path.is_file() {
            crate::storage::trash::move_to_trash(state.storage.layout(), &path)?;
        }
    }

    for audiobook in &audiobook_assets {
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
    }

    // La capa OCR y las grabaciones de ensayo son específicas del libro y no
    // son assets con hash.
    let ocr_dir = state.storage.layout().assets.join("ocr").join(&book_id);
    if ocr_dir.is_dir() {
        crate::storage::trash::move_to_trash(state.storage.layout(), &ocr_dir)?;
    }
    let practice_dir = state
        .storage
        .layout()
        .assets
        .join("tts")
        .join("practice")
        .join(&book_id);
    if practice_dir.is_dir() {
        crate::storage::trash::move_to_trash(state.storage.layout(), &practice_dir)?;
    }

    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(state.identity.account.id.clone()),
            action: "book.delete".to_string(),
            resource_type: Some(ResourceType::Book),
            resource_id: Some(book_id.clone()),
            metadata: serde_json::json!({ "title": book.title }).to_string(),
        })
        .await?;

    let book_asset = AssetRef {
        kind: AssetKind::Book,
        hash: book.content_hash.clone(),
        extension: book.content_ext.clone(),
    };
    if !state
        .storage
        .asset_is_referenced(AssetKind::Book, &book.content_hash)
        .await?
        && state.storage.assets.exists(&book_asset)?
    {
        let path = state.storage.assets.path(&book_asset)?;
        crate::storage::trash::move_to_trash(state.storage.layout(), &path)?;
    }

    if book.origin != BookOrigin::Imported {
        let generated = AssetRef {
            kind: AssetKind::GeneratedBook,
            hash: book.content_hash.clone(),
            extension: book.content_ext.clone(),
        };
        if state.storage.assets.exists(&generated)? {
            let path = state.storage.assets.path(&generated)?;
            crate::storage::trash::move_to_trash(state.storage.layout(), &path)?;
        }
    }

    if let (Some(hash), Some(extension)) = (&book.cover_hash, &book.cover_ext) {
        let cover = AssetRef {
            kind: AssetKind::Cover,
            hash: hash.clone(),
            extension: extension.clone(),
        };
        if !state
            .storage
            .asset_is_referenced(AssetKind::Cover, hash)
            .await?
            && state.storage.assets.exists(&cover)?
        {
            let path = state.storage.assets.path(&cover)?;
            crate::storage::trash::move_to_trash(state.storage.layout(), &path)?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn library_ensure_cover(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<BookSummary> {
    let mut book = state
        .storage
        .books
        .get(&book_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("libro {book_id}")))?;

    attach_generated_cover(&state, &mut book).await?;
    book_summary(&state, &book).await
}

async fn attach_generated_cover(state: &AppState, book: &mut Book) -> Result<()> {
    if book.cover_hash.is_some() {
        return Ok(());
    }

    let reference = AssetRef {
        kind: AssetKind::Book,
        hash: book.content_hash.clone(),
        extension: book.content_ext.clone(),
    };
    let path = state.storage.assets.path(&reference)?;
    let format = book.format;
    let title = book.title.clone();
    let author = book.author.clone();
    let task_path = path.clone();

    let generated = tokio::task::spawn_blocking(move || -> Option<(Vec<u8>, String)> {
        match format {
            BookFormat::Pdf => {
                if let Some((bytes, extension)) = crate::commands::reader::render_cover(&task_path)
                {
                    return Some((bytes, extension.to_string()));
                }
                fallback_cover(&title, author.as_deref())
            }
            BookFormat::Epub => {
                if let Ok(inspection) = ReaderService::new().inspect_epub(&task_path)
                    && let Some(cover) = inspection.cover
                {
                    return Some((cover.bytes, cover.extension.to_string()));
                }
                fallback_cover(&title, author.as_deref())
            }
        }
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de portada: {error}")))?;

    let Some((bytes, extension)) = generated else {
        return Ok(());
    };

    let asset = state
        .storage
        .assets
        .put_bytes(AssetKind::Cover, &bytes, &extension)?;
    book.cover_hash = Some(asset.hash);
    book.cover_ext = Some(asset.extension);
    state.storage.books.update(book).await?;
    Ok(())
}

fn fallback_cover(title: &str, author: Option<&str>) -> Option<(Vec<u8>, String)> {
    crate::tejido::cover::render_cover_bytes(title, author.unwrap_or("Kora"))
        .ok()
        .map(|bytes| (bytes, "png".to_string()))
}

#[tauri::command]
pub async fn library_replace_cover(
    state: State<'_, AppState>,
    book_id: String,
    source: String,
) -> Result<BookSummary> {
    let mut book = state
        .storage
        .books
        .get(&book_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("libro {book_id}")))?;

    let source_path = PathBuf::from(&source);
    let extension = detect_image_extension(&source_path)?;
    let asset = state
        .storage
        .assets
        .put_file(AssetKind::Cover, &source_path, extension)?;

    let previous = book.cover_hash.clone().zip(book.cover_ext.clone());

    state
        .storage
        .books
        .replace_cover(&book_id, &asset.hash, &asset.extension)
        .await?;

    book.cover_hash = Some(asset.hash.clone());
    book.cover_ext = Some(asset.extension.clone());

    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(state.identity.account.id.clone()),
            action: "book.cover_replace".to_string(),
            resource_type: Some(ResourceType::Book),
            resource_id: Some(book_id.clone()),
            metadata: serde_json::json!({ "coverHash": asset.hash }).to_string(),
        })
        .await?;

    if let Some((old_hash, old_ext)) = previous
        && old_hash != asset.hash
    {
        let old_cover = AssetRef {
            kind: AssetKind::Cover,
            hash: old_hash.clone(),
            extension: old_ext,
        };
        if !state
            .storage
            .asset_is_referenced(AssetKind::Cover, &old_hash)
            .await?
            && state.storage.assets.exists(&old_cover)?
        {
            let path = state.storage.assets.path(&old_cover)?;
            crate::storage::trash::move_to_trash(state.storage.layout(), &path)?;
        }
    }

    book_summary(&state, &book).await
}

#[tauri::command]
pub async fn library_assign_category(
    state: State<'_, AppState>,
    book_id: String,
    category_id: String,
) -> Result<()> {
    let owner = state.identity.account.id.clone();

    let book = state
        .storage
        .books
        .get(&book_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("libro {book_id}")))?;

    let category = state
        .storage
        .categories
        .get(&category_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("categoría {category_id}")))?;

    if book.owner_account_id != owner || category.owner_account_id != owner {
        return Err(KoraError::NotFound(
            "libro o categoría de otra cuenta".into(),
        ));
    }

    state
        .storage
        .book_categories
        .assign(&book_id, &category_id, &owner, None)
        .await?;

    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(owner),
            action: "book.category_assign".to_string(),
            resource_type: Some(ResourceType::Book),
            resource_id: Some(book_id),
            metadata: serde_json::json!({ "categoryId": category_id }).to_string(),
        })
        .await?;

    Ok(())
}

#[tauri::command]
pub async fn category_create(
    state: State<'_, AppState>,
    name: String,
    color: Option<String>,
) -> Result<Category> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(KoraError::InvalidInput(
            "el nombre de la categoría no puede estar vacío".into(),
        ));
    }

    let owner = state.identity.account.id.clone();

    let normalized = crate::classify::normalize::normalize_key(&name);
    if let Some(existing) = state
        .storage
        .categories
        .list_by_owner(&owner)
        .await?
        .into_iter()
        .find(|category| crate::classify::normalize::normalize_key(&category.name) == normalized)
    {
        return Ok(existing);
    }

    let category = state
        .storage
        .categories
        .create(NewCategory {
            name,
            color,
            sort_order: 0,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await?;

    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(owner),
            action: "category.create".to_string(),
            resource_type: Some(ResourceType::Category),
            resource_id: Some(category.id.clone()),
            metadata: serde_json::json!({ "name": category.name }).to_string(),
        })
        .await?;

    Ok(category)
}

#[tauri::command]
pub async fn library_update(
    state: State<'_, AppState>,
    book_id: String,
    title: String,
    author: Option<String>,
    description: Option<String>,
    language: Option<String>,
    publisher: Option<String>,
) -> Result<BookSummary> {
    let mut book = owned_book(&state, &book_id).await?;

    let title = title.trim().to_string();
    if title.is_empty() {
        return Err(KoraError::InvalidInput(
            "el título del libro no puede estar vacío".into(),
        ));
    }
    if title.chars().count() > 300 {
        return Err(KoraError::InvalidInput(
            "el título no puede superar 300 caracteres".into(),
        ));
    }

    let author = clean_optional(author);
    let description = clean_description(clean_optional(description));
    let language = clean_optional(language);
    let publisher = clean_optional(publisher);

    if author
        .as_ref()
        .is_some_and(|value| value.chars().count() > 200)
    {
        return Err(KoraError::InvalidInput(
            "el autor no puede superar 200 caracteres".into(),
        ));
    }
    if publisher
        .as_ref()
        .is_some_and(|value| value.chars().count() > 200)
    {
        return Err(KoraError::InvalidInput(
            "la editorial no puede superar 200 caracteres".into(),
        ));
    }
    if language
        .as_ref()
        .is_some_and(|value| value.chars().count() > 40)
    {
        return Err(KoraError::InvalidInput(
            "el idioma no puede superar 40 caracteres".into(),
        ));
    }
    if description
        .as_ref()
        .is_some_and(|value| value.chars().count() > 8_000)
    {
        return Err(KoraError::InvalidInput(
            "la descripción no puede superar 8000 caracteres".into(),
        ));
    }

    book.title = title;
    book.author = author;
    book.description = description;
    book.language = language;
    book.publisher = publisher;
    state.storage.books.update(&book).await?;

    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(state.identity.account.id.clone()),
            action: "book.update".to_string(),
            resource_type: Some(ResourceType::Book),
            resource_id: Some(book.id.clone()),
            metadata: serde_json::json!({ "title": book.title }).to_string(),
        })
        .await?;

    book_summary(&state, &book).await
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[tauri::command]
pub async fn library_unassign_category(
    state: State<'_, AppState>,
    book_id: String,
    category_id: String,
) -> Result<()> {
    let owner = state.identity.account.id.clone();
    let book = state
        .storage
        .books
        .get(&book_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("libro {book_id}")))?;
    let category = state
        .storage
        .categories
        .get(&category_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("categoría {category_id}")))?;

    if book.owner_account_id != owner || category.owner_account_id != owner {
        return Err(KoraError::NotFound(
            "libro o categoría de otra cuenta".into(),
        ));
    }

    state
        .storage
        .book_categories
        .unassign(&book_id, &category_id)
        .await?;

    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(owner),
            action: "book.category_unassign".to_string(),
            resource_type: Some(ResourceType::Book),
            resource_id: Some(book_id),
            metadata: serde_json::json!({ "categoryId": category_id }).to_string(),
        })
        .await?;

    Ok(())
}

#[tauri::command]
pub async fn category_update(
    state: State<'_, AppState>,
    category_id: String,
    name: String,
    color: Option<String>,
) -> Result<Category> {
    let owner = state.identity.account.id.clone();
    let mut category = state
        .storage
        .categories
        .get(&category_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("categoría {category_id}")))?;

    if category.owner_account_id != owner {
        return Err(KoraError::NotFound(format!("categoría {category_id}")));
    }

    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(KoraError::InvalidInput(
            "el nombre de la categoría no puede estar vacío".into(),
        ));
    }
    if name.chars().count() > 60 {
        return Err(KoraError::InvalidInput(
            "el nombre de la categoría no puede superar 60 caracteres".into(),
        ));
    }

    let normalized = crate::classify::normalize::normalize_key(&name);
    if state
        .storage
        .categories
        .list_by_owner(&owner)
        .await?
        .into_iter()
        .any(|other| {
            other.id != category.id
                && crate::classify::normalize::normalize_key(&other.name) == normalized
        })
    {
        return Err(KoraError::Conflict(
            "ya existe una categoría con ese nombre".into(),
        ));
    }

    let color = color
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    category.name = name;
    category.color = color;
    state.storage.categories.update(&category).await?;

    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(owner),
            action: "category.update".to_string(),
            resource_type: Some(ResourceType::Category),
            resource_id: Some(category.id.clone()),
            metadata: serde_json::json!({ "name": category.name }).to_string(),
        })
        .await?;

    Ok(category)
}

#[tauri::command]
pub async fn category_delete(state: State<'_, AppState>, category_id: String) -> Result<()> {
    let owner = state.identity.account.id.clone();
    let category = state
        .storage
        .categories
        .get(&category_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("categoría {category_id}")))?;

    if category.owner_account_id != owner {
        return Err(KoraError::NotFound(format!("categoría {category_id}")));
    }

    state.storage.categories.delete(&category_id).await?;

    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(owner),
            action: "category.delete".to_string(),
            resource_type: Some(ResourceType::Category),
            resource_id: Some(category_id),
            metadata: serde_json::json!({ "name": category.name }).to_string(),
        })
        .await?;

    Ok(())
}

#[tauri::command]
pub async fn library_set_favorite(
    state: State<'_, AppState>,
    book_id: String,
    favorite: bool,
) -> Result<BookSummary> {
    let book = owned_book(&state, &book_id).await?;
    state
        .storage
        .books
        .set_favorite(&state.identity.account.id, &book.id, favorite)
        .await?;
    book_summary(&state, &book).await
}

#[tauri::command]
pub async fn category_list(state: State<'_, AppState>) -> Result<Vec<Category>> {
    state
        .storage
        .categories
        .list_by_owner(&state.identity.account.id)
        .await
}

async fn book_summary(state: &AppState, book: &Book) -> Result<BookSummary> {
    book_summary_for(&state.storage, book).await
}

/// Variante sin `AppState` para tareas en segundo plano que solo conservan el
/// `Storage` clonado.
pub(crate) async fn book_summary_for(storage: &Storage, book: &Book) -> Result<BookSummary> {
    let progress = storage.progress.get(&book.id).await?;
    let percent = progress
        .as_ref()
        .map(|progress| progress.percent as f32)
        .unwrap_or(0.0);
    let last_read_at = progress.as_ref().map(|progress| progress.updated_at);

    Ok(BookSummary {
        id: book.id.clone(),
        title: book.title.clone(),
        author: book.author.clone(),
        format: book.format,
        cover_path: cover_path_for(
            storage,
            book.cover_hash.as_deref(),
            book.cover_ext.as_deref(),
        )?,
        size_bytes: book.size_bytes.max(0) as u64,
        added_at: book.added_at,
        language: book.language.clone(),
        publisher: book.publisher.clone(),
        description: clean_description(book.description.clone()),
        last_read_at,
        progress: percent,
        origin: book.origin,
        favorite: storage.books.is_favorite(&book.id).await?,
        read_only: book.read_only,
        received_share_id: book.received_share_id.clone(),
        category_ids: storage
            .book_categories
            .list_by_book(&book.id)
            .await?
            .into_iter()
            .map(|category| category.id)
            .collect(),
    })
}

fn cover_path(
    state: &AppState,
    hash: Option<&str>,
    extension: Option<&str>,
) -> Result<Option<String>> {
    cover_path_for(&state.storage, hash, extension)
}

pub(crate) fn cover_path_for(
    storage: &Storage,
    hash: Option<&str>,
    extension: Option<&str>,
) -> Result<Option<String>> {
    let (Some(hash), Some(extension)) = (hash, extension) else {
        return Ok(None);
    };

    let reference = AssetRef {
        kind: AssetKind::Cover,
        hash: hash.to_string(),
        extension: extension.to_string(),
    };

    Ok(Some(storage.assets.path(&reference)?.display().to_string()))
}

fn detect_book_format(path: &Path) -> Result<BookFormat> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();

    BookFormat::from_extension(extension).ok_or_else(|| {
        KoraError::Unsupported(format!(
            "formato de libro no soportado: {extension:?} (se espera epub o pdf)"
        ))
    })
}

fn detect_image_extension(path: &Path) -> Result<&'static str> {
    if let Some(extension) = sniff_image_extension(path) {
        return Ok(extension);
    }

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "jpg" | "jpeg" => Ok("jpg"),
        "png" => Ok("png"),
        "webp" => Ok("webp"),
        other => Err(KoraError::Unsupported(format!(
            "formato de portada no soportado: {other:?} (se espera jpg, png o webp)"
        ))),
    }
}

/// Reconoce la portada por su firma. Así no depende de que el nombre del
/// archivo traiga extensión (por ejemplo, al copiarlo desde una URI de Android).
fn sniff_image_extension(path: &Path) -> Option<&'static str> {
    let mut header = [0u8; 12];
    let read = std::fs::File::open(path).ok()?.read(&mut header).ok()?;
    let header = &header[..read];

    if header.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    if header.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("png");
    }
    if header.len() >= 12 && &header[..4] == b"RIFF" && &header[8..12] == b"WEBP" {
        return Some("webp");
    }

    None
}

#[cfg(test)]
mod tests {
    use super::detect_image_extension;
    use std::io::Write;

    fn temp_image(suffix: &str, bytes: &[u8]) -> tempfile::NamedTempFile {
        let mut file = tempfile::Builder::new()
            .suffix(suffix)
            .tempfile()
            .expect("archivo temporal");
        file.write_all(bytes).expect("escribir imagen");
        file
    }

    #[test]
    fn detects_cover_format_by_content() {
        let jpeg = temp_image("", &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10]);
        assert_eq!(detect_image_extension(jpeg.path()).expect("jpeg"), "jpg");

        let png = temp_image("", &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(detect_image_extension(png.path()).expect("png"), "png");

        let webp = temp_image("", b"RIFF\x24\x00\x00\x00WEBP");
        assert_eq!(detect_image_extension(webp.path()).expect("webp"), "webp");
    }

    #[test]
    fn falls_back_to_extension_when_content_is_unknown() {
        let jpg = temp_image(".jpg", b"no es una imagen");
        assert_eq!(detect_image_extension(jpg.path()).expect("jpg"), "jpg");
    }

    #[test]
    fn rejects_unsupported_formats() {
        let heic = temp_image(".heic", b"no es una imagen");
        assert!(detect_image_extension(heic.path()).is_err());
    }
}
