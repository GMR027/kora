use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::audiobooks::{engine, scope::SourceSection};
use crate::error::{KoraError, Result};
use crate::features::journal::{month_bounds, year_bounds};
use crate::state::AppState;
use crate::storage::DiaryEntry;
use crate::tejido::cover;
use crate::tejido::epub::{EpubChapter, build_epub};

const MONTHS: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiaryExport {
    pub epub_path: String,
    pub audiobook_path: Option<String>,
    pub entries: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportProgress {
    stage: String,
    percent: f64,
    message: String,
}

pub async fn export(
    state: &AppState,
    app: &AppHandle,
    year: i32,
    month: Option<u32>,
    include_audiobook: bool,
    voice_id: Option<String>,
) -> Result<DiaryExport> {
    if !(1970..=2100).contains(&year) {
        return Err(KoraError::InvalidInput("año fuera de rango".into()));
    }
    if let Some(month) = month
        && !(1..=12).contains(&month)
    {
        return Err(KoraError::InvalidInput("mes fuera de rango".into()));
    }

    let owner = &state.identity.account.id;
    let (from, to) = match month {
        Some(month) => month_bounds(year, month),
        None => year_bounds(year),
    };
    let entries = state.storage.diary.list_range(owner, &from, &to).await?;
    if entries.is_empty() {
        return Err(KoraError::InvalidInput(
            "el periodo elegido no tiene entradas de diario".into(),
        ));
    }

    let voice = if include_audiobook {
        let voice_id = voice_id.ok_or_else(|| {
            KoraError::InvalidInput("elige una voz para el audiolibro del diario".into())
        })?;
        let voices = state.tts.voices().await?;
        Some(
            voices
                .into_iter()
                .find(|voice| voice.id == voice_id)
                .ok_or_else(|| {
                    KoraError::NotFound(format!("la voz {voice_id} no está instalada"))
                })?,
        )
    } else {
        None
    };

    let title = match month {
        Some(month) => format!(
            "Diario de Kora · {} de {}",
            MONTHS[month as usize - 1],
            year
        ),
        None => format!("Diario de Kora · {year}"),
    };
    let file_stem = match month {
        Some(month) => format!("diario-{year}-{month:02}"),
        None => format!("diario-{year}"),
    };
    let chapters = build_chapters(&entries, year, month);

    emit(
        app,
        "epub",
        10.0,
        "Preparando el EPUB del diario".to_string(),
    );

    let workspace = state
        .layout
        .root
        .join("tmp")
        .join(format!("diary-export-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&workspace)?;
    let cover_path = workspace.join("cover.png");
    let has_cover = cover::render_cover(&title, "Kora · Diario", &cover_path).is_ok();
    let epub_workspace_path = workspace.join("diario.epub");

    build_epub(
        &epub_workspace_path,
        &title,
        "Kora",
        "es",
        &chapters,
        has_cover.then_some(cover_path.as_path()),
    )?;

    std::fs::create_dir_all(&state.layout.exports)?;
    let epub_path = state.layout.exports.join(format!("{file_stem}.epub"));
    std::fs::rename(&epub_workspace_path, &epub_path)?;
    emit(app, "epub", 45.0, "EPUB del diario listo".to_string());

    let audiobook_path = if let Some(voice) = voice {
        emit(
            app,
            "audiobook",
            50.0,
            "Sintetizando el audiolibro del diario".to_string(),
        );

        let sections = chapters
            .iter()
            .enumerate()
            .map(|(index, chapter)| SourceSection {
                section: index,
                title: chapter.title.clone(),
                text: chapter.body.clone(),
                offset_start: 0,
                offset_end: chapter.body.encode_utf16().count(),
            })
            .collect::<Vec<_>>();

        let audio_workspace = workspace.join("audio");
        let synthesized = match engine::synthesize_sections(
            state,
            Some(app),
            &file_stem,
            &file_stem,
            &title,
            Some("Kora"),
            &sections,
            &voice.id,
            voice.cloned,
            crate::audiobooks::NarrationSettings::default(),
            crate::audiobooks::CastInput::default(),
            None,
            &audio_workspace,
        )
        .await
        {
            Ok(synthesized) => synthesized,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&workspace);
                return Err(error);
            }
        };

        let target = state.layout.exports.join(format!("{file_stem}.m4b"));
        std::fs::rename(&synthesized.path, &target)?;
        emit(
            app,
            "audiobook",
            100.0,
            "Audiolibro del diario listo".to_string(),
        );
        Some(target.to_string_lossy().into_owned())
    } else {
        None
    };

    let _ = std::fs::remove_dir_all(&workspace);

    Ok(DiaryExport {
        epub_path: epub_path.to_string_lossy().into_owned(),
        audiobook_path,
        entries: entries.len(),
    })
}

fn build_chapters(entries: &[DiaryEntry], year: i32, month: Option<u32>) -> Vec<EpubChapter> {
    match month {
        Some(_) => build_month_chapters(entries),
        None => build_year_chapters(entries, year),
    }
}

fn build_month_chapters(entries: &[DiaryEntry]) -> Vec<EpubChapter> {
    let mut by_day: BTreeMap<&str, Vec<&DiaryEntry>> = BTreeMap::new();
    for entry in entries {
        by_day
            .entry(entry.entry_date.as_str())
            .or_default()
            .push(entry);
    }

    by_day
        .into_iter()
        .map(|(date, day_entries)| {
            let title = day_label(date);
            let body = day_body(&day_entries);
            EpubChapter {
                title,
                body,
                source: Some("Kora · Diario".to_string()),
            }
        })
        .collect()
}

fn build_year_chapters(entries: &[DiaryEntry], year: i32) -> Vec<EpubChapter> {
    let mut by_month: BTreeMap<u32, Vec<&DiaryEntry>> = BTreeMap::new();
    for entry in entries {
        if let Ok(date) = NaiveDate::parse_from_str(&entry.entry_date, "%Y-%m-%d") {
            by_month.entry(date.month()).or_default().push(entry);
        }
    }

    by_month
        .into_iter()
        .map(|(month, month_entries)| {
            let title = format!("{} de {year}", MONTHS[month as usize - 1]);
            let mut body = String::new();
            let mut current_day = "";
            for entry in &month_entries {
                if entry.entry_date != current_day {
                    if !body.is_empty() {
                        body.push('\n');
                    }
                    body.push_str(&day_label(&entry.entry_date));
                    body.push('\n');
                    current_day = &entry.entry_date;
                }
                body.push_str(&entry_body(entry));
                body.push('\n');
            }
            EpubChapter {
                title,
                body: body.trim_end().to_string(),
                source: Some("Kora · Diario".to_string()),
            }
        })
        .collect()
}

fn day_body(entries: &[&DiaryEntry]) -> String {
    let mut body = String::new();
    for entry in entries {
        body.push_str(&entry_body(entry));
        body.push('\n');
    }
    body.trim_end().to_string()
}

fn entry_body(entry: &DiaryEntry) -> String {
    entry.body.clone()
}

fn day_label(date: &str) -> String {
    let Ok(date) = NaiveDate::parse_from_str(date, "%Y-%m-%d") else {
        return date.to_string();
    };
    format!(
        "{} de {} de {}",
        date.day(),
        MONTHS[date.month() as usize - 1],
        date.year()
    )
}

fn emit(app: &AppHandle, stage: &str, percent: f64, message: String) {
    let _ = app.emit(
        "diary::export-progress",
        ExportProgress {
            stage: stage.to_string(),
            percent,
            message,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::DiaryKind;
    use chrono::{TimeZone, Utc};

    fn entry(date: &str, kind: DiaryKind, body: &str) -> DiaryEntry {
        DiaryEntry {
            id: format!("{date}-{body}"),
            book_id: None,
            entry_date: date.to_string(),
            body: body.to_string(),
            mood: None,
            kind,
            session_start: None,
            session_end: None,
            duration_minutes: 10,
            pages_read: 2,
            extracts_created: 0,
            notes_created: 0,
            pomodoro_round: None,
            created_at: Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
            owner_account_id: "owner".to_string(),
            origin_share_id: None,
        }
    }

    #[test]
    fn month_export_groups_entries_by_day() {
        let entries = vec![
            entry("2026-03-03", DiaryKind::Reading, "primera"),
            entry("2026-03-03", DiaryKind::Pomodoro, "segunda"),
            entry("2026-03-04", DiaryKind::Listening, "tercera"),
        ];
        let chapters = build_chapters(&entries, 2026, Some(3));
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].title, "3 de marzo de 2026");
        assert!(chapters[0].body.contains("primera"));
        assert!(chapters[0].body.contains("segunda"));
        assert_eq!(chapters[1].title, "4 de marzo de 2026");
    }

    #[test]
    fn year_export_groups_entries_by_month() {
        let entries = vec![
            entry("2026-01-03", DiaryKind::Reading, "enero"),
            entry("2026-03-04", DiaryKind::Listening, "marzo"),
        ];
        let chapters = build_chapters(&entries, 2026, None);
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].title, "enero de 2026");
        assert_eq!(chapters[1].title, "marzo de 2026");
    }
}
