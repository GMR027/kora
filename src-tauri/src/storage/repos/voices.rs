use sqlx::SqlitePool;

use super::{new_id, now, require_affected, track_owner, untrack_owner};
use crate::error::{KoraError, Result};
use crate::storage::model::{Audiobook, NewAudiobook, NewVoice, ResourceType, Voice};

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
        let id = new_id();
        let timestamp = now();

        sqlx::query(
            "INSERT INTO voices
                (id, name, engine, language, profile_hash, profile_ext, sample_hash, sample_ext,
                 created_at, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.name)
        .bind(&new.engine)
        .bind(&new.language)
        .bind(&new.profile_hash)
        .bind(&new.profile_ext)
        .bind(&new.sample_hash)
        .bind(&new.sample_ext)
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

    pub async fn update(&self, voice: &Voice) -> Result<()> {
        let result = sqlx::query(
            "UPDATE voices SET
                name = ?, engine = ?, language = ?, profile_hash = ?, profile_ext = ?,
                sample_hash = ?, sample_ext = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&voice.name)
        .bind(&voice.engine)
        .bind(&voice.language)
        .bind(&voice.profile_hash)
        .bind(&voice.profile_ext)
        .bind(&voice.sample_hash)
        .bind(&voice.sample_ext)
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
                (id, book_id, voice_id, status, manifest_hash, manifest_ext, duration_ms,
                 created_at, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.book_id)
        .bind(&new.voice_id)
        .bind(new.status)
        .bind(&new.manifest_hash)
        .bind(&new.manifest_ext)
        .bind(new.duration_ms)
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

    pub async fn update(&self, audiobook: &Audiobook) -> Result<()> {
        let result = sqlx::query(
            "UPDATE audiobooks SET
                voice_id = ?, status = ?, manifest_hash = ?, manifest_ext = ?,
                duration_ms = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&audiobook.voice_id)
        .bind(audiobook.status)
        .bind(&audiobook.manifest_hash)
        .bind(&audiobook.manifest_ext)
        .bind(audiobook.duration_ms)
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
}
