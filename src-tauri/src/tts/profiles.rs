use std::path::PathBuf;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{KoraError, Result};
use crate::storage::StorageLayout;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceProfile {
    pub id: String,
    pub name: String,
    pub engine: String,
    pub language: String,
    pub sample_path: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait VoiceProfileStore: Send + Sync {
    async fn list(&self) -> Result<Vec<VoiceProfile>>;
    async fn get(&self, id: &str) -> Result<Option<VoiceProfile>>;
    async fn save(&self, profile: &VoiceProfile) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
}

/// Perfiles guardados como `assets/voices/<id>/profile.json`.
pub struct FileVoiceProfileStore {
    layout: StorageLayout,
}

impl FileVoiceProfileStore {
    pub fn new(layout: StorageLayout) -> Self {
        Self { layout }
    }

    pub fn voices_root(&self) -> PathBuf {
        self.layout.assets.join("voices")
    }
}

#[async_trait]
impl VoiceProfileStore for FileVoiceProfileStore {
    async fn list(&self) -> Result<Vec<VoiceProfile>> {
        // TODO(fase 5): recorrer assets/voices/*/profile.json.
        Ok(Vec::new())
    }

    async fn get(&self, _id: &str) -> Result<Option<VoiceProfile>> {
        // TODO(fase 5): leer assets/voices/<id>/profile.json.
        Ok(None)
    }

    async fn save(&self, _profile: &VoiceProfile) -> Result<()> {
        Err(KoraError::not_implemented(
            "tts::FileVoiceProfileStore::save",
        ))
    }

    async fn delete(&self, _id: &str) -> Result<()> {
        Err(KoraError::not_implemented(
            "tts::FileVoiceProfileStore::delete",
        ))
    }
}

/// Clonación de voz. La inferencia puede ser local (ONNX) o remota; el trait
/// solo exige muestras de audio y devuelve un perfil reproducible.
#[async_trait]
pub trait VoiceCloner: Send + Sync {
    async fn create_profile(
        &self,
        name: &str,
        language: &str,
        samples: &[PathBuf],
    ) -> Result<VoiceProfile>;
}
