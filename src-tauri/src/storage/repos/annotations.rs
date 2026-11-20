use sqlx::SqlitePool;

use super::{new_id, now, require_affected, track_owner, untrack_owner};
use crate::error::{KoraError, Result};
use crate::storage::model::{
    Annotation, Extract, NewAnnotation, NewExtract, NewNote, Note, ResourceType,
};

#[derive(Clone)]
pub struct AnnotationRepo {
    pool: SqlitePool,
}

impl AnnotationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewAnnotation) -> Result<Annotation> {
        let id = new_id();
        let timestamp = now();

        sqlx::query(
            "INSERT INTO annotations
                (id, book_id, kind, locator, chapter_index, offset_start, offset_end,
                 text, note, color, created_at, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.book_id)
        .bind(new.kind)
        .bind(&new.locator)
        .bind(new.chapter_index)
        .bind(new.offset_start)
        .bind(new.offset_end)
        .bind(&new.text)
        .bind(&new.note)
        .bind(&new.color)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::Annotation,
            &id,
            &new.owner_account_id,
            new.origin_share_id.as_deref(),
        )
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("anotación recién creada no encontrada".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Annotation>> {
        Ok(
            sqlx::query_as::<_, Annotation>("SELECT * FROM annotations WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_book(&self, book_id: &str) -> Result<Vec<Annotation>> {
        Ok(sqlx::query_as::<_, Annotation>(
            "SELECT * FROM annotations WHERE book_id = ?
             ORDER BY chapter_index, offset_start, created_at",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update(&self, annotation: &Annotation) -> Result<()> {
        let result = sqlx::query(
            "UPDATE annotations SET
                kind = ?, locator = ?, chapter_index = ?, offset_start = ?, offset_end = ?,
                text = ?, note = ?, color = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(annotation.kind)
        .bind(&annotation.locator)
        .bind(annotation.chapter_index)
        .bind(annotation.offset_start)
        .bind(annotation.offset_end)
        .bind(&annotation.text)
        .bind(&annotation.note)
        .bind(&annotation.color)
        .bind(now())
        .bind(&annotation.id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("anotación {}", annotation.id))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM annotations WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("anotación {id}"))?;
        untrack_owner(&self.pool, ResourceType::Annotation, id).await
    }
}

#[derive(Clone)]
pub struct ExtractRepo {
    pool: SqlitePool,
}

impl ExtractRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewExtract) -> Result<Extract> {
        let id = new_id();
        let timestamp = now();

        sqlx::query(
            "INSERT INTO extracts
                (id, book_id, locator, text, note, created_at, updated_at,
                 owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.book_id)
        .bind(&new.locator)
        .bind(&new.text)
        .bind(&new.note)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::Extract,
            &id,
            &new.owner_account_id,
            new.origin_share_id.as_deref(),
        )
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("extracto recién creado no encontrado".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Extract>> {
        Ok(
            sqlx::query_as::<_, Extract>("SELECT * FROM extracts WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_book(&self, book_id: &str) -> Result<Vec<Extract>> {
        Ok(sqlx::query_as::<_, Extract>(
            "SELECT * FROM extracts WHERE book_id = ? ORDER BY created_at",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update(&self, extract: &Extract) -> Result<()> {
        let result = sqlx::query(
            "UPDATE extracts SET locator = ?, text = ?, note = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&extract.locator)
        .bind(&extract.text)
        .bind(&extract.note)
        .bind(now())
        .bind(&extract.id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("extracto {}", extract.id))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM extracts WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("extracto {id}"))?;
        untrack_owner(&self.pool, ResourceType::Extract, id).await
    }
}

#[derive(Clone)]
pub struct NoteRepo {
    pool: SqlitePool,
}

impl NoteRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewNote) -> Result<Note> {
        let id = new_id();
        let timestamp = now();

        sqlx::query(
            "INSERT INTO notes
                (id, book_id, title, body, audio_hash, audio_ext,
                 created_at, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.book_id)
        .bind(&new.title)
        .bind(&new.body)
        .bind(&new.audio_hash)
        .bind(&new.audio_ext)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::Note,
            &id,
            &new.owner_account_id,
            new.origin_share_id.as_deref(),
        )
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("nota recién creada no encontrada".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Note>> {
        Ok(
            sqlx::query_as::<_, Note>("SELECT * FROM notes WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_owner(&self, owner_account_id: &str) -> Result<Vec<Note>> {
        Ok(sqlx::query_as::<_, Note>(
            "SELECT * FROM notes WHERE owner_account_id = ? ORDER BY created_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_by_book(&self, book_id: &str) -> Result<Vec<Note>> {
        Ok(sqlx::query_as::<_, Note>(
            "SELECT * FROM notes WHERE book_id = ? ORDER BY created_at DESC",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update(&self, note: &Note) -> Result<()> {
        let result = sqlx::query(
            "UPDATE notes SET
                book_id = ?, title = ?, body = ?, audio_hash = ?, audio_ext = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&note.book_id)
        .bind(&note.title)
        .bind(&note.body)
        .bind(&note.audio_hash)
        .bind(&note.audio_ext)
        .bind(now())
        .bind(&note.id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("nota {}", note.id))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM notes WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("nota {id}"))?;
        untrack_owner(&self.pool, ResourceType::Note, id).await
    }

    pub async fn count_by_audio_hash(&self, audio_hash: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notes WHERE audio_hash = ?")
            .bind(audio_hash)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }
}
