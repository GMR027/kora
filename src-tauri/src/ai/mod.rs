#[cfg(target_os = "android")]
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{KoraError, Result};
use crate::storage::Storage;

#[cfg(target_os = "android")]
pub mod local;

const CONFIG_KEY: &str = "ai.config";
const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434";
const DEFAULT_LLAMA_URL: &str = "http://127.0.0.1:8080";
const MAX_MESSAGE_CHARS: usize = 24_000;
const MAX_CONTEXT_CHARS: usize = 80_000;

/// Configuración de los modelos locales. Kora no habla con ninguna nube:
/// solo con un servidor Ollama o llama.cpp que el usuario ejecute en su
/// equipo o en su propio dispositivo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    /// `auto`, `ollama` o `llama_cpp`.
    pub provider: String,
    pub ollama_url: String,
    pub llama_url: String,
    /// Vacío significa "elige el primer modelo de chat disponible".
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

impl Default for AiConfig {
    fn default() -> Self {
        #[allow(unused_mut)]
        let mut config = Self {
            provider: "auto".to_string(),
            ollama_url: DEFAULT_OLLAMA_URL.to_string(),
            llama_url: DEFAULT_LLAMA_URL.to_string(),
            model: String::new(),
            temperature: 0.55,
            max_tokens: 900,
        };
        #[cfg(target_os = "android")]
        {
            config.provider = "local".to_string();
            config.model = local::MODEL_ID.to_string();
            config.max_tokens = 512;
        }
        config
    }
}

impl AiConfig {
    fn normalize(mut self) -> Self {
        self.provider = self.provider.trim().to_ascii_lowercase();
        self.ollama_url = normalize_root(&self.ollama_url);
        self.llama_url = normalize_root(&self.llama_url);
        self.model = self.model.trim().to_string();
        self.temperature = self.temperature.clamp(0.0, 2.0);
        self.max_tokens = self.max_tokens.clamp(64, 8192);
        self
    }

    fn validate(&self) -> Result<()> {
        if !matches!(
            self.provider.as_str(),
            "auto" | "off" | "local" | "ollama" | "llama_cpp"
        ) {
            return Err(KoraError::InvalidInput(
                "el proveedor de IA debe ser auto, off, local, ollama o llama_cpp".into(),
            ));
        }
        for (label, url) in [("Ollama", &self.ollama_url), ("llama.cpp", &self.llama_url)] {
            if url.len() > 300 {
                return Err(KoraError::InvalidInput(format!(
                    "la URL de {label} es demasiado larga"
                )));
            }
            let parsed = reqwest::Url::parse(url)
                .map_err(|_| KoraError::InvalidInput(format!("la URL de {label} no es válida")))?;
            if !matches!(parsed.scheme(), "http" | "https") {
                return Err(KoraError::InvalidInput(format!(
                    "la URL de {label} debe usar http o https"
                )));
            }
        }
        if self.model.chars().count() > 200 {
            return Err(KoraError::InvalidInput(
                "el modelo de IA es demasiado largo".into(),
            ));
        }
        if !(0.0..=2.0).contains(&self.temperature) {
            return Err(KoraError::InvalidInput(
                "la temperatura debe estar entre 0 y 2".into(),
            ));
        }
        if !(64..=8192).contains(&self.max_tokens) {
            return Err(KoraError::InvalidInput(
                "el límite de respuesta debe estar entre 64 y 8192 tokens".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    /// Proveedor realmente detectado: `local`, `ollama` o `llama_cpp`.
    pub provider: String,
    pub configured_provider: String,
    pub available: bool,
    pub base_url: String,
    pub model: String,
    pub models: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiLocalStatus {
    pub supported: bool,
    pub installed: bool,
    pub size_bytes: u64,
    pub expected_size: u64,
    pub model: String,
    pub license: String,
    pub downloading: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiLocalProgress {
    pub downloaded: u64,
    pub total: u64,
    pub percent: f64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiReply {
    pub content: String,
    pub provider: String,
    pub model: String,
    pub elapsed_ms: u128,
}

#[derive(Clone)]
pub struct AiService {
    storage: Storage,
    probe: reqwest::Client,
    chat: reqwest::Client,
    #[cfg(target_os = "android")]
    local: Option<Arc<local::LocalAiEngine>>,
}

struct Probe {
    provider: &'static str,
    base_url: String,
    models: Vec<String>,
    model: String,
    error: Option<String>,
}

impl AiService {
    pub fn new(storage: Storage) -> Result<Self> {
        let probe = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|error| KoraError::Internal(format!("cliente IA: {error}")))?;
        let chat = reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .map_err(|error| KoraError::Internal(format!("cliente IA: {error}")))?;
        Ok(Self {
            storage,
            probe,
            chat,
            #[cfg(target_os = "android")]
            local: None,
        })
    }

    #[cfg(target_os = "android")]
    pub fn with_local(storage: Storage, models_dir: std::path::PathBuf) -> Result<Self> {
        let mut service = Self::new(storage)?;
        service.local = Some(Arc::new(local::LocalAiEngine::new(models_dir)));
        Ok(service)
    }

    pub fn local_status(&self) -> AiLocalStatus {
        #[cfg(target_os = "android")]
        if let Some(local) = &self.local {
            let status = local.status();
            return AiLocalStatus {
                supported: true,
                installed: status.installed,
                size_bytes: status.size_bytes,
                expected_size: status.expected_size,
                model: status.model,
                license: status.license,
                downloading: status.downloading,
            };
        }
        AiLocalStatus {
            supported: false,
            installed: false,
            size_bytes: 0,
            expected_size: 0,
            model: String::new(),
            license: String::new(),
            downloading: false,
        }
    }

    pub async fn local_download<F>(&self, progress: F) -> Result<AiLocalStatus>
    where
        F: FnMut(AiLocalProgress),
    {
        #[cfg(target_os = "android")]
        if let Some(local) = &self.local {
            local.download(progress).await?;
            return Ok(self.local_status());
        }
        #[cfg(not(target_os = "android"))]
        let _ = progress;
        Err(KoraError::Unsupported(
            "el asistente local solo está disponible en Android".into(),
        ))
    }

    pub fn local_delete(&self) -> Result<AiLocalStatus> {
        #[cfg(target_os = "android")]
        if let Some(local) = &self.local {
            local.delete()?;
            return Ok(self.local_status());
        }
        Err(KoraError::Unsupported(
            "el asistente local solo está disponible en Android".into(),
        ))
    }

    pub async fn config(&self) -> Result<AiConfig> {
        let Some(raw) = self.storage.settings.get(CONFIG_KEY).await? else {
            return Ok(AiConfig::default());
        };
        let config = serde_json::from_str::<AiConfig>(&raw)
            .map(|config| config.normalize())
            .unwrap_or_default();
        Ok(config)
    }

    pub async fn save_config(&self, config: AiConfig) -> Result<AiConfig> {
        let config = config.normalize();
        config.validate()?;
        let raw = serde_json::to_string(&config)?;
        self.storage.settings.set(CONFIG_KEY, &raw).await?;
        Ok(config)
    }

    pub async fn status(&self) -> Result<AiStatus> {
        let config = self.config().await?;
        Ok(self.detect(&config).await)
    }

    /// Comprueba los proveedores en el orden adecuado para la plataforma.
    /// En Android prioriza llama.cpp; en escritorio, Ollama.
    async fn detect(&self, config: &AiConfig) -> AiStatus {
        if config.provider == "off" {
            return AiStatus {
                provider: "local".to_string(),
                configured_provider: "off".to_string(),
                available: false,
                base_url: String::new(),
                model: String::new(),
                models: Vec::new(),
                error: None,
            };
        }
        #[cfg(target_os = "android")]
        if let Some(local) = &self.local {
            let local_status = local.status();
            let wants_local =
                config.provider == "local" || (config.provider == "auto" && local_status.installed);
            if wants_local {
                return AiStatus {
                    provider: "local".to_string(),
                    configured_provider: config.provider.clone(),
                    available: local_status.installed,
                    base_url: String::new(),
                    model: if local_status.installed {
                        local::MODEL_NAME.to_string()
                    } else {
                        String::new()
                    },
                    models: if local_status.installed {
                        vec![local::MODEL_ID.to_string()]
                    } else {
                        Vec::new()
                    },
                    error: (!local_status.installed)
                        .then(|| "el asistente local aún no está descargado".to_string()),
                };
            }
        }
        let preferred = if cfg!(target_os = "android") {
            "llama_cpp"
        } else {
            "ollama"
        };
        let order: Vec<&str> = match config.provider.as_str() {
            "ollama" => vec!["ollama"],
            "llama_cpp" => vec!["llama_cpp"],
            _ if preferred == "ollama" => vec!["ollama", "llama_cpp"],
            _ => vec!["llama_cpp", "ollama"],
        };

        let mut errors = Vec::new();
        let mut fallback: Option<AiStatus> = None;
        for provider in order {
            let (root, label) = if provider == "ollama" {
                (&config.ollama_url, "Ollama")
            } else {
                (&config.llama_url, "llama.cpp")
            };
            let probe = self.probe(provider, root, &config.model).await;
            let status = AiStatus {
                provider: probe.provider.to_string(),
                configured_provider: config.provider.clone(),
                available: probe.error.is_none() && !probe.model.is_empty(),
                base_url: probe.base_url,
                model: probe.model,
                models: probe.models,
                error: probe.error,
            };
            if status.available {
                return status;
            }
            errors.push(format!(
                "{label}: {}",
                status
                    .error
                    .as_deref()
                    .unwrap_or("no hay modelos disponibles")
            ));
            if fallback.is_none() {
                fallback = Some(status);
            }
        }

        let mut status = fallback.unwrap_or_else(|| AiStatus {
            provider: preferred.to_string(),
            configured_provider: config.provider.clone(),
            available: false,
            base_url: if preferred == "ollama" {
                config.ollama_url.clone()
            } else {
                config.llama_url.clone()
            },
            model: String::new(),
            models: Vec::new(),
            error: None,
        });
        status.available = false;
        status.error = Some(errors.join(" · "));
        status
    }

    async fn probe(&self, provider: &'static str, root: &str, configured_model: &str) -> Probe {
        let base_url = root.trim_end_matches('/').to_string();
        let (models, openai_error) = match self.openai_models(&base_url).await {
            Ok(models) => (models, None),
            Err(error) => (Vec::new(), Some(error)),
        };

        let (models, error) = if !models.is_empty() || provider != "ollama" {
            (models, openai_error)
        } else {
            // Ollama antiguo puede no exponer /v1/models; /api/tags siempre está.
            match self.ollama_models(&base_url).await {
                Ok(models) => (models, None),
                Err(error) => (Vec::new(), Some(error)),
            }
        };

        let model = choose_model(&models, configured_model);
        let error = if model.is_empty() {
            Some(
                error
                    .unwrap_or_else(|| "el servidor no tiene modelos de chat cargados".to_string()),
            )
        } else {
            None
        };

        Probe {
            provider,
            base_url,
            models,
            model,
            error,
        }
    }

    async fn openai_models(&self, root: &str) -> std::result::Result<Vec<String>, String> {
        let response = self
            .probe
            .get(format!("{}/v1/models", root))
            .send()
            .await
            .map_err(|error| friendly_network_error(&error))?;
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }
        let value: Value = response
            .json()
            .await
            .map_err(|error| format!("respuesta no válida: {error}"))?;
        let mut models = Vec::new();
        if let Some(data) = value.get("data").and_then(Value::as_array) {
            for item in data {
                if let Some(id) = item.get("id").and_then(Value::as_str) {
                    models.push(id.to_string());
                }
            }
        }
        Ok(models)
    }

    async fn ollama_models(&self, root: &str) -> std::result::Result<Vec<String>, String> {
        let response = self
            .probe
            .get(format!("{}/api/tags", root))
            .send()
            .await
            .map_err(|error| friendly_network_error(&error))?;
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }
        let value: Value = response
            .json()
            .await
            .map_err(|error| format!("respuesta no válida: {error}"))?;
        let mut models = Vec::new();
        if let Some(items) = value.get("models").and_then(Value::as_array) {
            for item in items {
                if let Some(name) = item.get("name").and_then(Value::as_str) {
                    models.push(name.to_string());
                }
            }
        }
        Ok(models)
    }

    pub async fn chat(
        &self,
        messages: Vec<AiMessage>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<AiReply> {
        if messages.is_empty() {
            return Err(KoraError::InvalidInput(
                "la conversación de IA está vacía".into(),
            ));
        }
        let mut total = 0usize;
        for message in &messages {
            if !matches!(message.role.as_str(), "system" | "user" | "assistant") {
                return Err(KoraError::InvalidInput(
                    "rol de mensaje de IA no válido".into(),
                ));
            }
            total += message.content.chars().count();
            if message.content.chars().count() > MAX_MESSAGE_CHARS {
                return Err(KoraError::InvalidInput(
                    "un mensaje de IA supera el tamaño máximo".into(),
                ));
            }
        }
        if total > MAX_CONTEXT_CHARS {
            return Err(KoraError::InvalidInput(
                "el contexto de IA es demasiado grande; reduce el texto seleccionado".into(),
            ));
        }

        let config = self.config().await?;
        let status = self.detect(&config).await;
        if !status.available {
            return Err(KoraError::Unsupported(format!(
                "no hay un modelo local disponible. {}",
                status
                    .error
                    .unwrap_or_else(|| "Inicia Ollama o llama.cpp y vuelve a intentarlo.".into())
            )));
        }

        let temperature = temperature.unwrap_or(config.temperature).clamp(0.0, 2.0);
        let max_tokens = max_tokens.unwrap_or(config.max_tokens).clamp(64, 8192);
        let started = Instant::now();

        #[cfg(target_os = "android")]
        if status.provider == "local" {
            let local = self.local.as_ref().ok_or_else(|| {
                KoraError::Unsupported("el asistente local no está inicializado".into())
            })?;
            let content = local
                .chat(
                    messages
                        .iter()
                        .map(|message| (message.role.clone(), message.content.clone()))
                        .collect(),
                    temperature,
                    max_tokens,
                )
                .await?;
            return Ok(AiReply {
                content,
                provider: "local".to_string(),
                model: local::MODEL_NAME.to_string(),
                elapsed_ms: started.elapsed().as_millis(),
            });
        }

        let content = match self
            .openai_chat(
                &status.base_url,
                &status.model,
                &messages,
                temperature,
                max_tokens,
            )
            .await
        {
            Ok(content) => content,
            Err(openai_error) if status.provider == "ollama" => self
                .ollama_chat(
                    &status.base_url,
                    &status.model,
                    &messages,
                    temperature,
                    max_tokens,
                )
                .await
                .map_err(|ollama_error| {
                    KoraError::Internal(format!(
                        "Ollama no respondió ({openai_error}); API nativa: {ollama_error}"
                    ))
                })?,
            Err(error) => return Err(KoraError::Internal(error)),
        };

        let content = content.trim().to_string();
        if content.is_empty() {
            return Err(KoraError::Internal(
                "el modelo local devolvió una respuesta vacía".into(),
            ));
        }

        Ok(AiReply {
            content,
            provider: status.provider,
            model: status.model,
            elapsed_ms: started.elapsed().as_millis(),
        })
    }

    async fn openai_chat(
        &self,
        root: &str,
        model: &str,
        messages: &[AiMessage],
        temperature: f32,
        max_tokens: u32,
    ) -> std::result::Result<String, String> {
        let response = self
            .chat
            .post(format!("{}/v1/chat/completions", root))
            .json(&json!({
                "model": model,
                "messages": messages,
                "temperature": temperature,
                "max_tokens": max_tokens,
                "stream": false,
            }))
            .send()
            .await
            .map_err(|error| friendly_network_error(&error))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!(
                "HTTP {status}: {}",
                body.chars().take(240).collect::<String>()
            ));
        }
        let value: Value = response
            .json()
            .await
            .map_err(|error| format!("respuesta no válida: {error}"))?;
        value
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "la respuesta no contiene texto".to_string())
    }

    async fn ollama_chat(
        &self,
        root: &str,
        model: &str,
        messages: &[AiMessage],
        temperature: f32,
        max_tokens: u32,
    ) -> std::result::Result<String, String> {
        let response = self
            .chat
            .post(format!("{}/api/chat", root))
            .json(&json!({
                "model": model,
                "messages": messages,
                "stream": false,
                "options": {
                    "temperature": temperature,
                    "num_predict": max_tokens,
                },
            }))
            .send()
            .await
            .map_err(|error| friendly_network_error(&error))?;
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }
        let value: Value = response
            .json()
            .await
            .map_err(|error| format!("respuesta no válida: {error}"))?;
        value
            .pointer("/message/content")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "la respuesta no contiene texto".to_string())
    }
}

fn normalize_root(value: &str) -> String {
    let mut value = value.trim().trim_end_matches('/').to_string();
    if value.is_empty() {
        return value;
    }
    if !value.starts_with("http://") && !value.starts_with("https://") {
        value = format!("http://{value}");
    }
    if value.ends_with("/v1") {
        value.truncate(value.len() - 3);
        value = value.trim_end_matches('/').to_string();
    }
    value
}

fn choose_model(models: &[String], configured: &str) -> String {
    if !configured.trim().is_empty() {
        if models.is_empty() {
            return configured.trim().to_string();
        }
        if let Some(model) = models
            .iter()
            .find(|model| model.as_str() == configured.trim())
        {
            return model.clone();
        }
        // Algunos servidores añaden prefijos o etiquetas; acepta una coincidencia
        // razonable sin cambiar el modelo pedido por el usuario.
        if let Some(model) = models
            .iter()
            .find(|model| model.contains(configured.trim()))
        {
            return model.clone();
        }
    }

    let chat_models: Vec<&String> = models
        .iter()
        .filter(|model| {
            let lower = model.to_ascii_lowercase();
            !lower.contains("embed") && !lower.contains("rerank")
        })
        .collect();
    chat_models
        .iter()
        .find(|model| {
            let lower = model.to_ascii_lowercase();
            lower.contains("instruct") || lower.contains("chat")
        })
        .or_else(|| chat_models.first())
        .map(|model| (*model).clone())
        .unwrap_or_default()
}

fn friendly_network_error(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "el servidor local no respondió a tiempo".to_string()
    } else if error.is_connect() {
        "no se pudo conectar con el servidor local".to_string()
    } else {
        format!("error de red local: {error}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_urls_and_models() {
        assert_eq!(
            normalize_root("http://127.0.0.1:11434/v1/"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(normalize_root("localhost:8080"), "http://localhost:8080");
        assert_eq!(normalize_root(""), "");
    }

    #[test]
    fn chooses_a_chat_model() {
        let models = vec![
            "nomic-embed-text:latest".to_string(),
            "llama3.2:latest".to_string(),
            "qwen2.5:7b-instruct".to_string(),
        ];
        assert_eq!(choose_model(&models, ""), "qwen2.5:7b-instruct");
        assert_eq!(choose_model(&models, "llama3.2"), "llama3.2:latest");
        assert_eq!(choose_model(&[], "custom.gguf"), "custom.gguf");
    }

    #[test]
    fn validates_config() {
        let config = AiConfig::default();
        assert!(config.validate().is_ok());
        let invalid = AiConfig {
            provider: "openai".to_string(),
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
    }
}
