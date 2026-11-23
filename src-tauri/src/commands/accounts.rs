use serde::Serialize;
use tauri::State;

use crate::accounts::{
    AccountService, AccountView, ConnectionKeyView, ShareCreateRequest, ShareView,
};
use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::storage::{Device, ReceivedShare, SharePermission};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountCreated {
    pub account: AccountView,
    pub connection: ConnectionKeyView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResetInfo {
    pub requires_restart: bool,
}

#[tauri::command]
pub async fn create_account(
    state: State<'_, AppState>,
    username: String,
    display_name: String,
    pin: String,
) -> Result<AccountCreated> {
    let connection = state
        .accounts
        .create_account(&username, &display_name, &pin)
        .await?;
    state.sync.refresh_account_cache().await;
    let account = state
        .accounts
        .get_account()
        .await?
        .ok_or_else(|| KoraError::Internal("cuenta creada no encontrada".into()))?;
    Ok(AccountCreated {
        account,
        connection,
    })
}

#[tauri::command]
pub async fn get_account(state: State<'_, AppState>) -> Result<Option<AccountView>> {
    state.accounts.get_account().await
}

#[tauri::command]
pub async fn update_display_name(
    state: State<'_, AppState>,
    display_name: String,
) -> Result<crate::accounts::AccountView> {
    state.accounts.update_display_name(&display_name).await
}

#[tauri::command]
pub async fn update_account_avatar(
    state: State<'_, AppState>,
    image: String,
) -> Result<AccountView> {
    state.accounts.update_avatar(&image).await
}

#[tauri::command]
pub async fn remove_account_avatar(state: State<'_, AppState>) -> Result<AccountView> {
    state.accounts.remove_avatar().await
}

#[tauri::command]
pub async fn get_connection_key(
    state: State<'_, AppState>,
    pin: String,
) -> Result<ConnectionKeyView> {
    state.accounts.get_connection_key(&pin).await
}

#[tauri::command]
pub async fn rotate_connection_key(
    state: State<'_, AppState>,
    pin: String,
) -> Result<ConnectionKeyView> {
    let view = state.accounts.rotate_connection_key(&pin).await?;
    state.sync.refresh_account_cache().await;
    state.sync.cancel_pairing().await;
    Ok(view)
}

/// Recupera la cuenta sin el PIN anterior: regenera la identidad y la clave
/// de conexión. La biblioteca se conserva, pero los dispositivos deben
/// volver a emparejarse.
#[tauri::command]
pub async fn recover_account_pin(
    state: State<'_, AppState>,
    new_pin: String,
) -> Result<ConnectionKeyView> {
    let view = state.accounts.recover_pin(&new_pin).await?;
    state.sync.refresh_account_cache().await;
    state.sync.cancel_pairing().await;
    Ok(view)
}

/// Prepara el borrado completo de la cuenta. Se aplica al reiniciar Kora,
/// antes de abrir la base de datos.
#[tauri::command]
pub async fn delete_account(state: State<'_, AppState>, pin: String) -> Result<AccountResetInfo> {
    state.accounts.unlock(&pin).await?;
    state.sync.cancel_pairing().await;
    std::fs::write(state.layout.root.join("account-reset.pending"), b"1")?;
    Ok(AccountResetInfo {
        requires_restart: true,
    })
}

#[tauri::command]
pub async fn change_pin(
    state: State<'_, AppState>,
    old_pin: String,
    new_pin: String,
) -> Result<()> {
    state.accounts.change_pin(&old_pin, &new_pin).await?;
    state.sync.refresh_account_cache().await;
    Ok(())
}

#[tauri::command]
pub async fn create_share(
    state: State<'_, AppState>,
    pin: String,
    request: ShareCreateRequest,
) -> Result<ShareView> {
    state.accounts.create_share(&pin, &request).await
}

#[tauri::command]
pub async fn create_audiobook_share(
    state: State<'_, AppState>,
    pin: String,
    audiobook_id: String,
    permission: SharePermission,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    max_uses: Option<i64>,
) -> Result<ShareView> {
    state
        .accounts
        .create_audiobook_share(&pin, &audiobook_id, permission, expires_at, max_uses)
        .await
}

#[tauri::command]
pub async fn list_shares(state: State<'_, AppState>) -> Result<Vec<ShareView>> {
    state.accounts.list_shares().await
}

#[tauri::command]
pub async fn reveal_share_key(
    state: State<'_, AppState>,
    pin: String,
    share_id: String,
) -> Result<ShareView> {
    state.accounts.reveal_share_key(&pin, &share_id).await
}

#[tauri::command]
pub async fn revoke_share(state: State<'_, AppState>, share_id: String) -> Result<()> {
    state.accounts.revoke_share(&share_id).await
}

#[tauri::command]
pub async fn accept_share(
    state: State<'_, AppState>,
    pin: String,
    key: String,
    peer_id: String,
) -> Result<ReceivedShare> {
    state.sync.accept_share(&pin, &key, &peer_id).await
}

#[tauri::command]
pub async fn remove_received_share(state: State<'_, AppState>, share_id: String) -> Result<()> {
    state.sync.remove_received_share(&share_id).await
}

#[tauri::command]
pub async fn list_received_shares(state: State<'_, AppState>) -> Result<Vec<ReceivedShare>> {
    state.storage.received_shares.list().await
}

#[tauri::command]
pub async fn list_devices(state: State<'_, AppState>) -> Result<Vec<Device>> {
    state
        .storage
        .devices
        .list_by_account(&state.identity.account.id)
        .await
}

#[tauri::command]
pub async fn revoke_device(state: State<'_, AppState>, device_id: String) -> Result<()> {
    let device = state
        .storage
        .devices
        .get(&device_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("dispositivo {device_id}")))?;
    if device.account_id != state.identity.account.id || device.is_local {
        return Err(KoraError::NotFound(format!("dispositivo {device_id}")));
    }
    state.storage.devices.revoke(&device_id).await?;
    state
        .accounts
        .audit("device.revoke", None, Some(&device_id))
        .await?;
    Ok(())
}

#[allow(dead_code)]
fn _keep(_: &AccountService) {}

#[tauri::command]
pub async fn account_status(state: State<'_, AppState>) -> Result<crate::accounts::AccountStatus> {
    state.account_status().await
}
