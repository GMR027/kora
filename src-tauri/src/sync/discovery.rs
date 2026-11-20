use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{KoraError, Result};

/// Tipo de servicio DNS-SD que anuncia Kora en la LAN.
pub const SERVICE_TYPE: &str = "_kora._udp.local.";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub instance: String,
    pub port: u16,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub id: String,
    pub name: String,
    pub address: Option<String>,
    pub port: u16,
    pub last_seen: Option<DateTime<Utc>>,
    pub trusted: bool,
}

/// Descubrimiento de pares. La implementación con `mdns-sd` se activa en la
/// fase de sincronización; en Android puede requerir NSD del sistema.
pub trait PeerDiscovery: Send + Sync {
    fn start(&self, service: ServiceInfo) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn peers(&self) -> Vec<PeerInfo>;
}

pub struct MdnsDiscovery {
    #[allow(dead_code)]
    daemon: Option<mdns_sd::ServiceDaemon>,
}

impl MdnsDiscovery {
    pub fn new() -> Self {
        Self { daemon: None }
    }
}

impl Default for MdnsDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl PeerDiscovery for MdnsDiscovery {
    fn start(&self, _service: ServiceInfo) -> Result<()> {
        Err(KoraError::not_implemented(
            "sync::MdnsDiscovery::start (fase 6)",
        ))
    }

    fn stop(&self) -> Result<()> {
        Err(KoraError::not_implemented("sync::MdnsDiscovery::stop"))
    }

    fn peers(&self) -> Vec<PeerInfo> {
        Vec::new()
    }
}
