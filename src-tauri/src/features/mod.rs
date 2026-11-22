pub mod achievements;
pub mod diary_export;
pub mod embeddings;
pub mod journal;
pub mod notes_export;
pub mod ocr;
pub mod pomodoro;
pub mod semantic;
pub mod sessions;

pub use achievements::AchievementsService;
pub use embeddings::{
    EMBEDDING_DIMENSIONS, EMBEDDING_MODEL_ID, EmbeddingProvider, platform_provider,
};
pub use journal::{DiaryDay, DiaryMonth, DiaryTotals, aggregate_month, month_bounds, today};
#[cfg(not(target_os = "android"))]
pub use ocr::TesseractOcrEngine;
pub use ocr::{OcrEngine, OcrResult, OcrWord, PlatformOcrEngine};
pub use pomodoro::{PomodoroConfig, PomodoroManager, PomodoroState, PomodoroStatus};
pub use semantic::{SemanticHit, SemanticProgress, SemanticService, SemanticStatus};
pub use sessions::{FinishedSession, SessionKind, SessionStatus, SessionTracker};

use serde::{Deserialize, Serialize};

/// Interruptores de funcionalidades opcionales. Todos arrancan apagados
/// hasta que su fase correspondiente los active.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeaturesStatus {
    pub ocr: bool,
    pub embeddings: bool,
    pub journal: bool,
    pub achievements: bool,
}
