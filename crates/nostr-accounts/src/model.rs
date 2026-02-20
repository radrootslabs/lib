use radroots_identity::{RadrootsIdentityId, RadrootsIdentityPublic};
use serde::{Deserialize, Serialize};

pub const RADROOTS_NOSTR_ACCOUNTS_STORE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadrootsNostrAccountRecord {
    pub account_id: RadrootsIdentityId,
    pub public_identity: RadrootsIdentityPublic,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub created_at_unix: u64,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadrootsNostrAccountStoreState {
    pub version: u32,
    pub selected_account_id: Option<RadrootsIdentityId>,
    pub accounts: Vec<RadrootsNostrAccountRecord>,
}

impl Default for RadrootsNostrAccountStoreState {
    fn default() -> Self {
        Self {
            version: RADROOTS_NOSTR_ACCOUNTS_STORE_VERSION,
            selected_account_id: None,
            accounts: Vec::new(),
        }
    }
}

impl RadrootsNostrAccountRecord {
    pub fn new(
        public_identity: RadrootsIdentityPublic,
        label: Option<String>,
        created_at_unix: u64,
    ) -> Self {
        let account_id = public_identity.id.clone();
        Self {
            account_id,
            public_identity,
            label,
            created_at_unix,
            updated_at_unix: created_at_unix,
        }
    }

    pub fn touch_updated(&mut self, updated_at_unix: u64) {
        self.updated_at_unix = updated_at_unix;
    }
}
