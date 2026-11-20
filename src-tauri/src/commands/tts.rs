use tauri::State;

use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::tts::{EngineInfo, SynthesisRequest, VoiceProfile};

#[tauri::command]
pub fn tts_engines(state: State<'_, AppState>) -> Result<Vec<EngineInfo>> {
    Ok(state.tts.engines())
}

#[tauri::command]
pub async fn tts_voices(state: State<'_, AppState>) -> Result<Vec<VoiceProfile>> {
    // Los perfiles se leerán de assets/voices en la fase de TTS.
    let _ = &state;
    Ok(Vec::new())
}

#[tauri::command]
pub async fn tts_synthesize(state: State<'_, AppState>, request: SynthesisRequest) -> Result<()> {
    let _ = (state, request);
    Err(KoraError::not_implemented("tts_synthesize (fase 5)"))
}
