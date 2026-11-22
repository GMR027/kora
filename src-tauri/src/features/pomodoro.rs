use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use crate::error::{KoraError, Result};
use crate::features::achievements::AchievementsService;
use crate::features::journal::today;
use crate::storage::{DiaryKind, NewDiarySession, Storage};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroConfig {
    pub focus_minutes: u32,
    pub break_minutes: u32,
    pub long_break_minutes: u32,
    pub rounds_before_long_break: u32,
    pub auto_start_breaks: bool,
    pub auto_start_focus: bool,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            break_minutes: 5,
            long_break_minutes: 15,
            rounds_before_long_break: 4,
            auto_start_breaks: true,
            auto_start_focus: false,
        }
    }
}

impl PomodoroConfig {
    fn validate(&self) -> Result<()> {
        if !(1..=180).contains(&self.focus_minutes) {
            return Err(KoraError::InvalidInput(
                "el foco debe durar entre 1 y 180 minutos".into(),
            ));
        }
        if !(1..=60).contains(&self.break_minutes) {
            return Err(KoraError::InvalidInput(
                "el descanso debe durar entre 1 y 60 minutos".into(),
            ));
        }
        if !(1..=120).contains(&self.long_break_minutes) {
            return Err(KoraError::InvalidInput(
                "el descanso largo debe durar entre 1 y 120 minutos".into(),
            ));
        }
        if !(2..=12).contains(&self.rounds_before_long_break) {
            return Err(KoraError::InvalidInput(
                "las rondas antes del descanso largo deben estar entre 2 y 12".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PomodoroState {
    Idle,
    Focus,
    Break,
    LongBreak,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroStatus {
    pub state: PomodoroState,
    pub ends_at: Option<DateTime<Utc>>,
    pub remaining_seconds: i64,
    pub round: u32,
    pub completed_focus: u32,
    pub config: PomodoroConfig,
}

struct Inner {
    state: PomodoroState,
    started_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
    round: u32,
    completed_focus: u32,
    config: PomodoroConfig,
}

pub struct PomodoroManager {
    storage: Storage,
    achievements: Arc<AchievementsService>,
    owner_account_id: String,
    inner: Arc<Mutex<Inner>>,
    generation: Arc<AtomicU64>,
}

impl PomodoroManager {
    pub fn new(
        storage: Storage,
        achievements: Arc<AchievementsService>,
        owner_account_id: String,
    ) -> Self {
        Self {
            storage,
            achievements,
            owner_account_id,
            inner: Arc::new(Mutex::new(Inner {
                state: PomodoroState::Idle,
                started_at: None,
                ends_at: None,
                round: 0,
                completed_focus: 0,
                config: PomodoroConfig::default(),
            })),
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    pub async fn load_config(&self) -> Result<()> {
        if let Some(raw) = self.storage.settings.get("pomodoro.config").await?
            && let Ok(config) = serde_json::from_str::<PomodoroConfig>(&raw)
            && config.validate().is_ok()
        {
            self.inner.lock().await.config = config;
        }

        let completed: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM diary_entries
             WHERE owner_account_id = ? AND kind = 'pomodoro'",
        )
        .bind(&self.owner_account_id)
        .fetch_one(self.storage.pool())
        .await?;
        let mut inner = self.inner.lock().await;
        inner.completed_focus = completed.max(0) as u32;
        inner.round = (inner.completed_focus % inner.config.rounds_before_long_break) + 1;
        Ok(())
    }

    pub async fn status(&self) -> PomodoroStatus {
        let inner = self.inner.lock().await;
        status_of(&inner)
    }

    pub async fn start(
        self: &Arc<Self>,
        app: Option<&AppHandle>,
        config: Option<PomodoroConfig>,
    ) -> Result<PomodoroStatus> {
        let config = match config {
            Some(config) => config,
            None => self.inner.lock().await.config.clone(),
        };
        config.validate()?;
        self.storage
            .settings
            .set("pomodoro.config", &serde_json::to_string(&config)?)
            .await?;

        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let now = Utc::now();
        let status = {
            let mut inner = self.inner.lock().await;
            inner.config = config.clone();
            inner.state = PomodoroState::Focus;
            inner.started_at = Some(now);
            inner.ends_at = Some(now + Duration::minutes(config.focus_minutes as i64));
            inner.round = inner.round.max(1);
            status_of(&inner)
        };
        emit_status(app, &status);

        let manager = self.clone();
        let app = app.cloned();
        tokio::spawn(async move {
            manager.run_loop(app, generation).await;
        });

        Ok(status)
    }

    pub async fn stop(&self, app: Option<&AppHandle>) -> Result<PomodoroStatus> {
        self.generation.fetch_add(1, Ordering::AcqRel);
        let status = {
            let mut inner = self.inner.lock().await;
            inner.state = PomodoroState::Idle;
            inner.started_at = None;
            inner.ends_at = None;
            status_of(&inner)
        };
        emit_status(app, &status);
        Ok(status)
    }

    /// Bucle único de fases. Evita recursión asíncrona y mantiene un solo
    /// temporizador vivo por Pomodoro.
    async fn run_loop(self: Arc<Self>, app: Option<AppHandle>, generation: u64) {
        loop {
            let (state, ends_at) = {
                let inner = self.inner.lock().await;
                (inner.state, inner.ends_at)
            };
            let Some(ends_at) = ends_at else {
                return;
            };
            sleep_until(ends_at).await;

            {
                let inner = self.inner.lock().await;
                if self.generation.load(Ordering::Acquire) != generation || inner.state != state {
                    return;
                }
            }

            match state {
                PomodoroState::Focus => {
                    if let Err(error) = self.complete_focus(app.as_ref(), generation).await {
                        tracing::warn!(%error, "no se pudo completar el Pomodoro");
                        return;
                    }
                }
                PomodoroState::Break | PomodoroState::LongBreak => {
                    self.complete_break(app.as_ref(), generation).await;
                }
                PomodoroState::Idle => return,
            }

            let next = {
                let inner = self.inner.lock().await;
                inner.state
            };
            if next == PomodoroState::Idle {
                return;
            }
        }
    }

    async fn complete_focus(&self, app: Option<&AppHandle>, generation: u64) -> Result<()> {
        let (config, started_at, round) = {
            let mut inner = self.inner.lock().await;
            if self.generation.load(Ordering::Acquire) != generation
                || inner.state != PomodoroState::Focus
            {
                return Ok(());
            }
            let started_at = inner.started_at.unwrap_or_else(Utc::now);
            inner.completed_focus += 1;
            let completed = inner.completed_focus;
            let round = inner.round.max(1);
            let long = completed.is_multiple_of(inner.config.rounds_before_long_break);
            inner.round += 1;
            inner.started_at = None;
            inner.ends_at = None;
            let config = inner.config.clone();
            if config.auto_start_breaks {
                let break_minutes = if long {
                    config.long_break_minutes
                } else {
                    config.break_minutes
                };
                inner.state = if long {
                    PomodoroState::LongBreak
                } else {
                    PomodoroState::Break
                };
                inner.ends_at = Some(Utc::now() + Duration::minutes(break_minutes as i64));
                inner.started_at = Some(Utc::now());
            } else {
                inner.state = PomodoroState::Idle;
            }
            (config, started_at, round)
        };

        let ended_at = started_at + Duration::minutes(config.focus_minutes as i64);
        let body = format!(
            "Pomodoro completado · {} min\nRonda {}",
            config.focus_minutes, round
        );
        self.storage
            .diary
            .create_session(NewDiarySession {
                book_id: None,
                entry_date: today(),
                body,
                kind: DiaryKind::Pomodoro,
                session_start: started_at,
                session_end: ended_at,
                duration_minutes: config.focus_minutes as i64,
                pages_read: 0,
                extracts_created: 0,
                notes_created: 0,
                pomodoro_round: Some(round as i64),
                owner_account_id: self.owner_account_id.clone(),
                origin_share_id: None,
            })
            .await?;

        if let Some(app) = app {
            let _ = app.emit(
                "diary::updated",
                serde_json::json!({ "source": "pomodoro" }),
            );
            for achievement in self.achievements.evaluate(&self.owner_account_id).await? {
                let _ = app.emit("achievement::unlocked", achievement);
            }
        }

        let status = self.status().await;
        emit_status(app, &status);
        Ok(())
    }

    async fn complete_break(&self, app: Option<&AppHandle>, generation: u64) {
        let status = {
            let mut inner = self.inner.lock().await;
            if self.generation.load(Ordering::Acquire) != generation
                || !matches!(inner.state, PomodoroState::Break | PomodoroState::LongBreak)
            {
                return;
            }

            if inner.config.auto_start_focus {
                let now = Utc::now();
                inner.state = PomodoroState::Focus;
                inner.started_at = Some(now);
                inner.ends_at = Some(now + Duration::minutes(inner.config.focus_minutes as i64));
                inner.round = inner.round.max(1);
            } else {
                inner.state = PomodoroState::Idle;
                inner.started_at = None;
                inner.ends_at = None;
            }
            status_of(&inner)
        };

        emit_status(app, &status);
    }

    #[cfg(test)]
    pub(crate) async fn force_complete_focus_for_test(&self) -> Result<()> {
        let generation = self.generation.load(Ordering::Acquire);
        self.complete_focus(None, generation).await
    }
}

fn status_of(inner: &Inner) -> PomodoroStatus {
    let remaining_seconds = inner
        .ends_at
        .map(|ends_at| (ends_at - Utc::now()).num_seconds().max(0))
        .unwrap_or(0);
    PomodoroStatus {
        state: inner.state,
        ends_at: inner.ends_at,
        remaining_seconds,
        round: inner.round,
        completed_focus: inner.completed_focus,
        config: inner.config.clone(),
    }
}

fn emit_status(app: Option<&AppHandle>, status: &PomodoroStatus) {
    if let Some(app) = app {
        let _ = app.emit("pomodoro::state", status);
    }
}

async fn sleep_until(ends_at: DateTime<Utc>) {
    let duration = (ends_at - Utc::now()).to_std().unwrap_or_default();
    tokio::time::sleep(duration).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::DiaryKind;
    use crate::storage::test_support::TestStorage;

    #[tokio::test]
    async fn completed_focus_writes_a_diary_entry() -> Result<()> {
        let (test, identity) = TestStorage::with_identity().await;
        let achievements = Arc::new(AchievementsService::new(test.storage.clone()));
        let manager = Arc::new(PomodoroManager::new(
            test.storage.clone(),
            achievements,
            identity.account.id.clone(),
        ));

        manager
            .start(
                None,
                Some(PomodoroConfig {
                    focus_minutes: 1,
                    ..PomodoroConfig::default()
                }),
            )
            .await?;
        manager.force_complete_focus_for_test().await?;

        let entries = test
            .storage
            .diary
            .list_by_owner(&identity.account.id)
            .await?;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, DiaryKind::Pomodoro);
        assert_eq!(entries[0].duration_minutes, 1);
        assert_eq!(entries[0].pomodoro_round, Some(1));

        manager.stop(None).await?;
        Ok(())
    }

    #[test]
    fn config_defaults_match_kora() {
        let config = PomodoroConfig::default();
        assert_eq!(config.focus_minutes, 25);
        assert_eq!(config.break_minutes, 5);
        assert_eq!(config.long_break_minutes, 15);
        assert_eq!(config.rounds_before_long_break, 4);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn invalid_config_is_rejected() {
        let config = PomodoroConfig {
            focus_minutes: 0,
            ..PomodoroConfig::default()
        };
        assert!(config.validate().is_err());
    }
}
