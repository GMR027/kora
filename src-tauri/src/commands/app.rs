use serde::Serialize;
use tauri::{AppHandle, State};

use crate::error::Result;
use crate::state::AppState;
use crate::storage::StorageLayout;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub identifier: String,
    pub platform: String,
    pub arch: String,
    pub storage: StorageLayout,
}

#[tauri::command]
pub fn app_info(app: AppHandle, state: State<'_, AppState>) -> Result<AppInfo> {
    let package = app.package_info();

    Ok(AppInfo {
        name: package.name.clone(),
        version: package.version.to_string(),
        identifier: app.config().identifier.clone(),
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        storage: state.layout.clone(),
    })
}

#[tauri::command]
pub fn storage_layout(state: State<'_, AppState>) -> Result<StorageLayout> {
    Ok(state.layout.clone())
}
