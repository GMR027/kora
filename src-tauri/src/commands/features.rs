use crate::error::Result;
use crate::features::FeaturesStatus;

#[tauri::command]
pub fn features_status() -> Result<FeaturesStatus> {
    Ok(FeaturesStatus::default())
}
