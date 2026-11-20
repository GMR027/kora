use tauri::State;

use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::sync::{PeerInfo, SyncStatus};

#[tauri::command]
pub fn sync_status(state: State<'_, AppState>) -> Result<SyncStatus> {
    Ok(state.sync.status())
}

#[tauri::command]
pub fn sync_peers() -> Result<Vec<PeerInfo>> {
    // mDNS todavía no está activo; el contrato ya existe.
    Ok(Vec::new())
}

#[tauri::command]
pub fn sync_start(state: State<'_, AppState>) -> Result<SyncStatus> {
    let _ = &state;
    Err(KoraError::not_implemented("sync_start (fase 6)"))
}
