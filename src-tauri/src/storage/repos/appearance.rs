use sqlx::SqlitePool;

use super::{new_id, now, require_affected};
use crate::error::{KoraError, Result};
use crate::storage::model::{AppPreferences, CustomTheme, NewCustomTheme};

#[derive(Clone)]
pub struct AppearanceRepo {
    pool: SqlitePool,
}

impl AppearanceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn preferences(&self, owner_account_id: &str) -> Result<Option<AppPreferences>> {
        Ok(sqlx::query_as::<_, AppPreferences>(
            "SELECT * FROM app_preferences WHERE owner_account_id = ?",
        )
        .bind(owner_account_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn ensure_preferences(&self, owner_account_id: &str) -> Result<AppPreferences> {
        if let Some(preferences) = self.preferences(owner_account_id).await? {
            return Ok(preferences);
        }

        sqlx::query(
            "INSERT INTO app_preferences
                (owner_account_id, theme_id, texture, locale, onboarding_done,
                 library_layout, updated_at)
             VALUES (?, 'light', 'paper', 'es', 0, 'grid', ?)
             ON CONFLICT (owner_account_id) DO NOTHING",
        )
        .bind(owner_account_id)
        .bind(now())
        .execute(&self.pool)
        .await?;

        self.preferences(owner_account_id)
            .await?
            .ok_or_else(|| KoraError::Internal("preferencias no encontradas".into()))
    }

    pub async fn save_preferences(&self, preferences: &AppPreferences) -> Result<AppPreferences> {
        let result = sqlx::query(
            "UPDATE app_preferences SET
                theme_id = ?, texture = ?, locale = ?, onboarding_done = ?,
                library_layout = ?, updated_at = ?
             WHERE owner_account_id = ?",
        )
        .bind(&preferences.theme_id)
        .bind(&preferences.texture)
        .bind(&preferences.locale)
        .bind(preferences.onboarding_done)
        .bind(&preferences.library_layout)
        .bind(now())
        .bind(&preferences.owner_account_id)
        .execute(&self.pool)
        .await?;
        require_affected(result, "preferencias de apariencia")?;
        self.preferences(&preferences.owner_account_id)
            .await?
            .ok_or_else(|| KoraError::Internal("preferencias no encontradas".into()))
    }

    pub async fn list_themes(&self, owner_account_id: &str) -> Result<Vec<CustomTheme>> {
        Ok(sqlx::query_as::<_, CustomTheme>(
            "SELECT * FROM custom_themes WHERE owner_account_id = ? ORDER BY name",
        )
        .bind(owner_account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn get_theme(
        &self,
        owner_account_id: &str,
        theme_id: &str,
    ) -> Result<Option<CustomTheme>> {
        Ok(sqlx::query_as::<_, CustomTheme>(
            "SELECT * FROM custom_themes WHERE id = ? AND owner_account_id = ?",
        )
        .bind(theme_id)
        .bind(owner_account_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn save_theme(&self, new: NewCustomTheme) -> Result<CustomTheme> {
        let timestamp = now();
        sqlx::query(
            "INSERT INTO custom_themes
                (id, name, background, foreground, accent, created_at, updated_at,
                 owner_account_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET
                name = excluded.name,
                background = excluded.background,
                foreground = excluded.foreground,
                accent = excluded.accent,
                updated_at = excluded.updated_at
             WHERE custom_themes.owner_account_id = excluded.owner_account_id",
        )
        .bind(&new.id)
        .bind(&new.name)
        .bind(&new.background)
        .bind(&new.foreground)
        .bind(&new.accent)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&new.owner_account_id)
        .execute(&self.pool)
        .await?;

        self.get_theme(&new.owner_account_id, &new.id)
            .await?
            .ok_or_else(|| KoraError::Internal("tema recién guardado no encontrado".into()))
    }

    pub async fn delete_theme(&self, owner_account_id: &str, theme_id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM custom_themes WHERE id = ? AND owner_account_id = ?")
            .bind(theme_id)
            .bind(owner_account_id)
            .execute(&self.pool)
            .await?;
        require_affected(result, &format!("tema {theme_id}"))
    }

    pub fn new_theme_id(&self) -> String {
        new_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::AppPreferences;
    use crate::storage::test_support::TestStorage;

    #[tokio::test]
    async fn preferences_and_custom_themes_round_trip() -> Result<()> {
        let (test, identity) = TestStorage::with_identity().await;
        let repo = &test.storage.appearance;
        let owner = &identity.account.id;

        let preferences = repo.ensure_preferences(owner).await?;
        assert_eq!(preferences.theme_id, "light");
        assert!(!preferences.onboarding_done);

        let mut changed = preferences.clone();
        changed.theme_id = "contrast".to_string();
        changed.locale = "en".to_string();
        changed.onboarding_done = true;
        let saved = repo.save_preferences(&changed).await?;
        assert_eq!(saved.theme_id, "contrast");
        assert_eq!(saved.locale, "en");
        assert!(saved.onboarding_done);

        let theme = repo
            .save_theme(NewCustomTheme {
                id: repo.new_theme_id(),
                name: "Bosque".to_string(),
                background: "#102010".to_string(),
                foreground: "#e8ffe8".to_string(),
                accent: "#66cc88".to_string(),
                owner_account_id: owner.clone(),
            })
            .await?;
        assert_eq!(theme.name, "Bosque");
        assert_eq!(repo.list_themes(owner).await?.len(), 1);

        let mut selected = saved;
        selected.theme_id = theme.id.clone();
        repo.save_preferences(&selected).await?;
        repo.delete_theme(owner, &theme.id).await?;
        assert!(repo.get_theme(owner, &theme.id).await?.is_none());

        // El comando corrige la preferencia; el repo conserva el id hasta que
        // se guarde de nuevo, así que comprobamos que se puede volver a light.
        selected.theme_id = "light".to_string();
        let fallback = repo.save_preferences(&selected).await?;
        assert_eq!(fallback.theme_id, "light");
        Ok(())
    }

    #[test]
    fn preferences_model_is_serializable() {
        let preferences = AppPreferences {
            owner_account_id: "owner".to_string(),
            theme_id: "light".to_string(),
            texture: "paper".to_string(),
            locale: "es".to_string(),
            onboarding_done: false,
            library_layout: "grid".to_string(),
            updated_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&preferences).expect("serializar");
        assert!(json.contains("onboardingDone"));
    }
}
