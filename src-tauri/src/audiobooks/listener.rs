/// Locución capturada del micrófono, ya remuestreada a 16 kHz mono para
/// Whisper. Los milisegundos indican cuánto duró la frase.
#[derive(Debug, Clone)]
pub struct Utterance {
    pub samples: Vec<f32>,
    pub duration_ms: i64,
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    use crate::error::{KoraError, Result};

    use super::Utterance;
    use crate::audiobooks::audio;
    use crate::audiobooks::voice_marks::{SegmenterConfig, UtteranceSegmenter};

    const WHISPER_RATE: u32 = 16_000;
    const MAX_CAPTURE_BUFFER_SECONDS: usize = 20;

    struct Active {
        stop: Arc<AtomicBool>,
        handle: Option<JoinHandle<()>>,
        error: Arc<Mutex<Option<String>>>,
    }

    #[derive(Default)]
    struct Inner {
        active: Option<Active>,
        receiver: Option<mpsc::Receiver<Utterance>>,
    }

    /// Escucha continua del micrófono para los marcadores de voz. El hilo de
    /// captura abre cpal, segmenta por silencios y entrega locuciones ya
    /// remuestreadas. La transcripción la hace el worker de los comandos.
    #[derive(Default)]
    pub struct VoiceMarkerListener {
        inner: Mutex<Inner>,
        session: AtomicU64,
    }

    impl VoiceMarkerListener {
        /// Abre el micrófono y devuelve la generación de esta sesión. Si el
        /// dispositivo no está disponible, el error se devuelve de inmediato.
        pub fn start(&self) -> Result<u64> {
            let mut inner = self
                .inner
                .lock()
                .map_err(|_| KoraError::Internal("escucha de marcadores bloqueada".into()))?;
            let stale = inner
                .active
                .as_ref()
                .and_then(|active| active.handle.as_ref())
                .map(|handle| handle.is_finished())
                .unwrap_or(true);
            if inner.active.is_some() && stale {
                let (active, receiver) = (inner.active.take(), inner.receiver.take());
                drop(receiver);
                if let Some(mut active) = active {
                    active.stop.store(true, Ordering::SeqCst);
                    if let Some(handle) = active.handle.take() {
                        let _ = handle.join();
                    }
                }
            }
            if inner.active.is_some() {
                return Err(KoraError::Conflict(
                    "ya hay una escucha de marcadores activa".into(),
                ));
            }

            let (sender, receiver) = mpsc::sync_channel::<Utterance>(8);
            let (ready_tx, ready_rx) = mpsc::sync_channel::<std::result::Result<(), String>>(1);
            let stop = Arc::new(AtomicBool::new(false));
            let error = Arc::new(Mutex::new(None));
            let stop_for_thread = stop.clone();
            let error_for_thread = error.clone();
            let ready_for_error = ready_tx.clone();

            let handle = thread::Builder::new()
                .name("kora-voice-marks".into())
                .spawn(move || {
                    if let Err(message) =
                        capture_loop(stop_for_thread, sender, ready_tx, error_for_thread.clone())
                    {
                        if let Ok(mut slot) = error_for_thread.lock() {
                            *slot = Some(message.clone());
                        }
                        let _ = ready_for_error.send(Err(message));
                    }
                })
                .map_err(|cause| {
                    KoraError::Internal(format!("no se pudo iniciar el micrófono: {cause}"))
                })?;

            match ready_rx.recv_timeout(Duration::from_secs(4)) {
                Ok(Ok(())) => {
                    let generation = self.session.fetch_add(1, Ordering::SeqCst) + 1;
                    inner.active = Some(Active {
                        stop,
                        handle: Some(handle),
                        error,
                    });
                    inner.receiver = Some(receiver);
                    Ok(generation)
                }
                Ok(Err(message)) => {
                    stop.store(true, Ordering::SeqCst);
                    let _ = handle.join();
                    Err(KoraError::Unsupported(message))
                }
                Err(_) => {
                    stop.store(true, Ordering::SeqCst);
                    let _ = handle.join();
                    Err(KoraError::Unsupported(
                        "el micrófono tardó demasiado en responder".into(),
                    ))
                }
            }
        }

        pub fn stop(&self) {
            self.session.store(0, Ordering::SeqCst);
            let active = self.inner.lock().ok().and_then(|mut inner| {
                // Soltar el receptor antes de unir el hilo: si el hilo estaba
                // bloqueado enviando una locución, el envío falla y termina.
                inner.receiver = None;
                inner.active.take()
            });
            if let Some(mut active) = active {
                active.stop.store(true, Ordering::SeqCst);
                if let Some(handle) = active.handle.take() {
                    let _ = handle.join();
                }
            }
        }

        pub fn is_listening(&self) -> bool {
            self.inner
                .lock()
                .map(|inner| {
                    inner
                        .active
                        .as_ref()
                        .and_then(|active| active.handle.as_ref())
                        .map(|handle| !handle.is_finished())
                        .unwrap_or(false)
                })
                .unwrap_or(false)
        }

        pub fn is_current(&self, generation: u64) -> bool {
            generation != 0 && self.session.load(Ordering::SeqCst) == generation
        }

        /// Devuelve la siguiente locución sin bloquear. Solo entrega audios de
        /// la generación indicada para que un worker antiguo no consuma los de
        /// una sesión nueva.
        pub fn try_next(&self, generation: u64) -> Option<Utterance> {
            if !self.is_current(generation) {
                return None;
            }
            let inner = self.inner.lock().ok()?;
            inner.receiver.as_ref()?.try_recv().ok()
        }

        pub fn take_error(&self) -> Option<String> {
            self.inner.lock().ok().and_then(|inner| {
                inner
                    .active
                    .as_ref()
                    .and_then(|active| active.error.lock().ok()?.clone())
            })
        }
    }

    impl Drop for VoiceMarkerListener {
        fn drop(&mut self) {
            self.session.store(0, Ordering::SeqCst);
            if let Ok(inner) = self.inner.get_mut() {
                let receiver = inner.receiver.take();
                drop(receiver);
                if let Some(mut active) = inner.active.take() {
                    active.stop.store(true, Ordering::SeqCst);
                    if let Some(handle) = active.handle.take() {
                        let _ = handle.join();
                    }
                }
            }
        }
    }

    fn capture_loop(
        stop: Arc<AtomicBool>,
        sender: mpsc::SyncSender<Utterance>,
        ready: mpsc::SyncSender<std::result::Result<(), String>>,
        error: Arc<Mutex<Option<String>>>,
    ) -> std::result::Result<(), String> {
        use cpal::SampleFormat;
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "no se encontró un micrófono".to_string())?;
        let supported = device
            .default_input_config()
            .map_err(|cause| format!("micrófono no disponible: {cause}"))?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let channels = config.channels as usize;
        let source_rate = config.sample_rate;
        let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
        let limit = source_rate as usize * MAX_CAPTURE_BUFFER_SECONDS;

        let stream = match sample_format {
            SampleFormat::F32 => {
                let buffer = buffer.clone();
                device
                    .build_input_stream(
                        config,
                        move |data: &[f32], _| push_f32(data, channels, &buffer, limit),
                        stream_error,
                        None,
                    )
                    .map_err(|cause| format!("no se pudo abrir el micrófono: {cause}"))?
            }
            SampleFormat::I16 => {
                let buffer = buffer.clone();
                device
                    .build_input_stream(
                        config,
                        move |data: &[i16], _| push_i16(data, channels, &buffer, limit),
                        stream_error,
                        None,
                    )
                    .map_err(|cause| format!("no se pudo abrir el micrófono: {cause}"))?
            }
            SampleFormat::U16 => {
                let buffer = buffer.clone();
                device
                    .build_input_stream(
                        config,
                        move |data: &[u16], _| push_u16(data, channels, &buffer, limit),
                        stream_error,
                        None,
                    )
                    .map_err(|cause| format!("no se pudo abrir el micrófono: {cause}"))?
            }
            other => {
                return Err(format!(
                    "el micrófono usa un formato no soportado: {other:?}"
                ));
            }
        };

        stream
            .play()
            .map_err(|cause| format!("no se pudo iniciar el micrófono: {cause}"))?;
        let _ = ready.send(Ok(()));

        let mut segmenter = UtteranceSegmenter::new(SegmenterConfig {
            sample_rate: source_rate,
            ..SegmenterConfig::default()
        });

        while !stop.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(40));
            if let Ok(buffer) = buffer.lock()
                && buffer.is_empty()
            {
                continue;
            }
            let chunk = match buffer.lock() {
                Ok(mut buffer) => std::mem::take(&mut *buffer),
                Err(_) => {
                    if let Ok(mut slot) = error.lock() {
                        *slot = Some("el buffer del micrófono quedó bloqueado".into());
                    }
                    break;
                }
            };

            for utterance in segmenter.push(&chunk) {
                let samples = audio::resample(&utterance, source_rate, WHISPER_RATE);
                if samples.is_empty() {
                    continue;
                }
                let duration_ms = samples.len() as i64 * 1000 / WHISPER_RATE as i64;
                if sender
                    .send(Utterance {
                        samples,
                        duration_ms,
                    })
                    .is_err()
                {
                    return Ok(());
                }
            }
        }

        drop(stream);
        Ok(())
    }

    fn stream_error(error: cpal::Error) {
        tracing::warn!(%error, "error del micrófono en los marcadores de voz");
    }

    fn push_f32(data: &[f32], channels: usize, out: &Arc<Mutex<Vec<f32>>>, limit: usize) {
        if let Ok(mut out) = out.lock() {
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

    fn push_i16(data: &[i16], channels: usize, out: &Arc<Mutex<Vec<f32>>>, limit: usize) {
        if let Ok(mut out) = out.lock() {
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

    fn push_u16(data: &[u16], channels: usize, out: &Arc<Mutex<Vec<f32>>>, limit: usize) {
        if let Ok(mut out) = out.lock() {
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

    use super::Utterance;

    #[derive(Default)]
    pub struct VoiceMarkerListener;

    impl VoiceMarkerListener {
        pub fn start(&self) -> Result<u64> {
            Err(KoraError::Unsupported(
                "los marcadores de voz aún no están disponibles en Android".into(),
            ))
        }

        pub fn stop(&self) {}

        pub fn is_listening(&self) -> bool {
            false
        }

        pub fn is_current(&self, _generation: u64) -> bool {
            false
        }

        pub fn try_next(&self, _generation: u64) -> Option<Utterance> {
            None
        }

        pub fn take_error(&self) -> Option<String> {
            None
        }
    }
}

#[cfg(target_os = "android")]
pub use android::VoiceMarkerListener;
#[cfg(not(target_os = "android"))]
pub use desktop::VoiceMarkerListener;
