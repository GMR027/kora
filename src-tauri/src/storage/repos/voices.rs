use sqlx::SqlitePool;

use super::{new_id, now, require_affected, track_owner, untrack_owner};
use crate::error::{KoraError, Result};
use crate::storage::model::{
    Audiobook, AudiobookProgress, AudiobookQueueItem, NewAudiobook, NewAudiobookQueueItem,
    NewVoice, ResourceType, Voice,
};

#[derive(Clone)]
pub struct VoiceRepo {
    pool: SqlitePool,
}

impl VoiceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewVoice) -> Result<Voice> {
        let id = new.id.clone().unwrap_or_else(new_id);
        let timestamp = now();

        sqlx::query(
            "INSERT INTO voices
                (id, name, engine, language, profile_hash, profile_ext, sample_hash, sample_ext,
                 kind, reference_duration_ms, created_at, updated_at, owner_account_id,
                 origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.name)
        .bind(&new.engine)
        .bind(&new.language)
        .bind(&new.profile_hash)
        .bind(&new.profile_ext)
        .bind(&new.sample_hash)
        .bind(&new.sample_ext)
        .bind(new.kind)
        .bind(new.reference_duration_ms)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::Voice,
            &id,
            &new.owner_account_id,
            new.origin_share_id.as_deref(),
        )
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("voz recién creada no encontrada".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Voice>> {
        Ok(
            sqlx::query_as::<_, Voice>("SELECT * FROM voices WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_owner(&self, owner_account_id: &str) -> Result<Vec<Voice>> {
        Ok(sqlx::query_as::<_, Voice>(
            "SELECT * FROM voices WHERE owner_account_id = ? ORDER BY created_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Voces clonadas del propietario actual, de la más reciente a la más
    /// antigua. Las voces de sistema viven en el catálogo del motor.
    pub async fn list_cloned_by_owner(&self, owner_account_id: &str) -> Result<Vec<Voice>> {
        Ok(sqlx::query_as::<_, Voice>(
            "SELECT * FROM voices
             WHERE owner_account_id = ? AND kind = 'cloned'
             ORDER BY created_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update(&self, voice: &Voice) -> Result<()> {
        let result = sqlx::query(
            "UPDATE voices SET
                name = ?, engine = ?, language = ?, profile_hash = ?, profile_ext = ?,
                sample_hash = ?, sample_ext = ?, kind = ?, reference_duration_ms = ?,
                updated_at = ?
             WHERE id = ?",
        )
        .bind(&voice.name)
        .bind(&voice.engine)
        .bind(&voice.language)
        .bind(&voice.profile_hash)
        .bind(&voice.profile_ext)
        .bind(&voice.sample_hash)
        .bind(&voice.sample_ext)
        .bind(voice.kind)
        .bind(voice.reference_duration_ms)
        .bind(now())
        .bind(&voice.id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("voz {}", voice.id))?;

        track_owner(
            &self.pool,
            ResourceType::Voice,
            &voice.id,
            &voice.owner_account_id,
            voice.origin_share_id.as_deref(),
        )
        .await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM voices WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("voz {id}"))?;
        untrack_owner(&self.pool, ResourceType::Voice, id).await
    }

    pub async fn count_by_asset_hash(&self, asset_hash: &str) -> Result<i64> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM voices WHERE profile_hash = ? OR sample_hash = ?")
                .bind(asset_hash)
                .bind(asset_hash)
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }
}

#[derive(Clone)]
pub struct AudiobookRepo {
    pool: SqlitePool,
}

impl AudiobookRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewAudiobook) -> Result<Audiobook> {
        let id = new_id();
        let timestamp = now();

        sqlx::query(
            "INSERT INTO audiobooks
                (id, book_id, voice_id, status, manifest_hash, manifest_ext,
                 audio_hash, audio_ext, scope, origin, duration_ms, narration, \"cast\",
                 created_at, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.book_id)
        .bind(&new.voice_id)
        .bind(new.status)
        .bind(&new.manifest_hash)
        .bind(&new.manifest_ext)
        .bind(&new.audio_hash)
        .bind(&new.audio_ext)
        .bind(&new.scope)
        .bind(new.origin)
        .bind(new.duration_ms)
        .bind(&new.narration)
        .bind(&new.cast)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::Audiobook,
            &id,
            &new.owner_account_id,
            new.origin_share_id.as_deref(),
        )
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("audiolibro recién creado no encontrado".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Audiobook>> {
        Ok(
            sqlx::query_as::<_, Audiobook>("SELECT * FROM audiobooks WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_book(&self, book_id: &str) -> Result<Vec<Audiobook>> {
        Ok(sqlx::query_as::<_, Audiobook>(
            "SELECT * FROM audiobooks WHERE book_id = ? ORDER BY created_at DESC",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Ids de libros que ya tienen al menos un audiolibro. Evita leer todos
    /// los manifiestos para pintar el filtro de la biblioteca.
    pub async fn list_book_ids_by_owner(&self, owner_account_id: &str) -> Result<Vec<String>> {
        Ok(sqlx::query_as::<_, (String,)>(
            "SELECT DISTINCT a.book_id FROM audiobooks a
             JOIN books b ON b.id = a.book_id
             WHERE b.owner_account_id = ? AND b.deleted_at IS NULL",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(book_id,)| book_id)
        .collect())
    }

    pub async fn list_by_owner(&self, owner_account_id: &str) -> Result<Vec<Audiobook>> {
        Ok(sqlx::query_as::<_, Audiobook>(
            "SELECT * FROM audiobooks WHERE owner_account_id = ? ORDER BY created_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update(&self, audiobook: &Audiobook) -> Result<()> {
        let result = sqlx::query(
            "UPDATE audiobooks SET
                voice_id = ?, status = ?, manifest_hash = ?, manifest_ext = ?,
                audio_hash = ?, audio_ext = ?, scope = ?, origin = ?,
                duration_ms = ?, narration = ?, \"cast\" = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&audiobook.voice_id)
        .bind(audiobook.status)
        .bind(&audiobook.manifest_hash)
        .bind(&audiobook.manifest_ext)
        .bind(&audiobook.audio_hash)
        .bind(&audiobook.audio_ext)
        .bind(&audiobook.scope)
        .bind(audiobook.origin)
        .bind(audiobook.duration_ms)
        .bind(&audiobook.narration)
        .bind(&audiobook.cast)
        .bind(now())
        .bind(&audiobook.id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("audiolibro {}", audiobook.id))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM audiobooks WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("audiolibro {id}"))?;
        untrack_owner(&self.pool, ResourceType::Audiobook, id).await
    }

    pub async fn count_by_manifest_hash(&self, manifest_hash: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audiobooks WHERE manifest_hash = ?")
            .bind(manifest_hash)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn count_by_audio_hash(&self, audio_hash: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audiobooks WHERE audio_hash = ?")
            .bind(audio_hash)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn set_favorite(
        &self,
        owner_account_id: &str,
        audiobook_id: &str,
        favorite: bool,
    ) -> Result<()> {
        if favorite {
            sqlx::query(
                "INSERT INTO audiobook_favorites (audiobook_id, owner_account_id, created_at)
                 VALUES (?, ?, ?)
                 ON CONFLICT (audiobook_id) DO UPDATE SET
                    owner_account_id = excluded.owner_account_id",
            )
            .bind(audiobook_id)
            .bind(owner_account_id)
            .bind(now())
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                "DELETE FROM audiobook_favorites
                 WHERE audiobook_id = ? AND owner_account_id = ?",
            )
            .bind(audiobook_id)
            .bind(owner_account_id)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn is_favorite(&self, audiobook_id: &str) -> Result<bool> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM audiobook_favorites WHERE audiobook_id = ?")
                .bind(audiobook_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
    }

    /// Inserta o actualiza una fila recibida por sync o por un share. El audio
    /// y el manifiesto se guardan como assets antes de llamar a este método.
    pub async fn upsert_from_sync(&self, audiobook: &Audiobook, owner: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO audiobooks
                (id, book_id, voice_id, status, manifest_hash, manifest_ext,
                 audio_hash, audio_ext, scope, origin, duration_ms, narration, \"cast\",
                 created_at, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET
                book_id = excluded.book_id,
                voice_id = excluded.voice_id,
                status = excluded.status,
                manifest_hash = excluded.manifest_hash,
                manifest_ext = excluded.manifest_ext,
                audio_hash = excluded.audio_hash,
                audio_ext = excluded.audio_ext,
                scope = excluded.scope,
                origin = excluded.origin,
                duration_ms = excluded.duration_ms,
                narration = excluded.narration,
                \"cast\" = excluded.\"cast\",
                updated_at = excluded.updated_at,
                owner_account_id = excluded.owner_account_id",
        )
        .bind(&audiobook.id)
        .bind(&audiobook.book_id)
        .bind(&audiobook.voice_id)
        .bind(audiobook.status)
        .bind(&audiobook.manifest_hash)
        .bind(&audiobook.manifest_ext)
        .bind(&audiobook.audio_hash)
        .bind(&audiobook.audio_ext)
        .bind(&audiobook.scope)
        .bind(audiobook.origin)
        .bind(audiobook.duration_ms)
        .bind(&audiobook.narration)
        .bind(&audiobook.cast)
        .bind(audiobook.created_at)
        .bind(audiobook.updated_at)
        .bind(owner)
        .bind(&audiobook.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::Audiobook,
            &audiobook.id,
            owner,
            audiobook.origin_share_id.as_deref(),
        )
        .await
    }

    pub async fn progress(&self, audiobook_id: &str) -> Result<Option<AudiobookProgress>> {
        Ok(sqlx::query_as::<_, AudiobookProgress>(
            "SELECT * FROM audiobook_progress WHERE audiobook_id = ?",
        )
        .bind(audiobook_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn latest_progress_by_owner(
        &self,
        owner_account_id: &str,
    ) -> Result<Option<AudiobookProgress>> {
        Ok(sqlx::query_as::<_, AudiobookProgress>(
            "SELECT * FROM audiobook_progress
             WHERE owner_account_id = ?
             ORDER BY updated_at DESC
             LIMIT 1",
        )
        .bind(owner_account_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_progress_by_owner(
        &self,
        owner_account_id: &str,
    ) -> Result<Vec<AudiobookProgress>> {
        Ok(sqlx::query_as::<_, AudiobookProgress>(
            "SELECT * FROM audiobook_progress
             WHERE owner_account_id = ?
             ORDER BY updated_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn save_progress(
        &self,
        audiobook_id: &str,
        owner_account_id: &str,
        position_ms: i64,
        chapter_index: i64,
    ) -> Result<AudiobookProgress> {
        let timestamp = now();
        sqlx::query(
            "INSERT INTO audiobook_progress
                (audiobook_id, position_ms, chapter_index, updated_at, owner_account_id)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (audiobook_id) DO UPDATE SET
                position_ms = excluded.position_ms,
                chapter_index = excluded.chapter_index,
                updated_at = excluded.updated_at,
                owner_account_id = excluded.owner_account_id",
        )
        .bind(audiobook_id)
        .bind(position_ms.max(0))
        .bind(chapter_index.max(0))
        .bind(timestamp)
        .bind(owner_account_id)
        .execute(&self.pool)
        .await?;

        self.progress(audiobook_id)
            .await?
            .ok_or_else(|| KoraError::Internal("progreso de audiolibro no encontrado".into()))
    }

    /// Marca como canceladas las generaciones que quedaron a medias por un
    /// cierre inesperado. La cola persistente no se toca.
    pub async fn reset_interrupted(&self, owner_account_id: &str) -> Result<u64> {
        sqlx::query(
            "UPDATE audiobook_queue
             SET status = 'pending'
             WHERE owner_account_id = ? AND status = 'running'",
        )
        .bind(owner_account_id)
        .execute(&self.pool)
        .await?;

        let result = sqlx::query(
            "UPDATE audiobooks
             SET status = 'cancelled', updated_at = ?
             WHERE owner_account_id = ? AND status IN ('pending', 'running')",
        )
        .bind(now())
        .bind(owner_account_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn enqueue(&self, new: NewAudiobookQueueItem) -> Result<AudiobookQueueItem> {
        let id = new_id();
        let row: (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(position), -1) + 1
             FROM audiobook_queue WHERE owner_account_id = ?",
        )
        .bind(&new.owner_account_id)
        .fetch_one(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO audiobook_queue
                (id, book_id, scope, voice_id, narration, \"cast\", origin, status, position,
                 created_at, owner_account_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.book_id)
        .bind(&new.scope)
        .bind(&new.voice_id)
        .bind(&new.narration)
        .bind(&new.cast)
        .bind(new.origin)
        .bind(row.0)
        .bind(now())
        .bind(&new.owner_account_id)
        .execute(&self.pool)
        .await?;

        self.queue_item(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("elemento de cola no encontrado".into()))
    }

    pub async fn queue_item(&self, id: &str) -> Result<Option<AudiobookQueueItem>> {
        Ok(
            sqlx::query_as::<_, AudiobookQueueItem>("SELECT * FROM audiobook_queue WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_queue(&self, owner_account_id: &str) -> Result<Vec<AudiobookQueueItem>> {
        Ok(sqlx::query_as::<_, AudiobookQueueItem>(
            "SELECT * FROM audiobook_queue
             WHERE owner_account_id = ? AND status = 'pending'
             ORDER BY position ASC, created_at ASC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn next_queue_item(
        &self,
        owner_account_id: &str,
    ) -> Result<Option<AudiobookQueueItem>> {
        Ok(sqlx::query_as::<_, AudiobookQueueItem>(
            "SELECT * FROM audiobook_queue
             WHERE owner_account_id = ? AND status = 'pending'
             ORDER BY position ASC, created_at ASC
             LIMIT 1",
        )
        .bind(owner_account_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Marca el trabajo como activo antes de sintetizar. Si dos workers lo
    /// intentan, solo uno obtiene la fila.
    pub async fn claim_queue_item(&self, id: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE audiobook_queue SET status = 'running'
             WHERE id = ? AND status = 'pending'",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        require_affected(result, &format!("elemento de cola {id}"))
    }

    /// Reordena los elementos pendientes según la lista de ids recibida.
    pub async fn reorder_queue(&self, owner_account_id: &str, ids: &[String]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for (index, id) in ids.iter().enumerate() {
            sqlx::query(
                "UPDATE audiobook_queue SET position = ?
                 WHERE id = ? AND owner_account_id = ? AND status = 'pending'",
            )
            .bind(index as i64)
            .bind(id)
            .bind(owner_account_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn remove_queue_item(&self, id: &str, owner_account_id: &str) -> Result<()> {
        let result =
            sqlx::query("DELETE FROM audiobook_queue WHERE id = ? AND owner_account_id = ?")
                .bind(id)
                .bind(owner_account_id)
                .execute(&self.pool)
                .await?;
        require_affected(result, &format!("elemento de cola {id}"))
    }

    pub async fn clear_queue(&self, owner_account_id: &str) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM audiobook_queue WHERE owner_account_id = ? AND status = 'pending'",
        )
        .bind(owner_account_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
