use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::storage::backup::{self, BackupInfo};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreInfo {
    pub requires_restart: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub total: u64,
    pub database: u64,
    pub library: u64,
    pub audiobooks: u64,
    pub voices: u64,
    pub models: u64,
    pub trash: u64,
    pub logs: u64,
    pub exports: u64,
    pub temporary: u64,
}

#[tauri::command]
pub async fn storage_usage(state: State<'_, AppState>) -> Result<StorageUsage> {
    let layout = state.layout.clone();
    tokio::task::spawn_blocking(move || usage(&layout))
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de almacenamiento: {error}")))?
}

#[tauri::command]
pub async fn backup_library(state: State<'_, AppState>, destination: String) -> Result<BackupInfo> {
    let layout = state.layout.clone();
    let pool = state.storage.pool().clone();
    let destination = PathBuf::from(destination);
    let snapshot = layout
        .db
        .join(format!("app.sqlite.snapshot-{}", uuid::Uuid::new_v4()));

    let escaped = snapshot.to_string_lossy().replace('\'', "''");
    sqlx::query(&format!("VACUUM INTO '{escaped}'"))
        .execute(&pool)
        .await?;

    tokio::task::spawn_blocking(move || {
        let info = backup::create(&layout, &snapshot, &destination);
        let _ = std::fs::remove_file(&snapshot);
        info
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de copia de seguridad: {error}")))?
}

#[tauri::command]
pub async fn restore_library(state: State<'_, AppState>, source: String) -> Result<RestoreInfo> {
    let layout = state.layout.clone();
    let staging_name = format!("restore-staging-{}", uuid::Uuid::new_v4());
    let staging = layout.root.join(&staging_name);
    let source = PathBuf::from(source);

    // Limpia restauraciones preparadas pero no aplicadas.
    if let Ok(entries) = std::fs::read_dir(&layout.root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("restore-staging-") {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    let staging_for_extract = staging.clone();
    tokio::task::spawn_blocking(move || backup::extract_to(&source, &staging_for_extract))
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de restauración: {error}")))??;

    let database = staging.join("db/app.sqlite");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&database)
        .read_only(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    let valid = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM schema_version")
        .fetch_one(&pool)
        .await
        .is_ok();
    pool.close().await;

    if !valid {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(KoraError::InvalidInput(
            "la copia de seguridad no contiene una base de datos válida".into(),
        ));
    }

    std::fs::write(layout.root.join("restore.pending"), &staging_name)?;
    Ok(RestoreInfo {
        requires_restart: true,
    })
}

#[tauri::command]
pub async fn purge_storage(state: State<'_, AppState>, scope: String) -> Result<StorageUsage> {
    let layout = state.layout.clone();
    let target = match scope.as_str() {
        "trash" => layout.trash.clone(),
        "temporary" => layout.root.join("tmp"),
        _ => {
            return Err(KoraError::InvalidInput(
                "solo se puede vaciar la papelera o los temporales".into(),
            ));
        }
    };
    tokio::task::spawn_blocking(move || {
        clear_directory(&target)?;
        std::fs::create_dir_all(&target)?;
        usage(&layout)
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de limpieza: {error}")))?
}

fn usage(layout: &crate::storage::StorageLayout) -> Result<StorageUsage> {
    let database = file_size(&layout.database_file());
    let library = dir_size(&layout.assets.join("books"))
        + dir_size(&layout.assets.join("covers"))
        + dir_size(&layout.assets.join("generated_books"));
    let audiobooks = dir_size(&layout.assets.join("audiobooks"));
    let voices = dir_size(&layout.assets.join("voices"))
        + dir_size(&layout.assets.join("notes_audio"))
        + dir_size(&layout.assets.join("avatars"));
    let models = dir_size(&layout.assets.join("models")) + dir_size(&layout.assets.join("whisper"));
    let trash = dir_size(&layout.trash);
    let logs = dir_size(&layout.logs);
    let exports = dir_size(&layout.exports);
    let temporary = dir_size(&layout.root.join("tmp"));
    let total =
        database + library + audiobooks + voices + models + trash + logs + exports + temporary;
    Ok(StorageUsage {
        total,
        database,
        library,
        audiobooks,
        voices,
        models,
        trash,
        logs,
        exports,
        temporary,
    })
}

fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => dir_size(&path),
                _ => file_size(&path),
            }
        })
        .sum()
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

fn clear_directory(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let target: PathBuf = entry.path();
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(target)?;
        } else {
            std::fs::remove_file(target)?;
        }
    }
    Ok(())
}
