use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::ai::AiMessage;
use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::tts::practice::{self, find_model, model_path};

use super::audio;
use super::chapters::{self, ChapterSource, DetectedChapter};
use super::engine::{AudiobookProgressEvent, emit_progress};
use super::jobs::GenerationControl;
use super::m4b;

/// Cómo se analiza un audio para dividirlo en capítulos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AudioImportMode {
    /// Solo señales acústicas. No requiere IA ni Whisper.
    #[default]
    Acoustic,
    /// Candidatos acústicos + muestras cortas + IA local.
    AiFast,
    /// Transcripción interna con Whisper (sin guardar texto) + IA local.
    AiPrecise,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioImportOptions {
    #[serde(default)]
    pub mode: AudioImportMode,
    #[serde(default)]
    pub language: Option<String>,
    /// `tiny`, `base` o vacío para elegir el mejor instalado.
    #[serde(default)]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AudioChapterDetection {
    pub chapters: Vec<DetectedChapter>,
    pub source: ChapterSource,
    pub ai_used: bool,
    pub note: Option<String>,
}

const MAX_BLOCKS: usize = 40;
const FAST_SAMPLE_SECONDS: f64 = 24.0;
const MIN_AI_BLOCK_MS: i64 = 300_000;
const MAX_AI_BLOCK_MS: i64 = 600_000;

/// Detecta capítulos para un audio importado. Nunca devuelve una lista vacía:
/// si todo lo demás falla, cae a bloques fijos de diez minutos.
pub async fn detect(
    state: &AppState,
    app: &AppHandle,
    source: &Path,
    duration_ms: i64,
    options: &AudioImportOptions,
    control: Option<&GenerationControl>,
) -> Result<AudioChapterDetection> {
    let duration_ms = duration_ms.max(1);
    let probe = tokio::task::spawn_blocking({
        let source = source.to_path_buf();
        move || m4b::probe(&source)
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de metadatos: {error}")))??;

    if probe.chapters.len() >= 2 {
        let embedded: Vec<(i64, i64, String)> = probe
            .chapters
            .into_iter()
            .map(|chapter| (chapter.start_ms, chapter.end_ms, chapter.title))
            .collect();
        if let Some(detection) = chapters::from_embedded(&embedded, duration_ms) {
            emit_progress(
                app,
                AudiobookProgressEvent {
                    audiobook_id: None,
                    phase: "detecting".to_string(),
                    chapter: detection.chapters.len(),
                    total_chapters: detection.chapters.len(),
                    percent: 18.0,
                    message: format!(
                        "{} capítulos embebidos encontrados",
                        detection.chapters.len()
                    ),
                },
            );
            return Ok(AudioChapterDetection {
                chapters: detection.chapters,
                source: ChapterSource::Embedded,
                ai_used: false,
                note: None,
            });
        }
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
            percent: 8.0,
            message: "Buscando pausas y cambios de energía".to_string(),
        },
    );
    let silences = tokio::task::spawn_blocking({
        let source = source.to_path_buf();
        move || m4b::detect_silences(&source, duration_ms)
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de silencios: {error}")))?
    .unwrap_or_else(|error| {
        tracing::warn!(%error, "no se pudieron detectar silencios");
        Vec::new()
    });

    if let Some(control) = control {
        control.checkpoint().await?;
    }

    let tones = tokio::task::spawn_blocking({
        let source = source.to_path_buf();
        move || m4b::tone_boundaries(&source)
    })
    .await
    .map_err(|error| KoraError::Internal(format!("tarea de tonos: {error}")))?
    .unwrap_or_else(|error| {
        tracing::warn!(%error, "no se pudieron detectar tonos");
        Vec::new()
    });

    let acoustic = chapters::from_signals(&silences, &tones, duration_ms);
    if options.mode == AudioImportMode::Acoustic {
        return Ok(AudioChapterDetection {
            chapters: acoustic.chapters,
            source: acoustic.source,
            ai_used: false,
            note: None,
        });
    }

    let ai_status = state.ai.status().await?;
    if !ai_status.available {
        emit_progress(
            app,
            AudiobookProgressEvent {
                audiobook_id: None,
                phase: "detecting".to_string(),
                chapter: 0,
                total_chapters: 0,
                percent: 22.0,
                message: "La IA local no está disponible; usando detección acústica".to_string(),
            },
        );
        return Ok(AudioChapterDetection {
            chapters: acoustic.chapters,
            source: acoustic.source,
            ai_used: false,
            note: Some(
                ai_status
                    .error
                    .unwrap_or_else(|| "la IA local no está disponible".to_string()),
            ),
        });
    }

    let Some((model_id, whisper_path)) = installed_analysis_model(state, options) else {
        emit_progress(
            app,
            AudiobookProgressEvent {
                audiobook_id: None,
                phase: "detecting".to_string(),
                chapter: 0,
                total_chapters: 0,
                percent: 22.0,
                message: "Whisper no está instalado; usando detección acústica".to_string(),
            },
        );
        return Ok(AudioChapterDetection {
            chapters: acoustic.chapters,
            source: acoustic.source,
            ai_used: false,
            note: Some(
                "descarga Whisper Tiny o Base para usar la detección de capítulos con IA"
                    .to_string(),
            ),
        });
    };

    let mut blocks = candidate_blocks(&silences, &tones, duration_ms);
    if options.mode == AudioImportMode::AiPrecise {
        blocks = coarsen_blocks(&blocks, MIN_AI_BLOCK_MS, MAX_AI_BLOCK_MS);
    }
    if blocks.is_empty() {
        return Ok(AudioChapterDetection {
            chapters: acoustic.chapters,
            source: acoustic.source,
            ai_used: false,
            note: None,
        });
    }

    let workspace = state
        .layout
        .root
        .join("tmp")
        .join(format!("audio-analysis-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace)?;
    let total = blocks.len();

    let mut analysed = Vec::with_capacity(total);
    for (index, (start_ms, end_ms)) in blocks.iter().enumerate() {
        if let Some(control) = control {
            control.checkpoint().await?;
        }
        emit_progress(
            app,
            AudiobookProgressEvent {
                audiobook_id: None,
                phase: "analyzing".to_string(),
                chapter: index + 1,
                total_chapters: total,
                percent: 25.0 + (index as f64 / total as f64) * 35.0,
                message: if options.mode == AudioImportMode::AiFast {
                    format!(
                        "Analizando muestra {} de {} con Whisper {model_id}",
                        index + 1,
                        total
                    )
                } else {
                    format!(
                        "Transcribiendo bloque {} de {} para la IA",
                        index + 1,
                        total
                    )
                },
            },
        );
        let text = analyse_block(
            source,
            &workspace,
            *start_ms,
            *end_ms,
            options.mode,
            &whisper_path,
            options.language.as_deref().unwrap_or("auto"),
        )
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "no se pudo analizar el bloque de audio");
            String::new()
        });
        analysed.push(AnalysisBlock {
            id: index,
            start_ms: *start_ms,
            end_ms: *end_ms,
            text,
        });
    }

    let _ = std::fs::remove_dir_all(&workspace);

    if let Some(control) = control {
        control.checkpoint().await?;
    }

    emit_progress(
        app,
        AudiobookProgressEvent {
            audiobook_id: None,
            phase: "analyzing".to_string(),
            chapter: total,
            total_chapters: total,
            percent: 64.0,
            message: format!(
                "Pidiendo a {} ({}) que agrupe los capítulos",
                ai_status.provider, ai_status.model
            ),
        },
    );

    match ask_ai_for_chapters(
        state,
        &analysed,
        &ai_status,
        options.language.as_deref(),
        control,
    )
    .await
    {
        Ok(groups) => {
            let detected = groups_to_chapters(&groups, &analysed, duration_ms);
            if detected.is_empty() {
                Ok(AudioChapterDetection {
                    chapters: acoustic.chapters,
                    source: acoustic.source,
                    ai_used: false,
                    note: Some("la IA no devolvió una división válida".to_string()),
                })
            } else {
                Ok(AudioChapterDetection {
                    chapters: detected,
                    source: ChapterSource::Ai,
                    ai_used: true,
                    note: None,
                })
            }
        }
        Err(error) => {
            tracing::warn!(%error, "la IA no pudo agrupar los capítulos");
            Ok(AudioChapterDetection {
                chapters: acoustic.chapters,
                source: acoustic.source,
                ai_used: false,
                note: Some(format!(
                    "no se pudo consultar la IA ({error}); se usó detección acústica"
                )),
            })
        }
    }
}

fn installed_analysis_model(
    state: &AppState,
    options: &AudioImportOptions,
) -> Option<(&'static str, PathBuf)> {
    let mut preferred = Vec::new();
    if let Some(id) = options.model_id.as_deref() {
        preferred.push(id);
    }
    preferred.extend(["tiny", "base"]);
    for id in preferred {
        let Some(spec) = find_model(id) else {
            continue;
        };
        if practice::installed(&state.layout.assets, spec.id) {
            return Some((spec.id, model_path(&state.layout.assets, spec.id)));
        }
    }
    None
}

/// Bloques candidatos a capítulo. Los tonos mandan; los silencios se ordenan
/// por duración para conservar las pausas más probables.
fn candidate_blocks(silences: &[(i64, i64)], tones: &[i64], duration_ms: i64) -> Vec<(i64, i64)> {
    let duration_ms = duration_ms.max(1);
    let mut boundaries = vec![0i64, duration_ms];

    for tone in tones {
        if *tone > 0 && *tone < duration_ms {
            boundaries.push(*tone);
        }
    }

    let mut silence_candidates: Vec<(i64, i64)> = silences
        .iter()
        .filter(|(begin, end)| end - begin >= 1_500)
        .map(|(begin, end)| (((begin + end) / 2), end - begin))
        .collect();
    silence_candidates.sort_by(|left, right| right.1.cmp(&left.1));
    silence_candidates.truncate(MAX_BLOCKS);
    boundaries.extend(silence_candidates.into_iter().map(|(middle, _)| middle));

    boundaries.sort_unstable();
    boundaries.dedup();
    let min_gap = if tones.is_empty() { 90_000 } else { 45_000 };
    let mut kept = Vec::with_capacity(boundaries.len());
    for boundary in boundaries {
        let is_edge = boundary == 0 || boundary == duration_ms;
        let far_enough = kept
            .last()
            .map(|last| boundary - *last >= min_gap)
            .unwrap_or(true);
        if is_edge || far_enough {
            kept.push(boundary);
        }
        if kept.len() >= MAX_BLOCKS + 1 {
            break;
        }
    }
    if !kept.contains(&duration_ms) {
        kept.push(duration_ms);
    }
    kept.sort_unstable();
    kept.dedup();

    let mut blocks: Vec<(i64, i64)> = kept
        .windows(2)
        .filter_map(|window| {
            let start = window[0];
            let end = window[1];
            (end - start >= 20_000).then_some((start, end))
        })
        .collect();

    if blocks.len() < 2 {
        blocks = fixed_blocks(duration_ms, 600_000);
    }
    if blocks.is_empty() {
        blocks.push((0, duration_ms));
    }
    blocks
}

fn fixed_blocks(duration_ms: i64, span_ms: i64) -> Vec<(i64, i64)> {
    let duration_ms = duration_ms.max(1);
    let mut blocks = Vec::new();
    let mut start = 0;
    while start < duration_ms {
        let end = (start + span_ms).min(duration_ms);
        blocks.push((start, end));
        start = end;
    }
    if blocks.is_empty() {
        blocks.push((0, duration_ms));
    }
    blocks
}

/// Une bloques contiguos hasta alcanzar el mínimo. Los bloques demasiado
/// largos se parten para no cargar diez minutos de audio de golpe.
fn coarsen_blocks(blocks: &[(i64, i64)], min_ms: i64, max_ms: i64) -> Vec<(i64, i64)> {
    let mut result = Vec::new();
    let mut current: Option<(i64, i64)> = None;
    for block in blocks {
        current = match current {
            None => Some(*block),
            Some((start, end)) => {
                let merged = (start, block.1);
                if merged.1 - merged.0 <= max_ms {
                    Some(merged)
                } else if end - start >= min_ms {
                    result.push((start, end));
                    Some(*block)
                } else {
                    Some(merged)
                }
            }
        };
    }
    if let Some(block) = current {
        result.push(block);
    }
    if result.is_empty() {
        return blocks.to_vec();
    }
    result
}

async fn analyse_block(
    source: &Path,
    workspace: &Path,
    start_ms: i64,
    end_ms: i64,
    mode: AudioImportMode,
    whisper_path: &Path,
    language: &str,
) -> Result<String> {
    let start_seconds = start_ms.max(0) as f64 / 1000.0;
    let span_seconds = if mode == AudioImportMode::AiFast {
        FAST_SAMPLE_SECONDS.min((end_ms - start_ms).max(1) as f64 / 1000.0)
    } else {
        (end_ms - start_ms).max(1) as f64 / 1000.0
    };
    let wav_path = workspace.join(format!("block-{start_ms}.wav"));
    m4b::extract_wav(source, &wav_path, Some(start_seconds), Some(span_seconds))?;
    let samples = audio::read_wav_mono(&wav_path)?;
    let _ = std::fs::remove_file(&wav_path);
    if samples.is_empty() {
        return Ok(String::new());
    }
    let model = whisper_path.to_path_buf();
    let language = language.to_string();
    tokio::task::spawn_blocking(move || practice::transcribe(&model, &samples, &language))
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de Whisper: {error}")))?
}

#[derive(Debug, Clone)]
struct AnalysisBlock {
    id: usize,
    start_ms: i64,
    end_ms: i64,
    text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiChapterGroup {
    start_block: usize,
    end_block: usize,
    #[serde(default)]
    title: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiChapterResponse {
    chapters: Vec<AiChapterGroup>,
}

async fn ask_ai_for_chapters(
    state: &AppState,
    blocks: &[AnalysisBlock],
    ai_status: &crate::ai::AiStatus,
    language: Option<&str>,
    control: Option<&GenerationControl>,
) -> Result<Vec<AiChapterGroup>> {
    let text_limit = if blocks.len() > 30 { 350 } else { 600 };
    let payload = blocks
        .iter()
        .map(|block| {
            serde_json::json!({
                "id": block.id,
                "startMs": block.start_ms,
                "endMs": block.end_ms,
                "text": block.text.chars().take(text_limit).collect::<String>(),
            })
        })
        .collect::<Vec<_>>();
    let language = language.unwrap_or("es");
    let system = "Eres un editor profesional de audiolibros. Recibes bloques contiguos \
        de un audio con una muestra o transcripción. Agrupa bloques consecutivos en \
        capítulos y propón un título breve y descriptivo. No inventes tiempos: solo \
        puedes agrupar bloques existentes. Devuelve exclusivamente JSON válido con la \
        forma {\"chapters\":[{\"startBlock\":0,\"endBlock\":2,\"title\":\"...\"}]}. \
        Debes cubrir todos los bloques, sin solapamientos ni huecos.";
    let user = format!(
        "Idioma preferido: {language}\nProveedor: {} ({})\nBloques:\n{}",
        ai_status.provider,
        ai_status.model,
        serde_json::to_string_pretty(&payload)?
    );
    let request = state.ai.chat(
        vec![
            AiMessage {
                role: "system".to_string(),
                content: system.to_string(),
            },
            AiMessage {
                role: "user".to_string(),
                content: user,
            },
        ],
        Some(0.2),
        Some(1_500),
    );
    let reply = if let Some(control) = control {
        tokio::select! {
            reply = request => reply?,
            _ = wait_for_cancel(control) => {
                return Err(KoraError::Cancelled(
                    "la detección de capítulos con IA se canceló".into(),
                ));
            }
        }
    } else {
        request.await?
    };
    parse_ai_chapters(&reply.content, blocks.len())
}

async fn wait_for_cancel(control: &GenerationControl) {
    loop {
        if control.cancelled() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    }
}

fn parse_ai_chapters(content: &str, block_count: usize) -> Result<Vec<AiChapterGroup>> {
    let start = content
        .find('{')
        .ok_or_else(|| KoraError::InvalidInput("la IA no devolvió JSON".into()))?;
    let end = content
        .rfind('}')
        .ok_or_else(|| KoraError::InvalidInput("la IA devolvió un JSON incompleto".into()))?;
    let parsed: AiChapterResponse = serde_json::from_str(&content[start..=end])
        .map_err(|error| KoraError::InvalidInput(format!("JSON de capítulos inválido: {error}")))?;
    let mut next = 0usize;
    for chapter in &parsed.chapters {
        if chapter.start_block != next || chapter.end_block < chapter.start_block {
            return Err(KoraError::InvalidInput(
                "la IA devolvió bloques solapados o con huecos".into(),
            ));
        }
        next = chapter.end_block + 1;
    }
    if next != block_count {
        return Err(KoraError::InvalidInput(
            "la IA no cubrió todos los bloques de audio".into(),
        ));
    }
    Ok(parsed.chapters)
}

fn groups_to_chapters(
    groups: &[AiChapterGroup],
    blocks: &[AnalysisBlock],
    duration_ms: i64,
) -> Vec<DetectedChapter> {
    let mut chapters = Vec::with_capacity(groups.len());
    for group in groups {
        let Some(first) = blocks.get(group.start_block) else {
            continue;
        };
        let Some(last) = blocks.get(group.end_block) else {
            continue;
        };
        let title = group.title.trim();
        let title = if title.is_empty() {
            format!("Capítulo {}", chapters.len() + 1)
        } else {
            title.chars().take(160).collect()
        };
        chapters.push(DetectedChapter {
            start_ms: first.start_ms,
            end_ms: last.end_ms.min(duration_ms.max(1)),
            title,
        });
    }
    if let Some(first) = chapters.first_mut() {
        first.start_ms = 0;
    }
    if let Some(last) = chapters.last_mut() {
        last.end_ms = duration_ms.max(1);
    }
    chapters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_blocks_fall_back_to_fixed_chapters() {
        let blocks = candidate_blocks(&[], &[], 1_800_000);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0], (0, 600_000));
        assert_eq!(blocks[2], (1_200_000, 1_800_000));
    }

    #[test]
    fn candidate_blocks_use_silence_midpoints() {
        let blocks = candidate_blocks(&[(100_000, 110_000)], &[], 600_000);
        assert!(blocks.len() >= 2);
        assert_eq!(blocks[0].0, 0);
        assert_eq!(blocks.last().expect("último").1, 600_000);
    }

    #[test]
    fn coarsen_blocks_respects_limits() {
        let blocks = vec![(0, 60_000), (60_000, 120_000), (120_000, 500_000)];
        let merged = coarsen_blocks(&blocks, 300_000, 600_000);
        assert!(!merged.is_empty());
        assert_eq!(merged.last().expect("último").1, 500_000);
    }

    #[test]
    fn parses_and_validates_ai_json() {
        let groups = parse_ai_chapters(
            r#"```json
            {"chapters":[{"startBlock":0,"endBlock":1,"title":"Uno"},
                         {"startBlock":2,"endBlock":2,"title":"Dos"}]}
            ```"#,
            3,
        )
        .expect("grupos");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[1].title, "Dos");
    }

    #[test]
    fn rejects_ai_groups_with_gaps() {
        let error = parse_ai_chapters(
            r#"{"chapters":[{"startBlock":0,"endBlock":0,"title":"Uno"}]}"#,
            2,
        )
        .expect_err("debe fallar");
        assert!(matches!(error, KoraError::InvalidInput(_)));
    }

    #[test]
    fn groups_cover_the_whole_duration() {
        let blocks = vec![
            AnalysisBlock {
                id: 0,
                start_ms: 0,
                end_ms: 1000,
                text: "a".into(),
            },
            AnalysisBlock {
                id: 1,
                start_ms: 1000,
                end_ms: 2000,
                text: "b".into(),
            },
        ];
        let chapters = groups_to_chapters(
            &[AiChapterGroup {
                start_block: 0,
                end_block: 1,
                title: "Uno".into(),
            }],
            &blocks,
            2500,
        );
        assert_eq!(chapters[0].start_ms, 0);
        assert_eq!(chapters[0].end_ms, 2500);
    }
}
