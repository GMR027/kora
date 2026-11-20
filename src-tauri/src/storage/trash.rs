use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::Utc;

use super::paths::StorageLayout;
use crate::error::Result;

/// Mueve un archivo a `trash/` sin sobrescribir entradas previas.
/// Si origen y destino están en sistemas de archivos distintos, copia y borra.
pub fn move_to_trash(layout: &StorageLayout, source: &Path) -> Result<PathBuf> {
    if !source.exists() {
        return Ok(source.to_path_buf());
    }

    fs::create_dir_all(&layout.trash)?;

    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("asset");
    let stamp = Utc::now().format("%Y%m%d-%H%M%S");

    let mut target = layout.trash.join(format!("{stamp}-{name}"));
    let mut suffix = 1;
    while target.exists() {
        target = layout.trash.join(format!("{stamp}-{suffix}-{name}"));
        suffix += 1;
    }

    match fs::rename(source, &target) {
        Ok(()) => Ok(target),
        Err(error)
            if error.kind() == ErrorKind::CrossesDevices || error.raw_os_error() == Some(18) =>
        {
            fs::copy(source, &target)?;
            fs::remove_file(source)?;
            Ok(target)
        }
        Err(error) => Err(error.into()),
    }
}

/// Borra definitivamente lo que lleva más de `older_than` en `trash/`.
/// Devuelve cuántas entradas se eliminaron.
pub fn purge(layout: &StorageLayout, older_than: Duration) -> Result<usize> {
    let entries = match fs::read_dir(&layout.trash) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };

    let now = SystemTime::now();
    let mut removed = 0;

    for entry in entries.flatten() {
        let path = entry.path();
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(now);
        let age = now.duration_since(modified).unwrap_or_default();

        if age < older_than {
            continue;
        }

        let result = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };

        if result.is_ok() {
            removed += 1;
        }
    }

    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_files_to_trash_without_overwriting() {
        let base = std::env::temp_dir().join(format!("kora-trash-{}", uuid::Uuid::new_v4()));
        let layout = StorageLayout::from_root(&base);
        layout.ensure().expect("árbol");

        let source = base.join("libro.epub");
        std::fs::write(&source, b"libro").expect("escribir");

        let first = move_to_trash(&layout, &source).expect("mover");
        assert!(!source.exists());
        assert!(first.exists());

        std::fs::write(&source, b"otro").expect("escribir de nuevo");
        let second = move_to_trash(&layout, &source).expect("mover de nuevo");
        assert_ne!(first, second);
        assert!(second.exists());

        std::fs::remove_dir_all(&base).ok();
    }
}
