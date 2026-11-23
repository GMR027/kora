use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{KoraError, Result};
use crate::storage::ResourceType;

pub const PROTOCOL_VERSION: u32 = 2;
pub const ALPN: &[u8] = b"kora/1";
const MAX_JSON_FRAME: usize = 16 * 1024 * 1024;
const MAX_BINARY_FRAME: usize = 4 * 1024 * 1024;
pub const ASSET_CHUNK: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Pair,
    Share,
    Sync,
    Mirror,
    Handoff,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub version: u32,
    pub device_id: String,
    pub account_id: String,
    pub account_hint: String,
    pub username: String,
    pub account_public_key: String,
    pub device_name: String,
    pub platform: String,
    pub listen_port: u16,
    pub public_key: String,
    pub fingerprint: String,
    pub sas_prefix: String,
    pub purpose: Purpose,
    pub nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRef {
    pub kind: ResourceType,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResource {
    pub reference: ResourceRef,
    pub hash: String,
    pub updated_at: DateTime<Utc>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub device_id: String,
    pub account_hint: String,
    pub resources: Vec<SyncResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Handoff {
    pub book_id: String,
    pub chapter: usize,
    pub offset: usize,
    pub percent: f64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorTtsState {
    Playing,
    Paused,
    Stopped,
    Finished,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Message {
    Hello(Hello),
    Error {
        message: String,
    },
    Manifest(Manifest),
    Pull {
        resources: Vec<ResourceRef>,
    },
    PullDone,
    ReadyForPull,
    ReadyServe,
    ResourceStart {
        resource: ResourceRef,
        hash: String,
        payload: serde_json::Value,
    },
    AssetStart {
        resource: ResourceRef,
        asset: String,
        hash: String,
        size: u64,
    },
    AssetChunk {
        resource: ResourceRef,
        asset: String,
        offset: u64,
    },
    AssetEnd {
        resource: ResourceRef,
        asset: String,
    },
    ResourceEnd {
        resource: ResourceRef,
    },
    Ack {
        resources: Vec<ResourceRef>,
    },
    Spake2A {
        payload: String,
    },
    Spake2B {
        payload: String,
        sas: String,
    },
    SasConfirm {
        sas: String,
    },
    PairRequest {
        ticket_id: String,
    },
    PairRejected {
        reason: String,
        #[serde(default)]
        retryable: bool,
    },
    ShareRequest {
        key_hash: String,
    },
    ShareGrant {
        payload: serde_json::Value,
    },
    Handoff(Handoff),
    MirrorJoin {
        book_id: String,
    },
    MirrorPosition {
        book_id: String,
        chapter: usize,
        offset: usize,
    },
    MirrorTts {
        book_id: String,
        section: usize,
        state: MirrorTtsState,
        offset: usize,
        #[serde(default)]
        end: Option<usize>,
    },
    MirrorStop {
        book_id: String,
    },
}

pub async fn write_frame<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    message: &Message,
    binary: &[u8],
) -> Result<()> {
    if binary.len() > MAX_BINARY_FRAME {
        return Err(KoraError::InvalidInput(
            "frame binario demasiado grande".into(),
        ));
    }
    let json = serde_json::to_vec(message)?;
    if json.len() > MAX_JSON_FRAME {
        return Err(KoraError::InvalidInput(
            "frame JSON demasiado grande".into(),
        ));
    }
    writer.write_u32(json.len() as u32).await?;
    writer.write_all(&json).await?;
    writer.write_u32(binary.len() as u32).await?;
    writer.write_all(binary).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read_frame<R: AsyncReadExt + Unpin>(
    reader: &mut R,
) -> Result<Option<(Message, Vec<u8>)>> {
    let json_len = match reader.read_u32().await {
        Ok(value) => value as usize,
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if json_len == 0 || json_len > MAX_JSON_FRAME {
        return Err(KoraError::InvalidInput(
            "longitud de frame JSON inválida".into(),
        ));
    }
    let mut json = vec![0u8; json_len];
    reader.read_exact(&mut json).await?;
    let binary_len = reader.read_u32().await? as usize;
    if binary_len > MAX_BINARY_FRAME {
        return Err(KoraError::InvalidInput(
            "longitud de frame binario inválida".into(),
        ));
    }
    let mut binary = vec![0u8; binary_len];
    if binary_len > 0 {
        reader.read_exact(&mut binary).await?;
    }
    let message = serde_json::from_slice(&json)
        .map_err(|error| KoraError::InvalidInput(format!("frame desconocido: {error}")))?;
    Ok(Some((message, binary)))
}

pub fn sas_matches(expected: &str, received: &str) -> bool {
    expected.eq_ignore_ascii_case(received.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_tts_accepts_messages_without_an_excerpt_end() {
        let message: Message = serde_json::from_str(
            r#"{"kind":"mirror_tts","book_id":"b","section":1,"state":"playing","offset":10}"#,
        )
        .expect("mensaje compatible");

        match message {
            Message::MirrorTts { end, .. } => assert_eq!(end, None),
            _ => panic!("se esperaba mirror_tts"),
        }
    }

    #[test]
    fn mirror_tts_keeps_the_excerpt_end() {
        let message: Message = serde_json::from_str(
            r#"{"kind":"mirror_tts","book_id":"b","section":1,"state":"playing","offset":10,"end":20}"#,
        )
        .expect("mensaje con extracto");

        match message {
            Message::MirrorTts { end, .. } => assert_eq!(end, Some(20)),
            _ => panic!("se esperaba mirror_tts"),
        }
    }
}
