pub mod crdt;
pub mod discovery;
pub mod identity;
pub mod manifest;
pub mod mirror;
pub mod protocol;
pub mod service;
pub mod transport;

pub use crdt::CrdtStore;
pub use discovery::{MdnsDiscovery, PeerInfo, SERVICE_TYPE, ServiceInfo};
pub use identity::{DeviceIdentity, sync_group_id};
pub use mirror::{MirrorEvent, MirrorHub, MirrorStatus};
pub use protocol::{
    Handoff, Hello, Manifest, Message, MirrorTtsState, Purpose, ResourceRef, SyncResource,
};
pub use service::{
    EventSink, JoinPairingInput, NullEventSink, PairedDeviceView, PairingState, PairingTicketView,
    SyncDiagnostics, SyncOutcome, SyncProgress, SyncService, TauriEventSink,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub enabled: bool,
    pub running: bool,
    pub peer_count: usize,
    pub listen_port: u16,
    pub trusted_peers: usize,
    pub pending_changes: i64,
    pub last_sync: Option<DateTime<Utc>>,
    pub pairing: bool,
    pub sharing: bool,
    pub mirror_active: bool,
    pub mirror_hosting: bool,
    pub syncing: bool,
    pub auto_sync: bool,
    pub online_peers: usize,
    pub last_error: Option<String>,
}
