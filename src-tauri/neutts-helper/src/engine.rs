use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::token::LlamaToken;
use llama_cpp_2::vocab::LlamaVocab;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::codec::Codec;

const CONTEXT: u32 = 2048;
const BATCH: u32 = 512;
const MAX_FRAMES: usize = 1200;

pub struct Synthesis {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub frames: usize,
    pub elapsed_ms: u128,
}

struct Backbone {
    path: PathBuf,
    model: LlamaModel,
    speech: HashMap<i32, i32>,
    end_token: i32,
}

impl Backbone {
    fn load(backend: &LlamaBackend, path: &Path) -> Result<Self> {
        let model = LlamaModel::load_from_file(
            backend,
            path,
            &LlamaModelParams::default().with_n_gpu_layers(0),
        )
        .with_context(|| format!("no se pudo cargar el backbone {}", path.display()))?;

        let (speech, end_token) = speech_tokens(model.vocab())?;
        if speech.is_empty() {
            bail!("el backbone no contiene tokens <|speech_N|>");
        }
        if end_token < 0 {
            bail!("el backbone no contiene <|SPEECH_GENERATION_END|>");
        }

        Ok(Self {
            path: path.to_path_buf(),
            model,
            speech,
            end_token,
        })
    }
}

fn speech_tokens(vocab: LlamaVocab<'_>) -> Result<(HashMap<i32, i32>, i32)> {
    let mut speech = HashMap::new();
    let mut end = -1i32;

    for token in vocab.tokens() {
        let Some(text) = vocab.text(token) else {
            continue;
        };
        let text = text.to_string_lossy();
        if text == "<|SPEECH_GENERATION_END|>" {
            end = token.0;
        } else if let Some(rest) = text
            .strip_prefix("<|speech_")
            .and_then(|value| value.strip_suffix("|>"))
            && let Ok(code) = rest.parse::<i32>()
        {
            speech.insert(token.0, code);
        }
    }

    Ok((speech, end))
}

pub struct NeuttsEngine {
    backend: LlamaBackend,
    backbone: Option<Backbone>,
    codec: Option<Codec>,
}

impl NeuttsEngine {
    pub fn new() -> Result<Self> {
        let backend = LlamaBackend::init().context("no se pudo iniciar llama.cpp")?;
        Ok(Self {
            backend,
            backbone: None,
            codec: None,
        })
    }

    pub fn ensure_codec(&mut self, encoder: &Path, decoder: &Path) -> Result<()> {
        if self.codec.is_none() {
            self.codec = Some(Codec::load(encoder, decoder)?);
        }
        Ok(())
    }

    pub fn encode_reference(
        &mut self,
        encoder: &Path,
        decoder: &Path,
        pcm16k: &[f32],
    ) -> Result<Vec<i32>> {
        self.ensure_codec(encoder, decoder)?;
        self.codec
            .as_ref()
            .context("códec no inicializado")?
            .encode(pcm16k)
    }

    fn ensure_backbone(&mut self, path: &Path) -> Result<&Backbone> {
        let matches = self
            .backbone
            .as_ref()
            .is_some_and(|backbone| backbone.path == path);
        if !matches {
            self.backbone = Some(Backbone::load(&self.backend, path)?);
        }
        self.backbone.as_ref().context("backbone no inicializado")
    }

    #[allow(clippy::too_many_arguments)]
    pub fn speak(
        &mut self,
        backbone_path: &Path,
        encoder_path: &Path,
        decoder_path: &Path,
        codes: &[i32],
        reference_text: &str,
        text: &str,
        input_format: &str,
        seed: u64,
        temperature: f32,
        top_k: usize,
    ) -> Result<Synthesis> {
        if codes.is_empty() {
            bail!("el perfil no tiene códigos de referencia");
        }
        if text.trim().is_empty() {
            bail!("no hay texto para sintetizar");
        }

        self.ensure_codec(encoder_path, decoder_path)?;
        self.ensure_backbone(backbone_path)?;

        let started = Instant::now();
        let prompt = build_prompt(input_format, reference_text, text, codes);
        let max_new = MAX_FRAMES.min(CONTEXT as usize - 8);
        let tokens = {
            let backbone = self.backbone.as_ref().context("backbone no inicializado")?;
            backbone.model.vocab().tokenize(
                prompt.as_bytes(),
                backbone.model.vocab().should_add_bos(),
                true,
            )
        };

        if tokens.is_empty() {
            bail!("el prompt quedó vacío");
        }
        if tokens.len() >= CONTEXT as usize - 8 {
            bail!("la referencia es demasiado larga para el contexto del modelo");
        }

        let mut last_error = None;
        for attempt in 0..3u64 {
            let current_seed = seed.wrapping_add(attempt.wrapping_mul(0x9e37_79b9_7f4a_7c15));
            let (generated, frames) = {
                let backbone = self.backbone.as_ref().context("backbone no inicializado")?;
                generate(
                    &self.backend,
                    &backbone.model,
                    &backbone.speech,
                    backbone.end_token,
                    &tokens,
                    max_new,
                    current_seed,
                    temperature,
                    top_k,
                )?
            };

            if generated.len() < 8 {
                last_error = Some(anyhow::anyhow!("el modelo generó muy poco audio"));
                continue;
            }

            match self
                .codec
                .as_ref()
                .context("códec no inicializado")?
                .decode(&generated)
            {
                Ok(samples) => {
                    return Ok(Synthesis {
                        samples,
                        sample_rate: 24_000,
                        frames,
                        elapsed_ms: started.elapsed().as_millis(),
                    });
                }
                Err(error) => last_error = Some(error),
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("el modelo no generó audio")))
    }
}

fn build_prompt(input_format: &str, reference_text: &str, text: &str, codes: &[i32]) -> String {
    let codes: String = codes
        .iter()
        .map(|code| format!("<|speech_{code}|>"))
        .collect();

    if input_format == "phonemes" {
        format!(
            "user: Convert the text to speech:<|TEXT_PROMPT_START|>{reference_text} {text}<|TEXT_PROMPT_END|>\nassistant:<|SPEECH_GENERATION_START|>{codes}"
        )
    } else {
        format!(
            "<|TEXT_PROMPT_START|>{reference_text} {text}<|TEXT_PROMPT_END|><|SPEECH_GENERATION_START|>{codes}"
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn generate(
    backend: &LlamaBackend,
    model: &LlamaModel,
    speech: &HashMap<i32, i32>,
    end_token: i32,
    prompt: &[LlamaToken],
    max_new: usize,
    seed: u64,
    temperature: f32,
    top_k: usize,
) -> Result<(Vec<i32>, usize)> {
    let threads = std::thread::available_parallelism()
        .map(|value| value.get() as i32)
        .unwrap_or(4)
        .clamp(1, 8);

    let mut context = model.new_context(
        backend,
        LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(CONTEXT))
            .with_n_batch(BATCH)
            .with_n_threads(threads)
            .with_n_threads_batch(threads),
    )?;

    context.clear_kv_cache();

    let mut batch = LlamaBatch::new(BATCH as usize, 1);
    let mut position = 0i32;

    for chunk in prompt.chunks(BATCH as usize) {
        batch.clear();
        for (index, token) in chunk.iter().enumerate() {
            batch.add(
                *token,
                position + index as i32,
                &[0],
                index + 1 == chunk.len(),
            )?;
        }
        context.decode(&mut batch)?;
        position += chunk.len() as i32;
    }

    let allowed: Vec<i32> = speech
        .keys()
        .copied()
        .chain(std::iter::once(end_token))
        .collect();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut codes = Vec::new();

    for _ in 0..max_new {
        let logits = context.get_logits_ith(-1);
        let allow_end = codes.len() >= 25;
        let token = sample(
            logits,
            &allowed,
            allow_end,
            end_token,
            &mut rng,
            temperature,
            top_k,
        );
        if token == end_token {
            break;
        }
        let Some(code) = speech.get(&token) else {
            break;
        };
        codes.push(*code);

        batch.clear();
        batch.add(LlamaToken(token), position, &[0], true)?;
        context.decode(&mut batch)?;
        position += 1;

        if position >= CONTEXT as i32 - 1 {
            break;
        }
    }

    let frames = codes.len();
    Ok((codes, frames))
}

fn sample(
    logits: &[f32],
    allowed: &[i32],
    allow_end: bool,
    end_token: i32,
    rng: &mut StdRng,
    temperature: f32,
    top_k: usize,
) -> i32 {
    let temperature = temperature.max(0.01);
    let mut items: Vec<(i32, f32)> = allowed
        .iter()
        .filter(|token| allow_end || **token != end_token)
        .map(|token| {
            let value = logits
                .get(*token as usize)
                .copied()
                .unwrap_or(f32::NEG_INFINITY);
            (*token, value / temperature)
        })
        .collect();

    items.sort_by(|a, b| b.1.total_cmp(&a.1));
    items.truncate(top_k.clamp(1, items.len()));
    if items.is_empty() {
        return end_token;
    }

    let max = items[0].1;
    let weights: Vec<f32> = items.iter().map(|(_, value)| (value - max).exp()).collect();
    let total: f32 = weights.iter().sum();
    if !total.is_finite() || total <= 0.0 {
        return items[0].0;
    }

    let mut pick = rng.random::<f32>() * total;
    for (index, weight) in weights.iter().enumerate() {
        pick -= weight;
        if pick <= 0.0 {
            return items[index].0;
        }
    }

    items[0].0
}
