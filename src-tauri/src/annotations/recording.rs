#[cfg(not(target_os = "android"))]
mod desktop {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use crate::error::{KoraError, Result};

    const MAX_SECONDS: usize = 300;

    #[derive(Debug, Clone)]
    pub struct Recording {
        pub samples: Vec<f32>,
        pub sample_rate: u32,
        pub duration_ms: i64,
    }

    struct Active {
        stop: Arc<AtomicBool>,
        handle: Option<std::thread::JoinHandle<Result<Recording>>>,
    }

    /// Grabadora de notas de voz. Mantiene el micrófono abierto en un hilo
    /// independiente y devuelve las muestras al detener.
    #[derive(Default)]
    pub struct VoiceNoteRecorder {
        active: Mutex<Option<Active>>,
    }

    impl VoiceNoteRecorder {
        pub fn start(&self) -> Result<()> {
            let mut slot = self
                .active
                .lock()
                .map_err(|_| KoraError::Internal("grabadora de voz bloqueada".into()))?;
            if slot.is_some() {
                return Err(KoraError::InvalidInput(
                    "ya hay una nota de voz grabándose".into(),
                ));
            }

            let stop = Arc::new(AtomicBool::new(false));
            let stop_for_thread = stop.clone();
            let handle = std::thread::Builder::new()
                .name("kora-voice-note".into())
                .spawn(move || record_blocking(stop_for_thread))
                .map_err(|error| {
                    KoraError::Internal(format!("no se pudo iniciar el micrófono: {error}"))
                })?;

            *slot = Some(Active {
                stop,
                handle: Some(handle),
            });
            Ok(())
        }

        pub fn stop(&self) -> Result<Recording> {
            let active = self
                .active
                .lock()
                .map_err(|_| KoraError::Internal("grabadora de voz bloqueada".into()))?
                .take()
                .ok_or_else(|| {
                    KoraError::InvalidInput("no hay una nota de voz grabándose".into())
                })?;

            active.stop.store(true, Ordering::SeqCst);
            let handle = active
                .handle
                .ok_or_else(|| KoraError::Internal("grabadora sin hilo".into()))?;
            handle.join().map_err(|_| {
                KoraError::Internal("la grabación terminó de forma inesperada".into())
            })?
        }

        pub fn cancel(&self) {
            if let Ok(mut slot) = self.active.lock()
                && let Some(mut active) = slot.take()
            {
                active.stop.store(true, Ordering::SeqCst);
                if let Some(handle) = active.handle.take() {
                    let _ = handle.join();
                }
            }
        }

        pub fn is_recording(&self) -> bool {
            self.active
                .lock()
                .map(|slot| slot.is_some())
                .unwrap_or(false)
        }
    }

    fn record_blocking(stop: Arc<AtomicBool>) -> Result<Recording> {
        use cpal::SampleFormat;
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| KoraError::Unsupported("no se encontró un micrófono".into()))?;
        let supported = device
            .default_input_config()
            .map_err(|error| KoraError::Unsupported(format!("micrófono no disponible: {error}")))?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let channels = config.channels as usize;
        let source_rate = config.sample_rate;
        let samples = Arc::new(Mutex::new(Vec::<f32>::new()));

        let stream = match sample_format {
            SampleFormat::F32 => {
                let samples = samples.clone();
                device
                    .build_input_stream(
                        config,
                        move |data: &[f32], _| push_mono_f32(data, channels, &samples, source_rate),
                        stream_error,
                        None,
                    )
                    .map_err(|error| {
                        KoraError::Unsupported(format!("no se pudo abrir el micrófono: {error}"))
                    })?
            }
            SampleFormat::I16 => {
                let samples = samples.clone();
                device
                    .build_input_stream(
                        config,
                        move |data: &[i16], _| push_mono_i16(data, channels, &samples, source_rate),
                        stream_error,
                        None,
                    )
                    .map_err(|error| {
                        KoraError::Unsupported(format!("no se pudo abrir el micrófono: {error}"))
                    })?
            }
            SampleFormat::U16 => {
                let samples = samples.clone();
                device
                    .build_input_stream(
                        config,
                        move |data: &[u16], _| push_mono_u16(data, channels, &samples, source_rate),
                        stream_error,
                        None,
                    )
                    .map_err(|error| {
                        KoraError::Unsupported(format!("no se pudo abrir el micrófono: {error}"))
                    })?
            }
            other => {
                return Err(KoraError::Unsupported(format!(
                    "el micrófono usa un formato no soportado: {other:?}"
                )));
            }
        };

        stream.play().map_err(|error| {
            KoraError::Unsupported(format!("no se pudo iniciar el micrófono: {error}"))
        })?;

        let started = Instant::now();
        while !stop.load(Ordering::SeqCst)
            && started.elapsed() < Duration::from_secs(MAX_SECONDS as u64)
        {
            std::thread::sleep(Duration::from_millis(50));
        }
        drop(stream);

        let mono = samples
            .lock()
            .map_err(|_| KoraError::Internal("buffer de micrófono envenenado".into()))?
            .clone();
        if mono.is_empty() {
            return Err(KoraError::InvalidInput(
                "la nota de voz quedó vacía; revisa el micrófono".into(),
            ));
        }

        let duration_ms = mono.len() as i64 * 1000 / source_rate.max(1) as i64;
        Ok(Recording {
            samples: mono,
            sample_rate: source_rate,
            duration_ms,
        })
    }

    fn stream_error(error: cpal::Error) {
        tracing::warn!(%error, "error del micrófono en la nota de voz");
    }

    fn push_mono_f32(data: &[f32], channels: usize, out: &Arc<Mutex<Vec<f32>>>, rate: u32) {
        if let Ok(mut out) = out.lock() {
            let limit = rate as usize * MAX_SECONDS;
            if out.len() >= limit {
                return;
            }
            if channels <= 1 {
                out.extend_from_slice(data);
            } else {
                for frame in data.chunks(channels) {
                    out.push(frame.iter().sum::<f32>() / frame.len() as f32);
                }
            }
        }
    }

    fn push_mono_i16(data: &[i16], channels: usize, out: &Arc<Mutex<Vec<f32>>>, rate: u32) {
        if let Ok(mut out) = out.lock() {
            let limit = rate as usize * MAX_SECONDS;
            if out.len() >= limit {
                return;
            }
            if channels <= 1 {
                out.extend(data.iter().map(|sample| *sample as f32 / i16::MAX as f32));
            } else {
                for frame in data.chunks(channels) {
                    let sum: f32 = frame
                        .iter()
                        .map(|sample| *sample as f32 / i16::MAX as f32)
                        .sum();
                    out.push(sum / frame.len() as f32);
                }
            }
        }
    }

    fn push_mono_u16(data: &[u16], channels: usize, out: &Arc<Mutex<Vec<f32>>>, rate: u32) {
        if let Ok(mut out) = out.lock() {
            let limit = rate as usize * MAX_SECONDS;
            if out.len() >= limit {
                return;
            }
            if channels <= 1 {
                out.extend(
                    data.iter()
                        .map(|sample| (*sample as f32 - 32768.0) / 32768.0),
                );
            } else {
                for frame in data.chunks(channels) {
                    let sum: f32 = frame
                        .iter()
                        .map(|sample| (*sample as f32 - 32768.0) / 32768.0)
                        .sum();
                    out.push(sum / frame.len() as f32);
                }
            }
        }
    }
}

#[cfg(target_os = "android")]
mod android {
    use crate::error::{KoraError, Result};

    #[derive(Debug, Clone)]
    pub struct Recording {
        pub samples: Vec<f32>,
        pub sample_rate: u32,
        pub duration_ms: i64,
    }

    #[derive(Default)]
    pub struct VoiceNoteRecorder;

    impl VoiceNoteRecorder {
        pub fn start(&self) -> Result<()> {
            Err(KoraError::Unsupported(
                "las notas de voz aún no están disponibles en Android".into(),
            ))
        }

        pub fn stop(&self) -> Result<Recording> {
            Err(KoraError::Unsupported(
                "las notas de voz aún no están disponibles en Android".into(),
            ))
        }

        pub fn cancel(&self) {}

        pub fn is_recording(&self) -> bool {
            false
        }
    }
}

#[cfg(target_os = "android")]
pub use android::{Recording, VoiceNoteRecorder};
#[cfg(not(target_os = "android"))]
pub use desktop::{Recording, VoiceNoteRecorder};
