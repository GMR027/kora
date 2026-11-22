use serde::{Deserialize, Serialize};

use super::cast::CastMember;
use super::chapters::ChapterSource;
use super::dsp::AudioTreatment;
use super::karaoke::KaraokeLine;
use super::narration::NarrationSettings;
use super::scope::AudiobookScope;

pub const MANIFEST_VERSION: u32 = 1;

/// Voz que produjo el audiolibro. Se guarda el nombre para no depender de que
/// el perfil siga existiendo cuando se abre el reproductor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVoice {
    pub id: String,
    pub name: String,
    pub cloned: bool,
}

/// Un capítulo del M4B y su ancla en el documento original. `section` más los
/// offsets permiten resaltar el texto mientras suena el audio.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestChapter {
    pub index: usize,
    pub title: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub section: usize,
    pub offset_start: usize,
    pub offset_end: usize,
    /// Frases sincronizadas con el audio. Los manifiestos anteriores a esta
    /// fase no lo incluyen y se leen como una lista vacía.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub karaoke: Vec<KaraokeLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookManifest {
    pub version: u32,
    pub book_id: String,
    pub book_title: String,
    pub language: Option<String>,
    pub voice: ManifestVoice,
    pub watermark: bool,
    pub scope: AudiobookScope,
    pub duration_ms: i64,
    /// Ajustes usados al sintetizar. Los manifiestos anteriores a la fase de
    /// estilos no lo incluyen y se leen como `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narration: Option<NarrationSettings>,
    /// Voces resueltas por personaje. El narrador sigue en `voice`; si la
    /// lista está vacía, el audiolibro usa una sola voz.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cast: Vec<CastMember>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapter_source: Option<ChapterSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub treatment: Option<AudioTreatment>,
    pub chapters: Vec<ManifestChapter>,
}

impl AudiobookManifest {
    pub fn chapter_at(&self, position_ms: i64) -> usize {
        if self.chapters.is_empty() {
            return 0;
        }
        let position = position_ms.max(0) as u64;
        self.chapters
            .iter()
            .position(|chapter| {
                position >= chapter.start_ms.max(0) as u64
                    && position < chapter.end_ms.max(0) as u64
            })
            .unwrap_or_else(|| self.chapters.len() - 1)
    }

    pub fn chapter(&self, index: usize) -> Option<&ManifestChapter> {
        self.chapters.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_manifests_written_before_narration_styles() {
        let raw = r#"{
            "version": 1,
            "bookId": "book",
            "bookTitle": "Libro",
            "language": null,
            "voice": { "id": "v", "name": "Voz", "cloned": false },
            "watermark": false,
            "scope": { "kind": "fullBook" },
            "durationMs": 1000,
            "chapters": [{
                "index": 0,
                "title": "Uno",
                "startMs": 0,
                "endMs": 1000,
                "section": 0,
                "offsetStart": 0,
                "offsetEnd": 10
            }]
        }"#;
        let manifest: AudiobookManifest = serde_json::from_str(raw).expect("manifiesto");
        assert!(manifest.narration.is_none());
        assert!(manifest.cast.is_empty());
        assert!(manifest.chapters[0].karaoke.is_empty());
        assert_eq!(manifest.chapter_at(0), 0);
    }

    #[test]
    fn omits_default_narration_when_serializing_none() {
        let manifest = AudiobookManifest {
            version: MANIFEST_VERSION,
            book_id: "book".into(),
            book_title: "Libro".into(),
            language: None,
            voice: ManifestVoice {
                id: "v".into(),
                name: "Voz".into(),
                cloned: false,
            },
            watermark: false,
            scope: AudiobookScope::FullBook,
            duration_ms: 0,
            narration: None,
            cast: Vec::new(),
            chapter_source: None,
            treatment: None,
            chapters: Vec::new(),
        };
        let json = serde_json::to_string(&manifest).expect("json");
        assert!(!json.contains("narration"));
    }
}
