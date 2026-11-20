use std::collections::HashMap;
use std::path::Path;

use async_trait::async_trait;
use base64::Engine;
use rbook::Epub;
use rbook::epub::rewrite::{EpubRewriteOptions, PathRewrite};
use scraper::{Html, Selector};

use super::DocumentParser;
use super::model::{ChapterContent, Document, DocumentFormat, Locator, Section};
use super::sanitize;
use crate::error::{KoraError, Result};

/// Límites para no inyectar capítulos gigantes en el WebView. Las imágenes
/// que superan el límite se descartan en lugar de romper el capítulo.
const MAX_IMAGE_BYTES: usize = 3 * 1024 * 1024;
const MAX_TOTAL_IMAGE_BYTES: usize = 12 * 1024 * 1024;
const MAX_COVER_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct CoverImage {
    pub bytes: Vec<u8>,
    pub extension: &'static str,
}

/// Resultado de inspeccionar un EPUB sin cargar todos sus capítulos.
#[derive(Debug, Clone)]
pub struct EpubInspection {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub sections: Vec<Section>,
    pub cover: Option<CoverImage>,
}

/// Parser EPUB basado en `rbook`. Todo el acceso a `rbook` queda encapsulado
/// aquí para poder cambiar de crate sin tocar comandos ni frontend.
pub struct EpubParser;

impl EpubParser {
    pub fn new() -> Self {
        Self
    }

    pub fn inspect(&self, path: &Path) -> Result<EpubInspection> {
        let epub = open_epub(path)?;
        let metadata = epub.metadata();

        let title = metadata
            .title()
            .map(|title| title.value().trim().to_string())
            .filter(|title| !title.is_empty());

        let author = metadata
            .creators()
            .next()
            .map(|creator| creator.value().trim().to_string())
            .filter(|author| !author.is_empty());

        let language = metadata
            .language()
            .map(|language| language.value().trim().to_string())
            .filter(|language| !language.is_empty());

        let publisher = metadata
            .publishers()
            .next()
            .map(|publisher| publisher.value().trim().to_string())
            .filter(|publisher| !publisher.is_empty());

        let description = metadata
            .description()
            .map(|description| description.value().trim().to_string())
            .filter(|description| !description.is_empty());

        Ok(EpubInspection {
            title,
            author,
            language,
            publisher,
            description,
            sections: sections(&epub),
            cover: cover(&epub),
        })
    }

    pub fn chapter(&self, path: &Path, index: usize) -> Result<ChapterContent> {
        let epub = open_epub(path)?;
        let spine_len = epub.spine().len();

        if index >= spine_len {
            return Err(KoraError::NotFound(format!(
                "capítulo {index} (el libro tiene {spine_len})"
            )));
        }

        let spine_entry = epub
            .spine()
            .get(index)
            .ok_or_else(|| KoraError::NotFound(format!("capítulo {index}")))?;

        let manifest_entry = spine_entry.manifest_entry().ok_or_else(|| {
            KoraError::InvalidInput(format!("el capítulo {index} no apunta a un recurso válido"))
        })?;

        let rewrite = EpubRewriteOptions::new().rewrite_paths(PathRewrite::root_relative());
        let xhtml = manifest_entry.read_str_with(&rewrite).map_err(|error| {
            KoraError::InvalidInput(format!("no se pudo leer el capítulo: {error}"))
        })?;

        let body = sanitize::extract_body(&xhtml);
        let images = collect_images(&epub, &body);
        let html = sanitize::sanitize_chapter(&body, &images);
        let char_count = sanitize::text_length(&html);

        let title = section_title(&epub, manifest_entry.href().path().as_str())
            .or_else(|| first_heading(&body))
            .unwrap_or_else(|| format!("Capítulo {}", index + 1));

        Ok(ChapterContent {
            book_id: String::new(),
            index,
            title,
            html,
            char_count,
            has_previous: index > 0,
            has_next: index + 1 < spine_len,
        })
    }
}

impl Default for EpubParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DocumentParser for EpubParser {
    async fn open(&self, path: &Path) -> Result<Document> {
        let inspection = self.inspect(path)?;

        Ok(Document {
            id: path.to_string_lossy().into_owned(),
            format: DocumentFormat::Epub,
            title: inspection.title.unwrap_or_else(|| "Sin título".to_string()),
            author: inspection.author,
            language: inspection.language,
            publisher: inspection.publisher,
            description: inspection.description,
            sections: inspection.sections,
        })
    }
}

fn open_epub(path: &Path) -> Result<Epub> {
    Epub::open(path)
        .map_err(|error| KoraError::InvalidInput(format!("no se pudo abrir el EPUB: {error}")))
}

fn sections(epub: &Epub) -> Vec<Section> {
    let labels = section_labels(epub);

    epub.spine()
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let title = entry
                .manifest_entry()
                .and_then(|manifest| labels.get(manifest.href().path().as_str()).cloned());

            Section {
                id: entry.idref().to_string(),
                title,
                locator: Locator {
                    section: index,
                    offset: 0,
                },
            }
        })
        .collect()
}

fn section_labels(epub: &Epub) -> HashMap<String, String> {
    let mut labels = HashMap::new();

    if let Some(contents) = epub.toc().contents() {
        for entry in contents.flatten() {
            let Some(manifest) = entry.manifest_entry() else {
                continue;
            };

            let label = entry.label().trim();
            if label.is_empty() {
                continue;
            }

            labels
                .entry(manifest.href().path().as_str().to_string())
                .or_insert_with(|| label.to_string());
        }
    }

    labels
}

fn section_title(epub: &Epub, href: &str) -> Option<String> {
    section_labels(epub).get(href).cloned()
}

fn first_heading(body: &str) -> Option<String> {
    let document = Html::parse_fragment(body);

    for tag in ["h1", "h2", "h3"] {
        let selector = Selector::parse(tag).expect("selector de encabezado válido");
        if let Some(element) = document.select(&selector).next() {
            let text = element.text().collect::<String>();
            let text = text.trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }

    None
}

fn cover(epub: &Epub) -> Option<CoverImage> {
    let manifest = epub.manifest();
    let entry = manifest.cover_image().or_else(|| {
        manifest.images().find(|entry| {
            let href = entry.href().as_str().to_ascii_lowercase();
            href.contains("cover") || entry.id().to_ascii_lowercase().contains("cover")
        })
    })?;

    let extension = image_extension(entry.media_type())?;
    let bytes = entry.read_bytes().ok()?;

    if bytes.is_empty() || bytes.len() > MAX_COVER_BYTES {
        return None;
    }

    Some(CoverImage { bytes, extension })
}

fn collect_images(epub: &Epub, body: &str) -> HashMap<String, String> {
    let document = Html::parse_fragment(body);
    let selector = Selector::parse("img").expect("selector img válido");
    let mut images = HashMap::new();
    let mut total = 0usize;

    for image in document.select(&selector) {
        let Some(source) = image.value().attr("src") else {
            continue;
        };

        if source.starts_with("data:") || images.contains_key(source) {
            continue;
        }

        let Ok(bytes) = epub.read_resource_bytes(source) else {
            continue;
        };

        if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
            continue;
        }

        total = total.saturating_add(bytes.len());
        if total > MAX_TOTAL_IMAGE_BYTES {
            break;
        }

        let media_type = epub
            .manifest()
            .by_href(source)
            .map(|entry| entry.media_type().to_string())
            .or_else(|| mime_from_path(source));

        let Some(media_type) = media_type else {
            continue;
        };

        if !media_type.starts_with("image/") || media_type.contains("svg") {
            continue;
        }

        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        images.insert(
            source.to_string(),
            format!("data:{media_type};base64,{encoded}"),
        );
    }

    images
}

fn image_extension(media_type: &str) -> Option<&'static str> {
    match media_type.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        _ => None,
    }
}

fn mime_from_path(path: &str) -> Option<String> {
    let extension = path.rsplit('.').next()?.to_ascii_lowercase();
    let mime = match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => return None,
    };

    Some(mime.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write;

    use tempfile::TempDir;
    use zip::CompressionMethod;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    use super::*;

    const PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xdd, 0x8d, 0xb0, 0x00, 0x00, 0x00,
        0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    fn write_entry(zip: &mut ZipWriter<File>, name: &str, content: &[u8]) {
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file(name, options).expect("entrada zip");
        zip.write_all(content).expect("escribir entrada");
    }

    fn fixture() -> (TempDir, std::path::PathBuf) {
        let directory = TempDir::new().expect("tempdir");
        let path = directory.path().join("fixture.epub");
        let file = File::create(&path).expect("crear epub");
        let mut zip = ZipWriter::new(file);

        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file("mimetype", stored).expect("mimetype");
        zip.write_all(b"application/epub+zip").expect("mimetype");

        write_entry(
            &mut zip,
            "META-INF/container.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="EPUB/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
        );

        write_entry(
            &mut zip,
            "EPUB/content.opf",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="uid">urn:uuid:00000000-0000-4000-8000-000000000001</dc:identifier>
    <dc:title>Fixture</dc:title>
    <dc:creator>Autora</dc:creator>
    <dc:language>es</dc:language>
    <dc:publisher>Editorial</dc:publisher>
    <dc:description>Descripción</dc:description>
    <meta property="dcterms:modified">2020-01-01T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2" href="c2.xhtml" media-type="application/xhtml+xml"/>
    <item id="img" href="images/pixel.png" media-type="image/png"/>
    <item id="cover" href="images/cover.png" media-type="image/png" properties="cover-image"/>
  </manifest>
  <spine>
    <itemref idref="c1"/>
    <itemref idref="c2"/>
  </spine>
</package>"#
                .as_bytes(),
        );

        write_entry(
            &mut zip,
            "EPUB/nav.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
  <head><title>Índice</title></head>
  <body>
    <nav epub:type="toc">
      <ol>
        <li><a href="c1.xhtml">Uno</a></li>
        <li><a href="c2.xhtml">Dos</a></li>
      </ol>
    </nav>
  </body>
</html>"#
                .as_bytes(),
        );

        write_entry(
            &mut zip,
            "EPUB/c1.xhtml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <head><title>Uno</title><style>p { color: red }</style></head>
  <body>
    <h1>Uno</h1>
    <p onclick="alert(1)">Hola <strong>mundo</strong>.</p>
    <img src="images/pixel.png" alt="pixel"/>
    <script>alert('x')</script>
  </body>
</html>"#,
        );

        write_entry(
            &mut zip,
            "EPUB/c2.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <head><title>Dos</title></head>
  <body><h1>Dos</h1><p>Segundo capítulo.</p></body>
</html>"#
                .as_bytes(),
        );

        write_entry(&mut zip, "EPUB/images/pixel.png", PNG);
        write_entry(&mut zip, "EPUB/images/cover.png", PNG);

        zip.finish().expect("cerrar epub");
        (directory, path)
    }

    #[test]
    fn inspects_metadata_sections_and_cover() {
        let (_directory, path) = fixture();
        let inspection = EpubParser::new().inspect(&path).expect("inspeccionar");

        assert_eq!(inspection.title.as_deref(), Some("Fixture"));
        assert_eq!(inspection.author.as_deref(), Some("Autora"));
        assert_eq!(inspection.language.as_deref(), Some("es"));
        assert_eq!(inspection.publisher.as_deref(), Some("Editorial"));
        assert_eq!(inspection.sections.len(), 2);
        assert_eq!(inspection.sections[0].title.as_deref(), Some("Uno"));
        assert!(inspection.cover.is_some());
    }

    #[test]
    fn sanitizes_chapter_and_inlines_images() {
        let (_directory, path) = fixture();
        let chapter = EpubParser::new().chapter(&path, 0).expect("capítulo");

        assert_eq!(chapter.index, 0);
        assert!(chapter.has_next);
        assert!(!chapter.has_previous);
        assert!(chapter.html.contains("Hola"));
        assert!(chapter.html.contains("data:image/png;base64,"));
        assert!(!chapter.html.contains("<script"));
        assert!(!chapter.html.contains("<style"));
        assert!(!chapter.html.to_ascii_lowercase().contains("onclick"));
        assert!(chapter.char_count > 0);
    }

    #[test]
    fn rejects_out_of_range_chapters() {
        let (_directory, path) = fixture();
        assert!(EpubParser::new().chapter(&path, 99).is_err());
    }

    #[test]
    fn rejects_non_epub_files() {
        let directory = TempDir::new().expect("tempdir");
        let path = directory.path().join("no-es-epub.epub");
        std::fs::write(&path, b"no soy un epub").expect("escribir");

        assert!(EpubParser::new().inspect(&path).is_err());
    }
}
