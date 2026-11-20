use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum BookFormat {
    Epub,
    Pdf,
}

impl BookFormat {
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "epub" => Some(Self::Epub),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Epub => "epub",
            Self::Pdf => "pdf",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub format: BookFormat,
    pub content_hash: String,
    pub content_ext: String,
    pub original_name: String,
    pub cover_hash: Option<String>,
    pub cover_ext: Option<String>,
    pub size_bytes: i64,
    pub language: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub added_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewBook {
    pub title: String,
    pub author: Option<String>,
    pub format: BookFormat,
    pub content_hash: String,
    pub content_ext: String,
    pub original_name: String,
    pub cover_hash: Option<String>,
    pub cover_ext: Option<String>,
    pub size_bytes: i64,
    pub language: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

/// Proyección para la biblioteca: evita cargar columnas que el frontend no usa.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BookListRow {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub format: BookFormat,
    pub cover_hash: Option<String>,
    pub cover_ext: Option<String>,
    pub size_bytes: i64,
    pub added_at: DateTime<Utc>,
    pub progress: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookSummary {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub format: BookFormat,
    pub cover_path: Option<String>,
    pub size_bytes: u64,
    pub added_at: DateTime<Utc>,
    pub progress: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ReadingProgress {
    pub book_id: String,
    pub locator: String,
    pub percent: f64,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewReadingProgress {
    pub book_id: String,
    pub locator: String,
    pub percent: f64,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCategory {
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i64,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum AnnotationKind {
    Highlight,
    Underline,
    Bookmark,
    Strike,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub id: String,
    pub book_id: String,
    pub kind: AnnotationKind,
    pub locator: String,
    pub text: Option<String>,
    pub note: Option<String>,
    pub color: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAnnotation {
    pub book_id: String,
    pub kind: AnnotationKind,
    pub locator: String,
    pub text: Option<String>,
    pub note: Option<String>,
    pub color: Option<String>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Extract {
    pub id: String,
    pub book_id: String,
    pub locator: String,
    pub text: String,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewExtract {
    pub book_id: String,
    pub locator: String,
    pub text: String,
    pub note: Option<String>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub book_id: Option<String>,
    pub title: Option<String>,
    pub body: String,
    pub audio_hash: Option<String>,
    pub audio_ext: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewNote {
    pub book_id: Option<String>,
    pub title: Option<String>,
    pub body: String,
    pub audio_hash: Option<String>,
    pub audio_ext: Option<String>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    pub id: String,
    pub name: String,
    pub engine: String,
    pub language: String,
    pub profile_hash: Option<String>,
    pub profile_ext: Option<String>,
    pub sample_hash: Option<String>,
    pub sample_ext: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewVoice {
    pub name: String,
    pub engine: String,
    pub language: String,
    pub profile_hash: Option<String>,
    pub profile_ext: Option<String>,
    pub sample_hash: Option<String>,
    pub sample_ext: Option<String>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum AudiobookStatus {
    Pending,
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Audiobook {
    pub id: String,
    pub book_id: String,
    pub voice_id: Option<String>,
    pub status: AudiobookStatus,
    pub manifest_hash: Option<String>,
    pub manifest_ext: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAudiobook {
    pub book_id: String,
    pub voice_id: Option<String>,
    pub status: AudiobookStatus,
    pub manifest_hash: Option<String>,
    pub manifest_ext: Option<String>,
    pub duration_ms: Option<i64>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DiaryEntry {
    pub id: String,
    pub book_id: Option<String>,
    pub entry_date: String,
    pub body: String,
    pub mood: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewDiaryEntry {
    pub book_id: Option<String>,
    pub entry_date: String,
    pub body: String,
    pub mood: Option<String>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    pub id: String,
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    pub progress: f64,
    pub unlocked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAchievement {
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    pub progress: f64,
    pub unlocked_at: Option<DateTime<Utc>>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}
