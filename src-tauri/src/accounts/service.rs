use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

use base64::Engine;

use crate::error::{KoraError, Result};
use crate::storage::{
    Account, AssetKind, AssetRef, AudiobookStatus, NewAuditEntry, NewShare, ResourceType, Share,
    SharePermission, ShareScopeType, Storage,
};
use crate::sync::identity::{DeviceIdentity, sync_group_id};

use super::crypto;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub public_key: String,
    pub has_pin: bool,
    pub connection_key_available: bool,
    pub connection_key_rotated_at: Option<DateTime<Utc>>,
    pub avatar_path: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionKeyView {
    pub key: String,
    pub uri: String,
    pub qr_svg: String,
    pub account_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareCreateRequest {
    pub scope_type: ShareScopeType,
    pub resource_id: Option<String>,
    pub permission: SharePermission,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_uses: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareView {
    pub share: Share,
    pub key: Option<String>,
    pub uri: Option<String>,
    pub qr_svg: Option<String>,
}

pub struct AccountService {
    storage: Storage,
    device: DeviceIdentity,
}

pub struct UnlockedAccount {
    pub account: Account,
    pub signing_key: SigningKey,
    pub connection_code: String,
    pub connection_key: String,
    pub account_hint: String,
}

impl AccountService {
    pub fn new(storage: Storage, device: DeviceIdentity) -> Self {
        Self { storage, device }
    }

    pub fn device(&self) -> &DeviceIdentity {
        &self.device
    }

    pub async fn account_hint(&self) -> Result<String> {
        let group = sync_group_id(&self.storage).await?;
        Ok(DeviceIdentity::account_hint(&group))
    }

    pub async fn get_account(&self) -> Result<Option<AccountView>> {
        let Some(account) = self.storage.accounts.find_local().await? else {
            return Ok(None);
        };
        let Some(username) = account.username.clone() else {
            return Ok(None);
        };
        let avatar_path = self.avatar_path(&account);
        Ok(Some(AccountView {
            id: account.id,
            username,
            display_name: account.display_name,
            public_key: account.public_key.unwrap_or_default(),
            has_pin: account.pin_hash.is_some(),
            connection_key_available: account.connection_key_ciphertext.is_some(),
            connection_key_rotated_at: account.connection_key_rotated_at,
            avatar_path,
            created_at: account.created_at,
        }))
    }

    fn avatar_path(&self, account: &Account) -> Option<String> {
        let hash = account.avatar_hash.as_deref()?;
        let extension = account.avatar_ext.as_deref()?;
        let reference = AssetRef {
            kind: AssetKind::Avatar,
            hash: hash.to_string(),
            extension: extension.to_string(),
        };
        self.storage
            .assets
            .path(&reference)
            .ok()
            .filter(|path| path.is_file())
            .map(|path| path.display().to_string())
    }

    pub async fn update_display_name(&self, display_name: &str) -> Result<AccountView> {
        let display_name = display_name.trim();
        if display_name.is_empty() || display_name.chars().count() > 80 {
            return Err(KoraError::InvalidInput(
                "el nombre visible debe tener entre 1 y 80 caracteres".into(),
            ));
        }
        let mut account = self.local_account().await?;
        account.display_name = display_name.to_string();
        self.storage.accounts.update(&account).await?;
        self.audit("account.profile", None, None).await?;
        self.get_account()
            .await?
            .ok_or_else(|| KoraError::Internal("cuenta local no encontrada".into()))
    }

    /// Guarda una foto de perfil circular ya recortada por el frontend.
    /// Acepta una data URL o base64 puro con PNG, JPEG o WebP.
    pub async fn update_avatar(&self, image: &str) -> Result<AccountView> {
        let data = decode_avatar(image)?;
        if data.len() > 6 * 1024 * 1024 {
            return Err(KoraError::InvalidInput(
                "la foto de perfil no puede superar 6 MB".into(),
            ));
        }
        let extension = avatar_extension(&data)
            .ok_or_else(|| KoraError::InvalidInput("la foto debe ser PNG, JPG o WebP".into()))?;

        let asset = self
            .storage
            .assets
            .put_bytes(AssetKind::Avatar, &data, extension)?;
        let mut account = self.local_account().await?;
        let previous = account.avatar_hash.take().zip(account.avatar_ext.take());
        account.avatar_hash = Some(asset.hash.clone());
        account.avatar_ext = Some(asset.extension.clone());
        self.storage.accounts.update(&account).await?;
        if let Some((hash, extension)) = previous
            && hash != asset.hash
        {
            self.delete_avatar_asset(&hash, &extension).await;
        }
        self.audit("account.avatar", None, None).await?;
        self.get_account()
            .await?
            .ok_or_else(|| KoraError::Internal("cuenta local no encontrada".into()))
    }

    pub async fn remove_avatar(&self) -> Result<AccountView> {
        let mut account = self.local_account().await?;
        let previous = account.avatar_hash.take().zip(account.avatar_ext.take());
        if let Some((hash, extension)) = previous {
            self.storage.accounts.update(&account).await?;
            self.delete_avatar_asset(&hash, &extension).await;
            self.audit("account.avatar.remove", None, None).await?;
        }
        self.get_account()
            .await?
            .ok_or_else(|| KoraError::Internal("cuenta local no encontrada".into()))
    }

    async fn delete_avatar_asset(&self, hash: &str, extension: &str) {
        if self
            .storage
            .asset_is_referenced(AssetKind::Avatar, hash)
            .await
            .unwrap_or(true)
        {
            return;
        }
        let reference = AssetRef {
            kind: AssetKind::Avatar,
            hash: hash.to_string(),
            extension: extension.to_string(),
        };
        let _ = self.storage.assets.remove(&reference);
    }

    pub async fn create_account(
        &self,
        username: &str,
        display_name: &str,
        pin: &str,
    ) -> Result<ConnectionKeyView> {
        crypto::validate_username(username)?;
        validate_pin(pin)?;
        let account = self.local_account().await?;
        if account.username.is_some() {
            return Err(KoraError::Conflict("la cuenta local ya existe".into()));
        }

        let pin_hash = crypto::hash_pin(pin)?;
        let account_key = crypto::new_account_key();
        let account_public = crypto::account_public_key(&account_key);
        let (account_cipher, account_nonce) =
            crypto::encrypt_secret(pin, &account.id, &account_key.to_bytes())?;

        let code = crypto::random_code();
        let key = crypto::connection_key(username, &code);
        let key_hash = crate::storage::hash::sha256_bytes(key.as_bytes());
        let (connection_cipher, connection_nonce) =
            crypto::encrypt_secret(pin, &account.id, code.as_bytes())?;

        self.storage
            .accounts
            .set_account_identity(
                &account.id,
                username,
                &pin_hash,
                &account_public,
                &account_cipher,
                &account_nonce,
                &key_hash,
                &connection_cipher,
                &connection_nonce,
                Utc::now(),
            )
            .await?;
        self.storage
            .accounts
            .update(&Account {
                display_name: display_name.trim().to_string(),
                ..account
            })
            .await?;
        self.storage
            .connection_keys
            .create(crate::storage::NewConnectionKey {
                account_id: self.local_account().await?.id,
                device_id: None,
                kind: crate::storage::ConnectionKeyKind::Ed25519,
                public_key: account_public,
                secret_ref: None,
                expires_at: None,
            })
            .await?;
        self.audit("account.create", None, None).await?;

        let hint = self.account_hint().await?;
        let uri = crypto::pairing_uri(
            username,
            &key,
            &hint,
            (Utc::now() + Duration::days(7)).timestamp(),
        );
        Ok(ConnectionKeyView {
            key,
            qr_svg: crypto::qr_svg(&uri)?,
            uri,
            account_hint: hint,
        })
    }

    pub async fn unlock(&self, pin: &str) -> Result<UnlockedAccount> {
        let account = self.local_account().await?;
        let username = account
            .username
            .clone()
            .ok_or_else(|| KoraError::Unsupported("todavía no hay una cuenta creada".into()))?;
        let pin_hash = account
            .pin_hash
            .clone()
            .ok_or_else(|| KoraError::Unsupported("la cuenta no tiene PIN".into()))?;
        if !crypto::verify_pin(pin, &pin_hash)? {
            return Err(KoraError::InvalidInput("PIN incorrecto".into()));
        }
        let account_key = crypto::decrypt_secret(
            pin,
            &account.id,
            account
                .account_key_ciphertext
                .as_deref()
                .ok_or_else(|| KoraError::Unsupported("la cuenta no tiene clave privada".into()))?,
            account
                .account_key_nonce
                .as_deref()
                .ok_or_else(|| KoraError::Unsupported("la cuenta no tiene nonce".into()))?,
        )?;
        let signing_key = SigningKey::from_bytes(
            &<[u8; 32]>::try_from(account_key.as_slice())
                .map_err(|_| KoraError::Internal("clave de cuenta inválida".into()))?,
        );
        let code = crypto::decrypt_secret(
            pin,
            &account.id,
            account
                .connection_key_ciphertext
                .as_deref()
                .ok_or_else(|| {
                    KoraError::Unsupported("la cuenta no tiene clave de conexión".into())
                })?,
            account
                .connection_key_nonce
                .as_deref()
                .ok_or_else(|| KoraError::Unsupported("la cuenta no tiene nonce".into()))?,
        )?;
        let connection_code = String::from_utf8_lossy(&code).to_string();
        let connection_key = crypto::connection_key(&username, &connection_code);
        let account_hint = self.account_hint().await?;
        Ok(UnlockedAccount {
            account,
            signing_key,
            connection_code,
            connection_key,
            account_hint,
        })
    }

    pub async fn get_connection_key(&self, pin: &str) -> Result<ConnectionKeyView> {
        let unlocked = self.unlock(pin).await?;
        let uri = crypto::pairing_uri(
            &unlocked.account.username.clone().unwrap_or_default(),
            &unlocked.connection_key,
            &unlocked.account_hint,
            (Utc::now() + Duration::days(7)).timestamp(),
        );
        Ok(ConnectionKeyView {
            key: unlocked.connection_key,
            qr_svg: crypto::qr_svg(&uri)?,
            uri,
            account_hint: unlocked.account_hint,
        })
    }

    pub async fn rotate_connection_key(&self, pin: &str) -> Result<ConnectionKeyView> {
        let unlocked = self.unlock(pin).await?;
        let username = unlocked.account.username.clone().unwrap_or_default();
        let code = crypto::random_code();
        let key = crypto::connection_key(&username, &code);
        let key_hash = crate::storage::hash::sha256_bytes(key.as_bytes());
        let (cipher, nonce) = crypto::encrypt_secret(pin, &unlocked.account.id, code.as_bytes())?;
        self.storage
            .accounts
            .update_secrets(
                &unlocked.account.id,
                unlocked.account.pin_hash.as_deref().unwrap_or_default(),
                unlocked
                    .account
                    .account_key_ciphertext
                    .as_deref()
                    .unwrap_or_default(),
                unlocked
                    .account
                    .account_key_nonce
                    .as_deref()
                    .unwrap_or_default(),
                &cipher,
                &nonce,
                &key_hash,
                Utc::now(),
            )
            .await?;
        self.audit("account.rotate_connection_key", None, None)
            .await?;
        let uri = crypto::pairing_uri(
            &username,
            &key,
            &unlocked.account_hint,
            (Utc::now() + Duration::days(7)).timestamp(),
        );
        Ok(ConnectionKeyView {
            key,
            qr_svg: crypto::qr_svg(&uri)?,
            uri,
            account_hint: unlocked.account_hint,
        })
    }

    pub async fn change_pin(&self, old_pin: &str, new_pin: &str) -> Result<()> {
        validate_pin(new_pin)?;
        let unlocked = self.unlock(old_pin).await?;
        let account = unlocked.account;
        let pin_hash = crypto::hash_pin(new_pin)?;
        let (account_cipher, account_nonce) =
            crypto::encrypt_secret(new_pin, &account.id, &unlocked.signing_key.to_bytes())?;
        let (connection_cipher, connection_nonce) =
            crypto::encrypt_secret(new_pin, &account.id, unlocked.connection_code.as_bytes())?;
        self.storage
            .accounts
            .update_secrets(
                &account.id,
                &pin_hash,
                &account_cipher,
                &account_nonce,
                &connection_cipher,
                &connection_nonce,
                account.connection_key_hash.as_deref().unwrap_or_default(),
                Utc::now(),
            )
            .await?;
        self.audit("account.change_pin", None, None).await?;
        Ok(())
    }

    /// Restablece el PIN sin conocer el anterior. Regenera la identidad de la
    /// cuenta y la clave de conexión, por lo que los dispositivos y shares
    /// anteriores dejan de ser válidos y hay que volver a emparejarlos.
    /// La biblioteca, las notas y los ajustes locales no se tocan.
    pub async fn recover_pin(&self, new_pin: &str) -> Result<ConnectionKeyView> {
        validate_pin(new_pin)?;
        let account = self.local_account().await?;
        let username = account
            .username
            .clone()
            .ok_or_else(|| KoraError::Unsupported("todavía no hay una cuenta creada".into()))?;

        let pin_hash = crypto::hash_pin(new_pin)?;
        let account_key = crypto::new_account_key();
        let account_public = crypto::account_public_key(&account_key);
        let (account_cipher, account_nonce) =
            crypto::encrypt_secret(new_pin, &account.id, &account_key.to_bytes())?;
        let code = crypto::random_code();
        let key = crypto::connection_key(&username, &code);
        let key_hash = crate::storage::hash::sha256_bytes(key.as_bytes());
        let (connection_cipher, connection_nonce) =
            crypto::encrypt_secret(new_pin, &account.id, code.as_bytes())?;

        self.storage
            .accounts
            .set_account_identity(
                &account.id,
                &username,
                &pin_hash,
                &account_public,
                &account_cipher,
                &account_nonce,
                &key_hash,
                &connection_cipher,
                &connection_nonce,
                Utc::now(),
            )
            .await?;

        // Las firmas antiguas ya no se pueden verificar con la nueva identidad:
        // se revocan shares y se retira la confianza a los pares.
        for share in self.storage.shares.list_by_owner(&account.id).await? {
            let _ = self.storage.shares.revoke(&share.id).await;
        }
        for device in self.storage.devices.list_trusted(&account.id).await? {
            let _ = self.storage.devices.revoke_trust(&device.id).await;
        }
        self.audit("account.recover_pin", None, None).await?;

        let hint = self.account_hint().await?;
        let uri = crypto::pairing_uri(
            &username,
            &key,
            &hint,
            (Utc::now() + Duration::days(7)).timestamp(),
        );
        Ok(ConnectionKeyView {
            key,
            qr_svg: crypto::qr_svg(&uri)?,
            uri,
            account_hint: hint,
        })
    }

    pub async fn create_share(&self, pin: &str, request: &ShareCreateRequest) -> Result<ShareView> {
        validate_pin(pin)?;
        let unlocked = self.unlock(pin).await?;
        let resource_id = match request.scope_type {
            ShareScopeType::Library => String::new(),
            ShareScopeType::Book => {
                let id = request
                    .resource_id
                    .clone()
                    .ok_or_else(|| KoraError::InvalidInput("elige un libro".into()))?;
                let book = self
                    .storage
                    .books
                    .get(&id)
                    .await?
                    .ok_or_else(|| KoraError::NotFound(format!("libro {id}")))?;
                if book.owner_account_id != unlocked.account.id {
                    return Err(KoraError::NotFound(format!("libro {id}")));
                }
                id
            }
            ShareScopeType::Voice => {
                let id = request
                    .resource_id
                    .clone()
                    .ok_or_else(|| KoraError::InvalidInput("elige una voz".into()))?;
                let voice = self
                    .storage
                    .voices
                    .get(&id)
                    .await?
                    .ok_or_else(|| KoraError::NotFound(format!("voz {id}")))?;
                if voice.owner_account_id != unlocked.account.id {
                    return Err(KoraError::NotFound(format!("voz {id}")));
                }
                id
            }
        };
        if request.max_uses.is_some_and(|uses| uses <= 0) {
            return Err(KoraError::InvalidInput(
                "max_uses debe ser mayor que cero".into(),
            ));
        }

        let code = crypto::random_code();
        let scope = match request.scope_type {
            ShareScopeType::Library => "library",
            ShareScopeType::Book => "book",
            ShareScopeType::Voice => "voice",
        };
        let key = crypto::share_key(scope, &code);
        let key_hash = crate::storage::hash::sha256_bytes(key.as_bytes());
        let (cipher, nonce) = crypto::encrypt_secret(pin, &unlocked.account.id, code.as_bytes())?;
        let resource_type = match request.scope_type {
            ShareScopeType::Library => ResourceType::Book,
            ShareScopeType::Book => ResourceType::Book,
            ShareScopeType::Voice => ResourceType::Voice,
        };
        let share = self
            .storage
            .shares
            .create(NewShare {
                owner_account_id: unlocked.account.id.clone(),
                resource_type,
                resource_id,
                scope_type: request.scope_type,
                permission: request.permission,
                expires_at: request.expires_at,
                max_uses: request.max_uses,
                share_key_hash: Some(key_hash),
                share_key_ciphertext: Some(cipher),
                share_key_nonce: Some(nonce),
            })
            .await?;
        self.audit("share.create", Some(resource_type), Some(&share.id))
            .await?;
        let expires = request
            .expires_at
            .unwrap_or_else(|| Utc::now() + Duration::days(7))
            .timestamp();
        let uri = crypto::share_uri(&key, &unlocked.account_hint, expires);
        Ok(ShareView {
            share,
            key: Some(key),
            uri: Some(uri.clone()),
            qr_svg: Some(crypto::qr_svg(&uri)?),
        })
    }

    /// Comparte un audiolibro concreto. El recurso transferido es el M4B y su
    /// manifiesto; el libro asociado viaja como dependencia para que el
    /// receptor pueda abrir el texto. El alcance SQLite sigue siendo `book`
    /// porque el CHECK existente no admite otro valor, pero `resource_type`
    /// distingue el share y la clave usa el prefijo AUDIOBOOK.
    pub async fn create_audiobook_share(
        &self,
        pin: &str,
        audiobook_id: &str,
        permission: SharePermission,
        expires_at: Option<DateTime<Utc>>,
        max_uses: Option<i64>,
    ) -> Result<ShareView> {
        validate_pin(pin)?;
        let unlocked = self.unlock(pin).await?;
        let audiobook = self
            .storage
            .audiobooks
            .get(audiobook_id)
            .await?
            .ok_or_else(|| KoraError::NotFound(format!("audiolibro {audiobook_id}")))?;
        if audiobook.owner_account_id != unlocked.account.id {
            return Err(KoraError::NotFound(format!("audiolibro {audiobook_id}")));
        }
        if audiobook.status != AudiobookStatus::Done {
            return Err(KoraError::Unsupported(
                "el audiolibro todavía no está listo".into(),
            ));
        }
        if audiobook.audio_hash.is_none() || audiobook.manifest_hash.is_none() {
            return Err(KoraError::Unsupported(
                "el audiolibro no tiene audio o manifiesto".into(),
            ));
        }
        for (hash, extension, label) in [
            (
                audiobook.audio_hash.as_deref(),
                audiobook.audio_ext.as_deref(),
                "audio",
            ),
            (
                audiobook.manifest_hash.as_deref(),
                audiobook.manifest_ext.as_deref(),
                "manifiesto",
            ),
        ] {
            let (Some(hash), Some(extension)) = (hash, extension) else {
                return Err(KoraError::Unsupported(format!(
                    "el audiolibro no tiene {label}"
                )));
            };
            let reference = crate::storage::AssetRef {
                kind: crate::storage::AssetKind::Audiobook,
                hash: hash.to_string(),
                extension: extension.to_string(),
            };
            if !self.storage.assets.exists(&reference)? {
                return Err(KoraError::NotFound(format!(
                    "falta el archivo de {label} del audiolibro"
                )));
            }
        }
        if max_uses.is_some_and(|uses| uses <= 0) {
            return Err(KoraError::InvalidInput(
                "max_uses debe ser mayor que cero".into(),
            ));
        }

        let code = crypto::random_code();
        let key = crypto::share_key("audiobook", &code);
        let key_hash = crate::storage::hash::sha256_bytes(key.as_bytes());
        let (cipher, nonce) = crypto::encrypt_secret(pin, &unlocked.account.id, code.as_bytes())?;
        let share = self
            .storage
            .shares
            .create(NewShare {
                owner_account_id: unlocked.account.id.clone(),
                resource_type: ResourceType::Audiobook,
                resource_id: audiobook.id.clone(),
                scope_type: ShareScopeType::Book,
                permission,
                expires_at,
                max_uses,
                share_key_hash: Some(key_hash),
                share_key_ciphertext: Some(cipher),
                share_key_nonce: Some(nonce),
            })
            .await?;
        self.audit(
            "share.create",
            Some(ResourceType::Audiobook),
            Some(&share.id),
        )
        .await?;
        let expires = expires_at
            .unwrap_or_else(|| Utc::now() + Duration::days(7))
            .timestamp();
        let uri = crypto::share_uri(&key, &unlocked.account_hint, expires);
        Ok(ShareView {
            share,
            key: Some(key),
            uri: Some(uri.clone()),
            qr_svg: Some(crypto::qr_svg(&uri)?),
        })
    }

    pub async fn list_shares(&self) -> Result<Vec<ShareView>> {
        let owner = self
            .storage
            .accounts
            .find_local()
            .await?
            .ok_or_else(|| KoraError::Unsupported("no hay cuenta local".into()))?;
        Ok(self
            .storage
            .shares
            .list_by_owner(&owner.id)
            .await?
            .into_iter()
            .map(|share| ShareView {
                share,
                key: None,
                uri: None,
                qr_svg: None,
            })
            .collect())
    }

    pub async fn revoke_share(&self, share_id: &str) -> Result<()> {
        let share = self
            .storage
            .shares
            .get(share_id)
            .await?
            .ok_or_else(|| KoraError::NotFound(format!("share {share_id}")))?;
        let owner = self.local_account().await?;
        if share.owner_account_id != owner.id {
            return Err(KoraError::NotFound(format!("share {share_id}")));
        }
        let resource_type = share.resource_type;
        self.storage.shares.revoke(share_id).await?;
        self.audit("share.revoke", Some(resource_type), Some(share_id))
            .await?;
        Ok(())
    }

    pub async fn reveal_share_key(&self, pin: &str, share_id: &str) -> Result<ShareView> {
        let unlocked = self.unlock(pin).await?;
        let share = self
            .storage
            .shares
            .get(share_id)
            .await?
            .ok_or_else(|| KoraError::NotFound(format!("share {share_id}")))?;
        if share.owner_account_id != unlocked.account.id {
            return Err(KoraError::NotFound(format!("share {share_id}")));
        }
        let code = crypto::decrypt_secret(
            pin,
            &unlocked.account.id,
            share
                .share_key_ciphertext
                .as_deref()
                .ok_or_else(|| KoraError::Unsupported("share sin clave".into()))?,
            share
                .share_key_nonce
                .as_deref()
                .ok_or_else(|| KoraError::Unsupported("share sin nonce".into()))?,
        )?;
        let scope = if share.resource_type == ResourceType::Audiobook {
            "audiobook"
        } else {
            match share.scope_type {
                ShareScopeType::Library => "library",
                ShareScopeType::Book => "book",
                ShareScopeType::Voice => "voice",
            }
        };
        let key = crypto::share_key(scope, &String::from_utf8_lossy(&code));
        let expires = share
            .expires_at
            .unwrap_or_else(|| Utc::now() + Duration::days(7))
            .timestamp();
        let uri = crypto::share_uri(&key, &unlocked.account_hint, expires);
        Ok(ShareView {
            share,
            key: Some(key),
            uri: Some(uri.clone()),
            qr_svg: Some(crypto::qr_svg(&uri)?),
        })
    }

    pub async fn local_account(&self) -> Result<Account> {
        self.storage
            .accounts
            .find_local()
            .await?
            .ok_or_else(|| KoraError::Unsupported("no hay cuenta local".into()))
    }

    pub async fn audit(
        &self,
        action: &str,
        resource_type: Option<ResourceType>,
        resource_id: Option<&str>,
    ) -> Result<()> {
        self.storage
            .audit
            .append(NewAuditEntry {
                actor_account_id: None,
                action: action.to_string(),
                resource_type,
                resource_id: resource_id.map(str::to_string),
                metadata: "{}".to_string(),
            })
            .await?;
        Ok(())
    }
}

fn decode_avatar(image: &str) -> Result<Vec<u8>> {
    let payload = image
        .split_once(',')
        .map(|(_, data)| data)
        .unwrap_or(image)
        .trim();
    if payload.is_empty() {
        return Err(KoraError::InvalidInput("la foto está vacía".into()));
    }
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|_| KoraError::InvalidInput("la foto no es un base64 válido".into()))
}

fn avatar_extension(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

fn validate_pin(pin: &str) -> Result<()> {
    if (4..=12).contains(&pin.chars().count())
        && pin.chars().all(|character| character.is_ascii_digit())
    {
        Ok(())
    } else {
        Err(KoraError::InvalidInput(
            "el PIN debe tener entre 4 y 12 dígitos".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine;

    use super::{AccountService, avatar_extension, decode_avatar};
    use crate::storage::test_support::TestStorage;
    use crate::sync::identity::DeviceIdentity;

    #[test]
    fn decodes_data_urls_and_raw_base64() {
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let encoded = "iVBORw0KGgo=";
        assert_eq!(decode_avatar(encoded).expect("base64"), png);
        assert_eq!(
            decode_avatar(&format!("data:image/png;base64,{encoded}")).expect("data url"),
            png
        );
    }

    #[test]
    fn detects_supported_avatar_formats() {
        assert_eq!(
            avatar_extension(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
            Some("png")
        );
        assert_eq!(avatar_extension(&[0xFF, 0xD8, 0xFF, 0x00]), Some("jpg"));
        assert_eq!(avatar_extension(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(avatar_extension(b"no es una imagen"), None);
    }

    #[tokio::test]
    async fn stores_and_removes_avatar() {
        let (test, identity) = TestStorage::with_identity().await;
        let device =
            DeviceIdentity::load_or_create(&test.storage, &identity, test.storage.layout())
                .await
                .expect("dispositivo");
        let service = AccountService::new(test.storage.clone(), device);
        service
            .create_account("edgar", "Edgar", "1234")
            .await
            .expect("cuenta");

        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];
        let encoded = base64::engine::general_purpose::STANDARD.encode(png);
        let view = service
            .update_avatar(&format!("data:image/png;base64,{encoded}"))
            .await
            .expect("avatar");
        let path = view.avatar_path.expect("ruta de avatar");
        assert!(std::path::Path::new(&path).is_file());

        let cleared = service.remove_avatar().await.expect("quitar avatar");
        assert!(cleared.avatar_path.is_none());
        assert!(!std::path::Path::new(&path).is_file());
    }
}
