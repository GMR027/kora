use tauri::State;

use crate::ai::{AiConfig, AiMessage, AiReply, AiStatus};
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
