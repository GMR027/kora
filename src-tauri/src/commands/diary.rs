use chrono::{Local, NaiveDate};
use tauri::{AppHandle, Emitter, State};

use crate::error::{KoraError, Result};
use crate::features::diary_export::{self, DiaryExport};
use crate::features::journal::{DiaryMonth, aggregate_month, month_bounds};
use crate::features::sessions::{SessionKind, SessionStatus};
use crate::state::AppState;
use crate::storage::{
    Achievement, DiaryEntry, DiaryKind, NewAuditEntry, NewDiaryEntry, NewDiarySession, ResourceType,
};

#[tauri::command]
pub async fn get_diary(state: State<'_, AppState>, year: i32, month: u32) -> Result<DiaryMonth> {
    if !(1970..=2100).contains(&year) {
        return Err(KoraError::InvalidInput("año fuera de rango".into()));
    }
    if !(1..=12).contains(&month) {
        return Err(KoraError::InvalidInput("mes fuera de rango".into()));
    }

    let (from, to) = month_bounds(year, month);
    let entries = state
        .storage
        .diary
        .list_range(&state.identity.account.id, &from, &to)
        .await?;
    Ok(aggregate_month(year, month, entries))
}

#[tauri::command]
pub async fn diary_create(
    state: State<'_, AppState>,
    app: AppHandle,
    entry_date: String,
    body: String,
    mood: Option<String>,
) -> Result<DiaryEntry> {
    let (entry_date, body, mood) = validate_manual_entry(&entry_date, &body, mood)?;
    let owner = state.identity.account.id.clone();

    let entry = state
        .storage
        .diary
        .create(NewDiaryEntry {
            book_id: None,
            entry_date,
            body,
            mood,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await?;

    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(owner),
            action: "diary.create".to_string(),
            resource_type: Some(ResourceType::DiaryEntry),
            resource_id: Some(entry.id.clone()),
            metadata: serde_json::json!({ "entryDate": entry.entry_date }).to_string(),
        })
        .await?;

    let _ = app.emit("diary::updated", serde_json::json!({ "source": "manual" }));
    Ok(entry)
}

#[tauri::command]
pub async fn diary_update(
    state: State<'_, AppState>,
    app: AppHandle,
    entry_id: String,
    body: String,
    mood: Option<String>,
) -> Result<DiaryEntry> {
    let owner = state.identity.account.id.clone();
    let mut entry = owned_entry(&state, &entry_id, &owner).await?;
    if entry.kind != DiaryKind::Manual {
        return Err(KoraError::InvalidInput(
            "solo las notas manuales se pueden editar".into(),
        ));
    }

    let (_, body, mood) = validate_manual_entry(&entry.entry_date, &body, mood)?;
    entry.body = body;
    entry.mood = mood;
    state.storage.diary.update(&entry).await?;

    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(owner),
            action: "diary.update".to_string(),
            resource_type: Some(ResourceType::DiaryEntry),
            resource_id: Some(entry.id.clone()),
            metadata: serde_json::json!({ "entryDate": entry.entry_date }).to_string(),
        })
        .await?;

    let _ = app.emit("diary::updated", serde_json::json!({ "source": "manual" }));
    Ok(entry)
}

#[tauri::command]
pub async fn diary_delete(
    state: State<'_, AppState>,
    app: AppHandle,
    entry_id: String,
) -> Result<()> {
    let owner = state.identity.account.id.clone();
    let entry = owned_entry(&state, &entry_id, &owner).await?;
    state.storage.diary.delete(&entry_id).await?;

    state
        .storage
        .audit
        .append(NewAuditEntry {
            actor_account_id: Some(owner),
            action: "diary.delete".to_string(),
            resource_type: Some(ResourceType::DiaryEntry),
            resource_id: Some(entry_id),
            metadata: serde_json::json!({
                "entryDate": entry.entry_date,
                "kind": entry.kind,
            })
            .to_string(),
        })
        .await?;

    let _ = app.emit("diary::updated", serde_json::json!({ "source": "manual" }));
    Ok(())
}

fn validate_manual_entry(
    entry_date: &str,
    body: &str,
    mood: Option<String>,
) -> Result<(String, String, Option<String>)> {
    NaiveDate::parse_from_str(entry_date, "%Y-%m-%d")
        .map_err(|_| KoraError::InvalidInput("la fecha del diario debe usar AAAA-MM-DD".into()))?;

    let body = body.trim().to_string();
    if body.is_empty() {
        return Err(KoraError::InvalidInput(
            "la entrada del diario no puede estar vacía".into(),
        ));
    }
    if body.chars().count() > 20_000 {
        return Err(KoraError::InvalidInput(
            "la entrada del diario no puede superar 20000 caracteres".into(),
        ));
    }

    let mood = mood
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if mood
        .as_ref()
        .is_some_and(|value| value.chars().count() > 40)
    {
        return Err(KoraError::InvalidInput(
            "el estado de ánimo no puede superar 40 caracteres".into(),
        ));
    }

    Ok((entry_date.to_string(), body, mood))
}

async fn owned_entry(state: &AppState, entry_id: &str, owner: &str) -> Result<DiaryEntry> {
    let entry = state
        .storage
        .diary
        .get(entry_id)
        .await?
        .ok_or_else(|| KoraError::NotFound(format!("entrada de diario {entry_id}")))?;
    if entry.owner_account_id != owner {
        return Err(KoraError::NotFound(format!("entrada de diario {entry_id}")));
    }
    Ok(entry)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn export_diary(
    state: State<'_, AppState>,
    app: AppHandle,
    year: i32,
    month: Option<u32>,
    include_audiobook: bool,
    voice_id: Option<String>,
) -> Result<DiaryExport> {
    diary_export::export(
        state.inner(),
        &app,
        year,
        month,
        include_audiobook,
        voice_id,
    )
    .await
}

#[tauri::command]
pub async fn get_achievements(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Vec<Achievement>> {
    let owner = state.identity.account.id.clone();
    for achievement in state.achievements.evaluate(&owner).await? {
        let _ = app.emit("achievement::unlocked", achievement);
    }
    state.achievements.list(&owner).await
}

#[tauri::command]
pub async fn session_start(
    state: State<'_, AppState>,
    book_id: String,
    kind: SessionKind,
    section: Option<i64>,
    percent: Option<f64>,
) -> Result<SessionStatus> {
    owned_book(state.inner(), &book_id).await?;
    state.sessions.start(&book_id, kind, section, percent)
}

#[tauri::command]
pub async fn session_end(
    state: State<'_, AppState>,
    app: AppHandle,
    session_id: String,
    pages_read: Option<i64>,
) -> Result<Option<DiaryEntry>> {
    let finished = state.sessions.finish(&session_id)?;
    if finished.duration_minutes <= 0 {
        return Ok(None);
    }

    let book = state.storage.books.get(&finished.book_id).await?;
    let title = book
        .as_ref()
        .map(|book| book.title.clone())
        .unwrap_or_else(|| "Libro desconocido".to_string());
    let origin_share_id = book.as_ref().and_then(|book| book.origin_share_id.clone());
    let owner = state.identity.account.id.clone();

    let pages_read = pages_read
        .unwrap_or_else(|| match (finished.start_section, finished.end_section) {
            (Some(start), Some(end)) => (end - start).max(0),
            _ => 0,
        })
        .max(0);

    let (extracts_created, notes_created) = session_counts(
        &state,
        &owner,
        &finished.book_id,
        &finished.started_at.to_rfc3339(),
        &finished.ended_at.to_rfc3339(),
    )
    .await?;

    let kind = match finished.kind {
        SessionKind::Reading => crate::storage::DiaryKind::Reading,
        SessionKind::Listening => crate::storage::DiaryKind::Listening,
    };
    let label = match finished.kind {
        SessionKind::Reading => "lectura",
        SessionKind::Listening => "escucha",
    };
    let body = format!(
        "Sesión de {label} · {} min · {pages_read} páginas\n«{title}»",
        finished.duration_minutes
    );

    let entry = state
        .storage
        .diary
        .create_session(NewDiarySession {
            book_id: Some(finished.book_id.clone()),
            entry_date: finished
                .ended_at
                .with_timezone(&Local)
                .format("%Y-%m-%d")
                .to_string(),
            body,
            kind,
            session_start: finished.started_at,
            session_end: finished.ended_at,
            duration_minutes: finished.duration_minutes,
            pages_read,
            extracts_created,
            notes_created,
            pomodoro_round: None,
            owner_account_id: owner.clone(),
            origin_share_id,
        })
        .await?;

    let _ = app.emit("diary::updated", serde_json::json!({ "source": "session" }));
    for achievement in state.achievements.evaluate(&owner).await? {
        let _ = app.emit("achievement::unlocked", achievement);
    }

    Ok(Some(entry))
}

async fn session_counts(
    state: &AppState,
    owner: &str,
    book_id: &str,
    from: &str,
    to: &str,
) -> Result<(i64, i64)> {
    let extracts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM extracts
         WHERE owner_account_id = ? AND book_id = ?
           AND datetime(created_at) >= datetime(?) AND datetime(created_at) <= datetime(?)",
    )
    .bind(owner)
    .bind(book_id)
    .bind(from)
    .bind(to)
    .fetch_one(state.storage.pool())
    .await?;

    let notes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notes
         WHERE owner_account_id = ? AND book_id = ?
           AND datetime(created_at) >= datetime(?) AND datetime(created_at) <= datetime(?)",
    )
    .bind(owner)
    .bind(book_id)
    .bind(from)
    .bind(to)
    .fetch_one(state.storage.pool())
    .await?;

    let voice_notes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM voice_notes
         WHERE owner_account_id = ? AND book_id = ?
           AND datetime(created_at) >= datetime(?) AND datetime(created_at) <= datetime(?)",
    )
    .bind(owner)
    .bind(book_id)
    .bind(from)
    .bind(to)
    .fetch_one(state.storage.pool())
    .await?;

    Ok((extracts, notes + voice_notes))
}

async fn owned_book(state: &AppState, book_id: &str) -> Result<crate::storage::Book> {
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
