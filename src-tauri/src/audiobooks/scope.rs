use serde::{Deserialize, Serialize};

/// Alcance de una generación. En EPUB `section` es el índice del capítulo; en
/// PDF es el número de página. Los offsets de `selection` son unidades UTF-16
/// sobre el texto visible de esa sección, igual que en las anotaciones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AudiobookScope {
    Chapter {
        section: usize,
    },
    Selection {
        section: usize,
        start: usize,
        end: usize,
    },
    FullBook,
}

impl AudiobookScope {
    pub fn label(&self) -> String {
        match self {
            Self::Chapter { section } => format!("capítulo {}", section + 1),
            Self::Selection {
                section,
                start,
                end,
            } => format!("selección {start}-{end} de la sección {}", section + 1),
            Self::FullBook => "libro completo".to_string(),
        }
    }
}

/// Texto resuelto de una sección, con el ancla necesaria para el manifiesto.
#[derive(Debug, Clone)]
pub struct SourceSection {
    pub section: usize,
    pub title: String,
    pub text: String,
    pub offset_start: usize,
    pub offset_end: usize,
}

impl SourceSection {
    pub fn utf16_len(&self) -> usize {
        self.text.encode_utf16().count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_deserializes_from_the_frontend_shape() {
        let chapter: AudiobookScope =
            serde_json::from_str(r#"{"kind":"chapter","section":2}"#).expect("chapter");
        assert_eq!(chapter, AudiobookScope::Chapter { section: 2 });

        let selection: AudiobookScope =
            serde_json::from_str(r#"{"kind":"selection","section":1,"start":10,"end":40}"#)
                .expect("selection");
        assert_eq!(
            selection,
            AudiobookScope::Selection {
                section: 1,
                start: 10,
                end: 40
            }
        );

        let full: AudiobookScope = serde_json::from_str(r#"{"kind":"fullBook"}"#).expect("full");
        assert_eq!(full, AudiobookScope::FullBook);
    }
}
