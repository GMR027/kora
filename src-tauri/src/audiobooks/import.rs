use std::path::{Path, PathBuf};

use tauri::AppHandle;

use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::storage::{
    AssetKind, AssetRef, Audiobook, AudiobookOrigin, AudiobookStatus, BookFormat, BookOrigin,
    NewAudiobook, NewBook,
};
use crate::tejido::cover;
use crate::tejido::epub::{self, EpubChapter};
use crate::tts::practice::{self, TimedSegment, find_model, model_path};

use super::audio;
use super::chapters::{self, ChapterDetection, ChapterSource, DetectedChapter};
use super::engine::{AudiobookProgressEvent, emit_progress};
use super::jobs::GenerationControl;
use super::karaoke::{KaraokeLine, KaraokeWord};
use super::m4b;
use super::manifest::{AudiobookManifest, MANIFEST_VERSION, ManifestChapter, ManifestVoice};
use super::scope::AudiobookScope;

const CHUNK_SECONDS: f64 = 600.0;
const DEFAULT_WHISPER_MODEL: &str = "base";

/// Importa un audiolibro externo, lo transcribe con Whisper y crea un libro
/// consultable (EPUB) más su fila de audiolibro.
pub async fn import(
    state: &AppState,
    app: &AppHandle,
    source: &str,
    control: Option<&GenerationControl>,
) -> Result<Audiobook> {
    if let Some(control) = control {
        control.checkpoint().await?;
    }
    let source_path = PathBuf::from(source);
    if !source_path.is_file() {
        return Err(KoraError::NotFound(format!(
            "no se encontró el audio {source}"
        )));
    }

    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(
        extension.as_str(),
        "m4b" | "m4a" | "mp3" | "aac" | "flac" | "ogg" | "opus"
    ) {
        return Err(KoraError::Unsupported(
            "el audiolibro debe ser M4B, M4A, MP3, AAC, FLAC u OGG".into(),
        ));
    }

    m4b::ensure_available()?;

    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: None,
            phase: "extracting".to_string(),
            chapter: 0,
            total_chapters: 0,
            percent: 0.5,
            message: "Preparando la importación del audiolibro".to_string(),
        },
    );

    let model = find_model(DEFAULT_WHISPER_MODEL)
        .ok_or_else(|| KoraError::Internal("no hay un modelo Whisper por defecto".into()))?;
    let whisper_path = model_path(&state.layout.assets, model.id);
    if !practice::installed(&state.layout.assets, model.id) {
        return Err(KoraError::Unsupported(
            "descarga el modelo Whisper Base en Ajustes para transcribir audiolibros".into(),
        ));
    }

    let probe = tokio::task::spawn_blocking({
        let path = source_path.clone();
        move || m4b::probe(&path)
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de ffprobe: {error}")))??;

    let title = probe
        .title
        .clone()
        .or_else(|| {
            source_path
                .file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_string)
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Audiolibro importado".to_string());
    let author = probe.artist.clone();
    let duration_ms = probe.duration_ms.max(1);

    let workspace = state
        .layout
        .root
        .join("tmp")
        .join(format!("import-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace)?;
    if let Some(control) = control
        && let Err(error) = control.checkpoint().await
    {
        let _ = std::fs::remove_dir_all(&workspace);
        return Err(error);
    }

    let embedded_cover = workspace.join("embedded-cover.png");
    let has_embedded_cover = tokio::task::spawn_blocking({
        let source_path = source_path.clone();
        let embedded_cover = embedded_cover.clone();
        move || m4b::extract_cover(&source_path, &embedded_cover)
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de portada: {error}")))?
    .unwrap_or(false);

    if let Some(control) = control
        && let Err(error) = control.checkpoint().await
    {
        let _ = std::fs::remove_dir_all(&workspace);
        return Err(error);
    }

    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: None,
            phase: "extracting".to_string(),
            chapter: 0,
            total_chapters: 0,
            percent: 2.0,
            message: "Analizando el audiolibro".to_string(),
        },
    );

    let detection = match detect_chapters(app, &source_path, &probe, duration_ms, control).await {
        Ok(detection) => detection,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&workspace);
            return Err(error);
        }
    };

    if let Some(control) = control
        && let Err(error) = control.checkpoint().await
    {
        let _ = std::fs::remove_dir_all(&workspace);
        return Err(error);
    }

    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: None,
            phase: "copying".to_string(),
            chapter: 0,
            total_chapters: 0,
            percent: 5.5,
            message: "Copiando el audio a la biblioteca".to_string(),
        },
    );

    let audio_asset =
        match state
            .storage
            .assets
            .put_file(AssetKind::Audiobook, &source_path, &extension)
        {
            Ok(asset) => asset,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&workspace);
                return Err(error);
            }
        };

    if let Some(control) = control
        && let Err(error) = control.checkpoint().await
    {
        let _ = discard_audio_asset(state, &audio_asset).await;
        let _ = std::fs::remove_dir_all(&workspace);
        return Err(error);
    }

    let transcription = transcribe_all(
        app,
        &source_path,
        &whisper_path,
        &workspace,
        &probe,
        duration_ms,
        &detection,
        control,
    )
    .await;

    let (transcript_chapters, mut manifest_chapters) = match transcription {
        Ok(value) => value,
        Err(error) => {
            let _ = discard_audio_asset(state, &audio_asset).await;
            let _ = std::fs::remove_dir_all(&workspace);
            return Err(error);
        }
    };

    if transcript_chapters.is_empty() {
        let _ = discard_audio_asset(state, &audio_asset).await;
        let _ = std::fs::remove_dir_all(&workspace);
        return Err(KoraError::Unsupported(
            "Whisper no reconoció texto en el audiolibro".into(),
        ));
    }

    let chapter_count = transcript_chapters.len();
    let result = async {
        if let Some(control) = control {
            control.checkpoint().await?;
        }
        emit_progress(
            app,
            AudiobookProgressEvent {
                audiobook_id: None,
                phase: "building".to_string(),
                chapter: transcript_chapters.len(),
                total_chapters: transcript_chapters.len(),
                percent: 88.0,
                message: "Creando el libro consultable".to_string(),
            },
        );

        let cover_path = if has_embedded_cover && embedded_cover.is_file() {
            embedded_cover.clone()
        } else {
            let generated = workspace.join("cover.png");
            cover::render_cover(
                &title,
                &format!(
                    "Kora · transcripción · {}",
                    author
                        .clone()
                        .unwrap_or_else(|| "audio importado".to_string())
                ),
                &generated,
            )?;
            generated
        };

        let epub_path = workspace.join("transcripcion.epub");
        epub::build_epub(
            &epub_path,
            &title,
            author.as_deref().unwrap_or("Desconocido"),
            "es",
            &transcript_chapters,
            Some(&cover_path),
        )?;

        // El EPUB inserta un encabezado con el título antes del cuerpo y puede
        // conservar espacios entre bloques. Calculamos los offsets reales
        // sobre el texto que verá el lector para que el karaoke y los
        // marcadores apunten al pasaje correcto.
        let body_offsets = transcript_offsets(&epub_path, &transcript_chapters)?;
        for (chapter, (offset_start, offset_end)) in manifest_chapters.iter_mut().zip(body_offsets)
        {
            chapter.offset_start = offset_start;
            chapter.offset_end = offset_end;
        }

        let epub_asset =
            state
                .storage
                .assets
                .put_file(AssetKind::GeneratedBook, &epub_path, "epub")?;
        let cover_asset = state
            .storage
            .assets
            .put_file(AssetKind::Cover, &cover_path, "png")?;
        let size_bytes = std::fs::metadata(&epub_path)
            .map(|meta| meta.len())
            .unwrap_or(0) as i64;

        let book = state
            .storage
            .books
            .create(NewBook {
                title: title.clone(),
                author: author.clone(),
                format: BookFormat::Epub,
                content_hash: epub_asset.hash,
                content_ext: "epub".to_string(),
                original_name: format!("{}.epub", slug(&title)),
                cover_hash: Some(cover_asset.hash),
                cover_ext: Some("png".to_string()),
                size_bytes,
                language: probe.language.clone().or_else(|| Some("es".to_string())),
                publisher: None,
                description: Some(probe.comment.clone().unwrap_or_else(|| {
                    "Transcripción local de un audiolibro externo.".to_string()
                })),
                origin: BookOrigin::Transcribed,
                owner_account_id: state.identity.account.id.clone(),
                origin_share_id: None,
                audio_only: false,
            })
            .await?;

        let manifest = AudiobookManifest {
            version: MANIFEST_VERSION,
            book_id: book.id.clone(),
            book_title: title.clone(),
            language: probe.language.clone(),
            voice: ManifestVoice {
                id: "external".to_string(),
                name: "Audio importado".to_string(),
                cloned: false,
            },
            watermark: false,
            scope: AudiobookScope::FullBook,
            duration_ms,
            narration: None,
            cast: Vec::new(),
            chapter_source: Some(detection.source),
            treatment: None,
            chapters: manifest_chapters,
        };
        let manifest_asset = state.storage.assets.put_bytes(
            AssetKind::Audiobook,
            &serde_json::to_vec_pretty(&manifest)?,
            "json",
        )?;

        let audiobook = state
            .storage
            .audiobooks
            .create(NewAudiobook {
                book_id: book.id.clone(),
                voice_id: None,
                status: AudiobookStatus::Done,
                manifest_hash: Some(manifest_asset.hash),
                manifest_ext: Some(manifest_asset.extension),
                audio_hash: Some(audio_asset.hash.clone()),
                audio_ext: Some(audio_asset.extension.clone()),
                scope: Some(serde_json::to_string(&AudiobookScope::FullBook)?),
                origin: AudiobookOrigin::External,
                duration_ms: Some(duration_ms),
                narration: None,
                cast: None,
                owner_account_id: state.identity.account.id.clone(),
                origin_share_id: None,
            })
            .await?;

        let classification = state.classification.clone();
        let classification_config = classification.config().await.unwrap_or_default();
        if classification_config.enabled && classification_config.classify_on_import {
            let handle = app.clone();
            let target_book = book.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = classification
                    .classify_book(Some(&handle), &target_book, false, false)
                    .await
                {
                    tracing::warn!(
                        book = %target_book.id,
                        %error,
                        "no se pudo clasificar el audiolibro transcrito"
                    );
                }
            });
        }

        Ok::<Audiobook, KoraError>(audiobook)
    }
    .await;

    let _ = std::fs::remove_dir_all(&workspace);

    match result {
        Ok(audiobook) => {
            emit_progress(
                app,
                AudiobookProgressEvent {
                    audiobook_id: Some(audiobook.id.clone()),
                    phase: "done".to_string(),
                    chapter: chapter_count,
                    total_chapters: chapter_count,
                    percent: 100.0,
                    message: format!("«{title}» importado y transcrito"),
                },
            );
            Ok(audiobook)
        }
        Err(error) => {
            let _ = discard_audio_asset(state, &audio_asset).await;
            Err(error)
        }
    }
}

/// Elimina el audio copiado si la importación falló antes de crear la fila del
/// audiolibro. Si otro audiolibro ya comparte el mismo hash, el asset se
/// conserva.
/// Transcribe un audiolibro que se importó solo como audio. Conserva el
/// audiolibro y su progreso, reemplaza el EPUB placeholder por la
/// transcripción y vuelve visible el libro en la biblioteca.
pub async fn transcribe_existing(
    state: &AppState,
    app: &AppHandle,
    audiobook_id: &str,
    control: Option<&GenerationControl>,
) -> Result<Audiobook> {
    let Some(mut audiobook) = state.storage.audiobooks.get(audiobook_id).await? else {
        return Err(KoraError::NotFound(format!("audiolibro {audiobook_id}")));
    };
    if audiobook.owner_account_id != state.identity.account.id {
        return Err(KoraError::NotFound(format!("audiolibro {audiobook_id}")));
    }
    let Some(mut book) = state.storage.books.get(&audiobook.book_id).await? else {
        return Err(KoraError::NotFound("libro del audiolibro".into()));
    };
    if !book.audio_only {
        return Err(KoraError::Unsupported(
            "este audiolibro ya tiene una transcripción".into(),
        ));
    }
    m4b::ensure_available()?;
    let model = find_model(DEFAULT_WHISPER_MODEL)
        .ok_or_else(|| KoraError::Internal("no hay un modelo Whisper por defecto".into()))?;
    let whisper_path = model_path(&state.layout.assets, model.id);
    if !practice::installed(&state.layout.assets, model.id) {
        return Err(KoraError::Unsupported(
            "descarga el modelo Whisper Base en Ajustes para transcribir audiolibros".into(),
        ));
    }
    if let Some(control) = control {
        control.checkpoint().await?;
    }

    let source_path = super::audio_path(state, &audiobook)?;
    let manifest = super::read_manifest(state, &audiobook)?;
    let duration_ms = manifest
        .duration_ms
        .max(audiobook.duration_ms.unwrap_or(0))
        .max(1);
    let probe = tokio::task::spawn_blocking({
        let source_path = source_path.clone();
        move || m4b::probe(&source_path)
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de ffprobe: {error}")))??;

    let detection = if manifest.chapters.is_empty() {
        detect_chapters(app, &source_path, &probe, duration_ms, control).await?
    } else {
        ChapterDetection {
            chapters: manifest
                .chapters
                .iter()
                .map(|chapter| DetectedChapter {
                    start_ms: chapter.start_ms,
                    end_ms: chapter.end_ms,
                    title: chapter.title.clone(),
                })
                .collect(),
            source: manifest.chapter_source.unwrap_or(ChapterSource::Fixed),
        }
    };
    let chapter_source = detection.source;

    let workspace = state
        .layout
        .root
        .join("tmp")
        .join(format!("transcribe-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace)?;
    if let Some(control) = control
        && let Err(error) = control.checkpoint().await
    {
        let _ = std::fs::remove_dir_all(&workspace);
        return Err(error);
    }

    let embedded_cover = workspace.join("embedded-cover.png");
    let has_embedded_cover = tokio::task::spawn_blocking({
        let source_path = source_path.clone();
        let embedded_cover = embedded_cover.clone();
        move || m4b::extract_cover(&source_path, &embedded_cover)
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de portada: {error}")))?
    .unwrap_or(false);

    let (transcript_chapters, mut manifest_chapters) = match transcribe_all(
        app,
        &source_path,
        &whisper_path,
        &workspace,
        &probe,
        duration_ms,
        &detection,
        control,
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&workspace);
            return Err(error);
        }
    };
    if transcript_chapters.is_empty() {
        let _ = std::fs::remove_dir_all(&workspace);
        return Err(KoraError::Unsupported(
            "Whisper no reconoció texto en el audiolibro".into(),
        ));
    }
    let chapter_count = manifest_chapters.len();
    if let Some(control) = control
        && let Err(error) = control.checkpoint().await
    {
        let _ = std::fs::remove_dir_all(&workspace);
        return Err(error);
    }

    let title = if book.title.trim().is_empty() {
        manifest.book_title.clone()
    } else {
        book.title.clone()
    };
    let author = book.author.clone().or_else(|| probe.artist.clone());
    let language = book
        .language
        .clone()
        .or_else(|| probe.language.clone())
        .or_else(|| Some("es".to_string()));

    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: Some(audiobook.id.clone()),
            phase: "building".to_string(),
            chapter: transcript_chapters.len(),
            total_chapters: transcript_chapters.len(),
            percent: 88.0,
            message: "Creando el libro consultable".to_string(),
        },
    );

    let result = async {
        let cover_path = if has_embedded_cover && embedded_cover.is_file() {
            embedded_cover.clone()
        } else {
            let generated = workspace.join("cover.png");
            cover::render_cover(
                &title,
                &format!(
                    "Kora · transcripción · {}",
                    author
                        .clone()
                        .unwrap_or_else(|| "audio importado".to_string())
                ),
                &generated,
            )?;
            generated
        };

        let epub_path = workspace.join("transcripcion.epub");
        epub::build_epub(
            &epub_path,
            &title,
            author.as_deref().unwrap_or("Desconocido"),
            language.as_deref().unwrap_or("es"),
            &transcript_chapters,
            Some(&cover_path),
        )?;
        let body_offsets = transcript_offsets(&epub_path, &transcript_chapters)?;
        for (chapter, (offset_start, offset_end)) in manifest_chapters.iter_mut().zip(body_offsets)
        {
            chapter.offset_start = offset_start;
            chapter.offset_end = offset_end;
        }

        let epub_asset =
            state
                .storage
                .assets
                .put_file(AssetKind::GeneratedBook, &epub_path, "epub")?;
        let cover_asset = state
            .storage
            .assets
            .put_file(AssetKind::Cover, &cover_path, "png")?;
        let size_bytes = std::fs::metadata(&epub_path)
            .map(|meta| meta.len())
            .unwrap_or(0) as i64;

        let previous_content = (book.content_hash.clone(), book.content_ext.clone());
        let previous_cover = (book.cover_hash.clone(), book.cover_ext.clone());
        book.content_hash = epub_asset.hash;
        book.content_ext = "epub".to_string();
        book.cover_hash = Some(cover_asset.hash);
        book.cover_ext = Some("png".to_string());
        book.size_bytes = size_bytes;
        book.description = Some(probe.comment.clone().unwrap_or_else(|| {
            "Transcripción local de un audiolibro importado sin texto.".to_string()
        }));
        book.origin = BookOrigin::Transcribed;
        book.audio_only = false;
        state.storage.books.update(&book).await?;
        if state
            .storage
            .books
            .count_by_content_hash(&previous_content.0)
            .await?
            == 0
        {
            let reference = AssetRef {
                kind: AssetKind::GeneratedBook,
                hash: previous_content.0,
                extension: previous_content.1,
            };
            if state.storage.assets.exists(&reference)? {
                let path = state.storage.assets.path(&reference)?;
                crate::storage::trash::move_to_trash(state.storage.layout(), &path)?;
            }
        }
        if let (Some(hash), Some(extension)) = previous_cover
            && state.storage.books.count_by_cover_hash(&hash).await? == 0
        {
            let reference = AssetRef {
                kind: AssetKind::Cover,
                hash,
                extension,
            };
            if state.storage.assets.exists(&reference)? {
                let path = state.storage.assets.path(&reference)?;
                crate::storage::trash::move_to_trash(state.storage.layout(), &path)?;
            }
        }

        let mut new_manifest = manifest.clone();
        new_manifest.book_title = title.clone();
        new_manifest.language = language.clone();
        new_manifest.duration_ms = duration_ms;
        new_manifest.chapter_source = Some(chapter_source);
        new_manifest.chapters = manifest_chapters;
        let manifest_asset = state.storage.assets.put_bytes(
            AssetKind::Audiobook,
            &serde_json::to_vec_pretty(&new_manifest)?,
            "json",
        )?;
        let previous_manifest = (
            audiobook.manifest_hash.clone(),
            audiobook.manifest_ext.clone(),
        );
        audiobook.manifest_hash = Some(manifest_asset.hash);
        audiobook.manifest_ext = Some(manifest_asset.extension);
        audiobook.duration_ms = Some(duration_ms);
        audiobook.status = AudiobookStatus::Done;
        state.storage.audiobooks.update(&audiobook).await?;
        if let (Some(hash), Some(extension)) = previous_manifest {
            crate::commands::audiobooks::discard_audiobook_asset(
                state,
                Some(&hash),
                Some(&extension),
            )
            .await?;
        }
        Ok::<(), KoraError>(())
    }
    .await;
    let _ = std::fs::remove_dir_all(&workspace);
    result?;

    let classification = state.classification.clone();
    let classification_config = classification.config().await.unwrap_or_default();
    if classification_config.enabled && classification_config.classify_on_import {
        let handle = app.clone();
        let target_book = book.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = classification
                .classify_book(Some(&handle), &target_book, false, false)
                .await
            {
                tracing::warn!(
                    book = %target_book.id,
                    %error,
                    "no se pudo clasificar el audiolibro transcrito"
                );
            }
        });
    }

    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: Some(audiobook.id.clone()),
            phase: "done".to_string(),
            chapter: chapter_count,
            total_chapters: chapter_count,
            percent: 100.0,
            message: format!("«{title}» transcrito y listo para leer"),
        },
    );
    Ok(audiobook)
}

pub(super) async fn discard_audio_asset(state: &AppState, asset: &AssetRef) -> Result<()> {
    if state
        .storage
        .asset_is_referenced(AssetKind::Audiobook, &asset.hash)
        .await?
    {
        return Ok(());
    }
    let path = state.storage.assets.path(asset)?;
    if path.is_file() {
        crate::storage::trash::move_to_trash(state.storage.layout(), &path)?;
    }
    Ok(())
}

async fn detect_chapters(
    app: &AppHandle,
    source: &Path,
    probe: &m4b::ProbeInfo,
    duration_ms: i64,
    control: Option<&GenerationControl>,
) -> Result<ChapterDetection> {
    let embedded: Vec<(i64, i64, String)> = probe
        .chapters
        .iter()
        .map(|chapter| (chapter.start_ms, chapter.end_ms, chapter.title.clone()))
        .collect();
    if let Some(detection) = chapters::from_embedded(&embedded, duration_ms) {
        emit_progress(
            app,
            AudiobookProgressEvent {
                audiobook_id: None,
                phase: "detecting".to_string(),
                chapter: detection.chapters.len(),
                total_chapters: detection.chapters.len(),
                percent: 6.0,
                message: format!("Usando {} capítulos embebidos", detection.chapters.len()),
            },
        );
        return Ok(detection);
    }

    if let Some(control) = control {
        control.checkpoint().await?;
    }
    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: None,
            phase: "detecting".to_string(),
            chapter: 0,
            total_chapters: 0,
            percent: 3.0,
            message: "Buscando pausas largas entre capítulos".to_string(),
        },
    );
    let path = source.to_path_buf();
    let silences = tokio::task::spawn_blocking(move || m4b::detect_silences(&path, duration_ms))
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de silencios: {error}")))?
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "no se pudieron detectar silencios");
            Vec::new()
        });

    if let Some(control) = control {
        control.checkpoint().await?;
    }
    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: None,
            phase: "detecting".to_string(),
            chapter: 0,
            total_chapters: 0,
            percent: 4.5,
            message: "Buscando tonos de capítulo".to_string(),
        },
    );
    let path = source.to_path_buf();
    let tones = tokio::task::spawn_blocking(move || m4b::tone_boundaries(&path))
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de tonos: {error}")))?
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "no se pudieron detectar tonos");
            Vec::new()
        });

    let detection = chapters::from_signals(&silences, &tones, duration_ms);
    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: None,
            phase: "detecting".to_string(),
            chapter: detection.chapters.len(),
            total_chapters: detection.chapters.len(),
            percent: 6.0,
            message: format!(
                "{} capítulos detectados por {}",
                detection.chapters.len(),
                match detection.source {
                    chapters::ChapterSource::Tone => "tonos y silencios",
                    chapters::ChapterSource::Silence => "silencios",
                    _ => "bloques fijos",
                }
            ),
        },
    );
    Ok(detection)
}

#[allow(clippy::too_many_arguments)]
async fn transcribe_all(
    app: &AppHandle,
    source: &Path,
    whisper_path: &Path,
    workspace: &Path,
    probe: &m4b::ProbeInfo,
    duration_ms: i64,
    detection: &ChapterDetection,
    control: Option<&GenerationControl>,
) -> Result<(Vec<EpubChapter>, Vec<ManifestChapter>)> {
    let duration_seconds = (duration_ms as f64 / 1000.0).max(1.0);
    let total_chapters = detection.chapters.len().max(1);
    let mut epub_chapters = Vec::with_capacity(total_chapters);
    let mut manifest_chapters = Vec::with_capacity(total_chapters);
    let mut processed_seconds = 0.0f64;

    for (chapter_index, chapter) in detection.chapters.iter().enumerate() {
        if let Some(control) = control {
            control.checkpoint().await?;
        }
        let chapter_start = chapter.start_ms.max(0) as f64 / 1000.0;
        let chapter_end = chapter.end_ms.min(duration_ms).max(chapter.start_ms + 1) as f64 / 1000.0;
        let span_seconds = (chapter_end - chapter_start).max(0.1);
        let part_count = (span_seconds / CHUNK_SECONDS).ceil().max(1.0) as usize;
        let span_start_ms = chapter
            .start_ms
            .clamp(0, duration_ms.saturating_sub(1).max(0));
        let span_end_ms = chapter
            .end_ms
            .max(span_start_ms + 1)
            .min(duration_ms.max(span_start_ms + 1));
        let chapter_span_ms = span_end_ms - span_start_ms;
        let mut body = String::new();
        let mut karaoke: Vec<KaraokeLine> = Vec::new();

        for part in 0..part_count {
            if let Some(control) = control {
                control.checkpoint().await?;
            }
            let start_seconds = chapter_start + part as f64 * CHUNK_SECONDS;
            let chunk_seconds = (chapter_end - start_seconds).min(CHUNK_SECONDS);
            if chunk_seconds <= 0.5 {
                break;
            }

            emit_progress(
                app,
                AudiobookProgressEvent {
                    audiobook_id: None,
                    phase: "transcribing".to_string(),
                    chapter: chapter_index + 1,
                    total_chapters,
                    percent: 8.0 + (processed_seconds / duration_seconds) * 78.0,
                    message: format!("Transcribiendo «{}»", chapter.title),
                },
            );

            let wav_path = workspace.join(format!("part-{chapter_index:03}-{part:02}.wav"));
            m4b::extract_wav(source, &wav_path, Some(start_seconds), Some(chunk_seconds))?;
            let samples = audio::read_wav_mono(&wav_path)?;
            let model_path = whisper_path.to_path_buf();
            let segments = tokio::task::spawn_blocking(move || -> Result<Vec<TimedSegment>> {
                match practice::transcribe_timed(&model_path, &samples, "auto") {
                    Ok(segments)
                        if segments
                            .iter()
                            .any(|segment| !segment.text.trim().is_empty()) =>
                    {
                        Ok(segments)
                    }
                    // Si el modelo no soporta timestamps de token, seguimos
                    // con la transcripción normal y la línea queda estimada.
                    _ => {
                        let text = practice::transcribe(&model_path, &samples, "auto")?;
                        let text = text.trim().to_string();
                        if text.is_empty() {
                            Ok(Vec::<TimedSegment>::new())
                        } else {
                            Ok(vec![TimedSegment {
                                start_ms: 0,
                                end_ms: (samples.len() as i64 * 1000 / 16_000).max(1),
                                text,
                                words: Vec::new(),
                            }])
                        }
                    }
                }
            })
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de Whisper: {error}")))??;

            if let Some(control) = control {
                control.checkpoint().await?;
            }

            let part_text = segments
                .iter()
                .map(|segment| normalize_whitespace(&segment.text))
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if !part_text.is_empty() {
                if !body.is_empty() {
                    body.push(' ');
                }
                body.push_str(&part_text);
            }

            let offset_ms = ((start_seconds - chapter_start) * 1000.0).round() as i64;
            for segment in segments {
                let text = normalize_whitespace(&segment.text);
                if text.is_empty() {
                    continue;
                }
                let line_start =
                    (offset_ms + segment.start_ms).clamp(0, chapter_span_ms.saturating_sub(1));
                let line_end = (offset_ms + segment.end_ms)
                    .clamp(line_start + 1, chapter_span_ms.max(line_start + 1));
                let words = segment
                    .words
                    .into_iter()
                    .map(|word| KaraokeWord {
                        start_ms: (offset_ms + word.start_ms).clamp(line_start, line_end),
                        end_ms: (offset_ms + word.end_ms).clamp(line_start, line_end),
                    })
                    .collect();
                if let Some(line) = KaraokeLine::aligned(&text, line_start, line_end, words) {
                    karaoke.push(line);
                }
            }
            processed_seconds += chunk_seconds;
        }

        if body.trim().is_empty() {
            continue;
        }

        let section = manifest_chapters.len();
        manifest_chapters.push(ManifestChapter {
            index: section,
            title: chapter.title.clone(),
            start_ms: span_start_ms,
            end_ms: span_end_ms,
            section,
            offset_start: 0,
            offset_end: body.encode_utf16().count(),
            karaoke,
        });
        epub_chapters.push(EpubChapter {
            title: chapter.title.clone(),
            body,
            source: probe.title.clone(),
        });
    }

    if epub_chapters.is_empty() {
        return Err(KoraError::Unsupported("Whisper no devolvió texto".into()));
    }
    Ok((epub_chapters, manifest_chapters))
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Localiza el cuerpo de cada capítulo dentro del texto que extrae el lector
/// del EPUB generado. Así los offsets UTF-16 incluyen (o excluyen) el título y
/// los espacios de bloque igual que el documento real.
fn transcript_offsets(epub_path: &Path, chapters: &[EpubChapter]) -> Result<Vec<(usize, usize)>> {
    let sections = super::engine::epub_sections(epub_path, &AudiobookScope::FullBook)?;
    let mut offsets = Vec::with_capacity(chapters.len());
    for (index, chapter) in chapters.iter().enumerate() {
        let body = normalize_whitespace(&chapter.body);
        let body_len = body.encode_utf16().count();
        let section = sections.iter().find(|section| section.section == index);
        let start = section.and_then(|section| {
            section
                .text
                .find(&body)
                .map(|byte| section.text[..byte].encode_utf16().count())
        });
        match (start, section) {
            (Some(start), _) => offsets.push((start, start + body_len)),
            (None, Some(section)) => offsets.push((0, section.utf16_len())),
            (None, None) => offsets.push((0, body_len)),
        }
    }
    Ok(offsets)
}

pub(super) fn slug(title: &str) -> String {
    let mut slug: String = title
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "audiolibro".to_string()
    } else {
        slug.chars().take(60).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_safe() {
        assert_eq!(
            slug("Mi audiolibro: capítulo 1"),
            "mi-audiolibro-capítulo-1"
        );
    }
}
