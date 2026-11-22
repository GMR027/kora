use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Tipo de recurso que puede compartirse o sincronizarse. Se guarda como
/// texto en SQLite para que el esquema no dependa de los nombres de variantes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum ResourceType {
    Book,
    ReadingProgress,
    Annotation,
    Extract,
    Note,
    VoiceNote,
    Reaction,
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
            Self::VoiceNote => "voice_note",
            Self::Reaction => "reaction",
            Self::Voice => "voice",
            Self::Audiobook => "audiobook",
            Self::DiaryEntry => "diary_entry",
            Self::Achievement => "achievement",
            Self::Category => "category",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum ShareScopeType {
    Library,
    Book,
    Voice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum SharePermission {
    Read,
    Copy,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    pub id: String,
    pub owner_account_id: String,
    pub resource_type: ResourceType,
    pub resource_id: String,
    pub scope_type: ShareScopeType,
    pub permission: SharePermission,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_uses: Option<i64>,
    pub uses: i64,
    #[serde(skip)]
    pub share_key_hash: Option<String>,
    #[serde(skip)]
    pub share_key_ciphertext: Option<String>,
    #[serde(skip)]
    pub share_key_nonce: Option<String>,
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
    pub scope_type: ShareScopeType,
    pub permission: SharePermission,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_uses: Option<i64>,
    pub share_key_hash: Option<String>,
    pub share_key_ciphertext: Option<String>,
    pub share_key_nonce: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ReceivedShare {
    pub share_id: String,
    pub owner_account_id: String,
    pub owner_username: Option<String>,
    pub owner_public_key: Option<String>,
    pub owner_account_hint: Option<String>,
    pub owner_device_id: Option<String>,
    pub scope_type: ShareScopeType,
    /// Distingue un audiolibro de un libro normal. Los shares de audiolibro
    /// usan `scope_type = book` para respetar el CHECK de SQLite existente.
    #[serde(default = "default_shared_resource_type")]
    pub resource_type: ResourceType,
    pub permission: SharePermission,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_uses: Option<i64>,
    pub uses: i64,
    pub revoked_at: Option<DateTime<Utc>>,
    pub accepted_at: DateTime<Utc>,
    pub owner_signature: Option<String>,
    pub peer_account_signature: Option<String>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub owner_account_signature: Option<String>,
}

fn default_shared_resource_type() -> ResourceType {
    ResourceType::Book
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewReceivedShare {
    pub share_id: String,
    pub owner_account_id: String,
    pub owner_username: Option<String>,
    pub owner_public_key: Option<String>,
    pub owner_account_hint: Option<String>,
    pub owner_device_id: Option<String>,
    pub scope_type: ShareScopeType,
    pub resource_type: ResourceType,
    pub permission: SharePermission,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_uses: Option<i64>,
    pub uses: i64,
    pub owner_signature: Option<String>,
    pub peer_account_signature: Option<String>,
    pub owner_account_signature: Option<String>,
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
