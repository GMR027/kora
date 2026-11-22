use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::error::{KoraError, Result};
use crate::storage::paths::StorageLayout;

/// Carpetas de assets que se incluyen en una copia de seguridad. Los modelos
/// descargables (embeddings, Piper y Whisper) se excluyen porque se pueden
/// volver a descargar y suelen pesar cientos de MB.
const BACKUP_ASSET_DIRS: &[&str] = &[
    "books",
    "covers",
    "voices",
    "notes_audio",
    "audiobooks",
    "generated_books",
    "ocr",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    pub bytes: u64,
    pub entries: u64,
}

const RESTORE_HELP: &str = "\
Copia de seguridad de Kora
==========================

Este ZIP contiene la base de datos y los assets de tu biblioteca (libros,
portadas, audiolibros, voces clonadas, notas de voz y capas OCR). No contiene
los modelos descargables de IA, que se pueden volver a descargar desde Kora.

Restauración desde Kora:
1. Abre Ajustes > Almacenamiento > Restaurar copia.
2. Selecciona este ZIP y confirma. Kora avisará de que hay que reiniciar.

Restauración manual:
1. Cierra Kora por completo.
2. Localiza la carpeta de datos de Kora (Ajustes > Almacenamiento).
3. Haz una copia de la carpeta actual antes de tocar nada.
4. Descomprime este ZIP sobre la carpeta de datos. Sustituye db/app.sqlite y
   fusiona la carpeta assets/.
5. Abre Kora de nuevo.
";

/// Crea un ZIP con una instantánea de la base de datos y los assets ligeros.
/// `snapshot` debe ser una copia consistente de SQLite creada con VACUUM INTO.
pub fn create(layout: &StorageLayout, snapshot: &Path, destination: &Path) -> Result<BackupInfo> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(destination)?;
    let mut zip = ZipWriter::new(file);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut entries = 0u64;

    write_bytes(&mut zip, "README.txt", RESTORE_HELP.as_bytes(), deflated)?;
    entries += 1;

    let mut database = File::open(snapshot)?;
    zip.start_file("db/app.sqlite", stored).map_err(zip_error)?;
    std::io::copy(&mut database, &mut zip)?;
    entries += 1;

    for directory in BACKUP_ASSET_DIRS {
        let path = layout.assets.join(directory);
        if path.is_dir() {
            entries += add_directory(&mut zip, &layout.assets, &path, stored)?;
        }
    }

    zip.finish().map_err(zip_error)?;
    let bytes = fs::metadata(destination)?.len();

    Ok(BackupInfo {
        path: destination.display().to_string(),
        bytes,
        entries,
    })
}

fn add_directory(
    zip: &mut ZipWriter<File>,
    base: &Path,
    directory: &Path,
    options: SimpleFileOptions,
) -> Result<u64> {
    let mut entries = 0u64;
    let mut stack = vec![directory.to_path_buf()];

    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !path.is_file() {
                continue;
            }

            let relative = path
                .strip_prefix(base)
                .map_err(|error| KoraError::Internal(format!("ruta de copia: {error}")))?;
            let name = format!("assets/{}", relative.to_string_lossy().replace('\\', "/"));
            zip.start_file(name, options).map_err(zip_error)?;
            let mut source = File::open(&path)?;
            std::io::copy(&mut source, zip)?;
            entries += 1;
        }
    }

    Ok(entries)
}

fn write_bytes(
    zip: &mut ZipWriter<File>,
    name: &str,
    bytes: &[u8],
    options: SimpleFileOptions,
) -> Result<()> {
    zip.start_file(name, options).map_err(zip_error)?;
    zip.write_all(bytes)?;
    Ok(())
}

fn zip_error(error: zip::result::ZipError) -> KoraError {
    KoraError::Storage(format!("copia de seguridad: {error}"))
}

/// Aplica una restauración preparada al arrancar, antes de abrir SQLite.
/// Conserva la base anterior por si algo falla y fusiona los assets.
pub fn apply_pending_restore(layout: &StorageLayout) -> Result<bool> {
    let marker = layout.root.join("restore.pending");
    if !marker.is_file() {
        return Ok(false);
    }

    let staging_name = fs::read_to_string(&marker)?.trim().to_string();
    if staging_name.is_empty()
        || staging_name.contains('/')
        || staging_name.contains('\\')
        || staging_name == "."
        || staging_name == ".."
    {
        let _ = fs::remove_file(&marker);
        return Err(KoraError::InvalidInput(
            "la restauración pendiente no es válida".into(),
        ));
    }

    let staging = layout.root.join(&staging_name);
    let staged_db = staging.join("db/app.sqlite");
    if !staged_db.is_file() {
        let _ = fs::remove_file(&marker);
        return Err(KoraError::InvalidInput(
            "la restauración pendiente no contiene la base de datos".into(),
        ));
    }

    layout.ensure()?;
    let database = layout.database_file();
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let previous = layout.db.join(format!("app.sqlite.before-restore-{stamp}"));
    if database.is_file() {
        fs::copy(&database, &previous)?;
    }

    // Los WAL/SHM pertenecen a la base anterior; si quedan, SQLite intentaría
    // aplicarlos sobre el archivo restaurado.
    let _ = fs::remove_file(layout.db.join("app.sqlite-wal"));
    let _ = fs::remove_file(layout.db.join("app.sqlite-shm"));

    let restore = (|| -> Result<()> {
        fs::copy(&staged_db, &database)?;
        let staged_assets = staging.join("assets");
        if staged_assets.is_dir() {
            copy_directory_over(&staged_assets, &layout.assets)?;
        }
        Ok(())
    })();

    if let Err(error) = restore {
        if previous.is_file() {
            let _ = fs::copy(&previous, &database);
        }
        return Err(error);
    }

    let _ = fs::remove_dir_all(&staging);
    let _ = fs::remove_file(&marker);
    Ok(true)
}

fn copy_directory_over(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let target = destination.join(entry.file_name());
        if path.is_dir() {
            copy_directory_over(&path, &target)?;
        } else if path.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

/// Extrae un ZIP de copia de seguridad a un directorio temporal y valida que
/// contenga una base de datos. La restauración real se hace al arrancar.
pub fn extract_to(source: &Path, staging: &Path) -> Result<PathBuf> {
    fs::create_dir_all(staging)?;
    let file = File::open(source)?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error)?;
    let mut database = None;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(zip_error)?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        if name.is_absolute()
            || name
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(KoraError::InvalidInput(
                "la copia de seguridad contiene una ruta no válida".into(),
            ));
        }
        let target = staging.join(&name);
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&target)?;
        std::io::copy(&mut entry, &mut output)?;
        if name == Path::new("db/app.sqlite") {
            database = Some(target);
        }
    }

    database.ok_or_else(|| {
        KoraError::InvalidInput("la copia de seguridad no contiene db/app.sqlite".into())
    })
}
