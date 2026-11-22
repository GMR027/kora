use tauri::{AppHandle, State};

use crate::error::Result;
use crate::state::AppState;
use crate::tejido::WeaveResult;

#[tauri::command]
pub async fn weave_extracts(
    state: State<'_, AppState>,
    app: AppHandle,
    extract_ids: Vec<String>,
    title: String,
    voice_id: String,
) -> Result<WeaveResult> {
    #[cfg(target_os = "android")]
    {
        let _ = (&state, &app, &extract_ids, &title, &voice_id);
        return Err(crate::error::KoraError::Unsupported(
            "Tejido aún no está disponible en Android".into(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        let _guard = state.audiobook_jobs.run_lock.lock().await;
        let voice_name = state.tts.voices().await.ok().and_then(|voices| {
            voices
                .into_iter()
                .find(|voice| voice.id == voice_id)
                .map(|voice| voice.name)
        });
        let control = state.audiobook_jobs.begin(&title, voice_name.as_deref());
        crate::audiobooks::queue::notify_queue(&app);
        let result = crate::tejido::weave(
            &state,
            &app,
            &extract_ids,
            &title,
            &voice_id,
            Some(control.as_ref()),
        )
        .await;
        state.audiobook_jobs.finish();
        crate::audiobooks::queue::notify_queue(&app);
        result
    }
}
