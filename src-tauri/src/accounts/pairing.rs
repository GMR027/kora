use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{KoraError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingTicket {
    /// Código corto que se muestra como QR y se puede dictar.
    pub code: String,
    /// Huella del certificado del dispositivo para verificación manual.
    pub fingerprint: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedDevice {
    pub device_id: String,
    pub name: String,
    pub public_key: String,
    pub paired_at: DateTime<Utc>,
}

/// Vinculación de dispositivos. El flujo previsto es QR + intercambio de
/// claves sobre QUIC, con confirmación por huella.
#[async_trait]
pub trait PairingService: Send + Sync {
    async fn start(&self) -> Result<PairingTicket>;
    async fn complete(&self, code: &str) -> Result<PairedDevice>;
    async fn devices(&self) -> Result<Vec<PairedDevice>>;
}

pub struct QrPairingService;

#[async_trait]
impl PairingService for QrPairingService {
    async fn start(&self) -> Result<PairingTicket> {
        Err(KoraError::not_implemented(
            "accounts::QrPairingService::start (fase 6)",
        ))
    }

    async fn complete(&self, _code: &str) -> Result<PairedDevice> {
        Err(KoraError::not_implemented(
            "accounts::QrPairingService::complete",
        ))
    }

    async fn devices(&self) -> Result<Vec<PairedDevice>> {
        Ok(Vec::new())
    }
}
