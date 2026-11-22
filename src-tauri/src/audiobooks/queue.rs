use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use crate::audiobooks::cast::CastInput;
use crate::audiobooks::engine::generate;
use crate::audiobooks::jobs::AudiobookJobManager;
use crate::audiobooks::narration::NarrationSettings;
use crate::audiobooks::scope::AudiobookScope;
use crate::audiobooks::{AudiobookProgressEvent, emit_progress};
use crate::state::AppState;
use crate::storage::AudiobookOrigin;

/// Avisa a la UI de que la cola cambió. El payload no lleva datos: la vista
/// vuelve a pedir el estado completo para no duplicar la consulta.
pub fn notify_queue(app: &AppHandle) {
    let _ = app.emit("audiobook::queue", ());
}

/// Arranca el procesador de la cola si no hay uno en marcha. Es idempotente:
/// varias llamadas seguidas reutilizan el mismo worker.
pub fn start_worker(app: AppHandle) {
    let manager = {
        let state = app.state::<AppState>();
        state.audiobook_jobs.clone()
    };
    if !manager.try_start_runner() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        {
            let _guard = RunnerGuard(manager);
            process_queue(&app).await;
        }
        // Cierra la carrera entre «la cola está vacía» y un enqueue que llega
        // justo antes de liberar el flag del worker.
        if crate::audiobooks::m4b::ensure_available().is_ok() && queue_has_items(&app).await {
            start_worker(app);
        }
    });
}

async fn queue_has_items(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let owner = state.identity.account.id.clone();
    matches!(
        state.storage.audiobooks.next_queue_item(&owner).await,
        Ok(Some(_))
    )
}

struct RunnerGuard(Arc<AudiobookJobManager>);

impl Drop for RunnerGuard {
    fn drop(&mut self) {
        self.0.finish();
        self.0.finish_runner();
    }
}

async fn process_queue(app: &AppHandle) {
    if let Err(error) = crate::audiobooks::m4b::ensure_available() {
        tracing::warn!(%error, "ffmpeg no disponible; la cola de audiolibros queda en espera");
        return;
    }

    loop {
        let state = app.state::<AppState>();
        let guard = state.audiobook_jobs.run_lock.lock().await;

        let owner = state.identity.account.id.clone();
        let next = match state.storage.audiobooks.next_queue_item(&owner).await {
            Ok(Some(item)) => item,
            Ok(None) => break,
            Err(error) => {
                tracing::warn!(%error, "no se pudo leer la cola de audiolibros");
                break;
            }
        };

        if state
            .storage
            .audiobooks
            .claim_queue_item(&next.id)
            .await
            .is_err()
        {
            continue;
        }
        notify_queue(app);

        let scope: AudiobookScope =
            serde_json::from_str(&next.scope).unwrap_or(AudiobookScope::FullBook);
        let narration: NarrationSettings = next
            .narration
            .as_deref()
            .and_then(|value| serde_json::from_str::<NarrationSettings>(value).ok())
            .unwrap_or_default()
            .clamped();
        let cast: CastInput = next
            .cast
            .as_deref()
            .and_then(|value| serde_json::from_str::<CastInput>(value).ok())
            .unwrap_or_default()
            .normalized();
        let narration = NarrationSettings {
            multi_voice: cast.enabled,
            ..narration
        };
        let origin = match next.origin {
            AudiobookOrigin::Tejido => AudiobookOrigin::Tejido,
            _ => AudiobookOrigin::Generated,
        };
        let title = state
            .storage
            .books
            .get(&next.book_id)
            .await
            .ok()
            .flatten()
            .map(|book| book.title)
            .unwrap_or_else(|| "Audiolibro".to_string());

        let voice_name = state.tts.voices().await.ok().and_then(|voices| {
            voices
                .into_iter()
                .find(|voice| voice.id == next.voice_id)
                .map(|voice| voice.name)
        });
        let control = state.audiobook_jobs.begin(&title, voice_name.as_deref());
        notify_queue(app);
        let result = generate(
            &state,
            app,
            &next.book_id,
            scope,
            &next.voice_id,
            origin,
            narration,
            cast,
            Some(control.as_ref()),
        )
        .await;

        if let Err(error) = result {
            tracing::warn!(%error, "la generación en cola falló");
            if control.audiobook_id().is_none() {
                emit_progress(
                    app,
                    AudiobookProgressEvent {
                        audiobook_id: None,
                        phase: "error".to_string(),
                        chapter: 0,
                        total_chapters: 0,
                        percent: 0.0,
                        message: format!("No se pudo generar «{title}»: {error:#}"),
                    },
                );
            }
        }
        let _ = state
            .storage
            .audiobooks
            .remove_queue_item(&next.id, &owner)
            .await;
        state.audiobook_jobs.finish();
        drop(guard);
        notify_queue(app);
    }
}
