use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, Runtime};

use crate::error::{KoraError, Result};

/// Directorios de assets. El nombre coincide con la carpeta dentro de
/// `assets/` y con `AssetKind::directory`.
pub const ASSET_DIRECTORIES: &[&str] = &[
    "books",
    "covers",
    "voices",
    "notes_audio",
    "audiobooks",
    "generated_books",
    "ocr",
    "tts",
    "whisper",
    "avatars",
];

/// Árbol de datos privados de Kora.
///
/// En Linux se resuelve a `~/.local/share/dev.kora.reader/` y en Android a
/// `/data/user/0/dev.kora.reader/files/`. Nunca se usan rutas públicas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageLayout {
    pub root: PathBuf,
    pub db: PathBuf,
    pub assets: PathBuf,
    pub exports: PathBuf,
    pub trash: PathBuf,
    pub logs: PathBuf,
}

impl StorageLayout {
    pub fn resolve<R: Runtime>(app: &AppHandle<R>) -> Result<Self> {
        let root = app.path().resolve("", BaseDirectory::AppLocalData)?;
        Ok(Self::from_root(root))
    }

    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            db: root.join("db"),
            assets: root.join("assets"),
            exports: root.join("exports"),
            trash: root.join("trash"),
            logs: root.join("logs"),
            root,
        }
    }

    /// Crea el árbol completo. Es idempotente.
    pub fn ensure(&self) -> Result<()> {
        for directory in [
            &self.db,
            &self.assets,
            &self.exports,
            &self.trash,
            &self.logs,
        ] {
            std::fs::create_dir_all(directory)?;
        }

        for directory in ASSET_DIRECTORIES {
            std::fs::create_dir_all(self.assets.join(directory))?;
        }

        Ok(())
    }

    pub fn database_file(&self) -> PathBuf {
        self.db.join("app.sqlite")
    }

    pub fn asset_dir(&self, directory: &str) -> Result<PathBuf> {
        validate_id(directory)?;
        Ok(self.assets.join(directory))
    }
}

/// Evita path traversal en identificadores que vienen del frontend o de la red.
pub fn validate_id(id: &str) -> Result<()> {
    let valid = !id.is_empty()
        && id.len() <= 128
        && id != "."
        && id != ".."
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));

    if valid {
        Ok(())
    } else {
        Err(KoraError::InvalidInput(format!(
            "identificador no válido: {id:?}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_creates_private_tree() {
        let base = std::env::temp_dir().join(format!("kora-paths-{}", uuid::Uuid::new_v4()));
        let layout = StorageLayout::from_root(&base);
        layout.ensure().expect("crear árbol");

        assert!(layout.db.is_dir());
        assert!(layout.assets.join("books").is_dir());
        assert!(layout.assets.join("covers").is_dir());
        assert!(layout.assets.join("voices").is_dir());
        assert!(layout.assets.join("notes_audio").is_dir());
        assert!(layout.assets.join("audiobooks").is_dir());
        assert!(layout.assets.join("generated_books").is_dir());
        assert!(layout.assets.join("ocr").is_dir());
        assert!(layout.assets.join("tts").is_dir());
        assert!(layout.assets.join("whisper").is_dir());
        assert!(layout.exports.is_dir());
        assert!(layout.trash.is_dir());
        assert!(layout.logs.is_dir());
        assert_eq!(
            layout
                .database_file()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("app.sqlite")
        );

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(validate_id("libro-123").is_ok());
        assert!(validate_id("../etc").is_err());
        assert!(validate_id("..").is_err());
        assert!(validate_id("a/b").is_err());
        assert!(validate_id("").is_err());
    }
}
