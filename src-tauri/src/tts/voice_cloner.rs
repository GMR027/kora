use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{KoraError, Result};
use crate::storage::{NewVoice, Storage, StorageLayout, Voice, VoiceKind};

/// Prefijo de los `voice_id` clonados. Evita colisiones con los identificadores
/// del catálogo Piper (`es_ES-davefx-medium`) y permite enrutar el motor sin
/// consultar la base de datos en cada frase.
pub const CLONE_PREFIX: &str = "clone:";
pub const PROFILE_VERSION: u32 = 1;
pub const CONSENT_TEXT: &str = "Clono solo voces propias o con permiso";

pub fn clone_voice_id(id: &str) -> String {
    format!("{CLONE_PREFIX}{id}")
}

pub fn strip_clone_prefix(voice_id: &str) -> Option<&str> {
    voice_id.strip_prefix(CLONE_PREFIX)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceProfile {
    pub id: String,
    pub name: String,
    pub engine: String,
    pub language: String,
    pub input_format: String,
    pub sample_path: Option<String>,
    pub reference_duration_ms: i64,
    pub created_at: DateTime<Utc>,
}

impl VoiceProfile {
    pub fn voice_id(&self) -> String {
        clone_voice_id(&self.id)
    }
}

/// `profile.json` dentro de `assets/voices/<id>/` y del archivo exportado.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileFile {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub engine: String,
    pub language: String,
    pub input_format: String,
    pub reference_duration_ms: i64,
    pub frames: usize,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    Backbone,
    Encoder,
    Decoder,
}

#[derive(Debug, Clone)]
pub struct VoiceModelSpec {
    pub id: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size_bytes: u64,
    pub label: &'static str,
    pub kind: ModelKind,
    pub languages: &'static [&'static str],
    pub license: &'static str,
}

/// Modelos que necesita la clonación. Todos se descargan bajo demanda:
/// suman ~1,3 GB para inglés o ~1,0 GB para español.
pub const VOICE_MODELS: &[VoiceModelSpec] = &[
    VoiceModelSpec {
        id: "encoder",
        file: "encoder.onnx",
        url: "https://huggingface.co/KevinAHM/distill-neucodec-onnx/resolve/main/onnx/distill_neucodec_encoder.onnx",
        sha256: "04af54f6af51a7573a8bbcfd691b4f2c68b6dbd03aef72b983cbb4e5140c3a23",
        size_bytes: 276_970_840,
        label: "Codificador NeuCodec",
        kind: ModelKind::Encoder,
        languages: &["es", "en"],
        license: "Apache-2.0",
    },
    VoiceModelSpec {
        id: "encoder-data",
        file: "encoder.onnx.data",
        url: "https://huggingface.co/KevinAHM/distill-neucodec-onnx/resolve/main/onnx/distill_neucodec_encoder.onnx.data",
        sha256: "935859ed7904671dc82da1c533b9bf2fd8bcf6d8fc702bdba5bc25c8f7329e4f",
        size_bytes: 241_565_696,
        label: "Pesos del codificador",
        kind: ModelKind::Encoder,
        languages: &["es", "en"],
        license: "Apache-2.0",
    },
    VoiceModelSpec {
        id: "decoder",
        file: "decoder.onnx",
        url: "https://huggingface.co/TamPham92/neucodec-onnx-decoder-int8/resolve/main/model.onnx",
        sha256: "3ddd9e56396e6029e0e948ac0255c89c803f981f23dcf4c154f50820bd74a6b3",
        size_bytes: 312_292_102,
        label: "Decodificador NeuCodec int8",
        kind: ModelKind::Decoder,
        languages: &["es", "en"],
        license: "Apache-2.0",
    },
    VoiceModelSpec {
        id: "backbone-es",
        file: "neutts-nano-es-q4.gguf",
        url: "https://huggingface.co/mradermacher/neutts-nano-spanish-GGUF/resolve/main/neutts-nano-spanish.Q4_K_M.gguf",
        sha256: "60d164f5b34fbbe0d31af682da865b75bd347406b2bb78a3e46569b1a82fda18",
        size_bytes: 209_572_288,
        label: "NeuTTS Nano español (Q4)",
        kind: ModelKind::Backbone,
        languages: &["es"],
        license: "NeuTTS Open License 1.0 (gratis por debajo de 5 M$ anuales)",
    },
    VoiceModelSpec {
        id: "backbone-en",
        file: "neutts-air-en-q4.gguf",
        url: "https://huggingface.co/BricksDisplay/NeuTTS-Air-GGUF/resolve/main/neutts-air-q4_0.gguf",
        sha256: "8303dd6c20e8b49aa169af81f31946336c98c5f9aa42bdd9fe5293d20011354f",
        size_bytes: 526_237_056,
        label: "NeuTTS Air inglés (Q4)",
        kind: ModelKind::Backbone,
        languages: &["en"],
        license: "Apache-2.0",
    },
];

pub fn models_for_language(language: &str) -> Vec<&'static VoiceModelSpec> {
    VOICE_MODELS
        .iter()
        .filter(|model| model.languages.contains(&language))
        .collect()
}

pub fn backbone_for(language: &str) -> Result<&'static VoiceModelSpec> {
    VOICE_MODELS
        .iter()
        .find(|model| model.kind == ModelKind::Backbone && model.languages.contains(&language))
        .ok_or_else(|| {
            KoraError::Unsupported(format!(
                "no hay un modelo de clonación para el idioma {language}"
            ))
        })
}

pub fn engine_for_language(language: &str) -> Result<&'static str> {
    match language {
        "es" => Ok("neutts-nano"),
        "en" => Ok("neutts-air"),
        _ => Err(KoraError::Unsupported(format!(
            "la clonación de voz solo está disponible en español e inglés; se pidió {language}"
        ))),
    }
}

pub fn input_format_for_language(language: &str) -> &'static str {
    if language == "en" { "phonemes" } else { "text" }
}

/// Nombre público del módulo de clonación.
pub type VoiceCloner = VoiceProfileService;

#[derive(Clone)]
pub struct VoiceProfileService {
    storage: Storage,
    layout: StorageLayout,
}

impl VoiceProfileService {
    pub fn new(storage: Storage, layout: StorageLayout) -> Self {
        Self { storage, layout }
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    pub fn layout(&self) -> &StorageLayout {
        &self.layout
    }

    pub fn voices_root(&self) -> PathBuf {
        self.layout.assets.join("voices")
    }

    pub fn models_dir(&self) -> PathBuf {
        self.voices_root().join("models")
    }

    pub fn model_path(&self, file: &str) -> PathBuf {
        self.models_dir().join(file)
    }

    pub fn profile_dir(&self, id: &str) -> PathBuf {
        self.voices_root().join(id)
    }

    pub fn codes_path(&self, id: &str) -> PathBuf {
        self.profile_dir(id).join("embedding.bin")
    }

    pub fn opus_path(&self, id: &str) -> PathBuf {
        self.profile_dir(id).join("reference.opus")
    }

    pub fn wav_path(&self, id: &str) -> PathBuf {
        self.profile_dir(id).join("reference.wav")
    }

    pub fn text_path(&self, id: &str) -> PathBuf {
        self.profile_dir(id).join("reference.txt")
    }

    pub fn file_path(&self, id: &str) -> PathBuf {
        self.profile_dir(id).join("profile.json")
    }

    pub fn models_installed(&self, language: &str) -> bool {
        models_for_language(language)
            .iter()
            .all(|model| self.model_path(model.file).is_file())
    }

    pub fn missing_models(&self, language: &str) -> Vec<&'static VoiceModelSpec> {
        models_for_language(language)
            .into_iter()
            .filter(|model| !self.model_path(model.file).is_file())
            .collect()
    }

    pub async fn list(&self, owner_account_id: &str) -> Result<Vec<VoiceProfile>> {
        let voices = self
            .storage
            .voices
            .list_cloned_by_owner(owner_account_id)
            .await?;
        Ok(voices
            .into_iter()
            .filter(|voice| self.codes_path(&voice.id).is_file())
            .map(profile_from_voice)
            .collect())
    }

    pub async fn list_all(&self, owner_account_id: &str) -> Result<Vec<Voice>> {
        self.storage
            .voices
            .list_cloned_by_owner(owner_account_id)
            .await
    }

    pub async fn get(&self, id: &str, owner_account_id: &str) -> Result<Option<VoiceProfile>> {
        let Some(voice) = self.storage.voices.get(id).await? else {
            return Ok(None);
        };
        if voice.kind != VoiceKind::Cloned || voice.owner_account_id != owner_account_id {
            return Ok(None);
        }
        Ok(Some(profile_from_voice(voice)))
    }

    /// Cambia solo el nombre visible. El idioma y la muestra quedan intactos
    /// para no invalidar los códigos de referencia ya calculados.
    pub async fn rename(
        &self,
        id: &str,
        owner_account_id: &str,
        name: &str,
    ) -> Result<VoiceProfile> {
        validate_id(id)?;
        let Some(mut voice) = self.storage.voices.get(id).await? else {
            return Err(KoraError::NotFound(format!("el perfil {id} no existe")));
        };
        if voice.kind != VoiceKind::Cloned || voice.owner_account_id != owner_account_id {
            return Err(KoraError::NotFound(format!("el perfil {id} no existe")));
        }

        let profile_path = self.file_path(id);
        if profile_path.is_file() {
            let raw = fs::read(&profile_path)?;
            let mut file: ProfileFile = serde_json::from_slice(&raw)?;
            file.name = name.to_string();
            fs::write(&profile_path, serde_json::to_vec_pretty(&file)?)?;
        }

        voice.name = name.to_string();
        self.storage.voices.update(&voice).await?;
        self.get(id, owner_account_id)
            .await?
            .ok_or_else(|| KoraError::Internal("no se pudo leer el perfil renombrado".into()))
    }

    /// Reemplaza la muestra y los códigos de un perfil conservando su id,
    /// nombre, idioma y fecha de creación.
    pub async fn replace_sample(
        &self,
        id: &str,
        owner_account_id: &str,
        source_dir: &Path,
        reference_text: &str,
        duration_ms: i64,
        frames: usize,
    ) -> Result<VoiceProfile> {
        validate_id(id)?;
        let Some(mut voice) = self.storage.voices.get(id).await? else {
            return Err(KoraError::NotFound(format!("el perfil {id} no existe")));
        };
        if voice.kind != VoiceKind::Cloned || voice.owner_account_id != owner_account_id {
            return Err(KoraError::NotFound(format!("el perfil {id} no existe")));
        }

        let directory = self.profile_dir(id);
        for name in ["embedding.bin", "reference.opus", "reference.wav"] {
            let source = source_dir.join(name);
            if !source.is_file() {
                return Err(KoraError::NotFound(format!(
                    "la muestra procesada no tiene {name}"
                )));
            }
            fs::copy(&source, directory.join(name))?;
        }
        fs::write(self.text_path(id), reference_text.trim())?;

        let profile_path = self.file_path(id);
        if profile_path.is_file() {
            let raw = fs::read(&profile_path)?;
            let mut file: ProfileFile = serde_json::from_slice(&raw)?;
            file.reference_duration_ms = duration_ms;
            file.frames = frames;
            fs::write(&profile_path, serde_json::to_vec_pretty(&file)?)?;
        }

        let codes = fs::read(self.codes_path(id))?;
        let sample = fs::read(self.opus_path(id))?;
        voice.profile_hash = Some(hex::encode(Sha256::digest(&codes)));
        voice.sample_hash = Some(hex::encode(Sha256::digest(&sample)));
        voice.reference_duration_ms = duration_ms;
        self.storage.voices.update(&voice).await?;

        self.get(id, owner_account_id)
            .await?
            .ok_or_else(|| KoraError::Internal("no se pudo leer el perfil actualizado".into()))
    }

    pub fn read_codes(&self, id: &str) -> Result<Vec<i32>> {
        let bytes = fs::read(self.codes_path(id))?;
        if bytes.len() % 4 != 0 {
            return Err(KoraError::InvalidInput(
                "el perfil de voz tiene códigos corruptos".into(),
            ));
        }
        let (chunks, _) = bytes.as_chunks::<4>();
        Ok(chunks
            .iter()
            .map(|chunk| i32::from_le_bytes(*chunk))
            .collect())
    }

    pub fn read_reference_text(&self, id: &str) -> Result<String> {
        Ok(fs::read_to_string(self.text_path(id))?.trim().to_string())
    }

    pub fn read_profile_file(&self, id: &str) -> Result<ProfileFile> {
        let raw = fs::read_to_string(self.file_path(id))?;
        Ok(serde_json::from_str(&raw)?)
    }

    /// Crea el directorio y escribe todos los archivos del perfil. El llamador
    /// debe haber validado el consentimiento y calculado los hashes.
    pub fn write_profile(
        &self,
        file: &ProfileFile,
        codes: &[i32],
        opus: &[u8],
        wav: &[u8],
        reference_text: &str,
    ) -> Result<()> {
        validate_id(&file.id)?;
        let directory = self.profile_dir(&file.id);
        if directory.exists() {
            return Err(KoraError::Conflict(format!(
                "ya existe un perfil con el id {}",
                file.id
            )));
        }
        fs::create_dir_all(&directory)?;

        let mut bytes = Vec::with_capacity(codes.len() * 4);
        for code in codes {
            bytes.extend_from_slice(&code.to_le_bytes());
        }

        fs::write(self.codes_path(&file.id), bytes)?;
        fs::write(self.opus_path(&file.id), opus)?;
        fs::write(self.wav_path(&file.id), wav)?;
        fs::write(self.text_path(&file.id), reference_text.trim())?;
        fs::write(self.file_path(&file.id), serde_json::to_vec_pretty(file)?)?;
        Ok(())
    }

    pub async fn insert(&self, file: &ProfileFile, owner_account_id: &str) -> Result<VoiceProfile> {
        let codes = fs::read(self.codes_path(&file.id))?;
        let opus = fs::read(self.opus_path(&file.id))?;
        let voice = self
            .storage
            .voices
            .create(NewVoice {
                id: Some(file.id.clone()),
                name: file.name.clone(),
                engine: file.engine.clone(),
                language: file.language.clone(),
                profile_hash: Some(hex::encode(Sha256::digest(&codes))),
                profile_ext: Some("bin".to_string()),
                sample_hash: Some(hex::encode(Sha256::digest(&opus))),
                sample_ext: Some("opus".to_string()),
                kind: VoiceKind::Cloned,
                reference_duration_ms: file.reference_duration_ms,
                owner_account_id: owner_account_id.to_string(),
                origin_share_id: None,
            })
            .await?;
        Ok(profile_from_voice(voice))
    }

    pub async fn delete(&self, id: &str, owner_account_id: &str) -> Result<()> {
        validate_id(id)?;
        let Some(voice) = self.storage.voices.get(id).await? else {
            return Err(KoraError::NotFound(format!("el perfil {id} no existe")));
        };
        if voice.kind != VoiceKind::Cloned || voice.owner_account_id != owner_account_id {
            return Err(KoraError::NotFound(format!("el perfil {id} no existe")));
        }

        let directory = self.profile_dir(id);
        if directory.exists() {
            crate::storage::trash::move_to_trash(&self.layout, &directory)?;
        }
        self.storage.voices.delete(id).await?;
        Ok(())
    }

    pub fn export(&self, id: &str, destination: &Path) -> Result<()> {
        validate_id(id)?;
        export_archive(&self.profile_dir(id), destination)
    }
}

fn profile_from_voice(voice: Voice) -> VoiceProfile {
    let input_format = input_format_for_language(&voice.language).to_string();
    let dir = voice.id.clone();
    VoiceProfile {
        id: voice.id,
        name: voice.name,
        engine: voice.engine,
        language: voice.language,
        input_format,
        sample_path: Some(format!("assets/voices/{dir}/reference.opus")),
        reference_duration_ms: voice.reference_duration_ms,
        created_at: voice.created_at,
    }
}

pub fn validate_id(id: &str) -> Result<()> {
    let valid = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || value == '-' || value == '_');
    if valid {
        Ok(())
    } else {
        Err(KoraError::InvalidInput(
            "el identificador del perfil no es válido".into(),
        ))
    }
}

const ARCHIVE_FILES: &[&str] = &[
    "profile.json",
    "embedding.bin",
    "reference.opus",
    "reference.wav",
    "reference.txt",
];

#[derive(Debug, Clone)]
pub struct ImportedProfile {
    pub profile: ProfileFile,
    pub codes: Vec<u8>,
    pub opus: Vec<u8>,
    pub wav: Option<Vec<u8>>,
    pub reference_text: String,
}

/// Empaqueta un perfil en un ZIP reproducible para compartirlo por LAN.
pub fn export_archive(directory: &Path, destination: &Path) -> Result<()> {
    let profile_path = directory.join("profile.json");
    if !profile_path.is_file() {
        return Err(KoraError::NotFound(
            "el perfil no tiene profile.json".into(),
        ));
    }

    let file = fs::File::create(destination)?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mut manifest = serde_json::Map::new();
    for name in ARCHIVE_FILES {
        let path = directory.join(name);
        if !path.is_file() {
            if *name == "reference.wav" {
                continue;
            }
            return Err(KoraError::NotFound(format!(
                "falta {name} en el perfil de voz"
            )));
        }
        let bytes = fs::read(&path)?;
        manifest.insert(
            (*name).to_string(),
            serde_json::json!({
                "sha256": hex::encode(Sha256::digest(&bytes)),
                "size": bytes.len(),
            }),
        );
        zip.start_file(*name, options)
            .map_err(|error| KoraError::Storage(format!("ZIP: {error}")))?;
        zip.write_all(&bytes)?;
    }

    zip.start_file("manifest.json", options)
        .map_err(|error| KoraError::Storage(format!("ZIP: {error}")))?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    zip.finish()
        .map_err(|error| KoraError::Storage(format!("ZIP: {error}")))?;
    Ok(())
}

/// Lee y valida un `.koravoice`. No escribe nada: el llamador decide el id
/// definitivo y el consentimiento.
pub fn read_archive(source: &Path) -> Result<ImportedProfile> {
    let file = fs::File::open(source)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| KoraError::Storage(format!("ZIP: {error}")))?;

    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    let mut total = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| KoraError::Storage(format!("ZIP: {error}")))?;
        let name = entry
            .name()
            .map_err(|error| KoraError::Storage(format!("ZIP: {error}")))?
            .to_string();
        if !ARCHIVE_FILES.contains(&name.as_str()) && name != "manifest.json" {
            continue;
        }
        if entry.size() > 64 * 1024 * 1024 {
            return Err(KoraError::InvalidInput(
                "el perfil exportado es demasiado grande".into(),
            ));
        }
        total += entry.size() as usize;
        if total > 96 * 1024 * 1024 {
            return Err(KoraError::InvalidInput(
                "el perfil exportado es demasiado grande".into(),
            ));
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        files.insert(name, bytes);
    }

    let manifest: serde_json::Value = serde_json::from_slice(
        files
            .get("manifest.json")
            .ok_or_else(|| KoraError::InvalidInput("el perfil no tiene manifest".into()))?,
    )?;

    for name in ARCHIVE_FILES {
        let Some(bytes) = files.get(*name) else {
            if *name == "reference.wav" {
                continue;
            }
            return Err(KoraError::InvalidInput(format!(
                "el perfil no contiene {name}"
            )));
        };
        let expected = manifest
            .get(*name)
            .and_then(|value| value.get("sha256"))
            .and_then(|value| value.as_str())
            .ok_or_else(|| KoraError::InvalidInput(format!("falta el hash de {name}")))?;
        let actual = hex::encode(Sha256::digest(bytes));
        if actual != expected {
            return Err(KoraError::InvalidInput(format!(
                "el archivo {name} está corrupto"
            )));
        }
    }

    let profile: ProfileFile = serde_json::from_slice(
        files
            .get("profile.json")
            .ok_or_else(|| KoraError::InvalidInput("falta profile.json".into()))?,
    )?;
    if profile.version != PROFILE_VERSION {
        return Err(KoraError::InvalidInput(format!(
            "versión de perfil no soportada: {}",
            profile.version
        )));
    }

    let codes = files
        .remove("embedding.bin")
        .ok_or_else(|| KoraError::InvalidInput("faltan los códigos".into()))?;
    if codes.is_empty() || codes.len() % 4 != 0 {
        return Err(KoraError::InvalidInput(
            "los códigos del perfil no son válidos".into(),
        ));
    }

    let opus = files
        .remove("reference.opus")
        .ok_or_else(|| KoraError::InvalidInput("falta reference.opus".into()))?;
    let reference_text = String::from_utf8(
        files
            .remove("reference.txt")
            .ok_or_else(|| KoraError::InvalidInput("falta reference.txt".into()))?,
    )
    .map_err(|_| KoraError::InvalidInput("la transcripción no es UTF-8".into()))?;

    Ok(ImportedProfile {
        profile,
        codes,
        opus,
        wav: files.remove("reference.wav"),
        reference_text,
    })
}

/// Escribe un perfil importado en su directorio definitivo.
pub fn write_imported(directory: &Path, imported: &ImportedProfile) -> Result<()> {
    fs::create_dir_all(directory)?;
    fs::write(
        directory.join("profile.json"),
        serde_json::to_vec_pretty(&imported.profile)?,
    )?;
    fs::write(directory.join("embedding.bin"), &imported.codes)?;
    fs::write(directory.join("reference.opus"), &imported.opus)?;
    fs::write(
        directory.join("reference.txt"),
        imported.reference_text.trim(),
    )?;
    if let Some(wav) = &imported.wav {
        fs::write(directory.join("reference.wav"), wav)?;
    }
    Ok(())
}

/// WAV auxiliar para el reproductor y para exportar; no se usa para clonar.
pub fn read_wav_bytes(path: &Path) -> Result<Vec<u8>> {
    Ok(fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::tempdir;

    fn sample_profile() -> ProfileFile {
        ProfileFile {
            version: PROFILE_VERSION,
            id: "voice-1".to_string(),
            name: "Prueba".to_string(),
            engine: "neutts-nano".to_string(),
            language: "es".to_string(),
            input_format: "text".to_string(),
            reference_duration_ms: 5000,
            frames: 250,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn validates_profile_ids() {
        assert!(validate_id("a1b2-c3d4").is_ok());
        assert!(validate_id("../fuera").is_err());
        assert!(validate_id("con espacio").is_err());
        assert!(validate_id("").is_err());
    }

    #[test]
    fn exports_and_reads_an_archive() {
        let workspace = tempdir().expect("tmp");
        let directory = workspace.path().join("perfil");
        fs::create_dir_all(&directory).expect("dir");

        let profile = sample_profile();
        fs::write(
            directory.join("profile.json"),
            serde_json::to_vec(&profile).unwrap(),
        )
        .unwrap();
        let codes = [1i32, 2, 3, 650];
        let mut bytes = Vec::new();
        for code in codes {
            bytes.extend_from_slice(&code.to_le_bytes());
        }
        fs::write(directory.join("embedding.bin"), &bytes).unwrap();
        fs::write(directory.join("reference.opus"), b"OpusHead").unwrap();
        fs::write(directory.join("reference.wav"), b"RIFF").unwrap();
        fs::write(directory.join("reference.txt"), "hola mundo").unwrap();

        let archive = workspace.path().join("perfil.koravoice");
        export_archive(&directory, &archive).expect("exportar");
        assert!(archive.is_file());

        let imported = read_archive(&archive).expect("importar");
        assert_eq!(imported.profile.name, "Prueba");
        assert_eq!(imported.codes, bytes);
        assert_eq!(imported.reference_text, "hola mundo");
        assert_eq!(imported.wav.as_deref(), Some(b"RIFF".as_slice()));
    }

    #[test]
    fn rejects_archives_with_a_bad_hash() {
        use sha2::{Digest, Sha256};
        use std::io::Write as _;

        let workspace = tempdir().expect("tmp");
        let archive_path = workspace.path().join("malo.koravoice");
        let profile = sample_profile();
        let codes = vec![0u8, 1, 0, 0, 2, 0, 0, 0];
        let opus = b"OpusHead".to_vec();
        let text = b"hola".to_vec();
        let profile_json = serde_json::to_vec(&profile).unwrap();

        let mut manifest = serde_json::Map::new();
        for (name, bytes) in [
            ("profile.json", profile_json.as_slice()),
            ("embedding.bin", codes.as_slice()),
            ("reference.opus", opus.as_slice()),
            ("reference.txt", text.as_slice()),
        ] {
            manifest.insert(
                name.to_string(),
                serde_json::json!({
                    "sha256": hex::encode(Sha256::digest(bytes)),
                    "size": bytes.len(),
                }),
            );
        }
        // El hash declarado no coincide con embedding.bin.
        manifest.insert(
            "embedding.bin".to_string(),
            serde_json::json!({"sha256": "00", "size": codes.len()}),
        );

        let file = std::fs::File::create(&archive_path).expect("zip");
        let mut zip = zip::ZipWriter::new(file);
        let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
        for (name, bytes) in [
            ("profile.json", profile_json),
            ("embedding.bin", codes),
            ("reference.opus", opus),
            ("reference.txt", text),
            (
                "manifest.json",
                serde_json::to_vec(&manifest).expect("manifest"),
            ),
        ] {
            zip.start_file(name, options).expect("entrada");
            zip.write_all(&bytes).expect("escribir");
        }
        zip.finish().expect("cerrar zip");

        let error = read_archive(&archive_path).expect_err("debe rechazar el hash");
        assert!(error.to_string().contains("corrupto"), "{error}");
    }
    #[tokio::test]
    async fn stores_lists_and_deletes_profiles() {
        use crate::storage::test_support::TestStorage;

        let (test, identity) = TestStorage::with_identity().await;
        let service = VoiceProfileService::new(test.storage.clone(), test.storage.layout().clone());

        let profile = ProfileFile {
            version: PROFILE_VERSION,
            id: "perfil-prueba".to_string(),
            name: "Mi voz".to_string(),
            engine: "neutts-nano".to_string(),
            language: "es".to_string(),
            input_format: "text".to_string(),
            reference_duration_ms: 6000,
            frames: 300,
            created_at: Utc::now(),
        };

        service
            .write_profile(&profile, &[1, 2, 3, 4], b"OpusHead", b"RIFF", "hola mundo")
            .expect("escribir perfil");
        let stored = service
            .insert(&profile, &identity.account.id)
            .await
            .expect("insertar perfil");

        let audit = test
            .storage
            .audit
            .append(crate::storage::NewAuditEntry {
                actor_account_id: Some(identity.account.id.clone()),
                action: "voice.clone.consent".to_string(),
                resource_type: Some(crate::storage::ResourceType::Voice),
                resource_id: Some(profile.id.clone()),
                metadata: format!(r#"{{"consent":"{CONSENT_TEXT}"}}"#),
            })
            .await
            .expect("auditoría");
        assert_eq!(audit.action, "voice.clone.consent");

        assert_eq!(stored.name, "Mi voz");
        assert_eq!(
            service.read_codes(&profile.id).expect("códigos"),
            [1, 2, 3, 4]
        );
        assert_eq!(
            service.read_reference_text(&profile.id).expect("texto"),
            "hola mundo"
        );
        assert_eq!(
            service
                .list(&identity.account.id)
                .await
                .expect("listar")
                .len(),
            1
        );

        service
            .delete(&profile.id, &identity.account.id)
            .await
            .expect("borrar");
        assert!(
            service
                .list(&identity.account.id)
                .await
                .expect("listar")
                .is_empty()
        );
        assert!(!service.profile_dir(&profile.id).exists());
    }
}
