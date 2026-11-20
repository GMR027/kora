use super::engine::AudioChunk;
use crate::error::Result;

/// Destino de audio: altavoz, archivo WAV o buffer para el frontend.
pub trait AudioSink: Send {
    fn write(&mut self, chunk: &AudioChunk) -> Result<()>;
    fn finish(&mut self) -> Result<()>;
}

/// Sink de prueba: descarta el audio. Útil para comandos y tests.
pub struct NullAudioSink;

impl AudioSink for NullAudioSink {
    fn write(&mut self, _chunk: &AudioChunk) -> Result<()> {
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}
