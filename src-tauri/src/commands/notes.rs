use tauri::State;

use crate::commands::reader::owned_book;
use crate::error::Result;
use crate::features::notes_export::{NotesExportResult, export};
use crate::state::AppState;

#[tauri::command]
pub async fn export_notes(
    state: State<'_, AppState>,
    book_id: Option<String>,
    format: String,
    destination: Option<String>,
) -> Result<NotesExportResult> {
    if let Some(book_id) = book_id.as_deref() {
        owned_book(&state, book_id).await?;
    }
    export(&state, book_id.as_deref(), &format, destination.as_deref()).await
}
