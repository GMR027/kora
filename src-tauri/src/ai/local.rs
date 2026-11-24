use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen2::ModelWeights;
use tokenizers::Tokenizer;

use crate::error::{KoraError, Result};

use super::{AiLocalProgress, AiLocalStatus};

pub const MODEL_ID: &str = "qwen2.5-0.5b-instruct-q4_0";
pub const MODEL_NAME: &str = "Qwen2.5 0.5B Instruct";
pub const MODEL_LICENSE: &str = "Apache-2.0";
const MODEL_FILE: &str = "qwen2.5-0.5b-instruct-q4_0.gguf";
const LEGACY_MODEL_FILE: &str = "qwen2.5-0.5b-instruct-q4_k_m.gguf";
const TOKENIZER_FILE: &str = "tokenizer.json";
const MODEL_URL: &str = "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_0.gguf";
const TOKENIZER_URL: &str =
    "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct/resolve/main/tokenizer.json";
const MODEL_SHA256: &str = "7671c0c304e6ce5a7fc577bcb12aba01e2c155cc2efd29b2213c95b18edaf6ed";
const TOKENIZER_SHA256: &str = "c0382117ea329cdf097041132f6d735924b697924d6f6fc3945713e96ce87539";
const MODEL_SIZE: u64 = 428_730_208;
const TOKENIZER_SIZE: u64 = 7_031_645;

struct LoadedModel {
    model: ModelWeights,
    tokenizer: Tokenizer,
}

pub struct LocalAiEngine {
    directory: PathBuf,
    model: Mutex<Option<LoadedModel>>,
    downloading: AtomicBool,
}

impl LocalAiEngine {
    pub fn new(directory: PathBuf) -> Self {
        // En móviles big.LITTLE, usar todos los núcleos suele provocar
        // contención térmica. Cuatro hilos mantienen la inferencia estable.
        // SAFETY: se configura antes de crear el pool de Candle, durante el
        // arranque de la app y antes de lanzar inferencias.
        unsafe {
            std::env::set_var("RAYON_NUM_THREADS", "4");
            std::env::set_var("CANDLE_NUM_THREADS", "4");
        }
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build_global();
        // El modelo Q4_K_M era más lento en Candle; al actualizar se libera.
        let legacy = directory.join(LEGACY_MODEL_FILE);
        if legacy.is_file() {
            let _ = std::fs::remove_file(legacy);
        }
        Self {
            directory,
            model: Mutex::new(None),
            downloading: AtomicBool::new(false),
        }
    }

    fn model_path(&self) -> PathBuf {
        self.directory.join(MODEL_FILE)
    }

    fn tokenizer_path(&self) -> PathBuf {
        self.directory.join(TOKENIZER_FILE)
    }

    pub fn installed(&self) -> bool {
        std::fs::metadata(self.model_path())
            .map(|metadata| metadata.is_file() && metadata.len() == MODEL_SIZE)
            .unwrap_or(false)
            && std::fs::metadata(self.tokenizer_path())
                .map(|metadata| metadata.is_file() && metadata.len() == TOKENIZER_SIZE)
                .unwrap_or(false)
    }

    pub fn status(&self) -> AiLocalStatus {
        let size_bytes = std::fs::metadata(self.model_path())
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        AiLocalStatus {
            supported: true,
            installed: self.installed(),
            size_bytes,
            expected_size: MODEL_SIZE,
            model: MODEL_NAME.to_string(),
            license: MODEL_LICENSE.to_string(),
            downloading: self.downloading.load(Ordering::SeqCst),
        }
    }

    pub fn delete(&self) -> Result<()> {
        if let Ok(mut model) = self.model.lock() {
            *model = None;
        }
        for path in [self.model_path(), self.tokenizer_path()] {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    pub async fn download<F>(&self, mut progress: F) -> Result<()>
    where
        F: FnMut(AiLocalProgress),
    {
        if self.installed() {
            progress(AiLocalProgress {
                downloaded: MODEL_SIZE,
                total: MODEL_SIZE,
                percent: 100.0,
                status: "ready".into(),
            });
            return Ok(());
        }
        if self
            .downloading
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(KoraError::Conflict(
                "ya hay una descarga de IA en curso".into(),
            ));
        }
        let result = self.download_inner(&mut progress).await;
        self.downloading.store(false, Ordering::SeqCst);
        result
    }

    async fn download_inner<F>(&self, progress: &mut F) -> Result<()>
    where
        F: FnMut(AiLocalProgress),
    {
        std::fs::create_dir_all(&self.directory)?;
        let total = MODEL_SIZE + TOKENIZER_SIZE;
        let mut downloaded = 0u64;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(900))
            .build()
            .map_err(|error| KoraError::Internal(format!("cliente de descarga: {error}")))?;

        let model_temp = self.model_path().with_extension("gguf.tmp");
        self.download_file(
            &client,
            MODEL_URL,
            &model_temp,
            MODEL_SIZE,
            &mut downloaded,
            total,
            progress,
        )
        .await?;
        let tokenizer_temp = self.tokenizer_path().with_extension("json.tmp");
        self.download_file(
            &client,
            TOKENIZER_URL,
            &tokenizer_temp,
            TOKENIZER_SIZE,
            &mut downloaded,
            total,
            progress,
        )
        .await?;

        let model_path = model_temp.clone();
        let tokenizer_path = tokenizer_temp.clone();
        let (model_hash, tokenizer_hash) = tokio::task::spawn_blocking(move || {
            (
                crate::storage::hash::sha256_file(&model_path),
                crate::storage::hash::sha256_file(&tokenizer_path),
            )
        })
        .await
        .map_err(|error| KoraError::Internal(format!("verificación de IA: {error}")))?;
        let model_hash = model_hash?;
        let tokenizer_hash = tokenizer_hash?;
        if model_hash != MODEL_SHA256 || tokenizer_hash != TOKENIZER_SHA256 {
            let _ = tokio::fs::remove_file(&model_temp).await;
            let _ = tokio::fs::remove_file(&tokenizer_temp).await;
            return Err(KoraError::Storage(
                "los archivos de IA no coinciden con los hashes esperados".into(),
            ));
        }
        tokio::fs::rename(&model_temp, self.model_path()).await?;
        tokio::fs::rename(&tokenizer_temp, self.tokenizer_path()).await?;
        progress(AiLocalProgress {
            downloaded: total,
            total,
            percent: 100.0,
            status: "ready".into(),
        });
        Ok(())
    }

    async fn download_file<F>(
        &self,
        client: &reqwest::Client,
        url: &str,
        target: &Path,
        expected: u64,
        downloaded: &mut u64,
        total: u64,
        progress: &mut F,
    ) -> Result<()>
    where
        F: FnMut(AiLocalProgress),
    {
        let mut response = client
            .get(url)
            .send()
            .await
            .map_err(|error| KoraError::Internal(format!("descarga de IA: {error}")))?
            .error_for_status()
            .map_err(|error| KoraError::Internal(format!("descarga de IA: {error}")))?;
        let mut file = tokio::fs::File::create(target).await?;
        let mut file_bytes = 0u64;
        let mut last_percent = -1.0f64;
        use tokio::io::AsyncWriteExt;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| KoraError::Internal(format!("descarga de IA: {error}")))?
        {
            file.write_all(&chunk).await?;
            file_bytes += chunk.len() as u64;
            *downloaded += chunk.len() as u64;
            let percent = (*downloaded as f64 / total.max(1) as f64 * 100.0).min(100.0);
            if percent - last_percent >= 1.0 || *downloaded == total {
                last_percent = percent;
                progress(AiLocalProgress {
                    downloaded: *downloaded,
                    total,
                    percent,
                    status: "downloading".into(),
                });
            }
        }
        file.flush().await?;
        drop(file);
        if file_bytes != expected {
            let _ = tokio::fs::remove_file(target).await;
            return Err(KoraError::Storage(
                "la descarga de IA quedó incompleta".into(),
            ));
        }
        Ok(())
    }

    pub async fn chat(
        self: &Arc<Self>,
        messages: Vec<(String, String)>,
        temperature: f32,
        max_tokens: u32,
    ) -> Result<String> {
        if !self.installed() {
            return Err(KoraError::Unsupported(
                "el asistente local aún no está descargado".into(),
            ));
        }
        let this = self.clone();
        tokio::task::spawn_blocking(move || this.chat_blocking(messages, temperature, max_tokens))
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de IA local: {error}")))?
    }

    fn chat_blocking(
        &self,
        messages: Vec<(String, String)>,
        temperature: f32,
        max_tokens: u32,
    ) -> Result<String> {
        let mut guard = self
            .model
            .lock()
            .map_err(|_| KoraError::Internal("modelo de IA bloqueado".into()))?;
        if guard.is_none() {
            *guard = Some(self.load_model()?);
        }
        let loaded = guard
            .as_mut()
            .ok_or_else(|| KoraError::Internal("modelo de IA no cargado".into()))?;

        let prompt = chat_prompt(&messages);
        let encoding = loaded
            .tokenizer
            .encode(prompt.as_str(), true)
            .map_err(|error| KoraError::Internal(format!("tokenizador de IA: {error}")))?;
        let prompt_tokens = encoding.get_ids().to_vec();
        let max_tokens = max_tokens.clamp(16, 192) as usize;
        if prompt_tokens.is_empty() {
            return Err(KoraError::InvalidInput(
                "el mensaje de IA está vacío".into(),
            ));
        }
        if prompt_tokens.len() + max_tokens > 2048 {
            return Err(KoraError::InvalidInput(
                "el texto es demasiado largo para el asistente local".into(),
            ));
        }
        let device = Device::Cpu;
        let input = Tensor::new(prompt_tokens.as_slice(), &device)
            .map_err(|error| KoraError::Internal(format!("tensor de IA: {error}")))?
            .unsqueeze(0)
            .map_err(|error| KoraError::Internal(format!("tensor de IA: {error}")))?;
        let logits = loaded
            .model
            .forward(&input, 0)
            .map_err(|error| KoraError::Internal(format!("inferencia de IA: {error}")))?
            .squeeze(0)
            .map_err(|error| KoraError::Internal(format!("inferencia de IA: {error}")))?;
        let sampling = if temperature <= 0.01 {
            Sampling::ArgMax
        } else {
            Sampling::TopKThenTopP {
                k: 40,
                p: 0.9,
                temperature: temperature.clamp(0.1, 1.5) as f64,
            }
        };
        let mut processor = LogitsProcessor::from_sampling(1234, sampling);
        let mut next = processor
            .sample(&logits)
            .map_err(|error| KoraError::Internal(format!("muestreo de IA: {error}")))?;
        let eos = loaded
            .tokenizer
            .token_to_id("<|im_end|>")
            .or_else(|| loaded.tokenizer.token_to_id("<|endoftext|>"));
        let mut output = Vec::new();
        for index in 0..max_tokens {
            if Some(next) == eos {
                break;
            }
            output.push(next);
            let input = Tensor::new(&[next], &device)
                .map_err(|error| KoraError::Internal(format!("tensor de IA: {error}")))?
                .unsqueeze(0)
                .map_err(|error| KoraError::Internal(format!("tensor de IA: {error}")))?;
            let logits = loaded
                .model
                .forward(&input, prompt_tokens.len() + index)
                .map_err(|error| KoraError::Internal(format!("inferencia de IA: {error}")))?
                .squeeze(0)
                .map_err(|error| KoraError::Internal(format!("inferencia de IA: {error}")))?;
            next = processor
                .sample(&logits)
                .map_err(|error| KoraError::Internal(format!("muestreo de IA: {error}")))?;
        }
        let text = loaded
            .tokenizer
            .decode(&output, true)
            .map_err(|error| KoraError::Internal(format!("texto de IA: {error}")))?
            .trim()
            .to_string();
        if text.is_empty() {
            return Err(KoraError::Internal(
                "el asistente local devolvió una respuesta vacía".into(),
            ));
        }
        Ok(text)
    }

    fn load_model(&self) -> Result<LoadedModel> {
        let tokenizer = Tokenizer::from_file(self.tokenizer_path())
            .map_err(|error| KoraError::Internal(format!("tokenizador de IA: {error}")))?;
        let mut file = std::fs::File::open(self.model_path())?;
        let content = gguf_file::Content::read(&mut file)
            .map_err(|error| KoraError::Internal(format!("modelo de IA: {error}")))?;
        let model = ModelWeights::from_gguf(content, &mut file, &Device::Cpu)
            .map_err(|error| KoraError::Internal(format!("modelo de IA: {error}")))?;
        Ok(LoadedModel { model, tokenizer })
    }
}

fn chat_prompt(messages: &[(String, String)]) -> String {
    let mut prompt = String::new();
    for (role, content) in messages {
        let role = match role.as_str() {
            "assistant" => "assistant",
            "system" => "system",
            _ => "user",
        };
        prompt.push_str("<|im_start|>");
        prompt.push_str(role);
        prompt.push('\n');
        prompt.push_str(content.trim());
        prompt.push_str("<|im_end|>\n");
    }
    prompt.push_str("<|im_start|>assistant\n");
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_uses_qwen_chat_format() {
        let prompt = chat_prompt(&[("user".into(), "Hola".into())]);
        assert!(prompt.starts_with("<|im_start|>user\nHola<|im_end|>"));
        assert!(prompt.ends_with("<|im_start|>assistant\n"));
    }
}
