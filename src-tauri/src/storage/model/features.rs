use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Estado del índice semántico de un libro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum SemanticIndexStatus {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SemanticBookIndex {
    pub book_id: String,
    pub model: String,
    pub status: SemanticIndexStatus,
    pub chunks: i64,
    pub error: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SemanticChunk {
    pub id: String,
    pub book_id: String,
    pub section: i64,
    pub chunk_index: i64,
    pub section_title: Option<String>,
    pub content: String,
    pub offset_start: i64,
    pub offset_end: i64,
    pub content_hash: String,
    pub model: String,
    pub dimensions: i64,
    pub embedding: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewSemanticChunk {
    pub id: String,
    pub book_id: String,
    pub section: i64,
    pub chunk_index: i64,
    pub section_title: Option<String>,
    pub content: String,
    pub offset_start: i64,
    pub offset_end: i64,
    pub content_hash: String,
    pub model: String,
    pub dimensions: i64,
    pub embedding: Vec<u8>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}
