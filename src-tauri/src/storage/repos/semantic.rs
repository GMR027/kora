use sqlx::SqlitePool;

use super::{now, require_affected};
use crate::error::{KoraError, Result};
use crate::storage::model::{
    NewSemanticChunk, SemanticBookIndex, SemanticChunk, SemanticIndexStatus,
};

#[derive(Clone)]
pub struct SemanticRepo {
    pool: SqlitePool,
}

impl SemanticRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn get_state(&self, book_id: &str) -> Result<Option<SemanticBookIndex>> {
        Ok(sqlx::query_as::<_, SemanticBookIndex>(
            "SELECT * FROM semantic_book_index WHERE book_id = ?",
        )
        .bind(book_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_states(&self, owner_account_id: &str) -> Result<Vec<SemanticBookIndex>> {
        Ok(sqlx::query_as::<_, SemanticBookIndex>(
            "SELECT * FROM semantic_book_index WHERE owner_account_id = ? ORDER BY book_id",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_state(
        &self,
        book_id: &str,
        model: &str,
        status: SemanticIndexStatus,
        chunks: i64,
        error: Option<&str>,
        owner_account_id: &str,
        origin_share_id: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO semantic_book_index
                (book_id, model, status, chunks, error, updated_at,
                 owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (book_id) DO UPDATE SET
                model = excluded.model,
                status = excluded.status,
                chunks = excluded.chunks,
                error = excluded.error,
                updated_at = excluded.updated_at,
                owner_account_id = excluded.owner_account_id,
                origin_share_id = excluded.origin_share_id",
        )
        .bind(book_id)
        .bind(model)
        .bind(status)
        .bind(chunks)
        .bind(error)
        .bind(now())
        .bind(owner_account_id)
        .bind(origin_share_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Sustituye todos los chunks de un libro para un modelo. La operación es
    /// transaccional: si falla una inserción, el índice anterior permanece.
    pub async fn replace_chunks(
        &self,
        book_id: &str,
        model: &str,
        chunks: &[NewSemanticChunk],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM semantic_chunks WHERE book_id = ? AND model = ?")
            .bind(book_id)
            .bind(model)
            .execute(&mut *tx)
            .await?;

        let timestamp = now();
        for chunk in chunks {
            sqlx::query(
                "INSERT INTO semantic_chunks
                    (id, book_id, section, chunk_index, section_title, content,
                     offset_start, offset_end, content_hash, model, dimensions,
                     embedding, created_at, updated_at, owner_account_id, origin_share_id)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&chunk.id)
            .bind(&chunk.book_id)
            .bind(chunk.section)
            .bind(chunk.chunk_index)
            .bind(&chunk.section_title)
            .bind(&chunk.content)
            .bind(chunk.offset_start)
            .bind(chunk.offset_end)
            .bind(&chunk.content_hash)
            .bind(&chunk.model)
            .bind(chunk.dimensions)
            .bind(&chunk.embedding)
            .bind(timestamp)
            .bind(timestamp)
            .bind(&chunk.owner_account_id)
            .bind(&chunk.origin_share_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn list_chunks(
        &self,
        owner_account_id: &str,
        model: &str,
    ) -> Result<Vec<SemanticChunk>> {
        Ok(sqlx::query_as::<_, SemanticChunk>(
            "SELECT * FROM semantic_chunks
             WHERE owner_account_id = ? AND model = ?",
        )
        .bind(owner_account_id)
        .bind(model)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn count_chunks(&self, owner_account_id: &str, model: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM semantic_chunks
             WHERE owner_account_id = ? AND model = ?",
        )
        .bind(owner_account_id)
        .bind(model)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    /// Un job que quedó en `running` al cerrar la app no tiene proceso vivo.
    pub async fn reset_interrupted(&self, owner_account_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE semantic_book_index
             SET status = 'pending', error = NULL, updated_at = ?
             WHERE owner_account_id = ? AND status = 'running'",
        )
        .bind(now())
        .bind(owner_account_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_state(&self, book_id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM semantic_book_index WHERE book_id = ?")
            .bind(book_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(KoraError::NotFound(format!("índice del libro {book_id}")));
        }
        Ok(())
    }

    /// Compatibilidad con el patrón de otros repos: falla si no existe.
    pub async fn touch(&self, book_id: &str, model: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE semantic_book_index SET model = ?, updated_at = ? WHERE book_id = ?",
        )
        .bind(model)
        .bind(now())
        .bind(book_id)
        .execute(&self.pool)
        .await?;
        require_affected(result, &format!("índice del libro {book_id}"))?;
        Ok(())
    }
}
