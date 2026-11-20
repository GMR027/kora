use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{KoraError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ShareScope {
    Library,
    Book(String),
    Voice(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareGrant {
    pub grant_id: String,
    pub peer_id: String,
    pub scope: ShareScope,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Permisos explícitos por par. Sin grant no se comparte ningún asset.
pub trait ShareService: Send + Sync {
    fn grants(&self) -> Result<Vec<ShareGrant>>;
    fn grant(&self, peer_id: &str, scope: ShareScope) -> Result<ShareGrant>;
    fn revoke(&self, grant_id: &str) -> Result<()>;
}

pub struct LocalShareService;

impl ShareService for LocalShareService {
    fn grants(&self) -> Result<Vec<ShareGrant>> {
        Ok(Vec::new())
    }

    fn grant(&self, _peer_id: &str, _scope: ShareScope) -> Result<ShareGrant> {
        Err(KoraError::not_implemented(
            "accounts::LocalShareService::grant (fase 6)",
        ))
    }

    fn revoke(&self, _grant_id: &str) -> Result<()> {
        Err(KoraError::not_implemented(
            "accounts::LocalShareService::revoke",
        ))
    }
}
