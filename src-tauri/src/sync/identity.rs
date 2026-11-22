use std::path::{Path, PathBuf};

use rcgen::{
    CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose, PKCS_ED25519,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};

use crate::error::{KoraError, Result};
use crate::storage::{LocalIdentity, Storage, StorageLayout, hash::sha256_bytes};

/// Identidad criptográfica del dispositivo. La clave privada Ed25519 se
/// guarda en `sync/device.key` (0600) y el certificado autofirmado en
/// `sync/device.crt`. La huella SHA-256 es la que se fija al emparejar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceIdentity {
    pub device_id: String,
    pub account_id: String,
    pub name: String,
    pub public_key: String,
    pub fingerprint: String,
    pub sas_prefix: String,
    #[serde(skip)]
    pub cert_der: Vec<u8>,
    #[serde(skip)]
    pub key_der: Vec<u8>,
}

impl DeviceIdentity {
    pub async fn load_or_create(
        storage: &Storage,
        identity: &LocalIdentity,
        layout: &StorageLayout,
    ) -> Result<Self> {
        let directory = layout.root.join("sync");
        std::fs::create_dir_all(&directory)?;
        let key_path = directory.join("device.key");
        let cert_path = directory.join("device.crt");

        let (key_pair, created) = if key_path.is_file() {
            let der = std::fs::read(&key_path)?;
            let key = KeyPair::from_pkcs8_der_and_sign_algo(
                &PrivatePkcs8KeyDer::from(der.as_slice()),
                &PKCS_ED25519,
            )
            .map_err(|error| {
                KoraError::Internal(format!("clave Ed25519 del dispositivo inválida: {error}"))
            })?;
            (key, false)
        } else {
            let key = KeyPair::generate_for(&PKCS_ED25519)
                .map_err(|error| KoraError::Internal(format!("Ed25519: {error}")))?;
            write_private(&key_path, &key.serialize_der())?;
            (key, true)
        };

        let cert_der = if !created && cert_path.is_file() {
            std::fs::read(&cert_path)?
        } else {
            let mut params = CertificateParams::new(vec!["kora.local".to_string()])
                .map_err(|error| KoraError::Internal(format!("certificado: {error}")))?;
            params.is_ca = IsCa::ExplicitNoCa;
            params.key_usages = vec![
                KeyUsagePurpose::DigitalSignature,
                KeyUsagePurpose::KeyEncipherment,
            ];
            params.extended_key_usages = vec![
                ExtendedKeyUsagePurpose::ServerAuth,
                ExtendedKeyUsagePurpose::ClientAuth,
            ];
            let cert = params
                .self_signed(&key_pair)
                .map_err(|error| KoraError::Internal(format!("certificado: {error}")))?;
            let der = cert.der().to_vec();
            write_private(&cert_path, &der)?;
            der
        };

        let public_key = hex::encode(key_pair.public_key_raw());
        let fingerprint = sha256_bytes(&cert_der);
        storage
            .devices
            .set_identity(&identity.device.id, &public_key, &fingerprint)
            .await?;

        Ok(Self {
            device_id: identity.device.id.clone(),
            account_id: identity.account.id.clone(),
            name: identity.device.name.clone(),
            public_key,
            sas_prefix: fingerprint.chars().take(6).collect(),
            fingerprint,
            cert_der,
            key_der: key_pair.serialize_der(),
        })
    }

    pub fn cert_chain(&self) -> Vec<CertificateDer<'static>> {
        vec![CertificateDer::from(self.cert_der.clone())]
    }

    pub fn private_key(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key_der.clone()))
    }

    /// Código SAS de 6 caracteres derivado de ambas huellas. El orden es
    /// estable para que los dos dispositivos calculen el mismo valor.
    pub fn sas_with(&self, other_fingerprint: &str) -> String {
        let mut values = [self.fingerprint.as_str(), other_fingerprint];
        values.sort_unstable();
        let material = format!("{}{}", values[0], values[1]);
        sha256_bytes(material.as_bytes())
            .chars()
            .take(6)
            .collect::<String>()
            .to_uppercase()
    }

    pub fn account_hint(group_id: &str) -> String {
        sha256_bytes(group_id.as_bytes()).chars().take(12).collect()
    }
}

pub async fn sync_group_id(storage: &Storage) -> Result<String> {
    if let Some(value) = storage.settings.get("sync.group_id").await? {
        return Ok(value);
    }
    let value = uuid::Uuid::now_v7().to_string();
    storage.settings.set("sync.group_id", &value).await?;
    Ok(value)
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn sync_dir(layout: &StorageLayout) -> PathBuf {
    layout.root.join("sync")
}
