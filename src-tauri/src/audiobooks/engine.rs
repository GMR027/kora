use std::path::PathBuf;

use anyhow::Context;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::commands::reader::{book_asset_path, page_size};
use crate::error::{KoraError, Result};
use crate::reader::pdf::{PdfParser, empty_layer};
use crate::reader::{PageTextLayer, ReaderService, SourceText, extract_text};
use crate::state::AppState;
use crate::storage::{
    AssetKind, AssetRef, Audiobook, AudiobookOrigin, AudiobookStatus, Book, BookFormat,
    NewAudiobook, NewVoice, VoiceKind,
};
use crate::tts::{ExpressiveSettings, SynthesisRequest, build_from_text};

use super::audio;
use super::cast::{CastInput, CastMember, CastResolver, CastVoice};
use super::jobs::GenerationControl;
use super::karaoke::KaraokeLine;
use super::m4b::{self, MuxChapter};
use super::manifest::{AudiobookManifest, MANIFEST_VERSION, ManifestChapter, ManifestVoice};
use super::narration::NarrationSettings;
use super::scope::{AudiobookScope, SourceSection};

const CHAPTER_GAP_MS: u32 = 140;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookProgressEvent {
    pub audiobook_id: Option<String>,
    pub phase: String,
    pub chapter: usize,
    pub total_chapters: usize,
    pub percent: f64,
    pub message: String,
}

pub fn emit_progress(app: &AppHandle, event: AudiobookProgressEvent) {
    #[cfg(target_os = "android")]
    {
        let _ = super::player::update_generation_service(event.percent, &event.message);
        if matches!(
            event.phase.as_str(),
            "transcribing" | "building" | "done" | "cancelled" | "error"
        ) {
            let _ = super::player::update_transcription_service(event.percent, &event.message);
        }
    }
    let _ = app.emit("audiobook::progress", event);
}

/// Mantiene vivo el proceso Android mientras se sintetiza y empaqueta un
/// audiolibro. El servicio se detiene al salir de `synthesize_sections`.
#[cfg(target_os = "android")]
struct AndroidGenerationService;

#[cfg(target_os = "android")]
impl Drop for AndroidGenerationService {
    fn drop(&mut self) {
        let _ = super::player::stop_generation_service();
    }
}

/// Resultado de sintetizar un conjunto de secciones sin tocar la base de
/// datos. Lo usan tanto los audiolibros de la biblioteca como la exportación
/// del diario.
pub struct SynthesizedAudio {
    pub path: PathBuf,
    pub duration_ms: i64,
    pub chapters: Vec<ManifestChapter>,
    pub cast: Vec<CastMember>,
}

/// Sintetiza secciones y las empaqueta en un M4B dentro de `workspace`.
/// No crea filas ni assets: el llamador decide dónde guardar el resultado.
#[allow(clippy::too_many_arguments)]
pub async fn synthesize_sections(
    state: &AppState,
    app: Option<&AppHandle>,
    context_id: &str,
    book_id: &str,
    title: &str,
    author: Option<&str>,
    sections: &[SourceSection],
    voice_id: &str,
    cloned: bool,
    narration: NarrationSettings,
    cast: CastInput,
    control: Option<&GenerationControl>,
    workspace: &std::path::Path,
) -> Result<SynthesizedAudio> {
    std::fs::create_dir_all(workspace)?;
    #[cfg(target_os = "android")]
    let _service = app.map(|_| {
        let _ = super::player::start_generation_service(title);
        AndroidGenerationService
    });

    let narration = narration.clamped();
    let mut cast_resolver = CastResolver::single(CastVoice {
        id: voice_id.to_string(),
        name: voice_id.to_string(),
        cloned,
    });
    if cast.enabled {
        let voices = state.tts.voices().await?;
        let installed: Vec<CastVoice> = voices
            .iter()
            .filter(|voice| voice.installed)
            .map(|voice| CastVoice {
                id: voice.id.clone(),
                name: voice.name.clone(),
                cloned: voice.cloned,
            })
            .collect();
        let base = installed
            .iter()
            .find(|voice| voice.id == voice_id)
            .cloned()
            .unwrap_or_else(|| CastVoice {
                id: voice_id.to_string(),
                name: voice_id.to_string(),
                cloned,
            });
        cast_resolver = CastResolver::new(cast, base, installed);
    }
    let profile = narration.profile();
    let expressive = ExpressiveSettings {
        enabled: true,
        pause_scale: profile.pause_scale,
        emphasis: profile.emphasis,
    };
    let chunk_gap_ms = (CHAPTER_GAP_MS as f32 * profile.pause_scale).round() as u32;

    let total = sections.len();
    let mut chapters: Vec<MuxChapter> = Vec::with_capacity(total);
    let mut manifest_chapters: Vec<ManifestChapter> = Vec::with_capacity(total);
    let mut cursor_ms = 0i64;

    for (index, section) in sections.iter().enumerate() {
        if let Some(control) = control {
            control.checkpoint().await?;
        }
        if let Some(app) = app {
            emit_progress(
                app,
                AudiobookProgressEvent {
                    audiobook_id: Some(context_id.to_string()),
                    phase: "synthesizing".to_string(),
                    chapter: index + 1,
                    total_chapters: total,
                    percent: (index as f64 / total as f64) * 90.0,
                    message: format!("Sintetizando «{}»", section.title),
                },
            );
        }

        let source = SourceText {
            text: section.text.clone(),
            utf16_len: section.utf16_len(),
            breaks: Vec::new(),
        };
        let sequence = build_from_text(section.section, &source, &expressive);
        if sequence.is_empty() {
            continue;
        }

        let mut samples: Vec<i16> = Vec::new();
        let mut karaoke: Vec<KaraokeLine> = Vec::new();
        for (chunk_index, chunk) in sequence.chunks.iter().enumerate() {
            if let Some(control) = control {
                control.checkpoint().await?;
            }
            if chunk.speech.trim().is_empty() {
                continue;
            }
            if chunk_index > 0 {
                audio::push_silence(&mut samples, chunk_gap_ms);
            }
            let line_start_ms = audio::duration_ms(samples.len());
            let (rate, pitch) = profile.apply(chunk.rate, chunk.pitch);
            let chunk_character = chunk
                .character
                .as_deref()
                .filter(|name| *name != crate::tts::NARRATOR);
            let chunk_voice = cast_resolver.voice_for(chunk_character);
            let synthesized = state
                .tts
                .synthesize(&SynthesisRequest {
                    text: chunk.speech.clone(),
                    voice_id: chunk_voice,
                    rate,
                    pitch,
                })
                .await
                .with_context(|| format!("no se pudo sintetizar «{}»", section.title))?;
            samples.extend(audio::chunk_to_samples(&synthesized));
            let line_end_ms = audio::duration_ms(samples.len());
            if let Some(line) = KaraokeLine::timed(&chunk.text, line_start_ms, line_end_ms) {
                karaoke.push(line);
            }
        }

        if samples.is_empty() {
            continue;
        }

        let wav_path = workspace.join(format!("chapter-{index:04}.wav"));
        audio::write_wav(&wav_path, &samples)?;
        let duration_ms = audio::duration_ms(samples.len());
        let start_ms = cursor_ms;
        let end_ms = cursor_ms + duration_ms;
        cursor_ms = end_ms;

        chapters.push(MuxChapter {
            title: section.title.clone(),
            path: wav_path,
            duration_ms,
        });
        manifest_chapters.push(ManifestChapter {
            index: manifest_chapters.len(),
            title: section.title.clone(),
            start_ms,
            end_ms,
            section: section.section,
            offset_start: section.offset_start,
            offset_end: section.offset_end,
            karaoke,
        });
    }

    if chapters.is_empty() {
        return Err(KoraError::Internal(
            "no se generó audio para ninguna sección".into(),
        ));
    }

    if let Some(control) = control {
        control.checkpoint().await?;
    }

    if let Some(app) = app {
        emit_progress(
            app,
            AudiobookProgressEvent {
                audiobook_id: Some(context_id.to_string()),
                phase: "encoding".to_string(),
                chapter: total,
                total_chapters: total,
                percent: 92.0,
                message: "Codificando M4B con capítulos".to_string(),
            },
        );
    }

    let cast_members = cast_resolver.into_members();
    let watermark = cloned || cast_members.iter().any(|member| member.cloned);

    let m4b_path = workspace.join("audiobook.m4b");
    let mut metadata: Vec<(String, String)> = vec![
        ("book_id".to_string(), book_id.to_string()),
        ("voice_id".to_string(), voice_id.to_string()),
    ];
    if watermark {
        metadata.push(("KORA_CLONED_VOICE".to_string(), "1".to_string()));
        metadata.push(("KORA_WATERMARK".to_string(), "cloned-voice-v1".to_string()));
        metadata.push(("KORA_VOICE_PROFILE".to_string(), voice_id.to_string()));
    }
    let metadata_refs: Vec<(&str, &str)> = metadata
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();

    m4b::mux(
        &chapters,
        &m4b_path,
        title,
        author.unwrap_or("Kora"),
        &metadata_refs,
    )?;

    Ok(SynthesizedAudio {
        path: m4b_path,
        duration_ms: cursor_ms,
        chapters: manifest_chapters,
        cast: cast_members,
    })
}

/// Genera un audiolibro completo. La fila se crea antes de sintetizar para que
/// la UI pueda mostrar el progreso y para dejar rastro si el proceso falla.
#[allow(clippy::too_many_arguments)]
pub async fn generate(
    state: &AppState,
    app: &AppHandle,
    book_id: &str,
    scope: AudiobookScope,
    voice_id: &str,
    origin: AudiobookOrigin,
    narration: NarrationSettings,
    cast: CastInput,
    control: Option<&GenerationControl>,
) -> Result<Audiobook> {
    if voice_id.trim().is_empty() {
        return Err(KoraError::InvalidInput(
            "elige una voz para generar el audiolibro".into(),
        ));
    }
    let narration = NarrationSettings {
        multi_voice: cast.enabled,
        ..narration.clamped()
    };

    let book = owned_book(state, book_id).await?;
    let voices = state.tts.voices().await?;
    let voice = voices
        .into_iter()
        .find(|voice| voice.id == voice_id)
        .ok_or_else(|| KoraError::NotFound(format!("la voz {voice_id} no está instalada")))?;

    // La tabla `audiobooks.voice_id` referencia `voices`. Las voces del catálogo
    // Piper no viven ahí hasta que se usan; se registran como voces de sistema
    // para que la clave foránea sea válida.
    if state.storage.voices.get(&voice.id).await?.is_none() {
        state
            .storage
            .voices
            .create(NewVoice {
                id: Some(voice.id.clone()),
                name: voice.name.clone(),
                engine: voice.engine.clone(),
                language: voice.language.clone(),
                profile_hash: None,
                profile_ext: None,
                sample_hash: None,
                sample_ext: None,
                kind: VoiceKind::System,
                reference_duration_ms: 0,
                owner_account_id: state.identity.account.id.clone(),
                origin_share_id: None,
            })
            .await?;
    }

    let sections = resolve_sections(state, &book, &scope).await?;
    if sections.is_empty() {
        return Err(KoraError::Unsupported(
            "el alcance elegido no tiene texto para leer".into(),
        ));
    }

    let scope_json = serde_json::to_string(&scope)?;
    let narration_json = serde_json::to_string(&narration)?;
    let cast_json = serde_json::to_string(&cast)?;
    let mut audiobook = state
        .storage
        .audiobooks
        .create(NewAudiobook {
            book_id: book.id.clone(),
            voice_id: Some(voice.id.clone()),
            status: AudiobookStatus::Running,
            manifest_hash: None,
            manifest_ext: None,
            audio_hash: None,
            audio_ext: None,
            scope: Some(scope_json.clone()),
            origin,
            duration_ms: None,
            narration: Some(narration_json),
            cast: Some(cast_json),
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await?;

    if let Some(control) = control {
        control.bind_audiobook(&audiobook.id);
    }

    let result = generate_inner(
        state,
        app,
        &book,
        &scope,
        &voice.id,
        &voice.name,
        voice.cloned,
        narration,
        cast,
        control,
        &sections,
        &mut audiobook,
    )
    .await;

    finish_generation(state, app, &book, sections.len(), audiobook, result).await
}

/// Reintenta un audiolibro fallido o cancelado reutilizando su fila, su voz y
/// sus ajustes de narración. Conserva marcadores y progreso asociados.
pub async fn regenerate(
    state: &AppState,
    app: &AppHandle,
    audiobook_id: &str,
    narration: NarrationSettings,
    cast: CastInput,
    control: Option<&GenerationControl>,
) -> Result<Audiobook> {
    let mut audiobook = state
        .storage
        .audiobooks
        .get(audiobook_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("audiolibro {audiobook_id}")))?;
    if !matches!(
        audiobook.status,
        AudiobookStatus::Failed | AudiobookStatus::Cancelled
    ) {
        return Err(KoraError::Conflict(
            "solo se pueden reintentar audiolibros fallidos o cancelados".into(),
        ));
    }

    let book = owned_book(state, &audiobook.book_id).await?;
    let scope: AudiobookScope = audiobook
        .scope
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or(AudiobookScope::FullBook);
    let voice_id = audiobook
        .voice_id
        .clone()
        .ok_or_else(|| KoraError::Unsupported("el audiolibro no tiene una voz asociada".into()))?;
    let narration = NarrationSettings {
        multi_voice: cast.enabled,
        ..narration.clamped()
    };
    let voices = state.tts.voices().await?;
    let voice = voices
        .into_iter()
        .find(|voice| voice.id == voice_id)
        .ok_or_else(|| KoraError::NotFound(format!("la voz {voice_id} no está instalada")))?;

    if state.storage.voices.get(&voice.id).await?.is_none() {
        state
            .storage
            .voices
            .create(NewVoice {
                id: Some(voice.id.clone()),
                name: voice.name.clone(),
                engine: voice.engine.clone(),
                language: voice.language.clone(),
                profile_hash: None,
                profile_ext: None,
                sample_hash: None,
                sample_ext: None,
                kind: VoiceKind::System,
                reference_duration_ms: 0,
                owner_account_id: state.identity.account.id.clone(),
                origin_share_id: None,
            })
            .await?;
    }

    let sections = resolve_sections(state, &book, &scope).await?;
    if sections.is_empty() {
        return Err(KoraError::Unsupported(
            "el alcance elegido no tiene texto para leer".into(),
        ));
    }

    audiobook.status = AudiobookStatus::Running;
    audiobook.manifest_hash = None;
    audiobook.manifest_ext = None;
    audiobook.audio_hash = None;
    audiobook.audio_ext = None;
    audiobook.duration_ms = None;
    audiobook.scope = Some(serde_json::to_string(&scope)?);
    audiobook.narration = Some(serde_json::to_string(&narration)?);
    audiobook.cast = Some(serde_json::to_string(&cast)?);
    state.storage.audiobooks.update(&audiobook).await?;

    if let Some(control) = control {
        control.bind_audiobook(&audiobook.id);
    }

    let result = generate_inner(
        state,
        app,
        &book,
        &scope,
        &voice.id,
        &voice.name,
        voice.cloned,
        narration,
        cast,
        control,
        &sections,
        &mut audiobook,
    )
    .await;

    finish_generation(state, app, &book, sections.len(), audiobook, result).await
}

async fn finish_generation(
    state: &AppState,
    app: &AppHandle,
    book: &Book,
    section_count: usize,
    mut audiobook: Audiobook,
    result: Result<()>,
) -> Result<Audiobook> {
    match result {
        Ok(()) => {
            emit_progress(
                app,
                AudiobookProgressEvent {
                    audiobook_id: Some(audiobook.id.clone()),
                    phase: "done".to_string(),
                    chapter: section_count,
                    total_chapters: section_count,
                    percent: 100.0,
                    message: format!("Audiolibro «{}» listo", book.title),
                },
            );
            state
                .storage
                .audiobooks
                .get(&audiobook.id)
                .await?
                .ok_or_else(|| KoraError::Internal("audiolibro generado no encontrado".into()))
        }
        Err(error @ KoraError::Cancelled(_)) => {
            audiobook.status = AudiobookStatus::Cancelled;
            let _ = state.storage.audiobooks.update(&audiobook).await;
            emit_progress(
                app,
                AudiobookProgressEvent {
                    audiobook_id: Some(audiobook.id.clone()),
                    phase: "cancelled".to_string(),
                    chapter: 0,
                    total_chapters: section_count,
                    percent: 0.0,
                    message: format!("Generación de «{}» cancelada", book.title),
                },
            );
            Err(error)
        }
        Err(error) => {
            audiobook.status = AudiobookStatus::Failed;
            let _ = state.storage.audiobooks.update(&audiobook).await;
            emit_progress(
                app,
                AudiobookProgressEvent {
                    audiobook_id: Some(audiobook.id.clone()),
                    phase: "error".to_string(),
                    chapter: 0,
                    total_chapters: section_count,
                    percent: 0.0,
                    message: format!("{error:#}"),
                },
            );
            Err(error)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn generate_inner(
    state: &AppState,
    app: &AppHandle,
    book: &Book,
    scope: &AudiobookScope,
    voice_id: &str,
    voice_name: &str,
    cloned: bool,
    narration: NarrationSettings,
    cast: CastInput,
    control: Option<&GenerationControl>,
    sections: &[SourceSection],
    audiobook: &mut Audiobook,
) -> Result<()> {
    let workspace = state
        .layout
        .root
        .join("tmp")
        .join(format!("audiobook-{}", audiobook.id));

    let synthesis = synthesize_sections(
        state,
        Some(app),
        &audiobook.id,
        &book.id,
        &book.title,
        book.author.as_deref(),
        sections,
        voice_id,
        cloned,
        narration,
        cast,
        control,
        &workspace,
    )
    .await;

    let synthesized = match synthesis {
        Ok(synthesized) => synthesized,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&workspace);
            if matches!(error, KoraError::Cancelled(_)) {
                return Err(error);
            }
            return Err(KoraError::Internal(format!("{error:#}")));
        }
    };

    let audio_asset =
        state
            .storage
            .assets
            .put_file(AssetKind::Audiobook, &synthesized.path, "m4b")?;
    let watermark = cloned || synthesized.cast.iter().any(|member| member.cloned);

    let manifest = AudiobookManifest {
        version: MANIFEST_VERSION,
        book_id: book.id.clone(),
        book_title: book.title.clone(),
        language: book.language.clone(),
        voice: ManifestVoice {
            id: voice_id.to_string(),
            name: voice_name.to_string(),
            cloned,
        },
        watermark,
        scope: scope.clone(),
        duration_ms: synthesized.duration_ms,
        narration: Some(narration.clamped()),
        cast: synthesized.cast,
        chapter_source: Some(super::chapters::ChapterSource::Sections),
        treatment: None,
        chapters: synthesized.chapters,
    };
    let manifest_asset = state.storage.assets.put_bytes(
        AssetKind::Audiobook,
        &serde_json::to_vec_pretty(&manifest)?,
        "json",
    )?;

    audiobook.audio_hash = Some(audio_asset.hash);
    audiobook.audio_ext = Some(audio_asset.extension);
    audiobook.manifest_hash = Some(manifest_asset.hash);
    audiobook.manifest_ext = Some(manifest_asset.extension);
    audiobook.duration_ms = Some(synthesized.duration_ms);
    audiobook.status = AudiobookStatus::Done;
    state.storage.audiobooks.update(audiobook).await?;

    let _ = std::fs::remove_dir_all(&workspace);
    Ok(())
}

async fn owned_book(state: &AppState, book_id: &str) -> Result<Book> {
    let book = state
        .storage
        .books
        .get(book_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("libro {book_id}")))?;
    if book.owner_account_id != state.identity.account.id {
        return Err(KoraError::NotFound(format!("libro {book_id}")));
    }
    Ok(book)
}

pub async fn resolve_sections(
    state: &AppState,
    book: &Book,
    scope: &AudiobookScope,
) -> Result<Vec<SourceSection>> {
    match book.format {
        BookFormat::Epub => resolve_epub(state, book, scope).await,
        BookFormat::Pdf => resolve_pdf(state, book, scope).await,
    }
}

async fn resolve_epub(
    state: &AppState,
    book: &Book,
    scope: &AudiobookScope,
) -> Result<Vec<SourceSection>> {
    let path = book_asset_path(state, book)?;
    let scope = scope.clone();
    tokio::task::spawn_blocking(move || epub_sections(&path, &scope))
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de lectura EPUB: {error}")))?
}

/// Resuelve capítulos de un EPUB sin depender de `AppState`, para reutilizarlo
/// en tests y en cualquier flujo que ya tenga la ruta del archivo.
pub fn epub_sections(path: &std::path::Path, scope: &AudiobookScope) -> Result<Vec<SourceSection>> {
    let service = ReaderService::new();
    let document = service.inspect_epub(path)?;
    let indices: Vec<usize> = match scope {
        AudiobookScope::FullBook => (0..document.sections.len()).collect(),
        AudiobookScope::Chapter { section } | AudiobookScope::Selection { section, .. } => {
            vec![*section]
        }
    };

    let mut sections = Vec::new();
    for index in indices {
        let chapter = service.epub_chapter(path, index)?;
        let source = extract_text(&chapter.html);
        if source.is_empty() {
            continue;
        }

        match scope {
            AudiobookScope::Selection { start, end, .. } => {
                let text = source.slice(*start, *end).ok_or_else(|| {
                    KoraError::InvalidInput("la selección no cae en límites UTF-16 válidos".into())
                })?;
                if !text.trim().is_empty() {
                    sections.push(SourceSection {
                        section: index,
                        title: format!("{} (fragmento)", chapter.title),
                        text: text.to_string(),
                        offset_start: *start,
                        offset_end: *end,
                    });
                }
            }
            _ => sections.push(SourceSection {
                section: index,
                title: chapter.title,
                text: source.text.clone(),
                offset_start: 0,
                offset_end: source.utf16_len,
            }),
        }
    }

    Ok(sections)
}

async fn resolve_pdf(
    state: &AppState,
    book: &Book,
    scope: &AudiobookScope,
) -> Result<Vec<SourceSection>> {
    let page_count = pdf_page_count(state, book).await?;
    let indices: Vec<usize> = match scope {
        AudiobookScope::FullBook => (0..page_count).collect(),
        AudiobookScope::Chapter { section } | AudiobookScope::Selection { section, .. } => {
            if *section >= page_count {
                return Err(KoraError::InvalidInput(format!(
                    "la página {} está fuera del documento",
                    section + 1
                )));
            }
            vec![*section]
        }
    };

    let mut sections = Vec::new();
    for page in indices {
        let layer = load_page_layer(state, book, page).await?;
        let text = layer.text();
        if text.trim().is_empty() {
            continue;
        }

        let title = format!("Página {}", page + 1);
        match scope {
            AudiobookScope::Selection { start, end, .. } => {
                let utf16 = text.encode_utf16().collect::<Vec<_>>();
                if *end > utf16.len() || start >= end {
                    return Err(KoraError::InvalidInput(
                        "la selección de la página no es válida".into(),
                    ));
                }
                let slice = String::from_utf16_lossy(&utf16[*start..*end]);
                if !slice.trim().is_empty() {
                    sections.push(SourceSection {
                        section: page,
                        title: format!("{title} (fragmento)"),
                        text: slice,
                        offset_start: *start,
                        offset_end: *end,
                    });
                }
            }
            _ => sections.push(SourceSection {
                section: page,
                title,
                text: text.clone(),
                offset_start: 0,
                offset_end: text.encode_utf16().count(),
            }),
        }
    }

    Ok(sections)
}

async fn pdf_page_count(state: &AppState, book: &Book) -> Result<usize> {
    if let Some(info) = state.storage.pdf.get_info(&book.id).await? {
        return Ok(info.page_count.max(0) as usize);
    }

    let path = book_asset_path(state, book)?;
    let inspection = tokio::task::spawn_blocking(move || PdfParser::new().inspect(&path))
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de inspección PDF: {error}")))??;
    Ok(inspection.page_count)
}

async fn load_page_layer(state: &AppState, book: &Book, page: usize) -> Result<PageTextLayer> {
    if let Some(mut layer) = crate::reader::ocr::read_ocr_page(&state.storage, &book.id, page)? {
        layer.book_id = book.id.clone();
        return Ok(layer);
    }

    let path = book_asset_path(state, book)?;
    let task_path = path.clone();
    let parser = PdfParser::new();
    match tokio::task::spawn_blocking(move || parser.page_text(&task_path, page)).await {
        Ok(Ok(mut layer)) => {
            layer.book_id = book.id.clone();
            Ok(layer)
        }
        _ => {
            let (width, height) = page_size(&path, page).unwrap_or((595.0, 842.0));
            Ok(empty_layer(&book.id, page, width, height))
        }
    }
}

/// Abre el manifiesto de un audiolibro y comprueba que el asset de audio existe.
pub fn read_manifest(state: &AppState, audiobook: &Audiobook) -> Result<AudiobookManifest> {
    let hash = audiobook
        .manifest_hash
        .as_deref()
        .ok_or_else(|| KoraError::NotFound("el audiolibro no tiene manifiesto".into()))?;
    let extension = audiobook.manifest_ext.as_deref().unwrap_or("json");
    let reference = AssetRef {
        kind: AssetKind::Audiobook,
        hash: hash.to_string(),
        extension: extension.to_string(),
    };
    let bytes = state.storage.assets.read(&reference)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn audio_path(state: &AppState, audiobook: &Audiobook) -> Result<PathBuf> {
    let hash = audiobook
        .audio_hash
        .as_deref()
        .ok_or_else(|| KoraError::NotFound("el audiolibro no tiene audio".into()))?;
    let extension = audiobook.audio_ext.as_deref().unwrap_or("m4b");
    let reference = AssetRef {
        kind: AssetKind::Audiobook,
        hash: hash.to_string(),
        extension: extension.to_string(),
    };
    let path = state.storage.assets.path(&reference)?;
    if !path.is_file() {
        return Err(KoraError::NotFound(
            "el audio del audiolibro no existe".into(),
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_epub_chapters_and_utf16_selections() {
        let workspace = tempfile::tempdir().expect("tmp");
        let path = workspace.path().join("libro.epub");
        let chapters = vec![
            crate::tejido::epub::EpubChapter {
                title: "Uno".into(),
                body: "Hola mundo. Otra frase.".into(),
                source: None,
            },
            crate::tejido::epub::EpubChapter {
                title: "Dos".into(),
                body: "Segundo capítulo.".into(),
                source: None,
            },
        ];
        crate::tejido::epub::build_epub(&path, "Libro", "Kora", "es", &chapters, None)
            .expect("epub");

        let full = epub_sections(&path, &AudiobookScope::FullBook).expect("completo");
        assert_eq!(full.len(), 2);
        assert_eq!(full[0].title, "Uno");
        assert!(full[1].text.contains("Segundo"));

        let chapter =
            epub_sections(&path, &AudiobookScope::Chapter { section: 1 }).expect("capítulo");
        assert_eq!(chapter.len(), 1);
        assert_eq!(chapter[0].section, 1);

        let start = full[0].text[..full[0].text.find("Hola").expect("texto")]
            .encode_utf16()
            .count();
        let selection = epub_sections(
            &path,
            &AudiobookScope::Selection {
                section: 0,
                start,
                end: start + 10,
            },
        )
        .expect("selección");
        assert_eq!(selection.len(), 1);
        assert_eq!(selection[0].text, "Hola mundo");
        assert_eq!(selection[0].offset_start, start);
        assert_eq!(selection[0].offset_end, start + 10);
    }

    #[test]
    fn manifest_prefers_the_chapter_containing_the_position() {
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
            duration_ms: 3000,
            narration: None,
            cast: Vec::new(),
            chapter_source: None,
            treatment: None,
            chapters: vec![
                ManifestChapter {
                    index: 0,
                    title: "Uno".into(),
                    start_ms: 0,
                    end_ms: 1000,
                    section: 0,
                    offset_start: 0,
                    offset_end: 10,
                    karaoke: Vec::new(),
                },
                ManifestChapter {
                    index: 1,
                    title: "Dos".into(),
                    start_ms: 1000,
                    end_ms: 3000,
                    section: 1,
                    offset_start: 0,
                    offset_end: 20,
                    karaoke: Vec::new(),
                },
            ],
        };
        assert_eq!(manifest.chapter_at(0), 0);
        assert_eq!(manifest.chapter_at(1500), 1);
        assert_eq!(manifest.chapter_at(99_000), 1);
    }
}
