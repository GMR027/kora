use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use super::{new_id, now, require_affected, track_owner, untrack_owner};
use crate::error::{KoraError, Result};
use crate::storage::model::{Achievement, DiaryEntry, NewAchievement, NewDiaryEntry, ResourceType};

#[derive(Clone)]
pub struct DiaryRepo {
    pool: SqlitePool,
}

impl DiaryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewDiaryEntry) -> Result<DiaryEntry> {
        let id = new_id();
        let timestamp = now();

        sqlx::query(
            "INSERT INTO diary_entries
                (id, book_id, entry_date, body, mood, created_at, updated_at,
                 owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.book_id)
        .bind(&new.entry_date)
        .bind(&new.body)
        .bind(&new.mood)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::DiaryEntry,
            &id,
            &new.owner_account_id,
            new.origin_share_id.as_deref(),
        )
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("entrada de diario no encontrada".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<DiaryEntry>> {
        Ok(
            sqlx::query_as::<_, DiaryEntry>("SELECT * FROM diary_entries WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_owner(&self, owner_account_id: &str) -> Result<Vec<DiaryEntry>> {
        Ok(sqlx::query_as::<_, DiaryEntry>(
            "SELECT * FROM diary_entries WHERE owner_account_id = ? ORDER BY entry_date DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update(&self, entry: &DiaryEntry) -> Result<()> {
        let result = sqlx::query(
            "UPDATE diary_entries SET
                book_id = ?, entry_date = ?, body = ?, mood = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&entry.book_id)
        .bind(&entry.entry_date)
        .bind(&entry.body)
        .bind(&entry.mood)
        .bind(now())
        .bind(&entry.id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("entrada de diario {}", entry.id))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM diary_entries WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("entrada de diario {id}"))?;
        untrack_owner(&self.pool, ResourceType::DiaryEntry, id).await
    }
}

#[derive(Clone)]
pub struct AchievementRepo {
    pool: SqlitePool,
}

impl AchievementRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn upsert(&self, new: NewAchievement) -> Result<Achievement> {
        let timestamp = now();

        let existing = self.get_by_key(&new.owner_account_id, &new.key).await?;

        let id = match &existing {
            Some(achievement) => achievement.id.clone(),
            None => new_id(),
        };

        sqlx::query(
            "INSERT INTO achievements
                (id, key, title, description, progress, unlocked_at,
                 created_at, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (owner_account_id, key) DO UPDATE SET
                title = excluded.title,
                description = excluded.description,
                progress = excluded.progress,
                unlocked_at = excluded.unlocked_at,
                updated_at = excluded.updated_at,
                origin_share_id = excluded.origin_share_id",
        )
        .bind(&id)
        .bind(&new.key)
        .bind(&new.title)
        .bind(&new.description)
        .bind(new.progress)
        .bind(new.unlocked_at)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::Achievement,
            &id,
            &new.owner_account_id,
            new.origin_share_id.as_deref(),
        )
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("logro recién guardado no encontrado".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Achievement>> {
        Ok(
            sqlx::query_as::<_, Achievement>("SELECT * FROM achievements WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn get_by_key(
        &self,
        owner_account_id: &str,
        key: &str,
    ) -> Result<Option<Achievement>> {
        Ok(sqlx::query_as::<_, Achievement>(
            "SELECT * FROM achievements WHERE owner_account_id = ? AND key = ?",
        )
        .bind(owner_account_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_by_owner(&self, owner_account_id: &str) -> Result<Vec<Achievement>> {
        Ok(sqlx::query_as::<_, Achievement>(
            "SELECT * FROM achievements WHERE owner_account_id = ? ORDER BY key",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn unlock(&self, id: &str, at: DateTime<Utc>) -> Result<()> {
        let result = sqlx::query(
            "UPDATE achievements SET progress = 1.0, unlocked_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(at)
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("logro {id}"))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM achievements WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("logro {id}"))?;
        untrack_owner(&self.pool, ResourceType::Achievement, id).await
    }
}
