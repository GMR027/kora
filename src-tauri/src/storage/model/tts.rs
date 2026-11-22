use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unidad de los offsets de una secuencia expresiva.
///
/// - `utf16`: offsets de código UTF-16 dentro del texto del capítulo (EPUB).
/// - `word`: índice de palabra dentro de la capa de texto (PDF/O CR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum OffsetUnit {
    Utf16,
    Word,
}

impl OffsetUnit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Utf16 => "utf16",
            Self::Word => "word",
        }
    }
}

/// Secuencia expresiva ya calculada para un capítulo o página. El JSON vive
/// en `sequence`; la clave incluye el hash del texto para invalidar la caché
/// cuando cambia el OCR o el contenido del capítulo.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TtsSequenceRecord {
    pub book_id: String,
    pub section: i64,
    pub content_hash: String,
    pub unit: OffsetUnit,
    pub sequence: String,
    pub created_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewTtsSequence {
    pub book_id: String,
    pub section: i64,
    pub content_hash: String,
    pub unit: OffsetUnit,
    pub sequence: String,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CharacterVoice {
    pub book_id: String,
    pub character: String,
    pub voice_id: String,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewCharacterVoice {
    pub book_id: String,
    pub character: String,
    pub voice_id: String,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TtsPreferences {
    pub book_id: String,
    pub voice_id: Option<String>,
    pub rate: f64,
    pub pitch: f64,
    pub pause_scale: f64,
    pub expressive: bool,
    pub multi_voice: bool,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

impl TtsPreferences {
    pub fn defaults(book_id: String, owner_account_id: String) -> Self {
        Self {
            book_id,
            voice_id: None,
            rate: 1.0,
            pitch: 1.0,
            pause_scale: 1.0,
            expressive: true,
            multi_voice: true,
            updated_at: Utc::now(),
            owner_account_id,
            origin_share_id: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewTtsPreferences {
    pub book_id: String,
    pub voice_id: Option<String>,
    pub rate: f64,
    pub pitch: f64,
    pub pause_scale: f64,
    pub expressive: bool,
    pub multi_voice: bool,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PracticeAttempt {
    pub id: String,
    pub book_id: String,
    pub section: i64,
    pub offset_start: i64,
    pub offset_end: i64,
    pub expected: String,
    pub transcript: String,
    pub accuracy: f64,
    pub diff: String,
    pub created_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewPracticeAttempt {
    pub id: String,
    pub book_id: String,
    pub section: i64,
    pub offset_start: i64,
    pub offset_end: i64,
    pub expected: String,
    pub transcript: String,
    pub accuracy: f64,
    pub diff: String,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}
