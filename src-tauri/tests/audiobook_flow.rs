//! Pruebas de extremo a extremo de la sección de audiolibros.
//!
//! Usan el runtime simulado de Tauri, una base de datos temporal y los modelos
//! reales de TTS/Whisper instalados en la app. Se activan explícitamente:
//!
//! ```bash
//! KORA_E2E_AUDIOBOOKS=1 cargo test --test audiobook_flow
//! ```
//!
//! Variables opcionales:
//! - `KORA_E2E_ASSETS`: carpeta `assets/` de Kora con `tts/` y `whisper/`.
//! - `KORA_E2E_BOOK`: EPUB de origen (por defecto, Hábitos atómicos).
//! - `KORA_E2E_AUDIO_SOURCE`: MP3 de El arte de la guerra.
//!
//! El audio de origen solo se lee: se extrae un fragmento de tres minutos en
//! la carpeta temporal para probar la importación sin transcribir dos horas.

#![cfg(not(target_os = "android"))]

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use kora_lib::audiobooks::{
    AudioImportMode, AudioImportOptions, AudioTreatment, AudiobookScope, CastInput, ChapterSource,
    EqPreset, NarrationSettings, NarrationStyle,
};
use kora_lib::commands::accounts;
use kora_lib::commands::annotations;
use kora_lib::commands::audiobook_voice;
use kora_lib::commands::audiobooks;
use kora_lib::commands::library;
use kora_lib::commands::reader;
use kora_lib::commands::tejido;
use kora_lib::error::KoraError;
use kora_lib::reader::extract_text;
use kora_lib::state::AppState;
use kora_lib::storage::{
    AssetKind, AssetRef, AudiobookOrigin, AudiobookStatus, BookFormat, BookOrigin,
    ClassificationConfig, NewVoiceNote, SharePermission,
};
use tauri::Manager;
use tauri::utils::config::AppDirectoriesOverride;

const DEFAULT_ASSETS: &str = "/home/edgar/.local/share/dev.kora.reader/assets";
const DEFAULT_BOOK: &str = "/home/edgar/.local/share/dev.kora.reader/assets/books/daa008f8905a7ba3ae4de4533563aa77b20641eb26b45dffc1d1cf6581e92ca3.epub";
const DEFAULT_AUDIO_SOURCE: &str = "/home/edgar/Descargas/El arte de la guerra.mp3";

type TestApp = tauri::App<tauri::Wry>;

struct Harness {
    app: TestApp,
    runtime: tokio::runtime::Runtime,
    temp: tempfile::TempDir,
}

fn enabled() -> bool {
    std::env::var("KORA_E2E_AUDIOBOOKS")
        .map(|value| value == "1")
        .unwrap_or(false)
}

fn assets_root() -> PathBuf {
    PathBuf::from(std::env::var("KORA_E2E_ASSETS").unwrap_or_else(|_| DEFAULT_ASSETS.into()))
}

fn book_source() -> PathBuf {
    PathBuf::from(std::env::var("KORA_E2E_BOOK").unwrap_or_else(|_| DEFAULT_BOOK.into()))
}

fn harness() -> Harness {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("data");
    std::fs::create_dir_all(&root).expect("raíz");

    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().app.app_directories_override =
        Some(AppDirectoriesOverride::Root(root.clone()));
    let app = tauri::Builder::default()
        .build(context)
        .expect("app de prueba");
    let state = AppState::new(app.handle()).expect("estado de la app");
    assert!(app.manage(state), "el estado ya estaba registrado");

    // Los motores se resuelven bajo demanda: enlazamos los modelos reales
    // después de crear el árbol temporal.
    for name in ["tts", "whisper"] {
        let target = root.join("assets").join(name);
        if target.is_symlink() || target.is_file() {
            let _ = std::fs::remove_file(&target);
        } else if target.is_dir() {
            let _ = std::fs::remove_dir(&target);
        }
        std::os::unix::fs::symlink(assets_root().join(name), &target)
            .unwrap_or_else(|error| panic!("no se pudo enlazar {name}: {error}"));
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime");
    Harness { app, runtime, temp }
}

async fn disable_classification(app: &TestApp) {
    let state = app.state::<AppState>();
    let mut config = state
        .classification
        .config()
        .await
        .unwrap_or_else(|_| ClassificationConfig::default());
    if config.enabled || config.classify_on_import {
        config.enabled = false;
        config.classify_on_import = false;
        if let Err(error) = state.classification.save_config(config).await {
            eprintln!("no se pudo desactivar la clasificación en el test: {error}");
        }
    }
}

async fn import_book(app: &TestApp) -> String {
    let source = book_source();
    assert!(source.is_file(), "falta el EPUB {}", source.display());
    let summary = library::library_import(
        app.state(),
        app.handle().clone(),
        source.to_string_lossy().to_string(),
    )
    .await
    .expect("importar libro");
    summary.id
}

async fn installed_voice(app: &TestApp) -> String {
    app.state::<AppState>()
        .tts
        .voices()
        .await
        .expect("voces TTS")
        .into_iter()
        .find(|voice| voice.installed)
        .expect("debe haber una voz instalada")
        .id
}

async fn installed_alternatives(app: &TestApp, base: &str) -> Vec<String> {
    app.state::<AppState>()
        .tts
        .voices()
        .await
        .expect("voces TTS")
        .into_iter()
        .filter(|voice| voice.installed && voice.id != base)
        .map(|voice| voice.id)
        .collect()
}

async fn wait_for_queue(app: &TestApp, timeout: Duration) {
    let started = Instant::now();
    loop {
        let view = audiobooks::audiobook_queue_status(app.state())
            .await
            .expect("estado de la cola");
        if view.active.is_none() && view.queue.is_empty() {
            return;
        }
        if started.elapsed() > timeout {
            panic!("la cola no terminó: {view:?}");
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn wait_for_active(app: &TestApp, timeout: Duration) {
    let started = Instant::now();
    loop {
        let view = audiobooks::audiobook_queue_status(app.state())
            .await
            .expect("estado de la cola");
        if view.active.is_some() {
            return;
        }
        if started.elapsed() > timeout {
            panic!("la generación no arrancó: {view:?}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn asset_exists(app: &TestApp, hash: Option<&str>, extension: Option<&str>) -> bool {
    let (Some(hash), Some(extension)) = (hash, extension) else {
        return false;
    };
    let reference = AssetRef {
        kind: AssetKind::Audiobook,
        hash: hash.to_string(),
        extension: extension.to_string(),
    };
    app.state::<AppState>()
        .storage
        .assets
        .exists(&reference)
        .unwrap_or(false)
}

async fn chapter_utf16(app: &TestApp, book_id: &str, section: usize) -> Vec<u16> {
    let chapter = reader::reader_get_chapter(app.state(), book_id.to_string(), section)
        .await
        .expect("capítulo");
    extract_text(&chapter.html).text.encode_utf16().collect()
}

async fn run_generated_tools(h: &Harness) {
    let app = &h.app;
    disable_classification(app).await;
    let state = app.state::<AppState>();
    let book_id = import_book(app).await;
    let voice_id = installed_voice(app).await;

    let characters = audiobooks::audiobook_characters(
        app.state(),
        book_id.clone(),
        AudiobookScope::Chapter { section: 0 },
    )
    .await
    .expect("personajes");
    let alternatives = installed_alternatives(app, &voice_id).await;
    let mut assigned = HashSet::new();
    let mut assignments = Vec::new();
    for (index, character) in characters
        .iter()
        .filter(|character| character.name != "Narrador" && character.name != "Diálogo")
        .enumerate()
    {
        if alternatives.is_empty() {
            break;
        }
        let voice = &alternatives[index % alternatives.len()];
        assigned.insert(character.name.clone());
        assignments.push(kora_lib::audiobooks::CastAssignment {
            character: character.name.clone(),
            voice_id: voice.clone(),
        });
    }
    let cast = CastInput {
        enabled: !assigned.is_empty(),
        assignments,
    };
    let narration = NarrationSettings {
        style: NarrationStyle::Dramatic,
        speed: 1.05,
        multi_voice: cast.enabled,
    };

    let queued = audiobooks::enqueue_audiobook(
        app.state(),
        app.handle().clone(),
        book_id.clone(),
        AudiobookScope::Chapter { section: 0 },
        voice_id,
        Some(narration),
        Some(cast.clone()),
    )
    .await
    .expect("encolar capítulo");
    assert!(
        queued.active.is_some() || !queued.queue.is_empty(),
        "el trabajo debe quedar activo o en cola"
    );
    wait_for_queue(app, Duration::from_secs(900)).await;

    let list = audiobooks::list_audiobooks(app.state())
        .await
        .expect("listar");
    let generated = list
        .iter()
        .find(|item| item.book_id == book_id && item.status == AudiobookStatus::Done)
        .unwrap_or_else(|| panic!("no se generó el audiolibro: {list:?}"));
    assert!(generated.duration_ms > 0, "duración vacía");
    assert!(generated.chapter_count >= 1);
    assert!(generated.narration.is_some());
    assert_eq!(
        generated.scope.as_ref(),
        Some(&AudiobookScope::Chapter { section: 0 })
    );

    let chapters = audiobooks::audiobook_chapters(app.state(), generated.id.clone())
        .await
        .expect("capítulos");
    assert!(!chapters.is_empty());
    assert!(
        chapters
            .iter()
            .all(|chapter| chapter.end_ms > chapter.start_ms)
    );
    assert!(chapters.iter().any(|chapter| chapter.karaoke));

    let karaoke = audiobooks::audiobook_karaoke(app.state(), generated.id.clone(), 0)
        .await
        .expect("karaoke");
    assert!(!karaoke.lines.is_empty(), "karaoke sin líneas");
    let line = &karaoke.lines[karaoke.lines.len() / 2];
    let position = karaoke.start_ms + line.start_ms + 50;
    let locator = audiobooks::audiobook_locator(app.state(), generated.id.clone(), position)
        .await
        .expect("localizador");
    assert!(!locator.quote.trim().is_empty());
    let utf16 = chapter_utf16(app, &book_id, locator.section).await;
    assert!(locator.offset_start >= 0);
    assert!((locator.offset_end as usize) <= utf16.len());
    let quote = String::from_utf16_lossy(
        &utf16[locator.offset_start as usize..locator.offset_end as usize],
    );
    assert_eq!(
        quote.trim(),
        locator.quote.trim(),
        "el ancla no coincide con el texto"
    );

    let waveform = audiobooks::audiobook_waveform(app.state(), generated.id.clone(), Some(128))
        .await
        .expect("waveform");
    assert_eq!(waveform.buckets.len(), 128);
    assert!(waveform.duration_ms > 0);

    let favorite = audiobooks::audiobook_set_favorite(app.state(), generated.id.clone(), true)
        .await
        .expect("favorito");
    assert!(favorite.favorite);

    let export = h.temp.path().join("export.m4b");
    let exported = audiobooks::export_audiobook(
        app.state(),
        generated.id.clone(),
        export.to_string_lossy().to_string(),
    )
    .await
    .expect("exportar");
    assert_eq!(PathBuf::from(exported), export);
    assert!(export.is_file() && std::fs::metadata(&export).expect("meta").len() > 0);
    assert!(
        asset_exists(
            app,
            generated.audio_hash.as_deref(),
            generated.audio_ext.as_deref()
        ),
        "falta el asset de audio"
    );

    let treated = audiobooks::apply_audiobook_treatment(
        app.state(),
        app.handle().clone(),
        generated.id.clone(),
        AudioTreatment {
            normalize: true,
            target_lufs: -18.0,
            eq: EqPreset::ClearVoice,
        },
    )
    .await
    .expect("tratamiento");
    assert!(treated.treatment.is_some());
    assert!((treated.duration_ms - generated.duration_ms).abs() < 2_500);
    let treated_waveform =
        audiobooks::audiobook_waveform(app.state(), treated.id.clone(), Some(64))
            .await
            .expect("waveform tratada");
    assert_eq!(treated_waveform.buckets.len(), 64);

    accounts::create_account(
        app.state(),
        "e2e".to_string(),
        "E2E".to_string(),
        "1234".to_string(),
    )
    .await
    .expect("cuenta local");
    let share = accounts::create_audiobook_share(
        app.state(),
        "1234".to_string(),
        treated.id.clone(),
        SharePermission::Copy,
        None,
        Some(2),
    )
    .await
    .expect("share");
    assert!(
        share
            .key
            .as_deref()
            .unwrap_or_default()
            .starts_with("SHARE-AUDIOBOOK")
    );
    assert!(share.qr_svg.is_some());

    let marker_status = audiobook_voice::voice_marker_status(app.state())
        .await
        .expect("estado de marcadores");
    assert!(marker_status.supported);
    assert!(
        marker_status.model_ready,
        "Whisper Base debe estar instalado"
    );
    let status =
        audiobook_voice::save_voice_marker_phrase(app.state(), "Kora, guarda esto".to_string())
            .await
            .expect("guardar frase");
    assert_eq!(status.phrase.as_deref(), Some("Kora, guarda esto"));

    // Marcador de voz simulado: comprueba listado y limpieza del Opus.
    let marker_id = kora_lib::storage::repos::new_id();
    let opus_path = kora_lib::annotations::note_audio_path(&state.layout, &marker_id);
    std::fs::create_dir_all(opus_path.parent().expect("carpeta")).expect("crear");
    let samples: Vec<f32> = (0..1_600)
        .map(|index| (index as f32 * 440.0 * std::f32::consts::TAU / 16_000.0).sin() * 0.2)
        .collect();
    kora_lib::annotations::opus::write_opus(&opus_path, &samples, 16_000).expect("opus de prueba");
    state
        .storage
        .voice_notes
        .create(NewVoiceNote {
            id: Some(marker_id.clone()),
            book_id: book_id.clone(),
            chapter_index: 0,
            offset_start: locator.offset_start,
            offset_end: locator.offset_end,
            text: locator.quote.clone(),
            duration_ms: 100,
            audio_hash: None,
            audiobook_id: Some(generated.id.clone()),
            position_ms: position,
            quote: Some(locator.quote.clone()),
            transcript: Some("nota de prueba".to_string()),
            source: "voice_mark".to_string(),
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await
        .expect("marcador");
    let markers = audiobook_voice::audiobook_voice_markers(app.state(), generated.id.clone())
        .await
        .expect("marcadores");
    assert!(markers.iter().any(|marker| marker.note.id == marker_id));
    annotations::delete_annotation(app.state(), "voice_note".to_string(), marker_id.clone())
        .await
        .expect("borrar marcador");
    assert!(
        !opus_path.exists(),
        "el Opus del marcador debe ir a la papelera"
    );

    let treated_asset = (treated.audio_hash.clone(), treated.audio_ext.clone());
    audiobooks::delete_audiobook(app.state(), treated.id.clone())
        .await
        .expect("borrar audiolibro");
    assert!(
        !asset_exists(app, treated_asset.0.as_deref(), treated_asset.1.as_deref()),
        "el audio borrado no debe quedar en assets"
    );
    let remaining = audiobooks::list_audiobooks(app.state())
        .await
        .expect("listar tras borrar");
    assert!(remaining.iter().all(|item| item.id != treated.id));
}

async fn run_imported_tools(h: &Harness) {
    let app = &h.app;
    disable_classification(app).await;
    let state = app.state::<AppState>();
    let source = PathBuf::from(
        std::env::var("KORA_E2E_AUDIO_SOURCE").unwrap_or_else(|_| DEFAULT_AUDIO_SOURCE.to_string()),
    );
    if !source.is_file() {
        eprintln!("falta {}; importación omitida", source.display());
        return;
    }
    let excerpt = h.temp.path().join("el-arte-de-la-guerra-3min.mp3");
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-ss",
            "600",
            "-t",
            "180",
            "-i",
        ])
        .arg(&source)
        .args(["-vn", "-c:a", "libmp3lame", "-b:a", "128k"])
        .arg(&excerpt)
        .status()
        .expect("ffmpeg para el fragmento");
    assert!(status.success(), "no se pudo cortar el fragmento de prueba");

    let imported = audiobooks::import_external_audiobook(
        app.state(),
        app.handle().clone(),
        excerpt.to_string_lossy().to_string(),
    )
    .await
    .expect("importar audiolibro");
    assert_eq!(imported.status, AudiobookStatus::Done);
    assert_eq!(imported.origin, AudiobookOrigin::External);
    assert!(imported.duration_ms >= 170_000 && imported.duration_ms <= 190_000);
    assert!(imported.chapter_count >= 1);
    assert!(imported.book_title.to_lowercase().contains("guerra"));

    let chapters = audiobooks::audiobook_chapters(app.state(), imported.id.clone())
        .await
        .expect("capítulos importados");
    assert!(!chapters.is_empty());
    assert!(
        chapters
            .iter()
            .all(|chapter| chapter.start_ms < chapter.end_ms)
    );
    assert!(chapters.iter().any(|chapter| chapter.karaoke));

    let karaoke = audiobooks::audiobook_karaoke(app.state(), imported.id.clone(), 0)
        .await
        .expect("karaoke importado");
    assert!(
        !karaoke.lines.is_empty(),
        "Whisper no generó líneas karaoke"
    );
    assert!(karaoke.lines.iter().all(|line| line.end_ms > line.start_ms));
    let line = karaoke
        .lines
        .iter()
        .find(|line| !line.words.is_empty())
        .unwrap_or(&karaoke.lines[0]);
    let position = karaoke.start_ms + line.start_ms + 100;
    let locator = audiobooks::audiobook_locator(app.state(), imported.id.clone(), position)
        .await
        .expect("localizador importado");
    assert!(!locator.quote.trim().is_empty(), "cita vacía en importado");
    let generated_book = state
        .storage
        .books
        .get(&imported.book_id)
        .await
        .expect("libro")
        .expect("el audiolibro importado debe tener libro");
    assert_eq!(generated_book.format, BookFormat::Epub);
    assert_eq!(generated_book.origin, BookOrigin::Transcribed);
    let utf16 = chapter_utf16(app, &imported.book_id, locator.section).await;
    assert!(locator.offset_start >= 0);
    assert!((locator.offset_end as usize) <= utf16.len());
    let quote = String::from_utf16_lossy(
        &utf16[locator.offset_start as usize..locator.offset_end as usize],
    );
    assert_eq!(
        quote.trim(),
        locator.quote.trim(),
        "el ancla importada no coincide"
    );

    let waveform = audiobooks::audiobook_waveform(app.state(), imported.id.clone(), Some(128))
        .await
        .expect("waveform importada");
    assert_eq!(waveform.buckets.len(), 128);
    assert!(waveform.buckets.iter().any(|peak| *peak > 0.1));

    // El MP3 importado debe abrirse con el mismo decodificador del reproductor.
    let row = state
        .storage
        .audiobooks
        .get(&imported.id)
        .await
        .expect("fila")
        .expect("audiolibro");
    let path = kora_lib::audiobooks::audio_path(&state, &row).expect("ruta de audio");
    let decoder = rodio::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(&path).expect("abrir audio"),
    ))
    .expect("decodificar importado");
    assert!(decoder.take(48_000).count() > 10_000);

    let session = reader::reader_open(
        app.state(),
        app.handle().clone(),
        imported.book_id.clone(),
        None,
    )
    .await
    .expect("abrir texto transcrito");
    assert!(!session.document.sections.is_empty());
    let chapter = reader::reader_get_chapter(app.state(), imported.book_id.clone(), 0)
        .await
        .expect("leer transcripción");
    assert!(extract_text(&chapter.html).utf16_len > 50);
}

async fn run_weave_and_queue(h: &Harness) {
    let app = &h.app;
    disable_classification(app).await;
    let book_id = import_book(app).await;
    let voice_id = installed_voice(app).await;

    let chapter = reader::reader_get_chapter(app.state(), book_id.clone(), 0)
        .await
        .expect("capítulo");
    let source = extract_text(&chapter.html);
    assert!(
        source.utf16_len > 300,
        "capítulo demasiado corto para el test"
    );
    let first = source.slice(0, 160).expect("extracto 1");
    let second = source.slice(200, 360).expect("extracto 2");
    let extract_a = reader::reader_add_extract(
        app.state(),
        app.handle().clone(),
        book_id.clone(),
        0,
        0,
        160,
        first.to_string(),
        Some("Primero".to_string()),
    )
    .await
    .expect("extracto A");
    let extract_b = reader::reader_add_extract(
        app.state(),
        app.handle().clone(),
        book_id.clone(),
        0,
        200,
        360,
        second.to_string(),
        Some("Segundo".to_string()),
    )
    .await
    .expect("extracto B");

    let woven = tejido::weave_extracts(
        app.state(),
        app.handle().clone(),
        vec![extract_a.id.clone(), extract_b.id.clone()],
        "Tejido E2E".to_string(),
        voice_id.clone(),
    )
    .await
    .expect("tejer");
    assert_eq!(woven.book.origin, BookOrigin::Tejido);
    assert_eq!(woven.audiobook.origin, AudiobookOrigin::Tejido);
    assert_eq!(woven.audiobook.status, AudiobookStatus::Done);
    assert!(woven.audiobook.duration_ms.unwrap_or(0) > 0);

    let woven_chapters = audiobooks::audiobook_chapters(app.state(), woven.audiobook.id.clone())
        .await
        .expect("capítulos tejido");
    assert!(woven_chapters.len() >= 2);

    // Control de pausa/reanudación/cancelación con un alcance largo.
    let queued = audiobooks::enqueue_audiobook(
        app.state(),
        app.handle().clone(),
        book_id.clone(),
        AudiobookScope::FullBook,
        voice_id,
        None,
        None,
    )
    .await
    .expect("encolar libro completo");
    assert!(
        queued.active.is_some() || !queued.queue.is_empty(),
        "el trabajo debe quedar activo o en cola"
    );

    wait_for_active(app, Duration::from_secs(30)).await;
    let paused = audiobooks::pause_audiobook_generation(app.state(), app.handle().clone())
        .await
        .expect("pausar");
    assert!(paused.active.as_ref().expect("activo").paused);
    tokio::time::sleep(Duration::from_millis(250)).await;
    let resumed = audiobooks::resume_audiobook_generation(app.state(), app.handle().clone())
        .await
        .expect("reanudar");
    assert!(!resumed.active.as_ref().expect("activo").paused);
    audiobooks::cancel_audiobook_generation(app.state(), app.handle().clone())
        .await
        .expect("cancelar");
    wait_for_queue(app, Duration::from_secs(30)).await;

    let after = audiobooks::list_audiobooks(app.state())
        .await
        .expect("listar");
    assert!(
        after.iter().any(|item| item.book_id == book_id
            && matches!(
                item.status,
                AudiobookStatus::Cancelled | AudiobookStatus::Failed
            )),
        "la cancelación debe dejar constancia del trabajo"
    );
}

#[test]
fn generated_audiobook_tools() {
    if !enabled() {
        eprintln!("KORA_E2E_AUDIOBOOKS no está definido; prueba omitida");
        return;
    }
    let h = harness();
    h.runtime.block_on(run_generated_tools(&h));
}

#[test]
fn imported_audiobook_tools() {
    if !enabled() {
        eprintln!("KORA_E2E_AUDIOBOOKS no está definido; prueba omitida");
        return;
    }
    let h = harness();
    h.runtime.block_on(run_imported_tools(&h));
}

#[test]
fn weave_and_queue_controls() {
    if !enabled() {
        eprintln!("KORA_E2E_AUDIOBOOKS no está definido; prueba omitida");
        return;
    }
    let h = harness();
    h.runtime.block_on(run_weave_and_queue(&h));
}

async fn run_import_cancel(h: &Harness) {
    let app = &h.app;
    let state = app.state::<AppState>();
    let source = PathBuf::from(
        std::env::var("KORA_E2E_AUDIO_SOURCE").unwrap_or_else(|_| DEFAULT_AUDIO_SOURCE.to_string()),
    );
    if !source.is_file() {
        eprintln!("falta {}; cancelación omitida", source.display());
        return;
    }
    let excerpt = h.temp.path().join("cancelacion.mp3");
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-ss",
            "600",
            "-t",
            "180",
            "-i",
        ])
        .arg(&source)
        .args(["-vn", "-c:a", "libmp3lame", "-b:a", "128k"])
        .arg(&excerpt)
        .status()
        .expect("ffmpeg para el fragmento");
    assert!(status.success(), "no se pudo cortar el fragmento de prueba");

    let import_future = audiobooks::import_external_audiobook(
        app.state(),
        app.handle().clone(),
        excerpt.to_string_lossy().to_string(),
    );
    let cancel_future = async {
        wait_for_active(app, Duration::from_secs(20)).await;
        audiobooks::cancel_audiobook_generation(app.state(), app.handle().clone()).await
    };
    let (import_result, cancel_result) = tokio::join!(import_future, cancel_future);
    let view = cancel_result.expect("cancelar importación");
    assert!(
        view.active.is_some(),
        "la importación debía seguir activa al cancelar"
    );
    let error = import_result.expect_err("la importación debe devolver error");
    assert!(
        matches!(error, KoraError::Cancelled(_)),
        "error inesperado: {error:?}"
    );

    wait_for_queue(app, Duration::from_secs(15)).await;
    let remaining = audiobooks::list_audiobooks(app.state())
        .await
        .expect("listar");
    assert!(
        remaining.is_empty(),
        "la importación cancelada no debe crear audiolibros"
    );
    let books = state
        .storage
        .books
        .list_by_owner(&state.identity.account.id)
        .await
        .expect("libros");
    assert!(
        books.is_empty(),
        "la importación cancelada no debe crear libros"
    );
    let audiobooks_dir = h.temp.path().join("data/assets/audiobooks");
    let leftovers = std::fs::read_dir(&audiobooks_dir)
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0);
    assert_eq!(leftovers, 0, "no deben quedar assets de audio temporales");
}

#[test]
fn import_can_be_cancelled() {
    if !enabled() {
        eprintln!("KORA_E2E_AUDIOBOOKS no está definido; prueba omitida");
        return;
    }
    let h = harness();
    h.runtime.block_on(run_import_cancel(&h));
}

async fn run_audio_only_tools(h: &Harness) {
    let app = &h.app;
    disable_classification(app).await;
    let state = app.state::<AppState>();
    let source = PathBuf::from(
        std::env::var("KORA_E2E_AUDIO_SOURCE").unwrap_or_else(|_| DEFAULT_AUDIO_SOURCE.to_string()),
    );
    if !source.is_file() {
        eprintln!("falta {}; importación solo audio omitida", source.display());
        return;
    }
    let excerpt = h.temp.path().join("solo-audio-3min.mp3");
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-ss",
            "600",
            "-t",
            "180",
            "-i",
        ])
        .arg(&source)
        .args(["-vn", "-c:a", "libmp3lame", "-b:a", "128k"])
        .arg(&excerpt)
        .status()
        .expect("ffmpeg para el fragmento");
    assert!(status.success(), "no se pudo cortar el fragmento de prueba");

    let imported = audiobooks::import_audio_audiobook(
        app.state(),
        app.handle().clone(),
        excerpt.to_string_lossy().to_string(),
        AudioImportOptions {
            mode: AudioImportMode::Acoustic,
            language: Some("es".to_string()),
            model_id: None,
        },
        None,
    )
    .await
    .expect("importar solo audio");
    assert_eq!(imported.status, AudiobookStatus::Done);
    assert_eq!(imported.origin, AudiobookOrigin::External);
    assert!(
        imported.audio_only,
        "debe ser un audiolibro sin transcripción"
    );
    assert!(imported.chapter_count >= 1);
    assert!(imported.duration_ms >= 170_000);

    let chapters = audiobooks::audiobook_chapters(app.state(), imported.id.clone())
        .await
        .expect("capítulos");
    assert!(!chapters.is_empty());
    assert!(
        chapters
            .iter()
            .all(|chapter| chapter.end_ms > chapter.start_ms)
    );
    assert!(
        chapters.iter().all(|chapter| !chapter.karaoke),
        "no debe haber karaoke sin transcripción"
    );
    let visible = state
        .storage
        .books
        .list_summaries(&state.identity.account.id)
        .await
        .expect("biblioteca");
    assert!(
        visible.iter().all(|book| book.id != imported.book_id),
        "el placeholder no debe aparecer en la biblioteca"
    );
    let hidden = state
        .storage
        .books
        .get(&imported.book_id)
        .await
        .expect("libro")
        .expect("placeholder");
    assert!(hidden.audio_only);

    let mid = imported.duration_ms / 2;
    let edited = audiobooks::update_audiobook_chapters(
        app.state(),
        imported.id.clone(),
        vec![
            kora_lib::commands::audiobooks::ChapterEditInput {
                start_ms: 0,
                end_ms: mid,
                title: "Primera parte".to_string(),
            },
            kora_lib::commands::audiobooks::ChapterEditInput {
                start_ms: mid,
                end_ms: imported.duration_ms,
                title: "Segunda parte".to_string(),
            },
        ],
    )
    .await
    .expect("editar capítulos");
    assert_eq!(edited.chapter_count, 2);
    assert_eq!(edited.chapter_source, Some(ChapterSource::Manual));
    let edited_chapters = audiobooks::audiobook_chapters(app.state(), imported.id.clone())
        .await
        .expect("capítulos editados");
    assert_eq!(edited_chapters[0].title, "Primera parte");
    assert_eq!(edited_chapters[1].start_ms, mid);

    let reanalyzed = audiobooks::reanalyze_audiobook_chapters(
        app.state(),
        app.handle().clone(),
        imported.id.clone(),
        AudioImportOptions {
            mode: AudioImportMode::Acoustic,
            language: Some("es".to_string()),
            model_id: None,
        },
    )
    .await
    .expect("reanalizar");
    assert_eq!(reanalyzed.id, imported.id);
    assert!(reanalyzed.chapter_count >= 1);

    audiobooks::delete_audiobook(app.state(), imported.id.clone())
        .await
        .expect("borrar audiolibro");
    assert!(
        state
            .storage
            .books
            .get(&imported.book_id)
            .await
            .expect("libro")
            .is_none(),
        "el placeholder debe borrarse con el audiolibro"
    );
    assert!(
        audiobooks::list_audiobooks(app.state())
            .await
            .expect("listar")
            .is_empty(),
        "no debe quedar el audiolibro"
    );
    let audiobooks_dir = h.temp.path().join("data/assets/audiobooks");
    let leftovers = std::fs::read_dir(&audiobooks_dir)
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0);
    assert_eq!(leftovers, 0, "no deben quedar assets de audio");
}

#[test]
fn audio_only_audiobook_tools() {
    if !enabled() {
        eprintln!("KORA_E2E_AUDIOBOOKS no está definido; prueba omitida");
        return;
    }
    let h = harness();
    h.runtime.block_on(run_audio_only_tools(&h));
}
