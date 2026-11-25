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

/// Procedencia del libro. `transcribed` son audiolibros externos convertidos a
/// texto y `tejido` los mini-libros creados desde extractos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum BookOrigin {
    Imported,
    Generated,
    Transcribed,
    Tejido,
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
    pub origin: BookOrigin,
    pub added_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Fecha en la que el libro pasó a la papelera; `None` si está activo.
    #[serde(default)]
    pub deleted_at: Option<DateTime<Utc>>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
    /// Copia local de un share de solo lectura.
    #[serde(default)]
    pub read_only: bool,
    /// Share del que procede una copia de solo lectura.
    #[serde(default)]
    pub received_share_id: Option<String>,
    /// Libro mínimo creado para un audiolibro importado sin transcripción.
    /// No aparece en la biblioteca hasta que se transcribe.
    #[serde(default)]
    pub audio_only: bool,
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
    pub origin: BookOrigin,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
    pub audio_only: bool,
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
    pub language: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub last_read_at: Option<DateTime<Utc>>,
    pub origin: BookOrigin,
    pub progress: f64,
    pub favorite: bool,
    pub read_only: bool,
    pub received_share_id: Option<String>,
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
    pub language: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub last_read_at: Option<DateTime<Utc>>,
    pub progress: f32,
    pub origin: BookOrigin,
    pub favorite: bool,
    pub read_only: bool,
    pub received_share_id: Option<String>,
    pub category_ids: Vec<String>,
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
    pub chapter_index: Option<i64>,
    pub offset_start: Option<i64>,
    pub offset_end: Option<i64>,
    pub text: Option<String>,
    pub note: Option<String>,
    pub color: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted: bool,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAnnotation {
    pub book_id: String,
    pub kind: AnnotationKind,
    pub locator: String,
    pub chapter_index: Option<i64>,
    pub offset_start: Option<i64>,
    pub offset_end: Option<i64>,
    pub text: Option<String>,
    pub note: Option<String>,
    pub color: Option<String>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

/// Ajustes tipográficos del lector, persistidos por libro.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ReaderSettings {
    pub book_id: String,
    pub font_size: f64,
    pub line_height: f64,
    pub margin: i64,
    pub font_family: String,
    pub font_face: String,
    pub column_width: i64,
    pub text_align: String,
    pub paragraph_indent: i64,
    pub hyphenate: bool,
    pub theme: String,
    pub texture: String,
    pub mode: String,
    pub zoom: f64,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

impl ReaderSettings {
    pub fn defaults(book_id: String, owner_account_id: String) -> Self {
        Self {
            book_id,
            font_size: 19.0,
            line_height: 1.7,
            margin: 48,
            font_family: "serif".to_string(),
            font_face: "newsreader".to_string(),
            column_width: 840,
            text_align: "left".to_string(),
            paragraph_indent: 0,
            hyphenate: false,
            theme: "inherit".to_string(),
            texture: "inherit".to_string(),
            mode: "paged".to_string(),
            zoom: 1.0,
            updated_at: Utc::now(),
            owner_account_id,
            origin_share_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewReaderSettings {
    pub book_id: String,
    pub font_size: f64,
    pub line_height: f64,
    pub margin: i64,
    pub font_family: String,
    pub font_face: String,
    pub column_width: i64,
    pub text_align: String,
    pub paragraph_indent: i64,
    pub hyphenate: bool,
    pub theme: String,
    pub texture: String,
    pub mode: String,
    pub zoom: f64,
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
    pub chapter_index: Option<i64>,
    pub offset_start: Option<i64>,
    pub offset_end: Option<i64>,
    pub citation: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted: bool,
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
    pub chapter_index: Option<i64>,
    pub offset_start: Option<i64>,
    pub offset_end: Option<i64>,
    pub citation: Option<String>,
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
    pub content_json: Option<String>,
    pub chapter_index: Option<i64>,
    pub offset_start: Option<i64>,
    pub offset_end: Option<i64>,
    pub text: Option<String>,
    pub audio_hash: Option<String>,
    pub audio_ext: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted: bool,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewNote {
    pub book_id: Option<String>,
    pub title: Option<String>,
    pub body: String,
    pub content_json: Option<String>,
    pub chapter_index: Option<i64>,
    pub offset_start: Option<i64>,
    pub offset_end: Option<i64>,
    pub text: Option<String>,
    pub audio_hash: Option<String>,
    pub audio_ext: Option<String>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

/// Nota de voz anclada a un pasaje. El archivo se deriva del id:
/// `assets/notes_audio/<id>.opus`.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct VoiceNote {
    pub id: String,
    pub book_id: String,
    pub chapter_index: i64,
    pub offset_start: i64,
    pub offset_end: i64,
    pub text: String,
    pub duration_ms: i64,
    pub audio_hash: Option<String>,
    /// Audiolibro al que pertenece el marcador; `None` en las notas de voz
    /// normales del lector.
    #[serde(default)]
    pub audiobook_id: Option<String>,
    /// Posición de reproducción en milisegundos cuando se creó el marcador.
    #[serde(default)]
    pub position_ms: i64,
    /// Cita del libro que estaba sonando.
    #[serde(default)]
    pub quote: Option<String>,
    /// Comentario dictado después del comando de voz.
    #[serde(default)]
    pub transcript: Option<String>,
    /// `manual` para notas del lector, `voice_mark` para marcadores de voz.
    #[serde(default = "default_voice_note_source")]
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted: bool,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

fn default_voice_note_source() -> String {
    "manual".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewVoiceNote {
    /// Si se conoce de antemano, el repositorio lo usa como nombre del archivo.
    pub id: Option<String>,
    pub book_id: String,
    pub chapter_index: i64,
    pub offset_start: i64,
    pub offset_end: i64,
    pub text: String,
    pub duration_ms: i64,
    pub audio_hash: Option<String>,
    pub audiobook_id: Option<String>,
    pub position_ms: i64,
    pub quote: Option<String>,
    pub transcript: Option<String>,
    pub source: String,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum ReactionKind {
    Like,
    Surprise,
    Tension,
    Boredom,
    Emotion,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Reaction {
    pub id: String,
    pub book_id: String,
    pub chapter_index: i64,
    pub offset_start: i64,
    pub offset_end: i64,
    pub kind: ReactionKind,
    pub text: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted: bool,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewReaction {
    pub book_id: String,
    pub chapter_index: i64,
    pub offset_start: i64,
    pub offset_end: i64,
    pub kind: ReactionKind,
    pub text: Option<String>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

/// Origen de una voz: catálogo del sistema o perfil clonado por el usuario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum VoiceKind {
    System,
    Cloned,
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
    pub kind: VoiceKind,
    pub reference_duration_ms: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewVoice {
    /// Identificador explícito para perfiles clonados. Si es `None`, el
    /// repositorio genera un UUID.
    pub id: Option<String>,
    pub name: String,
    pub engine: String,
    pub language: String,
    pub profile_hash: Option<String>,
    pub profile_ext: Option<String>,
    pub sample_hash: Option<String>,
    pub sample_ext: Option<String>,
    pub kind: VoiceKind,
    pub reference_duration_ms: i64,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum AudiobookOrigin {
    Generated,
    External,
    Tejido,
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
    pub audio_hash: Option<String>,
    pub audio_ext: Option<String>,
    pub scope: Option<String>,
    pub origin: AudiobookOrigin,
    pub duration_ms: Option<i64>,
    /// Ajustes de narración serializados para reintentos.
    #[serde(default)]
    pub narration: Option<String>,
    /// Reparto multi-voz serializado para reintentos.
    #[serde(default)]
    pub cast: Option<String>,
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
    pub audio_hash: Option<String>,
    pub audio_ext: Option<String>,
    pub scope: Option<String>,
    pub origin: AudiobookOrigin,
    pub duration_ms: Option<i64>,
    pub narration: Option<String>,
    pub cast: Option<String>,
    pub owner_account_id: String,
    pub origin_share_id: Option<String>,
}

/// Colección ordenada de audiolibros. Es local de la cuenta: no viaja en la
/// sincronización normal porque los audiolibros tampoco lo hacen.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookAlbum {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub sort_order: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
}

#[derive(Debug, Clone)]
pub struct NewAudiobookAlbum {
    pub title: String,
    pub description: Option<String>,
    pub sort_order: i64,
    pub owner_account_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum AudiobookQueueStatus {
    Pending,
    Running,
}

/// Trabajo de la cola de generación. `scope` y `narration` son JSON para no
/// duplicar en SQL la forma de los tipos del dominio.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookQueueItem {
    pub id: String,
    pub book_id: String,
    pub scope: String,
    pub voice_id: String,
    pub narration: Option<String>,
    pub cast: Option<String>,
    pub origin: AudiobookOrigin,
    pub status: AudiobookQueueStatus,
    pub position: i64,
    pub created_at: DateTime<Utc>,
    pub owner_account_id: String,
}

#[derive(Debug, Clone)]
pub struct NewAudiobookQueueItem {
    pub book_id: String,
    pub scope: String,
    pub voice_id: String,
    pub narration: Option<String>,
    pub cast: Option<String>,
    pub origin: AudiobookOrigin,
    pub owner_account_id: String,
}

/// Posición de escucha persistida. Se separa de `reading_progress` porque un
/// libro puede tener varias locuciones con duraciones distintas.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookProgress {
    pub audiobook_id: String,
    pub position_ms: i64,
    pub chapter_index: i64,
    pub updated_at: DateTime<Utc>,
    pub owner_account_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum DiaryKind {
    Manual,
    Reading,
    Listening,
    Pomodoro,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DiaryEntry {
    pub id: String,
    pub book_id: Option<String>,
    pub entry_date: String,
    pub body: String,
    pub mood: Option<String>,
    pub kind: DiaryKind,
    pub session_start: Option<DateTime<Utc>>,
    pub session_end: Option<DateTime<Utc>>,
    pub duration_minutes: i64,
    pub pages_read: i64,
    pub extracts_created: i64,
    pub notes_created: i64,
    pub pomodoro_round: Option<i64>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewDiarySession {
    pub book_id: Option<String>,
    pub entry_date: String,
    pub body: String,
    pub kind: DiaryKind,
    pub session_start: DateTime<Utc>,
    pub session_end: DateTime<Utc>,
    pub duration_minutes: i64,
    pub pages_read: i64,
    pub extracts_created: i64,
    pub notes_created: i64,
    pub pomodoro_round: Option<i64>,
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
