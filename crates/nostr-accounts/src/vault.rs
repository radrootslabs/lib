use crate::error::RadrootsNostrAccountsError;
use radroots_identity::RadrootsIdentityId;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub trait RadrootsNostrSecretVault: Send + Sync {
    fn store_secret_hex(
        &self,
        account_id: &RadrootsIdentityId,
        secret_key_hex: &str,
    ) -> Result<(), RadrootsNostrAccountsError>;
    fn load_secret_hex(
        &self,
        account_id: &RadrootsIdentityId,
    ) -> Result<Option<String>, RadrootsNostrAccountsError>;
    fn remove_secret(
        &self,
        account_id: &RadrootsIdentityId,
    ) -> Result<(), RadrootsNostrAccountsError>;
}

#[derive(Debug, Clone, Default)]
pub struct RadrootsNostrSecretVaultMemory {
    entries: Arc<RwLock<HashMap<String, String>>>,
}

impl RadrootsNostrSecretVaultMemory {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RadrootsNostrSecretVault for RadrootsNostrSecretVaultMemory {
    fn store_secret_hex(
        &self,
        account_id: &RadrootsIdentityId,
        secret_key_hex: &str,
    ) -> Result<(), RadrootsNostrAccountsError> {
        let mut guard = self
            .entries
            .write()
            .map_err(|_| RadrootsNostrAccountsError::Vault("memory vault poisoned".into()))?;
        guard.insert(account_id.to_string(), secret_key_hex.to_owned());
        Ok(())
    }

    fn load_secret_hex(
        &self,
        account_id: &RadrootsIdentityId,
    ) -> Result<Option<String>, RadrootsNostrAccountsError> {
        let guard = self
            .entries
            .read()
            .map_err(|_| RadrootsNostrAccountsError::Vault("memory vault poisoned".into()))?;
        Ok(guard.get(account_id.as_str()).cloned())
    }

    fn remove_secret(
        &self,
        account_id: &RadrootsIdentityId,
    ) -> Result<(), RadrootsNostrAccountsError> {
        let mut guard = self
            .entries
            .write()
            .map_err(|_| RadrootsNostrAccountsError::Vault("memory vault poisoned".into()))?;
        guard.remove(account_id.as_str());
        Ok(())
    }
}

#[cfg(feature = "os-keyring")]
#[derive(Debug, Clone)]
pub struct RadrootsNostrSecretVaultOsKeyring {
    service_name: String,
}

#[cfg(feature = "os-keyring")]
impl RadrootsNostrSecretVaultOsKeyring {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }
}

#[cfg(feature = "os-keyring")]
impl Default for RadrootsNostrSecretVaultOsKeyring {
    fn default() -> Self {
        Self::new("org.radroots.nostr.accounts")
    }
}

#[cfg(feature = "os-keyring")]
impl RadrootsNostrSecretVault for RadrootsNostrSecretVaultOsKeyring {
    fn store_secret_hex(
        &self,
        account_id: &RadrootsIdentityId,
        secret_key_hex: &str,
    ) -> Result<(), RadrootsNostrAccountsError> {
        let entry = keyring::Entry::new(self.service_name.as_str(), account_id.as_str())
            .map_err(|source| RadrootsNostrAccountsError::Vault(source.to_string()))?;
        entry
            .set_password(secret_key_hex)
            .map_err(|source| RadrootsNostrAccountsError::Vault(source.to_string()))
    }

    fn load_secret_hex(
        &self,
        account_id: &RadrootsIdentityId,
    ) -> Result<Option<String>, RadrootsNostrAccountsError> {
        let entry = keyring::Entry::new(self.service_name.as_str(), account_id.as_str())
            .map_err(|source| RadrootsNostrAccountsError::Vault(source.to_string()))?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(source) => Err(RadrootsNostrAccountsError::Vault(source.to_string())),
        }
    }

    fn remove_secret(
        &self,
        account_id: &RadrootsIdentityId,
    ) -> Result<(), RadrootsNostrAccountsError> {
        let entry = keyring::Entry::new(self.service_name.as_str(), account_id.as_str())
            .map_err(|source| RadrootsNostrAccountsError::Vault(source.to_string()))?;
        match entry.delete_password() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(source) => Err(RadrootsNostrAccountsError::Vault(source.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_identity::RadrootsIdentityId;

    #[test]
    fn memory_vault_round_trip() {
        let vault = RadrootsNostrSecretVaultMemory::new();
        let account_id = RadrootsIdentityId::parse(
            "3bf0c63f0f4478a288f6b67f0429dbf7f5119d4fa7218a4c40ef1378f80f7606",
        )
        .expect("account id");
        vault
            .store_secret_hex(&account_id, "abc123")
            .expect("store");
        let loaded = vault.load_secret_hex(&account_id).expect("load");
        assert_eq!(loaded.as_deref(), Some("abc123"));
        vault.remove_secret(&account_id).expect("remove");
        let loaded = vault.load_secret_hex(&account_id).expect("load");
        assert!(loaded.is_none());
    }
}
