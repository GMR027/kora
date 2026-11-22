use serde::Serialize;
use tauri::State;

use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::storage::{AppPreferences, CustomTheme, NewCustomTheme};

const BUILTIN_THEMES: &[&str] = &["light", "dark", "sepia", "contrast", "mint"];
const TEXTURES: &[&str] = &[
    "none",
    "paper",
    "parchment",
    "soft-dark",
    "grid",
    "linen",
    "dots",
    "waves",
    "stripes",
    "grain",
    "gingham",
];
const LOCALES: &[&str] = &["es", "en"];
const LAYOUTS: &[&str] = &["grid", "list"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceState {
    pub preferences: AppPreferences,
    pub themes: Vec<CustomTheme>,
}

#[tauri::command]
pub async fn get_appearance(state: State<'_, AppState>) -> Result<AppearanceState> {
    appearance_state(&state).await
}

#[tauri::command]
pub async fn save_appearance(
    state: State<'_, AppState>,
    theme_id: String,
    texture: String,
    locale: String,
    onboarding_done: Option<bool>,
    library_layout: Option<String>,
) -> Result<AppearanceState> {
    validate_theme(&state, &theme_id).await?;
    validate_texture(&texture)?;
    validate_locale(&locale)?;
    let layout = library_layout.unwrap_or_else(|| "grid".to_string());
    validate_layout(&layout)?;

    let mut preferences = state
        .storage
        .appearance
        .ensure_preferences(&state.identity.account.id)
        .await?;
    preferences.theme_id = theme_id;
    preferences.texture = texture;
    preferences.locale = locale;
    preferences.library_layout = layout;
    if let Some(done) = onboarding_done {
        preferences.onboarding_done = done;
    }
    state
        .storage
        .appearance
        .save_preferences(&preferences)
        .await?;
    appearance_state(&state).await
}

#[tauri::command]
pub async fn save_custom_theme(
    state: State<'_, AppState>,
    id: Option<String>,
    name: String,
    background: String,
    foreground: String,
    accent: String,
) -> Result<CustomTheme> {
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(KoraError::InvalidInput(
            "el nombre del tema debe tener entre 1 y 60 caracteres".into(),
        ));
    }
    for color in [&background, &foreground, &accent] {
        validate_hex(color)?;
    }

    let id = id.unwrap_or_else(|| state.storage.appearance.new_theme_id());
    if id.chars().count() > 80 {
        return Err(KoraError::InvalidInput("id de tema inválido".into()));
    }
    let theme = state
        .storage
        .appearance
        .save_theme(NewCustomTheme {
            id,
            name,
            background: background.to_ascii_lowercase(),
            foreground: foreground.to_ascii_lowercase(),
            accent: accent.to_ascii_lowercase(),
            owner_account_id: state.identity.account.id.clone(),
        })
        .await?;
    Ok(theme)
}

#[tauri::command]
pub async fn delete_custom_theme(
    state: State<'_, AppState>,
    theme_id: String,
) -> Result<AppearanceState> {
    let owner = state.identity.account.id.clone();
    state
        .storage
        .appearance
        .delete_theme(&owner, &theme_id)
        .await?;
    let mut preferences = state.storage.appearance.ensure_preferences(&owner).await?;
    if preferences.theme_id == theme_id {
        preferences.theme_id = "light".to_string();
        state
            .storage
            .appearance
            .save_preferences(&preferences)
            .await?;
    }
    appearance_state(&state).await
}

async fn appearance_state(state: &AppState) -> Result<AppearanceState> {
    let owner = state.identity.account.id.clone();
    let preferences = state.storage.appearance.ensure_preferences(&owner).await?;
    let themes = state.storage.appearance.list_themes(&owner).await?;
    Ok(AppearanceState {
        preferences,
        themes,
    })
}

async fn validate_theme(state: &AppState, theme_id: &str) -> Result<()> {
    if BUILTIN_THEMES.contains(&theme_id) {
        return Ok(());
    }
    if state
        .storage
        .appearance
        .get_theme(&state.identity.account.id, theme_id)
        .await?
        .is_some()
    {
        return Ok(());
    }
    Err(KoraError::InvalidInput("tema desconocido".into()))
}

fn validate_texture(texture: &str) -> Result<()> {
    if TEXTURES.contains(&texture) {
        Ok(())
    } else {
        Err(KoraError::InvalidInput("textura desconocida".into()))
    }
}

fn validate_locale(locale: &str) -> Result<()> {
    if LOCALES.contains(&locale) {
        Ok(())
    } else {
        Err(KoraError::InvalidInput("idioma desconocido".into()))
    }
}

fn validate_layout(layout: &str) -> Result<()> {
    if LAYOUTS.contains(&layout) {
        Ok(())
    } else {
        Err(KoraError::InvalidInput(
            "vista de biblioteca desconocida".into(),
        ))
    }
}

fn validate_hex(color: &str) -> Result<()> {
    let value = color.trim();
    let valid = (value.len() == 4 || value.len() == 7)
        && value.starts_with('#')
        && value[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(KoraError::InvalidInput(
            "los colores deben tener formato #rgb o #rrggbb".into(),
        ))
    }
}
