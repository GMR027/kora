use sqlx::SqlitePool;

use super::{new_id, now, require_affected, track_owner};
use crate::error::{KoraError, Result};
use crate::storage::model::{
    Annotation, Extract, NewAnnotation, NewExtract, NewNote, NewReaction, NewVoiceNote, Note,
    Reaction, ReactionKind, ResourceType, VoiceNote,
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

        self.get_any(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("anotación recién creada no encontrada".into()))
    }

    /// Devuelve solo anotaciones activas.
    pub async fn get(&self, id: &str) -> Result<Option<Annotation>> {
        Ok(sqlx::query_as::<_, Annotation>(
            "SELECT * FROM annotations WHERE id = ? AND deleted = 0",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Incluye tumbas; lo usará la sincronización de la fase 9.
    pub async fn get_any(&self, id: &str) -> Result<Option<Annotation>> {
        Ok(
            sqlx::query_as::<_, Annotation>("SELECT * FROM annotations WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_book(&self, book_id: &str) -> Result<Vec<Annotation>> {
        Ok(sqlx::query_as::<_, Annotation>(
            "SELECT * FROM annotations WHERE book_id = ? AND deleted = 0
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
             WHERE id = ? AND deleted = 0",
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

    /// Borrado suave: conserva la fila como tumba para CRDT/sync.
    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE annotations SET deleted = 1, updated_at = ? WHERE id = ? AND deleted = 0",
        )
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("anotación {id}"))
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
                (id, book_id, locator, text, note, chapter_index, offset_start, offset_end,
                 citation, created_at, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.book_id)
        .bind(&new.locator)
        .bind(&new.text)
        .bind(&new.note)
        .bind(new.chapter_index)
        .bind(new.offset_start)
        .bind(new.offset_end)
        .bind(&new.citation)
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

        self.get_any(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("extracto recién creado no encontrado".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Extract>> {
        Ok(
            sqlx::query_as::<_, Extract>("SELECT * FROM extracts WHERE id = ? AND deleted = 0")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn get_any(&self, id: &str) -> Result<Option<Extract>> {
        Ok(
            sqlx::query_as::<_, Extract>("SELECT * FROM extracts WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_book(&self, book_id: &str) -> Result<Vec<Extract>> {
        Ok(sqlx::query_as::<_, Extract>(
            "SELECT * FROM extracts WHERE book_id = ? AND deleted = 0 ORDER BY created_at",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_by_owner(&self, owner_account_id: &str) -> Result<Vec<Extract>> {
        Ok(sqlx::query_as::<_, Extract>(
            "SELECT * FROM extracts WHERE owner_account_id = ? AND deleted = 0
             ORDER BY created_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Búsqueda simple y segura sobre texto, nota y cita. `instr` evita
    /// interpretar `%` o `_` del usuario como comodines.
    pub async fn search(
        &self,
        owner_account_id: &str,
        book_id: Option<&str>,
        query: &str,
        limit: i64,
    ) -> Result<Vec<Extract>> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return match book_id {
                Some(book_id) => self.list_by_book(book_id).await,
                None => self.list_by_owner(owner_account_id).await,
            };
        }

        let limit = limit.clamp(1, 500);
        Ok(match book_id {
            Some(book_id) => {
                sqlx::query_as::<_, Extract>(
                    "SELECT * FROM extracts
                     WHERE book_id = ? AND owner_account_id = ? AND deleted = 0
                       AND (instr(lower(text), ?) > 0
                            OR instr(lower(coalesce(note, '')), ?) > 0
                            OR instr(lower(coalesce(citation, '')), ?) > 0)
                     ORDER BY created_at DESC LIMIT ?",
                )
                .bind(book_id)
                .bind(owner_account_id)
                .bind(&query)
                .bind(&query)
                .bind(&query)
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, Extract>(
                    "SELECT * FROM extracts
                     WHERE owner_account_id = ? AND deleted = 0
                       AND (instr(lower(text), ?) > 0
                            OR instr(lower(coalesce(note, '')), ?) > 0
                            OR instr(lower(coalesce(citation, '')), ?) > 0)
                     ORDER BY created_at DESC LIMIT ?",
                )
                .bind(owner_account_id)
                .bind(&query)
                .bind(&query)
                .bind(&query)
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
            }
        })
    }

    pub async fn update(&self, extract: &Extract) -> Result<()> {
        let result = sqlx::query(
            "UPDATE extracts SET
                locator = ?, text = ?, note = ?, chapter_index = ?, offset_start = ?,
                offset_end = ?, citation = ?, updated_at = ?
             WHERE id = ? AND deleted = 0",
        )
        .bind(&extract.locator)
        .bind(&extract.text)
        .bind(&extract.note)
        .bind(extract.chapter_index)
        .bind(extract.offset_start)
        .bind(extract.offset_end)
        .bind(&extract.citation)
        .bind(now())
        .bind(&extract.id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("extracto {}", extract.id))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE extracts SET deleted = 1, updated_at = ? WHERE id = ? AND deleted = 0",
        )
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("extracto {id}"))
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
                (id, book_id, title, body, content_json, chapter_index, offset_start, offset_end,
                 text, audio_hash, audio_ext, created_at, updated_at, owner_account_id,
                 origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.book_id)
        .bind(&new.title)
        .bind(&new.body)
        .bind(&new.content_json)
        .bind(new.chapter_index)
        .bind(new.offset_start)
        .bind(new.offset_end)
        .bind(&new.text)
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

        self.get_any(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("nota recién creada no encontrada".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Note>> {
        Ok(
            sqlx::query_as::<_, Note>("SELECT * FROM notes WHERE id = ? AND deleted = 0")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn get_any(&self, id: &str) -> Result<Option<Note>> {
        Ok(
            sqlx::query_as::<_, Note>("SELECT * FROM notes WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_owner(&self, owner_account_id: &str) -> Result<Vec<Note>> {
        Ok(sqlx::query_as::<_, Note>(
            "SELECT * FROM notes WHERE owner_account_id = ? AND deleted = 0
             ORDER BY created_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_by_book(&self, book_id: &str) -> Result<Vec<Note>> {
        Ok(sqlx::query_as::<_, Note>(
            "SELECT * FROM notes WHERE book_id = ? AND deleted = 0 ORDER BY created_at DESC",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update(&self, note: &Note) -> Result<()> {
        let result = sqlx::query(
            "UPDATE notes SET
                book_id = ?, title = ?, body = ?, content_json = ?, chapter_index = ?,
                offset_start = ?, offset_end = ?, text = ?, audio_hash = ?, audio_ext = ?,
                updated_at = ?
             WHERE id = ? AND deleted = 0",
        )
        .bind(&note.book_id)
        .bind(&note.title)
        .bind(&note.body)
        .bind(&note.content_json)
        .bind(note.chapter_index)
        .bind(note.offset_start)
        .bind(note.offset_end)
        .bind(&note.text)
        .bind(&note.audio_hash)
        .bind(&note.audio_ext)
        .bind(now())
        .bind(&note.id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("nota {}", note.id))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE notes SET deleted = 1, updated_at = ? WHERE id = ? AND deleted = 0",
        )
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("nota {id}"))
    }

    pub async fn count_by_audio_hash(&self, audio_hash: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notes WHERE audio_hash = ?")
            .bind(audio_hash)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }
}

#[derive(Clone)]
pub struct VoiceNoteRepo {
    pool: SqlitePool,
}

impl VoiceNoteRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewVoiceNote) -> Result<VoiceNote> {
        let id = new.id.clone().unwrap_or_else(new_id);
        let timestamp = now();

        sqlx::query(
            "INSERT INTO voice_notes
                (id, book_id, chapter_index, offset_start, offset_end, text, duration_ms,
                 audio_hash, audiobook_id, position_ms, quote, transcript, source, created_at,
                 updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.book_id)
        .bind(new.chapter_index)
        .bind(new.offset_start)
        .bind(new.offset_end)
        .bind(&new.text)
        .bind(new.duration_ms)
        .bind(&new.audio_hash)
        .bind(&new.audiobook_id)
        .bind(new.position_ms.max(0))
        .bind(&new.quote)
        .bind(&new.transcript)
        .bind(&new.source)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::VoiceNote,
            &id,
            &new.owner_account_id,
            new.origin_share_id.as_deref(),
        )
        .await?;

        self.get_any(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("nota de voz recién creada no encontrada".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<VoiceNote>> {
        Ok(
            sqlx::query_as::<_, VoiceNote>(
                "SELECT * FROM voice_notes WHERE id = ? AND deleted = 0",
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?,
        )
    }

    pub async fn get_any(&self, id: &str) -> Result<Option<VoiceNote>> {
        Ok(
            sqlx::query_as::<_, VoiceNote>("SELECT * FROM voice_notes WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_book(&self, book_id: &str) -> Result<Vec<VoiceNote>> {
        Ok(sqlx::query_as::<_, VoiceNote>(
            "SELECT * FROM voice_notes WHERE book_id = ? AND deleted = 0
             ORDER BY created_at DESC",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Marcadores de voz de un audiolibro, ordenados por posición de escucha.
    pub async fn list_by_audiobook(&self, audiobook_id: &str) -> Result<Vec<VoiceNote>> {
        Ok(sqlx::query_as::<_, VoiceNote>(
            "SELECT * FROM voice_notes
             WHERE audiobook_id = ? AND deleted = 0
             ORDER BY position_ms, created_at",
        )
        .bind(audiobook_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Incluye tumbas para poder borrar los archivos al eliminar el
    /// audiolibro, igual que hace la eliminación de un libro completo.
    pub async fn list_by_audiobook_including_deleted(
        &self,
        audiobook_id: &str,
    ) -> Result<Vec<VoiceNote>> {
        Ok(sqlx::query_as::<_, VoiceNote>(
            "SELECT * FROM voice_notes WHERE audiobook_id = ? ORDER BY position_ms, created_at",
        )
        .bind(audiobook_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Incluye tumbas para poder borrar los archivos al eliminar el libro.
    pub async fn list_by_book_including_deleted(&self, book_id: &str) -> Result<Vec<VoiceNote>> {
        Ok(sqlx::query_as::<_, VoiceNote>(
            "SELECT * FROM voice_notes WHERE book_id = ? ORDER BY created_at DESC",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Actualiza el texto, la transcripción y la cita de una nota de voz.
    /// El audio original no cambia.
    pub async fn update(&self, note: &VoiceNote) -> Result<()> {
        let result = sqlx::query(
            "UPDATE voice_notes
             SET text = ?, transcript = ?, quote = ?, updated_at = ?
             WHERE id = ? AND deleted = 0",
        )
        .bind(&note.text)
        .bind(&note.transcript)
        .bind(&note.quote)
        .bind(now())
        .bind(&note.id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("nota de voz {}", note.id))?;
        track_owner(
            &self.pool,
            ResourceType::VoiceNote,
            &note.id,
            &note.owner_account_id,
            note.origin_share_id.as_deref(),
        )
        .await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE voice_notes SET deleted = 1, updated_at = ? WHERE id = ? AND deleted = 0",
        )
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("nota de voz {id}"))
    }
}

#[derive(Clone)]
pub struct ReactionRepo {
    pool: SqlitePool,
}

impl ReactionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewReaction) -> Result<Reaction> {
        let id = new_id();
        let timestamp = now();

        sqlx::query(
            "INSERT INTO reactions
                (id, book_id, chapter_index, offset_start, offset_end, kind, text,
                 created_at, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.book_id)
        .bind(new.chapter_index)
        .bind(new.offset_start)
        .bind(new.offset_end)
        .bind(new.kind)
        .bind(&new.text)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::Reaction,
            &id,
            &new.owner_account_id,
            new.origin_share_id.as_deref(),
        )
        .await?;

        self.get_any(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("reacción recién creada no encontrada".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Reaction>> {
        Ok(
            sqlx::query_as::<_, Reaction>("SELECT * FROM reactions WHERE id = ? AND deleted = 0")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn get_any(&self, id: &str) -> Result<Option<Reaction>> {
        Ok(
            sqlx::query_as::<_, Reaction>("SELECT * FROM reactions WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_book(&self, book_id: &str) -> Result<Vec<Reaction>> {
        Ok(sqlx::query_as::<_, Reaction>(
            "SELECT * FROM reactions WHERE book_id = ? AND deleted = 0
             ORDER BY chapter_index, offset_start, created_at",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn counts_by_book(&self, book_id: &str) -> Result<Vec<(i64, ReactionKind, i64)>> {
        Ok(sqlx::query_as::<_, (i64, ReactionKind, i64)>(
            "SELECT chapter_index, kind, COUNT(*) FROM reactions
             WHERE book_id = ? AND deleted = 0
             GROUP BY chapter_index, kind
             ORDER BY chapter_index",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update(&self, reaction: &Reaction) -> Result<()> {
        let result = sqlx::query(
            "UPDATE reactions SET kind = ?, text = ?, updated_at = ?
             WHERE id = ? AND deleted = 0",
        )
        .bind(reaction.kind)
        .bind(&reaction.text)
        .bind(now())
        .bind(&reaction.id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("reacción {}", reaction.id))?;
        track_owner(
            &self.pool,
            ResourceType::Reaction,
            &reaction.id,
            &reaction.owner_account_id,
            reaction.origin_share_id.as_deref(),
        )
        .await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE reactions SET deleted = 1, updated_at = ? WHERE id = ? AND deleted = 0",
        )
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("reacción {id}"))
    }
}

impl AnnotationRepo {
    /// Aplica una anotación llegada por sync conservando su id y timestamps.
    pub async fn upsert_from_sync(&self, annotation: &Annotation, owner: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO annotations
                (id, book_id, kind, locator, chapter_index, offset_start, offset_end,
                 text, note, color, created_at, updated_at, deleted, owner_account_id,
                 origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET
                book_id = excluded.book_id,
                kind = excluded.kind,
                locator = excluded.locator,
                chapter_index = excluded.chapter_index,
                offset_start = excluded.offset_start,
                offset_end = excluded.offset_end,
                text = excluded.text,
                note = excluded.note,
                color = excluded.color,
                updated_at = excluded.updated_at,
                deleted = excluded.deleted,
                owner_account_id = excluded.owner_account_id",
        )
        .bind(&annotation.id)
        .bind(&annotation.book_id)
        .bind(annotation.kind)
        .bind(&annotation.locator)
        .bind(annotation.chapter_index)
        .bind(annotation.offset_start)
        .bind(annotation.offset_end)
        .bind(&annotation.text)
        .bind(&annotation.note)
        .bind(&annotation.color)
        .bind(annotation.created_at)
        .bind(annotation.updated_at)
        .bind(annotation.deleted)
        .bind(owner)
        .bind(&annotation.origin_share_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

impl ExtractRepo {
    pub async fn upsert_from_sync(&self, extract: &Extract, owner: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO extracts
                (id, book_id, locator, text, note, chapter_index, offset_start, offset_end,
                 citation, created_at, updated_at, deleted, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET
                book_id = excluded.book_id,
                locator = excluded.locator,
                text = excluded.text,
                note = excluded.note,
                chapter_index = excluded.chapter_index,
                offset_start = excluded.offset_start,
                offset_end = excluded.offset_end,
                citation = excluded.citation,
                updated_at = excluded.updated_at,
                deleted = excluded.deleted,
                owner_account_id = excluded.owner_account_id",
        )
        .bind(&extract.id)
        .bind(&extract.book_id)
        .bind(&extract.locator)
        .bind(&extract.text)
        .bind(&extract.note)
        .bind(extract.chapter_index)
        .bind(extract.offset_start)
        .bind(extract.offset_end)
        .bind(&extract.citation)
        .bind(extract.created_at)
        .bind(extract.updated_at)
        .bind(extract.deleted)
        .bind(owner)
        .bind(&extract.origin_share_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

impl NoteRepo {
    pub async fn upsert_from_sync(&self, note: &Note, owner: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO notes
                (id, book_id, title, body, content_json, chapter_index, offset_start,
                 offset_end, text, audio_hash, audio_ext, created_at, updated_at, deleted,
                 owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET
                book_id = excluded.book_id,
                title = excluded.title,
                body = excluded.body,
                content_json = excluded.content_json,
                chapter_index = excluded.chapter_index,
                offset_start = excluded.offset_start,
                offset_end = excluded.offset_end,
                text = excluded.text,
                audio_hash = excluded.audio_hash,
                audio_ext = excluded.audio_ext,
                updated_at = excluded.updated_at,
                deleted = excluded.deleted,
                owner_account_id = excluded.owner_account_id",
        )
        .bind(&note.id)
        .bind(&note.book_id)
        .bind(&note.title)
        .bind(&note.body)
        .bind(&note.content_json)
        .bind(note.chapter_index)
        .bind(note.offset_start)
        .bind(note.offset_end)
        .bind(&note.text)
        .bind(&note.audio_hash)
        .bind(&note.audio_ext)
        .bind(note.created_at)
        .bind(note.updated_at)
        .bind(note.deleted)
        .bind(owner)
        .bind(&note.origin_share_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

impl VoiceNoteRepo {
    pub async fn upsert_from_sync(&self, note: &VoiceNote, owner: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO voice_notes
                (id, book_id, chapter_index, offset_start, offset_end, text, duration_ms,
                 audio_hash, audiobook_id, position_ms, quote, transcript, source, created_at,
                 updated_at, deleted, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET
                book_id = excluded.book_id,
                chapter_index = excluded.chapter_index,
                offset_start = excluded.offset_start,
                offset_end = excluded.offset_end,
                text = excluded.text,
                duration_ms = excluded.duration_ms,
                audio_hash = excluded.audio_hash,
                audiobook_id = excluded.audiobook_id,
                position_ms = excluded.position_ms,
                quote = excluded.quote,
                transcript = excluded.transcript,
                source = excluded.source,
                updated_at = excluded.updated_at,
                deleted = excluded.deleted,
                owner_account_id = excluded.owner_account_id",
        )
        .bind(&note.id)
        .bind(&note.book_id)
        .bind(note.chapter_index)
        .bind(note.offset_start)
        .bind(note.offset_end)
        .bind(&note.text)
        .bind(note.duration_ms)
        .bind(&note.audio_hash)
        .bind(&note.audiobook_id)
        .bind(note.position_ms.max(0))
        .bind(&note.quote)
        .bind(&note.transcript)
        .bind(&note.source)
        .bind(note.created_at)
        .bind(note.updated_at)
        .bind(note.deleted)
        .bind(owner)
        .bind(&note.origin_share_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

impl ReactionRepo {
    pub async fn upsert_from_sync(&self, reaction: &Reaction, owner: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO reactions
                (id, book_id, chapter_index, offset_start, offset_end, kind, text,
                 created_at, updated_at, deleted, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET
                book_id = excluded.book_id,
                chapter_index = excluded.chapter_index,
                offset_start = excluded.offset_start,
                offset_end = excluded.offset_end,
                kind = excluded.kind,
                text = excluded.text,
                updated_at = excluded.updated_at,
                deleted = excluded.deleted,
                owner_account_id = excluded.owner_account_id",
        )
        .bind(&reaction.id)
        .bind(&reaction.book_id)
        .bind(reaction.chapter_index)
        .bind(reaction.offset_start)
        .bind(reaction.offset_end)
        .bind(reaction.kind)
        .bind(&reaction.text)
        .bind(reaction.created_at)
        .bind(reaction.updated_at)
        .bind(reaction.deleted)
        .bind(owner)
        .bind(&reaction.origin_share_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

impl AnnotationRepo {
    pub async fn list_by_book_including_deleted(&self, book_id: &str) -> Result<Vec<Annotation>> {
        Ok(sqlx::query_as::<_, Annotation>(
            "SELECT * FROM annotations WHERE book_id = ? ORDER BY chapter_index, offset_start",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }
}

impl ExtractRepo {
    pub async fn list_by_book_including_deleted(&self, book_id: &str) -> Result<Vec<Extract>> {
        Ok(sqlx::query_as::<_, Extract>(
            "SELECT * FROM extracts WHERE book_id = ? ORDER BY created_at",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }
}

impl NoteRepo {
    pub async fn list_by_owner_including_deleted(&self, owner: &str) -> Result<Vec<Note>> {
        Ok(sqlx::query_as::<_, Note>(
            "SELECT * FROM notes WHERE owner_account_id = ? ORDER BY created_at",
        )
        .bind(owner)
        .fetch_all(&self.pool)
        .await?)
    }
}

impl ReactionRepo {
    pub async fn list_by_book_including_deleted(&self, book_id: &str) -> Result<Vec<Reaction>> {
        Ok(sqlx::query_as::<_, Reaction>(
            "SELECT * FROM reactions WHERE book_id = ? ORDER BY chapter_index, offset_start",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }
}

impl VoiceNoteRepo {
    pub async fn set_audio_hash(&self, id: &str, hash: &str) -> Result<()> {
        sqlx::query("UPDATE voice_notes SET audio_hash = ? WHERE id = ?")
            .bind(hash)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
