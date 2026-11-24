use std::path::PathBuf;

use serde::Deserialize;
use tauri::AppHandle;

use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::storage::{
    AssetKind, AssetRef, Audiobook, AudiobookOrigin, AudiobookStatus, BookFormat, BookOrigin,
    NewAudiobook, NewBook,
};
use crate::tejido::cover;
use crate::tejido::epub::{self, EpubChapter};

use super::ai_chapters::{self, AudioChapterDetection, AudioImportOptions};
use super::chapters::{ChapterSource, DetectedChapter};
use super::engine::{AudiobookProgressEvent, emit_progress};
use super::import::{discard_audio_asset, slug};
use super::jobs::GenerationControl;
use super::m4b;
use super::manifest::{AudiobookManifest, MANIFEST_VERSION, ManifestChapter, ManifestVoice};
use super::scope::AudiobookScope;

const PLACEHOLDER_TITLE: &str = "Audiolibro sin transcripción";

/// Importa un MP3/M4B/M4A/etc. sin transcribir: guarda el audio y detecta
/// capítulos con señales acústicas y, opcionalmente, IA local.
pub async fn import(
    state: &AppState,
    app: &AppHandle,
    source: &str,
    options: AudioImportOptions,
    control: Option<&GenerationControl>,
) -> Result<Audiobook> {
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
    if let Some(control) = control {
        control.checkpoint().await?;
    }

    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: None,
            phase: "extracting".to_string(),
            chapter: 0,
            total_chapters: 0,
            percent: 0.5,
            message: "Preparando el audio para analizarlo".to_string(),
        },
    );

    let probe = tokio::task::spawn_blocking({
        let source_path = source_path.clone();
        move || m4b::probe(&source_path)
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
    let language = options
        .language
        .clone()
        .or_else(|| probe.language.clone())
        .or_else(|| Some("es".to_string()));

    let workspace = state
        .layout
        .root
        .join("tmp")
        .join(format!("audio-import-{}", uuid::Uuid::new_v4()));
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

    let detection =
        match ai_chapters::detect(state, app, &source_path, duration_ms, &options, control).await {
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
            percent: 68.0,
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

    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: None,
            phase: "building".to_string(),
            chapter: detection.chapters.len(),
            total_chapters: detection.chapters.len(),
            percent: 85.0,
            message: "Guardando capítulos y portada".to_string(),
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
                    "Kora · audiolibro · {}",
                    author
                        .clone()
                        .unwrap_or_else(|| "audio importado".to_string())
                ),
                &generated,
            )?;
            generated
        };

        let epub_path = workspace.join("audio-import.epub");
        let body = "Este audiolibro se importó sin transcripción. Los capítulos se \
            detectaron automáticamente. Usa «Transcribir este audiolibro» para añadir \
            el texto completo y la lectura sincronizada.";
        epub::build_epub(
            &epub_path,
            &title,
            author.as_deref().unwrap_or("Desconocido"),
            language.as_deref().unwrap_or("es"),
            &[EpubChapter {
                title: PLACEHOLDER_TITLE.to_string(),
                body: body.to_string(),
                source: None,
            }],
            Some(&cover_path),
        )?;

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
                language: language.clone(),
                publisher: None,
                description: Some(probe.comment.clone().unwrap_or_else(|| {
                    "Audiolibro importado sin transcripción. Los capítulos se detectaron \
                     automáticamente."
                        .to_string()
                })),
                origin: BookOrigin::Imported,
                owner_account_id: state.identity.account.id.clone(),
                origin_share_id: None,
                audio_only: true,
            })
            .await?;

        let manifest = build_manifest(&book.id, &title, language.clone(), duration_ms, &detection);
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
                        "no se pudo clasificar el audiolibro importado"
                    );
                }
            });
        }

        Ok::<Audiobook, KoraError>(audiobook)
    }
    .await;

    let _ = std::fs::remove_dir_all(&workspace);
    let audiobook = match result {
        Ok(audiobook) => audiobook,
        Err(error) => {
            let _ = discard_audio_asset(state, &audio_asset).await;
            return Err(error);
        }
    };

    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: Some(audiobook.id.clone()),
            phase: "done".to_string(),
            chapter: detection.chapters.len(),
            total_chapters: detection.chapters.len(),
            percent: 100.0,
            message: format!(
                "«{title}» importado con {} capítulos{}",
                detection.chapters.len(),
                if detection.ai_used {
                    " detectados por IA"
                } else {
                    ""
                }
            ),
        },
    );
    Ok(audiobook)
}

/// Metadatos que Android obtiene con `MediaMetadataRetriever` y envía al
/// comando de importación. En escritorio se ignora y se usa ffprobe.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidAudioMetadata {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub cover_path: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
}

pub(super) fn fixed_detection(duration_ms: i64) -> AudioChapterDetection {
    const BLOCK_MS: i64 = 600_000;
    const MAX_CHAPTERS: usize = 300;

    let duration = duration_ms.max(1);
    let mut chapters = Vec::new();
    let mut start = 0;
    while start < duration && chapters.len() < MAX_CHAPTERS {
        let end = (start + BLOCK_MS).min(duration);
        chapters.push(DetectedChapter {
            start_ms: start,
            end_ms: end,
            title: format!("Capítulo {}", chapters.len() + 1),
        });
        start = end;
    }
    if chapters.is_empty() {
        chapters.push(DetectedChapter {
            start_ms: 0,
            end_ms: duration,
            title: "Capítulo 1".to_string(),
        });
    }

    AudioChapterDetection {
        chapters,
        source: ChapterSource::Fixed,
        ai_used: false,
        note: None,
    }
}

/// Importa audio en Android sin ffmpeg: los metadatos y la portada vienen del
/// puente nativo y los capítulos se reparten en bloques de diez minutos.
pub async fn import_android(
    state: &AppState,
    app: &AppHandle,
    source: &str,
    options: AudioImportOptions,
    metadata: AndroidAudioMetadata,
    control: Option<&GenerationControl>,
    emit_done: bool,
) -> Result<Audiobook> {
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
    if metadata.duration_ms <= 0 {
        return Err(KoraError::InvalidInput(
            "no se pudo leer la duración del audio en Android".into(),
        ));
    }
    if let Some(control) = control {
        control.checkpoint().await?;
    }

    let duration_ms = metadata.duration_ms;
    let title = metadata
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            source_path
                .file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_string)
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Audiolibro importado".to_string());
    let author = metadata
        .artist
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let language = options
        .language
        .clone()
        .or_else(|| metadata.language.clone())
        .or_else(|| Some("es".to_string()));
    let detection = fixed_detection(duration_ms);

    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: None,
            phase: "extracting".to_string(),
            chapter: 0,
            total_chapters: detection.chapters.len(),
            percent: 5.0,
            message: "Preparando el audio importado".to_string(),
        },
    );

    let workspace = state
        .layout
        .root
        .join("tmp")
        .join(format!("audio-import-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace)?;

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

    let result = async {
        if let Some(control) = control {
            control.checkpoint().await?;
        }

        emit_progress(
            app,
            AudiobookProgressEvent {
                audiobook_id: None,
                phase: "building".to_string(),
                chapter: detection.chapters.len(),
                total_chapters: detection.chapters.len(),
                percent: 60.0,
                message: "Guardando portada y capítulos".to_string(),
            },
        );

        let cover_path = match metadata
            .cover_path
            .as_deref()
            .map(PathBuf::from)
            .filter(|path| path.is_file())
        {
            Some(path) => path,
            None => {
                let generated = workspace.join("cover.png");
                cover::render_cover(
                    &title,
                    &format!(
                        "Kora · audiolibro · {}",
                        author
                            .clone()
                            .unwrap_or_else(|| "audio importado".to_string())
                    ),
                    &generated,
                )?;
                generated
            }
        };
        let cover_ext = cover_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("png")
            .to_ascii_lowercase();

        let epub_path = workspace.join("audio-import.epub");
        let body = "Este audiolibro se importó sin transcripción. Los capítulos se \
            dividieron en bloques de diez minutos. Usa «Transcribir este audiolibro» \
            en el escritorio para añadir el texto completo y la lectura sincronizada.";
        epub::build_epub(
            &epub_path,
            &title,
            author.as_deref().unwrap_or("Desconocido"),
            language.as_deref().unwrap_or("es"),
            &[EpubChapter {
                title: PLACEHOLDER_TITLE.to_string(),
                body: body.to_string(),
                source: None,
            }],
            Some(&cover_path),
        )?;

        let epub_asset =
            state
                .storage
                .assets
                .put_file(AssetKind::GeneratedBook, &epub_path, "epub")?;
        let cover_asset =
            state
                .storage
                .assets
                .put_file(AssetKind::Cover, &cover_path, &cover_ext)?;
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
                cover_ext: Some(cover_asset.extension),
                size_bytes,
                language: language.clone(),
                publisher: None,
                description: Some(metadata.album.clone().unwrap_or_else(|| {
                    "Audiolibro importado en Android. Los capítulos se dividieron en \
                     bloques de diez minutos."
                        .to_string()
                })),
                origin: BookOrigin::Imported,
                owner_account_id: state.identity.account.id.clone(),
                origin_share_id: None,
                audio_only: true,
            })
            .await?;

        let manifest = build_manifest(&book.id, &title, language.clone(), duration_ms, &detection);
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
                        "no se pudo clasificar el audiolibro importado"
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
            if emit_done {
                emit_progress(
                    app,
                    AudiobookProgressEvent {
                        audiobook_id: Some(audiobook.id.clone()),
                        phase: "done".to_string(),
                        chapter: detection.chapters.len(),
                        total_chapters: detection.chapters.len(),
                        percent: 100.0,
                        message: format!(
                            "«{title}» importado con {} capítulos",
                            detection.chapters.len()
                        ),
                    },
                );
            }
            Ok(audiobook)
        }
        Err(error) => {
            let _ = discard_audio_asset(state, &audio_asset).await;
            Err(error)
        }
    }
}

fn build_manifest(
    book_id: &str,
    title: &str,
    language: Option<String>,
    duration_ms: i64,
    detection: &AudioChapterDetection,
) -> AudiobookManifest {
    AudiobookManifest {
        version: MANIFEST_VERSION,
        book_id: book_id.to_string(),
        book_title: title.to_string(),
        language,
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
        chapters: detection
            .chapters
            .iter()
            .enumerate()
            .map(|(index, chapter)| ManifestChapter {
                index,
                title: chapter.title.clone(),
                start_ms: chapter.start_ms,
                end_ms: chapter.end_ms,
                section: 0,
                offset_start: 0,
                offset_end: 0,
                karaoke: Vec::new(),
            })
            .collect(),
    }
}

/// Vuelve a analizar un audiolibro de solo audio y actualiza sus capítulos.
pub async fn reanalyze(
    state: &AppState,
    app: &AppHandle,
    audiobook_id: &str,
    options: AudioImportOptions,
    control: Option<&GenerationControl>,
) -> Result<Audiobook> {
    let Some(mut audiobook) = state.storage.audiobooks.get(audiobook_id).await? else {
        return Err(KoraError::NotFound(format!("audiolibro {audiobook_id}")));
    };
    if audiobook.owner_account_id != state.identity.account.id {
        return Err(KoraError::NotFound(format!("audiolibro {audiobook_id}")));
    }
    let Some(book) = state.storage.books.get(&audiobook.book_id).await? else {
        return Err(KoraError::NotFound("libro del audiolibro".into()));
    };
    if !book.audio_only {
        return Err(KoraError::Unsupported(
            "solo se pueden reanalizar audiolibros importados sin transcripción".into(),
        ));
    }
    let mut manifest = super::read_manifest(state, &audiobook)?;
    let source = super::audio_path(state, &audiobook)?;
    let duration_ms = manifest
        .duration_ms
        .max(audiobook.duration_ms.unwrap_or(0))
        .max(1);
    let options = AudioImportOptions {
        language: options
            .language
            .or_else(|| manifest.language.clone())
            .or_else(|| book.language.clone()),
        ..options
    };
    let detection =
        ai_chapters::detect(state, app, &source, duration_ms, &options, control).await?;

    if let Some(control) = control {
        control.checkpoint().await?;
    }
    if detection.chapters.is_empty() {
        return Err(KoraError::Internal(
            "no se detectaron capítulos para el audiolibro".into(),
        ));
    }

    let previous_manifest = (
        audiobook.manifest_hash.clone(),
        audiobook.manifest_ext.clone(),
    );
    manifest.chapters = detection
        .chapters
        .iter()
        .enumerate()
        .map(|(index, chapter)| ManifestChapter {
            index,
            title: chapter.title.clone(),
            start_ms: chapter.start_ms,
            end_ms: chapter.end_ms,
            section: 0,
            offset_start: 0,
            offset_end: 0,
            karaoke: Vec::new(),
        })
        .collect();
    manifest.chapter_source = Some(detection.source);
    manifest.duration_ms = duration_ms;

    let manifest_asset = state.storage.assets.put_bytes(
        AssetKind::Audiobook,
        &serde_json::to_vec_pretty(&manifest)?,
        "json",
    )?;
    audiobook.manifest_hash = Some(manifest_asset.hash);
    audiobook.manifest_ext = Some(manifest_asset.extension);
    state.storage.audiobooks.update(&audiobook).await?;

    if let (Some(hash), Some(extension)) = previous_manifest
        && let Ok(reference) = state.storage.assets.path(&AssetRef {
            kind: AssetKind::Audiobook,
            hash: hash.clone(),
            extension,
        })
        && !state
            .storage
            .asset_is_referenced(AssetKind::Audiobook, &hash)
            .await?
        && reference.is_file()
    {
        let _ = crate::storage::trash::move_to_trash(state.storage.layout(), &reference);
    }

    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: Some(audiobook.id.clone()),
            phase: "done".to_string(),
            chapter: detection.chapters.len(),
            total_chapters: detection.chapters.len(),
            percent: 100.0,
            message: format!(
                "{} capítulos actualizados{}",
                detection.chapters.len(),
                if detection.ai_used { " con IA" } else { "" }
            ),
        },
    );
    Ok(audiobook)
}
