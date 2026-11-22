use chrono::Utc;
use serde::Serialize;
use serde_json::json;

use crate::error::{KoraError, Result};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotesExportResult {
    pub path: String,
    pub books: usize,
    pub entries: usize,
}

/// Exporta subrayados, notas, extractos y notas de voz a Markdown o JSON.
/// El archivo se escribe en la ruta elegida por el usuario o en `exports/`.
pub async fn export(
    state: &AppState,
    book_id: Option<&str>,
    format: &str,
    destination: Option<&str>,
) -> Result<NotesExportResult> {
    if !matches!(format, "markdown" | "json") {
        return Err(KoraError::InvalidInput(
            "el formato de notas debe ser markdown o json".into(),
        ));
    }

    let owner = &state.identity.account.id;
    let books = match book_id {
        Some(id) => state
            .storage
            .books
            .get(id)
            .await?
            .into_iter()
            .collect::<Vec<_>>(),
        None => state.storage.books.list_by_owner(owner).await?,
    };

    let mut json_books = Vec::new();
    let mut markdown = String::from("# Notas de Kora\n\n");
    markdown.push_str(&format!(
        "Exportado el {} · {} libro(s)\n\n",
        Utc::now().format("%d/%m/%Y %H:%M"),
        books.len()
    ));
    let mut entry_count = 0usize;

    for book in &books {
        let annotations = state.storage.annotations.list_by_book(&book.id).await?;
        let notes = state.storage.notes.list_by_book(&book.id).await?;
        let extracts = state.storage.extracts.list_by_book(&book.id).await?;
        let voice_notes = state.storage.voice_notes.list_by_book(&book.id).await?;
        let total = annotations.len() + notes.len() + extracts.len() + voice_notes.len();
        if total == 0 {
            continue;
        }
        entry_count += total;

        markdown.push_str(&format!(
            "## {} — {}\n\n",
            book.title,
            book.author.as_deref().unwrap_or("Autor desconocido")
        ));

        if !annotations.is_empty() {
            markdown.push_str("### Marcadores y subrayados\n\n");
            for annotation in &annotations {
                let kind = format!("{:?}", annotation.kind).to_lowercase();
                let quote = annotation.text.as_deref().unwrap_or("").trim();
                if !quote.is_empty() {
                    markdown.push_str(&format!("- **{kind}**: «{quote}»\n"));
                } else {
                    markdown.push_str(&format!("- **{kind}**\n"));
                }
                if let Some(note) = annotation
                    .note
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                {
                    markdown.push_str(&format!("  - Nota: {note}\n"));
                }
            }
            markdown.push('\n');
        }

        if !extracts.is_empty() {
            markdown.push_str("### Extractos\n\n");
            for extract in &extracts {
                markdown.push_str(&format!("- «{}»\n", extract.text.trim()));
                if let Some(note) = extract
                    .note
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                {
                    markdown.push_str(&format!("  - {note}\n"));
                }
            }
            markdown.push('\n');
        }

        if !notes.is_empty() {
            markdown.push_str("### Notas\n\n");
            for note in &notes {
                if let Some(title) = note
                    .title
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                {
                    markdown.push_str(&format!("#### {title}\n\n"));
                }
                markdown.push_str(note.body.trim());
                markdown.push_str("\n\n");
            }
        }

        if !voice_notes.is_empty() {
            markdown.push_str("### Notas de voz\n\n");
            for voice in &voice_notes {
                markdown.push_str(&format!(
                    "- [{}] {}\n",
                    format_duration(voice.duration_ms),
                    voice.text.trim()
                ));
            }
            markdown.push('\n');
        }

        json_books.push(json!({
            "id": book.id,
            "title": book.title,
            "author": book.author,
            "annotations": annotations.iter().map(|annotation| json!({
                "kind": annotation.kind,
                "text": annotation.text,
                "note": annotation.note,
                "chapter": annotation.chapter_index,
                "createdAt": annotation.created_at,
            })).collect::<Vec<_>>(),
            "extracts": extracts.iter().map(|extract| json!({
                "text": extract.text,
                "note": extract.note,
                "citation": extract.citation,
                "chapter": extract.chapter_index,
                "createdAt": extract.created_at,
            })).collect::<Vec<_>>(),
            "notes": notes.iter().map(|note| json!({
                "title": note.title,
                "body": note.body,
                "chapter": note.chapter_index,
                "createdAt": note.created_at,
            })).collect::<Vec<_>>(),
            "voiceNotes": voice_notes.iter().map(|voice| json!({
                "text": voice.text,
                "durationMs": voice.duration_ms,
                "chapter": voice.chapter_index,
                "createdAt": voice.created_at,
            })).collect::<Vec<_>>(),
        }));
    }

    let extension = if format == "json" { "json" } else { "md" };
    let path = match destination {
        Some(path) if !path.trim().is_empty() => std::path::PathBuf::from(path.trim()),
        _ => state.layout.exports.join(format!(
            "kora-notas-{}.{}",
            Utc::now().format("%Y%m%d-%H%M"),
            extension
        )),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if format == "json" {
        let payload = json!({
            "exportedAt": Utc::now(),
            "books": json_books,
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&payload)?)?;
    } else {
        std::fs::write(&path, markdown.as_bytes())?;
    }

    Ok(NotesExportResult {
        path: path.to_string_lossy().to_string(),
        books: json_books.len(),
        entries: entry_count,
    })
}

fn format_duration(milliseconds: i64) -> String {
    let seconds = (milliseconds.max(0) / 1000) as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
