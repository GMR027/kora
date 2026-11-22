pub mod recording;

#[cfg(not(target_os = "android"))]
pub mod opus;

pub use recording::VoiceNoteRecorder;

use std::path::PathBuf;

use crate::storage::StorageLayout;

/// Ruta canónica de una nota de voz. Se deriva del id para que el archivo y la
/// fila de base de datos no puedan desincronizarse.
pub fn note_audio_path(layout: &StorageLayout, id: &str) -> PathBuf {
    layout.assets.join("notes_audio").join(format!("{id}.opus"))
}
