use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::storage::{
    AssetKind, AssetRef, BookFormat, Category, CategorySuggestionView, ClassificationConfig,
    ClassificationStatus, SuggestionSource, SuggestionStatus,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategorySuggestionDto {
    pub id: String,
    pub book_id: String,
    pub book_title: String,
    pub author: Option<String>,
    pub format: BookFormat,
    pub audio_only: bool,
    pub cover_path: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub proposed_name: Option<String>,
    pub score: f64,
    pub source: SuggestionSource,
    pub reason: Option<String>,
    pub status: SuggestionStatus,
    pub created_at: DateTime<Utc>,
}

#[tauri::command]
pub async fn classification_get_config(state: State<'_, AppState>) -> Result<ClassificationConfig> {
    state.classification.config().await
}

#[tauri::command]
pub async fn classification_save_config(
    state: State<'_, AppState>,
    config: ClassificationConfig,
) -> Result<ClassificationConfig> {
    state.classification.save_config(config).await
}

#[tauri::command]
pub async fn classification_status(state: State<'_, AppState>) -> Result<ClassificationStatus> {
    state.classification.status().await
}

#[tauri::command]
pub async fn library_classify(
    state: State<'_, AppState>,
    app: AppHandle,
    book_id: String,
    force: Option<bool>,
) -> Result<Vec<CategorySuggestionDto>> {
    let book = state
        .storage
        .books
        .get(&book_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("libro {book_id}")))?;

    let suggestions = state
        .classification
        .classify_book(Some(&app), &book, force.unwrap_or(false), true)
        .await?;

    Ok(to_dtos(&state, suggestions))
}

#[tauri::command]
pub async fn library_classify_pending(
    state: State<'_, AppState>,
    app: AppHandle,
    force: Option<bool>,
) -> Result<()> {
    let classification = state.classification.clone();
    let force = force.unwrap_or(false);
    tauri::async_runtime::spawn(async move {
        if let Err(error) = classification.classify_pending(Some(&app), force).await {
            tracing::warn!(%error, "no se pudo clasificar la biblioteca pendiente");
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn category_suggestions(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<Vec<CategorySuggestionDto>> {
    let suggestions = state.classification.pending_for_book(&book_id).await?;
    Ok(to_dtos(&state, suggestions))
}

#[tauri::command]
pub async fn category_suggestion_list_pending(
    state: State<'_, AppState>,
) -> Result<Vec<CategorySuggestionDto>> {
    let suggestions = state.classification.pending_all().await?;
    Ok(to_dtos(&state, suggestions))
}

#[tauri::command]
pub async fn category_suggestion_accept(
    state: State<'_, AppState>,
    app: AppHandle,
    suggestion_id: String,
) -> Result<()> {
    state
        .classification
        .accept_suggestion(Some(&app), &suggestion_id)
        .await
}

#[tauri::command]
pub async fn category_suggestion_dismiss(
    state: State<'_, AppState>,
    app: AppHandle,
    suggestion_id: String,
) -> Result<()> {
    state
        .classification
        .dismiss_suggestion(Some(&app), &suggestion_id)
        .await
}

#[tauri::command]
pub async fn category_suggestion_accept_all(
    state: State<'_, AppState>,
    app: AppHandle,
    book_id: String,
) -> Result<usize> {
    state
        .classification
        .accept_all_for_book(Some(&app), &book_id)
        .await
}

#[tauri::command]
pub async fn category_seed_apply(state: State<'_, AppState>) -> Result<Vec<Category>> {
    state.classification.apply_seed_categories().await
}

fn to_dtos(
    state: &AppState,
    suggestions: Vec<CategorySuggestionView>,
) -> Vec<CategorySuggestionDto> {
    suggestions
        .into_iter()
        .map(|suggestion| {
            let cover_path = suggestion
                .cover_hash
                .as_deref()
                .zip(suggestion.cover_ext.as_deref())
                .and_then(|(hash, extension)| {
                    let reference = AssetRef {
                        kind: AssetKind::Cover,
                        hash: hash.to_string(),
                        extension: extension.to_string(),
                    };
                    state
                        .storage
                        .assets
                        .path(&reference)
                        .ok()
                        .map(|path| path.display().to_string())
                });

            CategorySuggestionDto {
                id: suggestion.id,
                book_id: suggestion.book_id,
                book_title: suggestion.book_title,
                author: suggestion.author,
                format: suggestion.format,
                audio_only: suggestion.audio_only,
                cover_path,
                category_id: suggestion.category_id,
                category_name: suggestion.category_name,
                category_color: suggestion.category_color,
                proposed_name: suggestion.proposed_name,
                score: suggestion.score,
                source: suggestion.source,
                reason: suggestion.reason,
                status: suggestion.status,
                created_at: suggestion.created_at,
            }
        })
        .collect()
}
