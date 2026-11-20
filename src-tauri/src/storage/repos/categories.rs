use sqlx::SqlitePool;

use super::{new_id, now, require_affected, track_owner, untrack_owner};
use crate::error::{KoraError, Result};
use crate::storage::model::{Book, Category, NewCategory, ResourceType};

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
        sqlx::query(
            "INSERT OR IGNORE INTO book_categories
                (book_id, category_id, created_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(book_id)
        .bind(category_id)
        .bind(now())
        .bind(owner_account_id)
        .bind(origin_share_id)
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

    pub async fn list_by_category(&self, category_id: &str) -> Result<Vec<Book>> {
        Ok(sqlx::query_as::<_, Book>(
            "SELECT b.* FROM books b
             JOIN book_categories bc ON bc.book_id = b.id
             WHERE bc.category_id = ?
             ORDER BY b.title COLLATE NOCASE",
        )
        .bind(category_id)
        .fetch_all(&self.pool)
        .await?)
    }
}
