use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use super::{new_id, now, require_affected, track_owner};
use crate::error::{KoraError, Result};
use crate::storage::model::{
    AuditEntry, NewAuditEntry, NewSyncEntry, ResourceOwner, ResourceType, SyncEntry, SyncState,
};

#[derive(Clone)]
pub struct SyncStateRepo {
    pool: SqlitePool,
}

impl SyncStateRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn upsert(&self, new: NewSyncEntry) -> Result<SyncEntry> {
        sqlx::query(
            "INSERT INTO sync_state
                (resource_type, resource_id, peer_device_id, version, last_synced_at, state)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (resource_type, resource_id, peer_device_id) DO UPDATE SET
                version = excluded.version,
                last_synced_at = excluded.last_synced_at,
                state = excluded.state",
        )
        .bind(new.resource_type)
        .bind(&new.resource_id)
        .bind(&new.peer_device_id)
        .bind(new.version)
        .bind(now())
        .bind(new.state)
        .execute(&self.pool)
        .await?;

        self.get(new.resource_type, &new.resource_id, &new.peer_device_id)
            .await?
            .ok_or_else(|| KoraError::Internal("sync_state recién guardado no encontrado".into()))
    }

    pub async fn get(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
        peer_device_id: &str,
    ) -> Result<Option<SyncEntry>> {
        Ok(sqlx::query_as::<_, SyncEntry>(
            "SELECT * FROM sync_state
             WHERE resource_type = ? AND resource_id = ? AND peer_device_id = ?",
        )
        .bind(resource_type)
        .bind(resource_id)
        .bind(peer_device_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_by_resource(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
    ) -> Result<Vec<SyncEntry>> {
        Ok(sqlx::query_as::<_, SyncEntry>(
            "SELECT * FROM sync_state WHERE resource_type = ? AND resource_id = ?",
        )
        .bind(resource_type)
        .bind(resource_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn delete(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
        peer_device_id: &str,
    ) -> Result<()> {
        let result = sqlx::query(
            "DELETE FROM sync_state
             WHERE resource_type = ? AND resource_id = ? AND peer_device_id = ?",
        )
        .bind(resource_type)
        .bind(resource_id)
        .bind(peer_device_id)
        .execute(&self.pool)
        .await?;

        require_affected(
            result,
            &format!("sync_state {resource_id}/{peer_device_id}"),
        )
    }

    pub async fn delete_by_resource(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
    ) -> Result<()> {
        sqlx::query("DELETE FROM sync_state WHERE resource_type = ? AND resource_id = ?")
            .bind(resource_type)
            .bind(resource_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn set_state(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
        peer_device_id: &str,
        state: SyncState,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE sync_state SET state = ? WHERE resource_type = ? AND resource_id = ? AND peer_device_id = ?",
        )
        .bind(state)
        .bind(resource_type)
        .bind(resource_id)
        .bind(peer_device_id)
        .execute(&self.pool)
        .await?;

        require_affected(
            result,
            &format!("sync_state {resource_id}/{peer_device_id}"),
        )
    }
}

#[derive(Clone)]
pub struct ResourceOwnerRepo {
    pool: SqlitePool,
}

impl ResourceOwnerRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn upsert(&self, owner: &ResourceOwner) -> Result<()> {
        track_owner(
            &self.pool,
            owner.resource_type,
            &owner.resource_id,
            &owner.owner_account_id,
            owner.origin_share_id.as_deref(),
        )
        .await
    }

    pub async fn get(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
    ) -> Result<Option<ResourceOwner>> {
        Ok(sqlx::query_as::<_, ResourceOwner>(
            "SELECT * FROM resource_owners WHERE resource_type = ? AND resource_id = ?",
        )
        .bind(resource_type)
        .bind(resource_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_by_owner(&self, owner_account_id: &str) -> Result<Vec<ResourceOwner>> {
        Ok(sqlx::query_as::<_, ResourceOwner>(
            "SELECT * FROM resource_owners WHERE owner_account_id = ? ORDER BY updated_at DESC",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn delete(&self, resource_type: ResourceType, resource_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM resource_owners WHERE resource_type = ? AND resource_id = ?")
            .bind(resource_type)
            .bind(resource_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct AuditLogRepo {
    pool: SqlitePool,
}

impl AuditLogRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn append(&self, new: NewAuditEntry) -> Result<AuditEntry> {
        let id = new_id();

        sqlx::query(
            "INSERT INTO audit_log
                (id, actor_account_id, action, resource_type, resource_id, metadata, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.actor_account_id)
        .bind(&new.action)
        .bind(new.resource_type)
        .bind(&new.resource_id)
        .bind(&new.metadata)
        .bind(now())
        .execute(&self.pool)
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("entrada de auditoría no encontrada".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<AuditEntry>> {
        Ok(
            sqlx::query_as::<_, AuditEntry>("SELECT * FROM audit_log WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list(&self, limit: i64) -> Result<Vec<AuditEntry>> {
        Ok(sqlx::query_as::<_, AuditEntry>(
            "SELECT * FROM audit_log ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_by_resource(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
        limit: i64,
    ) -> Result<Vec<AuditEntry>> {
        Ok(sqlx::query_as::<_, AuditEntry>(
            "SELECT * FROM audit_log
             WHERE resource_type = ? AND resource_id = ?
             ORDER BY created_at DESC LIMIT ?",
        )
        .bind(resource_type)
        .bind(resource_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn purge_before(&self, cutoff: DateTime<Utc>) -> Result<u64> {
        let result = sqlx::query("DELETE FROM audit_log WHERE created_at < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}
