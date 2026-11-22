use chrono::Utc;
use sqlx::SqlitePool;

use crate::error::{KoraError, Result};
use crate::storage::model::{
    AutoIndex, AutoIndexEntry, PdfAutoIndexRecord, PdfDocumentInfo, PdfOcrState,
};

/// Repositorio de los datos derivados de un PDF: información del documento,
/// estado del OCR y auto-índice. Nunca toca el archivo PDF original.
#[derive(Clone)]
pub struct PdfRepo {
    pool: SqlitePool,
}

impl PdfRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn upsert_info(&self, info: &PdfDocumentInfo) -> Result<()> {
        sqlx::query(
            "INSERT INTO pdf_document_info
                (book_id, page_count, is_scanned, has_native_outline, inspected_at,
                 owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (book_id) DO UPDATE SET
                page_count = excluded.page_count,
                is_scanned = excluded.is_scanned,
                has_native_outline = excluded.has_native_outline,
                inspected_at = excluded.inspected_at,
                owner_account_id = excluded.owner_account_id,
                origin_share_id = excluded.origin_share_id",
        )
        .bind(&info.book_id)
        .bind(info.page_count)
        .bind(info.is_scanned)
        .bind(info.has_native_outline)
        .bind(info.inspected_at)
        .bind(&info.owner_account_id)
        .bind(&info.origin_share_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_info(&self, book_id: &str) -> Result<Option<PdfDocumentInfo>> {
        Ok(sqlx::query_as::<_, PdfDocumentInfo>(
            "SELECT * FROM pdf_document_info WHERE book_id = ?",
        )
        .bind(book_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn upsert_ocr_state(&self, state: &PdfOcrState) -> Result<()> {
        sqlx::query(
            "INSERT INTO pdf_ocr_status
                (book_id, status, pages_done, pages_total, language, error, updated_at,
                 owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (book_id) DO UPDATE SET
                status = excluded.status,
                pages_done = excluded.pages_done,
                pages_total = excluded.pages_total,
                language = excluded.language,
                error = excluded.error,
                updated_at = excluded.updated_at,
                owner_account_id = excluded.owner_account_id,
                origin_share_id = excluded.origin_share_id",
        )
        .bind(&state.book_id)
        .bind(state.status)
        .bind(state.pages_done)
        .bind(state.pages_total)
        .bind(&state.language)
        .bind(&state.error)
        .bind(state.updated_at)
        .bind(&state.owner_account_id)
        .bind(&state.origin_share_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_ocr_state(&self, book_id: &str) -> Result<Option<PdfOcrState>> {
        Ok(
            sqlx::query_as::<_, PdfOcrState>("SELECT * FROM pdf_ocr_status WHERE book_id = ?")
                .bind(book_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    /// Los jobs que quedaron en `running` tras cerrar la app no tienen proceso
    /// vivo; se marcan como `pending` para poder reanudarlos.
    pub async fn reset_interrupted_jobs(&self) -> Result<()> {
        sqlx::query(
            "UPDATE pdf_ocr_status
             SET status = 'pending', error = NULL, updated_at = ?
             WHERE status = 'running'",
        )
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn save_auto_index(
        &self,
        book_id: &str,
        source: &str,
        entries: &[AutoIndexEntry],
        owner_account_id: &str,
        origin_share_id: Option<&str>,
    ) -> Result<AutoIndex> {
        let generated_at = Utc::now();
        let json = serde_json::to_string(entries)?;

        sqlx::query(
            "INSERT INTO pdf_auto_index
                (book_id, source, entries, generated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (book_id) DO UPDATE SET
                source = excluded.source,
                entries = excluded.entries,
                generated_at = excluded.generated_at,
                owner_account_id = excluded.owner_account_id,
                origin_share_id = excluded.origin_share_id",
        )
        .bind(book_id)
        .bind(source)
        .bind(&json)
        .bind(generated_at)
        .bind(owner_account_id)
        .bind(origin_share_id)
        .execute(&self.pool)
        .await?;

        Ok(AutoIndex {
            book_id: book_id.to_string(),
            source: source.to_string(),
            entries: entries.to_vec(),
            generated_at,
        })
    }

    pub async fn get_auto_index(&self, book_id: &str) -> Result<Option<AutoIndex>> {
        let record = sqlx::query_as::<_, PdfAutoIndexRecord>(
            "SELECT * FROM pdf_auto_index WHERE book_id = ?",
        )
        .bind(book_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(record) = record else {
            return Ok(None);
        };

        let entries: Vec<AutoIndexEntry> =
            serde_json::from_str(&record.entries).map_err(|error| {
                KoraError::Database(format!(
                    "auto-índice corrupto para {}: {error}",
                    record.book_id
                ))
            })?;

        Ok(Some(AutoIndex {
            book_id: record.book_id,
            source: record.source,
            entries,
            generated_at: record.generated_at,
        }))
    }
}
