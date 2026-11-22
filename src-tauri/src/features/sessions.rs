use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{KoraError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Reading,
    Listening,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub id: String,
    pub book_id: String,
    pub kind: SessionKind,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct FinishedSession {
    pub id: String,
    pub book_id: String,
    pub kind: SessionKind,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub duration_minutes: i64,
    pub start_section: Option<i64>,
    pub end_section: Option<i64>,
    pub start_percent: f64,
    pub end_percent: f64,
}

struct ActiveSession {
    id: String,
    book_id: String,
    kind: SessionKind,
    started_at: DateTime<Utc>,
    start_section: Option<i64>,
    end_section: Option<i64>,
    start_percent: f64,
    end_percent: f64,
}

#[derive(Default)]
pub struct SessionTracker {
    active: Mutex<HashMap<String, ActiveSession>>,
}

impl SessionTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(
        &self,
        book_id: &str,
        kind: SessionKind,
        section: Option<i64>,
        percent: Option<f64>,
    ) -> Result<SessionStatus> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| KoraError::Internal("sesiones bloqueadas".into()))?;

        if let Some(session) = active
            .values()
            .find(|session| session.book_id == book_id && session.kind == kind)
        {
            return Ok(SessionStatus {
                id: session.id.clone(),
                book_id: session.book_id.clone(),
                kind: session.kind,
                started_at: session.started_at,
            });
        }

        let session = ActiveSession {
            id: Uuid::now_v7().to_string(),
            book_id: book_id.to_string(),
            kind,
            started_at: Utc::now(),
            start_section: section,
            end_section: section,
            start_percent: percent.unwrap_or(0.0).clamp(0.0, 1.0),
            end_percent: percent.unwrap_or(0.0).clamp(0.0, 1.0),
        };
        let status = SessionStatus {
            id: session.id.clone(),
            book_id: session.book_id.clone(),
            kind: session.kind,
            started_at: session.started_at,
        };
        active.insert(status.id.clone(), session);
        Ok(status)
    }

    pub fn track_progress(&self, book_id: &str, section: i64, percent: f64) {
        let Ok(mut active) = self.active.lock() else {
            return;
        };
        for session in active
            .values_mut()
            .filter(|session| session.book_id == book_id)
        {
            session.end_section = Some(
                session
                    .end_section
                    .map_or(section, |current| current.max(section)),
            );
            session.end_percent = session.end_percent.max(percent.clamp(0.0, 1.0));
        }
    }

    pub fn finish(&self, session_id: &str) -> Result<FinishedSession> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| KoraError::Internal("sesiones bloqueadas".into()))?;
        let session = active
            .remove(session_id)
            .ok_or_else(|| KoraError::NotFound(format!("sesión {session_id}")))?;
        let ended_at = Utc::now();
        let seconds = (ended_at - session.started_at).num_seconds().max(0);
        let duration_minutes = if seconds >= 30 {
            ((seconds + 59) / 60).max(1)
        } else {
            0
        };

        Ok(FinishedSession {
            id: session.id,
            book_id: session.book_id,
            kind: session.kind,
            started_at: session.started_at,
            ended_at,
            duration_minutes,
            start_section: session.start_section,
            end_section: session.end_section,
            start_percent: session.start_percent,
            end_percent: session.end_percent,
        })
    }

    pub fn cancel(&self, session_id: &str) -> Result<()> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| KoraError::Internal("sesiones bloqueadas".into()))?;
        active
            .remove(session_id)
            .map(|_| ())
            .ok_or_else(|| KoraError::NotFound(format!("sesión {session_id}")))
    }

    pub fn active_for_book(&self, book_id: &str) -> Option<SessionStatus> {
        let active = self.active.lock().ok()?;
        active
            .values()
            .find(|session| session.book_id == book_id)
            .map(|session| SessionStatus {
                id: session.id.clone(),
                book_id: session.book_id.clone(),
                kind: session.kind,
                started_at: session.started_at,
            })
    }

    pub fn active_count(&self) -> usize {
        self.active
            .lock()
            .map(|active| active.len())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_is_idempotent_per_book_and_kind() {
        let tracker = SessionTracker::new();
        let first = tracker
            .start("book", SessionKind::Reading, Some(1), Some(0.1))
            .expect("iniciar");
        let second = tracker
            .start("book", SessionKind::Reading, Some(1), Some(0.1))
            .expect("repetir");
        assert_eq!(first.id, second.id);
        assert_eq!(tracker.active_count(), 1);
    }

    #[test]
    fn finish_removes_the_session() {
        let tracker = SessionTracker::new();
        let status = tracker
            .start("book", SessionKind::Listening, Some(0), Some(0.0))
            .expect("iniciar");
        tracker.track_progress("book", 4, 0.5);
        let finished = tracker.finish(&status.id).expect("cerrar");
        assert_eq!(finished.book_id, "book");
        assert_eq!(finished.end_section, Some(4));
        assert_eq!(tracker.active_count(), 0);
    }
}
