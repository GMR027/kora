use tauri::{AppHandle, Emitter, State};

use crate::error::Result;
use crate::features::semantic::{SemanticHit, SemanticStatus};
use crate::features::{FeaturesStatus, ocr::default_engine};
use crate::state::AppState;

#[tauri::command]
pub async fn features_status(state: State<'_, AppState>) -> Result<FeaturesStatus> {
    Ok(FeaturesStatus {
        ocr: default_engine().available(),
        embeddings: state.semantic.provider().ready(),
        journal: true,
        achievements: true,
    })
}

#[tauri::command]
pub async fn semantic_status(state: State<'_, AppState>, app: AppHandle) -> Result<SemanticStatus> {
    let status = state.semantic.status().await?;
    if status.available && !status.indexing {
        let semantic = state.semantic.clone();
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = semantic.reconcile(Some(&handle), false).await {
                tracing::warn!(%error, "no se pudo reconciliar el índice semántico");
            }
        });
    }
    Ok(status)
}

#[tauri::command]
pub async fn prepare_semantic_model(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<SemanticStatus> {
    let _ = app.emit(
        "semantic::model",
        serde_json::json!({ "status": "preparing", "message": "Preparando el modelo de búsqueda" }),
    );
    state.semantic.prepare().await?;
    let status = state.semantic.status().await?;

    let semantic = state.semantic.clone();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = semantic.reconcile(Some(&handle), false).await {
            tracing::warn!(%error, "no se pudo reconciliar el índice semántico");
        }
    });

    let _ = app.emit(
        "semantic::model",
        serde_json::json!({ "status": "ready", "message": "Modelo listo" }),
    );
    Ok(status)
}

#[tauri::command]
pub async fn reindex_library(state: State<'_, AppState>, app: AppHandle) -> Result<SemanticStatus> {
    let status = state.semantic.status().await?;
    if !status.available {
        return Err(crate::error::KoraError::Unsupported(
            "prepara el modelo de búsqueda semántica antes de indexar".into(),
        ));
    }

    let semantic = state.semantic.clone();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = semantic.reconcile(Some(&handle), true).await {
            tracing::warn!(%error, "no se pudo reindexar la biblioteca");
        }
    });

    Ok(status)
}

#[tauri::command]
pub async fn search_semantic(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SemanticHit>> {
    state.semantic.search(&query, limit.unwrap_or(10)).await
}
