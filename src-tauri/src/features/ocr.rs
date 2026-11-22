use serde::{Deserialize, Serialize};

#[cfg(not(target_os = "android"))]
use std::path::Path;

use crate::error::{KoraError, Result};

/// Una palabra reconocida por OCR, en píxeles de la imagen de entrada.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrWord {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResult {
    pub text: String,
    pub language: String,
    pub confidence: f32,
    pub words: Vec<OcrWord>,
}

/// OCR para páginas escaneadas y capturas del lector. En escritorio usa
/// Tesseract; en Android queda detrás de un error claro hasta empaquetar las
/// librerías nativas de la ABI correspondiente.
pub trait OcrEngine: Send + Sync {
    fn id(&self) -> &'static str;
    fn available(&self) -> bool;
    fn recognize(&self, image: &[u8], language: &str) -> Result<OcrResult>;
}

#[cfg(not(target_os = "android"))]
pub struct TesseractOcrEngine {
    data_path: Option<String>,
}

#[cfg(not(target_os = "android"))]
impl TesseractOcrEngine {
    pub fn new() -> Self {
        Self {
            data_path: find_tessdata(),
        }
    }

    pub fn with_data_path(data_path: impl Into<String>) -> Self {
        Self {
            data_path: Some(data_path.into()),
        }
    }
}

#[cfg(not(target_os = "android"))]
impl Default for TesseractOcrEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_os = "android"))]
impl OcrEngine for TesseractOcrEngine {
    fn id(&self) -> &'static str {
        "tesseract"
    }

    fn available(&self) -> bool {
        true
    }

    fn recognize(&self, image: &[u8], language: &str) -> Result<OcrResult> {
        let language = language.trim();
        let language = if language.is_empty() {
            "spa+eng"
        } else {
            language
        };

        let mut tess =
            leptess::LepTess::new(self.data_path.as_deref(), language).map_err(|error| {
                KoraError::Unsupported(format!(
                    "no se pudo iniciar Tesseract con los idiomas {language}: {error}. \
                 Instala tesseract y los datos de idioma (spa, eng)"
                ))
            })?;

        tess.set_image_from_mem(image).map_err(|error| {
            KoraError::Internal(format!("Tesseract no pudo leer la imagen: {error}"))
        })?;
        tess.set_source_resolution(300);

        let tsv = tess.get_tsv_text(0).map_err(|error| {
            KoraError::Internal(format!("Tesseract no pudo generar el TSV: {error}"))
        })?;

        let words = parse_tsv(&tsv);
        let text = words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let confidence = if words.is_empty() {
            0.0
        } else {
            words.iter().map(|word| word.confidence).sum::<f32>() / words.len() as f32
        };

        Ok(OcrResult {
            text,
            language: language.to_string(),
            confidence,
            words,
        })
    }
}

/// Parser del TSV de Tesseract (nivel 5 = palabra).
pub fn parse_tsv(tsv: &str) -> Vec<OcrWord> {
    let mut words = Vec::new();

    for line in tsv.lines() {
        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() < 12 {
            continue;
        }
        if columns[0].trim() != "5" {
            continue;
        }

        let text = columns[11].trim();
        if text.is_empty() {
            continue;
        }

        let Ok(left) = columns[6].parse::<f32>() else {
            continue;
        };
        let Ok(top) = columns[7].parse::<f32>() else {
            continue;
        };
        let Ok(width) = columns[8].parse::<f32>() else {
            continue;
        };
        let Ok(height) = columns[9].parse::<f32>() else {
            continue;
        };
        let confidence = columns[10].parse::<f32>().unwrap_or(-1.0);

        words.push(OcrWord {
            text: text.to_string(),
            x: left,
            y: top,
            width,
            height,
            confidence: if confidence < 0.0 { 0.0 } else { confidence },
        });
    }

    words
}

#[cfg(not(target_os = "android"))]
fn find_tessdata() -> Option<String> {
    if let Ok(path) = std::env::var("TESSDATA_PREFIX") {
        let path = Path::new(&path);
        let candidate = if path.file_name().is_some_and(|name| name == "tessdata") {
            path.to_path_buf()
        } else {
            path.join("tessdata")
        };
        if has_language_files(&candidate) {
            return Some(candidate.display().to_string());
        }
    }

    for candidate in [
        "/usr/share/tesseract/tessdata",
        "/usr/share/tessdata",
        "/usr/local/share/tessdata",
        "/opt/homebrew/share/tessdata",
    ] {
        let path = Path::new(candidate);
        if has_language_files(path) {
            return Some(candidate.to_string());
        }
    }

    None
}

#[cfg(not(target_os = "android"))]
fn has_language_files(path: &Path) -> bool {
    path.join("eng.traineddata").is_file() || path.join("spa.traineddata").is_file()
}

/// Implementación inactiva para plataformas sin Tesseract empaquetado.
pub struct PlatformOcrEngine;

impl OcrEngine for PlatformOcrEngine {
    fn id(&self) -> &'static str {
        "platform"
    }

    fn available(&self) -> bool {
        false
    }

    fn recognize(&self, _image: &[u8], _language: &str) -> Result<OcrResult> {
        Err(KoraError::Unsupported(
            "el OCR con Tesseract no está disponible en esta plataforma; \
             instala las librerías nativas o usa la versión de escritorio"
                .into(),
        ))
    }
}

/// Motor por defecto según la plataforma.
pub fn default_engine() -> Box<dyn OcrEngine> {
    #[cfg(not(target_os = "android"))]
    {
        Box::new(TesseractOcrEngine::new())
    }
    #[cfg(target_os = "android")]
    {
        Box::new(PlatformOcrEngine)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_word_rows_from_tsv() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   1\t1\t0\t0\t0\t0\t0\t0\t100\t100\t-1\t\n\
                   5\t1\t1\t1\t1\t1\t10\t20\t30\t12\t91.5\tHola\n\
                   5\t1\t1\t1\t1\t2\t45\t20\t35\t12\t88.0\tmundo\n";

        let words = parse_tsv(tsv);
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hola");
        assert_eq!(words[0].x, 10.0);
        assert_eq!(words[0].y, 20.0);
        assert_eq!(words[0].confidence, 91.5);
        assert_eq!(words[1].text, "mundo");
    }

    #[test]
    fn ignores_invalid_rows() {
        let tsv = "garbage\n5\t1\t1\t1\t1\t1\tx\t20\t30\t12\t91.5\tHola\n";
        assert!(parse_tsv(tsv).is_empty());
    }
}
