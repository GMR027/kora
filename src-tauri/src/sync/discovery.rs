use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use chrono::{DateTime, Utc};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use serde::{Deserialize, Serialize};

use crate::error::{KoraError, Result};

/// Tipo de servicio DNS-SD que anuncia Kora en la LAN.
pub const SERVICE_TYPE: &str = "_kora._udp.local.";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub instance: String,
    pub name: String,
    pub port: u16,
    pub fingerprint: String,
    pub device_id: String,
    pub account_hint: String,
    pub sas_prefix: String,
    pub purpose: String,
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
    pub account_hint: String,
    pub purpose: String,
    pub sas_prefix: String,
    pub fingerprint: String,
    pub fullname: String,
    pub protocol_version: u32,
}

/// Descubrimiento mDNS. La navegación es pasiva y solo se registra el
/// servicio mientras la pantalla de emparejar/compartir está abierta.
pub struct MdnsDiscovery {
    daemon: Mutex<Option<ServiceDaemon>>,
    peers: Arc<RwLock<HashMap<String, PeerInfo>>>,
    advertised: Mutex<Option<String>>,
}

impl MdnsDiscovery {
    pub fn new() -> Self {
        Self {
            daemon: Mutex::new(None),
            peers: Arc::new(RwLock::new(HashMap::new())),
            advertised: Mutex::new(None),
        }
    }

    pub fn start_browse(&self) -> Result<()> {
        let mut slot = self
            .daemon
            .lock()
            .map_err(|_| KoraError::Internal("mDNS bloqueado".into()))?;
        if slot.is_some() {
            return Ok(());
        }

        let daemon = ServiceDaemon::new()
            .map_err(|error| KoraError::Unsupported(format!("mDNS no disponible: {error}")))?;
        let receiver = daemon
            .browse(SERVICE_TYPE)
            .map_err(|error| KoraError::Unsupported(format!("mDNS browse: {error}")))?;
        let peers = self.peers.clone();

        std::thread::Builder::new()
            .name("kora-mdns".into())
            .spawn(move || {
                while let Ok(event) = receiver.recv() {
                    match event {
                        ServiceEvent::ServiceResolved(info) => {
                            if let Some(peer) = peer_from_service(&info)
                                && let Ok(mut peers) = peers.write()
                            {
                                peers.insert(peer.id.clone(), peer);
                            }
                        }
                        ServiceEvent::ServiceRemoved(_, fullname) => {
                            if let Ok(mut peers) = peers.write() {
                                peers.retain(|_, peer| peer.fullname != fullname);
                            }
                        }
                        ServiceEvent::SearchStopped(_) => break,
                        _ => {}
                    }
                }
            })
            .map_err(|error| KoraError::Internal(format!("hilo mDNS: {error}")))?;

        *slot = Some(daemon);
        Ok(())
    }

    pub fn advertise(&self, service: &ServiceInfo) -> Result<()> {
        self.start_browse()?;
        self.stop_advertise()?;

        let daemon = self
            .daemon
            .lock()
            .map_err(|_| KoraError::Internal("mDNS bloqueado".into()))?
            .clone()
            .ok_or_else(|| KoraError::Internal("mDNS no iniciado".into()))?;

        let host = format!("{}.local.", service.instance.to_lowercase());
        let properties = [
            ("v".to_string(), "1".to_string()),
            ("purpose".to_string(), service.purpose.clone()),
            ("account_hint".to_string(), service.account_hint.clone()),
            ("sas_prefix".to_string(), service.sas_prefix.clone()),
            ("device_id".to_string(), service.device_id.clone()),
            ("fingerprint".to_string(), service.fingerprint.clone()),
            ("name".to_string(), service.name.clone()),
        ];
        let info = mdns_sd::ServiceInfo::new(
            SERVICE_TYPE,
            &service.instance,
            &host,
            (),
            service.port,
            &properties[..],
        )
        .map_err(|error| KoraError::InvalidInput(format!("servicio mDNS: {error}")))?;

        let fullname = info.get_fullname().to_string();
        daemon
            .register(info)
            .map_err(|error| KoraError::Unsupported(format!("mDNS register: {error}")))?;
        *self
            .advertised
            .lock()
            .map_err(|_| KoraError::Internal("mDNS bloqueado".into()))? = Some(fullname);
        Ok(())
    }

    pub fn stop_advertise(&self) -> Result<()> {
        let fullname = self
            .advertised
            .lock()
            .map_err(|_| KoraError::Internal("mDNS bloqueado".into()))?
            .take();
        if let Some(fullname) = fullname
            && let Some(daemon) = self
                .daemon
                .lock()
                .map_err(|_| KoraError::Internal("mDNS bloqueado".into()))?
                .clone()
        {
            let _ = daemon.unregister(&fullname);
        }
        Ok(())
    }

    pub fn peers(&self) -> Vec<PeerInfo> {
        self.peers
            .read()
            .map(|peers| peers.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn stop(&self) -> Result<()> {
        self.stop_advertise()?;
        let daemon = self
            .daemon
            .lock()
            .map_err(|_| KoraError::Internal("mDNS bloqueado".into()))?
            .take();
        if let Some(daemon) = daemon {
            let _ = daemon.shutdown();
        }
        if let Ok(mut peers) = self.peers.write() {
            peers.clear();
        }
        Ok(())
    }
}

impl Default for MdnsDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

fn peer_from_service(info: &mdns_sd::ResolvedService) -> Option<PeerInfo> {
    let device_id = info.get_property_val_str("device_id")?.to_string();
    let address = info
        .addresses
        .iter()
        .find(|address| !address.is_loopback())
        .or_else(|| info.addresses.iter().next())
        .map(|address| address.to_ip_addr().to_string());
    let version = info
        .get_property_val_str("v")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    Some(PeerInfo {
        id: device_id,
        name: info
            .get_property_val_str("name")
            .unwrap_or_else(|| info.get_hostname())
            .to_string(),
        address,
        port: info.port,
        last_seen: Some(Utc::now()),
        trusted: false,
        account_hint: info
            .get_property_val_str("account_hint")
            .unwrap_or_default()
            .to_string(),
        purpose: info
            .get_property_val_str("purpose")
            .unwrap_or_default()
            .to_string(),
        sas_prefix: info
            .get_property_val_str("sas_prefix")
            .unwrap_or_default()
            .to_string(),
        fingerprint: info
            .get_property_val_str("fingerprint")
            .unwrap_or_default()
            .to_string(),
        fullname: info.fullname.clone(),
        protocol_version: version,
    })
}
