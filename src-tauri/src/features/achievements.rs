use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{KoraError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    pub id: String,
    pub title: String,
    pub description: String,
    pub progress: f32,
    pub unlocked_at: Option<DateTime<Utc>>,
}

pub trait AchievementService: Send + Sync {
    fn list(&self) -> Result<Vec<Achievement>>;
    fn record(&self, event: &str) -> Result<()>;
}

pub struct LocalAchievementService;

impl AchievementService for LocalAchievementService {
    fn list(&self) -> Result<Vec<Achievement>> {
        Ok(Vec::new())
    }

    fn record(&self, _event: &str) -> Result<()> {
        Err(KoraError::not_implemented(
            "features::LocalAchievementService::record (fase 7)",
        ))
    }
}
