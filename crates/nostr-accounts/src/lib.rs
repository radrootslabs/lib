#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

pub mod error;
#[cfg(feature = "std")]
pub mod manager;
#[cfg(feature = "std")]
pub mod model;
#[cfg(feature = "ndb-bridge")]
pub mod ndb_bridge;
#[cfg(feature = "std")]
pub mod store;
#[cfg(feature = "std")]
pub mod vault;

pub mod prelude {
    pub use crate::error::RadrootsNostrAccountsError;
    #[cfg(feature = "std")]
    pub use crate::manager::RadrootsNostrAccountsManager;
    #[cfg(feature = "std")]
    pub use crate::model::{
        RADROOTS_NOSTR_ACCOUNTS_STORE_VERSION, RadrootsNostrAccountRecord,
        RadrootsNostrAccountStoreState,
    };
    #[cfg(feature = "ndb-bridge")]
    pub use crate::ndb_bridge::radroots_nostr_accounts_register_selected_secret_with_ndb;
    #[cfg(feature = "std")]
    pub use crate::store::{
        RadrootsNostrAccountStore, RadrootsNostrFileAccountStore, RadrootsNostrMemoryAccountStore,
    };
    #[cfg(feature = "os-keyring")]
    pub use crate::vault::RadrootsNostrSecretVaultOsKeyring;
    #[cfg(feature = "std")]
    pub use crate::vault::{RadrootsNostrSecretVault, RadrootsNostrSecretVaultMemory};
}
