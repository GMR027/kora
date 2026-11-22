pub mod accounts;
pub mod annotations;
pub mod appearance;
pub mod books;
pub mod categories;
pub mod classification;
pub mod diary;
pub mod pdf;
pub mod reader_settings;
pub mod semantic;
pub mod settings;
pub mod sharing;
pub mod sync;
pub mod tts;
pub mod voices;

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::Result;
use crate::storage::model::ResourceType;

/// Identificadores ordenables por tiempo (UUIDv7). La fase 9 los usará como
/// claves estables de CRDT; los registros antiguos conservan su UUIDv4.
pub(crate) fn new_id() -> String {
    Uuid::now_v7().to_string()
}

pub(crate) fn now() -> DateTime<Utc> {
    Utc::now()
}

pub(crate) async fn track_owner(
    pool: &SqlitePool,
    resource_type: ResourceType,
    resource_id: &str,
    owner_account_id: &str,
    origin_share_id: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO resource_owners
            (resource_type, resource_id, owner_account_id, origin_share_id, updated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT (resource_type, resource_id) DO UPDATE SET
            owner_account_id = excluded.owner_account_id,
            origin_share_id = excluded.origin_share_id,
            updated_at = excluded.updated_at",
    )
    .bind(resource_type)
    .bind(resource_id)
    .bind(owner_account_id)
    .bind(origin_share_id)
    .bind(now())
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn untrack_owner(
    pool: &SqlitePool,
    resource_type: ResourceType,
    resource_id: &str,
) -> Result<()> {
    sqlx::query("DELETE FROM resource_owners WHERE resource_type = ? AND resource_id = ?")
        .bind(resource_type)
        .bind(resource_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub(crate) fn require_affected(result: sqlx::sqlite::SqliteQueryResult, what: &str) -> Result<()> {
    if result.rows_affected() == 0 {
        Err(crate::error::KoraError::NotFound(what.to_string()))
    } else {
        Ok(())
    }
}
