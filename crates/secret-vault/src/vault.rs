use alloc::{format, string::String};

use crate::error::RadrootsSecretVaultAccessError;

pub trait RadrootsSecretVault: Send + Sync {
    fn store_secret(&self, slot: &str, secret: &str) -> Result<(), RadrootsSecretVaultAccessError>;

    fn load_secret(&self, slot: &str) -> Result<Option<String>, RadrootsSecretVaultAccessError>;

    fn remove_secret(&self, slot: &str) -> Result<(), RadrootsSecretVaultAccessError>;
}

#[cfg(feature = "memory-vault")]
#[derive(Debug, Clone, Default)]
pub struct RadrootsSecretVaultMemory {
    entries: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, String>>>,
}

#[cfg(feature = "memory-vault")]
impl RadrootsSecretVaultMemory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "memory-vault")]
impl RadrootsSecretVault for RadrootsSecretVaultMemory {
    fn store_secret(&self, slot: &str, secret: &str) -> Result<(), RadrootsSecretVaultAccessError> {
        let mut guard = self
            .entries
            .write()
            .map_err(|_| RadrootsSecretVaultAccessError::Backend("memory vault poisoned".into()))?;
        guard.insert(String::from(slot), String::from(secret));
        Ok(())
    }

    fn load_secret(&self, slot: &str) -> Result<Option<String>, RadrootsSecretVaultAccessError> {
        let guard = self
            .entries
            .read()
            .map_err(|_| RadrootsSecretVaultAccessError::Backend("memory vault poisoned".into()))?;
        Ok(guard.get(slot).cloned())
    }

    fn remove_secret(&self, slot: &str) -> Result<(), RadrootsSecretVaultAccessError> {
        let mut guard = self
            .entries
            .write()
            .map_err(|_| RadrootsSecretVaultAccessError::Backend("memory vault poisoned".into()))?;
        guard.remove(slot);
        Ok(())
    }
}

#[cfg(feature = "os-keyring")]
#[derive(Debug, Clone)]
pub struct RadrootsSecretVaultOsKeyring {
    service_name: String,
}

#[cfg(feature = "os-keyring")]
impl RadrootsSecretVaultOsKeyring {
    #[must_use]
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }
}

#[cfg(feature = "os-keyring")]
impl Default for RadrootsSecretVaultOsKeyring {
    fn default() -> Self {
        Self::new("org.radroots.secret-vault")
    }
}

#[cfg(feature = "os-keyring")]
impl RadrootsSecretVault for RadrootsSecretVaultOsKeyring {
    fn store_secret(&self, slot: &str, secret: &str) -> Result<(), RadrootsSecretVaultAccessError> {
        let entry = keyring::Entry::new(self.service_name.as_str(), slot)
            .map_err(|source| RadrootsSecretVaultAccessError::Backend(format!("{source}")))?;
        entry
            .set_password(secret)
            .map_err(|source| RadrootsSecretVaultAccessError::Backend(format!("{source}")))
    }

    fn load_secret(&self, slot: &str) -> Result<Option<String>, RadrootsSecretVaultAccessError> {
        let entry = keyring::Entry::new(self.service_name.as_str(), slot)
            .map_err(|source| RadrootsSecretVaultAccessError::Backend(format!("{source}")))?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(source) => Err(RadrootsSecretVaultAccessError::Backend(format!("{source}"))),
        }
    }

    fn remove_secret(&self, slot: &str) -> Result<(), RadrootsSecretVaultAccessError> {
        let entry = keyring::Entry::new(self.service_name.as_str(), slot)
            .map_err(|source| RadrootsSecretVaultAccessError::Backend(format!("{source}")))?;
        match entry.delete_credential() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(source) => Err(RadrootsSecretVaultAccessError::Backend(format!("{source}"))),
        }
    }
}

#[cfg(all(test, feature = "memory-vault"))]
mod tests {
    use super::*;

    #[test]
    fn memory_vault_round_trip() {
        let vault = RadrootsSecretVaultMemory::new();
        vault.store_secret("alice", "abc123").expect("store");
        let loaded = vault.load_secret("alice").expect("load");
        assert_eq!(loaded.as_deref(), Some("abc123"));
        vault.remove_secret("alice").expect("remove");
        let loaded = vault.load_secret("alice").expect("load");
        assert!(loaded.is_none());
    }
}
