use radroots_identity::RadrootsIdentityId;

#[cfg(feature = "memory-vault")]
pub use radroots_secret_vault::RadrootsSecretVaultMemory as RadrootsNostrSecretVaultMemory;
#[cfg(feature = "os-keyring")]
pub use radroots_secret_vault::RadrootsSecretVaultOsKeyring as RadrootsNostrSecretVaultOsKeyring;
pub use radroots_secret_vault::{RadrootsSecretVault, RadrootsSecretVaultAccessError};

#[must_use]
pub fn account_secret_slot(account_id: &RadrootsIdentityId) -> String {
    account_id.to_string()
}
