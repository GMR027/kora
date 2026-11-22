use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::sharing::ResourceType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum SyncState {
    Pending,
    Syncing,
    Synced,
    Conflict,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SyncEntry {
    pub resource_type: ResourceType,
    pub resource_id: String,
    pub peer_device_id: String,
    pub version: i64,
    pub hash: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub state: SyncState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSyncEntry {
    pub resource_type: ResourceType,
    pub resource_id: String,
    pub peer_device_id: String,
    pub version: i64,
    pub hash: Option<String>,
    pub state: SyncState,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SyncPending {
    pub resource_type: ResourceType,
    pub resource_id: String,
    pub changed_at: DateTime<Utc>,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CrdtDocument {
    pub resource_type: ResourceType,
    pub resource_id: String,
    pub snapshot: Vec<u8>,
    pub hash: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SyncCollection {
    pub device_id: String,
    pub category_id: String,
    pub enabled: bool,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ResourceOwner {
    pub resource_type: ResourceType,
    pub resource_id: String,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub id: String,
    pub actor_account_id: Option<String>,
    pub action: String,
    pub resource_type: Option<ResourceType>,
    pub resource_id: Option<String>,
    pub metadata: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAuditEntry {
    pub actor_account_id: Option<String>,
    pub action: String,
    pub resource_type: Option<ResourceType>,
    pub resource_id: Option<String>,
    pub metadata: String,
}
