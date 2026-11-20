use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{KoraError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub account_id: String,
    pub display_name: String,
    pub public_key: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature(pub String);

/// Identidad local del dispositivo. En la fase de cuentas se respaldará con
/// Ed25519 y el keystore del sistema (Keyring en Linux, Keystore en Android).
pub trait IdentityProvider: Send + Sync {
    fn identity(&self) -> Result<Identity>;
    fn sign(&self, data: &[u8]) -> Result<Signature>;
    fn verify(&self, data: &[u8], signature: &Signature) -> Result<bool>;
}

/// Par de claves autofirmado que usará el pairing (fase 6).
pub type RcgenKeyPair = rcgen::KeyPair;

pub struct LocalIdentityProvider;

impl IdentityProvider for LocalIdentityProvider {
    fn identity(&self) -> Result<Identity> {
        Err(KoraError::not_implemented(
            "accounts::LocalIdentityProvider::identity (fase 6)",
        ))
    }

    fn sign(&self, _data: &[u8]) -> Result<Signature> {
        Err(KoraError::not_implemented(
            "accounts::LocalIdentityProvider::sign",
        ))
    }

    fn verify(&self, _data: &[u8], _signature: &Signature) -> Result<bool> {
        Err(KoraError::not_implemented(
            "accounts::LocalIdentityProvider::verify",
        ))
    }
}
