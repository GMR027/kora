use sqlx::SqlitePool;

use super::{new_id, now, require_affected, track_owner, untrack_owner};
use crate::error::{KoraError, Result};
use crate::storage::model::{Book, Category, CategoryAssignSource, NewCategory, ResourceType};

#[derive(Clone)]
pub struct CategoryRepo {
    pool: SqlitePool,
}

impl CategoryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, new: NewCategory) -> Result<Category> {
        let id = new_id();
        let timestamp = now();

        sqlx::query(
            "INSERT INTO categories
                (id, name, color, sort_order, created_at, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.name)
        .bind(&new.color)
        .bind(new.sort_order)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::Category,
            &id,
            &new.owner_account_id,
            new.origin_share_id.as_deref(),
        )
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| KoraError::Internal("categoría recién creada no encontrada".into()))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Category>> {
        Ok(
            sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_by_owner(&self, owner_account_id: &str) -> Result<Vec<Category>> {
        Ok(sqlx::query_as::<_, Category>(
            "SELECT * FROM categories WHERE owner_account_id = ?
             ORDER BY sort_order, name COLLATE NOCASE",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn find_by_name(
        &self,
        owner_account_id: &str,
        name: &str,
    ) -> Result<Option<Category>> {
        Ok(sqlx::query_as::<_, Category>(
            "SELECT * FROM categories WHERE owner_account_id = ? AND name = ?",
        )
        .bind(owner_account_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn update(&self, category: &Category) -> Result<()> {
        let result = sqlx::query(
            "UPDATE categories SET name = ?, color = ?, sort_order = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&category.name)
        .bind(&category.color)
        .bind(category.sort_order)
        .bind(now())
        .bind(&category.id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("categoría {}", category.id))?;

        track_owner(
            &self.pool,
            ResourceType::Category,
            &category.id,
            &category.owner_account_id,
            category.origin_share_id.as_deref(),
        )
        .await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM categories WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        require_affected(result, &format!("categoría {id}"))?;
        untrack_owner(&self.pool, ResourceType::Category, id).await
    }
}

#[derive(Clone)]
pub struct BookCategoryRepo {
    pool: SqlitePool,
}

impl BookCategoryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn assign(
        &self,
        book_id: &str,
        category_id: &str,
        owner_account_id: &str,
        origin_share_id: Option<&str>,
    ) -> Result<()> {
        self.assign_detailed(
            book_id,
            category_id,
            owner_account_id,
            origin_share_id,
            CategoryAssignSource::Manual,
            Some(1.0),
            true,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn assign_detailed(
        &self,
        book_id: &str,
        category_id: &str,
        owner_account_id: &str,
        origin_share_id: Option<&str>,
        source: CategoryAssignSource,
        confidence: Option<f64>,
        locked: bool,
    ) -> Result<()> {
        // Una asignación manual puede convertir en manual una sugerencia ya
        // aplicada; una automática nunca pisa una manual.
        sqlx::query(
            "INSERT INTO book_categories
                (book_id, category_id, created_at, owner_account_id, origin_share_id,
                 source, confidence, locked)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (book_id, category_id) DO UPDATE SET
                source = excluded.source,
                confidence = excluded.confidence,
                locked = excluded.locked
             WHERE book_categories.locked = 0 OR excluded.locked = 1",
        )
        .bind(book_id)
        .bind(category_id)
        .bind(now())
        .bind(owner_account_id)
        .bind(origin_share_id)
        .bind(source)
        .bind(confidence)
        .bind(locked)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn unassign(&self, book_id: &str, category_id: &str) -> Result<()> {
        let result =
            sqlx::query("DELETE FROM book_categories WHERE book_id = ? AND category_id = ?")
                .bind(book_id)
                .bind(category_id)
                .execute(&self.pool)
                .await?;

        require_affected(
            result,
            &format!("categoría {category_id} del libro {book_id}"),
        )
    }

    /// Asignaciones de toda la biblioteca en una sola consulta. Evita una
    /// consulta por libro al construir la lista de la biblioteca.
    pub async fn list_pairs_by_owner(
        &self,
        owner_account_id: &str,
    ) -> Result<Vec<(String, String)>> {
        Ok(sqlx::query_as::<_, (String, String)>(
            "SELECT bc.book_id, bc.category_id
             FROM book_categories bc
             JOIN books b ON b.id = bc.book_id
             WHERE b.owner_account_id = ? AND b.deleted_at IS NULL",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_by_book(&self, book_id: &str) -> Result<Vec<Category>> {
        Ok(sqlx::query_as::<_, Category>(
            "SELECT c.* FROM categories c
             JOIN book_categories bc ON bc.category_id = c.id
             WHERE bc.book_id = ?
             ORDER BY c.sort_order, c.name COLLATE NOCASE",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn has_locked_category(&self, book_id: &str) -> Result<bool> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM book_categories WHERE book_id = ? AND locked = 1 LIMIT 1",
        )
        .bind(book_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    pub async fn list_by_category(&self, category_id: &str) -> Result<Vec<Book>> {
        Ok(sqlx::query_as::<_, Book>(
            "SELECT b.* FROM books b
             JOIN book_categories bc ON bc.book_id = b.id
             WHERE bc.category_id = ? AND b.deleted_at IS NULL
             ORDER BY b.title COLLATE NOCASE",
        )
        .bind(category_id)
        .fetch_all(&self.pool)
        .await?)
    }
}

impl CategoryRepo {
    pub async fn upsert_from_sync(&self, category: &Category, owner: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO categories
                (id, name, color, sort_order, created_at, updated_at, owner_account_id,
                 origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET
                name = excluded.name,
                color = excluded.color,
                sort_order = excluded.sort_order,
                updated_at = excluded.updated_at,
                owner_account_id = excluded.owner_account_id",
        )
        .bind(&category.id)
        .bind(&category.name)
        .bind(&category.color)
        .bind(category.sort_order)
        .bind(category.created_at)
        .bind(category.updated_at)
        .bind(owner)
        .bind(&category.origin_share_id)
        .execute(&self.pool)
        .await?;

        track_owner(
            &self.pool,
            ResourceType::Category,
            &category.id,
            owner,
            category.origin_share_id.as_deref(),
        )
        .await?;
        Ok(())
    }
}

impl BookCategoryRepo {
    pub async fn replace_for_book(
        &self,
        book_id: &str,
        category_ids: &[String],
        owner: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM book_categories WHERE book_id = ?")
            .bind(book_id)
            .execute(&mut *tx)
            .await?;
        for category_id in category_ids {
            sqlx::query(
                "INSERT OR IGNORE INTO book_categories
                    (book_id, category_id, created_at, owner_account_id, origin_share_id,
                     source, confidence, locked)
                 VALUES (?, ?, ?, ?, NULL, 'sync', NULL, 0)",
            )
            .bind(book_id)
            .bind(category_id)
            .bind(now())
            .bind(owner)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
