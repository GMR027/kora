use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::State;

use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::storage::{AudiobookAlbum, AudiobookStatus, NewAudiobookAlbum};

const MAX_ALBUMS_PER_ACCOUNT: i64 = 500;
const MAX_ITEMS_PER_ALBUM: usize = 500;
const MAX_TITLE_CHARS: usize = 120;
const MAX_DESCRIPTION_CHARS: usize = 2000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookAlbumInfo {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub sort_order: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Ids de audiolibros en el orden del álbum. Solo incluye audiolibros
    /// cuyos libros siguen visibles en la biblioteca.
    pub audiobook_ids: Vec<String>,
}

async fn owned_album(state: &AppState, album_id: &str) -> Result<AudiobookAlbum> {
    let owner = state.identity.account.id.clone();
    state
        .storage
        .audiobook_albums
        .get_owned(album_id, &owner)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("álbum {album_id}")))
}

async fn info_for(state: &AppState, album: AudiobookAlbum) -> Result<AudiobookAlbumInfo> {
    let owner = state.identity.account.id.clone();
    let audiobook_ids = state
        .storage
        .audiobook_albums
        .list_visible_items(&album.id, &owner)
        .await?;
    Ok(AudiobookAlbumInfo {
        id: album.id,
        title: album.title,
        description: album.description,
        sort_order: album.sort_order,
        created_at: album.created_at,
        updated_at: album.updated_at,
        audiobook_ids,
    })
}

fn normalize_title(title: &str) -> Result<String> {
    let title = title.trim();
    if title.is_empty() {
        return Err(KoraError::InvalidInput(
            "el álbum necesita un nombre".into(),
        ));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(KoraError::InvalidInput(format!(
            "el nombre del álbum no puede superar los {MAX_TITLE_CHARS} caracteres"
        )));
    }
    Ok(title.to_string())
}

fn normalize_description(description: Option<String>) -> Result<Option<String>> {
    let Some(description) = description else {
        return Ok(None);
    };
    let description = description.trim();
    if description.is_empty() {
        return Ok(None);
    }
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(KoraError::InvalidInput(format!(
            "la descripción del álbum no puede superar los {MAX_DESCRIPTION_CHARS} caracteres"
        )));
    }
    Ok(Some(description.to_string()))
}

/// Valida que los audiolibros pertenecen a la cuenta, que su libro no está en
/// la papelera y que están terminados. Devuelve los ids sin duplicados.
async fn validate_audiobooks(state: &AppState, ids: &[String]) -> Result<Vec<String>> {
    if ids.len() > MAX_ITEMS_PER_ALBUM {
        return Err(KoraError::InvalidInput(format!(
            "un álbum no puede tener más de {MAX_ITEMS_PER_ALBUM} audiolibros"
        )));
    }
    let owner = state.identity.account.id.clone();
    let mut seen = HashSet::new();
    let mut validated = Vec::with_capacity(ids.len());
    for id in ids {
        if !seen.insert(id.clone()) {
            continue;
        }
        let audiobook = state
            .storage
            .audiobooks
            .get(id)
            .await?
            .filter(|audiobook| audiobook.owner_account_id == owner)
            .ok_or_else(|| KoraError::NotFound(format!("audiolibro {id}")))?;
        if audiobook.status != AudiobookStatus::Done {
            return Err(KoraError::Conflict(
                "solo se pueden añadir audiolibros terminados a un álbum".into(),
            ));
        }
        let book = state
            .storage
            .books
            .get(&audiobook.book_id)
            .await?
            .filter(|book| book.deleted_at.is_none())
            .ok_or_else(|| KoraError::NotFound(format!("libro del audiolibro {id}")))?;
        if book.owner_account_id != owner {
            return Err(KoraError::NotFound(format!("audiolibro {id}")));
        }
        validated.push(id.clone());
    }
    Ok(validated)
}

#[tauri::command]
pub async fn audiobook_album_list(state: State<'_, AppState>) -> Result<Vec<AudiobookAlbumInfo>> {
    let owner = state.identity.account.id.clone();
    let albums = state.storage.audiobook_albums.list_by_owner(&owner).await?;
    let mut infos = Vec::with_capacity(albums.len());
    for album in albums {
        infos.push(info_for(&state, album).await?);
    }
    Ok(infos)
}

#[tauri::command]
pub async fn audiobook_album_create(
    state: State<'_, AppState>,
    title: String,
    description: Option<String>,
) -> Result<AudiobookAlbumInfo> {
    let owner = state.identity.account.id.clone();
    let count = state
        .storage
        .audiobook_albums
        .list_by_owner(&owner)
        .await?
        .len() as i64;
    if count >= MAX_ALBUMS_PER_ACCOUNT {
        return Err(KoraError::Conflict(format!(
            "no se pueden crear más de {MAX_ALBUMS_PER_ACCOUNT} álbumes"
        )));
    }
    let album = state
        .storage
        .audiobook_albums
        .create(NewAudiobookAlbum {
            title: normalize_title(&title)?,
            description: normalize_description(description)?,
            sort_order: count,
            owner_account_id: owner,
        })
        .await?;
    info_for(&state, album).await
}

#[tauri::command]
pub async fn audiobook_album_update(
    state: State<'_, AppState>,
    album_id: String,
    title: String,
    description: Option<String>,
) -> Result<AudiobookAlbumInfo> {
    let mut album = owned_album(&state, &album_id).await?;
    album.title = normalize_title(&title)?;
    album.description = normalize_description(description)?;
    state.storage.audiobook_albums.update(&album).await?;
    info_for(&state, album).await
}

#[tauri::command]
pub async fn audiobook_album_delete(state: State<'_, AppState>, album_id: String) -> Result<()> {
    let album = owned_album(&state, &album_id).await?;
    state.storage.audiobook_albums.delete(&album.id).await
}

#[tauri::command]
pub async fn audiobook_album_add(
    state: State<'_, AppState>,
    album_id: String,
    audiobook_ids: Vec<String>,
) -> Result<AudiobookAlbumInfo> {
    let album = owned_album(&state, &album_id).await?;
    let audiobook_ids = validate_audiobooks(&state, &audiobook_ids).await?;
    state
        .storage
        .audiobook_albums
        .add_items(&album.id, &album.owner_account_id, &audiobook_ids)
        .await?;
    info_for(&state, album).await
}

#[tauri::command]
pub async fn audiobook_album_remove(
    state: State<'_, AppState>,
    album_id: String,
    audiobook_id: String,
) -> Result<AudiobookAlbumInfo> {
    let album = owned_album(&state, &album_id).await?;
    state
        .storage
        .audiobook_albums
        .remove_item(&album.id, &audiobook_id)
        .await?;
    info_for(&state, album).await
}

#[tauri::command]
pub async fn audiobook_album_reorder(
    state: State<'_, AppState>,
    album_id: String,
    audiobook_ids: Vec<String>,
) -> Result<AudiobookAlbumInfo> {
    let album = owned_album(&state, &album_id).await?;
    state
        .storage
        .audiobook_albums
        .reorder(&album.id, &audiobook_ids)
        .await?;
    info_for(&state, album).await
}
