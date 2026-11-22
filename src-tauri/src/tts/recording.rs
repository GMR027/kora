use std::path::{Path, PathBuf};

use crate::error::{KoraError, Result};

pub const MAX_SAMPLE_SECONDS: u64 = 30;

#[derive(Debug, Clone)]
pub struct Recording {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration_ms: u64,
    pub wav_path: PathBuf,
    /// Códigos de aviso sobre la calidad de la muestra (`clipped`, `trimmed`, `lowLevel`).
    pub warnings: Vec<String>,
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use crate::error::{KoraError, Result};
    use crate::tts::engine::AudioChunk;

    use super::Recording;

    struct ActiveSample {
        stop: Arc<AtomicBool>,
        handle: Option<std::thread::JoinHandle<Result<Recording>>>,
    }

    /// Grabadora de muestras de voz para clonación. Mantiene el micrófono
    /// abierto en un hilo propio y escribe un WAV mono de 16 kHz.
    #[derive(Default)]
    pub struct SampleRecorder {
        active: Mutex<Option<ActiveSample>>,
        /// Pico suavizado del micrófono, codificado como bits de `f32`.
        level: Arc<AtomicU32>,
    }

    impl SampleRecorder {
        pub fn start(
            &self,
            directory: PathBuf,
            max_seconds: u64,
            device_name: Option<String>,
        ) -> Result<()> {
            let mut slot = self
                .active
                .lock()
                .map_err(|_| KoraError::Internal("grabadora de voz bloqueada".into()))?;
            if slot.is_some() {
                return Err(KoraError::Conflict(
                    "ya hay una grabación de voz en curso".into(),
                ));
            }

            std::fs::create_dir_all(&directory)?;
            let id = uuid::Uuid::new_v4().to_string();
            let stop = Arc::new(AtomicBool::new(false));
            let stop_for_thread = stop.clone();
            let level = self.level.clone();
            self.level.store(0f32.to_bits(), Ordering::SeqCst);
            let handle = std::thread::Builder::new()
                .name("kora-voice-sample".into())
                .spawn(move || {
                    record_to_wav(
                        &directory,
                        &id,
                        stop_for_thread,
                        max_seconds,
                        device_name.as_deref(),
                        &level,
                        true,
                    )
                })
                .map_err(|error| {
                    KoraError::Internal(format!("no se pudo iniciar el micrófono: {error}"))
                })?;

            *slot = Some(ActiveSample {
                stop,
                handle: Some(handle),
            });
            Ok(())
        }

        pub fn stop(&self) -> Result<Recording> {
            let mut slot = self
                .active
                .lock()
                .map_err(|_| KoraError::Internal("grabadora de voz bloqueada".into()))?;
            let mut active = slot
                .take()
                .ok_or_else(|| KoraError::InvalidInput("no hay una grabación en curso".into()))?;
            active.stop.store(true, Ordering::SeqCst);
            let result = active
                .handle
                .take()
                .ok_or_else(|| KoraError::Internal("grabadora sin hilo".into()))?
                .join()
                .map_err(|_| {
                    KoraError::Internal("la grabación terminó de forma inesperada".into())
                })?;
            self.level.store(0f32.to_bits(), Ordering::SeqCst);
            result
        }

        pub fn cancel(&self) -> Result<()> {
            let mut slot = self
                .active
                .lock()
                .map_err(|_| KoraError::Internal("grabadora de voz bloqueada".into()))?;
            if let Some(mut active) = slot.take() {
                active.stop.store(true, Ordering::SeqCst);
                if let Some(handle) = active.handle.take() {
                    let _ = handle.join();
                }
            }
            self.level.store(0f32.to_bits(), Ordering::SeqCst);
            Ok(())
        }

        pub fn is_recording(&self) -> bool {
            self.active
                .lock()
                .map(|slot| slot.is_some())
                .unwrap_or(false)
        }

        pub fn level(&self) -> f32 {
            f32::from_bits(self.level.load(Ordering::Relaxed)).clamp(0.0, 1.0)
        }
    }

    /// Lista los micrófonos disponibles, con el predeterminado primero.
    pub fn input_devices() -> Result<Vec<String>> {
        use cpal::traits::HostTrait;

        let host = cpal::default_host();
        let default_name = host.default_input_device().map(|device| device.to_string());
        let mut names = Vec::new();
        if let Ok(devices) = host.input_devices() {
            for device in devices {
                let name = device.to_string();
                if !name.trim().is_empty() && !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        if let Some(default_name) = default_name {
            if let Some(index) = names.iter().position(|name| name == &default_name) {
                let name = names.remove(index);
                names.insert(0, name);
            } else if !names.contains(&default_name) {
                names.insert(0, default_name);
            }
        }
        Ok(names)
    }

    pub fn record_to_wav(
        directory: &Path,
        id: &str,
        stop: Arc<AtomicBool>,
        max_seconds: u64,
        device_name: Option<&str>,
        level: &Arc<AtomicU32>,
        enhance: bool,
    ) -> Result<Recording> {
        use cpal::SampleFormat;
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = match device_name {
            Some(name) => host
                .input_devices()
                .map_err(|error| {
                    KoraError::Unsupported(format!("no se pudieron listar los micrófonos: {error}"))
                })?
                .find(|device| device.to_string() == name)
                .ok_or_else(|| {
                    KoraError::NotFound(format!("no se encontró el micrófono «{name}»"))
                })?,
            None => host
                .default_input_device()
                .ok_or_else(|| KoraError::Unsupported("no se encontró un micrófono".into()))?,
        };
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
                let level = Arc::clone(level);
                device
                    .build_input_stream(
                        config,
                        move |data: &[f32], _| {
                            push_mono_f32(
                                data,
                                channels,
                                &samples,
                                source_rate,
                                max_seconds,
                                &level,
                            )
                        },
                        stream_error,
                        None,
                    )
                    .map_err(|error| {
                        KoraError::Unsupported(format!("no se pudo abrir el micrófono: {error}"))
                    })?
            }
            SampleFormat::I16 => {
                let samples = samples.clone();
                let level = Arc::clone(level);
                device
                    .build_input_stream(
                        config,
                        move |data: &[i16], _| {
                            push_mono_i16(
                                data,
                                channels,
                                &samples,
                                source_rate,
                                max_seconds,
                                &level,
                            )
                        },
                        stream_error,
                        None,
                    )
                    .map_err(|error| {
                        KoraError::Unsupported(format!("no se pudo abrir el micrófono: {error}"))
                    })?
            }
            SampleFormat::U16 => {
                let samples = samples.clone();
                let level = Arc::clone(level);
                device
                    .build_input_stream(
                        config,
                        move |data: &[u16], _| {
                            push_mono_u16(
                                data,
                                channels,
                                &samples,
                                source_rate,
                                max_seconds,
                                &level,
                            )
                        },
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
            && started.elapsed() < Duration::from_secs(max_seconds.max(1))
        {
            std::thread::sleep(Duration::from_millis(50));
        }
        drop(stream);

        let mono = samples
            .lock()
            .map_err(|_| KoraError::Internal("buffer de micrófono envenenado".into()))?
            .clone();
        let pcm16k = resample_linear(&mono, source_rate, 16_000);
        let (pcm16k, warnings) = if enhance {
            trim_and_normalize(pcm16k, 16_000)
        } else {
            (pcm16k, Vec::new())
        };
        if pcm16k.is_empty() {
            return Err(KoraError::InvalidInput(
                "la grabación quedó vacía o no se detectó voz; revisa el micrófono".into(),
            ));
        }

        let duration_ms = pcm16k.len() as u64 * 1000 / 16_000;
        let pcm: Vec<i16> = pcm16k
            .iter()
            .map(|sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16)
            .collect();
        let chunk = AudioChunk {
            pcm,
            sample_rate: 16_000,
            channels: 1,
        };
        let wav = crate::tts::encode_wav(&chunk)?;
        let wav_path = directory.join(format!("{id}.wav"));
        std::fs::write(&wav_path, wav)?;

        Ok(Recording {
            samples: pcm16k,
            sample_rate: 16_000,
            duration_ms,
            wav_path,
            warnings,
        })
    }

    fn stream_error(error: cpal::Error) {
        tracing::warn!(%error, "error del micrófono en la muestra de voz");
    }

    fn push_mono_f32(
        data: &[f32],
        channels: usize,
        out: &Arc<Mutex<Vec<f32>>>,
        rate: u32,
        max_seconds: u64,
        level: &AtomicU32,
    ) {
        if let Ok(mut out) = out.lock() {
            let limit = rate as usize * max_seconds as usize;
            if out.len() >= limit {
                return;
            }
            let mut peak: f32 = 0.0;
            if channels <= 1 {
                peak = data
                    .iter()
                    .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
                out.extend_from_slice(data);
            } else {
                for frame in data.chunks(channels) {
                    let value = frame.iter().sum::<f32>() / frame.len() as f32;
                    peak = peak.max(value.abs());
                    out.push(value);
                }
            }
            update_level(level, peak);
        }
    }

    fn push_mono_i16(
        data: &[i16],
        channels: usize,
        out: &Arc<Mutex<Vec<f32>>>,
        rate: u32,
        max_seconds: u64,
        level: &AtomicU32,
    ) {
        if let Ok(mut out) = out.lock() {
            let limit = rate as usize * max_seconds as usize;
            if out.len() >= limit {
                return;
            }
            let mut peak: f32 = 0.0;
            if channels <= 1 {
                for sample in data {
                    let value = *sample as f32 / i16::MAX as f32;
                    peak = peak.max(value.abs());
                    out.push(value);
                }
            } else {
                for frame in data.chunks(channels) {
                    let sum: f32 = frame
                        .iter()
                        .map(|sample| *sample as f32 / i16::MAX as f32)
                        .sum();
                    let value = sum / frame.len() as f32;
                    peak = peak.max(value.abs());
                    out.push(value);
                }
            }
            update_level(level, peak);
        }
    }

    fn push_mono_u16(
        data: &[u16],
        channels: usize,
        out: &Arc<Mutex<Vec<f32>>>,
        rate: u32,
        max_seconds: u64,
        level: &AtomicU32,
    ) {
        if let Ok(mut out) = out.lock() {
            let limit = rate as usize * max_seconds as usize;
            if out.len() >= limit {
                return;
            }
            let mut peak: f32 = 0.0;
            if channels <= 1 {
                for sample in data {
                    let value = (*sample as f32 - 32768.0) / 32768.0;
                    peak = peak.max(value.abs());
                    out.push(value);
                }
            } else {
                for frame in data.chunks(channels) {
                    let sum: f32 = frame
                        .iter()
                        .map(|sample| (*sample as f32 - 32768.0) / 32768.0)
                        .sum();
                    let value = sum / frame.len() as f32;
                    peak = peak.max(value.abs());
                    out.push(value);
                }
            }
            update_level(level, peak);
        }
    }

    fn update_level(level: &AtomicU32, peak: f32) {
        let previous = f32::from_bits(level.load(Ordering::Relaxed)) * 0.82;
        let next = previous.max(peak.clamp(0.0, 1.0));
        level.store(next.to_bits(), Ordering::Relaxed);
    }

    /// Recorta silencios de los extremos y sube el nivel de la muestra. Los
    /// avisos se devuelven como códigos para que la interfaz los traduzca.
    fn trim_and_normalize(mut samples: Vec<f32>, sample_rate: u32) -> (Vec<f32>, Vec<String>) {
        let mut warnings = Vec::new();
        let peak = samples
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        if peak < 0.008 {
            return (Vec::new(), warnings);
        }

        let clipped = samples.iter().filter(|sample| sample.abs() >= 0.98).count();
        if clipped > 0 && clipped * 1000 >= samples.len() * 3 {
            warnings.push("clipped".to_string());
        }

        let threshold = (peak * 0.03).max(0.006);
        let first = samples.iter().position(|sample| sample.abs() >= threshold);
        let last = samples.iter().rposition(|sample| sample.abs() >= threshold);
        if let (Some(first), Some(last)) = (first, last) {
            let margin = (sample_rate as usize * 150) / 1000;
            let start = first.saturating_sub(margin);
            let end = (last + margin + 1).min(samples.len());
            if start > 0 || end < samples.len() {
                warnings.push("trimmed".to_string());
                samples = samples[start..end].to_vec();
            }
        }

        let peak = samples
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        if peak > 0.0 {
            let gain = 0.95 / peak;
            if gain > 1.05 {
                warnings.push("lowLevel".to_string());
            }
            if (gain - 1.0).abs() > 0.01 {
                for sample in &mut samples {
                    *sample = (*sample * gain).clamp(-1.0, 1.0);
                }
            }
        }
        (samples, warnings)
    }

    fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
        if input.is_empty() || from == 0 {
            return Vec::new();
        }
        if from == to {
            return input.to_vec();
        }

        let ratio = from as f64 / to as f64;
        let output_len = ((input.len() as f64) / ratio).round() as usize;
        let mut output = Vec::with_capacity(output_len.max(1));

        for index in 0..output_len.max(1) {
            let source = index as f64 * ratio;
            let left = source.floor() as usize;
            let fraction = (source - left as f64) as f32;
            let a = input.get(left).copied().unwrap_or(0.0);
            let b = input.get(left + 1).copied().unwrap_or(a);
            output.push(a + (b - a) * fraction);
        }

        output
    }
}

#[cfg(target_os = "android")]
mod android {
    use std::path::PathBuf;

    use crate::error::{KoraError, Result};

    use super::Recording;

    #[derive(Default)]
    pub struct SampleRecorder;

    impl SampleRecorder {
        pub fn start(
            &self,
            _directory: PathBuf,
            _max_seconds: u64,
            _device_name: Option<String>,
        ) -> Result<()> {
            Err(KoraError::Unsupported(
                "la grabación de muestras de voz aún no está disponible en Android".into(),
            ))
        }

        pub fn stop(&self) -> Result<Recording> {
            Err(KoraError::Unsupported(
                "la grabación de muestras de voz aún no está disponible en Android".into(),
            ))
        }

        pub fn cancel(&self) -> Result<()> {
            Ok(())
        }

        pub fn is_recording(&self) -> bool {
            false
        }

        pub fn level(&self) -> f32 {
            0.0
        }
    }

    pub fn input_devices() -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}

#[cfg(target_os = "android")]
pub use android::{SampleRecorder, input_devices};
#[cfg(not(target_os = "android"))]
pub use desktop::{SampleRecorder, input_devices, record_to_wav};

/// Borra una grabación temporal solo si está dentro de la carpeta indicada.
pub fn discard_recording(directory: &Path, path: &Path) -> Result<()> {
    if !path.is_file() {
        return Ok(());
    }
    let directory = directory.canonicalize()?;
    let path = path.canonicalize()?;
    if !path.starts_with(&directory) {
        return Err(KoraError::InvalidInput(
            "la ruta no pertenece a las grabaciones de voz".into(),
        ));
    }
    std::fs::remove_file(path)?;
    Ok(())
}
