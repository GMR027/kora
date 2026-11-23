use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::State;

use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::storage::AuditEntry;
use crate::storage::{Category, Device};
use crate::sync::{
    JoinPairingInput, MirrorStatus, PairedDeviceView, PairingTicketView, PeerInfo, SyncDiagnostics,
    SyncOutcome, SyncStatus,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCollectionInfo {
    pub category: Category,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCollections {
    pub device_id: String,
    pub device_name: String,
    pub sync_all: bool,
    pub categories: Vec<SyncCollectionInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDeviceInfo {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub is_local: bool,
    pub trusted: bool,
    pub last_seen_at: Option<DateTime<Utc>>,
}

#[tauri::command]
pub async fn start_sync(state: State<'_, AppState>) -> Result<SyncStatus> {
    state.sync.start().await
}

#[tauri::command]
pub async fn stop_sync(state: State<'_, AppState>) -> Result<SyncStatus> {
    state.sync.stop().await
}

#[tauri::command]
pub async fn get_sync_status(state: State<'_, AppState>) -> Result<SyncStatus> {
    Ok(state.sync.status().await)
}

#[tauri::command]
pub async fn get_peers(state: State<'_, AppState>) -> Result<Vec<PeerInfo>> {
    state.sync.peers().await
}

#[tauri::command]
pub async fn sync_now(state: State<'_, AppState>, peer_id: String) -> Result<SyncOutcome> {
    state.sync.sync_now(&peer_id).await
}

#[tauri::command]
pub async fn start_device_pairing(state: State<'_, AppState>) -> Result<PairingTicketView> {
    state.sync.start_device_pairing().await
}

#[tauri::command]
pub async fn join_device_pairing(
    state: State<'_, AppState>,
    input: JoinPairingInput,
) -> Result<PairedDeviceView> {
    state.sync.join_device_pairing(input).await
}

#[tauri::command]
pub async fn cancel_device_pairing(state: State<'_, AppState>) -> Result<SyncStatus> {
    state.sync.cancel_device_pairing().await
}

#[tauri::command]
pub async fn set_auto_sync(state: State<'_, AppState>, enabled: bool) -> Result<SyncStatus> {
    state.sync.set_auto_sync(enabled).await
}

#[tauri::command]
pub async fn sync_now_all(state: State<'_, AppState>) -> Result<Vec<SyncOutcome>> {
    state.sync.sync_now_all().await
}

#[tauri::command]
pub async fn get_sync_diagnostics(state: State<'_, AppState>) -> Result<SyncDiagnostics> {
    Ok(state.sync.diagnostics().await)
}

#[tauri::command]
pub async fn start_share(state: State<'_, AppState>) -> Result<SyncStatus> {
    state.sync.start_share().await
}

#[tauri::command]
pub async fn stop_share(state: State<'_, AppState>) -> Result<SyncStatus> {
    state.sync.stop_share().await
}

#[tauri::command]
pub async fn revoke_peer(state: State<'_, AppState>, device_id: String) -> Result<()> {
    state.sync.revoke_peer(&device_id).await
}

#[tauri::command]
pub async fn get_sync_collections(
    state: State<'_, AppState>,
    device_id: Option<String>,
) -> Result<SyncCollections> {
    let owner = state.identity.account.id.clone();
    let device = resolve_device(&state, device_id.as_deref()).await?;
    let categories = state.storage.categories.list_by_owner(&owner).await?;
    let selected = state
        .storage
        .sync_collections
        .enabled_category_ids(&owner, &device.id)
        .await?;
    let mode = state.storage.settings.get(&mode_key(&device.id)).await?;
    let sync_all = match mode.as_deref() {
        Some("selected") => false,
        Some("all") => true,
        _ => {
            !state
                .storage
                .sync_collections
                .has_selection(&owner, &device.id)
                .await?
        }
    };

    Ok(SyncCollections {
        device_id: device.id,
        device_name: device.name,
        sync_all,
        categories: categories
            .into_iter()
            .map(|category| SyncCollectionInfo {
                enabled: sync_all || selected.contains(&category.id),
                category,
            })
            .collect(),
    })
}

#[tauri::command]
pub async fn set_sync_categories(
    state: State<'_, AppState>,
    device_id: Option<String>,
    category_ids: Vec<String>,
    sync_all: bool,
) -> Result<SyncCollections> {
    let owner = state.identity.account.id.clone();
    let device = resolve_device(&state, device_id.as_deref()).await?;
    state
        .storage
        .settings
        .set(
            &mode_key(&device.id),
            if sync_all { "all" } else { "selected" },
        )
        .await?;

    if sync_all {
        state
            .storage
            .sync_collections
            .replace(&owner, &device.id, &[])
            .await?;
    } else {
        state
            .storage
            .sync_collections
            .replace(&owner, &device.id, &category_ids)
            .await?;
    }

    get_sync_collections(state, Some(device.id)).await
}

#[tauri::command]
pub async fn list_sync_devices(state: State<'_, AppState>) -> Result<Vec<SyncDeviceInfo>> {
    let owner = state.identity.account.id.clone();
    let mut devices = state
        .storage
        .devices
        .list_by_account(&owner)
        .await?
        .into_iter()
        .filter(|device| device.is_local || device.trusted)
        .map(device_info)
        .collect::<Vec<_>>();
    devices.sort_by(|left, right| {
        right
            .is_local
            .cmp(&left.is_local)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(devices)
}

/// Alias de compatibilidad: la selección pasa a ser siempre por dispositivo.
#[tauri::command]
pub async fn set_sync_collections(
    state: State<'_, AppState>,
    category_ids: Vec<String>,
    sync_all: bool,
) -> Result<SyncCollections> {
    set_sync_categories(state, None, category_ids, sync_all).await
}

fn mode_key(device_id: &str) -> String {
    format!("sync.collections.mode.{device_id}")
}

async fn resolve_device(state: &AppState, device_id: Option<&str>) -> Result<Device> {
    let owner = &state.identity.account.id;
    let device = match device_id {
        Some(device_id) => state.storage.devices.get(device_id).await?,
        None => state.storage.devices.find_local(owner).await?,
    }
    .ok_or_else(|| KoraError::NotFound("dispositivo de sincronización".into()))?;

    if device.account_id.as_str() != owner.as_str() {
        return Err(KoraError::NotFound(format!("dispositivo {}", device.id)));
    }
    Ok(device)
}

fn device_info(device: Device) -> SyncDeviceInfo {
    SyncDeviceInfo {
        id: device.id,
        name: device.name,
        platform: device.platform,
        is_local: device.is_local,
        trusted: device.trusted,
        last_seen_at: device.last_seen_at,
    }
}

#[tauri::command]
pub async fn start_mirror(state: State<'_, AppState>, book_id: String) -> Result<MirrorStatus> {
    state.sync.start_mirror(&book_id).await
}

#[tauri::command]
pub async fn stop_mirror(state: State<'_, AppState>) -> Result<MirrorStatus> {
    state.sync.stop_mirror().await
}

#[tauri::command]
pub async fn join_mirror(
    state: State<'_, AppState>,
    peer_id: String,
    book_id: String,
) -> Result<MirrorStatus> {
    state.sync.join_mirror(&peer_id, &book_id).await
}

#[tauri::command]
pub async fn leave_mirror(state: State<'_, AppState>) -> Result<MirrorStatus> {
    state.sync.leave_mirror().await
}

#[tauri::command]
pub async fn get_mirror_status(state: State<'_, AppState>) -> Result<MirrorStatus> {
    Ok(state.sync.mirror_status().await)
}

// Alias de compatibilidad con la fase 1.
#[tauri::command]
pub async fn sync_status(state: State<'_, AppState>) -> Result<SyncStatus> {
    Ok(state.sync.status().await)
}

#[tauri::command]
pub async fn sync_peers(state: State<'_, AppState>) -> Result<Vec<PeerInfo>> {
    state.sync.peers().await
}

#[tauri::command]
pub async fn sync_start(state: State<'_, AppState>) -> Result<SyncStatus> {
    state.sync.start().await
}

#[tauri::command]
pub async fn list_audit_log(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<AuditEntry>> {
    state
        .storage
        .audit
        .list(limit.unwrap_or(50).clamp(1, 500))
        .await
}

#[tauri::command]
pub async fn export_audit_log(
    state: State<'_, AppState>,
    destination: String,
    limit: Option<i64>,
) -> Result<String> {
    let entries = state
        .storage
        .audit
        .list(limit.unwrap_or(500).clamp(1, 5000))
        .await?;
    let path = std::path::PathBuf::from(destination.trim());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let payload = serde_json::json!({
        "exportedAt": chrono::Utc::now(),
        "entries": entries,
    });
    std::fs::write(&path, serde_json::to_vec_pretty(&payload)?)?;
    Ok(path.to_string_lossy().to_string())
}
