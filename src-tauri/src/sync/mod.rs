pub mod crdt;
pub mod discovery;
pub mod transport;

pub use crdt::{CrdtEngine, DocId, YrsEngine};
pub use discovery::{MdnsDiscovery, PeerDiscovery, PeerInfo, SERVICE_TYPE, ServiceInfo};
pub use transport::{Channel, Frame, PeerConnection, QuinnTransport, Transport};

use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub enabled: bool,
    pub running: bool,
    pub peer_count: usize,
    pub last_sync: Option<DateTime<Utc>>,
}

/// Estado observable por el frontend. La orquestación real (discovery +
/// transporte + CRDT) se conecta en la fase de sincronización.
pub struct SyncService {
    status: Mutex<SyncStatus>,
}

impl SyncService {
    pub fn new() -> Self {
        Self {
            status: Mutex::new(SyncStatus::default()),
        }
    }

    pub fn status(&self) -> SyncStatus {
        self.status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default()
    }

    pub fn set_running(&self, running: bool) {
        if let Ok(mut status) = self.status.lock() {
            status.running = running;
            status.enabled = running;
            if !running {
                status.peer_count = 0;
            }
        }
    }
}

impl Default for SyncService {
    fn default() -> Self {
        Self::new()
    }
}
