use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::error::{KoraError, Result};

const PAUSE_POLL_MS: u64 = 120;

/// Control cooperativo de una generación. La síntesis comprueba este control
/// antes de cada fragmento, así que pausar o cancelar no espera a que termine
/// el capítulo completo.
#[derive(Debug, Default)]
pub struct GenerationControl {
    paused: AtomicBool,
    cancelled: AtomicBool,
    audiobook_id: Mutex<Option<String>>,
}

impl GenerationControl {
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Vincula el id de la fila `audiobooks` en cuanto se crea, para que la UI
    /// pueda mostrarla aunque la generación venga de la cola persistente.
    pub fn bind_audiobook(&self, id: &str) {
        if let Ok(mut slot) = self.audiobook_id.lock() {
            *slot = Some(id.to_string());
        }
    }

    pub fn audiobook_id(&self) -> Option<String> {
        self.audiobook_id.lock().ok().and_then(|slot| slot.clone())
    }

    /// Punto de control que se llama entre fragmentos. Si está pausado espera
    /// sin consumir CPU; si se cancela, devuelve un error específico.
    pub async fn checkpoint(&self) -> Result<()> {
        loop {
            if self.cancelled() {
                return Err(KoraError::Cancelled(
                    "la generación del audiolibro se canceló".into(),
                ));
            }
            if !self.paused() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(PAUSE_POLL_MS)).await;
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveGenerationStatus {
    pub title: String,
    pub voice_name: Option<String>,
    pub audiobook_id: Option<String>,
    pub paused: bool,
    pub started_at: DateTime<Utc>,
}

struct ActiveGeneration {
    title: String,
    voice_name: Option<String>,
    control: Arc<GenerationControl>,
    started_at: DateTime<Utc>,
}

/// Coordina la generación activa y evita que dos trabajos de la cola corran a
/// la vez. La cola en sí vive en SQLite; aquí solo se guarda el trabajo actual.
#[derive(Default)]
pub struct AudiobookJobManager {
    active: Mutex<Option<ActiveGeneration>>,
    runner: AtomicBool,
    /// Serializa la generación directa y la cola. La UI usa la cola, pero el
    /// comando directo sigue disponible para integraciones y pruebas.
    pub run_lock: tokio::sync::Mutex<()>,
}

impl AudiobookJobManager {
    pub fn try_start_runner(&self) -> bool {
        !self.runner.swap(true, Ordering::SeqCst)
    }

    pub fn finish_runner(&self) {
        self.runner.store(false, Ordering::SeqCst);
    }

    pub fn begin(&self, title: &str, voice_name: Option<&str>) -> Arc<GenerationControl> {
        let control = Arc::new(GenerationControl::default());
        if let Ok(mut active) = self.active.lock() {
            *active = Some(ActiveGeneration {
                title: title.to_string(),
                voice_name: voice_name.map(str::to_string),
                control: control.clone(),
                started_at: Utc::now(),
            });
        }
        control
    }

    pub fn finish(&self) {
        if let Ok(mut active) = self.active.lock() {
            *active = None;
        }
    }

    pub fn active_control(&self) -> Option<Arc<GenerationControl>> {
        self.active
            .lock()
            .ok()
            .and_then(|active| active.as_ref().map(|job| job.control.clone()))
    }

    pub fn status(&self) -> Option<ActiveGenerationStatus> {
        self.active.lock().ok().and_then(|active| {
            active.as_ref().map(|job| ActiveGenerationStatus {
                title: job.title.clone(),
                voice_name: job.voice_name.clone(),
                audiobook_id: job.control.audiobook_id(),
                paused: job.control.paused(),
                started_at: job.started_at,
            })
        })
    }

    pub fn pause(&self) -> Result<()> {
        let control = self
            .active_control()
            .ok_or_else(|| KoraError::NotFound("no hay una generación activa".into()))?;
        control.pause();
        Ok(())
    }

    pub fn resume(&self) -> Result<()> {
        let control = self
            .active_control()
            .ok_or_else(|| KoraError::NotFound("no hay una generación activa".into()))?;
        control.resume();
        Ok(())
    }

    pub fn cancel(&self) -> Result<()> {
        let control = self
            .active_control()
            .ok_or_else(|| KoraError::NotFound("no hay una generación activa".into()))?;
        control.cancel();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn checkpoint_blocks_while_paused_and_releases_on_resume() {
        let control = GenerationControl::default();
        control.pause();
        let probe = Arc::new(control);
        let waiter = probe.clone();

        let mut task = tokio::spawn(async move { waiter.checkpoint().await });
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!task.is_finished());

        probe.resume();
        let outcome = tokio::time::timeout(Duration::from_secs(2), &mut task)
            .await
            .expect("checkpoint no se liberó")
            .expect("tarea");
        assert!(outcome.is_ok());
    }

    #[tokio::test]
    async fn cancel_unblocks_paused_generations() {
        let control = Arc::new(GenerationControl::default());
        control.pause();
        let waiter = control.clone();
        let task = tokio::spawn(async move { waiter.checkpoint().await });

        tokio::time::sleep(Duration::from_millis(150)).await;
        control.cancel();

        let outcome = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("checkpoint no se liberó")
            .expect("tarea");
        assert!(matches!(outcome, Err(KoraError::Cancelled(_))));
    }

    #[test]
    fn manager_tracks_the_active_generation() {
        let manager = AudiobookJobManager::default();
        assert!(manager.status().is_none());

        let control = manager.begin("Libro", Some("Clara"));
        control.bind_audiobook("audiobook-1");
        let status = manager.status().expect("activo");
        assert_eq!(status.title, "Libro");
        assert_eq!(status.voice_name.as_deref(), Some("Clara"));
        assert_eq!(status.audiobook_id.as_deref(), Some("audiobook-1"));
        assert!(!status.paused);

        manager.pause().expect("pausa");
        assert!(manager.status().expect("activo").paused);
        manager.resume().expect("reanudar");
        assert!(!manager.status().expect("activo").paused);

        manager.finish();
        assert!(manager.status().is_none());
    }

    #[test]
    fn runner_flag_allows_only_one_worker() {
        let manager = AudiobookJobManager::default();
        assert!(manager.try_start_runner());
        assert!(!manager.try_start_runner());
        manager.finish_runner();
        assert!(manager.try_start_runner());
    }
}
