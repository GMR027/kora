use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{KoraError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResult {
    pub text: String,
    pub language: String,
    pub confidence: f32,
}

/// OCR para páginas escaneadas y capturas del lector. Candidatos: ML Kit en
/// Android y tesseract/ONNX en Linux.
#[async_trait]
pub trait OcrEngine: Send + Sync {
    fn id(&self) -> &'static str;
    async fn recognize(&self, image: &Path, language: &str) -> Result<OcrResult>;
}

pub struct PlatformOcrEngine;

#[async_trait]
impl OcrEngine for PlatformOcrEngine {
    fn id(&self) -> &'static str {
        "platform"
    }

    async fn recognize(&self, _image: &Path, _language: &str) -> Result<OcrResult> {
        Err(KoraError::not_implemented(
            "features::PlatformOcrEngine::recognize (fase 7)",
        ))
    }
}
