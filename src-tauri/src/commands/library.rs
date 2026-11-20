use std::path::{Path, PathBuf};

use tauri::State;

use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::storage::{
    AssetKind, AssetRef, Book, BookFormat, BookSummary, Category, NewAuditEntry, NewBook,
    NewCategory, ResourceType,
};

#[tauri::command]
pub async fn library_list(state: State<'_, AppState>) -> Result<Vec<BookSummary>> {
    let owner = state.identity.account.id.clone();
    let rows = state.storage.books.list_summaries(&owner).await?;

    let mut books = Vec::with_capacity(rows.len());
    for row in rows {
        let cover_path = cover_path(&state, row.cover_hash.as_deref(), row.cover_ext.as_deref())?;

        books.push(BookSummary {
            id: row.id,
            title: row.title,
            author: row.author,
            format: row.format,
            cover_path,
            size_bytes: row.size_bytes.max(0) as u64,
            added_at: row.added_at,
            progress: row.progress as f32,
        });
    }

    Ok(books)
}

#[tauri::command]
pub async fn library_import(state: State<'_, AppState>, source: String) -> Result<BookSummary> {
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

    if let Some(existing) = state
        .storage
        .books
        .find_by_hash(&owner, &asset.hash)
        .await?
    {
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

    let book = state
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
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await?;

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

    book_summary(&state, &book).await
}

#[tauri::command]
pub async fn library_delete(state: State<'_, AppState>, book_id: String) -> Result<()> {
    let book = state
        .storage
        .books
        .get(&book_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("libro {book_id}")))?;

    state.storage.books.delete(&book_id).await?;

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

    if let Some(existing) = state.storage.categories.find_by_name(&owner, &name).await? {
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
pub async fn category_list(state: State<'_, AppState>) -> Result<Vec<Category>> {
    state
        .storage
        .categories
        .list_by_owner(&state.identity.account.id)
        .await
}

async fn book_summary(state: &AppState, book: &Book) -> Result<BookSummary> {
    let progress = state
        .storage
        .progress
        .get(&book.id)
        .await?
        .map(|progress| progress.percent as f32)
        .unwrap_or(0.0);

    Ok(BookSummary {
        id: book.id.clone(),
        title: book.title.clone(),
        author: book.author.clone(),
        format: book.format,
        cover_path: cover_path(state, book.cover_hash.as_deref(), book.cover_ext.as_deref())?,
        size_bytes: book.size_bytes.max(0) as u64,
        added_at: book.added_at,
        progress,
    })
}

fn cover_path(
    state: &AppState,
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

    Ok(Some(
        state.storage.assets.path(&reference)?.display().to_string(),
    ))
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
