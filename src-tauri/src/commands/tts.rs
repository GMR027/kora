use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tauri::{AppHandle, State};

use crate::commands::reader::{book_asset_path, ensure_pdf, owned_book, page_size};
use crate::error::{KoraError, Result};
use crate::reader::ocr::read_ocr_page;
use crate::reader::pdf::{PdfParser, empty_layer};
use crate::reader::{PageTextLayer, ReaderService, SourceText, extract_text};
use crate::state::AppState;
use crate::storage::{
    Book, BookFormat, CharacterVoice, NewCharacterVoice, NewTtsPreferences, NewTtsSequence,
    TtsPreferences,
};
use crate::tts::{
    CharacterInfo, EngineInfo, ExpressiveSettings, NARRATOR, SessionPlan, SpeechSequence,
    SupertonicStatus, SynthesisRequest, TtsMediaMetadata, TtsStatus, VoiceDescriptor,
    build_from_text, build_from_words, content_hash,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthesisOutput {
    pub audio_base64: String,
    pub sample_rate: u32,
    pub duration_ms: u64,
}

#[cfg(not(target_os = "android"))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceDownloadProgress {
    pub voice_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsOffset {
    pub section: usize,
    pub offset: usize,
    /// Fin exclusivo del rango que se quiere leer. Si falta, se lee hasta el
    /// final de la sección.
    #[serde(default)]
    pub end: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsPreferencesInput {
    pub voice_id: Option<String>,
    pub rate: f32,
    pub pitch: f32,
    pub pause_scale: f32,
    pub expressive: bool,
    pub multi_voice: bool,
}

struct PreparedSequence {
    sequence: SpeechSequence,
    preferences: TtsPreferences,
    language: Option<String>,
    media: TtsMediaMetadata,
}

#[tauri::command]
pub fn tts_engines(state: State<'_, AppState>) -> Result<Vec<EngineInfo>> {
    Ok(state.tts.engines())
}

#[tauri::command]
pub async fn tts_voices(
    state: State<'_, AppState>,
    show_all: Option<bool>,
) -> Result<Vec<VoiceDescriptor>> {
    let voices = state.tts.voices().await?;
    if show_all.unwrap_or(false) {
        Ok(sort_voices(voices))
    } else {
        Ok(curate_voices(voices))
    }
}

/// Ordena las voces mejoradas primero y deja el resto agrupado por motor.
fn sort_voices(mut voices: Vec<VoiceDescriptor>) -> Vec<VoiceDescriptor> {
    // `sort_by_key` es estable: conserva el orden del catálogo (español antes
    // que inglés en Supertonic) y solo agrupa mejoradas, otros motores y nativas.
    voices.sort_by_key(|voice| (!voice.enhanced, voice.engine == "android"));
    voices
}

/// Deja tres voces nativas por idioma (una por acento) y descarta variantes
/// técnicas, de red y de baja calidad. Piper, Supertonic y los perfiles
/// clonados se conservan intactos.
fn curate_voices(voices: Vec<VoiceDescriptor>) -> Vec<VoiceDescriptor> {
    let mut curated: Vec<VoiceDescriptor> = voices
        .iter()
        .filter(|voice| voice.enhanced || voice.engine != "android")
        .cloned()
        .collect();

    for language in ["es", "en"] {
        let all: Vec<&VoiceDescriptor> = voices
            .iter()
            .filter(|voice| voice.engine == "android" && voice.language == language)
            .collect();
        if all.is_empty() {
            continue;
        }

        let best_quality = all
            .iter()
            .filter_map(|voice| voice.quality)
            .max()
            .unwrap_or(0);
        let has_local = all.iter().any(|voice| !voice.requires_network);
        let mut candidates: Vec<&VoiceDescriptor> = all
            .iter()
            .copied()
            .filter(|voice| !voice.requires_network || !has_local)
            .filter(|voice| best_quality < 300 || voice.quality.unwrap_or(best_quality) >= 300)
            .collect();
        if candidates.is_empty() {
            candidates = all;
        }

        candidates.sort_by_key(|voice| {
            (
                voice.requires_network,
                std::cmp::Reverse(voice.quality.unwrap_or(300)),
                voice.latency.unwrap_or(300),
            )
        });

        let mut chosen: Vec<VoiceDescriptor> = Vec::new();
        for preferred in locale_preferences(language) {
            if chosen.len() >= 3 {
                break;
            }
            let Some(voice) = candidates.iter().find(|voice| {
                voice
                    .locale
                    .as_deref()
                    .map(|locale| locale.to_lowercase().starts_with(preferred))
                    .unwrap_or(false)
            }) else {
                continue;
            };
            if !chosen.iter().any(|item| item.id == voice.id) {
                chosen.push((*voice).clone());
            }
        }

        for voice in candidates {
            if chosen.len() >= 3 {
                break;
            }
            if chosen.iter().any(|item| item.id == voice.id) {
                continue;
            }
            let Some(locale) = voice.locale.as_deref() else {
                chosen.push(voice.clone());
                continue;
            };
            let duplicated = chosen.iter().any(|item| {
                item.locale
                    .as_deref()
                    .map(|current| current.eq_ignore_ascii_case(locale))
                    .unwrap_or(false)
            });
            if !duplicated {
                chosen.push(voice.clone());
            }
        }

        for mut voice in chosen {
            voice.name = friendly_voice_name(&voice.name, voice.requires_network);
            curated.push(voice);
        }
    }

    sort_voices(curated)
}

fn locale_preferences(language: &str) -> &'static [&'static str] {
    match language {
        "es" => &["es-es", "es-us", "es-mx", "es-419", "es-ar", "es-co"],
        "en" => &["en-us", "en-gb", "en-au", "en-in", "en-ca", "en-ie"],
        _ => &[],
    }
}

fn friendly_voice_name(name: &str, requires_network: bool) -> String {
    let base = name.split(" · ").next().unwrap_or(name).trim();
    let mut characters = base.chars();
    let base = match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    };
    if requires_network {
        format!("{base} · Red")
    } else {
        base
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupertonicDownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub done: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub fn supertonic_status(state: State<'_, AppState>) -> SupertonicStatus {
    state.supertonic.status()
}

#[tauri::command]
pub fn supertonic_accept_license(state: State<'_, AppState>) -> Result<()> {
    state.supertonic.accept_license()
}

#[tauri::command]
pub fn supertonic_set_enabled(state: State<'_, AppState>, enabled: bool) -> Result<()> {
    state.supertonic.set_enabled(enabled)
}

#[tauri::command]
pub fn supertonic_delete(state: State<'_, AppState>) -> Result<()> {
    state.supertonic.delete_pack()
}

#[tauri::command]
pub fn supertonic_unload(state: State<'_, AppState>) {
    state.supertonic.unload();
}

#[tauri::command]
pub async fn supertonic_preload(state: State<'_, AppState>) -> Result<()> {
    let service = state.supertonic.clone();
    tokio::task::spawn_blocking(move || service.preload())
        .await
        .map_err(|error| KoraError::Internal(format!("tarea Supertonic: {error}")))??;
    Ok(())
}

#[tauri::command]
pub async fn supertonic_download(state: State<'_, AppState>, app: AppHandle) -> Result<()> {
    let service = state.supertonic.clone();
    let app_clone = app.clone();

    let result = tokio::task::spawn_blocking(move || {
        let mut last_emit = std::time::Instant::now() - std::time::Duration::from_secs(1);
        let result = service.download(&mut |downloaded, total| {
            if last_emit.elapsed() >= std::time::Duration::from_millis(150) || downloaded >= total {
                last_emit = std::time::Instant::now();
                let _ = app_clone.emit(
                    "supertonic-download-progress",
                    SupertonicDownloadProgress {
                        downloaded,
                        total,
                        done: false,
                        error: None,
                    },
                );
            }
        });
        (result, service.status())
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de descarga Supertonic: {error}")))?;

    let (result, status) = result;
    match &result {
        Ok(()) => {
            let _ = state.supertonic.set_enabled(true);
            let _ = app.emit(
                "supertonic-download-progress",
                SupertonicDownloadProgress {
                    downloaded: status.total_bytes,
                    total: status.total_bytes,
                    done: true,
                    error: None,
                },
            );
        }
        Err(error) => {
            let _ = app.emit(
                "supertonic-download-progress",
                SupertonicDownloadProgress {
                    downloaded: status.downloaded_bytes,
                    total: status.total_bytes,
                    done: true,
                    error: Some(error.to_string()),
                },
            );
        }
    }
    result
}

#[tauri::command]
pub async fn tts_synthesize(
    state: State<'_, AppState>,
    request: SynthesisRequest,
) -> Result<SynthesisOutput> {
    let chunk = state.tts.synthesize(&request).await?;
    let wav = if let Some(id) = crate::tts::strip_clone_prefix(&request.voice_id) {
        let profile = state
            .voice_profiles
            .get(id, &state.identity.account.id)
            .await?
            .ok_or_else(|| KoraError::NotFound("el perfil de voz no existe".into()))?;
        crate::tts::encode_wav_with_watermark(
            &chunk,
            &crate::tts::wav::Watermark {
                profile_id: profile.id,
                profile_name: profile.name,
            },
        )?
    } else {
        crate::tts::encode_wav(&chunk)?
    };
    let duration_ms = if chunk.sample_rate > 0 {
        chunk.pcm.len() as u64 * 1000 / (chunk.sample_rate as u64 * chunk.channels.max(1) as u64)
    } else {
        0
    };

    Ok(SynthesisOutput {
        audio_base64: base64::engine::general_purpose::STANDARD.encode(wav),
        sample_rate: chunk.sample_rate,
        duration_ms,
    })
}

#[tauri::command]
pub async fn start_tts(
    state: State<'_, AppState>,
    app: AppHandle,
    book_id: String,
    from_offset: TtsOffset,
) -> Result<TtsStatus> {
    let excerpt = from_offset
        .end
        .map(|end| {
            if end <= from_offset.offset {
                Err(KoraError::InvalidInput(
                    "la selección no contiene texto para leer".into(),
                ))
            } else {
                Ok((from_offset.offset, end))
            }
        })
        .transpose()?;

    let prepared = match excerpt {
        Some((start, end)) => {
            prepare_excerpt(&state, &book_id, from_offset.section, start, end).await?
        }
        None => prepare_sequence(&state, &book_id, from_offset.section).await?,
    };
    if prepared.sequence.chunks.is_empty() {
        return Err(KoraError::Unsupported(
            "esta sección no tiene texto para leer".into(),
        ));
    }

    let installed = installed_voices(&state).await?;
    let default_voice = resolve_default_voice(
        &prepared.preferences,
        &installed,
        prepared.language.as_deref(),
    )?;
    let (character_voices, _) = build_character_voices(
        &state,
        &book_id,
        &prepared.sequence,
        &prepared.preferences,
        &installed,
        &default_voice,
    )
    .await?;

    let start_index = if excerpt.is_some() {
        0
    } else {
        prepared
            .sequence
            .chunks
            .iter()
            .position(|chunk| chunk.offset_end > from_offset.offset)
            .unwrap_or(prepared.sequence.chunks.len())
    };

    if start_index >= prepared.sequence.chunks.len() {
        return Err(KoraError::InvalidInput(
            "no hay texto desde esa posición".into(),
        ));
    }

    // Una sola fuente de audio: si había un audiolibro sonando, se guarda su
    // posición y se detiene al empezar a narrar el texto en vivo.
    if let Ok(Some(snapshot)) = state.audiobook_player.snapshot() {
        let _ = state
            .storage
            .audiobooks
            .save_progress(
                &snapshot.audiobook_id,
                &state.identity.account.id,
                snapshot.position_ms,
                snapshot.chapter_index as i64,
            )
            .await;
    }
    if let Err(error) = state.audiobook_player.stop() {
        tracing::warn!(%error, "no se pudo detener el audiolibro antes del TTS");
    }

    let plan = SessionPlan {
        book_id,
        section: from_offset.section,
        sequence: Arc::new(prepared.sequence),
        start_index,
        language: prepared.language,
        default_voice,
        character_voices,
        rate: prepared.preferences.rate as f32,
        pitch: prepared.preferences.pitch as f32,
        excerpt_end: excerpt.map(|(_, end)| end),
        media: prepared.media,
    };

    state.tts_session.start(app, plan).await
}

#[tauri::command]
pub async fn pause_tts(state: State<'_, AppState>, app: AppHandle) -> Result<TtsStatus> {
    state.tts_session.pause(&app).await
}

#[tauri::command]
pub async fn resume_tts(state: State<'_, AppState>, app: AppHandle) -> Result<TtsStatus> {
    state.tts_session.resume(&app).await
}

#[tauri::command]
pub async fn stop_tts(state: State<'_, AppState>, app: AppHandle) -> Result<TtsStatus> {
    state.tts_session.stop(&app).await
}

#[tauri::command]
pub async fn set_rate(state: State<'_, AppState>, rate: f32) -> Result<TtsStatus> {
    state.tts_session.set_rate(rate).await
}

#[tauri::command]
pub async fn set_pitch(state: State<'_, AppState>, pitch: f32) -> Result<TtsStatus> {
    state.tts_session.set_pitch(pitch).await
}

#[tauri::command]
pub async fn tts_status(state: State<'_, AppState>) -> Result<TtsStatus> {
    Ok(state.tts_session.status().await)
}

#[tauri::command]
pub async fn tts_characters(
    state: State<'_, AppState>,
    book_id: String,
    section: usize,
) -> Result<Vec<CharacterInfo>> {
    let prepared = prepare_sequence(&state, &book_id, section).await?;
    Ok(prepared.sequence.characters)
}

#[tauri::command]
pub async fn tts_character_voices(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<Vec<CharacterVoice>> {
    let _ = owned_book(&state, &book_id).await?;
    state.storage.tts.character_voices(&book_id).await
}

#[tauri::command]
pub async fn assign_character_voice(
    state: State<'_, AppState>,
    book_id: String,
    character: String,
    voice_id: String,
) -> Result<CharacterVoice> {
    let _ = owned_book(&state, &book_id).await?;
    let character = character.trim().to_string();
    if character.is_empty() || character.chars().count() > 120 {
        return Err(KoraError::InvalidInput(
            "el personaje debe tener entre 1 y 120 caracteres".into(),
        ));
    }

    let installed = installed_voices(&state).await?;
    if !installed.iter().any(|voice| voice.id == voice_id) {
        return Err(KoraError::InvalidInput(format!(
            "la voz {voice_id} no está instalada"
        )));
    }

    let saved = state
        .storage
        .tts
        .set_character_voice(NewCharacterVoice {
            book_id: book_id.clone(),
            character: character.clone(),
            voice_id: voice_id.clone(),
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await?;

    state.tts_session.set_voice(&character, &voice_id).await;

    Ok(saved)
}

#[tauri::command]
pub async fn tts_preferences(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<TtsPreferences> {
    let _ = owned_book(&state, &book_id).await?;
    Ok(state
        .storage
        .tts
        .preferences(&book_id)
        .await?
        .unwrap_or_else(|| {
            TtsPreferences::defaults(book_id.clone(), state.identity.account.id.clone())
        }))
}

#[tauri::command]
pub async fn save_tts_preferences(
    state: State<'_, AppState>,
    book_id: String,
    preferences: TtsPreferencesInput,
) -> Result<TtsPreferences> {
    let _ = owned_book(&state, &book_id).await?;

    if !(0.5..=3.0).contains(&preferences.rate) || !preferences.rate.is_finite() {
        return Err(KoraError::InvalidInput(
            "la velocidad debe estar entre 0.5 y 3.0".into(),
        ));
    }
    if !(0.5..=1.5).contains(&preferences.pitch) || !preferences.pitch.is_finite() {
        return Err(KoraError::InvalidInput(
            "el tono debe estar entre 0.5 y 1.5".into(),
        ));
    }
    if !(0.0..=3.0).contains(&preferences.pause_scale) || !preferences.pause_scale.is_finite() {
        return Err(KoraError::InvalidInput(
            "la escala de pausas debe estar entre 0 y 3".into(),
        ));
    }

    let saved = state
        .storage
        .tts
        .save_preferences(NewTtsPreferences {
            book_id,
            voice_id: preferences.voice_id,
            rate: preferences.rate as f64,
            pitch: preferences.pitch as f64,
            pause_scale: preferences.pause_scale as f64,
            expressive: preferences.expressive,
            multi_voice: preferences.multi_voice,
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await?;

    let _ = state.tts_session.set_rate(saved.rate as f32).await;
    let _ = state.tts_session.set_pitch(saved.pitch as f32).await;
    if let Some(voice_id) = &saved.voice_id {
        let installed = installed_voices(&state).await?;
        if installed.iter().any(|voice| &voice.id == voice_id) {
            state
                .tts_session
                .set_voice(crate::tts::NARRATOR, voice_id)
                .await;
        }
    }

    Ok(saved)
}

async fn prepare_sequence(
    state: &AppState,
    book_id: &str,
    section: usize,
) -> Result<PreparedSequence> {
    let book = owned_book(state, book_id).await?;
    let preferences = load_preferences(state, book_id).await?;
    let settings = expressive_settings(&preferences);

    match book.format {
        BookFormat::Epub => {
            let path = book_asset_path(state, &book)?;
            let chapter = tokio::task::spawn_blocking(move || {
                ReaderService::new().epub_chapter(&path, section)
            })
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de capítulo TTS: {error}")))??;
            let source = extract_text(&chapter.html);
            if source.is_empty() {
                return Err(KoraError::Unsupported(
                    "este capítulo no tiene texto para leer".into(),
                ));
            }

            let chapter_title = chapter.title.trim();
            let chapter_title = if chapter_title.is_empty() {
                format!("Sección {}", section + 1)
            } else {
                chapter_title.to_string()
            };
            let media = media_metadata(state, &book, chapter_title);
            let hash = content_hash(&source.text, &settings);
            if let Some(sequence) = load_cached_sequence(state, book_id, section, &hash).await? {
                return Ok(PreparedSequence {
                    sequence,
                    preferences,
                    language: book.language.clone(),
                    media,
                });
            }

            let sequence = build_from_text(section, &source, &settings);
            save_sequence(state, book_id, section, &hash, &sequence).await?;
            Ok(PreparedSequence {
                sequence,
                preferences,
                language: book.language,
                media,
            })
        }
        BookFormat::Pdf => {
            ensure_pdf(&book)?;
            let info = state.storage.pdf.get_info(book_id).await?;
            let page_count = info
                .as_ref()
                .map(|info| info.page_count.max(0) as usize)
                .unwrap_or(usize::MAX);
            if section >= page_count {
                return Err(KoraError::InvalidInput(format!(
                    "la página {section} está fuera del documento"
                )));
            }

            let layer = load_page_layer(state, &book, section).await?;
            let text = layer.text();
            if text.trim().is_empty() {
                return Err(KoraError::Unsupported(
                    "esta página no tiene texto reconocido; ejecuta el OCR".into(),
                ));
            }

            let media = media_metadata(state, &book, format!("Página {}", section + 1));
            let hash = content_hash(&text, &settings);
            if let Some(sequence) = load_cached_sequence(state, book_id, section, &hash).await? {
                return Ok(PreparedSequence {
                    sequence,
                    preferences,
                    language: book.language.clone(),
                    media,
                });
            }

            let sequence = build_from_words(section, &layer.words, &settings);
            save_sequence(state, book_id, section, &hash, &sequence).await?;
            Ok(PreparedSequence {
                sequence,
                preferences,
                language: book.language,
                media,
            })
        }
    }
}

fn media_metadata(state: &AppState, book: &Book, artist: String) -> TtsMediaMetadata {
    TtsMediaMetadata {
        title: book.title.clone(),
        artist,
        album: book.author.clone(),
        cover_path: crate::commands::audiobooks::cover_path(state, book),
    }
}

async fn load_preferences(state: &AppState, book_id: &str) -> Result<TtsPreferences> {
    Ok(state
        .storage
        .tts
        .preferences(book_id)
        .await?
        .unwrap_or_else(|| {
            TtsPreferences::defaults(book_id.to_string(), state.identity.account.id.clone())
        }))
}

fn expressive_settings(preferences: &TtsPreferences) -> ExpressiveSettings {
    ExpressiveSettings {
        enabled: preferences.expressive,
        pause_scale: preferences.pause_scale as f32,
        emphasis: 1.0,
    }
}

/// Construye una secuencia que contiene solo el rango seleccionado. Los
/// offsets se mantienen absolutos para que las anotaciones y el resaltado de
/// frases sigan apuntando al pasaje correcto.
async fn prepare_excerpt(
    state: &AppState,
    book_id: &str,
    section: usize,
    start: usize,
    end: usize,
) -> Result<PreparedSequence> {
    let book = owned_book(state, book_id).await?;
    let preferences = load_preferences(state, book_id).await?;
    let settings = expressive_settings(&preferences);

    match book.format {
        BookFormat::Epub => {
            let path = book_asset_path(state, &book)?;
            let chapter = tokio::task::spawn_blocking(move || {
                ReaderService::new().epub_chapter(&path, section)
            })
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de capítulo TTS: {error}")))??;
            let source = extract_text(&chapter.html);
            let excerpt = source_excerpt(&source, start, end).ok_or_else(|| {
                KoraError::InvalidInput("la selección no coincide con el texto del capítulo".into())
            })?;
            if excerpt.is_empty() {
                return Err(KoraError::Unsupported(
                    "la selección no tiene texto para leer".into(),
                ));
            }

            let chapter_title = chapter.title.trim();
            let chapter_title = if chapter_title.is_empty() {
                format!("Sección {}", section + 1)
            } else {
                chapter_title.to_string()
            };
            let media = media_metadata(state, &book, chapter_title);
            let mut sequence = build_from_text(section, &excerpt, &settings);
            shift_offsets(&mut sequence, start);
            Ok(PreparedSequence {
                sequence,
                preferences,
                language: book.language,
                media,
            })
        }
        BookFormat::Pdf => {
            ensure_pdf(&book)?;
            let layer = load_page_layer(state, &book, section).await?;
            let end = end.min(layer.words.len());
            if start >= end {
                return Err(KoraError::InvalidInput(
                    "la selección no coincide con el texto de la página".into(),
                ));
            }

            let media = media_metadata(state, &book, format!("Página {}", section + 1));
            let mut sequence = build_from_words(section, &layer.words[start..end], &settings);
            shift_offsets(&mut sequence, start);
            if sequence.chunks.is_empty() {
                return Err(KoraError::Unsupported(
                    "la selección no tiene texto para leer".into(),
                ));
            }
            Ok(PreparedSequence {
                sequence,
                preferences,
                language: book.language,
                media,
            })
        }
    }
}

fn source_excerpt(source: &SourceText, start: usize, end: usize) -> Option<SourceText> {
    if start >= end || end > source.utf16_len {
        return None;
    }

    let text = source.slice(start, end)?.to_string();
    let breaks = source
        .breaks
        .iter()
        .copied()
        .filter(|offset| *offset > start && *offset < end)
        .map(|offset| offset - start)
        .collect();
    Some(SourceText {
        text,
        utf16_len: end - start,
        breaks,
    })
}

fn shift_offsets(sequence: &mut SpeechSequence, base: usize) {
    if base == 0 {
        return;
    }
    for chunk in &mut sequence.chunks {
        chunk.offset_start += base;
        chunk.offset_end += base;
    }
}

async fn load_cached_sequence(
    state: &AppState,
    book_id: &str,
    section: usize,
    hash: &str,
) -> Result<Option<SpeechSequence>> {
    let Some(record) = state
        .storage
        .tts
        .get_sequence(book_id, section as i64, hash)
        .await?
    else {
        return Ok(None);
    };

    let sequence: SpeechSequence = serde_json::from_str(&record.sequence)?;
    if sequence.is_empty() {
        Ok(None)
    } else {
        Ok(Some(sequence))
    }
}

async fn save_sequence(
    state: &AppState,
    book_id: &str,
    section: usize,
    hash: &str,
    sequence: &SpeechSequence,
) -> Result<()> {
    state
        .storage
        .tts
        .save_sequence(NewTtsSequence {
            book_id: book_id.to_string(),
            section: section as i64,
            content_hash: hash.to_string(),
            unit: sequence.unit,
            sequence: serde_json::to_string(sequence)?,
            owner_account_id: state.identity.account.id.clone(),
            origin_share_id: None,
        })
        .await?;
    Ok(())
}

async fn load_page_layer(state: &AppState, book: &Book, page: usize) -> Result<PageTextLayer> {
    if let Some(mut layer) = read_ocr_page(&state.storage, &book.id, page)? {
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

async fn installed_voices(state: &AppState) -> Result<Vec<VoiceDescriptor>> {
    Ok(state
        .tts
        .voices()
        .await?
        .into_iter()
        .filter(|voice| voice.installed)
        .collect())
}

fn resolve_default_voice(
    preferences: &TtsPreferences,
    installed: &[VoiceDescriptor],
    language: Option<&str>,
) -> Result<String> {
    if installed.is_empty() {
        return Err(KoraError::Unsupported(
            "no hay ninguna voz instalada; descarga una voz para leer".into(),
        ));
    }

    if let Some(voice_id) = &preferences.voice_id
        && installed.iter().any(|voice| &voice.id == voice_id)
    {
        return Ok(voice_id.clone());
    }

    let language = language.map(|value| {
        value
            .split(['-', '_'])
            .next()
            .unwrap_or(value)
            .to_lowercase()
    });
    if let Some(language) = language {
        // Si el usuario no eligió voz, las voces mejoradas del pack opcional
        // tienen prioridad: evitan que «Escuchar» acabe usando Piper por orden
        // de registro del motor.
        if let Some(voice) = installed
            .iter()
            .find(|voice| voice.enhanced && voice.language.eq_ignore_ascii_case(&language))
        {
            return Ok(voice.id.clone());
        }
        if let Some(voice) = installed
            .iter()
            .find(|voice| voice.language.eq_ignore_ascii_case(&language))
        {
            return Ok(voice.id.clone());
        }
    }

    Ok(installed[0].id.clone())
}

async fn build_character_voices(
    state: &AppState,
    book_id: &str,
    sequence: &SpeechSequence,
    preferences: &TtsPreferences,
    installed: &[VoiceDescriptor],
    default_voice: &str,
) -> Result<(HashMap<String, String>, Vec<CharacterVoice>)> {
    let stored = state.storage.tts.character_voices(book_id).await?;
    let mut assignments: HashMap<String, String> = stored
        .iter()
        .filter(|voice| installed.iter().any(|item| item.id == voice.voice_id))
        .map(|voice| (voice.character.clone(), voice.voice_id.clone()))
        .collect();
    // La voz del narrador la controla el selector principal del lector.
    assignments.insert(NARRATOR.to_string(), default_voice.to_string());

    if !preferences.multi_voice {
        return Ok((
            HashMap::from([(NARRATOR.to_string(), default_voice.to_string())]),
            stored,
        ));
    }

    let alternatives: Vec<&VoiceDescriptor> = installed
        .iter()
        .filter(|voice| voice.id != default_voice)
        .collect();

    if !alternatives.is_empty() {
        let mut next = 0usize;
        for info in &sequence.characters {
            if info.name == NARRATOR || assignments.contains_key(&info.name) {
                continue;
            }

            let voice = alternatives[next % alternatives.len()];
            next += 1;
            assignments.insert(info.name.clone(), voice.id.clone());
            state
                .storage
                .tts
                .set_character_voice(NewCharacterVoice {
                    book_id: book_id.to_string(),
                    character: info.name.clone(),
                    voice_id: voice.id.clone(),
                    owner_account_id: state.identity.account.id.clone(),
                    origin_share_id: None,
                })
                .await?;
        }
    }

    Ok((assignments, stored))
}

#[tauri::command]
pub async fn tts_download_voice(
    state: State<'_, AppState>,
    app: AppHandle,
    voice_id: String,
) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        let _ = (&state, &app, &voice_id);
        return Err(KoraError::Unsupported(
            "la descarga de voces Piper no está disponible en Android".into(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        let root = state.layout.assets.join("tts");
        let app_clone = app.clone();
        let id = voice_id.clone();

        tokio::task::spawn_blocking(move || {
            let mut last_emit = std::time::Instant::now() - std::time::Duration::from_secs(1);
            let result =
                crate::tts::download::download_voice(&root, &id, &mut |downloaded, total| {
                    if last_emit.elapsed() >= std::time::Duration::from_millis(150)
                        || downloaded >= total
                    {
                        last_emit = std::time::Instant::now();
                        let _ = app_clone.emit(
                            "tts-download-progress",
                            VoiceDownloadProgress {
                                voice_id: id.clone(),
                                downloaded,
                                total,
                                done: false,
                                error: None,
                            },
                        );
                    }
                });

            match &result {
                Ok(()) => {
                    let _ = app_clone.emit(
                        "tts-download-progress",
                        VoiceDownloadProgress {
                            voice_id: id.clone(),
                            downloaded: 1,
                            total: 1,
                            done: true,
                            error: None,
                        },
                    );
                }
                Err(error) => {
                    let _ = app_clone.emit(
                        "tts-download-progress",
                        VoiceDownloadProgress {
                            voice_id: id.clone(),
                            downloaded: 0,
                            total: 0,
                            done: true,
                            error: Some(error.to_string()),
                        },
                    );
                }
            }

            result
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de descarga TTS: {error}")))??;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::TextWord;

    #[test]
    fn source_excerpt_slices_text_and_keeps_absolute_offsets() {
        let source = SourceText {
            text: "Hola mundo. Otra frase.".to_string(),
            utf16_len: "Hola mundo. Otra frase.".encode_utf16().count(),
            breaks: vec![],
        };

        let excerpt = source_excerpt(&source, 5, 10).expect("recorte válido");
        assert_eq!(excerpt.text, "mundo");

        let mut sequence = build_from_text(1, &excerpt, &ExpressiveSettings::default());
        shift_offsets(&mut sequence, 5);
        assert_eq!(sequence.chunks.len(), 1);
        assert_eq!(sequence.chunks[0].text, "mundo");
        assert_eq!(sequence.chunks[0].offset_start, 5);
        assert_eq!(sequence.chunks[0].offset_end, 10);
    }

    #[test]
    fn source_excerpt_rejects_empty_or_out_of_bounds_ranges() {
        let source = SourceText {
            text: "Uno".to_string(),
            utf16_len: 3,
            breaks: vec![],
        };

        assert!(source_excerpt(&source, 2, 2).is_none());
        assert!(source_excerpt(&source, 1, 4).is_none());
        assert!(source_excerpt(&source, 0, 4).is_none());
    }

    fn voice(id: &str, language: &str, enhanced: bool) -> VoiceDescriptor {
        VoiceDescriptor {
            id: id.to_string(),
            name: id.to_string(),
            language: language.to_string(),
            gender: None,
            engine: if enhanced { "supertonic" } else { "piper" }.to_string(),
            installed: true,
            size_bytes: 0,
            cloned: false,
            enhanced,
            quality: None,
            latency: None,
            requires_network: false,
            locale: Some(language.to_string()),
        }
    }

    #[test]
    fn default_voice_prefers_enhanced_pack() {
        let preferences = TtsPreferences::defaults("libro".into(), "cuenta".into());
        let installed = vec![
            voice("es_ES-davefx-medium", "es", false),
            voice("supertonic:es:F1", "es", true),
        ];
        assert_eq!(
            resolve_default_voice(&preferences, &installed, Some("es")).unwrap(),
            "supertonic:es:F1"
        );
    }

    #[test]
    fn explicit_voice_wins_over_enhanced_default() {
        let mut preferences = TtsPreferences::defaults("libro".into(), "cuenta".into());
        preferences.voice_id = Some("es_ES-davefx-medium".into());
        let installed = vec![
            voice("es_ES-davefx-medium", "es", false),
            voice("supertonic:es:F1", "es", true),
        ];
        assert_eq!(
            resolve_default_voice(&preferences, &installed, Some("es")).unwrap(),
            "es_ES-davefx-medium"
        );
    }

    #[test]
    fn shift_offsets_moves_word_ranges() {
        let words: Vec<TextWord> = ["Uno", "Dos", "Tres"]
            .iter()
            .map(|text| TextWord {
                text: text.to_string(),
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
                font_size: 1.0,
                confidence: None,
            })
            .collect();

        let mut sequence = build_from_words(4, &words, &ExpressiveSettings::default());
        shift_offsets(&mut sequence, 2);
        assert_eq!(sequence.chunks[0].offset_start, 2);
        assert_eq!(sequence.chunks[0].offset_end, 5);
    }
}
