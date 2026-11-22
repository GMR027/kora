use std::path::Path;
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result, bail};
use ndarray::Array3;
use ort::environment::Environment;
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::Tensor;

fn onnx<E: std::fmt::Display>(error: E) -> anyhow::Error {
    anyhow::anyhow!("ONNX Runtime: {error}")
}

fn environment() -> &'static Environment {
    static ENVIRONMENT: OnceLock<Environment> = OnceLock::new();
    ENVIRONMENT.get_or_init(|| {
        ort::environment::init()
            .build()
            .expect("no se pudo crear el entorno de ONNX Runtime")
    })
}

/// Codificador y decodificador NeuCodec en ONNX.
///
/// El codificador es el `distill-neucodec` exportado por la comunidad: produce
/// los mismos códigos que el modelo PyTorch y es compatible con el decodificador
/// oficial de NeuCodec.
pub struct Codec {
    encoder: Mutex<Session>,
    decoder: Mutex<Session>,
}

impl Codec {
    pub fn load(encoder: &Path, decoder: &Path) -> Result<Self> {
        let encoder_session = Session::builder(environment())
            .map_err(onnx)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(onnx)?
            .commit_from_file(encoder)
            .map_err(onnx)
            .with_context(|| format!("no se pudo cargar el codificador {}", encoder.display()))?;
        let decoder_session = Session::builder(environment())
            .map_err(onnx)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(onnx)?
            .commit_from_file(decoder)
            .map_err(onnx)
            .with_context(|| format!("no se pudo cargar el decodificador {}", decoder.display()))?;

        Ok(Self {
            encoder: Mutex::new(encoder_session),
            decoder: Mutex::new(decoder_session),
        })
    }

    /// Codifica PCM mono a 16 kHz. Devuelve códigos a 50 Hz.
    pub fn encode(&self, samples: &[f32]) -> Result<Vec<i32>> {
        if samples.is_empty() {
            bail!("la muestra de referencia está vacía");
        }

        let mut padded = samples.to_vec();
        let remainder = padded.len() % 320;
        if remainder != 0 {
            padded.resize(padded.len() + (320 - remainder), 0.0);
        }

        let length = padded.len();
        let input =
            Tensor::from_array(Array3::from_shape_vec((1, 1, length), padded)?).map_err(onnx)?;
        let mut session = self
            .encoder
            .lock()
            .map_err(|_| anyhow::anyhow!("codificador bloqueado"))?;
        let outputs = session.run(ort::inputs!["audio" => input]).map_err(onnx)?;
        let (_, codes) = outputs["codes"]
            .try_extract_tensor::<i32>()
            .map_err(onnx)
            .context("el codificador no devolvió códigos i32")?;

        if codes.is_empty() {
            bail!("el codificador devolvió una secuencia vacía");
        }

        Ok(codes.to_vec())
    }

    /// Decodifica códigos a PCM mono de 24 kHz.
    pub fn decode(&self, codes: &[i32]) -> Result<Vec<f32>> {
        if codes.is_empty() {
            bail!("no hay códigos para decodificar");
        }

        let length = codes.len();
        let input = Tensor::from_array(Array3::from_shape_vec((1, 1, length), codes.to_vec())?)
            .map_err(onnx)?;
        let mut session = self
            .decoder
            .lock()
            .map_err(|_| anyhow::anyhow!("decodificador bloqueado"))?;
        let outputs = session.run(ort::inputs!["codes" => input]).map_err(onnx)?;
        let (_, samples) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(onnx)
            .context("el decodificador no devolvió audio f32")?;

        if samples.is_empty() {
            bail!(
                "el decodificador devolvió audio vacío para {} códigos",
                codes.len()
            );
        }

        Ok(samples.to_vec())
    }
}
