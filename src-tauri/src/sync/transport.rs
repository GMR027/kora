use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::discovery::PeerInfo;
use crate::error::{KoraError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Control,
    Crdt,
    Assets,
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub channel: Channel,
    pub payload: Vec<u8>,
}

#[async_trait]
pub trait PeerConnection: Send + Sync {
    fn remote_id(&self) -> &str;
    async fn send(&self, frame: Frame) -> Result<()>;
    async fn recv(&mut self) -> Result<Option<Frame>>;
    async fn close(&self) -> Result<()>;
}

/// Transporte entre pares. QUIC aporta multiplexación, TLS 1.3 y reconexión
/// sobre UDP; los certificados se generan por dispositivo en la fase 6.
#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&self, peer: &PeerInfo) -> Result<Box<dyn PeerConnection>>;
    async fn accept(&self) -> Result<Box<dyn PeerConnection>>;
}

pub struct QuinnTransport {
    #[allow(dead_code)]
    endpoint: Option<quinn::Endpoint>,
}

impl QuinnTransport {
    pub fn new() -> Self {
        Self { endpoint: None }
    }
}

impl Default for QuinnTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for QuinnTransport {
    async fn connect(&self, _peer: &PeerInfo) -> Result<Box<dyn PeerConnection>> {
        Err(KoraError::not_implemented(
            "sync::QuinnTransport::connect (fase 6)",
        ))
    }

    async fn accept(&self) -> Result<Box<dyn PeerConnection>> {
        Err(KoraError::not_implemented(
            "sync::QuinnTransport::accept (fase 6)",
        ))
    }
}
