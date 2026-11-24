#[cfg(not(target_os = "android"))]
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::error::{KoraError, Result};

#[cfg(not(target_os = "android"))]
use super::piper::{find_voice, installed, voice_dir};

/// Descarga una voz Piper desde HuggingFace y la deja en
/// `assets/tts/<voice_id>/model.onnx` + `config.json`.
#[cfg(not(target_os = "android"))]
pub fn download_voice(
    root: &Path,
    voice_id: &str,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<()> {
    let spec = find_voice(voice_id)
        .ok_or_else(|| KoraError::NotFound(format!("voz {voice_id} desconocida")))?;

    if installed(root, voice_id) {
        return Ok(());
    }

    let directory = voice_dir(root, voice_id);
    fs::create_dir_all(&directory)?;

    let model_temp = directory.join(".model.onnx.tmp");
    let config_temp = directory.join(".config.json.tmp");

    let result = (|| -> Result<()> {
        download_file(spec.model_url, &model_temp, spec.size_bytes, progress)?;
        let config_size = download_file(spec.config_url, &config_temp, 8 * 1024, &mut |_, _| {})?;
        if config_size == 0 {
            return Err(KoraError::Storage(
                "el archivo de configuración de la voz llegó vacío".into(),
            ));
        }

        // Comprobación mínima: el config debe ser JSON con el mapa de fonemas.
        let config = fs::read_to_string(&config_temp)?;
        if !config.contains("phoneme_id_map") {
            return Err(KoraError::Storage(
                "el archivo de configuración de la voz no parece válido".into(),
            ));
        }

        fs::rename(&model_temp, directory.join("model.onnx"))?;
        fs::rename(&config_temp, directory.join("config.json"))?;

        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&model_temp);
        let _ = fs::remove_file(&config_temp);
    }

    result
}

pub(crate) fn download_file(
    url: &str,
    destination: &Path,
    total_hint: u64,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<u64> {
    let response = ureq::get(url).call().map_err(|error| {
        KoraError::Internal(format!("no se pudo descargar el archivo: {error}"))
    })?;

    let total = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(total_hint);

    let mut response = response;
    let mut reader = response
        .body_mut()
        .with_config()
        .limit(200 * 1024 * 1024)
        .reader();

    let mut file = File::create(destination)?;
    let mut buffer = vec![0u8; 64 * 1024];
    let mut downloaded = 0u64;

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| KoraError::Io(std::io::Error::other(error)))?;
        if read == 0 {
            break;
        }

        file.write_all(&buffer[..read])?;
        downloaded += read as u64;
        progress(downloaded, total);
    }

    file.sync_all()?;

    Ok(downloaded)
}
