use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use super::{new_id, now, require_affected};
use crate::error::{KoraError, Result};
use crate::storage::model::{
    Account, ConnectionKey, Device, NewAccount, NewConnectionKey, NewDevice,
};

#[derive(Clone)]
pub struct AccountRepo {
    pool: SqlitePool,
}

impl AccountRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewAccount) -> Result<Account> {
        let id = new_id();
        let timestamp = now();

        sqlx::query(
            "INSERT INTO accounts (id, display_name, email, is_local, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.display_name)
        .bind(&new.email)
        .bind(new.is_local)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("cuenta recién creada no encontrada".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Account>> {
        Ok(sqlx::query_as::<_, Account>(
            "SELECT id, display_name, email, is_local, username, pin_hash, public_key,
             account_key_ciphertext, account_key_nonce, connection_key_hash,
             connection_key_ciphertext, connection_key_nonce,
             connection_key_rotated_at, avatar_hash, avatar_ext, created_at, updated_at FROM accounts WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list(&self) -> Result<Vec<Account>> {
        Ok(sqlx::query_as::<_, Account>(
            "SELECT id, display_name, email, is_local, username, pin_hash, public_key,
             account_key_ciphertext, account_key_nonce, connection_key_hash,
             connection_key_ciphertext, connection_key_nonce,
             connection_key_rotated_at, avatar_hash, avatar_ext, created_at, updated_at FROM accounts ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn find_local(&self) -> Result<Option<Account>> {
        Ok(
            sqlx::query_as::<_, Account>(
                "SELECT id, display_name, email, is_local, username, pin_hash, public_key,
             account_key_ciphertext, account_key_nonce, connection_key_hash,
             connection_key_ciphertext, connection_key_nonce,
             connection_key_rotated_at, avatar_hash, avatar_ext, created_at, updated_at FROM accounts WHERE is_local = 1 LIMIT 1",
            )
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn count_by_avatar_hash(&self, hash: &str) -> Result<i64> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE avatar_hash = ?")
            .bind(hash)
            .fetch_one(&self.pool)
            .await?;
        Ok(count.0)
    }

    pub async fn update(&self, account: &Account) -> Result<()> {
        let result = sqlx::query(
            "UPDATE accounts SET display_name = ?, email = ?, avatar_hash = ?, avatar_ext = ?,
             updated_at = ? WHERE id = ?",
        )
        .bind(&account.display_name)
        .bind(&account.email)
        .bind(&account.avatar_hash)
        .bind(&account.avatar_ext)
        .bind(now())
        .bind(&account.id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("cuenta {}", account.id))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM accounts WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("cuenta {id}"))
    }
}

#[derive(Clone)]
pub struct DeviceRepo {
    pool: SqlitePool,
}

impl DeviceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewDevice) -> Result<Device> {
        let id = new_id();

        sqlx::query(
            "INSERT INTO devices
                (id, account_id, name, platform, public_key, last_seen_at, is_local, created_at)
             VALUES (?, ?, ?, ?, ?, NULL, ?, ?)",
        )
        .bind(&id)
        .bind(&new.account_id)
        .bind(&new.name)
        .bind(&new.platform)
        .bind(&new.public_key)
        .bind(new.is_local)
        .bind(now())
        .execute(&self.pool)
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("dispositivo recién creado no encontrado".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Device>> {
        Ok(
            sqlx::query_as::<_, Device>(
                "SELECT id, account_id, name, platform, public_key, certificate_fingerprint,
             trusted, sas_verified_at, last_address, last_port, peer_account_id,
             peer_username, peer_account_public_key, account_signature,
             peer_account_signature, revoked_at, last_seen_at, is_local, created_at FROM devices WHERE id = ?",
            )
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_account(&self, account_id: &str) -> Result<Vec<Device>> {
        Ok(sqlx::query_as::<_, Device>(
            "SELECT * FROM devices WHERE account_id = ? ORDER BY created_at",
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn find_local(&self, account_id: &str) -> Result<Option<Device>> {
        Ok(sqlx::query_as::<_, Device>(
            "SELECT * FROM devices WHERE account_id = ? AND is_local = 1 LIMIT 1",
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn count_by_account(&self, account_id: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM devices WHERE account_id = ?")
            .bind(account_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn touch(&self, id: &str, at: DateTime<Utc>) -> Result<()> {
        let result = sqlx::query("UPDATE devices SET last_seen_at = ? WHERE id = ?")
            .bind(at)
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("dispositivo {id}"))
    }

    pub async fn set_identity(&self, id: &str, public_key: &str, fingerprint: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE devices SET public_key = ?, certificate_fingerprint = ?, last_seen_at = ?
             WHERE id = ?",
        )
        .bind(public_key)
        .bind(fingerprint)
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("dispositivo {id}"))
    }

    /// Inserta o actualiza un dispositivo remoto ya emparejado.
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_remote(
        &self,
        account_id: &str,
        device_id: &str,
        name: &str,
        platform: &str,
        public_key: Option<&str>,
        fingerprint: &str,
        address: Option<&str>,
        port: Option<i64>,
    ) -> Result<Device> {
        let timestamp = now();
        sqlx::query(
            "INSERT INTO devices
                (id, account_id, name, platform, public_key, certificate_fingerprint,
                 trusted, sas_verified_at, last_address, last_port, last_seen_at, is_local,
                 created_at)
             VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, 0, ?)
             ON CONFLICT (id) DO UPDATE SET
                name = excluded.name,
                platform = excluded.platform,
                public_key = excluded.public_key,
                certificate_fingerprint = excluded.certificate_fingerprint,
                trusted = 1,
                sas_verified_at = excluded.sas_verified_at,
                last_address = excluded.last_address,
                last_port = excluded.last_port,
                last_seen_at = excluded.last_seen_at",
        )
        .bind(device_id)
        .bind(account_id)
        .bind(name)
        .bind(platform)
        .bind(public_key)
        .bind(fingerprint)
        .bind(timestamp)
        .bind(address)
        .bind(port)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;

        self.get(device_id)
            .await?
            .ok_or_else(|| KoraError::Internal("dispositivo remoto no encontrado".into()))
    }

    pub async fn list_trusted(&self, account_id: &str) -> Result<Vec<Device>> {
        Ok(sqlx::query_as::<_, Device>(
            "SELECT * FROM devices
             WHERE account_id = ? AND trusted = 1 AND is_local = 0
             ORDER BY last_seen_at DESC, name",
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn mark_seen(
        &self,
        id: &str,
        address: Option<&str>,
        port: Option<i64>,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE devices SET last_seen_at = ?, last_address = coalesce(?, last_address),
                last_port = coalesce(?, last_port) WHERE id = ?",
        )
        .bind(now())
        .bind(address)
        .bind(port)
        .bind(id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("dispositivo {id}"))
    }

    pub async fn revoke_trust(&self, id: &str) -> Result<()> {
        let result = sqlx::query("UPDATE devices SET trusted = 0 WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("dispositivo {id}"))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM devices WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("dispositivo {id}"))
    }
}

#[derive(Clone)]
pub struct ConnectionKeyRepo {
    pool: SqlitePool,
}

impl ConnectionKeyRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewConnectionKey) -> Result<ConnectionKey> {
        let id = new_id();

        sqlx::query(
            "INSERT INTO connection_keys
                (id, account_id, device_id, kind, public_key, secret_ref, created_at, expires_at, revoked_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(&id)
        .bind(&new.account_id)
        .bind(&new.device_id)
        .bind(new.kind)
        .bind(&new.public_key)
        .bind(&new.secret_ref)
        .bind(now())
        .bind(new.expires_at)
        .execute(&self.pool)
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("clave de conexión no encontrada".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<ConnectionKey>> {
        Ok(
            sqlx::query_as::<_, ConnectionKey>("SELECT * FROM connection_keys WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_account(&self, account_id: &str) -> Result<Vec<ConnectionKey>> {
        Ok(sqlx::query_as::<_, ConnectionKey>(
            "SELECT * FROM connection_keys WHERE account_id = ? ORDER BY created_at DESC",
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn revoke(&self, id: &str) -> Result<()> {
        let result = sqlx::query("UPDATE connection_keys SET revoked_at = ? WHERE id = ?")
            .bind(now())
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("clave de conexión {id}"))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM connection_keys WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("clave de conexión {id}"))
    }
}

impl AccountRepo {
    #[allow(clippy::too_many_arguments)]
    pub async fn set_account_identity(
        &self,
        id: &str,
        username: &str,
        pin_hash: &str,
        public_key: &str,
        account_key_ciphertext: &str,
        account_key_nonce: &str,
        connection_key_hash: &str,
        connection_key_ciphertext: &str,
        connection_key_nonce: &str,
        rotated_at: DateTime<Utc>,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE accounts SET
                username = ?, pin_hash = ?, public_key = ?,
                account_key_ciphertext = ?, account_key_nonce = ?,
                connection_key_hash = ?, connection_key_ciphertext = ?, connection_key_nonce = ?,
                connection_key_rotated_at = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(username)
        .bind(pin_hash)
        .bind(public_key)
        .bind(account_key_ciphertext)
        .bind(account_key_nonce)
        .bind(connection_key_hash)
        .bind(connection_key_ciphertext)
        .bind(connection_key_nonce)
        .bind(rotated_at)
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("cuenta {id}"))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_secrets(
        &self,
        id: &str,
        pin_hash: &str,
        account_key_ciphertext: &str,
        account_key_nonce: &str,
        connection_key_ciphertext: &str,
        connection_key_nonce: &str,
        connection_key_hash: &str,
        rotated_at: DateTime<Utc>,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE accounts SET
                pin_hash = ?, account_key_ciphertext = ?, account_key_nonce = ?,
                connection_key_ciphertext = ?, connection_key_nonce = ?,
                connection_key_hash = ?, connection_key_rotated_at = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(pin_hash)
        .bind(account_key_ciphertext)
        .bind(account_key_nonce)
        .bind(connection_key_ciphertext)
        .bind(connection_key_nonce)
        .bind(connection_key_hash)
        .bind(rotated_at)
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("cuenta {id}"))
    }
}

impl DeviceRepo {
    pub async fn revoke(&self, id: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE devices SET trusted = 0, revoked_at = ?, last_seen_at = ? WHERE id = ?",
        )
        .bind(now())
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;
        require_affected(result, &format!("dispositivo {id}"))
    }
}

impl DeviceRepo {
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_paired(
        &self,
        owner_account_id: &str,
        device_id: &str,
        name: &str,
        platform: &str,
        device_public_key: &str,
        fingerprint: &str,
        peer_account_id: &str,
        peer_username: &str,
        peer_account_public_key: &str,
        account_signature: Option<&str>,
        peer_account_signature: Option<&str>,
        address: Option<&str>,
        port: Option<i64>,
    ) -> Result<Device> {
        let timestamp = now();
        sqlx::query(
            "INSERT INTO devices
                (id, account_id, name, platform, public_key, certificate_fingerprint,
                 trusted, sas_verified_at, last_address, last_port, last_seen_at, is_local,
                 created_at, peer_account_id, peer_username, peer_account_public_key,
                 account_signature, peer_account_signature)
             VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET
                name = excluded.name,
                platform = excluded.platform,
                public_key = excluded.public_key,
                certificate_fingerprint = excluded.certificate_fingerprint,
                trusted = 1,
                sas_verified_at = excluded.sas_verified_at,
                last_address = excluded.last_address,
                last_port = excluded.last_port,
                last_seen_at = excluded.last_seen_at,
                peer_account_id = excluded.peer_account_id,
                peer_username = excluded.peer_username,
                peer_account_public_key = excluded.peer_account_public_key,
                account_signature = excluded.account_signature,
                peer_account_signature = excluded.peer_account_signature,
                revoked_at = NULL",
        )
        .bind(device_id)
        .bind(owner_account_id)
        .bind(name)
        .bind(platform)
        .bind(device_public_key)
        .bind(fingerprint)
        .bind(timestamp)
        .bind(address)
        .bind(port)
        .bind(timestamp)
        .bind(timestamp)
        .bind(peer_account_id)
        .bind(peer_username)
        .bind(peer_account_public_key)
        .bind(account_signature)
        .bind(peer_account_signature)
        .execute(&self.pool)
        .await?;

        self.get(device_id)
            .await?
            .ok_or_else(|| KoraError::Internal("dispositivo emparejado no encontrado".into()))
    }
}
