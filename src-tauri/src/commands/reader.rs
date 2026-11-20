use tauri::State;

use crate::error::{KoraError, Result};
use crate::reader::{Locator, ReaderSession};
use crate::state::AppState;

#[tauri::command]
pub async fn reader_open(state: State<'_, AppState>, book_id: String) -> Result<ReaderSession> {
    match state.storage.books.get(&book_id).await? {
        Some(_book) => Err(KoraError::not_implemented(
            "reader_open (fase 4: resolver asset y abrir parser)",
        )),
        None => Err(KoraError::NotFound(format!("libro {book_id}"))),
    }
}

#[tauri::command]
pub async fn reader_update_progress(
    state: State<'_, AppState>,
    book_id: String,
    locator: Locator,
) -> Result<()> {
    let _ = (state, book_id, locator);
    Err(KoraError::not_implemented(
        "reader_update_progress (fase 4)",
    ))
}
