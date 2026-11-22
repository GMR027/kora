use sqlx::SqlitePool;

use super::{new_id, now, require_affected};
use crate::error::Result;
use crate::storage::model::{
    AliasSource, BookClassification, CategoryAlias, CategorySuggestion, CategorySuggestionView,
    ClassificationRunStatus, NewCategoryAlias, NewCategorySuggestion, SuggestionStatus,
};

#[derive(Clone)]
pub struct ClassificationRepo {
    pool: SqlitePool,
}

impl ClassificationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // --- Alias -------------------------------------------------------------

    pub async fn list_aliases(&self, owner_account_id: &str) -> Result<Vec<CategoryAlias>> {
        Ok(sqlx::query_as::<_, CategoryAlias>(
            "SELECT * FROM category_aliases
             WHERE owner_account_id = ?
             ORDER BY length(alias_normalized) DESC, alias_normalized",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn insert_alias(&self, alias: &NewCategoryAlias) -> Result<bool> {
        let result = sqlx::query(
            "INSERT OR IGNORE INTO category_aliases
                (id, category_id, alias, alias_normalized, source, created_at,
                 owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(new_id())
        .bind(&alias.category_id)
        .bind(&alias.alias)
        .bind(&alias.alias_normalized)
        .bind(alias.source)
        .bind(now())
        .bind(&alias.owner_account_id)
        .bind(&alias.origin_share_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn alias_names_for_category(&self, category_id: &str) -> Result<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT alias FROM category_aliases WHERE category_id = ?")
                .bind(category_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(alias,)| alias).collect())
    }

    pub async fn insert_learned_aliases(
        &self,
        category_id: &str,
        aliases: &[String],
        owner_account_id: &str,
    ) -> Result<()> {
        for alias in aliases {
            let normalized = crate::classify::normalize::normalize_key(alias);
            if normalized.len() < 2 || normalized.len() > 120 {
                continue;
            }
            self.insert_alias(&NewCategoryAlias {
                category_id: category_id.to_string(),
                alias: alias.trim().to_string(),
                alias_normalized: normalized,
                source: AliasSource::Learned,
                owner_account_id: owner_account_id.to_string(),
                origin_share_id: None,
            })
            .await?;
        }
        Ok(())
    }

    // --- Sugerencias -------------------------------------------------------

    pub async fn insert_suggestion(
        &self,
        suggestion: &NewCategorySuggestion,
    ) -> Result<CategorySuggestion> {
        sqlx::query(
            "INSERT INTO category_suggestions
                (id, book_id, category_id, proposed_name, proposed_name_normalized,
                 target_key, score, source, reason, status, created_at,
                 owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?)
             ON CONFLICT DO NOTHING",
        )
        .bind(new_id())
        .bind(&suggestion.book_id)
        .bind(&suggestion.category_id)
        .bind(&suggestion.proposed_name)
        .bind(&suggestion.proposed_name_normalized)
        .bind(&suggestion.target_key)
        .bind(suggestion.score)
        .bind(suggestion.source)
        .bind(&suggestion.reason)
        .bind(now())
        .bind(&suggestion.owner_account_id)
        .bind(&suggestion.origin_share_id)
        .execute(&self.pool)
        .await?;

        self.get_pending_by_target(&suggestion.book_id, &suggestion.target_key)
            .await?
            .ok_or_else(|| {
                crate::error::KoraError::Internal("sugerencia recién creada no encontrada".into())
            })
    }

    pub async fn get_suggestion(&self, id: &str) -> Result<Option<CategorySuggestion>> {
        Ok(sqlx::query_as::<_, CategorySuggestion>(
            "SELECT * FROM category_suggestions WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn get_pending_by_target(
        &self,
        book_id: &str,
        target_key: &str,
    ) -> Result<Option<CategorySuggestion>> {
        Ok(sqlx::query_as::<_, CategorySuggestion>(
            "SELECT * FROM category_suggestions
             WHERE book_id = ? AND target_key = ? AND status = 'pending'",
        )
        .bind(book_id)
        .bind(target_key)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_pending_by_book(
        &self,
        owner_account_id: &str,
        book_id: &str,
    ) -> Result<Vec<CategorySuggestionView>> {
        Ok(sqlx::query_as::<_, CategorySuggestionView>(&format!(
            "{SUGGESTION_VIEW_SQL}
                 WHERE cs.owner_account_id = ? AND cs.book_id = ? AND cs.status = 'pending'
                 ORDER BY cs.score DESC, cs.created_at DESC"
        ))
        .bind(owner_account_id)
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_pending_by_owner(
        &self,
        owner_account_id: &str,
    ) -> Result<Vec<CategorySuggestionView>> {
        Ok(sqlx::query_as::<_, CategorySuggestionView>(&format!(
            "{SUGGESTION_VIEW_SQL}
                 WHERE cs.owner_account_id = ? AND cs.status = 'pending'
                 ORDER BY cs.score DESC, cs.created_at DESC"
        ))
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn count_pending(&self, owner_account_id: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM category_suggestions
             WHERE owner_account_id = ? AND status = 'pending'",
        )
        .bind(owner_account_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn has_pending_for_book(&self, book_id: &str) -> Result<bool> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM category_suggestions
             WHERE book_id = ? AND status = 'pending' LIMIT 1",
        )
        .bind(book_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    pub async fn resolve_suggestion(
        &self,
        id: &str,
        owner_account_id: &str,
        status: SuggestionStatus,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE category_suggestions
             SET status = ?, resolved_at = ?
             WHERE id = ? AND owner_account_id = ? AND status = 'pending'",
        )
        .bind(status)
        .bind(now())
        .bind(id)
        .bind(owner_account_id)
        .execute(&self.pool)
        .await?;

        require_affected(result, &format!("sugerencia {id} pendiente"))
    }

    // --- Estado de ejecución ----------------------------------------------

    pub async fn get_run(&self, book_id: &str) -> Result<Option<BookClassification>> {
        Ok(sqlx::query_as::<_, BookClassification>(
            "SELECT * FROM book_classification WHERE book_id = ?",
        )
        .bind(book_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_run(
        &self,
        book_id: &str,
        input_hash: &str,
        model: Option<&str>,
        status: ClassificationRunStatus,
        error: Option<&str>,
        owner_account_id: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO book_classification
                (book_id, input_hash, model, status, error, updated_at, owner_account_id)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (book_id) DO UPDATE SET
                input_hash = excluded.input_hash,
                model = excluded.model,
                status = excluded.status,
                error = excluded.error,
                updated_at = excluded.updated_at,
                owner_account_id = excluded.owner_account_id",
        )
        .bind(book_id)
        .bind(input_hash)
        .bind(model)
        .bind(status)
        .bind(error)
        .bind(now())
        .bind(owner_account_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn count_done(&self, owner_account_id: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM book_classification
             WHERE owner_account_id = ? AND status = 'done'",
        )
        .bind(owner_account_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }
}

const SUGGESTION_VIEW_SQL: &str = "SELECT cs.id, cs.book_id, b.title AS book_title, b.author,
        b.format, b.audio_only, b.cover_hash, b.cover_ext,
        cs.category_id, c.name AS category_name, c.color AS category_color,
        cs.proposed_name, cs.score, cs.source, cs.reason, cs.status, cs.created_at
     FROM category_suggestions cs
     JOIN books b ON b.id = cs.book_id
     LEFT JOIN categories c ON c.id = cs.category_id";
