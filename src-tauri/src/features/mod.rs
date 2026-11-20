pub mod achievements;
pub mod embeddings;
pub mod journal;
pub mod ocr;

pub use achievements::{Achievement, AchievementService, LocalAchievementService};
pub use embeddings::{EmbeddingProvider, OnnxEmbeddingProvider};
pub use journal::{JournalEntry, JournalService, SqliteJournalService};
pub use ocr::{OcrEngine, OcrResult, PlatformOcrEngine};

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
