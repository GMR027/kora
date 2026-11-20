use sqlx::SqlitePool;

use super::{new_id, now, require_affected, track_owner, untrack_owner};
use crate::error::{KoraError, Result};
use crate::storage::model::{
    Book, BookListRow, NewBook, NewReadingProgress, ReadingProgress, ResourceType,
};

#[derive(Clone)]
pub struct BookRepo {
    pool: SqlitePool,
}

impl BookRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewBook) -> Result<Book> {
        let id = new_id();
        let timestamp = now();

        sqlx::query(
            "INSERT INTO books
                (id, title, author, format, content_hash, content_ext, original_name,
                 cover_hash, cover_ext, size_bytes, language, publisher, description,
                 added_at, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.title)
        .bind(&new.author)
        .bind(new.format)
        .bind(&new.content_hash)
        .bind(&new.content_ext)
        .bind(&new.original_name)
        .bind(&new.cover_hash)
        .bind(&new.cover_ext)
        .bind(new.size_bytes)
        .bind(&new.language)
        .bind(&new.publisher)
        .bind(&new.description)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::Book,
            &id,
            &new.owner_account_id,
            new.origin_share_id.as_deref(),
        )
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("libro recién creado no encontrado".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Book>> {
        Ok(
            sqlx::query_as::<_, Book>("SELECT * FROM books WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_owner(&self, owner_account_id: &str) -> Result<Vec<Book>> {
        Ok(sqlx::query_as::<_, Book>(
            "SELECT * FROM books WHERE owner_account_id = ? ORDER BY added_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_summaries(&self, owner_account_id: &str) -> Result<Vec<BookListRow>> {
        Ok(sqlx::query_as::<_, BookListRow>(
            "SELECT b.id, b.title, b.author, b.format, b.cover_hash, b.cover_ext,
                    b.size_bytes, b.added_at, COALESCE(rp.percent, 0.0) AS progress
             FROM books b
             LEFT JOIN reading_progress rp ON rp.book_id = b.id
             WHERE b.owner_account_id = ?
             ORDER BY b.added_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn find_by_hash(
        &self,
        owner_account_id: &str,
        content_hash: &str,
    ) -> Result<Option<Book>> {
        Ok(sqlx::query_as::<_, Book>(
            "SELECT * FROM books WHERE owner_account_id = ? AND content_hash = ?",
        )
        .bind(owner_account_id)
        .bind(content_hash)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn update(&self, book: &Book) -> Result<()> {
        let result = sqlx::query(
            "UPDATE books SET
                title = ?, author = ?, language = ?, publisher = ?, description = ?,
                cover_hash = ?, cover_ext = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&book.title)
        .bind(&book.author)
        .bind(&book.language)
        .bind(&book.publisher)
        .bind(&book.description)
        .bind(&book.cover_hash)
        .bind(&book.cover_ext)
        .bind(now())
        .bind(&book.id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("libro {}", book.id))?;

        track_owner(
            &self.pool,
            ResourceType::Book,
            &book.id,
            &book.owner_account_id,
            book.origin_share_id.as_deref(),
        )
        .await
    }

    pub async fn replace_cover(&self, id: &str, cover_hash: &str, cover_ext: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE books SET cover_hash = ?, cover_ext = ?, updated_at = ? WHERE id = ?",
        )
        .bind(cover_hash)
        .bind(cover_ext)
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("libro {id}"))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Los hijos se borran por FK en cascada, pero `resource_owners` y
        // `sync_state` son polimórficos y necesitan limpieza explícita.
        let mut dependents = vec![(ResourceType::ReadingProgress, id.to_string())];

        for (resource_type, table) in [
            (ResourceType::Annotation, "annotations"),
            (ResourceType::Extract, "extracts"),
            (ResourceType::Audiobook, "audiobooks"),
        ] {
            let rows: Vec<(String,)> =
                sqlx::query_as(&format!("SELECT id FROM {table} WHERE book_id = ?"))
                    .bind(id)
                    .fetch_all(&mut *tx)
                    .await?;

            dependents.extend(
                rows.into_iter()
                    .map(|(child_id,)| (resource_type, child_id)),
            );
        }

        let result = sqlx::query("DELETE FROM books WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        if result.rows_affected() == 0 {
            return Err(KoraError::NotFound(format!("libro {id}")));
        }

        dependents.push((ResourceType::Book, id.to_string()));

        for (resource_type, resource_id) in dependents {
            sqlx::query("DELETE FROM resource_owners WHERE resource_type = ? AND resource_id = ?")
                .bind(resource_type)
                .bind(&resource_id)
                .execute(&mut *tx)
                .await?;

            sqlx::query("DELETE FROM sync_state WHERE resource_type = ? AND resource_id = ?")
                .bind(resource_type)
                .bind(&resource_id)
                .execute(&mut *tx)
                .await?;
        }

        sqlx::query(
            "UPDATE shares SET revoked_at = ?, updated_at = ?
             WHERE resource_type = ? AND resource_id = ? AND revoked_at IS NULL",
        )
        .bind(now())
        .bind(now())
        .bind(ResourceType::Book)
        .bind(id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn count_by_content_hash(&self, content_hash: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM books WHERE content_hash = ?")
            .bind(content_hash)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn count_by_cover_hash(&self, cover_hash: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM books WHERE cover_hash = ?")
            .bind(cover_hash)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }
}

#[derive(Clone)]
pub struct ReadingProgressRepo {
    pool: SqlitePool,
}

impl ReadingProgressRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn upsert(&self, new: NewReadingProgress) -> Result<ReadingProgress> {
        sqlx::query(
            "INSERT INTO reading_progress
                (book_id, locator, percent, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (book_id) DO UPDATE SET
                locator = excluded.locator,
                percent = excluded.percent,
                updated_at = excluded.updated_at,
                owner_account_id = excluded.owner_account_id,
                origin_share_id = excluded.origin_share_id",
        )
        .bind(&new.book_id)
        .bind(&new.locator)
        .bind(new.percent)
        .bind(now())
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::ReadingProgress,
            &new.book_id,
            &new.owner_account_id,
            new.origin_share_id.as_deref(),
        )
        .await?;

        self.get(&new.book_id)
            .await?
            .ok_or_else(|| KoraError::Internal("progreso recién guardado no encontrado".into()))
    }

    pub async fn get(&self, book_id: &str) -> Result<Option<ReadingProgress>> {
        Ok(
            sqlx::query_as::<_, ReadingProgress>(
                "SELECT * FROM reading_progress WHERE book_id = ?",
            )
            .bind(book_id)
            .fetch_optional(&self.pool)
            .await?,
        )
    }

    pub async fn delete(&self, book_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM reading_progress WHERE book_id = ?")
            .bind(book_id)
            .execute(&self.pool)
            .await?;

        untrack_owner(&self.pool, ResourceType::ReadingProgress, book_id).await
    }
}
