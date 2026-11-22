use sqlx::SqlitePool;

use super::{new_id, now, require_affected};
use crate::error::{KoraError, Result};
use crate::storage::model::{
    NewReceivedShare, NewShare, NewShareKey, ReceivedShare, ResourceType, Share, ShareKey,
};

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
                (id, owner_account_id, resource_type, resource_id, scope_type, permission,
                 expires_at, max_uses, uses, share_key_hash, share_key_ciphertext,
                 share_key_nonce, created_at, updated_at, revoked_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(&id)
        .bind(&new.owner_account_id)
        .bind(new.resource_type)
        .bind(&new.resource_id)
        .bind(new.scope_type)
        .bind(new.permission)
        .bind(new.expires_at)
        .bind(new.max_uses)
        .bind(&new.share_key_hash)
        .bind(&new.share_key_ciphertext)
        .bind(&new.share_key_nonce)
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

    pub async fn find_by_key_hash(&self, hash: &str) -> Result<Option<Share>> {
        Ok(sqlx::query_as::<_, Share>(
            "SELECT * FROM shares WHERE share_key_hash = ? AND revoked_at IS NULL",
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn increment_uses(&self, id: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE shares SET uses = uses + 1, updated_at = ?
             WHERE id = ? AND revoked_at IS NULL
               AND (expires_at IS NULL OR expires_at > ?)
               AND (max_uses IS NULL OR uses < max_uses)",
        )
        .bind(now())
        .bind(id)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
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

    pub async fn find_by_key_hash(&self, hash: &str) -> Result<Option<Share>> {
        Ok(sqlx::query_as::<_, Share>(
            "SELECT * FROM shares WHERE share_key_hash = ? AND revoked_at IS NULL",
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn increment_uses(&self, id: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE shares SET uses = uses + 1, updated_at = ?
             WHERE id = ? AND revoked_at IS NULL
               AND (expires_at IS NULL OR expires_at > ?)
               AND (max_uses IS NULL OR uses < max_uses)",
        )
        .bind(now())
        .bind(id)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
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

#[derive(Clone)]
pub struct ReceivedShareRepo {
    pool: SqlitePool,
}

impl ReceivedShareRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn put(&self, new: &NewReceivedShare) -> Result<ReceivedShare> {
        sqlx::query(
            "INSERT INTO received_shares
                (share_id, owner_account_id, owner_username, owner_public_key,
                 owner_account_hint, owner_device_id, scope_type, resource_type, permission,
                 expires_at, max_uses, uses, revoked_at, accepted_at, owner_signature,
                 peer_account_signature, last_checked_at, owner_account_signature)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?)
             ON CONFLICT (share_id) DO UPDATE SET
                owner_account_id = excluded.owner_account_id,
                owner_username = excluded.owner_username,
                owner_public_key = excluded.owner_public_key,
                owner_account_hint = excluded.owner_account_hint,
                owner_device_id = excluded.owner_device_id,
                scope_type = excluded.scope_type,
                resource_type = excluded.resource_type,
                permission = excluded.permission,
                expires_at = excluded.expires_at,
                max_uses = excluded.max_uses,
                accepted_at = excluded.accepted_at,
                owner_signature = excluded.owner_signature,
                peer_account_signature = excluded.peer_account_signature,
                owner_account_signature = excluded.owner_account_signature",
        )
        .bind(&new.share_id)
        .bind(&new.owner_account_id)
        .bind(&new.owner_username)
        .bind(&new.owner_public_key)
        .bind(&new.owner_account_hint)
        .bind(&new.owner_device_id)
        .bind(new.scope_type)
        .bind(new.resource_type)
        .bind(new.permission)
        .bind(new.expires_at)
        .bind(new.max_uses)
        .bind(new.uses)
        .bind(now())
        .bind(&new.owner_signature)
        .bind(&new.peer_account_signature)
        .bind(now())
        .bind(&new.owner_account_signature)
        .execute(&self.pool)
        .await?;

        self.get(&new.share_id)
            .await?
            .ok_or_else(|| KoraError::Internal("share recibido no encontrado".into()))
    }

    pub async fn get(&self, share_id: &str) -> Result<Option<ReceivedShare>> {
        Ok(
            sqlx::query_as::<_, ReceivedShare>("SELECT * FROM received_shares WHERE share_id = ?")
                .bind(share_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list(&self) -> Result<Vec<ReceivedShare>> {
        Ok(sqlx::query_as::<_, ReceivedShare>(
            "SELECT * FROM received_shares ORDER BY accepted_at DESC",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn mark_revoked(&self, share_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE received_shares SET revoked_at = ?, last_checked_at = ?
             WHERE share_id = ? AND revoked_at IS NULL",
        )
        .bind(now())
        .bind(now())
        .bind(share_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn touch(&self, share_id: &str) -> Result<()> {
        sqlx::query("UPDATE received_shares SET last_checked_at = ? WHERE share_id = ?")
            .bind(now())
            .bind(share_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

impl ReceivedShareRepo {
    pub async fn delete(&self, share_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM received_shares WHERE share_id = ?")
            .bind(share_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
