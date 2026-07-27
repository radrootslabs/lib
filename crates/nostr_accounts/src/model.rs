use radroots_identity::{AccountId, account::Record};
use serde::{Deserialize, Serialize};

pub const RADROOTS_NOSTR_ACCOUNTS_STORE_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadrootsNostrAccountStoreState {
    pub version: u32,
    pub default_account_id: Option<AccountId>,
    pub accounts: Vec<Record>,
}

impl Default for RadrootsNostrAccountStoreState {
    fn default() -> Self {
        Self {
            version: RADROOTS_NOSTR_ACCOUNTS_STORE_VERSION,
            default_account_id: None,
            accounts: Vec::new(),
        }
    }
}
