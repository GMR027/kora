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
    Migration {
        version: 5,
        name: "0005_reader",
        sql: include_str!("migrations/0005_reader.sql"),
    },
    Migration {
        version: 6,
        name: "0006_pdf",
        sql: include_str!("migrations/0006_pdf.sql"),
    },
    Migration {
        version: 7,
        name: "0007_tts",
        sql: include_str!("migrations/0007_tts.sql"),
    },
    Migration {
        version: 8,
        name: "0008_voice_cloning",
        sql: include_str!("migrations/0008_voice_cloning.sql"),
    },
    Migration {
        version: 9,
        name: "0009_audiobooks",
        sql: include_str!("migrations/0009_audiobooks.sql"),
    },
    Migration {
        version: 10,
        name: "0010_annotations",
        sql: include_str!("migrations/0010_annotations.sql"),
    },
    Migration {
        version: 11,
        name: "0011_sync",
        sql: include_str!("migrations/0011_sync.sql"),
    },
    Migration {
        version: 12,
        name: "0012_accounts",
        sql: include_str!("migrations/0012_accounts.sql"),
    },
    Migration {
        version: 13,
        name: "0013_features",
        sql: include_str!("migrations/0013_features.sql"),
    },
    Migration {
        version: 14,
        name: "0014_appearance",
        sql: include_str!("migrations/0014_appearance.sql"),
    },
    Migration {
        version: 15,
        name: "0015_reader_typography",
        sql: include_str!("migrations/0015_reader_typography.sql"),
    },
    Migration {
        version: 16,
        name: "0016_favorites",
        sql: include_str!("migrations/0016_favorites.sql"),
    },
    Migration {
        version: 17,
        name: "0017_font_face_compat",
        sql: include_str!("migrations/0017_font_face_compat.sql"),
    },
    Migration {
        version: 18,
        name: "0018_audiobook_queue",
        sql: include_str!("migrations/0018_audiobook_queue.sql"),
    },
    Migration {
        version: 19,
        name: "0019_audiobook_cast",
        sql: include_str!("migrations/0019_audiobook_cast.sql"),
    },
    Migration {
        version: 20,
        name: "0020_voice_marks",
        sql: include_str!("migrations/0020_voice_marks.sql"),
    },
    Migration {
        version: 21,
        name: "0021_audiobook_favorites",
        sql: include_str!("migrations/0021_audiobook_favorites.sql"),
    },
    Migration {
        version: 22,
        name: "0022_audio_only_books",
        sql: include_str!("migrations/0022_audio_only_books.sql"),
    },
    Migration {
        version: 23,
        name: "0023_classification",
        sql: include_str!("migrations/0023_classification.sql"),
    },
    Migration {
        version: 24,
        name: "0024_trash_retry",
        sql: include_str!("migrations/0024_trash_retry.sql"),
    },
];

pub const LATEST_VERSION: i64 = 24;

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
