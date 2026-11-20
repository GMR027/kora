use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};

use crate::error::{KoraError, Result};

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

/// Las migraciones se embeben en el binario: no hace falta leer archivos en
/// Android ni en un bundle instalado. Añade siempre al final, nunca edites una
/// migración ya publicada (el checksum lo detectaría).
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "0001_identity",
        sql: include_str!("migrations/0001_identity.sql"),
    },
    Migration {
        version: 2,
        name: "0002_sharing",
        sql: include_str!("migrations/0002_sharing.sql"),
    },
    Migration {
        version: 3,
        name: "0003_sync",
        sql: include_str!("migrations/0003_sync.sql"),
    },
    Migration {
        version: 4,
        name: "0004_content",
        sql: include_str!("migrations/0004_content.sql"),
    },
];

pub const LATEST_VERSION: i64 = 4;

pub async fn run(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            checksum   TEXT NOT NULL,
            applied_at TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    let applied = sqlx::query("SELECT version, checksum FROM schema_version ORDER BY version")
        .fetch_all(pool)
        .await?;

    for row in &applied {
        let version: i64 = row.get("version");
        let checksum: String = row.get("checksum");

        let migration = MIGRATIONS
            .iter()
            .find(|migration| migration.version == version)
            .ok_or_else(|| {
                KoraError::Database(format!(
                    "la base tiene aplicada la migración {version}, que no existe en este binario"
                ))
            })?;

        if checksum != checksum_of(migration.sql) {
            return Err(KoraError::Database(format!(
                "checksum distinto en la migración {} ({}); el archivo cambió después de aplicarse",
                migration.version, migration.name
            )));
        }
    }

    for migration in MIGRATIONS {
        if applied
            .iter()
            .any(|row| row.get::<i64, _>("version") == migration.version)
        {
            continue;
        }

        let mut transaction = pool.begin().await?;

        sqlx::raw_sql(migration.sql)
            .execute(&mut *transaction)
            .await?;

        sqlx::query(
            "INSERT INTO schema_version (version, name, checksum, applied_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(migration.version)
        .bind(migration.name)
        .bind(checksum_of(migration.sql))
        .bind(Utc::now())
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
    }

    Ok(())
}

fn checksum_of(sql: &str) -> String {
    hex::encode(Sha256::digest(sql.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    #[tokio::test]
    async fn applies_migrations_once_and_tracks_versions() {
        let path = std::env::temp_dir().join(format!("kora-mig-{}.sqlite", uuid::Uuid::new_v4()));
        let pool = db::connect(&path).await.expect("abrir");

        run(&pool).await.expect("migrar");
        run(&pool).await.expect("migrar de nuevo");

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM schema_version")
            .fetch_one(&pool)
            .await
            .expect("contar");
        assert_eq!(count.0, LATEST_VERSION);

        let latest: (i64,) = sqlx::query_as("SELECT MAX(version) FROM schema_version")
            .fetch_one(&pool)
            .await
            .expect("última versión");
        assert_eq!(latest.0, LATEST_VERSION);

        let tables: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'
             AND name IN ('accounts', 'books', 'annotations', 'voices', 'audit_log')",
        )
        .fetch_one(&pool)
        .await
        .expect("tablas");
        assert_eq!(tables.0, 5);

        pool.close().await;
        std::fs::remove_file(&path).ok();
        std::fs::remove_file(path.with_extension("sqlite-wal")).ok();
        std::fs::remove_file(path.with_extension("sqlite-shm")).ok();
    }
}
