use chrono::Utc;
use serde_json::Value;
use yrs::updates::decoder::Decode;
use yrs::{
    Any, ClientID, Doc, Map, Options, Out, ReadTxn, StateVector, Transact, Update, WriteTxn,
};

use crate::error::{KoraError, Result};
use crate::storage::{
    Annotation, CrdtDocument, Extract, Note, Reaction, ResourceType, Storage, VoiceNote,
};

/// Almacén CRDT de anotaciones. Cada anotación es un `yrs::Doc` independiente
/// con un `Y.Map` llamado `data`; `payload` guarda la fila serializada y
/// `deleted` la tumba. El snapshot completo se persiste en SQLite.
#[derive(Clone)]
pub struct CrdtStore {
    storage: Storage,
    client_id: u64,
    owner_account_id: String,
}

impl CrdtStore {
    pub fn new(storage: Storage, client_id: u64, owner_account_id: String) -> Self {
        Self {
            storage,
            client_id,
            owner_account_id,
        }
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    /// Marca una fila local como modificada y actualiza su documento CRDT.
    pub async fn touch(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
    ) -> Result<Option<CrdtDocument>> {
        let Some((payload, deleted)) = self.payload(resource_type, resource_id).await? else {
            return Ok(None);
        };
        let doc = self.load_doc(resource_type, resource_id).await?;
        {
            let mut txn = doc.transact_mut();
            let map = txn.get_or_insert_map("data");
            map.try_update(&mut txn, "payload", payload);
            map.try_update(&mut txn, "deleted", deleted);
        }
        self.save(resource_type, resource_id, &doc).await.map(Some)
    }

    /// Garantiza que una anotación tenga documento antes de construir el
    /// manifiesto, incluidas las filas creadas antes de esta fase.
    pub async fn ensure(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
    ) -> Result<Option<CrdtDocument>> {
        if let Some(document) = self
            .storage
            .crdt_documents
            .get(resource_type, resource_id)
            .await?
        {
            return Ok(Some(document));
        }
        self.touch(resource_type, resource_id).await
    }

    /// Fusiona un snapshot remoto y materializa el resultado en SQL.
    pub async fn apply(
        &self,
        peer_device_id: &str,
        resource_type: ResourceType,
        resource_id: &str,
        snapshot: &[u8],
    ) -> Result<CrdtDocument> {
        let doc = self.load_doc(resource_type, resource_id).await?;
        let update = Update::decode_v1(snapshot)
            .map_err(|error| KoraError::InvalidInput(format!("CRDT inválido: {error}")))?;
        doc.transact_mut()
            .apply_update(update)
            .map_err(|error| KoraError::InvalidInput(format!("CRDT no aplicable: {error}")))?;

        let payload = {
            let txn = doc.transact();
            let map = txn
                .get_map("data")
                .ok_or_else(|| KoraError::InvalidInput("documento CRDT sin datos".into()))?;
            match map.get(&txn, "payload") {
                Some(Out::Any(Any::String(value))) => value.to_string(),
                _ => {
                    return Err(KoraError::InvalidInput("documento CRDT sin payload".into()));
                }
            }
        };

        self.upsert_payload(peer_device_id, resource_type, &payload)
            .await?;
        self.save(resource_type, resource_id, &doc).await
    }

    async fn payload(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
    ) -> Result<Option<(String, bool)>> {
        let (payload, deleted) = match resource_type {
            ResourceType::Annotation => {
                match self.storage.annotations.get_any(resource_id).await? {
                    Some(row) => (serde_json::to_string(&row)?, row.deleted),
                    None => return Ok(None),
                }
            }
            ResourceType::Note => match self.storage.notes.get_any(resource_id).await? {
                Some(row) => (serde_json::to_string(&row)?, row.deleted),
                None => return Ok(None),
            },
            ResourceType::Extract => match self.storage.extracts.get_any(resource_id).await? {
                Some(row) => (serde_json::to_string(&row)?, row.deleted),
                None => return Ok(None),
            },
            ResourceType::Reaction => match self.storage.reactions.get_any(resource_id).await? {
                Some(row) => (serde_json::to_string(&row)?, row.deleted),
                None => return Ok(None),
            },
            ResourceType::VoiceNote => match self.storage.voice_notes.get_any(resource_id).await? {
                Some(row) => (serde_json::to_string(&row)?, row.deleted),
                None => return Ok(None),
            },
            _ => return Ok(None),
        };
        Ok(Some((payload, deleted)))
    }

    async fn load_doc(&self, resource_type: ResourceType, resource_id: &str) -> Result<Doc> {
        let client_id = ClientID::new(self.client_id & ((1u64 << 53) - 1));
        let doc = Doc::with_options(Options {
            client_id,
            ..Default::default()
        });
        if let Some(existing) = self
            .storage
            .crdt_documents
            .get(resource_type, resource_id)
            .await?
        {
            let update = Update::decode_v1(&existing.snapshot)
                .map_err(|error| KoraError::Internal(format!("snapshot CRDT corrupto: {error}")))?;
            doc.transact_mut()
                .apply_update(update)
                .map_err(|error| KoraError::Internal(format!("snapshot CRDT inválido: {error}")))?;
        }
        Ok(doc)
    }

    async fn save(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
        doc: &Doc,
    ) -> Result<CrdtDocument> {
        let snapshot = doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default());
        let hash = crate::storage::hash::sha256_bytes(&snapshot);
        let document = CrdtDocument {
            resource_type,
            resource_id: resource_id.to_string(),
            snapshot,
            hash,
            updated_at: Utc::now(),
        };
        self.storage.crdt_documents.upsert(&document).await?;
        Ok(document)
    }

    async fn upsert_payload(
        &self,
        peer_device_id: &str,
        resource_type: ResourceType,
        payload: &str,
    ) -> Result<()> {
        let mut value: Value = serde_json::from_str(payload)
            .map_err(|error| KoraError::InvalidInput(format!("payload CRDT inválido: {error}")))?;

        // Si el libro remoto ya existe localmente con otro id, se traduce.
        if let Some(book_id) = value.get("bookId").and_then(Value::as_str)
            && let Some(local_id) = self
                .storage
                .sync_aliases
                .get(peer_device_id, ResourceType::Book, book_id)
                .await?
        {
            value["bookId"] = Value::String(local_id);
        }

        match resource_type {
            ResourceType::Annotation => {
                let mut row: Annotation = serde_json::from_value(value)?;
                row.owner_account_id = self.owner_account_id.clone();
                self.storage
                    .annotations
                    .upsert_from_sync(&row, &self.owner_account_id)
                    .await?;
            }
            ResourceType::Note => {
                let mut row: Note = serde_json::from_value(value)?;
                row.owner_account_id = self.owner_account_id.clone();
                self.storage
                    .notes
                    .upsert_from_sync(&row, &self.owner_account_id)
                    .await?;
            }
            ResourceType::Extract => {
                let mut row: Extract = serde_json::from_value(value)?;
                row.owner_account_id = self.owner_account_id.clone();
                self.storage
                    .extracts
                    .upsert_from_sync(&row, &self.owner_account_id)
                    .await?;
            }
            ResourceType::Reaction => {
                let mut row: Reaction = serde_json::from_value(value)?;
                row.owner_account_id = self.owner_account_id.clone();
                self.storage
                    .reactions
                    .upsert_from_sync(&row, &self.owner_account_id)
                    .await?;
            }
            ResourceType::VoiceNote => {
                let mut row: VoiceNote = serde_json::from_value(value)?;
                row.owner_account_id = self.owner_account_id.clone();
                self.storage
                    .voice_notes
                    .upsert_from_sync(&row, &self.owner_account_id)
                    .await?;
            }
            _ => {
                return Err(KoraError::InvalidInput(format!(
                    "tipo sin CRDT: {resource_type:?}"
                )));
            }
        }
        Ok(())
    }
}
