use std::path::PathBuf;

use kora_lib::features::ocr::OcrEngine;
use kora_lib::features::ocr::TesseractOcrEngine;
use kora_lib::reader::PdfParser;
use lopdf::{Document, Object, Stream, dictionary};

fn write_pdf(path: &PathBuf, content: &[u8]) -> PathBuf {
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();

    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let content_id = document.add_object(Stream::new(dictionary! {}, content.to_vec()));

    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Contents" => content_id,
        "Resources" => dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        },
    });

    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        }),
    );

    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    document.trailer.set("Root", catalog_id);

    document.save(path).expect("guardar PDF de prueba");
    path.clone()
}

fn pdfium_available() -> bool {
    std::env::var("KORA_PDFIUM_PATH").is_ok()
}

#[test]
fn extracts_native_text_and_renders_page() {
    if !pdfium_available() {
        eprintln!("KORA_PDFIUM_PATH no definido; se omite la prueba de PDFium");
        return;
    }

    let directory = std::env::temp_dir().join(format!("kora-pdf-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).expect("tempdir");
    let path = write_pdf(
        &directory.join("texto.pdf"),
        b"BT /F1 24 Tf 72 720 Td (Capitulo 1) Tj ET\nBT /F1 12 Tf 72 680 Td (Hola mundo, esto es una prueba.) Tj ET",
    );

    let parser = PdfParser::new();
    let inspection = parser.inspect(&path).expect("inspeccionar");
    assert_eq!(inspection.page_count, 1);
    assert!(!inspection.scanned);

    let layer = parser.page_text(&path, 0).expect("texto nativo");
    let text = layer.text();
    assert!(text.contains("Capitulo"), "texto extraído: {text:?}");
    assert!(text.contains("mundo"), "texto extraído: {text:?}");
    assert_eq!(layer.source, "native");

    let rendered = parser.render_page(&path, 0, 150.0).expect("renderizar");
    assert!(rendered.width > 100);
    assert!(rendered.height > 100);
    assert!(rendered.png.starts_with(b"\x89PNG"));

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn detects_scanned_pdf_without_text() {
    if !pdfium_available() {
        eprintln!("KORA_PDFIUM_PATH no definido; se omite la prueba de PDFium");
        return;
    }

    let directory = std::env::temp_dir().join(format!("kora-pdf-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).expect("tempdir");
    let path = write_pdf(&directory.join("escaneado.pdf"), b"");

    let parser = PdfParser::new();
    let inspection = parser.inspect(&path).expect("inspeccionar");
    assert_eq!(inspection.page_count, 1);
    assert!(inspection.scanned);

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn recognizes_rendered_text_with_tesseract() {
    if !pdfium_available() {
        eprintln!("KORA_PDFIUM_PATH no definido; se omite la prueba de OCR");
        return;
    }

    let directory = std::env::temp_dir().join(format!("kora-ocr-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).expect("tempdir");
    let path = write_pdf(
        &directory.join("ocr.pdf"),
        b"BT /F1 32 Tf 72 700 Td (Hola mundo) Tj ET",
    );

    let parser = PdfParser::new();
    let rendered = parser.render_page(&path, 0, 300.0).expect("renderizar");
    let engine = TesseractOcrEngine::new();
    let result = engine
        .recognize(&rendered.png, "spa")
        .expect("reconocer texto");

    assert!(
        result.text.to_lowercase().contains("hola"),
        "texto OCR: {:?}",
        result.text
    );

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn builds_a_heuristic_index_from_native_text() {
    if !pdfium_available() {
        eprintln!("KORA_PDFIUM_PATH no definido; se omite la prueba de índice");
        return;
    }

    let directory = std::env::temp_dir().join(format!("kora-index-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).expect("tempdir");
    let path = write_pdf(
        &directory.join("indice.pdf"),
        b"BT /F1 26 Tf 72 720 Td (Capitulo 1) Tj ET\nBT /F1 12 Tf 72 670 Td (Este es el cuerpo del capitulo con texto normal.) Tj ET",
    );

    let parser = PdfParser::new();
    let lines = parser.index_lines(&path, 100).expect("líneas");
    let entries = kora_lib::reader::auto_index::build_from_lines(&lines);

    assert_eq!(entries.len(), 1, "entradas: {entries:?}");
    assert_eq!(entries[0].title, "Capitulo 1");
    assert_eq!(entries[0].page, 0);
    assert_eq!(entries[0].level, 0);

    std::fs::remove_dir_all(&directory).ok();
}
