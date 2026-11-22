use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::BookFormat;

/// Origen de una asignación libro-categoría.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum CategoryAssignSource {
    Manual,
    Metadata,
    Embedding,
    Ai,
    Sync,
}

/// Origen de una sugerencia de clasificación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum SuggestionSource {
    Metadata,
    Embedding,
    Ai,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum SuggestionStatus {
    Pending,
    Accepted,
    Dismissed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum AliasSource {
    Seed,
    Learned,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum ClassificationRunStatus {
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CategoryAlias {
    pub id: String,
    pub category_id: String,
    pub alias: String,
    pub alias_normalized: String,
    pub source: AliasSource,
    pub created_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewCategoryAlias {
    pub category_id: String,
    pub alias: String,
    pub alias_normalized: String,
    pub source: AliasSource,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CategorySuggestion {
    pub id: String,
    pub book_id: String,
    pub category_id: Option<String>,
    pub proposed_name: Option<String>,
    pub proposed_name_normalized: Option<String>,
    pub target_key: String,
    pub score: f64,
    pub source: SuggestionSource,
    pub reason: Option<String>,
    pub status: SuggestionStatus,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewCategorySuggestion {
    pub book_id: String,
    /// Categoría existente o nombre propuesto; exactamente uno de los dos.
    pub category_id: Option<String>,
    pub proposed_name: Option<String>,
    pub proposed_name_normalized: Option<String>,
    pub target_key: String,
    pub score: f64,
    pub source: SuggestionSource,
    pub reason: Option<String>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

impl NewCategorySuggestion {
    pub fn for_category(
        book_id: &str,
        category_id: &str,
        score: f64,
        source: SuggestionSource,
        reason: Option<String>,
        owner_account_id: &str,
        origin_share_id: Option<&str>,
    ) -> Self {
        Self {
            book_id: book_id.to_string(),
            category_id: Some(category_id.to_string()),
            proposed_name: None,
            proposed_name_normalized: None,
            target_key: format!("category:{category_id}"),
            score: score.clamp(0.0, 1.0),
            source,
            reason,
            owner_account_id: owner_account_id.to_string(),
            origin_share_id: origin_share_id.map(str::to_string),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn for_new_category(
        book_id: &str,
        proposed_name: &str,
        proposed_name_normalized: &str,
        score: f64,
        source: SuggestionSource,
        reason: Option<String>,
        owner_account_id: &str,
        origin_share_id: Option<&str>,
    ) -> Self {
        Self {
            book_id: book_id.to_string(),
            category_id: None,
            proposed_name: Some(proposed_name.to_string()),
            proposed_name_normalized: Some(proposed_name_normalized.to_string()),
            target_key: format!("name:{proposed_name_normalized}"),
            score: score.clamp(0.0, 1.0),
            source,
            reason,
            owner_account_id: owner_account_id.to_string(),
            origin_share_id: origin_share_id.map(str::to_string),
        }
    }
}

/// Vista de una sugerencia con los datos del libro que necesita la UI.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CategorySuggestionView {
    pub id: String,
    pub book_id: String,
    pub book_title: String,
    pub author: Option<String>,
    pub format: BookFormat,
    pub audio_only: bool,
    pub cover_hash: Option<String>,
    pub cover_ext: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub proposed_name: Option<String>,
    pub score: f64,
    pub source: SuggestionSource,
    pub reason: Option<String>,
    pub status: SuggestionStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct BookClassification {
    pub book_id: String,
    pub input_hash: String,
    pub model: Option<String>,
    pub status: ClassificationRunStatus,
    pub error: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
}

/// Configuración local de la clasificación. Se guarda como JSON en `settings`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClassificationConfig {
    pub enabled: bool,
    pub classify_on_import: bool,
    pub use_ai: bool,
    pub allow_new_categories: bool,
    pub embedding_threshold: f32,
    pub ai_threshold: f32,
    pub max_suggestions: u32,
}

impl Default for ClassificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            classify_on_import: true,
            use_ai: true,
            allow_new_categories: true,
            embedding_threshold: 0.80,
            ai_threshold: 0.60,
            max_suggestions: 3,
        }
    }
}

impl ClassificationConfig {
    pub fn normalize(mut self) -> Self {
        self.embedding_threshold = self.embedding_threshold.clamp(0.0, 1.0);
        self.ai_threshold = self.ai_threshold.clamp(0.0, 1.0);
        self.max_suggestions = self.max_suggestions.clamp(1, 8);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationStatus {
    pub enabled: bool,
    pub seeded: bool,
    pub ai_available: bool,
    pub embeddings_ready: bool,
    pub pending_suggestions: i64,
    pub classified_books: i64,
    pub total_books: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationProgress {
    pub book_id: String,
    pub title: String,
    pub stage: String,
    pub percent: f64,
    pub status: String,
    pub message: String,
}
