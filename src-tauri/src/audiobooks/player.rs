use serde::Serialize;

use super::manifest::AudiobookManifest;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSnapshot {
    pub audiobook_id: String,
    pub title: String,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub chapter_index: usize,
    pub chapter_title: String,
    pub section: usize,
    pub offset_start: usize,
    pub offset_end: usize,
    pub playing: bool,
}

impl PlaybackSnapshot {
    pub fn from_manifest(
        audiobook_id: String,
        manifest: &AudiobookManifest,
        position_ms: i64,
        playing: bool,
    ) -> Self {
        let index = manifest.chapter_at(position_ms);
        let chapter = manifest.chapter(index);
        Self {
            audiobook_id,
            title: manifest.book_title.clone(),
            position_ms,
            duration_ms: manifest.duration_ms,
            chapter_index: index,
            chapter_title: chapter
                .map(|chapter| chapter.title.clone())
                .unwrap_or_else(|| "Audiolibro".to_string()),
            section: chapter.map(|chapter| chapter.section).unwrap_or(0),
            offset_start: chapter.map(|chapter| chapter.offset_start).unwrap_or(0),
            offset_end: chapter.map(|chapter| chapter.offset_end).unwrap_or(0),
            playing,
        }
    }
}

#[cfg(test)]
mod decode_probe {
    use std::fs::File;
    use std::io::BufReader;

    use rodio::{Decoder, Source};

    #[test]
    #[ignore]
    fn counts_decoded_audio_samples() {
        let Ok(path) = std::env::var("KORA_TEST_AUDIOBOOK") else {
            return;
        };
        let file = File::open(path).expect("abrir audio");
        let decoder = Decoder::new(BufReader::new(file)).expect("decodificar");
        let channels = decoder.channels();
        let rate = decoder.sample_rate();
        let samples = decoder.count();
        eprintln!("decoded samples={samples} channels={channels} rate={rate}");
        assert!(samples > 10_000);
    }
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use std::fs::File;
    use std::io::BufReader;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use anyhow::{Context, Result};
    use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

    use super::PlaybackSnapshot;
    use crate::audiobooks::manifest::AudiobookManifest;

    struct Output {
        _sink: Arc<MixerDeviceSink>,
        player: Arc<Player>,
    }

    struct Current {
        audiobook_id: String,
        manifest: AudiobookManifest,
    }

    /// Reproductor de M4B/MP3 con búsqueda. Comparte un único dispositivo de
    /// salida para no abrir cientos de sinks durante la sesión.
    #[derive(Default)]
    pub struct AudiobookPlayback {
        output: Mutex<Option<Output>>,
        current: Mutex<Option<Current>>,
        /// Marca un final natural para que la capa de comandos guarde el
        /// progreso una sola vez, aunque la UI sondee varias veces.
        finished: AtomicBool,
    }

    impl AudiobookPlayback {
        fn output(&self) -> Result<(Arc<MixerDeviceSink>, Arc<Player>)> {
            let mut slot = self
                .output
                .lock()
                .map_err(|_| anyhow::anyhow!("reproductor de audiolibros bloqueado"))?;
            if slot.is_none() {
                let mut sink = DeviceSinkBuilder::open_default_sink()
                    .with_context(|| "no se pudo abrir el dispositivo de audio para audiolibros")?;
                sink.log_on_drop(false);
                let player = Arc::new(Player::connect_new(sink.mixer()));
                *slot = Some(Output {
                    _sink: Arc::new(sink),
                    player,
                });
            }
            let output = slot.as_ref().expect("salida inicializada");
            Ok((output._sink.clone(), output.player.clone()))
        }

        pub fn play(
            &self,
            audiobook_id: String,
            path: &Path,
            manifest: AudiobookManifest,
            start_ms: i64,
        ) -> Result<PlaybackSnapshot> {
            let (_, player) = self.output()?;
            player.stop();
            self.finished.store(false, Ordering::SeqCst);

            let file =
                File::open(path).with_context(|| format!("no se pudo abrir {}", path.display()))?;
            let decoder = Decoder::new(BufReader::new(file))
                .with_context(|| "el audiolibro no se pudo decodificar")?;
            player.append(decoder);
            player.play();

            if start_ms > 0 {
                let target = start_ms.clamp(0, manifest.duration_ms.max(0)) as u64;
                let _ = player.try_seek(Duration::from_millis(target));
            }

            let position = player.get_pos().as_millis() as i64;
            let snapshot =
                PlaybackSnapshot::from_manifest(audiobook_id.clone(), &manifest, position, true);
            *self
                .current
                .lock()
                .map_err(|_| anyhow::anyhow!("reproductor de audiolibros bloqueado"))? =
                Some(Current {
                    audiobook_id,
                    manifest,
                });
            Ok(snapshot)
        }

        pub fn pause(&self) -> Result<Option<PlaybackSnapshot>> {
            let (_, player) = self.output()?;
            player.pause();
            self.snapshot()
        }

        pub fn resume(&self) -> Result<Option<PlaybackSnapshot>> {
            let (_, player) = self.output()?;
            player.play();
            self.snapshot()
        }

        pub fn stop(&self) -> Result<()> {
            let (_, player) = self.output()?;
            player.stop();
            self.finished.store(false, Ordering::SeqCst);
            *self
                .current
                .lock()
                .map_err(|_| anyhow::anyhow!("reproductor de audiolibros bloqueado"))? = None;
            Ok(())
        }

        pub fn set_speed(&self, speed: f32) -> Result<Option<PlaybackSnapshot>> {
            let (_, player) = self.output()?;
            player.set_speed(speed.clamp(0.5, 2.5));
            self.snapshot()
        }

        pub fn seek(&self, position_ms: i64) -> Result<Option<PlaybackSnapshot>> {
            let (_, player) = self.output()?;
            let current = self
                .current
                .lock()
                .map_err(|_| anyhow::anyhow!("reproductor de audiolibros bloqueado"))?
                .as_ref()
                .map(|current| (current.audiobook_id.clone(), current.manifest.duration_ms));
            let Some((_, duration)) = current else {
                return Ok(None);
            };
            let target = position_ms.clamp(0, duration.max(0)) as u64;
            self.finished.store(false, Ordering::SeqCst);
            player
                .try_seek(Duration::from_millis(target))
                .map_err(|error| anyhow::anyhow!("no se pudo buscar en el audiolibro: {error}"))?;
            self.snapshot()
        }

        pub fn snapshot(&self) -> Result<Option<PlaybackSnapshot>> {
            let (_, player) = self.output()?;
            let current = self
                .current
                .lock()
                .map_err(|_| anyhow::anyhow!("reproductor de audiolibros bloqueado"))?;
            let Some(current) = current.as_ref() else {
                return Ok(None);
            };
            let duration = current.manifest.duration_ms.max(0);
            let finished = player.empty() && !player.is_paused();
            let position = if finished {
                duration
            } else {
                player.get_pos().as_millis() as i64
            };
            if finished {
                self.finished.store(true, Ordering::SeqCst);
            }
            let playing = !player.is_paused() && !finished;
            Ok(Some(PlaybackSnapshot::from_manifest(
                current.audiobook_id.clone(),
                &current.manifest,
                position,
                playing,
            )))
        }

        /// Indica si el audio terminó de forma natural. El comando que sondea
        /// guarda el progreso y después llama a `clear_finished`.
        pub fn finished(&self) -> bool {
            self.finished.load(Ordering::SeqCst)
        }

        pub fn clear_finished(&self) {
            self.finished.store(false, Ordering::SeqCst);
        }

        pub fn position_ms(&self) -> i64 {
            self.output
                .lock()
                .ok()
                .and_then(|slot| slot.as_ref().map(|output| output.player.get_pos()))
                .map(|position| position.as_millis() as i64)
                .unwrap_or(0)
        }
    }
}

#[cfg(target_os = "android")]
mod android {
    use std::path::Path;

    use anyhow::{Result, bail};

    use super::PlaybackSnapshot;
    use crate::audiobooks::manifest::AudiobookManifest;

    #[derive(Default)]
    pub struct AudiobookPlayback;

    impl AudiobookPlayback {
        pub fn play(
            &self,
            _audiobook_id: String,
            _path: &Path,
            _manifest: AudiobookManifest,
            _start_ms: i64,
        ) -> Result<PlaybackSnapshot> {
            bail!("el reproductor de audiolibros aún no está disponible en Android")
        }

        pub fn pause(&self) -> Result<Option<PlaybackSnapshot>> {
            bail!("el reproductor de audiolibros aún no está disponible en Android")
        }

        pub fn resume(&self) -> Result<Option<PlaybackSnapshot>> {
            bail!("el reproductor de audiolibros aún no está disponible en Android")
        }

        pub fn stop(&self) -> Result<()> {
            Ok(())
        }

        pub fn set_speed(&self, _speed: f32) -> Result<Option<PlaybackSnapshot>> {
            bail!("el reproductor de audiolibros aún no está disponible en Android")
        }

        pub fn seek(&self, _position_ms: i64) -> Result<Option<PlaybackSnapshot>> {
            bail!("el reproductor de audiolibros aún no está disponible en Android")
        }

        pub fn snapshot(&self) -> Result<Option<PlaybackSnapshot>> {
            Ok(None)
        }

        pub fn position_ms(&self) -> i64 {
            0
        }

        pub fn finished(&self) -> bool {
            false
        }

        pub fn clear_finished(&self) {}
    }
}

#[cfg(target_os = "android")]
pub use android::AudiobookPlayback;
#[cfg(not(target_os = "android"))]
pub use desktop::AudiobookPlayback;
