use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use super::{new_id, now, require_affected, track_owner};
use crate::error::{KoraError, Result};
use crate::storage::model::{
    AuditEntry, CrdtDocument, NewAuditEntry, NewSyncEntry, ResourceOwner, ResourceType,
    SyncCollection, SyncEntry, SyncPending, SyncState,
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
                (resource_type, resource_id, peer_device_id, version, hash, last_synced_at, state)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (resource_type, resource_id, peer_device_id) DO UPDATE SET
                version = excluded.version,
                hash = excluded.hash,
                last_synced_at = excluded.last_synced_at,
                state = excluded.state",
        )
        .bind(new.resource_type)
        .bind(&new.resource_id)
        .bind(&new.peer_device_id)
        .bind(new.version)
        .bind(&new.hash)
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

    pub async fn list_by_peer(&self, peer_device_id: &str) -> Result<Vec<SyncEntry>> {
        Ok(sqlx::query_as::<_, SyncEntry>(
            "SELECT * FROM sync_state WHERE peer_device_id = ? ORDER BY resource_type, resource_id",
        )
        .bind(peer_device_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn set_hash(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
        peer_device_id: &str,
        hash: Option<&str>,
        state: SyncState,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE sync_state SET hash = ?, state = ?, last_synced_at = ?
             WHERE resource_type = ? AND resource_id = ? AND peer_device_id = ?",
        )
        .bind(hash)
        .bind(state)
        .bind(now())
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

#[derive(Clone)]
pub struct SyncPendingRepo {
    pool: SqlitePool,
}

impl SyncPendingRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, limit: i64) -> Result<Vec<SyncPending>> {
        Ok(sqlx::query_as::<_, SyncPending>(
            "SELECT * FROM sync_pending ORDER BY changed_at LIMIT ?",
        )
        .bind(limit.clamp(1, 1000))
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn count(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sync_pending")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn mark_syncing(&self, resource_type: ResourceType, resource_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE sync_pending SET state = 'syncing'
             WHERE resource_type = ? AND resource_id = ?",
        )
        .bind(resource_type)
        .bind(resource_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear(&self, resource_type: ResourceType, resource_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM sync_pending WHERE resource_type = ? AND resource_id = ?")
            .bind(resource_type)
            .bind(resource_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn clear_all(&self) -> Result<()> {
        sqlx::query("DELETE FROM sync_pending")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct CrdtDocumentRepo {
    pool: SqlitePool,
}

impl CrdtDocumentRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, document: &CrdtDocument) -> Result<()> {
        sqlx::query(
            "INSERT INTO crdt_documents
                (resource_type, resource_id, snapshot, hash, updated_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (resource_type, resource_id) DO UPDATE SET
                snapshot = excluded.snapshot,
                hash = excluded.hash,
                updated_at = excluded.updated_at",
        )
        .bind(document.resource_type)
        .bind(&document.resource_id)
        .bind(&document.snapshot)
        .bind(&document.hash)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
    ) -> Result<Option<CrdtDocument>> {
        Ok(sqlx::query_as::<_, CrdtDocument>(
            "SELECT * FROM crdt_documents WHERE resource_type = ? AND resource_id = ?",
        )
        .bind(resource_type)
        .bind(resource_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_by_type(&self, resource_type: ResourceType) -> Result<Vec<CrdtDocument>> {
        Ok(sqlx::query_as::<_, CrdtDocument>(
            "SELECT * FROM crdt_documents WHERE resource_type = ? ORDER BY updated_at",
        )
        .bind(resource_type)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn delete(&self, resource_type: ResourceType, resource_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM crdt_documents WHERE resource_type = ? AND resource_id = ?")
            .bind(resource_type)
            .bind(resource_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct SyncCollectionRepo {
    pool: SqlitePool,
}

impl SyncCollectionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(
        &self,
        owner_account_id: &str,
        device_id: &str,
    ) -> Result<Vec<SyncCollection>> {
        Ok(sqlx::query_as::<_, SyncCollection>(
            "SELECT * FROM sync_device_collections
             WHERE owner_account_id = ? AND device_id = ? ORDER BY category_id",
        )
        .bind(owner_account_id)
        .bind(device_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn enabled_category_ids(
        &self,
        owner_account_id: &str,
        device_id: &str,
    ) -> Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT category_id FROM sync_device_collections
             WHERE owner_account_id = ? AND device_id = ? AND enabled = 1
             ORDER BY category_id",
        )
        .bind(owner_account_id)
        .bind(device_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub async fn has_selection(&self, owner_account_id: &str, device_id: &str) -> Result<bool> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sync_device_collections
             WHERE owner_account_id = ? AND device_id = ?",
        )
        .bind(owner_account_id)
        .bind(device_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0 > 0)
    }

    pub async fn set(
        &self,
        owner_account_id: &str,
        device_id: &str,
        category_id: &str,
        enabled: bool,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_device_collections
                (device_id, category_id, enabled, updated_at, owner_account_id)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (device_id, category_id) DO UPDATE SET
                enabled = excluded.enabled,
                updated_at = excluded.updated_at,
                owner_account_id = excluded.owner_account_id",
        )
        .bind(device_id)
        .bind(category_id)
        .bind(enabled)
        .bind(now())
        .bind(owner_account_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn replace(
        &self,
        owner_account_id: &str,
        device_id: &str,
        enabled_category_ids: &[String],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM sync_device_collections
             WHERE owner_account_id = ? AND device_id = ?",
        )
        .bind(owner_account_id)
        .bind(device_id)
        .execute(&mut *tx)
        .await?;
        for category_id in enabled_category_ids {
            sqlx::query(
                "INSERT INTO sync_device_collections
                    (device_id, category_id, enabled, updated_at, owner_account_id)
                 VALUES (?, ?, 1, ?, ?)",
            )
            .bind(device_id)
            .bind(category_id)
            .bind(now())
            .bind(owner_account_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct SyncAliasRepo {
    pool: SqlitePool,
}

impl SyncAliasRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn put(
        &self,
        peer_device_id: &str,
        resource_type: ResourceType,
        remote_id: &str,
        local_id: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_aliases
                (peer_device_id, resource_type, remote_id, local_id, created_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (peer_device_id, resource_type, remote_id) DO UPDATE SET
                local_id = excluded.local_id",
        )
        .bind(peer_device_id)
        .bind(resource_type)
        .bind(remote_id)
        .bind(local_id)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(
        &self,
        peer_device_id: &str,
        resource_type: ResourceType,
        remote_id: &str,
    ) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT local_id FROM sync_aliases
             WHERE peer_device_id = ? AND resource_type = ? AND remote_id = ?",
        )
        .bind(peer_device_id)
        .bind(resource_type)
        .bind(remote_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(id,)| id))
    }
}
