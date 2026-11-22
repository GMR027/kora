use tauri::{AppHandle, State};

use crate::error::Result;
use crate::features::pomodoro::{PomodoroConfig, PomodoroStatus};
use crate::state::AppState;

#[tauri::command]
pub async fn get_pomodoro(state: State<'_, AppState>) -> Result<PomodoroStatus> {
    Ok(state.pomodoro.status().await)
}

#[tauri::command]
pub async fn start_pomodoro(
    state: State<'_, AppState>,
    app: AppHandle,
    config: Option<PomodoroConfig>,
) -> Result<PomodoroStatus> {
    state.pomodoro.start(Some(&app), config).await
}

#[tauri::command]
pub async fn stop_pomodoro(state: State<'_, AppState>, app: AppHandle) -> Result<PomodoroStatus> {
    state.pomodoro.stop(Some(&app)).await
}
