use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::hash::sha256_bytes;
use super::paths::StorageLayout;
use crate::error::{KoraError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Book,
    Cover,
    Voice,
    NoteAudio,
    Audiobook,
    GeneratedBook,
}

impl AssetKind {
    pub fn directory(self) -> &'static str {
        match self {
            Self::Book => "books",
            Self::Cover => "covers",
            Self::Voice => "voices",
            Self::NoteAudio => "notes_audio",
            Self::Audiobook => "audiobooks",
            Self::GeneratedBook => "generated_books",
        }
    }
}

/// Referencia estable a un asset. El archivo vive en
/// `assets/<kind.directory()>/<hash>.<extension>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetRef {
    pub kind: AssetKind,
    pub hash: String,
    pub extension: String,
}

#[derive(Debug, Clone)]
pub struct LocalAssetStore {
    layout: StorageLayout,
}

impl LocalAssetStore {
    pub fn new(layout: StorageLayout) -> Self {
        Self { layout }
    }

    pub fn layout(&self) -> &StorageLayout {
        &self.layout
    }

    /// Guarda bytes en memoria. Deduplica por SHA-256.
    pub fn put_bytes(&self, kind: AssetKind, data: &[u8], extension: &str) -> Result<AssetRef> {
        let reference = self.reference(kind, sha256_bytes(data), extension)?;
        let path = self.path(&reference)?;

        if path.is_file() {
            return Ok(reference);
        }

        let directory = self.directory(kind)?;
        fs::create_dir_all(&directory)?;

        let temporary = directory.join(format!(".tmp-{}", Uuid::new_v4()));
        if let Err(error) = fs::write(&temporary, data) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }

        if let Err(error) = fs::rename(&temporary, &path) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }

        Ok(reference)
    }

    /// Copia un archivo calculando su SHA-256 en streaming y deduplicando.
    pub fn put_file(&self, kind: AssetKind, source: &Path, extension: &str) -> Result<AssetRef> {
        let extension = normalize_extension(extension)?;
        let directory = self.directory(kind)?;
        fs::create_dir_all(&directory)?;

        let temporary = directory.join(format!(".tmp-{}", Uuid::new_v4()));

        let hash = {
            let result = (|| -> Result<String> {
                let mut input = BufReader::new(File::open(source)?);
                let mut output = File::create(&temporary)?;
                let mut hasher = Sha256::new();
                let mut buffer = [0u8; 64 * 1024];

                loop {
                    let read = input.read(&mut buffer)?;
                    if read == 0 {
                        break;
                    }
                    hasher.update(&buffer[..read]);
                    output.write_all(&buffer[..read])?;
                }

                output.sync_all()?;
                Ok(hex::encode(hasher.finalize()))
            })();

            match result {
                Ok(hash) => hash,
                Err(error) => {
                    let _ = fs::remove_file(&temporary);
                    return Err(error);
                }
            }
        };

        let reference = self.reference(kind, hash, &extension)?;
        let path = self.path(&reference)?;

        if path.is_file() {
            let _ = fs::remove_file(&temporary);
        } else if let Err(error) = fs::rename(&temporary, &path) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }

        Ok(reference)
    }

    pub fn path(&self, reference: &AssetRef) -> Result<PathBuf> {
        validate_hash(&reference.hash)?;
        let extension = normalize_extension(&reference.extension)?;
        Ok(self
            .layout
            .asset_dir(reference.kind.directory())?
            .join(format!("{}.{}", reference.hash, extension)))
    }

    pub fn read(&self, reference: &AssetRef) -> Result<Vec<u8>> {
        Ok(fs::read(self.path(reference)?)?)
    }

    pub fn open(&self, reference: &AssetRef) -> Result<File> {
        Ok(File::open(self.path(reference)?)?)
    }

    pub fn exists(&self, reference: &AssetRef) -> Result<bool> {
        Ok(self.path(reference)?.is_file())
    }

    pub fn remove(&self, reference: &AssetRef) -> Result<()> {
        let path = self.path(reference)?;
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(KoraError::Io(error)),
        }
    }

    fn directory(&self, kind: AssetKind) -> Result<PathBuf> {
        self.layout.asset_dir(kind.directory())
    }

    fn reference(&self, kind: AssetKind, hash: String, extension: &str) -> Result<AssetRef> {
        let extension = normalize_extension(extension)?;
        validate_hash(&hash)?;

        Ok(AssetRef {
            kind,
            hash,
            extension,
        })
    }
}

pub fn normalize_extension(extension: &str) -> Result<String> {
    let extension = extension
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase();
    let valid = !extension.is_empty()
        && extension.len() <= 8
        && extension.chars().all(|c| c.is_ascii_alphanumeric());

    if valid {
        Ok(extension)
    } else {
        Err(KoraError::InvalidInput(format!(
            "extensión no válida: {extension:?}"
        )))
    }
}

pub fn validate_hash(hash: &str) -> Result<()> {
    let valid = hash.len() == 64
        && hash
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());

    if valid {
        Ok(())
    } else {
        Err(KoraError::InvalidInput(format!(
            "hash SHA-256 no válido: {hash:?}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (LocalAssetStore, PathBuf) {
        let base = std::env::temp_dir().join(format!("kora-assets-{}", Uuid::new_v4()));
        let layout = StorageLayout::from_root(&base);
        layout.ensure().expect("árbol");
        (LocalAssetStore::new(layout), base)
    }

    #[test]
    fn stores_dedups_and_removes_assets() {
        let (store, base) = store();

        let first = store
            .put_bytes(AssetKind::Book, b"contenido", "epub")
            .expect("guardar asset");
        let second = store
            .put_bytes(AssetKind::Book, b"contenido", "epub")
            .expect("guardar de nuevo");

        assert_eq!(first, second);
        assert_eq!(
            first.hash,
            "6f9566ef46386b8cf372671cb9eddff5488256eff5ea5fb99e1041c3e27082bf"
        );

        let path = store.path(&first).expect("resolver");
        assert_eq!(std::fs::read(&path).expect("leer"), b"contenido");
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(".epub")
        );

        store.remove(&first).expect("borrar");
        assert!(!path.exists());

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn copies_files_by_hash() {
        let (store, base) = store();
        let source = base.join("origen.pdf");
        std::fs::write(&source, b"pdf").expect("escribir origen");

        let reference = store
            .put_file(AssetKind::Book, &source, "pdf")
            .expect("copiar");

        assert!(store.exists(&reference).expect("existe"));
        assert_eq!(store.read(&reference).expect("leer"), b"pdf");

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn rejects_invalid_hashes_and_extensions() {
        assert!(validate_hash("../etc/passwd").is_err());
        assert!(validate_hash("ABCDEF").is_err());
        assert!(normalize_extension("../x").is_err());
        assert!(normalize_extension("").is_err());
    }
}
