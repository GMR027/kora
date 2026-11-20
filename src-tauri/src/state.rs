use tauri::{AppHandle, Runtime};

use crate::accounts::AccountStatus;
use crate::error::Result;
use crate::reader::ReaderService;
use crate::storage::{LocalIdentity, Storage, StorageLayout};
use crate::sync::SyncService;
use crate::tts::TtsService;

/// Estado compartido por todos los comandos. Solo contiene infraestructura:
/// la lógica de negocio vive en los módulos de storage, reader, tts y sync.
pub struct AppState {
    pub layout: StorageLayout,
    pub storage: Storage,
    pub identity: LocalIdentity,
    pub reader: ReaderService,
    pub tts: TtsService,
    pub sync: SyncService,
}

impl AppState {
    pub fn new<R: Runtime>(app: &AppHandle<R>) -> Result<Self> {
        let layout = StorageLayout::resolve(app)?;

        let storage = tauri::async_runtime::block_on(Storage::open(layout.clone()))?;
        let device_name =
            std::env::var("HOSTNAME").unwrap_or_else(|_| "Este dispositivo".to_string());

        let identity = tauri::async_runtime::block_on(
            storage.ensure_local_identity("Biblioteca local", &device_name),
        )?;

        Ok(Self {
            layout,
            storage,
            identity,
            reader: ReaderService::new(),
            tts: TtsService::new(),
            sync: SyncService::new(),
        })
    }

    pub async fn account_status(&self) -> Result<AccountStatus> {
        let devices = self
            .storage
            .devices
            .count_by_account(&self.identity.account.id)
            .await?;

        Ok(AccountStatus {
            signed_in: true,
            account_id: Some(self.identity.account.id.clone()),
            display_name: Some(self.identity.account.display_name.clone()),
            device_count: devices as usize,
        })
    }
}
