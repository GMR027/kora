use async_trait::async_trait;

use super::engine::AudioChunk;
use crate::error::Result;

/// Salida de audio del motor TTS. En escritorio la implementa `RodioPlayer`;
/// en Android el propio motor nativo reproduce y no usa este trait.
#[async_trait]
pub trait AudioPlayer: Send + Sync {
    /// Reproduce el bloque completo y vuelve cuando termina o se detiene.
    async fn play(&self, chunk: AudioChunk) -> Result<()>;
    async fn pause(&self) -> Result<()>;
    async fn resume(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
}

/// Reproductor nulo para tests y para plataformas sin salida de audio.
#[derive(Debug, Default)]
pub struct NullAudioPlayer;

#[async_trait]
impl AudioPlayer for NullAudioPlayer {
    async fn play(&self, _chunk: AudioChunk) -> Result<()> {
        Ok(())
    }

    async fn pause(&self) -> Result<()> {
        Ok(())
    }

    async fn resume(&self) -> Result<()> {
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use std::num::{NonZeroU16, NonZeroU32};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};

    use super::AudioPlayer;
    use crate::error::{KoraError, Result};
    use crate::tts::engine::AudioChunk;

    struct Output {
        _sink: Arc<MixerDeviceSink>,
        player: Arc<Player>,
    }

    /// Reproductor basado en `rodio` (cpal por debajo). El dispositivo se abre
    /// de forma perezosa para que la app arranque aunque no haya tarjeta de
    /// sonido; en ese caso `play` devuelve un error claro.
    #[derive(Default)]
    pub struct RodioPlayer {
        output: Mutex<Option<Output>>,
        paused: AtomicBool,
        /// Cambia con cada `stop` para que una frase que aún se está
        /// sintetizando no empiece a sonar después de detener la sesión.
        generation: Arc<AtomicU64>,
    }

    impl RodioPlayer {
        fn player(&self) -> Result<Arc<Player>> {
            let mut slot = self
                .output
                .lock()
                .map_err(|_| KoraError::Internal("reproductor de audio envenenado".into()))?;

            if slot.is_none() {
                let mut sink = DeviceSinkBuilder::open_default_sink().map_err(|error| {
                    KoraError::Unsupported(format!(
                        "no se pudo abrir el dispositivo de audio: {error}"
                    ))
                })?;
                sink.log_on_drop(false);
                let player = Arc::new(Player::connect_new(sink.mixer()));
                *slot = Some(Output {
                    _sink: Arc::new(sink),
                    player,
                });
            }

            Ok(slot
                .as_ref()
                .expect("salida de audio inicializada")
                .player
                .clone())
        }
    }

    #[async_trait]
    impl AudioPlayer for RodioPlayer {
        async fn play(&self, chunk: AudioChunk) -> Result<()> {
            let player = self.player()?;
            let samples: Vec<f32> = chunk
                .pcm
                .iter()
                .map(|sample| *sample as f32 / i16::MAX as f32)
                .collect();
            if samples.is_empty() {
                return Ok(());
            }

            let channels = NonZeroU16::new(chunk.channels.max(1)).unwrap_or(NonZeroU16::MIN);
            let sample_rate = NonZeroU32::new(chunk.sample_rate.max(1)).unwrap_or(NonZeroU32::MIN);

            tokio::task::spawn_blocking(move || {
                // `stop()` deja la cola vacía; `play()` garantiza que una
                // sesión nueva no herede una pausa anterior.
                player.play();
                player.append(rodio::buffer::SamplesBuffer::new(
                    channels,
                    sample_rate,
                    samples,
                ));
                player.sleep_until_end();
            })
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de audio: {error}")))?;

            Ok(())
        }

        async fn pause(&self) -> Result<()> {
            self.paused.store(true, Ordering::SeqCst);
            if let Ok(player) = self.player() {
                player.pause();
            }
            Ok(())
        }

        async fn resume(&self) -> Result<()> {
            self.paused.store(false, Ordering::SeqCst);
            if let Ok(player) = self.player() {
                player.play();
            }
            Ok(())
        }

        async fn stop(&self) -> Result<()> {
            self.paused.store(false, Ordering::SeqCst);
            self.generation.fetch_add(1, Ordering::SeqCst);
            if let Ok(player) = self.player() {
                player.stop();
            }
            Ok(())
        }
    }
}

#[cfg(not(target_os = "android"))]
pub use desktop::RodioPlayer;

#[cfg(all(test, not(target_os = "android")))]
mod audio_tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn plays_a_tone_on_the_default_output() {
        let player = RodioPlayer::default();
        let samples = (0..24_000)
            .map(|index| {
                let phase = index as f32 / 24_000.0 * 440.0 * std::f32::consts::TAU;
                (phase.sin() * 0.08 * i16::MAX as f32) as i16
            })
            .collect();
        player
            .play(AudioChunk {
                pcm: samples,
                sample_rate: 24_000,
                channels: 1,
            })
            .await
            .expect("reproducir tono");
    }
}
