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
        Ok(
            sqlx::query_as::<_, Account>("SELECT * FROM accounts WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list(&self) -> Result<Vec<Account>> {
        Ok(
            sqlx::query_as::<_, Account>("SELECT * FROM accounts ORDER BY created_at")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn find_local(&self) -> Result<Option<Account>> {
        Ok(
            sqlx::query_as::<_, Account>("SELECT * FROM accounts WHERE is_local = 1 LIMIT 1")
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn update(&self, account: &Account) -> Result<()> {
        let result = sqlx::query(
            "UPDATE accounts SET display_name = ?, email = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&account.display_name)
        .bind(&account.email)
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
            sqlx::query_as::<_, Device>("SELECT * FROM devices WHERE id = ?")
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
