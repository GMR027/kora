use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

pub const MIRROR_CHANNEL_CAPACITY: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorEvent {
    Position {
        book_id: String,
        chapter: usize,
        offset: usize,
    },
    Tts {
        book_id: String,
        section: usize,
        state: String,
        offset: usize,
        /// Fin exclusivo cuando el anfitrión reproduce un extracto.
        end: Option<usize>,
    },
}

#[derive(Clone)]
pub struct MirrorHub {
    sender: broadcast::Sender<MirrorEvent>,
}

impl MirrorHub {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(MIRROR_CHANNEL_CAPACITY);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<MirrorEvent> {
        self.sender.subscribe()
    }

    pub fn position(&self, book_id: &str, chapter: usize, offset: usize) {
        let _ = self.sender.send(MirrorEvent::Position {
            book_id: book_id.to_string(),
            chapter,
            offset,
        });
    }

    pub fn tts(
        &self,
        book_id: &str,
        section: usize,
        state: &str,
        offset: usize,
        end: Option<usize>,
    ) {
        let _ = self.sender.send(MirrorEvent::Tts {
            book_id: book_id.to_string(),
            section,
            state: state.to_string(),
            offset,
            end,
        });
    }
}

impl Default for MirrorHub {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorStatus {
    pub active: bool,
    pub hosting: bool,
    pub book_id: Option<String>,
    pub host_device_id: Option<String>,
    pub followers: Vec<String>,
}
