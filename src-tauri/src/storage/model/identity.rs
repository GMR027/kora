use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub display_name: String,
    pub email: Option<String>,
    pub is_local: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAccount {
    pub display_name: String,
    pub email: Option<String>,
    pub is_local: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub account_id: String,
    pub name: String,
    pub platform: String,
    pub public_key: Option<String>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub is_local: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewDevice {
    pub account_id: String,
    pub name: String,
    pub platform: String,
    pub public_key: Option<String>,
    pub is_local: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum ConnectionKeyKind {
    Ed25519,
    X25519,
    Pairing,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionKey {
    pub id: String,
    pub account_id: String,
    pub device_id: Option<String>,
    pub kind: ConnectionKeyKind,
    pub public_key: String,
    pub secret_ref: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewConnectionKey {
    pub account_id: String,
    pub device_id: Option<String>,
    pub kind: ConnectionKeyKind,
    pub public_key: String,
    pub secret_ref: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalIdentity {
    pub account: Account,
    pub device: Device,
}
