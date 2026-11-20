use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Tipo de recurso que puede compartirse o sincronizarse. Se guarda como
/// texto en SQLite para que el esquema no dependa de los nombres de variantes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum ResourceType {
    Book,
    ReadingProgress,
    Annotation,
    Extract,
    Note,
    Voice,
    Audiobook,
    DiaryEntry,
    Achievement,
    Category,
}

impl ResourceType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Book => "book",
            Self::ReadingProgress => "reading_progress",
            Self::Annotation => "annotation",
            Self::Extract => "extract",
            Self::Note => "note",
            Self::Voice => "voice",
            Self::Audiobook => "audiobook",
            Self::DiaryEntry => "diary_entry",
            Self::Achievement => "achievement",
            Self::Category => "category",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    pub id: String,
    pub owner_account_id: String,
    pub resource_type: ResourceType,
    pub resource_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewShare {
    pub owner_account_id: String,
    pub resource_type: ResourceType,
    pub resource_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ShareKey {
    pub id: String,
    pub share_id: String,
    pub recipient_account_id: Option<String>,
    pub wrapped_key: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewShareKey {
    pub share_id: String,
    pub recipient_account_id: Option<String>,
    pub wrapped_key: String,
    pub expires_at: Option<DateTime<Utc>>,
}
