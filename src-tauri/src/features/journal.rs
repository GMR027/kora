use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{KoraError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    pub id: String,
    pub book_id: Option<String>,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait JournalService: Send + Sync {
    async fn list(&self, book_id: Option<&str>) -> Result<Vec<JournalEntry>>;
    async fn save(&self, entry: &JournalEntry) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
}

pub struct SqliteJournalService;

#[async_trait]
impl JournalService for SqliteJournalService {
    async fn list(&self, _book_id: Option<&str>) -> Result<Vec<JournalEntry>> {
        Ok(Vec::new())
    }

    async fn save(&self, _entry: &JournalEntry) -> Result<()> {
        Err(KoraError::not_implemented(
            "features::SqliteJournalService::save (fase 7)",
        ))
    }

    async fn delete(&self, _id: &str) -> Result<()> {
        Err(KoraError::not_implemented(
            "features::SqliteJournalService::delete",
        ))
    }
}
