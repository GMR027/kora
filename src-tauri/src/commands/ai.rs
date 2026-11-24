use tauri::{AppHandle, Emitter, State};

use crate::ai::{AiConfig, AiLocalStatus, AiMessage, AiReply, AiStatus};
#[cfg(not(target_os = "android"))]
use crate::error::KoraError;
use crate::error::Result;
use crate::state::AppState;

#[tauri::command]
pub async fn ai_get_config(state: State<'_, AppState>) -> Result<AiConfig> {
    state.ai.config().await
}

#[tauri::command]
pub async fn ai_save_config(state: State<'_, AppState>, config: AiConfig) -> Result<AiConfig> {
    state.ai.save_config(config).await
}

#[tauri::command]
pub async fn ai_status(state: State<'_, AppState>) -> Result<AiStatus> {
    state.ai.status().await
}

#[tauri::command]
pub async fn ai_chat(
    state: State<'_, AppState>,
    messages: Vec<AiMessage>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<AiReply> {
    state.ai.chat(messages, temperature, max_tokens).await
}

#[tauri::command]
pub fn ai_local_status(state: State<'_, AppState>) -> AiLocalStatus {
    state.ai.local_status()
}

#[tauri::command]
pub async fn ai_local_download(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AiLocalStatus> {
    let emitter = app.clone();
    state
        .ai
        .local_download(move |progress| {
            let _ = emitter.emit("ai::model_progress", progress);
        })
        .await
}

#[tauri::command]
pub fn ai_local_delete(state: State<'_, AppState>) -> Result<AiLocalStatus> {
    state.ai.local_delete()
}

#[tauri::command]
pub async fn ai_activate_local(state: State<'_, AppState>) -> Result<AiConfig> {
    #[cfg(target_os = "android")]
    {
        let mut config = state.ai.config().await?;
        config.provider = "local".to_string();
        config.model = crate::ai::local::MODEL_ID.to_string();
        state.ai.save_config(config).await
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = state;
        Err(KoraError::Unsupported(
            "el asistente local solo está disponible en Android".into(),
        ))
    }
}

#[tauri::command]
pub async fn ai_deactivate_local(state: State<'_, AppState>) -> Result<AiConfig> {
    let mut config = state.ai.config().await?;
    config.provider = "off".to_string();
    config.model.clear();
    state.ai.save_config(config).await
}
