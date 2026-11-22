use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Estado del trabajo de OCR para un PDF. Un libro solo puede tener un job a
/// la vez; si la app se cierra a mitad, al arrancar se reanuda como `pending`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum OcrStatus {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
}

impl OcrStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Skipped)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PdfDocumentInfo {
    pub book_id: String,
    pub page_count: i64,
    pub is_scanned: bool,
    pub has_native_outline: bool,
    pub inspected_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PdfOcrState {
    pub book_id: String,
    pub status: OcrStatus,
    pub pages_done: i64,
    pub pages_total: i64,
    pub language: String,
    pub error: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

impl PdfOcrState {
    pub fn pending(book_id: String, pages_total: i64, owner_account_id: String) -> Self {
        Self {
            book_id,
            status: OcrStatus::Pending,
            pages_done: 0,
            pages_total,
            language: "spa+eng".to_string(),
            error: None,
            updated_at: Utc::now(),
            owner_account_id,
            origin_share_id: None,
        }
    }

    pub fn progress(&self) -> f64 {
        if self.pages_total <= 0 {
            return 0.0;
        }
        (self.pages_done as f64 / self.pages_total as f64).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PdfAutoIndexRecord {
    pub book_id: String,
    pub source: String,
    pub entries: String,
    pub generated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

/// Entrada de índice común para bookmarks nativos y heurísticas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoIndexEntry {
    pub title: String,
    pub page: usize,
    pub level: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoIndex {
    pub book_id: String,
    pub source: String,
    pub entries: Vec<AutoIndexEntry>,
    pub generated_at: DateTime<Utc>,
}
