pub mod crypto;
pub mod identity;
pub mod pairing;
pub mod service;
pub mod sharing;

pub use identity::{Identity, IdentityProvider, LocalIdentityProvider, Signature};
pub use pairing::{PairedDevice, PairingService, PairingTicket, QrPairingService};
pub use service::{
    AccountService, AccountView, ConnectionKeyView, ShareCreateRequest, ShareView, UnlockedAccount,
};
pub use sharing::{LocalShareService, ShareGrant, ShareScope, ShareService};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub signed_in: bool,
    pub account_id: Option<String>,
    pub display_name: Option<String>,
    pub device_count: usize,
}
