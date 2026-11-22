use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AppPreferences {
    pub owner_account_id: String,
    pub theme_id: String,
    pub texture: String,
    pub locale: String,
    pub onboarding_done: bool,
    pub library_layout: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CustomTheme {
    pub id: String,
    pub name: String,
    pub background: String,
    pub foreground: String,
    pub accent: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
}

#[derive(Debug, Clone)]
pub struct NewCustomTheme {
    pub id: String,
    pub name: String,
    pub background: String,
    pub foreground: String,
    pub accent: String,
    pub owner_account_id: String,
}
