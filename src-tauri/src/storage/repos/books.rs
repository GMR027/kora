use chrono::{DateTime, Utc};
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
                 origin, audio_only, added_at, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        .bind(new.origin)
        .bind(new.audio_only)
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
            "SELECT * FROM books
             WHERE owner_account_id = ? AND audio_only = 0 AND deleted_at IS NULL
             ORDER BY added_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Incluye los libros placeholder de audiolibros sin transcripción. Solo
    /// debe usarse para shares explícitos y tareas internas.
    pub async fn list_all_by_owner(&self, owner_account_id: &str) -> Result<Vec<Book>> {
        Ok(sqlx::query_as::<_, Book>(
            "SELECT * FROM books
             WHERE owner_account_id = ? AND deleted_at IS NULL
             ORDER BY added_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Libros en la papelera, del más reciente al más antiguo.
    pub async fn list_trash(&self, owner_account_id: &str) -> Result<Vec<Book>> {
        Ok(sqlx::query_as::<_, Book>(
            "SELECT * FROM books
             WHERE owner_account_id = ? AND deleted_at IS NOT NULL
             ORDER BY deleted_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_summaries(&self, owner_account_id: &str) -> Result<Vec<BookListRow>> {
        Ok(sqlx::query_as::<_, BookListRow>(
            "SELECT b.id, b.title, b.author, b.format, b.cover_hash, b.cover_ext,
                    b.size_bytes, b.added_at, b.language, b.publisher, b.description,
                    rp.updated_at AS last_read_at, b.origin,
                    COALESCE(rp.percent, 0.0) AS progress,
                    CASE WHEN bf.book_id IS NULL THEN 0 ELSE 1 END AS favorite,
                    b.read_only, b.received_share_id
             FROM books b
             LEFT JOIN reading_progress rp ON rp.book_id = b.id
             LEFT JOIN book_favorites bf ON bf.book_id = b.id
             WHERE b.owner_account_id = ? AND b.audio_only = 0 AND b.deleted_at IS NULL
             ORDER BY b.added_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn set_favorite(
        &self,
        owner_account_id: &str,
        book_id: &str,
        favorite: bool,
    ) -> Result<()> {
        if favorite {
            sqlx::query(
                "INSERT INTO book_favorites (book_id, owner_account_id, created_at)
                 VALUES (?, ?, ?)
                 ON CONFLICT (book_id) DO UPDATE SET owner_account_id = excluded.owner_account_id",
            )
            .bind(book_id)
            .bind(owner_account_id)
            .bind(now())
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query("DELETE FROM book_favorites WHERE book_id = ? AND owner_account_id = ?")
                .bind(book_id)
                .bind(owner_account_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn is_favorite(&self, book_id: &str) -> Result<bool> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM book_favorites WHERE book_id = ?")
            .bind(book_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    pub async fn find_by_hash(
        &self,
        owner_account_id: &str,
        content_hash: &str,
    ) -> Result<Option<Book>> {
        Ok(sqlx::query_as::<_, Book>(
            "SELECT * FROM books
             WHERE owner_account_id = ? AND content_hash = ? AND deleted_at IS NULL",
        )
        .bind(owner_account_id)
        .bind(content_hash)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Busca una copia en la papelera con el mismo contenido para restaurarla
    /// en lugar de crear un duplicado (la tabla tiene un índice único).
    pub async fn find_trashed_by_hash(
        &self,
        owner_account_id: &str,
        content_hash: &str,
    ) -> Result<Option<Book>> {
        Ok(sqlx::query_as::<_, Book>(
            "SELECT * FROM books
             WHERE owner_account_id = ? AND content_hash = ? AND deleted_at IS NOT NULL",
        )
        .bind(owner_account_id)
        .bind(content_hash)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Marca o desmarca un libro como eliminado sin tocar sus dependencias.
    pub async fn set_deleted(&self, id: &str, deleted: bool) -> Result<()> {
        let result = sqlx::query(
            "UPDATE books
             SET deleted_at = CASE WHEN ? THEN ? ELSE NULL END, updated_at = ?
             WHERE id = ?",
        )
        .bind(deleted)
        .bind(now())
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("libro {id}"))
    }

    pub async fn update(&self, book: &Book) -> Result<()> {
        let result = sqlx::query(
            "UPDATE books SET
                title = ?, author = ?, format = ?, content_hash = ?, content_ext = ?,
                size_bytes = ?, language = ?, publisher = ?, description = ?,
                origin = ?, cover_hash = ?, cover_ext = ?, audio_only = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&book.title)
        .bind(&book.author)
        .bind(book.format)
        .bind(&book.content_hash)
        .bind(&book.content_ext)
        .bind(book.size_bytes)
        .bind(&book.language)
        .bind(&book.publisher)
        .bind(&book.description)
        .bind(book.origin)
        .bind(&book.cover_hash)
        .bind(&book.cover_ext)
        .bind(book.audio_only)
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

    /// Limpia las descripciones que se importaron con HTML incrustado.
    ///
    /// Se ejecuta al abrir la base de datos para corregir bibliotecas creadas
    /// antes de que la inspección del EPUB saneara `dc:description`. Devuelve
    /// cuántos libros cambiaron.
    pub async fn repair_descriptions(&self) -> Result<usize> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, description FROM books
             WHERE description IS NOT NULL
               AND TRIM(description) <> ''
               AND (description LIKE '%<%' OR description LIKE '%&%')",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut repaired = 0;
        for (id, description) in rows {
            let cleaned = crate::html::clean_description(Some(description.clone()));
            if cleaned.as_deref() == Some(description.as_str()) {
                continue;
            }

            sqlx::query("UPDATE books SET description = ?, updated_at = ? WHERE id = ?")
                .bind(&cleaned)
                .bind(now())
                .bind(&id)
                .execute(&self.pool)
                .await?;
            repaired += 1;
        }

        Ok(repaired)
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
            (ResourceType::VoiceNote, "voice_notes"),
            (ResourceType::Reaction, "reactions"),
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

    pub async fn count_by_owner(&self, owner_account_id: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM books WHERE owner_account_id = ? AND deleted_at IS NULL",
        )
        .bind(owner_account_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
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

impl BookRepo {
    /// Crea un libro conservando el id remoto (sync).
    pub async fn create_with_id(&self, id: &str, new: NewBook) -> Result<Book> {
        let timestamp = now();
        sqlx::query(
            "INSERT INTO books
                (id, title, author, format, content_hash, content_ext, original_name,
                 cover_hash, cover_ext, size_bytes, language, publisher, description,
                 origin, audio_only, added_at, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
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
        .bind(new.origin)
        .bind(new.audio_only)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::Book,
            id,
            &new.owner_account_id,
            new.origin_share_id.as_deref(),
        )
        .await?;

        self.get(id)
            .await?
            .ok_or_else(|| KoraError::Internal("libro sincronizado no encontrado".into()))
    }

    pub async fn upsert_from_sync(&self, book: &Book, owner: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO books
                (id, title, author, format, content_hash, content_ext, original_name,
                 cover_hash, cover_ext, size_bytes, language, publisher, description,
                 origin, audio_only, added_at, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET
                title = excluded.title,
                author = excluded.author,
                format = excluded.format,
                content_hash = excluded.content_hash,
                content_ext = excluded.content_ext,
                original_name = excluded.original_name,
                cover_hash = excluded.cover_hash,
                cover_ext = excluded.cover_ext,
                size_bytes = excluded.size_bytes,
                language = excluded.language,
                publisher = excluded.publisher,
                description = excluded.description,
                origin = excluded.origin,
                audio_only = excluded.audio_only,
                updated_at = excluded.updated_at,
                owner_account_id = excluded.owner_account_id",
        )
        .bind(&book.id)
        .bind(&book.title)
        .bind(&book.author)
        .bind(book.format)
        .bind(&book.content_hash)
        .bind(&book.content_ext)
        .bind(&book.original_name)
        .bind(&book.cover_hash)
        .bind(&book.cover_ext)
        .bind(book.size_bytes)
        .bind(&book.language)
        .bind(&book.publisher)
        .bind(&book.description)
        .bind(book.origin)
        .bind(book.audio_only)
        .bind(book.added_at)
        .bind(book.updated_at)
        .bind(owner)
        .bind(&book.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::Book,
            &book.id,
            owner,
            book.origin_share_id.as_deref(),
        )
        .await?;
        Ok(())
    }
}

impl ReadingProgressRepo {
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_from_sync(
        &self,
        book_id: &str,
        locator: &str,
        percent: f64,
        updated_at: DateTime<Utc>,
        owner: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO reading_progress
                (book_id, locator, percent, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, NULL)
             ON CONFLICT (book_id) DO UPDATE SET
                locator = excluded.locator,
                percent = excluded.percent,
                updated_at = excluded.updated_at,
                owner_account_id = excluded.owner_account_id",
        )
        .bind(book_id)
        .bind(locator)
        .bind(percent)
        .bind(updated_at)
        .bind(owner)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::ReadingProgress,
            book_id,
            owner,
            None,
        )
        .await?;
        Ok(())
    }
}

impl BookRepo {
    pub async fn mark_read_only(&self, book_id: &str, share_id: &str) -> Result<()> {
        sqlx::query("UPDATE books SET read_only = 1, received_share_id = ? WHERE id = ?")
            .bind(share_id)
            .bind(book_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_read_only_by_share(&self, share_id: &str) -> Result<Vec<Book>> {
        Ok(sqlx::query_as::<_, Book>(
            "SELECT * FROM books WHERE received_share_id = ? AND read_only = 1",
        )
        .bind(share_id)
        .fetch_all(&self.pool)
        .await?)
    }
}
