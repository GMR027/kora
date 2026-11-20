use sqlx::SqlitePool;

use super::now;
use crate::error::Result;
use crate::storage::model::{NewReaderSettings, ReaderSettings};

/// Ajustes tipográficos del lector. Hay una fila por libro y se eliminan en
/// cascada cuando se borra el libro.
#[derive(Clone)]
pub struct ReaderSettingsRepo {
    pool: SqlitePool,
}

impl ReaderSettingsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn get(&self, book_id: &str) -> Result<Option<ReaderSettings>> {
        Ok(
            sqlx::query_as::<_, ReaderSettings>("SELECT * FROM reader_settings WHERE book_id = ?")
                .bind(book_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn save(&self, new: NewReaderSettings) -> Result<ReaderSettings> {
        sqlx::query(
            "INSERT INTO reader_settings
                (book_id, font_size, line_height, margin, font_family, theme, texture,
                 mode, zoom, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (book_id) DO UPDATE SET
                font_size = excluded.font_size,
                line_height = excluded.line_height,
                margin = excluded.margin,
                font_family = excluded.font_family,
                theme = excluded.theme,
                texture = excluded.texture,
                mode = excluded.mode,
                zoom = excluded.zoom,
                updated_at = excluded.updated_at,
                owner_account_id = excluded.owner_account_id,
                origin_share_id = excluded.origin_share_id",
        )
        .bind(&new.book_id)
        .bind(new.font_size)
        .bind(new.line_height)
        .bind(new.margin)
        .bind(&new.font_family)
        .bind(&new.theme)
        .bind(&new.texture)
        .bind(&new.mode)
        .bind(new.zoom)
        .bind(now())
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        self.get(&new.book_id).await?.ok_or_else(|| {
            crate::error::KoraError::Internal("ajustes del lector no encontrados".into())
        })
    }

    pub async fn delete(&self, book_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM reader_settings WHERE book_id = ?")
            .bind(book_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
