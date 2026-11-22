use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[derive(Debug, Clone)]
pub struct EpubChapter {
    pub title: String,
    pub body: String,
    pub source: Option<String>,
}

/// Genera un EPUB 3 mínimo pero válido: mimetype sin comprimir, contenedor,
/// navegación, capítulos XHTML y portada opcional.
pub fn build_epub(
    path: &Path,
    title: &str,
    author: &str,
    language: &str,
    chapters: &[EpubChapter],
    cover: Option<&Path>,
) -> Result<()> {
    if chapters.is_empty() {
        anyhow::bail!("un EPUB necesita al menos un capítulo");
    }

    let file = std::fs::File::create(path)
        .with_context(|| format!("no se pudo crear {}", path.display()))?;
    let mut zip = ZipWriter::new(file);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("mimetype", stored)?;
    zip.write_all(b"application/epub+zip")?;

    zip.start_file("META-INF/container.xml", deflated)?;
    zip.write_all(CONTAINER.as_bytes())?;

    zip.start_file("OEBPS/style.css", deflated)?;
    zip.write_all(STYLE.as_bytes())?;

    let cover_asset = match cover {
        Some(path) => Some((path, cover_format(path)?)),
        None => None,
    };
    let has_cover = cover_asset.is_some();
    if let Some((cover, (name, _))) = cover_asset.as_ref() {
        let bytes = std::fs::read(cover)
            .with_context(|| format!("no se pudo leer la portada {}", cover.display()))?;
        zip.start_file(format!("OEBPS/{name}"), stored)?;
        zip.write_all(&bytes)?;

        zip.start_file("OEBPS/cover.xhtml", deflated)?;
        zip.write_all(cover_xhtml(title, language, name).as_bytes())?;
    }

    for (index, chapter) in chapters.iter().enumerate() {
        zip.start_file(format!("OEBPS/chapter-{:03}.xhtml", index + 1), deflated)?;
        zip.write_all(chapter_xhtml(&chapter.title, &chapter.body, language).as_bytes())?;
    }

    zip.start_file("OEBPS/nav.xhtml", deflated)?;
    zip.write_all(nav_xhtml(title, language, chapters, has_cover).as_bytes())?;

    zip.start_file("OEBPS/toc.ncx", deflated)?;
    zip.write_all(toc_ncx(title, chapters, has_cover).as_bytes())?;

    zip.start_file("OEBPS/content.opf", deflated)?;
    let cover_meta = cover_asset.as_ref().map(|(_, format)| *format);
    zip.write_all(opf_xhtml(title, author, language, chapters.len(), cover_meta).as_bytes())?;

    zip.finish()?;
    Ok(())
}

const CONTAINER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;

const STYLE: &str = r#"body { font-family: serif; line-height: 1.6; margin: 2em; }
h1 { font-size: 1.4em; margin-bottom: 1.2em; }
p { margin: 0 0 0.9em; text-align: justify; }
.source { color: #666; font-size: 0.85em; font-style: italic; }
.cover { margin: 0; padding: 0; text-align: center; }
.cover img { max-width: 100%; height: auto; }
"#;

fn cover_format(path: &Path) -> Result<(&'static str, &'static str)> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" => Ok(("cover.png", "image/png")),
        "jpg" | "jpeg" => Ok(("cover.jpg", "image/jpeg")),
        "webp" => Ok(("cover.webp", "image/webp")),
        _ => anyhow::bail!(
            "formato de portada no soportado: {} (usa PNG, JPEG o WebP)",
            path.display()
        ),
    }
}

fn cover_xhtml(title: &str, language: &str, image: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{language}">
<head><title>{title}</title><link rel="stylesheet" href="style.css"/></head>
<body class="cover" epub:type="cover"><img src="{image}" alt="{title}"/></body>
</html>
"#,
        language = escape_xml(language),
        title = escape_xml(title),
        image = escape_xml(image),
    )
}

fn chapter_xhtml(title: &str, body: &str, language: &str) -> String {
    let paragraphs: String = body
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| format!("<p>{}</p>", escape_xml(line)))
        .collect();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{language}">
<head><title>{title}</title><link rel="stylesheet" href="style.css"/></head>
<body>
<section epub:type="chapter">
<h1>{title}</h1>
{paragraphs}
</section>
</body>
</html>
"#,
        language = escape_xml(language),
        title = escape_xml(title),
        paragraphs = paragraphs,
    )
}

fn nav_xhtml(title: &str, language: &str, chapters: &[EpubChapter], has_cover: bool) -> String {
    let mut items = String::new();
    if has_cover {
        items.push_str(r#"<li><a href="cover.xhtml">Portada</a></li>"#);
    }
    for (index, chapter) in chapters.iter().enumerate() {
        let source = chapter
            .source
            .as_deref()
            .map(|source| format!(" <span class=\"source\">({})</span>", escape_xml(source)))
            .unwrap_or_default();
        items.push_str(&format!(
            "<li><a href=\"chapter-{:03}.xhtml\">{}</a>{}</li>",
            index + 1,
            escape_xml(&chapter.title),
            source
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{language}">
<head><title>{title}</title><link rel="stylesheet" href="style.css"/></head>
<body>
<nav epub:type="toc" id="toc">
<h1>Índice</h1>
<ol>{items}</ol>
</nav>
</body>
</html>
"#,
        language = escape_xml(language),
        title = escape_xml(title),
        items = items,
    )
}

fn toc_ncx(title: &str, chapters: &[EpubChapter], has_cover: bool) -> String {
    let mut points = String::new();
    let mut order = 1;
    if has_cover {
        points.push_str(&nav_point(order, "Portada", "cover.xhtml"));
        order += 1;
    }
    for (index, chapter) in chapters.iter().enumerate() {
        points.push_str(&nav_point(
            order,
            &chapter.title,
            &format!("chapter-{:03}.xhtml", index + 1),
        ));
        order += 1;
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
<head>
<meta name="dtb:uid" content="{uid}"/>
<meta name="dtb:depth" content="1"/>
</head>
<docTitle><text>{title}</text></docTitle>
<navMap>{points}</navMap>
</ncx>
"#,
        uid = uuid::Uuid::new_v4(),
        title = escape_xml(title),
        points = points,
    )
}

fn nav_point(order: i32, title: &str, href: &str) -> String {
    format!(
        r#"<navPoint id="nav-{order}" playOrder="{order}"><navLabel><text>{title}</text></navLabel><content src="{href}"/></navPoint>"#,
        order = order,
        title = escape_xml(title),
        href = href,
    )
}

fn opf_xhtml(
    title: &str,
    author: &str,
    language: &str,
    chapter_count: usize,
    cover: Option<(&'static str, &'static str)>,
) -> String {
    let mut manifest = String::new();
    if let Some((name, media_type)) = cover {
        manifest.push_str(&format!(
            r#"<item id="cover-image" href="{name}" media-type="{media_type}" properties="cover-image"/><item id="cover" href="cover.xhtml" media-type="application/xhtml+xml"/>"#
        ));
    }
    for index in 0..chapter_count {
        manifest.push_str(&format!(
            r#"<item id="chapter-{index}" href="chapter-{:03}.xhtml" media-type="application/xhtml+xml"/>"#,
            index + 1
        ));
    }
    manifest.push_str(
        r#"<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="style" href="style.css" media-type="text/css"/>"#,
    );

    // El índice y la portada quedan fuera del spine: así el lector solo
    // recorre los capítulos reales y el audiolibro no narra el índice.
    let mut spine = String::new();
    for index in 0..chapter_count {
        spine.push_str(&format!(r#"<itemref idref="chapter-{index}"/>"#));
    }

    let cover_meta = if cover.is_some() {
        r#"<meta name="cover" content="cover-image"/>"#
    } else {
        ""
    };

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="bookid" xml:lang="{language}">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:identifier id="bookid">urn:uuid:{uid}</dc:identifier>
<dc:title>{title}</dc:title>
<dc:creator>{author}</dc:creator>
<dc:language>{language}</dc:language>
<meta property="dcterms:modified">{modified}</meta>
{cover_meta}
</metadata>
<manifest>{manifest}</manifest>
<spine toc="ncx">{spine}</spine>
</package>
"#,
        uid = uuid::Uuid::new_v4(),
        title = escape_xml(title),
        author = escape_xml(author),
        language = escape_xml(language),
        modified = Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
        cover_meta = cover_meta,
        manifest = manifest,
        spine = spine,
    )
}

pub fn escape_xml(value: &str) -> String {
    // XML 1.0 no admite la mayoría de caracteres de control. El texto de un
    // PDF puede traer form feeds o separadores que romperían el EPUB.
    let sanitized: String = value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\t' | '\n' | '\r'))
        .collect();

    sanitized
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epub_contains_the_mimetype_stored_first() {
        let workspace = tempfile::tempdir().expect("tmp");
        let path = workspace.path().join("mini.epub");
        let chapters = vec![
            EpubChapter {
                title: "Uno".into(),
                body: "Hola mundo.\nOtra línea.".into(),
                source: Some("Libro A".into()),
            },
            EpubChapter {
                title: "Dos".into(),
                body: "Texto <raro> & ampersand.".into(),
                source: None,
            },
        ];
        let cover = workspace.path().join("cover.png");
        crate::tejido::cover::render_cover("Mini", "Kora", &cover).expect("portada");
        build_epub(&path, "Mini", "Kora", "es", &chapters, Some(&cover)).expect("epub");

        let file = std::fs::File::open(&path).expect("abrir");
        let mut archive = zip::ZipArchive::new(file).expect("zip");
        {
            let mut mimetype = archive.by_name("mimetype").expect("mimetype");
            let mut value = String::new();
            std::io::Read::read_to_string(&mut mimetype, &mut value).expect("leer");
            assert_eq!(value, "application/epub+zip");
        }
        assert!(archive.by_name("OEBPS/nav.xhtml").is_ok());
        assert!(archive.by_name("OEBPS/chapter-002.xhtml").is_ok());
        assert!(archive.by_name("OEBPS/cover.png").is_ok());
        assert!(archive.by_name("OEBPS/cover.xhtml").is_ok());
        drop(archive);

        let inspection = crate::reader::EpubParser::new()
            .inspect(&path)
            .expect("el EPUB generado debe abrirse con rbook");
        assert_eq!(inspection.sections.len(), 2);
    }

    #[test]
    fn keeps_jpeg_cover_media_type() {
        let workspace = tempfile::tempdir().expect("tmp");
        let path = workspace.path().join("mini.epub");
        let cover = workspace.path().join("cover.jpg");
        std::fs::write(&cover, b"jpeg-bytes").expect("portada");
        let chapters = vec![EpubChapter {
            title: "Uno".into(),
            body: "Hola.".into(),
            source: None,
        }];

        build_epub(&path, "Mini", "Kora", "es", &chapters, Some(&cover)).expect("epub");

        let file = std::fs::File::open(&path).expect("abrir");
        let mut archive = zip::ZipArchive::new(file).expect("zip");
        assert!(archive.by_name("OEBPS/cover.jpg").is_ok());
        let mut opf = String::new();
        std::io::Read::read_to_string(
            &mut archive.by_name("OEBPS/content.opf").expect("opf"),
            &mut opf,
        )
        .expect("leer opf");
        assert!(
            opf.contains(r#"href="cover.jpg" media-type="image/jpeg""#),
            "{opf}"
        );
        let mut cover_xhtml = String::new();
        std::io::Read::read_to_string(
            &mut archive.by_name("OEBPS/cover.xhtml").expect("cover"),
            &mut cover_xhtml,
        )
        .expect("leer cover");
        assert!(cover_xhtml.contains("cover.jpg"), "{cover_xhtml}");
    }
}
