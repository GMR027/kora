use std::collections::HashSet;

use sqlx::SqlitePool;

use super::{new_id, now, require_affected};
use crate::error::{KoraError, Result};
use crate::storage::model::{AudiobookAlbum, NewAudiobookAlbum};

/// Álbumes de audiolibros: colecciones ordenadas y locales de una cuenta.
/// Las categorías siguen siendo etiquetas; un álbum aporta orden y una
/// reproducción continua.
#[derive(Clone)]
pub struct AudiobookAlbumRepo {
    pool: SqlitePool,
}

impl AudiobookAlbumRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewAudiobookAlbum) -> Result<AudiobookAlbum> {
        let id = new_id();
        let timestamp = now();
        sqlx::query(
            "INSERT INTO audiobook_albums
                (id, title, description, sort_order, created_at, updated_at, owner_account_id)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.title)
        .bind(&new.description)
        .bind(new.sort_order)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&new.owner_account_id)
        .execute(&self.pool)
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("álbum recién creado no encontrado".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<AudiobookAlbum>> {
        Ok(
            sqlx::query_as::<_, AudiobookAlbum>("SELECT * FROM audiobook_albums WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn get_owned(
        &self,
        id: &str,
        owner_account_id: &str,
    ) -> Result<Option<AudiobookAlbum>> {
        Ok(sqlx::query_as::<_, AudiobookAlbum>(
            "SELECT * FROM audiobook_albums WHERE id = ? AND owner_account_id = ?",
        )
        .bind(id)
        .bind(owner_account_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_by_owner(&self, owner_account_id: &str) -> Result<Vec<AudiobookAlbum>> {
        Ok(sqlx::query_as::<_, AudiobookAlbum>(
            "SELECT * FROM audiobook_albums
             WHERE owner_account_id = ?
             ORDER BY sort_order, title COLLATE NOCASE, created_at",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update(&self, album: &AudiobookAlbum) -> Result<()> {
        let result = sqlx::query(
            "UPDATE audiobook_albums
             SET title = ?, description = ?, sort_order = ?, updated_at = ?
             WHERE id = ? AND owner_account_id = ?",
        )
        .bind(&album.title)
        .bind(&album.description)
        .bind(album.sort_order)
        .bind(now())
        .bind(&album.id)
        .bind(&album.owner_account_id)
        .execute(&self.pool)
        .await?;
        require_affected(result, &format!("álbum {}", album.id))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM audiobook_albums WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        require_affected(result, &format!("álbum {id}"))
    }

    /// Ids de todos los audiolibros del álbum, incluidos los que pertenecen a
    /// libros en la papelera. Se usa para operaciones internas.
    pub async fn list_items(&self, album_id: &str) -> Result<Vec<String>> {
        Ok(sqlx::query_as::<_, (String,)>(
            "SELECT audiobook_id FROM audiobook_album_items
             WHERE album_id = ?
             ORDER BY position, added_at",
        )
        .bind(album_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(id,)| id)
        .collect())
    }

    /// Ids visibles para la biblioteca: oculta audiolibros cuyo libro está en
    /// la papelera. La pertenencia se conserva para que reaparezcan al
    /// restaurar el libro.
    pub async fn list_visible_items(
        &self,
        album_id: &str,
        owner_account_id: &str,
    ) -> Result<Vec<String>> {
        Ok(sqlx::query_as::<_, (String,)>(
            "SELECT ai.audiobook_id
             FROM audiobook_album_items ai
             JOIN audiobooks a ON a.id = ai.audiobook_id
             JOIN books b ON b.id = a.book_id
             WHERE ai.album_id = ?
               AND ai.owner_account_id = ?
               AND b.deleted_at IS NULL
             ORDER BY ai.position, ai.added_at",
        )
        .bind(album_id)
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(id,)| id)
        .collect())
    }

    /// Añade audiolibros al final del álbum. Los que ya pertenecen al álbum
    /// se ignoran para que la operación sea idempotente.
    pub async fn add_items(
        &self,
        album_id: &str,
        owner_account_id: &str,
        audiobook_ids: &[String],
    ) -> Result<()> {
        if audiobook_ids.is_empty() {
            return Ok(());
        }
        let mut transaction = self.pool.begin().await?;
        let next: (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(position), -1) + 1
             FROM audiobook_album_items WHERE album_id = ?",
        )
        .bind(album_id)
        .fetch_one(&mut *transaction)
        .await?;
        let added_at = now();
        for (index, audiobook_id) in audiobook_ids.iter().enumerate() {
            sqlx::query(
                "INSERT INTO audiobook_album_items
                    (album_id, audiobook_id, position, added_at, owner_account_id)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT (album_id, audiobook_id) DO NOTHING",
            )
            .bind(album_id)
            .bind(audiobook_id)
            .bind(next.0 + index as i64)
            .bind(added_at)
            .bind(owner_account_id)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn remove_item(&self, album_id: &str, audiobook_id: &str) -> Result<()> {
        let result = sqlx::query(
            "DELETE FROM audiobook_album_items WHERE album_id = ? AND audiobook_id = ?",
        )
        .bind(album_id)
        .bind(audiobook_id)
        .execute(&self.pool)
        .await?;
        require_affected(
            result,
            &format!("audiolibro {audiobook_id} del álbum {album_id}"),
        )
    }

    /// Reordena los ids indicados. Se acepta un subconjunto para no fallar si
    /// el álbum conserva elementos ocultos de libros en la papelera.
    pub async fn reorder(&self, album_id: &str, ordered_ids: &[String]) -> Result<()> {
        if ordered_ids.is_empty() {
            return Ok(());
        }
        let seen: HashSet<&str> = ordered_ids.iter().map(String::as_str).collect();
        if seen.len() != ordered_ids.len() {
            return Err(KoraError::InvalidInput(
                "el orden del álbum contiene audiolibros duplicados".into(),
            ));
        }
        let current = self.list_items(album_id).await?;
        let current: HashSet<&str> = current.iter().map(String::as_str).collect();
        if let Some(missing) = ordered_ids.iter().find(|id| !current.contains(id.as_str())) {
            return Err(KoraError::NotFound(format!(
                "el audiolibro {missing} no pertenece al álbum"
            )));
        }

        let mut transaction = self.pool.begin().await?;
        for (position, audiobook_id) in ordered_ids.iter().enumerate() {
            sqlx::query(
                "UPDATE audiobook_album_items SET position = ?
                 WHERE album_id = ? AND audiobook_id = ?",
            )
            .bind(position as i64)
            .bind(album_id)
            .bind(audiobook_id)
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query("UPDATE audiobook_albums SET updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(album_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Número de audiolibros visibles en un álbum. Se usa en listados
    /// agregados sin cargar todos los ids.
    pub async fn count_visible_items(&self, album_id: &str, owner_account_id: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*)
             FROM audiobook_album_items ai
             JOIN audiobooks a ON a.id = ai.audiobook_id
             JOIN books b ON b.id = a.book_id
             WHERE ai.album_id = ? AND ai.owner_account_id = ?
               AND b.deleted_at IS NULL",
        )
        .bind(album_id)
        .bind(owner_account_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }
}
