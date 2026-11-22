use chrono::Utc;

use crate::error::{KoraError, Result};
use crate::storage::{BookFormat, Storage};

/// Una anotación, extracto, nota, nota de voz o reacción reubicada en el
/// documento nuevo. El `locator` textual solo existe en anotaciones y
/// extractos; el resto usa directamente capítulo y offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorUpdate {
    pub id: String,
    pub chapter_index: i64,
    pub offset_start: i64,
    pub offset_end: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressUpdate {
    pub locator: String,
    pub percent: f64,
}

/// Datos que reemplazan al PDF dentro de la misma fila de `books`. Mantener el
/// mismo id conserva categorías, favoritos, progreso, audiolibros y anotaciones.
pub struct EpubReplacement<'a> {
    pub book_id: &'a str,
    pub title: &'a str,
    pub author: Option<&'a str>,
    pub language: Option<&'a str>,
    pub description: Option<&'a str>,
    pub content_hash: &'a str,
    pub content_ext: &'a str,
    pub original_name: &'a str,
    pub size_bytes: i64,
    pub cover_hash: Option<&'a str>,
    pub cover_ext: Option<&'a str>,
    pub progress: Option<ProgressUpdate>,
    pub annotations: Vec<AnchorUpdate>,
    pub extracts: Vec<AnchorUpdate>,
    pub notes: Vec<AnchorUpdate>,
    pub voice_notes: Vec<AnchorUpdate>,
    pub reactions: Vec<AnchorUpdate>,
}

/// Aplica el cambio de formato y toda la migración derivada en una sola
/// transacción. Si algo falla, el PDF y sus datos quedan exactamente igual.
pub async fn replace_book_with_epub(
    storage: &Storage,
    replacement: &EpubReplacement<'_>,
) -> Result<()> {
    let mut tx = storage.pool().begin().await?;
    let timestamp = Utc::now();

    let updated = sqlx::query(
        "UPDATE books SET
            title = ?, author = ?, format = ?, content_hash = ?, content_ext = ?,
            original_name = ?, cover_hash = ?, cover_ext = ?, size_bytes = ?,
            language = ?, description = ?, updated_at = ?
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(replacement.title)
    .bind(replacement.author)
    .bind(BookFormat::Epub)
    .bind(replacement.content_hash)
    .bind(replacement.content_ext)
    .bind(replacement.original_name)
    .bind(replacement.cover_hash)
    .bind(replacement.cover_ext)
    .bind(replacement.size_bytes)
    .bind(replacement.language)
    .bind(replacement.description)
    .bind(timestamp)
    .bind(replacement.book_id)
    .execute(&mut *tx)
    .await?;

    if updated.rows_affected() == 0 {
        tx.rollback().await?;
        return Err(KoraError::Conflict(
            "el PDF ya no está disponible para convertir".into(),
        ));
    }

    if let Some(progress) = replacement.progress.as_ref() {
        sqlx::query(
            "INSERT INTO reading_progress
                (book_id, locator, percent, updated_at, owner_account_id, origin_share_id)
             SELECT id, ?, ?, ?, owner_account_id, NULL FROM books WHERE id = ?
             ON CONFLICT (book_id) DO UPDATE SET
                locator = excluded.locator,
                percent = excluded.percent,
                updated_at = excluded.updated_at",
        )
        .bind(&progress.locator)
        .bind(progress.percent.clamp(0.0, 1.0))
        .bind(timestamp)
        .bind(replacement.book_id)
        .execute(&mut *tx)
        .await?;
    }

    for anchor in &replacement.annotations {
        let locator = locator(
            &anchor.chapter_index,
            anchor.offset_start,
            anchor.offset_end,
        );
        sqlx::query(
            "UPDATE annotations SET
                locator = ?, chapter_index = ?, offset_start = ?, offset_end = ?, updated_at = ?
             WHERE id = ? AND book_id = ? AND deleted = 0",
        )
        .bind(locator)
        .bind(anchor.chapter_index)
        .bind(anchor.offset_start)
        .bind(anchor.offset_end)
        .bind(timestamp)
        .bind(&anchor.id)
        .bind(replacement.book_id)
        .execute(&mut *tx)
        .await?;
    }

    for anchor in &replacement.extracts {
        let locator = locator(
            &anchor.chapter_index,
            anchor.offset_start,
            anchor.offset_end,
        );
        sqlx::query(
            "UPDATE extracts SET
                locator = ?, chapter_index = ?, offset_start = ?, offset_end = ?, updated_at = ?
             WHERE id = ? AND book_id = ? AND deleted = 0",
        )
        .bind(locator)
        .bind(anchor.chapter_index)
        .bind(anchor.offset_start)
        .bind(anchor.offset_end)
        .bind(timestamp)
        .bind(&anchor.id)
        .bind(replacement.book_id)
        .execute(&mut *tx)
        .await?;
    }

    for anchor in &replacement.notes {
        sqlx::query(
            "UPDATE notes SET
                chapter_index = ?, offset_start = ?, offset_end = ?, updated_at = ?
             WHERE id = ? AND book_id = ? AND deleted = 0",
        )
        .bind(anchor.chapter_index)
        .bind(anchor.offset_start)
        .bind(anchor.offset_end)
        .bind(timestamp)
        .bind(&anchor.id)
        .bind(replacement.book_id)
        .execute(&mut *tx)
        .await?;
    }

    for anchor in &replacement.voice_notes {
        sqlx::query(
            "UPDATE voice_notes SET
                chapter_index = ?, offset_start = ?, offset_end = ?, updated_at = ?
             WHERE id = ? AND book_id = ? AND deleted = 0",
        )
        .bind(anchor.chapter_index)
        .bind(anchor.offset_start)
        .bind(anchor.offset_end)
        .bind(timestamp)
        .bind(&anchor.id)
        .bind(replacement.book_id)
        .execute(&mut *tx)
        .await?;
    }

    for anchor in &replacement.reactions {
        sqlx::query(
            "UPDATE reactions SET
                chapter_index = ?, offset_start = ?, offset_end = ?, updated_at = ?
             WHERE id = ? AND book_id = ? AND deleted = 0",
        )
        .bind(anchor.chapter_index)
        .bind(anchor.offset_start)
        .bind(anchor.offset_end)
        .bind(timestamp)
        .bind(&anchor.id)
        .bind(replacement.book_id)
        .execute(&mut *tx)
        .await?;
    }

    // Los datos derivados del PDF ya no describen al libro. Se eliminan en la
    // misma transacción para que no queden referencias a páginas inexistentes.
    for table in ["pdf_document_info", "pdf_ocr_status", "pdf_auto_index"] {
        sqlx::query(&format!("DELETE FROM {table} WHERE book_id = ?"))
            .bind(replacement.book_id)
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query("DELETE FROM semantic_chunks WHERE book_id = ?")
        .bind(replacement.book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM semantic_book_index WHERE book_id = ?")
        .bind(replacement.book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM tts_sequences WHERE book_id = ?")
        .bind(replacement.book_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

fn locator(chapter_index: &i64, start: i64, end: i64) -> String {
    format!("chapter:{chapter_index}:{start}-{end}")
}

/// Crea un libro nuevo a partir del EPUB generado, sin tocar el PDF original.
#[allow(clippy::too_many_arguments)]
pub async fn create_converted_book(
    storage: &Storage,
    title: &str,
    author: Option<&str>,
    language: Option<&str>,
    description: Option<&str>,
    content_hash: &str,
    content_ext: &str,
    original_name: &str,
    size_bytes: i64,
    cover_hash: Option<&str>,
    cover_ext: Option<&str>,
    owner_account_id: &str,
) -> Result<crate::storage::Book> {
    storage
        .books
        .create(crate::storage::NewBook {
            title: title.to_string(),
            author: author.map(str::to_string),
            format: BookFormat::Epub,
            content_hash: content_hash.to_string(),
            content_ext: content_ext.to_string(),
            original_name: original_name.to_string(),
            cover_hash: cover_hash.map(str::to_string),
            cover_ext: cover_ext.map(str::to_string),
            size_bytes,
            language: language.map(str::to_string),
            publisher: None,
            description: description.map(str::to_string),
            origin: crate::storage::BookOrigin::Generated,
            owner_account_id: owner_account_id.to_string(),
            origin_share_id: None,
            audio_only: false,
        })
        .await
}
