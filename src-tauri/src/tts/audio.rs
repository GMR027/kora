use async_trait::async_trait;

use super::engine::{AudioChunk, AudioPlaybackEvent, QueuedAudio};
use crate::error::{KoraError, Result};

/// Salida de audio del motor TTS. En escritorio la implementa `RodioPlayer`;
/// en Android el propio motor nativo reproduce y no usa este trait.
#[async_trait]
pub trait AudioPlayer: Send + Sync {
    /// Prepara una locución nueva y descarta cualquier orden de parada previa.
    async fn reset(&self) -> Result<()> {
        Ok(())
    }
    /// Reproduce el bloque completo y vuelve cuando termina o se detiene.
    async fn play(&self, chunk: AudioChunk) -> Result<()>;
    async fn pause(&self) -> Result<()>;
    async fn resume(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;

    /// Reproduce una cola sin cerrar el reproductor entre elementos.
    async fn stream_audio(
        &self,
        _items: tokio::sync::mpsc::Receiver<QueuedAudio>,
        _events: tokio::sync::mpsc::Sender<AudioPlaybackEvent>,
    ) -> Result<()> {
        Err(KoraError::Unsupported(
            "este reproductor no admite cola continua".into(),
        ))
    }
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

#[cfg(target_os = "android")]
mod android {
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::Duration;

    use async_trait::async_trait;
    use tokio::sync::mpsc;
    use tokio::sync::mpsc::error::TryRecvError;

    use super::super::engine::{AudioChunk, AudioPlaybackEvent, QueuedAudio};
    use super::AudioPlayer;
    use crate::error::{KoraError, Result};

    /// Reproduce los WAV que sintetiza Supertonic con Media3.
    ///
    /// El motor de Android resuelve la voz nativa por su cuenta; este
    /// reproductor solo se usa para audio PCM generado en Rust.
    pub struct AndroidAudioPlayer {
        directory: PathBuf,
        generation: Arc<AtomicU64>,
        stopped: Arc<AtomicBool>,
        sequence: Arc<AtomicU64>,
    }

    impl AndroidAudioPlayer {
        pub fn new(directory: PathBuf) -> Self {
            Self {
                directory,
                generation: Arc::new(AtomicU64::new(0)),
                stopped: Arc::new(AtomicBool::new(false)),
                sequence: Arc::new(AtomicU64::new(0)),
            }
        }
    }

    fn write_wav(directory: &Path, sequence: &AtomicU64, chunk: &AudioChunk) -> Result<PathBuf> {
        std::fs::create_dir_all(directory)?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or(0);
        let order = sequence.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!("speech-{stamp}-{order}.wav"));
        let wav = super::super::wav::encode_wav(chunk)?;
        std::fs::write(&path, wav)?;
        Ok(path)
    }

    #[async_trait]
    impl AudioPlayer for AndroidAudioPlayer {
        async fn reset(&self) -> Result<()> {
            self.stopped.store(false, Ordering::SeqCst);
            Ok(())
        }

        async fn play(&self, chunk: AudioChunk) -> Result<()> {
            let directory = self.directory.clone();
            let generation = self.generation.load(Ordering::SeqCst);
            let stopped = self.stopped.clone();
            let generation_slot = self.generation.clone();

            tokio::task::spawn_blocking(move || -> Result<()> {
                if stopped.load(Ordering::SeqCst) {
                    return Ok(());
                }
                std::fs::create_dir_all(&directory)?;
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|value| value.as_millis())
                    .unwrap_or(0);
                let path = directory.join(format!("speech-{stamp}.wav"));
                let wav = super::super::wav::encode_wav(&chunk)?;
                std::fs::write(&path, wav)?;

                let path_text = path
                    .to_str()
                    .ok_or_else(|| KoraError::Internal("ruta de audio no válida".into()))?
                    .to_string();

                if let Err(error) = super::super::android::play_speech_file(&path_text, 1.0) {
                    let _ = std::fs::remove_file(&path);
                    return Err(error);
                }

                let result = loop {
                    if stopped.load(Ordering::SeqCst)
                        || generation_slot.load(Ordering::SeqCst) != generation
                    {
                        let _ = super::super::android::stop_speech_file();
                        break Ok(());
                    }
                    match super::super::android::speech_file_status() {
                        Ok(status) => match status.state.as_str() {
                            "finished" | "idle" => break Ok(()),
                            "error" => {
                                break Err(KoraError::Internal(
                                    status
                                        .error
                                        .unwrap_or_else(|| "error de reproducción".into()),
                                ));
                            }
                            _ => std::thread::sleep(Duration::from_millis(80)),
                        },
                        Err(error) => break Err(error),
                    }
                };

                let _ = std::fs::remove_file(&path);
                result
            })
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de audio Android: {error}")))?
        }

        async fn stream_audio(
            &self,
            mut items: mpsc::Receiver<QueuedAudio>,
            events: mpsc::Sender<AudioPlaybackEvent>,
        ) -> Result<()> {
            let directory = self.directory.clone();
            let generation = self.generation.load(Ordering::SeqCst);
            let stopped = self.stopped.clone();
            let generation_slot = self.generation.clone();
            let sequence = self.sequence.clone();

            tokio::task::spawn_blocking(move || -> Result<()> {
                let mut enqueued: VecDeque<(usize, PathBuf)> = VecDeque::new();
                let mut current: Option<usize> = None;
                let mut producer_done = false;

                loop {
                    if stopped.load(Ordering::SeqCst)
                        || generation_slot.load(Ordering::SeqCst) != generation
                    {
                        break;
                    }

                    loop {
                        match items.try_recv() {
                            Ok(item) => {
                                let path = write_wav(&directory, &sequence, &item.audio)?;
                                let path_text = path
                                    .to_str()
                                    .ok_or_else(|| {
                                        KoraError::Internal(
                                            "ruta de audio Android no válida".into(),
                                        )
                                    })?
                                    .to_string();
                                if let Err(error) =
                                    super::super::android::enqueue_speech_file(&path_text, 1.0)
                                {
                                    let _ = std::fs::remove_file(&path);
                                    return Err(error);
                                }
                                enqueued.push_back((item.index, path));
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                producer_done = true;
                                break;
                            }
                        }
                    }

                    let status = super::super::android::speech_file_status()?;
                    if status.state == "error" {
                        return Err(KoraError::Internal(
                            status
                                .error
                                .unwrap_or_else(|| "error de reproducción Android".into()),
                        ));
                    }

                    if !enqueued.is_empty()
                        && matches!(
                            status.state.as_str(),
                            "starting" | "buffering" | "playing" | "paused"
                        )
                    {
                        let index = status.current_index.min(enqueued.len() - 1);
                        if current != Some(index) {
                            if let Some(previous) = current
                                && let Some((item_index, _)) = enqueued.get(previous)
                                && events
                                    .blocking_send(AudioPlaybackEvent::Finished(*item_index))
                                    .is_err()
                            {
                                break;
                            }
                            current = Some(index);
                            if let Some((item_index, _)) = enqueued.get(index)
                                && events
                                    .blocking_send(AudioPlaybackEvent::Started(*item_index))
                                    .is_err()
                            {
                                break;
                            }
                        }
                    }

                    if status.state == "finished" && producer_done {
                        if let Some(previous) = current
                            && let Some((item_index, _)) = enqueued.get(previous)
                        {
                            let _ = events.blocking_send(AudioPlaybackEvent::Finished(*item_index));
                        }
                        break;
                    }
                    if status.state == "idle" && producer_done && enqueued.is_empty() {
                        break;
                    }

                    std::thread::sleep(Duration::from_millis(40));
                }

                for (_, path) in enqueued {
                    let _ = std::fs::remove_file(path);
                }
                Ok(())
            })
            .await
            .map_err(|error| KoraError::Internal(format!("tarea de audio Android: {error}")))?
        }

        async fn pause(&self) -> Result<()> {
            super::super::android::pause_speech_file()
        }

        async fn resume(&self) -> Result<()> {
            super::super::android::resume_speech_file()
        }

        async fn stop(&self) -> Result<()> {
            self.stopped.store(true, Ordering::SeqCst);
            self.generation.fetch_add(1, Ordering::SeqCst);
            super::super::android::stop_speech_file()
        }
    }
}

#[cfg(target_os = "android")]
pub use android::AndroidAudioPlayer;

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
