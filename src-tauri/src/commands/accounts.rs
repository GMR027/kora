use tauri::State;

use crate::accounts::AccountStatus;
use crate::error::Result;
use crate::state::AppState;

#[tauri::command]
pub async fn account_status(state: State<'_, AppState>) -> Result<AccountStatus> {
    state.account_status().await
}
