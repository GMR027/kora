use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::error::{KoraError, Result};

const DEFAULT_MANIFEST_URL: &str =
    "https://github.com/GMR027/kora/releases/latest/download/latest.json";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateManifest {
    version: String,
    url: String,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub notes: Option<String>,
    pub url: Option<String>,
    pub sha256: Option<String>,
    pub published_at: Option<String>,
    pub manifest_url: String,
}

fn version_parts(value: &str) -> Vec<u64> {
    value
        .trim()
        .trim_start_matches(['v', 'V'])
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(|character| character.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

fn is_newer(latest: &str, current: &str) -> bool {
    let latest = version_parts(latest);
    let current = version_parts(current);
    for index in 0..latest.len().max(current.len()) {
        let left = latest.get(index).copied().unwrap_or(0);
        let right = current.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

fn validate_http_url(value: &str) -> Result<()> {
    if value.starts_with("https://") || value.starts_with("http://") {
        Ok(())
    } else {
        Err(KoraError::InvalidInput(
            "la URL de actualización debe usar http(s)".into(),
        ))
    }
}

#[tauri::command]
pub async fn check_app_update(app: AppHandle, manifest_url: Option<String>) -> Result<UpdateCheck> {
    let current_version = app.package_info().version.to_string();
    let manifest_url = manifest_url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("KORA_UPDATE_MANIFEST_URL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| DEFAULT_MANIFEST_URL.to_string());
    validate_http_url(&manifest_url)?;

    let client = reqwest::Client::builder()
        .user_agent(format!("Kora/{current_version}"))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| KoraError::Internal(format!("cliente HTTP: {error}")))?;

    let response = client.get(&manifest_url).send().await.map_err(|error| {
        KoraError::Internal(format!("no se pudo consultar la actualización: {error}"))
    })?;
    if !response.status().is_success() {
        return Err(KoraError::Internal(format!(
            "el servidor de actualizaciones respondió {}",
            response.status()
        )));
    }

    let manifest: UpdateManifest = response
        .json()
        .await
        .map_err(|error| KoraError::Internal(format!("manifiesto inválido: {error}")))?;
    if manifest.version.trim().is_empty() || manifest.url.trim().is_empty() {
        return Err(KoraError::InvalidInput(
            "el manifiesto debe incluir version y url".into(),
        ));
    }
    validate_http_url(&manifest.url)?;

    let available = is_newer(&manifest.version, &current_version);
    Ok(UpdateCheck {
        available,
        current_version,
        latest_version: manifest.version,
        notes: manifest.notes,
        url: Some(manifest.url),
        sha256: manifest.sha256,
        published_at: manifest.published_at,
        manifest_url,
    })
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<()> {
    validate_http_url(&url)?;

    #[cfg(target_os = "android")]
    {
        Err(KoraError::Unsupported(
            "en Android la actualización se instala desde la app".into(),
        ))
    }

    #[cfg(not(target_os = "android"))]
    {
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(&url)
                .spawn()
                .map_err(|error| {
                    KoraError::Internal(format!("no se pudo abrir el navegador: {error}"))
                })?;
        }

        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(&url)
                .spawn()
                .map_err(|error| {
                    KoraError::Internal(format!("no se pudo abrir el navegador: {error}"))
                })?;
        }

        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", &url])
                .spawn()
                .map_err(|error| {
                    KoraError::Internal(format!("no se pudo abrir el navegador: {error}"))
                })?;
        }

        Ok(())
    }
}
