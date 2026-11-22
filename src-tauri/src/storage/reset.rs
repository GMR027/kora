use std::fs;

use crate::error::Result;
use crate::storage::paths::StorageLayout;

/// Carpetas de datos personales que se eliminan al borrar la cuenta. Los
/// modelos de IA descargables se conservan para no obligar a descargarlos de
/// nuevo.
const PERSONAL_ASSET_DIRS: &[&str] = &[
    "books",
    "covers",
    "voices",
    "notes_audio",
    "audiobooks",
    "generated_books",
    "ocr",
];

/// Aplica un borrado de cuenta preparado desde Ajustes. Se ejecuta antes de
/// abrir SQLite: si se intentara con el pool abierto, las escrituras en curso
/// podrían recrear archivos a medio borrar.
pub fn apply_pending_account_reset(layout: &StorageLayout) -> Result<bool> {
    let marker = layout.root.join("account-reset.pending");
    if !marker.is_file() {
        return Ok(false);
    }

    layout.ensure()?;

    for path in [
        layout.database_file(),
        layout.db.join("app.sqlite-wal"),
        layout.db.join("app.sqlite-shm"),
    ] {
        if path.is_file() {
            let _ = fs::remove_file(path);
        }
    }

    for directory in PERSONAL_ASSET_DIRS {
        let path = layout.assets.join(directory);
        if path.is_dir() {
            let _ = fs::remove_dir_all(path);
        }
    }

    // Las grabaciones de ensayo son personales; los modelos de TTS no.
    let practice = layout.assets.join("tts").join("practice");
    if practice.is_dir() {
        let _ = fs::remove_dir_all(practice);
    }

    for path in [&layout.exports, &layout.trash] {
        if path.is_dir()
            && let Ok(entries) = fs::read_dir(path)
        {
            for entry in entries.flatten() {
                let target = entry.path();
                let result = if target.is_dir() {
                    fs::remove_dir_all(&target)
                } else {
                    fs::remove_file(&target)
                };
                let _ = result;
            }
        }
    }

    // Copias de la base anterior y snapshots temporales también contienen
    // datos personales.
    if let Ok(entries) = fs::read_dir(&layout.db) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("app.sqlite.before-restore-")
                || name.starts_with("app.sqlite.snapshot-")
            {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    // La identidad del dispositivo y las restauraciones pendientes pertenecen
    // a la cuenta anterior.
    let sync = layout.root.join("sync");
    if sync.is_dir() {
        let _ = fs::remove_dir_all(sync);
    }
    let _ = fs::remove_file(layout.root.join("restore.pending"));
    if let Ok(entries) = fs::read_dir(&layout.root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("restore-staging-") {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }

    let _ = fs::remove_file(marker);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_personal_data_but_keeps_ai_models() {
        let base = std::env::temp_dir().join(format!("kora-reset-{}", uuid::Uuid::new_v4()));
        let layout = StorageLayout::from_root(&base);
        layout.ensure().expect("árbol");

        fs::write(layout.database_file(), b"base de datos").expect("db");
        fs::write(layout.root.join("account-reset.pending"), b"1").expect("marcador");
        fs::create_dir_all(layout.assets.join("books")).expect("libros");
        fs::write(layout.assets.join("books").join("libro.epub"), b"libro").expect("libro");
        let models = layout.assets.join("tts").join("models");
        fs::create_dir_all(&models).expect("modelos");
        fs::write(models.join("voz.onnx"), b"modelo").expect("modelo");

        assert!(apply_pending_account_reset(&layout).expect("reset"));
        assert!(!layout.database_file().exists());
        assert!(!layout.assets.join("books").exists());
        assert!(!layout.root.join("account-reset.pending").exists());
        assert!(models.join("voz.onnx").exists());

        std::fs::remove_dir_all(&base).ok();
    }
}
