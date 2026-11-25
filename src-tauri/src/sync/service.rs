use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::time::Instant;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{Mutex, Notify, RwLock};
use tokio::task::JoinHandle;

use super::mirror::MirrorHub;
use crate::accounts::AccountService;
use crate::error::{KoraError, Result};
use crate::storage::{
    LocalIdentity, NewReceivedShare, ReceivedShare, ResourceType, SharePermission, ShareScopeType,
    Storage, StorageLayout, SyncState,
};

use super::SyncStatus;
use super::crdt::CrdtStore;
use super::discovery::{MdnsDiscovery, PeerInfo, ServiceInfo, ipv4_rank};
use super::identity::{DeviceIdentity, sync_group_id};
use super::manifest;
use super::mirror::{MirrorEvent, MirrorStatus};
use super::protocol::{
    Handoff, Hello, Manifest, Message, MirrorTtsState, PROTOCOL_VERSION, Purpose, ResourceRef,
    SyncResource, read_frame, write_frame,
};
use super::transport;
use crate::accounts::crypto;
use sha2::{Digest, Sha256};
use spake2::{Ed25519Group, Identity, Password, Spake2};

/// Asset recibido en un archivo temporal. Los audiolibros pueden pesar más de
/// 1 GB, así que no se mantienen en memoria durante la transferencia; al
/// aplicar el recurso se copian al almacén definitivo y se borra el temporal.
struct ReceivedAsset {
    path: std::path::PathBuf,
    hash: String,
}

impl Drop for ReceivedAsset {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

struct PendingAsset {
    hash: String,
    size: u64,
    written: u64,
    path: std::path::PathBuf,
    file: Option<tokio::fs::File>,
    verified: bool,
}

impl Drop for PendingAsset {
    fn drop(&mut self) {
        if !self.verified {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

struct IncomingResource {
    resource: ResourceRef,
    payload: Value,
    assets: HashMap<String, ReceivedAsset>,
    pending: HashMap<String, PendingAsset>,
}

impl Drop for IncomingResource {
    fn drop(&mut self) {
        // Los assets ya aplicados se copiaron al almacén; los temporales se
        // eliminan tanto al terminar como si la conexión se corta.
        self.assets.clear();
    }
}

fn received_bytes(assets: &HashMap<String, ReceivedAsset>, name: &str) -> Result<Option<Vec<u8>>> {
    match assets.get(name) {
        Some(asset) => Ok(Some(std::fs::read(&asset.path)?)),
        None => Ok(None),
    }
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: Value);
}

pub struct TauriEventSink<R: tauri::Runtime = tauri::Wry> {
    app: tauri::AppHandle<R>,
}

impl<R: tauri::Runtime> TauriEventSink<R> {
    pub fn new(app: tauri::AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: tauri::Runtime> EventSink for TauriEventSink<R> {
    fn emit(&self, event: &str, payload: Value) {
        use tauri::Emitter;
        let _ = self.app.emit(event, payload);
    }
}

#[derive(Default)]
pub struct NullEventSink;

impl EventSink for NullEventSink {
    fn emit(&self, _event: &str, _payload: Value) {}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOutcome {
    pub peer_id: String,
    pub sent: usize,
    pub received: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingTicketView {
    pub code: String,
    pub qr_svg: String,
    pub uri: String,
    pub expires_at: i64,
    pub addresses: Vec<String>,
    pub port: u16,
    pub device_name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinPairingInput {
    #[serde(default)]
    pub peer_id: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedDeviceView {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub fingerprint: String,
    pub account_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDiagnostics {
    pub mdns_active: bool,
    pub listen_port: u16,
    pub local_addresses: Vec<String>,
    pub auto_sync: bool,
    pub trusted_peers: usize,
    pub online_peers: usize,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub peer_id: String,
    pub direction: String,
    pub resource_kind: String,
    pub current: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingState {
    pub state: String,
    pub message: String,
    pub peer_name: Option<String>,
}

#[derive(Debug, Clone)]
struct PairingTicket {
    id: String,
    code: String,
    expires_at: i64,
    attempts: u32,
}

impl PairingTicket {
    fn expired(&self) -> bool {
        Utc::now().timestamp() > self.expires_at
    }
}

struct MirrorPeer {
    book_id: String,
    send: Arc<Mutex<quinn::SendStream>>,
    _connection: quinn::Connection,
}

pub struct SyncService {
    inner: Arc<Inner>,
}

struct Inner {
    storage: Storage,
    identity: DeviceIdentity,
    owner_account_id: String,
    account_hint: String,
    crdt: CrdtStore,
    events: Arc<dyn EventSink>,
    mdns: Arc<MdnsDiscovery>,
    mirror: MirrorHub,
    accounts: Arc<AccountService>,
    account_username: std::sync::RwLock<Option<String>>,
    account_public_key: std::sync::RwLock<Option<String>>,
    pairing_ticket: Mutex<Option<PairingTicket>>,
    status: RwLock<SyncStatus>,
    endpoint: Mutex<Option<quinn::Endpoint>>,
    listener: Mutex<Option<JoinHandle<()>>>,
    sync_lock: Mutex<()>,
    auto_sync: AtomicBool,
    auto_sync_notify: Notify,
    auto_sync_task: Mutex<Option<JoinHandle<()>>>,
    syncing: AtomicBool,
    last_error: RwLock<Option<String>>,
    mirror_bus_task: Mutex<Option<JoinHandle<()>>>,
    mirror_sessions: Mutex<HashMap<String, MirrorPeer>>,
    mirror_connection: Mutex<Option<quinn::Connection>>,
    mirror_send: Mutex<Option<quinn::SendStream>>,
    mirror_task: Mutex<Option<JoinHandle<()>>>,
    mirror_status: RwLock<MirrorStatus>,
    handoffs: Mutex<HashMap<String, Handoff>>,
    listen_port: AtomicU16,
    pairing_mode: AtomicBool,
    share_mode: AtomicBool,
    last_sync: RwLock<Option<DateTime<Utc>>>,
}

impl SyncService {
    pub async fn new(
        storage: Storage,
        identity: &LocalIdentity,
        layout: StorageLayout,
        events: Arc<dyn EventSink>,
        mirror: MirrorHub,
        accounts: Arc<AccountService>,
    ) -> Result<Arc<Self>> {
        let device = DeviceIdentity::load_or_create(&storage, identity, &layout).await?;
        let group_id = sync_group_id(&storage).await?;
        let account_hint = DeviceIdentity::account_hint(&group_id);
        let client_id = {
            let bytes = crate::storage::hash::sha256_bytes(identity.device.id.as_bytes());
            u64::from_str_radix(&bytes[..16], 16).unwrap_or(1)
        };
        let crdt = CrdtStore::new(storage.clone(), client_id, identity.account.id.clone());
        let local_account = storage.accounts.find_local().await?;
        let account_username = local_account
            .as_ref()
            .and_then(|account| account.username.clone());
        let account_public_key = local_account
            .as_ref()
            .and_then(|account| account.public_key.clone());
        let auto_sync = storage
            .settings
            .get("sync.auto")
            .await?
            .map(|value| value != "false")
            .unwrap_or(true);

        Ok(Arc::new(Self {
            inner: Arc::new(Inner {
                storage,
                identity: device,
                owner_account_id: identity.account.id.clone(),
                account_hint,
                crdt,
                events,
                mdns: Arc::new(MdnsDiscovery::new()),
                mirror,
                accounts,
                account_username: std::sync::RwLock::new(account_username),
                account_public_key: std::sync::RwLock::new(account_public_key),
                pairing_ticket: Mutex::new(None),
                status: RwLock::new(SyncStatus::default()),
                endpoint: Mutex::new(None),
                listener: Mutex::new(None),
                sync_lock: Mutex::new(()),
                auto_sync: AtomicBool::new(auto_sync),
                auto_sync_notify: Notify::new(),
                auto_sync_task: Mutex::new(None),
                syncing: AtomicBool::new(false),
                last_error: RwLock::new(None),
                mirror_bus_task: Mutex::new(None),
                mirror_sessions: Mutex::new(HashMap::new()),
                mirror_connection: Mutex::new(None),
                mirror_send: Mutex::new(None),
                mirror_task: Mutex::new(None),
                mirror_status: RwLock::new(MirrorStatus::default()),
                handoffs: Mutex::new(HashMap::new()),
                listen_port: AtomicU16::new(0),
                pairing_mode: AtomicBool::new(false),
                share_mode: AtomicBool::new(false),
                last_sync: RwLock::new(None),
            }),
        }))
    }

    pub async fn status(&self) -> SyncStatus {
        self.inner.refresh_status().await
    }

    pub fn listen_port(&self) -> u16 {
        self.inner.listen_port.load(Ordering::SeqCst)
    }

    pub fn device_id(&self) -> &str {
        &self.inner.identity.device_id
    }

    pub fn fingerprint(&self) -> &str {
        &self.inner.identity.fingerprint
    }

    pub fn public_key(&self) -> &str {
        &self.inner.identity.public_key
    }

    pub async fn start(&self) -> Result<SyncStatus> {
        self.ensure_endpoint().await?;
        if let Err(error) = self.inner.mdns.start_browse() {
            tracing::warn!(%error, "mDNS no disponible; sync seguirá por dirección conocida");
        }
        {
            let mut status = self.inner.status.write().await;
            status.running = true;
            status.enabled = true;
        }
        if self.inner.mirror_bus_task.lock().await.is_none() {
            let inner = self.inner.clone();
            let mut receiver = inner.mirror.subscribe();
            let task = tokio::spawn(async move {
                while let Ok(event) = receiver.recv().await {
                    inner.broadcast_mirror(event).await;
                }
            });
            *self.inner.mirror_bus_task.lock().await = Some(task);
        }
        if !self.inner.pairing_mode.load(Ordering::SeqCst)
            && !self.inner.share_mode.load(Ordering::SeqCst)
        {
            let _ = self.inner.advertise_presence();
        }
        self.ensure_auto_sync_task().await;
        let status = self.inner.refresh_status().await;
        self.inner.emit_status(&status);
        Ok(status)
    }

    pub async fn bootstrap(&self) -> Result<()> {
        if !self.inner.auto_sync.load(Ordering::SeqCst) {
            return Ok(());
        }
        let trusted = self
            .inner
            .storage
            .devices
            .list_trusted(&self.inner.owner_account_id)
            .await?;
        if trusted.is_empty() {
            return Ok(());
        }
        self.start().await?;
        Ok(())
    }

    pub async fn stop(&self) -> Result<SyncStatus> {
        if let Some(task) = self.inner.listener.lock().await.take() {
            task.abort();
        }
        if let Some(task) = self.inner.auto_sync_task.lock().await.take() {
            task.abort();
        }
        if let Some(task) = self.inner.mirror_bus_task.lock().await.take() {
            task.abort();
        }
        if let Some(task) = self.inner.mirror_task.lock().await.take() {
            task.abort();
        }
        if let Some(endpoint) = self.inner.endpoint.lock().await.take() {
            endpoint.close(0u32.into(), b"sync stopped");
        }
        self.inner.mdns.stop()?;
        self.inner.pairing_mode.store(false, Ordering::SeqCst);
        self.inner.share_mode.store(false, Ordering::SeqCst);
        *self.inner.pairing_ticket.lock().await = None;
        self.inner.mirror_sessions.lock().await.clear();
        *self.inner.mirror_connection.lock().await = None;
        *self.inner.mirror_send.lock().await = None;
        *self.inner.mirror_status.write().await = MirrorStatus::default();
        let mut status = self.inner.status.write().await;
        status.running = false;
        status.enabled = false;
        status.peer_count = 0;
        status.pairing = false;
        status.sharing = false;
        status.syncing = false;
        status.mirror_active = false;
        status.mirror_hosting = false;
        drop(status);
        let status = self.inner.refresh_status().await;
        self.inner.emit_status(&status);
        Ok(status)
    }

    pub async fn peers(&self) -> Result<Vec<PeerInfo>> {
        self.inner.peers().await
    }

    /// Muestra un código temporal y deja el dispositivo listo para recibir
    /// una conexión de vinculación. No requiere PIN ni clave de conexión.
    pub async fn start_device_pairing(&self) -> Result<PairingTicketView> {
        self.start().await?;
        let now = Utc::now();
        let created = {
            let mut guard = self.inner.pairing_ticket.lock().await;
            if let Some(ticket) = guard.as_ref()
                && !ticket.expired()
            {
                ticket.clone()
            } else {
                let ticket = PairingTicket {
                    id: uuid::Uuid::now_v7().to_string(),
                    code: crypto::random_pairing_code(),
                    expires_at: (now + Duration::minutes(2)).timestamp(),
                    attempts: 0,
                };
                *guard = Some(ticket.clone());
                ticket
            }
        };
        self.inner.pairing_mode.store(true, Ordering::SeqCst);
        self.inner.share_mode.store(false, Ordering::SeqCst);
        let port = self.inner.listen_port.load(Ordering::SeqCst);
        let addresses = self.inner.local_addresses();
        let uri = crypto::pairing_uri_v2(
            &created.id,
            &created.code,
            &addresses,
            port,
            &self.inner.identity.fingerprint,
            created.expires_at,
        );
        if let Err(error) = self.inner.advertise_pair(&created.id) {
            tracing::warn!(%error, "no se pudo anunciar el pairing por mDNS; usa el QR o la dirección manual");
        }
        {
            let mut status = self.inner.status.write().await;
            status.pairing = true;
            status.sharing = false;
        }
        self.inner.emit_pairing_state(
            "waiting",
            "Esperando a que el otro dispositivo introduzca el código",
            None,
        );
        let status = self.inner.refresh_status().await;
        self.inner.emit_status(&status);
        Ok(PairingTicketView {
            code: created.code,
            qr_svg: crypto::qr_svg(&uri)?,
            uri,
            expires_at: created.expires_at,
            addresses,
            port,
            device_name: self.inner.identity.name.clone(),
        })
    }

    pub async fn cancel_device_pairing(&self) -> Result<SyncStatus> {
        *self.inner.pairing_ticket.lock().await = None;
        self.inner.pairing_mode.store(false, Ordering::SeqCst);
        self.inner.mdns.stop_advertise()?;
        if self.inner.status.read().await.running {
            if self.inner.share_mode.load(Ordering::SeqCst) {
                let _ = self.inner.advertise_share();
            } else {
                let _ = self.inner.advertise_presence();
            }
        }
        let mut status = self.inner.status.write().await;
        status.pairing = false;
        drop(status);
        let status = self.inner.refresh_status().await;
        self.inner.emit_status(&status);
        Ok(status)
    }

    pub async fn cancel_pairing(&self) {
        let _ = self.cancel_device_pairing().await;
    }

    /// Conecta con un dispositivo que está mostrando un código. Toda la
    /// verificación (SPAKE2, SAS automático y huella TLS) se completa antes de
    /// guardar la confianza; no hay una segunda pantalla de confirmación.
    pub async fn join_device_pairing(&self, input: JoinPairingInput) -> Result<PairedDeviceView> {
        self.start().await?;
        let (ticket_id, code, hosts, port, expected_fingerprint) =
            self.inner.pairing_target(&input).await?;
        if code.len() != 6 || !code.chars().all(|character| character.is_ascii_digit()) {
            return Err(KoraError::InvalidInput(
                "el código de vinculación debe tener 6 dígitos".into(),
            ));
        }
        let candidates = self
            .inner
            .socket_candidates(&hosts, port)
            .await
            .unwrap_or_default();
        if candidates.is_empty() {
            return Err(KoraError::NotFound(
                "no se encontró la dirección del dispositivo. Escanea el QR o introduce su IP."
                    .into(),
            ));
        }
        self.inner
            .emit_pairing_state("connecting", "Conectando con el dispositivo", None);
        let endpoint = self.ensure_endpoint().await?;
        let expected = (!expected_fingerprint.is_empty()).then_some(expected_fingerprint.as_str());
        let (connection, observed, address) =
            transport::connect_any(&endpoint, &candidates, expected).await?;
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|error| KoraError::Internal(format!("stream de pairing: {error}")))?;
        let local_hello = self.inner.hello(Purpose::Pair);
        write_frame(&mut send, &Message::Hello(local_hello.clone()), &[]).await?;
        let host_hello = self.inner.read_remote_hello(&mut recv).await?;
        if host_hello.version < 2 {
            return Err(KoraError::Unsupported(
                "actualiza Kora en el otro dispositivo para vincularlo".into(),
            ));
        }
        if host_hello.device_id == self.inner.identity.device_id {
            return Err(KoraError::InvalidInput(
                "no puedes vincular este mismo dispositivo".into(),
            ));
        }
        if host_hello.fingerprint != observed {
            return Err(KoraError::Conflict(
                "el certificado del anfitrión no coincide".into(),
            ));
        }
        write_frame(
            &mut send,
            &Message::PairRequest {
                ticket_id: ticket_id.clone(),
            },
            &[],
        )
        .await?;
        // El nonce del anfitrión es público y único por conexión; sirve como
        // sal para que un intento antiguo no pueda reutilizarse.
        let password = crypto::pairing_password_v2(&code, &host_hello.nonce)?;
        let id_a = local_hello.nonce.as_bytes();
        let id_b = host_hello.nonce.as_bytes();
        let (state, message_a) = Spake2::<Ed25519Group>::start_a(
            &Password::new(password.as_slice()),
            &Identity::new(id_a),
            &Identity::new(id_b),
        );
        write_frame(
            &mut send,
            &Message::Spake2A {
                payload: hex::encode(&message_a),
            },
            &[],
        )
        .await?;
        let (message_b, host_sas) = match read_frame(&mut recv).await? {
            Some((Message::Spake2B { payload, sas }, _)) => (payload, sas),
            Some((Message::PairRejected { reason, .. }, _)) => {
                return Err(KoraError::Conflict(reason));
            }
            _ => {
                return Err(KoraError::InvalidInput(
                    "respuesta de vinculación inesperada".into(),
                ));
            }
        };
        self.inner
            .emit_pairing_state("verifying", "Comprobando el código", None);
        let message_b = hex::decode(message_b)
            .map_err(|_| KoraError::InvalidInput("SPAKE2 inválido".into()))?;
        let shared = match state.finish(&message_b) {
            Ok(shared) => shared,
            Err(_) => {
                let _ = write_frame(
                    &mut send,
                    &Message::PairRejected {
                        reason: "el código de vinculación no coincide".to_string(),
                        retryable: true,
                    },
                    &[],
                )
                .await;
                return Err(KoraError::Conflict(
                    "código incorrecto; inténtalo de nuevo".into(),
                ));
            }
        };
        let transcript =
            pairing_transcript(&host_hello, &local_hello, Some(&message_a), &message_b);
        let sas = crypto::sas_from_key(&shared, &transcript)?;
        if sas != host_sas {
            let _ = write_frame(
                &mut send,
                &Message::PairRejected {
                    reason: "el código de vinculación no coincide".to_string(),
                    retryable: true,
                },
                &[],
            )
            .await;
            return Err(KoraError::Conflict(
                "código incorrecto; inténtalo de nuevo".into(),
            ));
        }
        write_frame(&mut send, &Message::SasConfirm { sas: sas.clone() }, &[]).await?;
        match read_frame(&mut recv).await? {
            Some((Message::SasConfirm { sas: remote }, _)) if remote == sas => {}
            Some((Message::PairRejected { reason, .. }, _)) => {
                return Err(KoraError::Conflict(reason));
            }
            _ => {
                return Err(KoraError::Conflict(
                    "el otro dispositivo no confirmó la vinculación".into(),
                ));
            }
        }

        self.inner
            .store_paired_device(
                &host_hello,
                &observed,
                Some(address.ip()),
                Some(host_hello.listen_port as i64),
            )
            .await?;
        self.inner
            .accounts
            .audit("device.pair", None, Some(&host_hello.device_id))
            .await?;
        let result = PairedDeviceView {
            id: host_hello.device_id.clone(),
            name: host_hello.device_name.clone(),
            platform: host_hello.platform.clone(),
            fingerprint: observed,
            account_hint: host_hello.account_hint.clone(),
        };
        *self.inner.pairing_ticket.lock().await = None;
        self.inner.pairing_mode.store(false, Ordering::SeqCst);
        self.inner.status.write().await.pairing = false;
        let _ = self.inner.mdns.stop_advertise();
        let _ = self.inner.advertise_presence();
        self.inner
            .emit("sync::paired", serde_json::to_value(&result)?);
        self.inner
            .emit_pairing_state("paired", "Dispositivo vinculado", Some(result.name.clone()));
        let status = self.inner.refresh_status().await;
        self.inner.emit_status(&status);
        connection.close(0u32.into(), b"paired");
        self.inner.auto_sync_notify.notify_one();
        Ok(result)
    }

    pub async fn start_share(&self) -> Result<SyncStatus> {
        self.start().await?;
        self.inner.share_mode.store(true, Ordering::SeqCst);
        self.inner.pairing_mode.store(false, Ordering::SeqCst);
        *self.inner.pairing_ticket.lock().await = None;
        self.inner.advertise_share()?;
        let mut status = self.inner.status.write().await;
        status.sharing = true;
        status.pairing = false;
        drop(status);
        let status = self.inner.refresh_status().await;
        self.inner.emit_status(&status);
        Ok(status)
    }

    pub async fn refresh_account_cache(&self) {
        if let Ok(Some(account)) = self.inner.storage.accounts.find_local().await {
            if let Ok(mut username) = self.inner.account_username.write() {
                *username = account.username;
            }
            if let Ok(mut public_key) = self.inner.account_public_key.write() {
                *public_key = account.public_key;
            }
        }
    }

    pub async fn accept_share(&self, pin: &str, key: &str, peer_id: &str) -> Result<ReceivedShare> {
        let unlocked = self.inner.accounts.unlock(pin).await?;
        let key = key_from_uri(key, "share").to_ascii_uppercase();
        let (scope, _) = crypto::parse_share_key(&key)
            .ok_or_else(|| KoraError::InvalidInput("clave de share inválida".into()))?;
        if scope == "voice" {
            return Err(KoraError::Unsupported(
                "compartir perfiles de voz se implementará en una fase posterior".into(),
            ));
        }
        let candidates = self.inner.peer_addresses(peer_id).await?;
        let endpoint = self.ensure_endpoint().await?;
        let (connection, observed, _) =
            transport::connect_any(&endpoint, &candidates, None).await?;
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|error| KoraError::Internal(format!("stream de share: {error}")))?;
        let local_hello = self.inner.hello(Purpose::Share);
        let key_hash = crate::storage::hash::sha256_bytes(key.as_bytes());
        write_frame(&mut send, &Message::Hello(local_hello.clone()), &[]).await?;
        write_frame(
            &mut send,
            &Message::ShareRequest {
                key_hash: key_hash.clone(),
            },
            &[],
        )
        .await?;
        let owner_hello = self.inner.read_remote_hello(&mut recv).await?;
        if owner_hello.fingerprint != observed {
            return Err(KoraError::Conflict(
                "el certificado del dueño no coincide".into(),
            ));
        }
        let password = hex::decode(&key_hash)
            .map_err(|_| KoraError::InvalidInput("hash de share inválido".into()))?;
        let id_a = local_hello.nonce.as_bytes();
        let id_b = owner_hello.nonce.as_bytes();
        let (state, message_a) = Spake2::<Ed25519Group>::start_a(
            &Password::new(password.as_slice()),
            &Identity::new(id_a),
            &Identity::new(id_b),
        );
        write_frame(
            &mut send,
            &Message::Spake2A {
                payload: hex::encode(&message_a),
            },
            &[],
        )
        .await?;
        let (message_b, owner_sas) = match read_frame(&mut recv).await? {
            Some((Message::Spake2B { payload, sas }, _)) => (payload, sas),
            Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
            _ => {
                return Err(KoraError::InvalidInput(
                    "respuesta SPAKE2 de share inesperada".into(),
                ));
            }
        };
        let message_b = hex::decode(message_b)
            .map_err(|_| KoraError::InvalidInput("SPAKE2 de share inválido".into()))?;
        let shared = state
            .finish(&message_b)
            .map_err(|_| KoraError::Conflict("la clave de share no coincide".into()))?;
        let transcript =
            pairing_transcript(&owner_hello, &local_hello, Some(&message_a), &message_b);
        let sas = crypto::sas_from_key(&shared, &transcript)?;
        if sas != owner_sas {
            return Err(KoraError::Conflict("el SAS del share no coincide".into()));
        }
        self.inner.events.emit(
            "accounts::share_sas",
            json!({ "shareKey": key, "sas": sas }),
        );

        let grant = match read_frame(&mut recv).await? {
            Some((Message::ShareGrant { payload }, _)) => payload,
            Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
            _ => return Err(KoraError::InvalidInput("share sin concesión".into())),
        };
        let share_id = grant
            .get("shareId")
            .and_then(Value::as_str)
            .ok_or_else(|| KoraError::InvalidInput("share sin id".into()))?
            .to_string();
        let permission: SharePermission = serde_json::from_value(
            grant
                .get("permission")
                .cloned()
                .ok_or_else(|| KoraError::InvalidInput("share sin permiso".into()))?,
        )?;
        let scope_type: ShareScopeType = serde_json::from_value(
            grant
                .get("scopeType")
                .cloned()
                .ok_or_else(|| KoraError::InvalidInput("share sin alcance".into()))?,
        )?;
        let resource_type: ResourceType = grant
            .get("resourceType")
            .cloned()
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or(ResourceType::Book);
        let expires_at: Option<DateTime<Utc>> =
            serde_json::from_value(grant.get("expiresAt").cloned().unwrap_or(Value::Null))?;
        let max_uses = grant.get("maxUses").and_then(Value::as_i64);
        let uses = grant.get("uses").and_then(Value::as_i64).unwrap_or(1);

        let manifest = match read_frame(&mut recv).await? {
            Some((Message::Manifest(manifest), _)) => manifest,
            Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
            _ => return Err(KoraError::InvalidInput("share sin manifiesto".into())),
        };
        let resources: Vec<ResourceRef> = manifest
            .resources
            .iter()
            .map(|resource| resource.reference.clone())
            .collect();
        write_frame(&mut send, &Message::Pull { resources }, &[]).await?;
        let received = self
            .inner
            .recv_resources(&mut send, &mut recv, &owner_hello.device_id)
            .await?;
        // Confirmar al anfitrión que todos los frames se han leído antes de
        // que cierre la conexión; si no, el cierre puede adelantar al último
        // recurso en conexiones lentas.
        if owner_hello.version >= 2 {
            write_frame(
                &mut send,
                &Message::Ack {
                    resources: Vec::new(),
                },
                &[],
            )
            .await?;
        }
        if received == 0 {
            return Err(KoraError::Unsupported(
                "el share no contenía recursos transferibles".into(),
            ));
        }

        if permission == SharePermission::Read {
            for resource in &manifest.resources {
                if resource.reference.kind != ResourceType::Book {
                    continue;
                }
                if let Ok((book, _)) = manifest::book_from_payload(&resource.payload)
                    && let Some(local) = self
                        .inner
                        .storage
                        .books
                        .find_by_hash(&self.inner.owner_account_id, &book.content_hash)
                        .await?
                {
                    self.inner
                        .storage
                        .books
                        .mark_read_only(&local.id, &share_id)
                        .await?;
                }
            }
        }

        let received_share = self
            .inner
            .storage
            .received_shares
            .put(&NewReceivedShare {
                share_id: share_id.clone(),
                owner_account_id: owner_hello.account_id.clone(),
                owner_username: Some(owner_hello.username.clone()),
                owner_public_key: Some(owner_hello.account_public_key.clone()),
                owner_account_hint: Some(owner_hello.account_hint.clone()),
                owner_device_id: Some(owner_hello.device_id.clone()),
                scope_type,
                resource_type,
                permission,
                expires_at,
                max_uses,
                uses,
                owner_signature: None,
                peer_account_signature: None,
                owner_account_signature: None,
            })
            .await?;
        self.inner
            .accounts
            .audit("share.accept", None, Some(&share_id))
            .await?;
        self.inner.events.emit(
            "accounts::share_accepted",
            serde_json::to_value(&received_share)?,
        );
        connection.close(0u32.into(), b"share done");
        let _ = unlocked;
        Ok(received_share)
    }

    pub async fn remove_received_share(&self, share_id: &str) -> Result<()> {
        let books = self
            .inner
            .storage
            .books
            .list_read_only_by_share(share_id)
            .await?;
        for book in books {
            self.inner.storage.books.delete(&book.id).await?;
        }
        self.inner.storage.received_shares.delete(share_id).await?;
        self.inner
            .accounts
            .audit("share.remove_received", None, Some(share_id))
            .await?;
        Ok(())
    }

    pub async fn stop_share(&self) -> Result<SyncStatus> {
        self.inner.share_mode.store(false, Ordering::SeqCst);
        self.inner.mdns.stop_advertise()?;
        if self.inner.status.read().await.running {
            if self.inner.pairing_mode.load(Ordering::SeqCst) {
                if let Some(ticket) = self.inner.pairing_ticket.lock().await.clone() {
                    let _ = self.inner.advertise_pair(&ticket.id);
                }
            } else {
                let _ = self.inner.advertise_presence();
            }
        }
        let mut status = self.inner.status.write().await;
        status.sharing = false;
        drop(status);
        let status = self.inner.refresh_status().await;
        self.inner.emit_status(&status);
        Ok(status)
    }

    pub async fn revoke_peer(&self, device_id: &str) -> Result<()> {
        self.inner.storage.devices.revoke_trust(device_id).await
    }

    pub async fn sync_now(&self, peer_id: &str) -> Result<SyncOutcome> {
        self.start().await?;
        let _guard = self.inner.sync_lock.lock().await;
        self.inner.sync_peer(peer_id).await
    }

    pub async fn sync_now_all(&self) -> Result<Vec<SyncOutcome>> {
        self.start().await?;
        let _guard = self.inner.sync_lock.lock().await;
        let peers = self.inner.online_trusted_ids().await?;
        let mut outcomes = Vec::new();
        for peer_id in peers {
            match self.inner.sync_peer(&peer_id).await {
                Ok(outcome) => outcomes.push(outcome),
                Err(error) => {
                    tracing::warn!(%error, peer = %peer_id, "no se pudo sincronizar con el peer");
                }
            }
        }
        Ok(outcomes)
    }

    pub async fn set_auto_sync(&self, enabled: bool) -> Result<SyncStatus> {
        self.inner.auto_sync.store(enabled, Ordering::SeqCst);
        self.inner
            .storage
            .settings
            .set("sync.auto", if enabled { "true" } else { "false" })
            .await?;
        if enabled {
            self.ensure_auto_sync_task().await;
            self.inner.auto_sync_notify.notify_one();
        } else if let Some(task) = self.inner.auto_sync_task.lock().await.take() {
            task.abort();
        }
        let status = self.inner.refresh_status().await;
        self.inner.emit_status(&status);
        Ok(status)
    }

    async fn ensure_auto_sync_task(&self) {
        if !self.inner.auto_sync.load(Ordering::SeqCst) {
            return;
        }
        let mut slot = self.inner.auto_sync_task.lock().await;
        if slot.is_some() {
            return;
        }
        let inner = self.inner.clone();
        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = interval.tick() => {}
                    _ = inner.auto_sync_notify.notified() => {
                        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                    }
                }
                if !inner.auto_sync.load(Ordering::SeqCst) {
                    continue;
                }
                if !inner.status.read().await.running {
                    continue;
                }
                let guard = match inner.sync_lock.try_lock() {
                    Ok(guard) => guard,
                    Err(_) => continue,
                };
                let peers = inner.online_trusted_ids().await.unwrap_or_default();
                for peer_id in peers {
                    if let Err(error) = inner.sync_peer(&peer_id).await {
                        tracing::warn!(%error, peer = %peer_id, "auto-sync falló");
                    }
                }
                drop(guard);
            }
        });
        *slot = Some(task);
    }

    pub async fn diagnostics(&self) -> SyncDiagnostics {
        let trusted = self
            .inner
            .storage
            .devices
            .list_trusted(&self.inner.owner_account_id)
            .await
            .map(|devices| devices.len())
            .unwrap_or(0);
        let online = self
            .inner
            .mdns
            .peers()
            .into_iter()
            .filter(|peer| peer.id != self.inner.identity.device_id)
            .count();
        SyncDiagnostics {
            mdns_active: self.inner.mdns.is_running(),
            listen_port: self.inner.listen_port.load(Ordering::SeqCst),
            local_addresses: self.inner.local_addresses(),
            auto_sync: self.inner.auto_sync.load(Ordering::SeqCst),
            trusted_peers: trusted,
            online_peers: online,
            last_error: self.inner.last_error.read().await.clone(),
        }
    }

    pub fn position(&self, book_id: &str, chapter: usize, offset: usize) {
        if let Ok(status) = self.inner.mirror_status.try_read()
            && status.hosting
        {
            self.inner.mirror.position(book_id, chapter, offset);
        }
    }

    pub fn handoff(&self, book_id: &str, chapter: usize, offset: usize, percent: f64) {
        let inner = self.inner.clone();
        let book_id = book_id.to_string();
        let handoff = Handoff {
            book_id: book_id.clone(),
            chapter,
            offset,
            percent,
            updated_at: Utc::now(),
        };
        tokio::spawn(async move {
            if let Ok(mut cache) = inner.handoffs.try_lock() {
                cache.insert(book_id, handoff.clone());
            }
            let _ = inner.send_handoff(handoff).await;
        });
    }

    pub async fn start_mirror(&self, book_id: &str) -> Result<MirrorStatus> {
        self.start().await?;
        let book = self
            .inner
            .storage
            .books
            .get(book_id)
            .await?
            .ok_or_else(|| KoraError::NotFound(format!("libro {book_id}")))?;
        if book.owner_account_id != self.inner.owner_account_id {
            return Err(KoraError::NotFound(format!("libro {book_id}")));
        }
        let status = {
            let mut status = self.inner.mirror_status.write().await;
            status.active = true;
            status.hosting = true;
            status.book_id = Some(book_id.to_string());
            status.host_device_id = Some(self.inner.identity.device_id.clone());
            status.followers.clear();
            status.clone()
        };
        self.inner.emit_status(&self.inner.refresh_status().await);
        Ok(status)
    }

    pub async fn stop_mirror(&self) -> Result<MirrorStatus> {
        let peers = self.inner.mirror_sessions.lock().await;
        for peer in peers.values() {
            let mut send = peer.send.lock().await;
            let _ = write_frame(
                &mut *send,
                &Message::MirrorStop {
                    book_id: peer.book_id.clone(),
                },
                &[],
            )
            .await;
        }
        drop(peers);
        self.inner.mirror_sessions.lock().await.clear();
        let status = {
            let mut status = self.inner.mirror_status.write().await;
            *status = MirrorStatus::default();
            status.clone()
        };
        self.inner.emit_status(&self.inner.refresh_status().await);
        Ok(status)
    }

    pub async fn join_mirror(&self, peer_id: &str, book_id: &str) -> Result<MirrorStatus> {
        self.start().await?;
        let book = self
            .inner
            .storage
            .books
            .get(book_id)
            .await?
            .ok_or_else(|| KoraError::NotFound(format!("libro {book_id}")))?;
        if book.owner_account_id != self.inner.owner_account_id {
            return Err(KoraError::NotFound(format!("libro {book_id}")));
        }
        let peer = self.inner.trusted_device(peer_id).await?;
        let candidates = self.inner.peer_addresses(peer_id).await?;
        let endpoint = self.ensure_endpoint().await?;
        let expected = peer.certificate_fingerprint.clone();
        let (connection, observed, _) =
            transport::connect_any(&endpoint, &candidates, expected.as_deref()).await?;
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|error| KoraError::Internal(format!("stream de espejo: {error}")))?;
        write_frame(
            &mut send,
            &Message::Hello(self.inner.hello(Purpose::Mirror)),
            &[],
        )
        .await?;
        let remote = self.inner.read_remote_hello(&mut recv).await?;
        self.inner.verify_trusted(&remote, &observed).await?;
        write_frame(
            &mut send,
            &Message::MirrorJoin {
                book_id: book_id.to_string(),
            },
            &[],
        )
        .await?;
        match read_frame(&mut recv).await? {
            Some((Message::Ack { .. }, _)) => {}
            Some((Message::Error { message }, _)) => {
                return Err(KoraError::Conflict(format!("espejo rechazado: {message}")));
            }
            _ => {
                return Err(KoraError::InvalidInput(
                    "respuesta de espejo inesperada".into(),
                ));
            }
        }

        if let Some(task) = self.inner.mirror_task.lock().await.take() {
            task.abort();
        }
        let inner = self.inner.clone();
        let connection_for_task = connection.clone();
        let book = book_id.to_string();
        let task = tokio::spawn(async move {
            inner.mirror_reader(connection_for_task, recv, book).await;
        });
        *self.inner.mirror_task.lock().await = Some(task);
        *self.inner.mirror_connection.lock().await = Some(connection);
        *self.inner.mirror_send.lock().await = Some(send);
        let status = {
            let mut status = self.inner.mirror_status.write().await;
            status.active = true;
            status.hosting = false;
            status.book_id = Some(book_id.to_string());
            status.host_device_id = Some(remote.device_id);
            status.clone()
        };
        self.inner.emit_status(&self.inner.refresh_status().await);
        Ok(status)
    }

    pub async fn leave_mirror(&self) -> Result<MirrorStatus> {
        if let Some(task) = self.inner.mirror_task.lock().await.take() {
            task.abort();
        }
        *self.inner.mirror_send.lock().await = None;
        if let Some(connection) = self.inner.mirror_connection.lock().await.take() {
            connection.close(0u32.into(), b"leaving mirror");
        }
        let status = {
            let mut status = self.inner.mirror_status.write().await;
            *status = MirrorStatus::default();
            status.clone()
        };
        self.inner.emit_status(&self.inner.refresh_status().await);
        Ok(status)
    }

    pub async fn mirror_status(&self) -> MirrorStatus {
        self.inner.mirror_status.read().await.clone()
    }

    pub async fn touch(&self, resource_type: ResourceType, id: &str) {
        if let Err(error) = self.inner.crdt.touch(resource_type, id).await {
            tracing::warn!(%error, ?resource_type, id, "no se pudo actualizar el CRDT local");
        }
        self.inner.auto_sync_notify.notify_one();
    }

    async fn ensure_endpoint(&self) -> Result<quinn::Endpoint> {
        if let Some(endpoint) = self.inner.endpoint.lock().await.clone() {
            return Ok(endpoint);
        }
        let bind = "0.0.0.0:47821"
            .parse()
            .map_err(|_| KoraError::Internal("dirección de sync inválida".into()))?;
        let endpoint = match transport::server_endpoint(&self.inner.identity, bind) {
            Ok(endpoint) => endpoint,
            Err(error) => {
                tracing::warn!(%error, "puerto 47821 ocupado; usando un puerto efímero");
                transport::server_endpoint(
                    &self.inner.identity,
                    "0.0.0.0:0"
                        .parse()
                        .map_err(|_| KoraError::Internal("dirección de sync inválida".into()))?,
                )?
            }
        };
        let port = endpoint
            .local_addr()
            .map_err(|error| KoraError::Internal(format!("UDP local: {error}")))?
            .port();
        self.inner.listen_port.store(port, Ordering::SeqCst);
        let inner = self.inner.clone();
        let accept_endpoint = endpoint.clone();
        let task = tokio::spawn(async move {
            accept_loop(inner, accept_endpoint).await;
        });
        *self.inner.listener.lock().await = Some(task);
        *self.inner.endpoint.lock().await = Some(endpoint.clone());
        Ok(endpoint)
    }
}

async fn accept_loop(inner: Arc<Inner>, endpoint: quinn::Endpoint) {
    while let Some(connecting) = endpoint.accept().await {
        match connecting.await {
            Ok(connection) => {
                let inner = inner.clone();
                tokio::spawn(async move {
                    if let Err(error) = inner.handle_connection(connection).await {
                        tracing::warn!(%error, "conexión de sync terminó con error");
                    }
                });
            }
            Err(error) => {
                tracing::debug!(%error, "conexión QUIC rechazada");
            }
        }
    }
}

impl Inner {
    fn hello(&self, purpose: Purpose) -> Hello {
        Hello {
            version: PROTOCOL_VERSION,
            device_id: self.identity.device_id.clone(),
            account_id: self.owner_account_id.clone(),
            account_hint: self.account_hint.clone(),
            username: self
                .account_username
                .read()
                .ok()
                .and_then(|value| value.clone())
                .unwrap_or_default(),
            account_public_key: self
                .account_public_key
                .read()
                .ok()
                .and_then(|value| value.clone())
                .unwrap_or_default(),
            device_name: self.identity.name.clone(),
            platform: std::env::consts::OS.to_string(),
            listen_port: self.listen_port.load(Ordering::SeqCst),
            public_key: self.identity.public_key.clone(),
            fingerprint: self.identity.fingerprint.clone(),
            sas_prefix: self.identity.sas_prefix.clone(),
            purpose,
            nonce: uuid::Uuid::now_v7().to_string(),
        }
    }

    async fn read_remote_hello(&self, recv: &mut quinn::RecvStream) -> Result<Hello> {
        match read_frame(recv).await? {
            Some((Message::Hello(hello), _)) => Ok(hello),
            Some((Message::Error { message }, _)) => Err(KoraError::Conflict(message)),
            _ => Err(KoraError::InvalidInput(
                "el peer no envió un saludo de sync".into(),
            )),
        }
    }

    async fn handle_connection(&self, connection: quinn::Connection) -> Result<()> {
        let observed = transport::peer_fingerprint(&connection)
            .ok_or_else(|| KoraError::Unsupported("peer sin certificado".into()))?;
        let (mut send, mut recv) = connection
            .accept_bi()
            .await
            .map_err(|error| KoraError::Internal(format!("stream entrante: {error}")))?;
        let remote = self.read_remote_hello(&mut recv).await?;
        if remote.fingerprint != observed {
            let _ = write_frame(
                &mut send,
                &Message::Error {
                    message: "huella de certificado inconsistente".to_string(),
                },
                &[],
            )
            .await;
            return Err(KoraError::Conflict(
                "la huella del Hello no coincide con el certificado".into(),
            ));
        }

        match remote.purpose {
            Purpose::Pair => {
                self.handle_pairing(connection, send, recv, &remote, &observed)
                    .await
            }
            Purpose::Sync => {
                self.verify_trusted(&remote, &observed).await?;
                write_frame(&mut send, &Message::Hello(self.hello(Purpose::Sync)), &[]).await?;
                let (sent, received) = self
                    .run_responder(&mut send, &mut recv, &remote.device_id)
                    .await?;
                *self.last_sync.write().await = Some(Utc::now());
                let status = self.refresh_status().await;
                self.emit_status(&status);
                self.events.emit(
                    "sync::completed",
                    json!({
                        "peerId": remote.device_id,
                        "sent": sent,
                        "received": received,
                    }),
                );
                connection.close(0u32.into(), b"sync done");
                Ok(())
            }
            Purpose::Share => {
                self.handle_share(connection, send, recv, &remote, &observed)
                    .await
            }
            Purpose::Handoff => {
                self.verify_trusted(&remote, &observed).await?;
                write_frame(
                    &mut send,
                    &Message::Hello(self.hello(Purpose::Handoff)),
                    &[],
                )
                .await?;
                if let Some((Message::Handoff(handoff), _)) = read_frame(&mut recv).await? {
                    let book_id = handoff.book_id.clone();
                    self.handoffs
                        .lock()
                        .await
                        .insert(book_id.clone(), handoff.clone());
                    self.events.emit(
                        "sync::handoff",
                        json!({
                            "peerId": remote.device_id,
                            "peerName": remote.device_name,
                            "handoff": handoff,
                        }),
                    );
                    let _ = write_frame(
                        &mut send,
                        &Message::Ack {
                            resources: Vec::new(),
                        },
                        &[],
                    )
                    .await;
                }
                connection.close(0u32.into(), b"handoff");
                Ok(())
            }
            Purpose::Mirror => self.handle_mirror(connection, send, recv, remote).await,
        }
    }

    async fn handle_pairing(
        &self,
        connection: quinn::Connection,
        mut send: quinn::SendStream,
        mut recv: quinn::RecvStream,
        remote: &Hello,
        observed: &str,
    ) -> Result<()> {
        if remote.version < 2 {
            reject_pairing(
                &mut send,
                "actualiza Kora en ambos dispositivos para vincularlos",
                false,
            )
            .await;
            return Ok(());
        }
        if !self.pairing_mode.load(Ordering::SeqCst) {
            reject_pairing(
                &mut send,
                "este dispositivo no está mostrando un código de vinculación",
                true,
            )
            .await;
            return Ok(());
        }
        if remote.fingerprint != observed {
            return Err(KoraError::Conflict(
                "la huella del Hello no coincide con el certificado".into(),
            ));
        }
        let ticket = {
            let mut guard = self.pairing_ticket.lock().await;
            match guard.as_ref() {
                Some(ticket) if !ticket.expired() && ticket.attempts < 5 => ticket.clone(),
                Some(ticket) if ticket.expired() => {
                    *guard = None;
                    self.pairing_mode.store(false, Ordering::SeqCst);
                    self.status.write().await.pairing = false;
                    drop(guard);
                    let _ = self.mdns.stop_advertise();
                    self.emit_pairing_state("error", "El código ha caducado", None);
                    let status = self.refresh_status().await;
                    self.emit_status(&status);
                    reject_pairing(&mut send, "el código ha caducado", false).await;
                    return Ok(());
                }
                Some(_) => {
                    *guard = None;
                    self.pairing_mode.store(false, Ordering::SeqCst);
                    self.status.write().await.pairing = false;
                    drop(guard);
                    let _ = self.mdns.stop_advertise();
                    self.emit_pairing_state(
                        "error",
                        "Demasiados intentos. Genera un código nuevo.",
                        None,
                    );
                    let status = self.refresh_status().await;
                    self.emit_status(&status);
                    reject_pairing(
                        &mut send,
                        "demasiados intentos; genera un código nuevo",
                        false,
                    )
                    .await;
                    return Ok(());
                }
                None => {
                    drop(guard);
                    reject_pairing(
                        &mut send,
                        "este dispositivo no está mostrando un código de vinculación",
                        true,
                    )
                    .await;
                    return Ok(());
                }
            }
        };
        let local_hello = self.hello(Purpose::Pair);
        write_frame(&mut send, &Message::Hello(local_hello.clone()), &[]).await?;
        let requested_ticket = match read_frame(&mut recv).await? {
            Some((Message::PairRequest { ticket_id }, _)) => ticket_id,
            Some((Message::PairRejected { reason, .. }, _)) => {
                return Err(KoraError::Conflict(reason));
            }
            _ => {
                return Err(KoraError::InvalidInput(
                    "el invitado no envió un ticket de vinculación".into(),
                ));
            }
        };
        if !requested_ticket.is_empty() && requested_ticket != ticket.id {
            self.record_pairing_failure(&ticket.id).await;
            reject_pairing(&mut send, "el código ya no es válido", true).await;
            return Ok(());
        }
        let message_a = match read_frame(&mut recv).await? {
            Some((Message::Spake2A { payload }, _)) => hex::decode(payload)
                .map_err(|_| KoraError::InvalidInput("SPAKE2 inválido".into()))?,
            Some((Message::PairRejected { reason, .. }, _)) => {
                return Err(KoraError::Conflict(reason));
            }
            _ => {
                return Err(KoraError::InvalidInput(
                    "el invitado no inició SPAKE2".into(),
                ));
            }
        };
        self.emit_pairing_state(
            "verifying",
            "Comprobando el código",
            Some(remote.device_name.clone()),
        );
        let password = crypto::pairing_password_v2(&ticket.code, &local_hello.nonce)?;
        let id_a = remote.nonce.as_bytes();
        let id_b = local_hello.nonce.as_bytes();
        let (state, message_b) = Spake2::<Ed25519Group>::start_b(
            &Password::new(password.as_slice()),
            &Identity::new(id_a),
            &Identity::new(id_b),
        );
        let shared = match state.finish(&message_a) {
            Ok(shared) => shared,
            Err(_) => {
                self.record_pairing_failure(&ticket.id).await;
                reject_pairing(&mut send, "el código de vinculación no coincide", true).await;
                return Ok(());
            }
        };
        let transcript = pairing_transcript(&local_hello, remote, Some(&message_a), &message_b);
        let sas = crypto::sas_from_key(&shared, &transcript)?;
        write_frame(
            &mut send,
            &Message::Spake2B {
                payload: hex::encode(&message_b),
                sas: sas.clone(),
            },
            &[],
        )
        .await?;
        let confirmation =
            tokio::time::timeout(std::time::Duration::from_secs(20), read_frame(&mut recv)).await;
        match confirmation {
            Ok(Ok(Some((Message::SasConfirm { sas: remote_sas }, _)))) if remote_sas == sas => {}
            Ok(Ok(Some((Message::PairRejected { reason, retryable }, _)))) => {
                self.record_pairing_failure(&ticket.id).await;
                let _ =
                    write_frame(&mut send, &Message::PairRejected { reason, retryable }, &[]).await;
                return Ok(());
            }
            _ => {
                self.record_pairing_failure(&ticket.id).await;
                reject_pairing(&mut send, "no se pudo confirmar el código", true).await;
                return Ok(());
            }
        }

        self.store_paired_device(
            remote,
            observed,
            Some(connection.remote_address().ip()),
            Some(remote.listen_port as i64),
        )
        .await?;
        self.accounts
            .audit("device.pair", None, Some(&remote.device_id))
            .await?;
        *self.pairing_ticket.lock().await = None;
        self.pairing_mode.store(false, Ordering::SeqCst);
        self.status.write().await.pairing = false;
        let _ = self.mdns.stop_advertise();
        let _ = self.advertise_presence();
        write_frame(&mut send, &Message::SasConfirm { sas }, &[]).await?;
        let result = PairedDeviceView {
            id: remote.device_id.clone(),
            name: remote.device_name.clone(),
            platform: remote.platform.clone(),
            fingerprint: observed.to_string(),
            account_hint: remote.account_hint.clone(),
        };
        self.emit("sync::paired", serde_json::to_value(&result)?);
        self.emit_pairing_state("paired", "Dispositivo vinculado", Some(result.name.clone()));
        let status = self.refresh_status().await;
        self.emit_status(&status);
        self.auto_sync_notify.notify_one();
        connection.close(0u32.into(), b"paired");
        Ok(())
    }

    async fn record_pairing_failure(&self, ticket_id: &str) {
        let mut guard = self.pairing_ticket.lock().await;
        let Some(ticket) = guard.as_mut() else {
            return;
        };
        if ticket.id != ticket_id {
            return;
        }
        ticket.attempts += 1;
        if ticket.attempts >= 5 {
            *guard = None;
            self.pairing_mode.store(false, Ordering::SeqCst);
            self.status.write().await.pairing = false;
            let _ = self.mdns.stop_advertise();
            self.emit_pairing_state(
                "error",
                "Demasiados intentos. Genera un código nuevo.",
                None,
            );
            let status = self.refresh_status().await;
            self.emit_status(&status);
        }
    }

    async fn handle_share(
        &self,
        connection: quinn::Connection,
        mut send: quinn::SendStream,
        mut recv: quinn::RecvStream,
        remote: &Hello,
        observed: &str,
    ) -> Result<()> {
        if remote.fingerprint != observed {
            return Err(KoraError::Conflict(
                "la huella del invitado no coincide".into(),
            ));
        }
        let key_hash = match read_frame(&mut recv).await? {
            Some((Message::ShareRequest { key_hash }, _)) => key_hash,
            Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
            _ => {
                return Err(KoraError::InvalidInput(
                    "el invitado no pidió un share".into(),
                ));
            }
        };
        let share = self
            .storage
            .shares
            .find_by_key_hash(&key_hash)
            .await?
            .ok_or_else(|| KoraError::NotFound("share no encontrado o revocado".into()))?;
        if share.scope_type == ShareScopeType::Voice {
            let _ = write_frame(
                &mut send,
                &Message::Error {
                    message: "compartir perfiles de voz aún no está disponible".to_string(),
                },
                &[],
            )
            .await;
            return Ok(());
        }
        if !self.storage.shares.increment_uses(&share.id).await? {
            let _ = write_frame(
                &mut send,
                &Message::Error {
                    message: "share expirado, revocado o sin usos".to_string(),
                },
                &[],
            )
            .await;
            return Ok(());
        }

        let local_hello = self.hello(Purpose::Share);
        write_frame(&mut send, &Message::Hello(local_hello.clone()), &[]).await?;
        let message_a = match read_frame(&mut recv).await? {
            Some((Message::Spake2A { payload }, _)) => hex::decode(payload)
                .map_err(|_| KoraError::InvalidInput("SPAKE2 inválido".into()))?,
            Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
            _ => {
                return Err(KoraError::InvalidInput(
                    "el invitado no inició SPAKE2".into(),
                ));
            }
        };
        let password = hex::decode(&key_hash)
            .map_err(|_| KoraError::InvalidInput("hash de share inválido".into()))?;
        let id_a = remote.nonce.as_bytes();
        let id_b = local_hello.nonce.as_bytes();
        let (state, message_b) = Spake2::<Ed25519Group>::start_b(
            &Password::new(password.as_slice()),
            &Identity::new(id_a),
            &Identity::new(id_b),
        );
        let shared = state
            .finish(&message_a)
            .map_err(|_| KoraError::Conflict("la clave de share no coincide".into()))?;
        let transcript = pairing_transcript(&local_hello, remote, Some(&message_a), &message_b);
        let sas = crypto::sas_from_key(&shared, &transcript)?;
        write_frame(
            &mut send,
            &Message::Spake2B {
                payload: hex::encode(&message_b),
                sas,
            },
            &[],
        )
        .await?;

        let owner = self
            .storage
            .accounts
            .find_local()
            .await?
            .ok_or_else(|| KoraError::Unsupported("no hay cuenta local".into()))?;
        let grant = json!({
            "shareId": share.id,
            "ownerAccountId": owner.id,
            "ownerUsername": owner.username,
            "ownerPublicKey": owner.public_key,
            "permission": share.permission,
            "scopeType": share.scope_type,
            "resourceType": share.resource_type,
            "expiresAt": share.expires_at,
            "maxUses": share.max_uses,
            "uses": share.uses + 1,
        });
        write_frame(&mut send, &Message::ShareGrant { payload: grant }, &[]).await?;

        let mut manifest = manifest::build_for_share(
            &self.storage,
            &self.crdt,
            &self.owner_account_id,
            &self.account_hint,
            &self.identity.device_id,
        )
        .await?;
        let shared_audiobook = if share.resource_type == ResourceType::Audiobook {
            Some(
                self.storage
                    .audiobooks
                    .get(&share.resource_id)
                    .await?
                    .ok_or_else(|| {
                        KoraError::NotFound(format!("audiolibro {}", share.resource_id))
                    })?,
            )
        } else {
            None
        };
        if let Some(audiobook) = &shared_audiobook {
            // El libro asociado viaja como dependencia del audiolibro para que
            // el receptor conserve texto, portada y metadatos.
            manifest.resources.retain(|resource| {
                resource.reference.kind == ResourceType::Category
                    || (resource.reference.kind == ResourceType::Book
                        && resource.reference.id == audiobook.book_id)
            });
            manifest
                .resources
                .push(manifest::audiobook_resource(audiobook)?);
        } else {
            manifest.resources.retain(|resource| {
                resource.reference.kind == ResourceType::Category
                    || (resource.reference.kind == ResourceType::Book
                        && (share.scope_type == ShareScopeType::Library
                            || resource.reference.id == share.resource_id))
            });
        }
        write_frame(&mut send, &Message::Manifest(manifest.clone()), &[]).await?;
        let requested = match read_frame(&mut recv).await? {
            Some((Message::Pull { resources }, _)) => resources,
            Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
            _ => {
                return Err(KoraError::InvalidInput(
                    "el invitado no pidió recursos".into(),
                ));
            }
        };
        self.serve_resources(&mut send, &manifest, &requested, &remote.device_id)
            .await?;
        // El invitado confirma cuando ha aplicado todos los recursos. Así el
        // cierre no puede perder el último frame.
        if remote.version >= 2 {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(20), read_frame(&mut recv))
                .await;
        }
        self.accounts
            .audit("share.accepted", None, Some(&share.id))
            .await?;
        connection.close(0u32.into(), b"share done");
        Ok(())
    }

    async fn handle_mirror(
        &self,
        connection: quinn::Connection,
        mut send: quinn::SendStream,
        mut recv: quinn::RecvStream,
        remote: Hello,
    ) -> Result<()> {
        self.verify_trusted(
            &remote,
            &transport::peer_fingerprint(&connection).unwrap_or_default(),
        )
        .await?;
        write_frame(&mut send, &Message::Hello(self.hello(Purpose::Mirror)), &[]).await?;
        let book_id = match read_frame(&mut recv).await? {
            Some((Message::MirrorJoin { book_id }, _)) => book_id,
            Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
            _ => {
                return Err(KoraError::InvalidInput(
                    "el seguidor no se unió a un espejo".into(),
                ));
            }
        };
        let send = Arc::new(Mutex::new(send));
        self.mirror_sessions.lock().await.insert(
            remote.device_id.clone(),
            MirrorPeer {
                book_id: book_id.clone(),
                send: send.clone(),
                _connection: connection,
            },
        );
        {
            let mut status = self.mirror_status.write().await;
            if !status.followers.contains(&remote.device_id) {
                status.followers.push(remote.device_id.clone());
            }
        }
        write_frame(
            &mut *send.lock().await,
            &Message::Ack {
                resources: Vec::new(),
            },
            &[],
        )
        .await?;
        self.emit_status(&self.refresh_status().await);

        loop {
            match read_frame(&mut recv).await {
                Ok(Some((Message::MirrorStop { .. }, _))) | Ok(None) => break,
                Ok(Some(_)) => {}
                Err(_) => break,
            }
        }
        self.mirror_sessions.lock().await.remove(&remote.device_id);
        {
            let mut status = self.mirror_status.write().await;
            status.followers.retain(|id| id != &remote.device_id);
        }
        Ok(())
    }

    async fn verify_trusted(
        &self,
        hello: &Hello,
        observed: &str,
    ) -> Result<crate::storage::Device> {
        if hello.fingerprint != observed {
            return Err(KoraError::Conflict(
                "la huella del dispositivo no coincide".into(),
            ));
        }
        let device = self
            .storage
            .devices
            .get(&hello.device_id)
            .await?
            .ok_or_else(|| {
                KoraError::Unsupported(
                    "el dispositivo no está emparejado; abre la pantalla de emparejamiento".into(),
                )
            })?;
        if !device.trusted {
            return Err(KoraError::Unsupported(
                "el dispositivo no es de confianza".into(),
            ));
        }
        if let Some(expected) = &device.certificate_fingerprint
            && expected != observed
        {
            return Err(KoraError::Conflict(
                "el certificado del dispositivo cambió; vuelve a emparejar".into(),
            ));
        }
        self.storage
            .devices
            .mark_seen(&hello.device_id, None, Some(hello.listen_port as i64))
            .await?;
        Ok(device)
    }

    async fn store_paired_device(
        &self,
        hello: &Hello,
        fingerprint: &str,
        address: Option<IpAddr>,
        port: Option<i64>,
    ) -> Result<()> {
        let address = match address {
            Some(address) => Some(address.to_string()),
            None => self
                .storage
                .devices
                .get(&hello.device_id)
                .await?
                .and_then(|device| device.last_address),
        };
        self.storage
            .devices
            .upsert_paired(
                &self.owner_account_id,
                &hello.device_id,
                &hello.device_name,
                &hello.platform,
                &hello.public_key,
                fingerprint,
                &hello.account_id,
                &hello.username,
                &hello.account_public_key,
                None,
                None,
                address.as_deref(),
                port,
            )
            .await?;
        Ok(())
    }

    async fn trusted_device(&self, peer_id: &str) -> Result<crate::storage::Device> {
        let device = self
            .storage
            .devices
            .get(peer_id)
            .await?
            .ok_or_else(|| KoraError::NotFound(format!("dispositivo {peer_id}")))?;
        if !device.trusted || device.is_local {
            return Err(KoraError::Unsupported(
                "el dispositivo no es un peer de confianza".into(),
            ));
        }
        Ok(device)
    }

    async fn peer_addresses(&self, peer_id: &str) -> Result<Vec<SocketAddr>> {
        let mut candidates = Vec::new();
        if let Some(peer) = self
            .mdns
            .peers()
            .into_iter()
            .find(|peer| peer.id == peer_id)
        {
            let hosts = if peer.addresses.is_empty() {
                peer.address.iter().cloned().collect::<Vec<_>>()
            } else {
                peer.addresses.clone()
            };
            for host in hosts {
                if let Some(address) = parse_candidate(&host, peer.port) {
                    candidates.push(address);
                }
            }
        }
        if let Some(device) = self.storage.devices.get(peer_id).await?
            && let (Some(address), Some(port)) = (device.last_address, device.last_port)
            && let Some(address) = parse_candidate(&address, port as u16)
        {
            candidates.push(address);
        }
        dedupe_addresses(candidates)
    }

    async fn socket_candidates(
        &self,
        hosts: &[String],
        default_port: u16,
    ) -> Result<Vec<SocketAddr>> {
        let mut candidates = Vec::new();
        for host in hosts {
            if let Some(address) = parse_candidate(host, default_port) {
                candidates.push(address);
                continue;
            }
            if let Ok(resolved) = tokio::net::lookup_host((host.as_str(), default_port)).await {
                candidates.extend(resolved);
            }
        }
        dedupe_addresses(candidates)
    }

    async fn pairing_target(
        &self,
        input: &JoinPairingInput,
    ) -> Result<(String, String, Vec<String>, u16, String)> {
        if let Some(uri) = input.uri.as_deref().filter(|uri| !uri.trim().is_empty()) {
            let link = crypto::parse_pairing_link(uri).ok_or_else(|| {
                KoraError::InvalidInput("el QR o enlace de vinculación no es válido".into())
            })?;
            let code = input
                .code
                .as_deref()
                .filter(|code| !code.trim().is_empty())
                .unwrap_or(&link.code)
                .trim()
                .to_string();
            return Ok((
                link.ticket_id,
                code,
                link.hosts,
                link.port,
                link.fingerprint,
            ));
        }

        let peer = match input
            .peer_id
            .as_deref()
            .filter(|peer_id| !peer_id.trim().is_empty())
        {
            Some(peer_id) => self
                .mdns
                .peers()
                .into_iter()
                .find(|peer| peer.id == peer_id && peer.id != self.identity.device_id),
            None => self
                .mdns
                .peers()
                .into_iter()
                .find(|peer| peer.purpose == "pair" && peer.id != self.identity.device_id),
        };
        if let Some(peer) = peer {
            let code = input
                .code
                .clone()
                .filter(|code| !code.trim().is_empty())
                .ok_or_else(|| {
                    KoraError::InvalidInput(
                        "escribe el código que muestra el otro dispositivo".into(),
                    )
                })?;
            let mut hosts = if peer.addresses.is_empty() {
                peer.address.iter().cloned().collect::<Vec<_>>()
            } else {
                peer.addresses.clone()
            };
            let mut port = peer.port;
            // Algunos entornos (CI, contenedores o mDNS con solo loopback o
            // IPv6 link-local) descubren el peer sin una dirección conectable.
            // Si el usuario escribió una IP, se añade como candidata y se
            // prueba antes de rendirse, sin perder el ticket ni la huella del
            // peer descubierto.
            if let Some(address) = input
                .address
                .as_deref()
                .map(str::trim)
                .filter(|address| !address.is_empty())
            {
                if let Some(parsed) = parse_candidate(address, port) {
                    let host = parsed.ip().to_string();
                    if !hosts.iter().any(|candidate| candidate == &host) {
                        hosts.push(host);
                    }
                    port = parsed.port();
                } else if !hosts.iter().any(|candidate| candidate == address) {
                    hosts.push(address.to_string());
                }
            }
            return Ok((
                peer.ticket.unwrap_or_default(),
                code.trim().to_string(),
                hosts,
                port,
                peer.fingerprint,
            ));
        }

        let address = input
            .address
            .as_deref()
            .map(str::trim)
            .filter(|address| !address.is_empty())
            .ok_or_else(|| {
                KoraError::NotFound(
                    "no hay dispositivos visibles; escanea el QR o introduce la dirección del otro dispositivo"
                        .into(),
                )
            })?;
        let code = input
            .code
            .clone()
            .filter(|code| !code.trim().is_empty())
            .ok_or_else(|| {
                KoraError::InvalidInput("escribe el código que muestra el otro dispositivo".into())
            })?;
        let (host, port) = match parse_candidate(address, 47821) {
            Some(parsed) => (parsed.ip().to_string(), parsed.port()),
            None => match address.rsplit_once(':') {
                Some((host, port)) if !host.contains(':') => match port.parse::<u16>() {
                    Ok(port) => (host.to_string(), port),
                    Err(_) => (address.to_string(), 47821),
                },
                _ => (address.to_string(), 47821),
            },
        };
        Ok((
            String::new(),
            code.trim().to_string(),
            vec![host],
            port,
            String::new(),
        ))
    }

    async fn peers(&self) -> Result<Vec<PeerInfo>> {
        let trusted = self
            .storage
            .devices
            .list_trusted(&self.owner_account_id)
            .await?;
        let mut peers: HashMap<String, PeerInfo> = self
            .mdns
            .peers()
            .into_iter()
            .filter(|peer| peer.id != self.identity.device_id)
            .map(|peer| (peer.id.clone(), peer))
            .collect();
        for device in trusted {
            let peer = peers.entry(device.id.clone()).or_insert_with(|| PeerInfo {
                id: device.id.clone(),
                name: device.name.clone(),
                platform: device.platform.clone(),
                address: device.last_address.clone(),
                addresses: device.last_address.clone().into_iter().collect(),
                port: device.last_port.unwrap_or(0) as u16,
                last_seen: device.last_seen_at,
                trusted: true,
                account_hint: self.account_hint.clone(),
                purpose: "trusted".to_string(),
                sas_prefix: String::new(),
                fingerprint: device.certificate_fingerprint.clone().unwrap_or_default(),
                fullname: String::new(),
                protocol_version: PROTOCOL_VERSION,
                ticket: None,
            });
            peer.trusted = true;
            if peer.address.is_none() {
                peer.address = device.last_address.clone();
            }
            if peer.addresses.is_empty()
                && let Some(address) = device.last_address
            {
                peer.addresses.push(address);
            }
            if peer.port == 0 {
                peer.port = device.last_port.unwrap_or(0) as u16;
            }
        }
        let mut peers: Vec<_> = peers.into_values().collect();
        peers.sort_by(|left, right| {
            right
                .trusted
                .cmp(&left.trusted)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(peers)
    }

    async fn refresh_status(&self) -> SyncStatus {
        let pending = self.storage.sync_pending.count().await.unwrap_or(0);
        let trusted = self
            .storage
            .devices
            .list_trusted(&self.owner_account_id)
            .await
            .map(|devices| devices.len())
            .unwrap_or(0);
        let peers = self.mdns.peers();
        let online_peers = peers
            .iter()
            .filter(|peer| peer.id != self.identity.device_id)
            .count();
        let peer_count = peers.len();
        let mirror = self.mirror_status.read().await.clone();
        let last_sync = *self.last_sync.read().await;
        let mut status = self.status.write().await;
        status.peer_count = peer_count;
        status.online_peers = online_peers;
        status.listen_port = self.listen_port.load(std::sync::atomic::Ordering::SeqCst);
        status.trusted_peers = trusted;
        status.pending_changes = pending;
        status.last_sync = last_sync;
        status.mirror_active = mirror.active;
        status.mirror_hosting = mirror.hosting;
        status.syncing = self.syncing.load(Ordering::SeqCst);
        status.auto_sync = self.auto_sync.load(Ordering::SeqCst);
        status.last_error = self.last_error.read().await.clone();
        status.clone()
    }

    fn emit_status(&self, status: &SyncStatus) {
        self.events.emit(
            "sync::status",
            serde_json::to_value(status).unwrap_or(Value::Null),
        );
    }

    fn emit(&self, event: &str, payload: Value) {
        self.events.emit(event, payload);
    }

    fn emit_pairing_state(&self, state: &str, message: &str, peer_name: Option<String>) {
        let payload = PairingState {
            state: state.to_string(),
            message: message.to_string(),
            peer_name,
        };
        if let Ok(value) = serde_json::to_value(payload) {
            self.events.emit("sync::pairing_state", value);
        }
    }

    fn emit_progress(
        &self,
        peer_id: &str,
        direction: &str,
        kind: ResourceType,
        current: usize,
        total: usize,
    ) {
        let payload = SyncProgress {
            peer_id: peer_id.to_string(),
            direction: direction.to_string(),
            resource_kind: format!("{kind:?}").to_lowercase(),
            current,
            total,
        };
        if let Ok(value) = serde_json::to_value(payload) {
            self.events.emit("sync::progress", value);
        }
    }

    fn local_addresses(&self) -> Vec<String> {
        let mut addresses: Vec<(u8, String)> = Vec::new();
        if let Ok(interfaces) = if_addrs::get_if_addrs() {
            for interface in interfaces {
                let name = interface.name.to_ascii_lowercase();
                if name == "lo"
                    || name.starts_with("docker")
                    || name.starts_with("veth")
                    || name.starts_with("br-")
                    || name.starts_with("virbr")
                    || name.starts_with("tailscale")
                    || name.starts_with("zt")
                    || name.starts_with("tun")
                    || name.starts_with("tap")
                {
                    continue;
                }
                match interface.addr.ip() {
                    IpAddr::V4(address) if !address.is_loopback() => {
                        addresses.push((ipv4_rank(address), address.to_string()));
                    }
                    IpAddr::V6(address) if !address.is_loopback() => {
                        if address.is_unicast_link_local() {
                            if let Some(index) = interface.index {
                                addresses.push((7, format!("{address}%{index}")));
                            }
                        } else {
                            addresses.push((6, address.to_string()));
                        }
                    }
                    _ => {}
                }
            }
        }
        addresses.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        let mut seen = std::collections::HashSet::new();
        addresses
            .into_iter()
            .filter(|(_, address)| seen.insert(address.clone()))
            .map(|(_, address)| address)
            .collect()
    }

    fn advertise_with(&self, purpose: &str, ticket: Option<&str>) -> Result<()> {
        let port = self.listen_port.load(Ordering::SeqCst);
        let instance = format!(
            "kora-{}-{purpose}",
            self.identity.device_id.chars().take(8).collect::<String>()
        );
        self.mdns.advertise(&ServiceInfo {
            instance,
            name: self.identity.name.clone(),
            platform: std::env::consts::OS.to_string(),
            port,
            fingerprint: self.identity.fingerprint.clone(),
            device_id: self.identity.device_id.clone(),
            account_hint: self.account_hint.clone(),
            sas_prefix: self.identity.sas_prefix.clone(),
            purpose: purpose.to_string(),
            version: PROTOCOL_VERSION,
            ticket: ticket.map(str::to_string),
        })
    }

    fn advertise_pair(&self, ticket_id: &str) -> Result<()> {
        self.advertise_with("pair", Some(ticket_id))
    }

    fn advertise_presence(&self) -> Result<()> {
        self.advertise_with("sync", None)
    }

    fn advertise_share(&self) -> Result<()> {
        self.advertise_with("share", None)
    }

    async fn online_trusted_ids(&self) -> Result<Vec<String>> {
        let trusted = self
            .storage
            .devices
            .list_trusted(&self.owner_account_id)
            .await?;
        let online: std::collections::HashSet<String> = self
            .mdns
            .peers()
            .into_iter()
            .filter(|peer| peer.id != self.identity.device_id)
            .map(|peer| peer.id)
            .collect();
        Ok(trusted
            .into_iter()
            .filter(|device| online.contains(&device.id))
            .map(|device| device.id)
            .collect())
    }

    async fn sync_peer(&self, peer_id: &str) -> Result<SyncOutcome> {
        let peer = self.trusted_device(peer_id).await?;
        let candidates = self.peer_addresses(peer_id).await?;
        if candidates.is_empty() {
            let message = "el dispositivo no tiene una dirección conocida".to_string();
            *self.last_error.write().await = Some(message.clone());
            return Err(KoraError::NotFound(message));
        }
        let endpoint = self
            .endpoint
            .lock()
            .await
            .clone()
            .ok_or_else(|| KoraError::Unsupported("sync no está iniciado".into()))?;
        self.syncing.store(true, Ordering::SeqCst);
        let running_status = self.refresh_status().await;
        self.emit_status(&running_status);
        let started = Instant::now();
        let result = async {
            let expected = peer.certificate_fingerprint.clone();
            let (connection, observed, _) =
                transport::connect_any(&endpoint, &candidates, expected.as_deref()).await?;
            let (mut send, mut recv) = connection
                .open_bi()
                .await
                .map_err(|error| KoraError::Internal(format!("stream de sync: {error}")))?;
            write_frame(&mut send, &Message::Hello(self.hello(Purpose::Sync)), &[]).await?;
            let remote = self.read_remote_hello(&mut recv).await?;
            self.verify_trusted(&remote, &observed).await?;
            let (sent, received) = self
                .run_initiator(&mut send, &mut recv, &remote.device_id)
                .await?;
            connection.close(0u32.into(), b"sync done");
            Ok::<_, KoraError>(SyncOutcome {
                peer_id: remote.device_id,
                sent,
                received,
                duration_ms: started.elapsed().as_millis() as u64,
            })
        }
        .await;
        self.syncing.store(false, Ordering::SeqCst);
        match result {
            Ok(outcome) => {
                *self.last_sync.write().await = Some(Utc::now());
                *self.last_error.write().await = None;
                if let Ok(value) = serde_json::to_value(&outcome) {
                    self.events.emit("sync::completed", value);
                }
                let status = self.refresh_status().await;
                self.emit_status(&status);
                Ok(outcome)
            }
            Err(error) => {
                *self.last_error.write().await = Some(error.to_string());
                let status = self.refresh_status().await;
                self.emit_status(&status);
                self.events
                    .emit("sync::error", json!({ "message": error.to_string() }));
                Err(error)
            }
        }
    }

    async fn run_initiator(
        &self,
        send: &mut quinn::SendStream,
        recv: &mut quinn::RecvStream,
        peer_id: &str,
    ) -> Result<(usize, usize)> {
        let mut local = manifest::build(
            &self.storage,
            &self.crdt,
            &self.owner_account_id,
            &self.account_hint,
            peer_id,
        )
        .await?;
        write_frame(send, &Message::Manifest(local.clone()), &[]).await?;
        let mut remote = match read_frame(recv).await? {
            Some((Message::Manifest(manifest), _)) => manifest,
            Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
            _ => return Err(KoraError::InvalidInput("manifiesto remoto ausente".into())),
        };
        let mut sent_total = 0;
        let mut received_total = 0;

        for _ in 0..3 {
            let needs = diff(&remote, &local);
            write_frame(
                send,
                &Message::Pull {
                    resources: needs.clone(),
                },
                &[],
            )
            .await?;
            let received = self.recv_resources(send, recv, peer_id).await?;
            received_total += received;
            write_frame(send, &Message::ReadyForPull, &[]).await?;

            let remote_needs = match read_frame(recv).await? {
                Some((Message::Pull { resources }, _)) => resources,
                Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
                _ => return Err(KoraError::InvalidInput("pull remoto ausente".into())),
            };
            let sent = self
                .serve_resources(send, &local, &remote_needs, peer_id)
                .await?;
            sent_total += sent;

            match read_frame(recv).await? {
                Some((Message::ReadyServe, _)) => {}
                Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
                _ => return Err(KoraError::InvalidInput("ready remoto ausente".into())),
            }

            local = manifest::build(
                &self.storage,
                &self.crdt,
                &self.owner_account_id,
                &self.account_hint,
                peer_id,
            )
            .await?;
            write_frame(send, &Message::Manifest(local.clone()), &[]).await?;
            remote = match read_frame(recv).await? {
                Some((Message::Manifest(manifest), _)) => manifest,
                Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
                _ => return Err(KoraError::InvalidInput("manifiesto remoto ausente".into())),
            };
            if needs.is_empty() && remote_needs.is_empty() {
                break;
            }
        }

        self.mark_synced(peer_id, &local.resources).await?;
        Ok((sent_total, received_total))
    }

    async fn run_responder(
        &self,
        send: &mut quinn::SendStream,
        recv: &mut quinn::RecvStream,
        peer_id: &str,
    ) -> Result<(usize, usize)> {
        let mut remote = match read_frame(recv).await? {
            Some((Message::Manifest(manifest), _)) => manifest,
            Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
            _ => return Err(KoraError::InvalidInput("manifiesto remoto ausente".into())),
        };
        let mut local = manifest::build(
            &self.storage,
            &self.crdt,
            &self.owner_account_id,
            &self.account_hint,
            peer_id,
        )
        .await?;
        write_frame(send, &Message::Manifest(local.clone()), &[]).await?;
        let mut sent_total = 0;
        let mut received_total = 0;

        for _ in 0..3 {
            let needs = match read_frame(recv).await? {
                Some((Message::Pull { resources }, _)) => resources,
                Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
                _ => return Err(KoraError::InvalidInput("pull remoto ausente".into())),
            };
            let sent = self.serve_resources(send, &local, &needs, peer_id).await?;
            sent_total += sent;

            match read_frame(recv).await? {
                Some((Message::ReadyForPull, _)) => {}
                Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
                _ => return Err(KoraError::InvalidInput("ready remoto ausente".into())),
            }

            let remote_needs = diff(&remote, &local);
            write_frame(
                send,
                &Message::Pull {
                    resources: remote_needs.clone(),
                },
                &[],
            )
            .await?;
            let received = self.recv_resources(send, recv, peer_id).await?;
            received_total += received;
            write_frame(send, &Message::ReadyServe, &[]).await?;

            local = manifest::build(
                &self.storage,
                &self.crdt,
                &self.owner_account_id,
                &self.account_hint,
                peer_id,
            )
            .await?;
            write_frame(send, &Message::Manifest(local.clone()), &[]).await?;
            remote = match read_frame(recv).await? {
                Some((Message::Manifest(manifest), _)) => manifest,
                Some((Message::Error { message }, _)) => return Err(KoraError::Conflict(message)),
                _ => return Err(KoraError::InvalidInput("manifiesto remoto ausente".into())),
            };
            if needs.is_empty() && remote_needs.is_empty() {
                break;
            }
        }

        self.mark_synced(peer_id, &local.resources).await?;
        Ok((sent_total, received_total))
    }
}

impl Inner {
    async fn serve_resources(
        &self,
        send: &mut quinn::SendStream,
        manifest: &Manifest,
        refs: &[ResourceRef],
        peer_id: &str,
    ) -> Result<usize> {
        let mut served = 0;
        let total = refs.len();
        for (index, reference) in refs.iter().enumerate() {
            self.emit_progress(peer_id, "send", reference.kind, index + 1, total);
            let Some(resource) = manifest
                .resources
                .iter()
                .find(|resource| &resource.reference == reference)
            else {
                write_frame(
                    send,
                    &Message::ResourceEnd {
                        resource: reference.clone(),
                    },
                    &[],
                )
                .await?;
                continue;
            };
            write_frame(
                send,
                &Message::ResourceStart {
                    resource: resource.reference.clone(),
                    hash: resource.hash.clone(),
                    payload: resource.payload.clone(),
                },
                &[],
            )
            .await?;

            match resource.reference.kind {
                ResourceType::Book => {
                    if let Ok((book, _)) = manifest::book_from_payload(&resource.payload) {
                        let content = crate::storage::AssetRef {
                            kind: crate::storage::AssetKind::Book,
                            hash: book.content_hash.clone(),
                            extension: book.content_ext.clone(),
                        };
                        let content_path = self
                            .storage
                            .assets
                            .path(&content)
                            .ok()
                            .filter(|path| path.is_file())
                            .or_else(|| {
                                // Tejido y las transcripciones guardan el EPUB en
                                // generated_books; el receptor siempre lo recibe
                                // como libro normal.
                                self.storage
                                    .assets
                                    .path(&crate::storage::AssetRef {
                                        kind: crate::storage::AssetKind::GeneratedBook,
                                        hash: book.content_hash.clone(),
                                        extension: book.content_ext.clone(),
                                    })
                                    .ok()
                                    .filter(|path| path.is_file())
                            });
                        if let Some(path) = content_path {
                            self.serve_asset(
                                send,
                                &resource.reference,
                                "content",
                                &path,
                                &book.content_hash,
                            )
                            .await?;
                        }
                        if let (Some(hash), Some(extension)) = (&book.cover_hash, &book.cover_ext) {
                            let cover = crate::storage::AssetRef {
                                kind: crate::storage::AssetKind::Cover,
                                hash: hash.clone(),
                                extension: extension.clone(),
                            };
                            if let Ok(path) = self.storage.assets.path(&cover) {
                                self.serve_asset(send, &resource.reference, "cover", &path, hash)
                                    .await?;
                            }
                        }
                    }
                }
                ResourceType::Audiobook => {
                    if let Ok(audiobook) = manifest::audiobook_from_payload(&resource.payload) {
                        if let (Some(hash), Some(extension)) =
                            (&audiobook.audio_hash, &audiobook.audio_ext)
                        {
                            let audio = crate::storage::AssetRef {
                                kind: crate::storage::AssetKind::Audiobook,
                                hash: hash.clone(),
                                extension: extension.clone(),
                            };
                            if let Ok(path) = self.storage.assets.path(&audio) {
                                self.serve_asset(send, &resource.reference, "audio", &path, hash)
                                    .await?;
                            }
                        }
                        if let (Some(hash), Some(extension)) =
                            (&audiobook.manifest_hash, &audiobook.manifest_ext)
                        {
                            let manifest_asset = crate::storage::AssetRef {
                                kind: crate::storage::AssetKind::Audiobook,
                                hash: hash.clone(),
                                extension: extension.clone(),
                            };
                            if let Ok(path) = self.storage.assets.path(&manifest_asset) {
                                self.serve_asset(
                                    send,
                                    &resource.reference,
                                    "manifest",
                                    &path,
                                    hash,
                                )
                                .await?;
                            }
                        }
                    }
                }
                ResourceType::VoiceNote => {
                    let hash = resource
                        .payload
                        .get("audioHash")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let path =
                        crate::annotations::note_audio_path(self.storage.layout(), &reference.id);
                    if path.is_file() {
                        self.serve_asset(send, &resource.reference, "audio", &path, &hash)
                            .await?;
                    }
                }
                ResourceType::Annotation
                | ResourceType::Note
                | ResourceType::Extract
                | ResourceType::Reaction => {
                    if let Some(document) = self
                        .storage
                        .crdt_documents
                        .get(reference.kind, &reference.id)
                        .await?
                    {
                        self.serve_bytes(
                            send,
                            &resource.reference,
                            "crdt",
                            &document.snapshot,
                            &document.hash,
                        )
                        .await?;
                    }
                }
                _ => {}
            }
            write_frame(
                send,
                &Message::ResourceEnd {
                    resource: reference.clone(),
                },
                &[],
            )
            .await?;
            served += 1;
        }
        write_frame(send, &Message::PullDone, &[]).await?;
        Ok(served)
    }

    async fn serve_asset(
        &self,
        send: &mut quinn::SendStream,
        resource: &ResourceRef,
        asset: &str,
        path: &std::path::Path,
        hash: &str,
    ) -> Result<()> {
        use tokio::io::AsyncReadExt;
        let metadata = match tokio::fs::metadata(path).await {
            Ok(metadata) if metadata.is_file() => metadata,
            _ => return Ok(()),
        };
        write_frame(
            send,
            &Message::AssetStart {
                resource: resource.clone(),
                asset: asset.to_string(),
                hash: hash.to_string(),
                size: metadata.len(),
            },
            &[],
        )
        .await?;
        let mut file = tokio::fs::File::open(path)
            .await
            .map_err(|error| KoraError::Internal(format!("abrir asset: {error}")))?;
        let mut offset = 0u64;
        let mut buffer = vec![0u8; super::protocol::ASSET_CHUNK];
        loop {
            let read = file
                .read(&mut buffer)
                .await
                .map_err(|error| KoraError::Internal(format!("leer asset: {error}")))?;
            if read == 0 {
                break;
            }
            write_frame(
                send,
                &Message::AssetChunk {
                    resource: resource.clone(),
                    asset: asset.to_string(),
                    offset,
                },
                &buffer[..read],
            )
            .await?;
            offset += read as u64;
        }
        write_frame(
            send,
            &Message::AssetEnd {
                resource: resource.clone(),
                asset: asset.to_string(),
            },
            &[],
        )
        .await?;
        Ok(())
    }

    async fn serve_bytes(
        &self,
        send: &mut quinn::SendStream,
        resource: &ResourceRef,
        asset: &str,
        bytes: &[u8],
        hash: &str,
    ) -> Result<()> {
        write_frame(
            send,
            &Message::AssetStart {
                resource: resource.clone(),
                asset: asset.to_string(),
                hash: hash.to_string(),
                size: bytes.len() as u64,
            },
            &[],
        )
        .await?;
        for (index, chunk) in bytes.chunks(super::protocol::ASSET_CHUNK).enumerate() {
            write_frame(
                send,
                &Message::AssetChunk {
                    resource: resource.clone(),
                    asset: asset.to_string(),
                    offset: (index * super::protocol::ASSET_CHUNK) as u64,
                },
                chunk,
            )
            .await?;
        }
        write_frame(
            send,
            &Message::AssetEnd {
                resource: resource.clone(),
                asset: asset.to_string(),
            },
            &[],
        )
        .await?;
        Ok(())
    }

    async fn recv_resources(
        &self,
        _send: &mut quinn::SendStream,
        recv: &mut quinn::RecvStream,
        peer_id: &str,
    ) -> Result<usize> {
        use tokio::io::AsyncWriteExt;

        const MAX_ASSET_BYTES: u64 = 4 * 1024 * 1024 * 1024;

        let mut current: Option<IncomingResource> = None;
        let mut received = 0;
        loop {
            let Some((message, binary)) = read_frame(recv).await? else {
                return Err(KoraError::InvalidInput(
                    "conexión cerrada durante la sincronización".into(),
                ));
            };
            match message {
                Message::ResourceStart {
                    resource,
                    hash: _,
                    payload,
                } => {
                    current = Some(IncomingResource {
                        resource,
                        payload,
                        assets: HashMap::new(),
                        pending: HashMap::new(),
                    });
                }
                Message::AssetStart {
                    resource,
                    asset,
                    hash,
                    size,
                } => {
                    let current = current.as_mut().ok_or_else(|| {
                        KoraError::InvalidInput("asset sin recurso actual".into())
                    })?;
                    if current.resource != resource {
                        return Err(KoraError::InvalidInput(
                            "asset de un recurso distinto".into(),
                        ));
                    }
                    if size > MAX_ASSET_BYTES {
                        return Err(KoraError::Unsupported(
                            "el asset supera el límite de 4 GB".into(),
                        ));
                    }
                    let path = self
                        .storage
                        .layout()
                        .assets
                        .join(format!(".incoming-{}", uuid::Uuid::new_v4()));
                    if let Some(parent) = path.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    let file = tokio::fs::File::create(&path).await?;
                    current.pending.insert(
                        asset,
                        PendingAsset {
                            hash,
                            size,
                            written: 0,
                            path,
                            file: Some(file),
                            verified: false,
                        },
                    );
                }
                Message::AssetChunk {
                    resource,
                    asset,
                    offset: _,
                } => {
                    let current = current.as_mut().ok_or_else(|| {
                        KoraError::InvalidInput("chunk sin recurso actual".into())
                    })?;
                    if current.resource != resource {
                        return Err(KoraError::InvalidInput(
                            "chunk de un recurso distinto".into(),
                        ));
                    }
                    let pending = current
                        .pending
                        .get_mut(&asset)
                        .ok_or_else(|| KoraError::InvalidInput("chunk sin AssetStart".into()))?;
                    pending.written = pending
                        .written
                        .checked_add(binary.len() as u64)
                        .ok_or_else(|| KoraError::InvalidInput("asset demasiado grande".into()))?;
                    if pending.written > pending.size {
                        return Err(KoraError::InvalidInput(
                            "el asset excede el tamaño anunciado".into(),
                        ));
                    }
                    let file = pending
                        .file
                        .as_mut()
                        .ok_or_else(|| KoraError::Internal("asset sin archivo temporal".into()))?;
                    file.write_all(&binary).await?;
                }
                Message::AssetEnd { resource, asset } => {
                    let current = current.as_mut().ok_or_else(|| {
                        KoraError::InvalidInput("fin de asset sin recurso".into())
                    })?;
                    if current.resource != resource {
                        return Err(KoraError::InvalidInput(
                            "fin de asset de otro recurso".into(),
                        ));
                    }
                    let mut pending = current
                        .pending
                        .remove(&asset)
                        .ok_or_else(|| KoraError::InvalidInput("fin de asset sin inicio".into()))?;
                    if pending.written != pending.size {
                        return Err(KoraError::InvalidInput("el asset llegó incompleto".into()));
                    }
                    if let Some(mut file) = pending.file.take() {
                        file.flush().await?;
                    }
                    let path_for_hash = pending.path.clone();
                    let expected = pending.hash.clone();
                    let hash = tokio::task::spawn_blocking(move || {
                        crate::storage::hash::sha256_file(&path_for_hash)
                    })
                    .await
                    .map_err(|error| {
                        KoraError::Internal(format!("tarea de hash del asset: {error}"))
                    })??;
                    if hash != expected {
                        return Err(KoraError::Conflict("el hash del asset no coincide".into()));
                    }
                    pending.verified = true;
                    current.assets.insert(
                        asset,
                        ReceivedAsset {
                            path: pending.path.clone(),
                            hash,
                        },
                    );
                }
                Message::ResourceEnd { resource } => {
                    let Some(current) = current.take() else {
                        continue;
                    };
                    if current.resource != resource {
                        return Err(KoraError::InvalidInput("fin de recurso inesperado".into()));
                    }
                    self.apply_resource(
                        peer_id,
                        &current.resource,
                        &current.payload,
                        &current.assets,
                    )
                    .await?;
                    received += 1;
                    self.emit_progress(peer_id, "receive", current.resource.kind, received, 0);
                }
                Message::PullDone => return Ok(received),
                Message::Error { message } => return Err(KoraError::Conflict(message)),
                other => {
                    return Err(KoraError::InvalidInput(format!(
                        "mensaje inesperado durante la descarga: {other:?}"
                    )));
                }
            }
        }
    }

    async fn apply_resource(
        &self,
        peer_id: &str,
        resource: &ResourceRef,
        payload: &Value,
        assets: &HashMap<String, ReceivedAsset>,
    ) -> Result<()> {
        match resource.kind {
            ResourceType::Category => {
                let mut category: crate::storage::Category = serde_json::from_value(
                    payload
                        .get("category")
                        .cloned()
                        .ok_or_else(|| KoraError::InvalidInput("categoría sin datos".into()))?,
                )?;
                category.owner_account_id = self.owner_account_id.clone();
                category.origin_share_id = None;
                // Las categorías iniciales se crean con ids distintos en cada
                // dispositivo, pero comparten nombre. La restricción
                // (owner_account_id, name) impide insertar el duplicado, así
                // que se reutiliza la categoría local y se guarda el alias
                // para reescribir las asignaciones de los libros.
                if let Some(local) = self
                    .storage
                    .categories
                    .find_by_name(&self.owner_account_id, &category.name)
                    .await?
                    && local.id != category.id
                {
                    self.storage
                        .sync_aliases
                        .put(peer_id, ResourceType::Category, &category.id, &local.id)
                        .await?;
                } else {
                    self.storage
                        .categories
                        .upsert_from_sync(&category, &self.owner_account_id)
                        .await?;
                }
            }
            ResourceType::Book => {
                let (mut book, category_ids) = manifest::book_from_payload(payload)?;
                let mut local_id = book.id.clone();
                if let Some(existing) = self
                    .storage
                    .books
                    .find_by_hash(&self.owner_account_id, &book.content_hash)
                    .await?
                    && existing.id != book.id
                {
                    self.storage
                        .sync_aliases
                        .put(peer_id, ResourceType::Book, &book.id, &existing.id)
                        .await?;
                    local_id = existing.id;
                }
                if let Some(content) = received_bytes(assets, "content")? {
                    let asset = self.storage.assets.put_bytes(
                        crate::storage::AssetKind::Book,
                        &content,
                        &book.content_ext,
                    )?;
                    book.content_hash = asset.hash;
                    book.content_ext = asset.extension;
                    book.size_bytes = content.len() as i64;
                } else {
                    let reference = crate::storage::AssetRef {
                        kind: crate::storage::AssetKind::Book,
                        hash: book.content_hash.clone(),
                        extension: book.content_ext.clone(),
                    };
                    let generated = crate::storage::AssetRef {
                        kind: crate::storage::AssetKind::GeneratedBook,
                        hash: book.content_hash.clone(),
                        extension: book.content_ext.clone(),
                    };
                    if !self.storage.assets.exists(&reference)?
                        && !self.storage.assets.exists(&generated)?
                    {
                        return Err(KoraError::NotFound(
                            "falta el archivo del libro sincronizado".into(),
                        ));
                    }
                }
                if let Some(cover) = received_bytes(assets, "cover")? {
                    let extension = book.cover_ext.as_deref().unwrap_or("png");
                    let asset = self.storage.assets.put_bytes(
                        crate::storage::AssetKind::Cover,
                        &cover,
                        extension,
                    )?;
                    book.cover_hash = Some(asset.hash);
                    book.cover_ext = Some(asset.extension);
                }
                book.id = local_id;
                book.owner_account_id = self.owner_account_id.clone();
                book.origin_share_id = None;
                self.storage
                    .books
                    .upsert_from_sync(&book, &self.owner_account_id)
                    .await?;

                let mut valid_categories = Vec::new();
                for category_id in category_ids {
                    let local_category = self
                        .storage
                        .sync_aliases
                        .get(peer_id, ResourceType::Category, &category_id)
                        .await?
                        .unwrap_or(category_id);
                    if self
                        .storage
                        .categories
                        .get(&local_category)
                        .await?
                        .is_some()
                    {
                        valid_categories.push(local_category);
                    }
                }
                self.storage
                    .book_categories
                    .replace_for_book(&book.id, &valid_categories, &self.owner_account_id)
                    .await?;
            }
            ResourceType::Audiobook => {
                let mut audiobook = manifest::audiobook_from_payload(payload)?;
                if let Some(local_book) = self
                    .storage
                    .sync_aliases
                    .get(peer_id, ResourceType::Book, &audiobook.book_id)
                    .await?
                {
                    audiobook.book_id = local_book;
                }
                if self.storage.books.get(&audiobook.book_id).await?.is_none() {
                    return Err(KoraError::NotFound(format!(
                        "falta el libro del audiolibro compartido {}",
                        audiobook.book_id
                    )));
                }
                if let Some(voice_id) = audiobook.voice_id.clone()
                    && self.storage.voices.get(&voice_id).await?.is_none()
                {
                    audiobook.voice_id = None;
                }

                if let Some(audio) = assets.get("audio") {
                    let extension = audiobook.audio_ext.clone().unwrap_or_else(|| "m4b".into());
                    let store = self.storage.assets.clone();
                    let source = audio.path.clone();
                    let hash = audio.hash.clone();
                    let asset = tokio::task::spawn_blocking(move || {
                        store.adopt_file(
                            crate::storage::AssetKind::Audiobook,
                            &source,
                            &extension,
                            &hash,
                        )
                    })
                    .await
                    .map_err(|error| {
                        KoraError::Internal(format!("tarea de audio del audiolibro: {error}"))
                    })??;
                    audiobook.audio_hash = Some(asset.hash);
                    audiobook.audio_ext = Some(asset.extension);
                } else if let (Some(hash), Some(extension)) =
                    (&audiobook.audio_hash, &audiobook.audio_ext)
                {
                    let reference = crate::storage::AssetRef {
                        kind: crate::storage::AssetKind::Audiobook,
                        hash: hash.clone(),
                        extension: extension.clone(),
                    };
                    if !self.storage.assets.exists(&reference)? {
                        return Err(KoraError::NotFound(
                            "falta el audio del audiolibro compartido".into(),
                        ));
                    }
                } else {
                    return Err(KoraError::NotFound(
                        "falta el audio del audiolibro compartido".into(),
                    ));
                }

                if let Some(manifest_asset) = assets.get("manifest") {
                    let extension = audiobook
                        .manifest_ext
                        .clone()
                        .unwrap_or_else(|| "json".into());
                    let asset = self.storage.assets.adopt_file(
                        crate::storage::AssetKind::Audiobook,
                        &manifest_asset.path,
                        &extension,
                        &manifest_asset.hash,
                    )?;
                    audiobook.manifest_hash = Some(asset.hash);
                    audiobook.manifest_ext = Some(asset.extension);
                } else if let (Some(hash), Some(extension)) =
                    (&audiobook.manifest_hash, &audiobook.manifest_ext)
                {
                    let reference = crate::storage::AssetRef {
                        kind: crate::storage::AssetKind::Audiobook,
                        hash: hash.clone(),
                        extension: extension.clone(),
                    };
                    if !self.storage.assets.exists(&reference)? {
                        return Err(KoraError::NotFound(
                            "falta el manifiesto del audiolibro compartido".into(),
                        ));
                    }
                } else {
                    return Err(KoraError::NotFound(
                        "falta el manifiesto del audiolibro compartido".into(),
                    ));
                }

                audiobook.owner_account_id = self.owner_account_id.clone();
                audiobook.origin_share_id = None;
                self.storage
                    .audiobooks
                    .upsert_from_sync(&audiobook, &self.owner_account_id)
                    .await?;
            }
            ResourceType::ReadingProgress => {
                let mut progress: crate::storage::ReadingProgress = serde_json::from_value(
                    payload
                        .get("progress")
                        .cloned()
                        .ok_or_else(|| KoraError::InvalidInput("progreso sin datos".into()))?,
                )?;
                if let Some(local_id) = self
                    .storage
                    .sync_aliases
                    .get(peer_id, ResourceType::Book, &progress.book_id)
                    .await?
                {
                    progress.book_id = local_id;
                }
                let should_apply = match self.storage.progress.get(&progress.book_id).await? {
                    Some(local) => progress.updated_at > local.updated_at,
                    None => true,
                };
                if should_apply {
                    self.storage
                        .progress
                        .upsert_from_sync(
                            &progress.book_id,
                            &progress.locator,
                            progress.percent,
                            progress.updated_at,
                            &self.owner_account_id,
                        )
                        .await?;
                }
            }
            ResourceType::Annotation
            | ResourceType::Note
            | ResourceType::Extract
            | ResourceType::Reaction => {
                let snapshot = received_bytes(assets, "crdt")?
                    .ok_or_else(|| KoraError::InvalidInput("anotación sin snapshot CRDT".into()))?;
                self.crdt
                    .apply(peer_id, resource.kind, &resource.id, &snapshot)
                    .await?;
            }
            ResourceType::VoiceNote => {
                let snapshot = received_bytes(assets, "crdt")?.ok_or_else(|| {
                    KoraError::InvalidInput("nota de voz sin snapshot CRDT".into())
                })?;
                self.crdt
                    .apply(peer_id, resource.kind, &resource.id, &snapshot)
                    .await?;
                if let Some(audio) = received_bytes(assets, "audio")? {
                    let path =
                        crate::annotations::note_audio_path(self.storage.layout(), &resource.id);
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&path, &audio)?;
                    let hash = crate::storage::hash::sha256_bytes(&audio);
                    self.storage
                        .voice_notes
                        .set_audio_hash(&resource.id, &hash)
                        .await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn mark_synced(&self, peer_id: &str, resources: &[SyncResource]) -> Result<()> {
        for resource in resources {
            self.storage
                .sync_state
                .upsert(crate::storage::NewSyncEntry {
                    resource_type: resource.reference.kind,
                    resource_id: resource.reference.id.clone(),
                    peer_device_id: peer_id.to_string(),
                    version: resource.updated_at.timestamp_millis(),
                    hash: Some(resource.hash.clone()),
                    state: SyncState::Synced,
                })
                .await?;
            self.storage
                .sync_pending
                .clear(resource.reference.kind, &resource.reference.id)
                .await?;
        }
        Ok(())
    }

    async fn send_handoff(&self, handoff: Handoff) -> Result<()> {
        let peers = self
            .storage
            .devices
            .list_trusted(&self.owner_account_id)
            .await?;
        let endpoint = match self.endpoint.lock().await.clone() {
            Some(endpoint) => endpoint,
            None => return Ok(()),
        };
        for peer in peers {
            let Ok(candidates) = self.peer_addresses(&peer.id).await else {
                continue;
            };
            let Ok((connection, _, _)) = transport::connect_any(
                &endpoint,
                &candidates,
                peer.certificate_fingerprint.as_deref(),
            )
            .await
            else {
                continue;
            };
            if let Ok((mut send, mut recv)) = connection.open_bi().await {
                let hello_sent = write_frame(
                    &mut send,
                    &Message::Hello(self.hello(Purpose::Handoff)),
                    &[],
                )
                .await
                .is_ok();
                let handoff_sent = hello_sent
                    && write_frame(&mut send, &Message::Handoff(handoff.clone()), &[])
                        .await
                        .is_ok();
                if handoff_sent {
                    let _ = tokio::time::timeout(
                        std::time::Duration::from_secs(2),
                        read_frame(&mut recv),
                    )
                    .await;
                }
            }
            connection.close(0u32.into(), b"handoff");
        }
        Ok(())
    }

    async fn broadcast_mirror(&self, event: MirrorEvent) {
        let message = match event {
            MirrorEvent::Position {
                book_id,
                chapter,
                offset,
            } => Message::MirrorPosition {
                book_id: book_id.clone(),
                chapter,
                offset,
            },
            MirrorEvent::Tts {
                book_id,
                section,
                state,
                offset,
                end,
            } => Message::MirrorTts {
                book_id: book_id.clone(),
                section,
                state: mirror_state(&state),
                offset,
                end,
            },
        };
        let book_id = match &message {
            Message::MirrorPosition { book_id, .. } | Message::MirrorTts { book_id, .. } => {
                book_id.clone()
            }
            _ => return,
        };
        let targets: Vec<(String, Arc<Mutex<quinn::SendStream>>)> = {
            let sessions = self.mirror_sessions.lock().await;
            sessions
                .iter()
                .filter(|(_, peer)| peer.book_id == book_id)
                .map(|(id, peer)| (id.clone(), peer.send.clone()))
                .collect()
        };
        let mut failed = Vec::new();
        for (device_id, send) in targets {
            let mut send = send.lock().await;
            if write_frame(&mut *send, &message, &[]).await.is_err() {
                failed.push(device_id);
            }
        }
        if !failed.is_empty() {
            let mut sessions = self.mirror_sessions.lock().await;
            for device_id in failed {
                sessions.remove(&device_id);
            }
        }
    }

    async fn mirror_reader(
        &self,
        _connection: quinn::Connection,
        mut recv: quinn::RecvStream,
        _book_id: String,
    ) {
        loop {
            match read_frame(&mut recv).await {
                Ok(Some((
                    Message::MirrorPosition {
                        book_id,
                        chapter,
                        offset,
                    },
                    _,
                ))) => {
                    self.events.emit(
                        "mirror::position",
                        json!({ "book_id": book_id, "chapter": chapter, "offset": offset }),
                    );
                }
                Ok(Some((
                    Message::MirrorTts {
                        book_id,
                        section,
                        state,
                        offset,
                        end,
                    },
                    _,
                ))) => {
                    self.events.emit(
                        "mirror::tts_state",
                        json!({
                            "book_id": book_id,
                            "section": section,
                            "state": mirror_state_str(state),
                            "offset": offset,
                            "end": end
                        }),
                    );
                }
                Ok(Some((Message::MirrorStop { .. }, _))) | Ok(None) => break,
                Ok(Some(_)) => {}
                Err(_) => break,
            }
        }
        let mut status = self.mirror_status.write().await;
        if !status.hosting {
            *status = MirrorStatus::default();
        }
    }
}

fn diff(remote: &Manifest, local: &Manifest) -> Vec<ResourceRef> {
    let local_hashes: HashMap<&ResourceRef, &str> = local
        .resources
        .iter()
        .map(|resource| (&resource.reference, resource.hash.as_str()))
        .collect();
    remote
        .resources
        .iter()
        .filter(|resource| {
            local_hashes.get(&resource.reference).copied() != Some(resource.hash.as_str())
        })
        .map(|resource| resource.reference.clone())
        .collect()
}

fn mirror_state(state: &str) -> MirrorTtsState {
    match state {
        "playing" => MirrorTtsState::Playing,
        "paused" => MirrorTtsState::Paused,
        "finished" => MirrorTtsState::Finished,
        _ => MirrorTtsState::Stopped,
    }
}

fn mirror_state_str(state: MirrorTtsState) -> &'static str {
    match state {
        MirrorTtsState::Playing => "playing",
        MirrorTtsState::Paused => "paused",
        MirrorTtsState::Stopped => "stopped",
        MirrorTtsState::Finished => "finished",
    }
}

async fn reject_pairing(send: &mut quinn::SendStream, reason: &str, retryable: bool) {
    let _ = write_frame(
        send,
        &Message::PairRejected {
            reason: reason.to_string(),
            retryable,
        },
        &[],
    )
    .await;
}

fn parse_candidate(value: &str, default_port: u16) -> Option<SocketAddr> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(address) = value.parse::<SocketAddr>() {
        return Some(address);
    }
    if let Some(inner) = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        && let Ok(ip) = inner.parse::<IpAddr>()
    {
        return Some(SocketAddr::new(ip, default_port));
    }
    if value.contains('%')
        && let Some((host, scope)) = value.split_once('%')
        && let (Ok(ip), Ok(scope)) = (host.parse::<Ipv6Addr>(), scope.parse::<u32>())
    {
        return Some(SocketAddr::V6(SocketAddrV6::new(
            ip,
            default_port,
            0,
            scope,
        )));
    }
    if value.matches(':').count() == 1
        && let Some((host, port)) = value.rsplit_once(':')
        && let (Ok(ip), Ok(port)) = (host.parse::<IpAddr>(), port.parse::<u16>())
    {
        return Some(SocketAddr::new(ip, port));
    }
    value
        .parse::<IpAddr>()
        .ok()
        .map(|ip| SocketAddr::new(ip, default_port))
}

fn dedupe_addresses(addresses: Vec<SocketAddr>) -> Result<Vec<SocketAddr>> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for address in addresses {
        if seen.insert(address) {
            result.push(address);
        }
    }
    if result.is_empty() {
        return Err(KoraError::NotFound(
            "no se encontró una dirección válida para el dispositivo".into(),
        ));
    }
    Ok(result)
}

fn key_from_uri(input: &str, expected_kind: &str) -> String {
    if let Some((kind, params)) = crypto::parse_uri(input)
        && kind == expected_kind
        && let Some(key) = crypto::param(&params, "k")
    {
        return key.to_string();
    }
    input.trim().to_string()
}

fn pairing_transcript(
    host: &Hello,
    client: &Hello,
    message_a: Option<&[u8]>,
    message_b: &[u8],
) -> Vec<u8> {
    let mut hasher = Sha256::new();
    if let Ok(bytes) = serde_json::to_vec(host) {
        hasher.update(bytes);
    }
    if let Ok(bytes) = serde_json::to_vec(client) {
        hasher.update(bytes);
    }
    if let Some(message_a) = message_a {
        hasher.update(message_a);
    }
    hasher.update(message_b);
    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::TestStorage;
    use crate::storage::{
        AnnotationKind, AssetKind, BookFormat, BookOrigin, NewAnnotation, NewBook,
    };

    async fn service(test: &TestStorage, identity: &LocalIdentity) -> Result<Arc<SyncService>> {
        let device =
            DeviceIdentity::load_or_create(&test.storage, identity, test.storage.layout()).await?;
        let accounts = Arc::new(AccountService::new(test.storage.clone(), device));
        let service = SyncService::new(
            test.storage.clone(),
            identity,
            test.storage.layout().clone(),
            Arc::new(NullEventSink),
            MirrorHub::new(),
            accounts,
        )
        .await?;
        service.start().await?;
        Ok(service)
    }

    async fn trust(owner: &str, storage: &Storage, peer: &SyncService) -> Result<()> {
        storage
            .devices
            .upsert_remote(
                owner,
                peer.device_id(),
                "Peer",
                "linux",
                Some(peer.public_key()),
                peer.fingerprint(),
                Some("127.0.0.1"),
                Some(peer.listen_port() as i64),
            )
            .await?;
        Ok(())
    }

    struct RecordingSink {
        events: std::sync::Mutex<Vec<(String, Value)>>,
    }

    impl RecordingSink {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                events: std::sync::Mutex::new(Vec::new()),
            })
        }

        fn has(&self, name: &str) -> bool {
            self.events
                .lock()
                .map(|events| events.iter().any(|(event, _)| event == name))
                .unwrap_or(false)
        }
    }

    impl EventSink for RecordingSink {
        fn emit(&self, event: &str, payload: Value) {
            if let Ok(mut events) = self.events.lock() {
                events.push((event.to_string(), payload));
            }
        }
    }

    async fn service_with(
        test: &TestStorage,
        identity: &LocalIdentity,
        events: Arc<dyn EventSink>,
        mirror: MirrorHub,
    ) -> Result<Arc<SyncService>> {
        let device =
            DeviceIdentity::load_or_create(&test.storage, identity, test.storage.layout()).await?;
        let accounts = Arc::new(AccountService::new(test.storage.clone(), device));
        let service = SyncService::new(
            test.storage.clone(),
            identity,
            test.storage.layout().clone(),
            events,
            mirror,
            accounts,
        )
        .await?;
        service.start().await?;
        Ok(service)
    }

    async fn wait_for<F: Fn() -> bool>(check: F) -> bool {
        for _ in 0..80 {
            if check() {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        false
    }

    async fn seed_book(
        storage: &Storage,
        owner: &str,
        title: &str,
    ) -> Result<crate::storage::Book> {
        let content = storage
            .assets
            .put_bytes(AssetKind::Book, b"contenido", "epub")?;
        storage
            .books
            .create(NewBook {
                title: title.to_string(),
                author: Some("Kora".to_string()),
                format: BookFormat::Epub,
                content_hash: content.hash,
                content_ext: "epub".to_string(),
                original_name: "libro.epub".to_string(),
                cover_hash: None,
                cover_ext: None,
                size_bytes: 9,
                language: Some("es".to_string()),
                publisher: None,
                description: None,
                origin: BookOrigin::Imported,
                owner_account_id: owner.to_string(),
                origin_share_id: None,
                audio_only: false,
            })
            .await
    }

    #[tokio::test]
    async fn handoff_and_mirror_reach_the_peer() -> Result<()> {
        let (left, left_identity) = TestStorage::with_identity().await;
        let (right, right_identity) = TestStorage::with_identity().await;
        let left_events = RecordingSink::new();
        let right_events = RecordingSink::new();
        let hub = MirrorHub::new();
        let left_service =
            service_with(&left, &left_identity, left_events.clone(), hub.clone()).await?;
        let right_service = service_with(
            &right,
            &right_identity,
            right_events.clone(),
            MirrorHub::new(),
        )
        .await?;
        trust(&left_identity.account.id, &left.storage, &right_service).await?;
        trust(&right_identity.account.id, &right.storage, &left_service).await?;
        let book = seed_book(&left.storage, &left_identity.account.id, "Espejo").await?;
        right_service.sync_now(left_service.device_id()).await?;

        left_service.handoff(&book.id, 3, 17, 0.25);
        assert!(
            wait_for(|| right_events.has("sync::handoff")).await,
            "no llegó el handoff"
        );

        let mirror = left_service.start_mirror(&book.id).await?;
        assert!(mirror.hosting);
        right_service
            .join_mirror(left_service.device_id(), &book.id)
            .await?;
        assert!(
            wait_for(|| {
                left_service
                    .inner
                    .mirror_status
                    .try_read()
                    .map(|status| status.followers.len() == 1)
                    .unwrap_or(false)
            })
            .await,
            "el seguidor no se registró"
        );

        left_service.position(&book.id, 4, 22);
        assert!(
            wait_for(|| right_events.has("mirror::position")).await,
            "no llegó la posición del espejo"
        );
        hub.tts(&book.id, 4, "playing", 22, None);
        assert!(
            wait_for(|| right_events.has("mirror::tts_state")).await,
            "no llegó el estado TTS del espejo"
        );

        left_service.stop_mirror().await?;
        left_service.stop().await?;
        right_service.stop().await?;
        Ok(())
    }

    #[tokio::test]
    async fn category_filter_excludes_books() -> Result<()> {
        let (left, left_identity) = TestStorage::with_identity().await;
        let (right, right_identity) = TestStorage::with_identity().await;
        let left_service = service(&left, &left_identity).await?;
        let right_service = service(&right, &right_identity).await?;
        trust(&left_identity.account.id, &left.storage, &right_service).await?;
        trust(&right_identity.account.id, &right.storage, &left_service).await?;
        let book = seed_book(&left.storage, &left_identity.account.id, "Filtrado").await?;
        let category = left
            .storage
            .categories
            .create(crate::storage::NewCategory {
                name: "Otra".to_string(),
                color: None,
                sort_order: 0,
                owner_account_id: left_identity.account.id.clone(),
                origin_share_id: None,
            })
            .await?;
        left.storage
            .sync_collections
            .replace(
                &left_identity.account.id,
                right_service.device_id(),
                std::slice::from_ref(&category.id),
            )
            .await?;

        right_service.sync_now(left_service.device_id()).await?;
        assert!(right.storage.books.get(&book.id).await?.is_none());
        assert!(right.storage.categories.get(&category.id).await?.is_some());

        left.storage
            .sync_collections
            .replace(&left_identity.account.id, right_service.device_id(), &[])
            .await?;
        right_service.sync_now(left_service.device_id()).await?;
        assert!(right.storage.books.get(&book.id).await?.is_some());

        left_service.stop().await?;
        right_service.stop().await?;
        Ok(())
    }

    #[tokio::test]
    async fn duplicate_category_names_map_to_the_local_category() -> Result<()> {
        let (left, left_identity) = TestStorage::with_identity().await;
        let (right, right_identity) = TestStorage::with_identity().await;
        let left_service = service(&left, &left_identity).await?;
        let right_service = service(&right, &right_identity).await?;
        trust(&left_identity.account.id, &left.storage, &right_service).await?;
        trust(&right_identity.account.id, &right.storage, &left_service).await?;

        // Cada dispositivo crea su propia categoría «Ficción» con un id
        // distinto, como ocurre con las categorías iniciales.
        let left_category = left
            .storage
            .categories
            .create(crate::storage::NewCategory {
                name: "Ficción".to_string(),
                color: Some("#b45309".to_string()),
                sort_order: 0,
                owner_account_id: left_identity.account.id.clone(),
                origin_share_id: None,
            })
            .await?;
        let right_category = right
            .storage
            .categories
            .create(crate::storage::NewCategory {
                name: "Ficción".to_string(),
                color: Some("#1d4ed8".to_string()),
                sort_order: 0,
                owner_account_id: right_identity.account.id.clone(),
                origin_share_id: None,
            })
            .await?;
        let book = seed_book(&left.storage, &left_identity.account.id, "Con categoría").await?;
        left.storage
            .book_categories
            .assign(&book.id, &left_category.id, &left_identity.account.id, None)
            .await?;

        right_service.sync_now(left_service.device_id()).await?;

        let categories = right
            .storage
            .categories
            .list_by_owner(&right_identity.account.id)
            .await?;
        assert_eq!(
            categories
                .iter()
                .filter(|category| category.name == "Ficción")
                .count(),
            1
        );
        let assigned = right.storage.book_categories.list_by_book(&book.id).await?;
        assert_eq!(assigned.len(), 1);
        assert_eq!(assigned[0].id, right_category.id);
        assert_eq!(
            right
                .storage
                .sync_aliases
                .get(
                    left_service.device_id(),
                    ResourceType::Category,
                    &left_category.id,
                )
                .await?,
            Some(right_category.id.clone())
        );

        left_service.stop().await?;
        right_service.stop().await?;
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_annotation_edits_converge() -> Result<()> {
        let (left, left_identity) = TestStorage::with_identity().await;
        let (right, right_identity) = TestStorage::with_identity().await;
        let left_service = service(&left, &left_identity).await?;
        let right_service = service(&right, &right_identity).await?;
        trust(&left_identity.account.id, &left.storage, &right_service).await?;
        trust(&right_identity.account.id, &right.storage, &left_service).await?;
        let book = seed_book(&left.storage, &left_identity.account.id, "CRDT").await?;
        let annotation = left
            .storage
            .annotations
            .create(NewAnnotation {
                book_id: book.id.clone(),
                kind: AnnotationKind::Highlight,
                locator: "chapter:0:0-4".to_string(),
                chapter_index: Some(0),
                offset_start: Some(0),
                offset_end: Some(4),
                text: Some("hola".to_string()),
                note: Some("inicial".to_string()),
                color: Some("#ffd166".to_string()),
                owner_account_id: left_identity.account.id.clone(),
                origin_share_id: None,
            })
            .await?;
        left_service
            .touch(ResourceType::Annotation, &annotation.id)
            .await;
        right_service.sync_now(left_service.device_id()).await?;

        let mut left_note = left.storage.annotations.get(&annotation.id).await?.unwrap();
        left_note.note = Some("desde izquierda".to_string());
        left.storage.annotations.update(&left_note).await?;
        left_service
            .touch(ResourceType::Annotation, &annotation.id)
            .await;

        let mut right_note = right
            .storage
            .annotations
            .get(&annotation.id)
            .await?
            .unwrap();
        right_note.note = Some("desde derecha".to_string());
        right.storage.annotations.update(&right_note).await?;
        right_service
            .touch(ResourceType::Annotation, &annotation.id)
            .await;

        right_service.sync_now(left_service.device_id()).await?;
        let left_note = left.storage.annotations.get(&annotation.id).await?.unwrap();
        let right_note = right
            .storage
            .annotations
            .get(&annotation.id)
            .await?
            .unwrap();
        assert_eq!(left_note.note, right_note.note);
        assert_eq!(left_note.text, right_note.text);

        left_service.stop().await?;
        right_service.stop().await?;
        Ok(())
    }

    #[tokio::test]
    async fn ticket_pairing_links_devices_without_account_keys() -> Result<()> {
        let (left, left_identity) = TestStorage::with_identity().await;
        let (right, right_identity) = TestStorage::with_identity().await;
        let left_service = service(&left, &left_identity).await?;
        let right_service = service(&right, &right_identity).await?;

        let host = left_service.start_device_pairing().await?;
        let guest = right_service
            .join_device_pairing(JoinPairingInput {
                address: Some(format!("127.0.0.1:{}", left_service.listen_port())),
                code: Some(host.code),
                ..Default::default()
            })
            .await?;

        assert_eq!(guest.id, left_service.device_id());
        assert!(left_result_trusted(&left, right_service.device_id()).await?);
        assert!(left_result_trusted(&right, left_service.device_id()).await?);

        left_service.stop().await?;
        right_service.stop().await?;
        Ok(())
    }

    #[tokio::test]
    async fn mdns_pairing_discovers_and_pairs() -> Result<()> {
        let (left, left_identity) = TestStorage::with_identity().await;
        let (right, right_identity) = TestStorage::with_identity().await;
        let left_service = service(&left, &left_identity).await?;
        let right_service = service(&right, &right_identity).await?;

        let host = left_service.start_device_pairing().await?;
        let mut discovered = false;
        for _ in 0..60 {
            if right_service
                .peers()
                .await?
                .iter()
                .any(|peer| peer.id == left_service.device_id() && peer.purpose == "pair")
            {
                discovered = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        if !discovered {
            // Algunos entornos de CI bloquean multicast; el fallback manual
            // sigue cubierto por el test anterior.
            left_service.stop().await?;
            right_service.stop().await?;
            return Ok(());
        }

        let guest = right_service
            .join_device_pairing(JoinPairingInput {
                peer_id: Some(left_service.device_id().to_string()),
                // En CI el anuncio mDNS puede resolver solo direcciones de
                // loopback, que `peer_from_service` descarta. El peer_id ya
                // validó el descubrimiento; la dirección local mantiene el
                // test determinista.
                address: Some(format!("127.0.0.1:{}", left_service.listen_port())),
                code: Some(host.code),
                ..Default::default()
            })
            .await?;
        assert_eq!(guest.id, left_service.device_id());
        assert!(left_result_trusted(&left, right_service.device_id()).await?);
        assert!(left_result_trusted(&right, left_service.device_id()).await?);

        left_service.stop().await?;
        right_service.stop().await?;
        Ok(())
    }

    #[tokio::test]
    async fn wrong_pairing_code_keeps_the_ticket_alive() -> Result<()> {
        let (left, left_identity) = TestStorage::with_identity().await;
        let (right, right_identity) = TestStorage::with_identity().await;
        let left_service = service(&left, &left_identity).await?;
        let right_service = service(&right, &right_identity).await?;

        let host = left_service.start_device_pairing().await?;
        let wrong = if host.code == "000000" {
            "000001"
        } else {
            "000000"
        };
        let error = right_service
            .join_device_pairing(JoinPairingInput {
                address: Some(format!("127.0.0.1:{}", left_service.listen_port())),
                code: Some(wrong.to_string()),
                ..Default::default()
            })
            .await
            .expect_err("el código incorrecto debe fallar");
        assert!(error.to_string().contains("código"));

        let guest = right_service
            .join_device_pairing(JoinPairingInput {
                address: Some(format!("127.0.0.1:{}", left_service.listen_port())),
                code: Some(host.code),
                ..Default::default()
            })
            .await?;
        assert_eq!(guest.id, left_service.device_id());
        assert!(left_result_trusted(&left, right_service.device_id()).await?);

        left_service.stop().await?;
        right_service.stop().await?;
        Ok(())
    }

    async fn left_result_trusted(storage: &TestStorage, device_id: &str) -> Result<bool> {
        Ok(storage
            .storage
            .devices
            .get(device_id)
            .await?
            .map(|device| device.trusted)
            .unwrap_or(false))
    }

    #[tokio::test]
    async fn share_copy_and_read_only() -> Result<()> {
        let (left, left_identity) = TestStorage::with_identity().await;
        let (right, right_identity) = TestStorage::with_identity().await;
        let left_service = service(&left, &left_identity).await?;
        let right_service = service(&right, &right_identity).await?;
        left_service
            .inner
            .accounts
            .create_account("owner", "Owner", "1111")
            .await?;
        right_service
            .inner
            .accounts
            .create_account("guest", "Guest", "2222")
            .await?;
        left_service.refresh_account_cache().await;
        right_service.refresh_account_cache().await;
        right
            .storage
            .devices
            .upsert_remote(
                &right_identity.account.id,
                left_service.device_id(),
                "Owner",
                "linux",
                Some(left_service.public_key()),
                left_service.fingerprint(),
                Some("127.0.0.1"),
                Some(left_service.listen_port() as i64),
            )
            .await?;

        let book = seed_book(&left.storage, &left_identity.account.id, "Compartido").await?;
        let share = left_service
            .inner
            .accounts
            .create_share(
                "1111",
                &crate::accounts::ShareCreateRequest {
                    scope_type: ShareScopeType::Book,
                    resource_id: Some(book.id.clone()),
                    permission: SharePermission::Copy,
                    expires_at: None,
                    max_uses: Some(1),
                },
            )
            .await?;
        let key = share.key.clone().expect("share key");
        let received = right_service
            .accept_share("2222", &key, left_service.device_id())
            .await?;
        assert_eq!(received.share_id, share.share.id);
        assert_eq!(received.permission, SharePermission::Copy);
        let copied = right
            .storage
            .books
            .get(&book.id)
            .await?
            .expect("libro copiado");
        assert_eq!(copied.title, "Compartido");
        assert_eq!(copied.owner_account_id, right_identity.account.id);

        let read_share = left_service
            .inner
            .accounts
            .create_share(
                "1111",
                &crate::accounts::ShareCreateRequest {
                    scope_type: ShareScopeType::Book,
                    resource_id: Some(book.id.clone()),
                    permission: SharePermission::Read,
                    expires_at: None,
                    max_uses: Some(1),
                },
            )
            .await?;
        let read_key = read_share.key.clone().expect("share key");
        right_service
            .accept_share("2222", &read_key, left_service.device_id())
            .await?;
        let read_book = right
            .storage
            .books
            .list_read_only_by_share(&read_share.share.id)
            .await?;
        assert_eq!(read_book.len(), 1);
        assert_eq!(read_book[0].id, book.id);

        let second = right_service
            .accept_share("2222", &key, left_service.device_id())
            .await;
        assert!(second.is_err(), "max_uses debe impedir el segundo uso");

        left_service.stop().await?;
        right_service.stop().await?;
        Ok(())
    }

    #[tokio::test]
    async fn two_nodes_sync_book_and_annotation() -> Result<()> {
        let (left, left_identity) = TestStorage::with_identity().await;
        let (right, right_identity) = TestStorage::with_identity().await;
        let left_service = service(&left, &left_identity).await?;
        let right_service = service(&right, &right_identity).await?;

        trust(&left_identity.account.id, &left.storage, &right_service).await?;
        trust(&right_identity.account.id, &right.storage, &left_service).await?;

        let content =
            left.storage
                .assets
                .put_bytes(AssetKind::Book, b"contenido de prueba", "epub")?;
        let book = left
            .storage
            .books
            .create(NewBook {
                title: "Libro sincronizado".to_string(),
                author: Some("Kora".to_string()),
                format: BookFormat::Epub,
                content_hash: content.hash,
                content_ext: "epub".to_string(),
                original_name: "libro.epub".to_string(),
                cover_hash: None,
                cover_ext: None,
                size_bytes: 18,
                language: Some("es".to_string()),
                publisher: None,
                description: None,
                origin: BookOrigin::Imported,
                owner_account_id: left_identity.account.id.clone(),
                origin_share_id: None,
                audio_only: false,
            })
            .await?;
        let annotation = left
            .storage
            .annotations
            .create(NewAnnotation {
                book_id: book.id.clone(),
                kind: AnnotationKind::Highlight,
                locator: "chapter:0:0-4".to_string(),
                chapter_index: Some(0),
                offset_start: Some(0),
                offset_end: Some(4),
                text: Some("hola".to_string()),
                note: Some("nota sincronizada".to_string()),
                color: Some("#ffd166".to_string()),
                owner_account_id: left_identity.account.id.clone(),
                origin_share_id: None,
            })
            .await?;
        left_service
            .touch(ResourceType::Annotation, &annotation.id)
            .await;

        let outcome = right_service.sync_now(left_service.device_id()).await?;
        assert_eq!(outcome.peer_id, left_service.device_id());
        assert!(outcome.received >= 2);

        let synced_book = right
            .storage
            .books
            .get(&book.id)
            .await?
            .expect("libro sincronizado");
        assert_eq!(synced_book.title, "Libro sincronizado");
        let synced_annotations = right.storage.annotations.list_by_book(&book.id).await?;
        assert_eq!(synced_annotations.len(), 1);
        assert_eq!(synced_annotations[0].id, annotation.id);
        assert_eq!(
            synced_annotations[0].note.as_deref(),
            Some("nota sincronizada")
        );

        left.storage.annotations.delete(&annotation.id).await?;
        left_service
            .touch(ResourceType::Annotation, &annotation.id)
            .await;
        right_service.sync_now(left_service.device_id()).await?;
        assert!(
            right
                .storage
                .annotations
                .list_by_book(&book.id)
                .await?
                .is_empty()
        );
        assert!(
            right
                .storage
                .annotations
                .get_any(&annotation.id)
                .await?
                .expect("tumba")
                .deleted
        );

        left_service.stop().await?;
        right_service.stop().await?;
        Ok(())
    }
}
