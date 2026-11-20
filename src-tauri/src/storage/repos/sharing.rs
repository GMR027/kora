use sqlx::SqlitePool;

use super::{new_id, now, require_affected};
use crate::error::{KoraError, Result};
use crate::storage::model::{NewShare, NewShareKey, ResourceType, Share, ShareKey};

#[derive(Clone)]
pub struct ShareRepo {
    pool: SqlitePool,
}

impl ShareRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewShare) -> Result<Share> {
        let id = new_id();
        let timestamp = now();

        sqlx::query(
            "INSERT INTO shares
                (id, owner_account_id, resource_type, resource_id, created_at, updated_at, revoked_at)
             VALUES (?, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(&id)
        .bind(&new.owner_account_id)
        .bind(new.resource_type)
        .bind(&new.resource_id)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("share recién creado no encontrado".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Share>> {
        Ok(
            sqlx::query_as::<_, Share>("SELECT * FROM shares WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_owner(&self, owner_account_id: &str) -> Result<Vec<Share>> {
        Ok(sqlx::query_as::<_, Share>(
            "SELECT * FROM shares WHERE owner_account_id = ? ORDER BY created_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_by_resource(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
    ) -> Result<Vec<Share>> {
        Ok(sqlx::query_as::<_, Share>(
            "SELECT * FROM shares WHERE resource_type = ? AND resource_id = ? ORDER BY created_at DESC",
        )
        .bind(resource_type)
        .bind(resource_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn revoke(&self, id: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE shares SET revoked_at = ?, updated_at = ? WHERE id = ? AND revoked_at IS NULL",
        )
        .bind(now())
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("share {id}"))
    }

    pub async fn revoke_by_resource(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE shares SET revoked_at = ?, updated_at = ?
             WHERE resource_type = ? AND resource_id = ? AND revoked_at IS NULL",
        )
        .bind(now())
        .bind(now())
        .bind(resource_type)
        .bind(resource_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM shares WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("share {id}"))
    }
}

#[derive(Clone)]
pub struct ShareKeyRepo {
    pool: SqlitePool,
}

impl ShareKeyRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewShareKey) -> Result<ShareKey> {
        let id = new_id();

        sqlx::query(
            "INSERT INTO share_keys
                (id, share_id, recipient_account_id, wrapped_key, created_at, expires_at, revoked_at)
             VALUES (?, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(&id)
        .bind(&new.share_id)
        .bind(&new.recipient_account_id)
        .bind(&new.wrapped_key)
        .bind(now())
        .bind(new.expires_at)
        .execute(&self.pool)
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("share_key recién creada no encontrada".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<ShareKey>> {
        Ok(
            sqlx::query_as::<_, ShareKey>("SELECT * FROM share_keys WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_share(&self, share_id: &str) -> Result<Vec<ShareKey>> {
        Ok(sqlx::query_as::<_, ShareKey>(
            "SELECT * FROM share_keys WHERE share_id = ? ORDER BY created_at DESC",
        )
        .bind(share_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn revoke(&self, id: &str) -> Result<()> {
        let result =
            sqlx::query("UPDATE share_keys SET revoked_at = ? WHERE id = ? AND revoked_at IS NULL")
                .bind(now())
                .bind(id)
                .execute(&self.pool)
                .await?;

        require_affected(result, &format!("share_key {id}"))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM share_keys WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("share_key {id}"))
    }
}
