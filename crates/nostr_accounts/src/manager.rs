use crate::error::RadrootsNostrAccountsError;
use crate::model::{
    RadrootsNostrAccountRecord, RadrootsNostrAccountStatus, RadrootsNostrAccountStoreState,
};
#[cfg(feature = "memory-vault")]
use crate::store::RadrootsNostrMemoryAccountStore;
use crate::store::{RadrootsNostrAccountStore, RadrootsNostrFileAccountStore};
#[cfg(feature = "memory-vault")]
use crate::vault::RadrootsNostrSecretVaultMemory;
#[cfg(feature = "os-keyring")]
use crate::vault::RadrootsNostrSecretVaultOsKeyring;
use crate::vault::{RadrootsSecretVault, account_secret_slot};
use radroots_identity::{RadrootsIdentity, RadrootsIdentityId, RadrootsIdentityPublic};
use radroots_nostr_signer::prelude::{
    RadrootsNostrLocalSignerAvailability, RadrootsNostrLocalSignerCapability,
    RadrootsNostrSignerCapability,
};
use radroots_protected_store::RadrootsProtectedFileSecretVault;
use radroots_secret_vault::{
    RadrootsResolvedSecretBackend, RadrootsSecretBackend, RadrootsSecretBackendAvailability,
    RadrootsSecretBackendSelection, RadrootsSecretVaultError,
};
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct RadrootsNostrAccountsManager {
    store: Arc<dyn RadrootsNostrAccountStore>,
    vault: Arc<dyn RadrootsSecretVault>,
    state: Arc<RwLock<RadrootsNostrAccountStoreState>>,
}

impl RadrootsNostrAccountsManager {
    #[cfg(feature = "memory-vault")]
    pub fn new_in_memory() -> Self {
        Self {
            store: Arc::new(RadrootsNostrMemoryAccountStore::new()),
            vault: Arc::new(RadrootsNostrSecretVaultMemory::new()),
            state: Arc::new(RwLock::new(RadrootsNostrAccountStoreState::default())),
        }
    }

    pub fn new(
        store: Arc<dyn RadrootsNostrAccountStore>,
        vault: Arc<dyn RadrootsSecretVault>,
    ) -> Result<Self, RadrootsNostrAccountsError> {
        let mut state = store.load()?;
        let mut state_dirty = match state.version {
            1 => {
                state.version = crate::model::RADROOTS_NOSTR_ACCOUNTS_STORE_VERSION;
                true
            }
            crate::model::RADROOTS_NOSTR_ACCOUNTS_STORE_VERSION => false,
            _ => {
                return Err(RadrootsNostrAccountsError::InvalidState(format!(
                    "unsupported accounts schema version {}",
                    state.version
                )));
            }
        };

        if let Some(default_account_id) = state.default_account_id.clone() {
            let exists = state
                .accounts
                .iter()
                .any(|record| record.account_id == default_account_id);
            if !exists {
                state.default_account_id = None;
                state_dirty = true;
            }
        }

        if state_dirty {
            store.save(&state)?;
        }

        Ok(Self {
            store,
            vault,
            state: Arc::new(RwLock::new(state)),
        })
    }

    pub fn new_file_backed(
        path: impl AsRef<Path>,
        vault: Arc<dyn RadrootsSecretVault>,
    ) -> Result<Self, RadrootsNostrAccountsError> {
        Self::new(
            Arc::new(RadrootsNostrFileAccountStore::new(path.as_ref())),
            vault,
        )
    }

    pub fn new_file_backed_with_vault<V>(
        path: impl AsRef<Path>,
        vault: V,
    ) -> Result<Self, RadrootsNostrAccountsError>
    where
        V: RadrootsSecretVault + 'static,
    {
        Self::new_file_backed(path, Arc::new(vault))
    }

    pub fn resolve_local_backend(
        selection: RadrootsSecretBackendSelection,
        availability: RadrootsSecretBackendAvailability,
    ) -> Result<RadrootsResolvedSecretBackend, RadrootsSecretVaultError> {
        selection.resolve(availability)
    }

    pub fn new_local_file_backed(
        path: impl AsRef<Path>,
        secrets_dir: impl AsRef<Path>,
        selection: RadrootsSecretBackendSelection,
        availability: RadrootsSecretBackendAvailability,
        host_vault_service_name: impl Into<String>,
    ) -> Result<(Self, RadrootsResolvedSecretBackend), RadrootsNostrAccountsError> {
        let resolved = Self::resolve_local_backend(selection, availability)
            .map_err(|error| RadrootsNostrAccountsError::Vault(error.to_string()))?;
        let vault = local_file_backed_secret_vault(
            resolved.backend,
            secrets_dir.as_ref(),
            host_vault_service_name.into(),
        )?;
        let manager = Self::new_file_backed(path, vault)?;
        Ok((manager, resolved))
    }

    pub fn list_accounts(
        &self,
    ) -> Result<Vec<RadrootsNostrAccountRecord>, RadrootsNostrAccountsError> {
        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        Ok(guard.accounts.clone())
    }

    pub fn default_account_id(
        &self,
    ) -> Result<Option<RadrootsIdentityId>, RadrootsNostrAccountsError> {
        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        Ok(guard.default_account_id.clone())
    }

    pub fn default_account(
        &self,
    ) -> Result<Option<RadrootsNostrAccountRecord>, RadrootsNostrAccountsError> {
        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        let Some(default_account_id) = guard.default_account_id.as_ref() else {
            return Ok(None);
        };
        Ok(guard
            .accounts
            .iter()
            .find(|record| &record.account_id == default_account_id)
            .cloned())
    }

    pub fn default_public_identity(
        &self,
    ) -> Result<Option<RadrootsIdentityPublic>, RadrootsNostrAccountsError> {
        Ok(self
            .default_account()?
            .map(|record| record.public_identity.clone()))
    }

    pub fn default_account_status(
        &self,
    ) -> Result<RadrootsNostrAccountStatus, RadrootsNostrAccountsError> {
        let Some(record) = self.default_account()? else {
            return Ok(RadrootsNostrAccountStatus::NotConfigured);
        };

        Ok(match self.local_signer_availability(&record)? {
            RadrootsNostrLocalSignerAvailability::PublicOnly => {
                RadrootsNostrAccountStatus::PublicOnly { account: record }
            }
            RadrootsNostrLocalSignerAvailability::SecretBacked => {
                RadrootsNostrAccountStatus::Ready { account: record }
            }
        })
    }

    pub fn default_signing_identity(
        &self,
    ) -> Result<Option<RadrootsIdentity>, RadrootsNostrAccountsError> {
        let Some(record) = self.default_account()? else {
            return Ok(None);
        };
        self.resolve_signing_identity(record)
    }

    pub fn get_signing_identity(
        &self,
        account_id: &RadrootsIdentityId,
    ) -> Result<Option<RadrootsIdentity>, RadrootsNostrAccountsError> {
        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        let Some(record) = guard
            .accounts
            .iter()
            .find(|record| &record.account_id == account_id)
            .cloned()
        else {
            return Ok(None);
        };
        drop(guard);
        self.resolve_signing_identity(record)
    }

    pub fn default_signer_capability(
        &self,
    ) -> Result<Option<RadrootsNostrSignerCapability>, RadrootsNostrAccountsError> {
        let Some(record) = self.default_account()? else {
            return Ok(None);
        };
        Ok(Some(self.local_signer_capability(record)?))
    }

    pub fn get_signer_capability(
        &self,
        account_id: &RadrootsIdentityId,
    ) -> Result<Option<RadrootsNostrSignerCapability>, RadrootsNostrAccountsError> {
        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        let Some(record) = guard
            .accounts
            .iter()
            .find(|record| &record.account_id == account_id)
            .cloned()
        else {
            return Ok(None);
        };
        drop(guard);
        Ok(Some(self.local_signer_capability(record)?))
    }

    pub fn resolve_signing_identity_for_signer(
        &self,
        signer: &RadrootsNostrSignerCapability,
    ) -> Result<Option<RadrootsIdentity>, RadrootsNostrAccountsError> {
        match signer {
            RadrootsNostrSignerCapability::LocalAccount(capability) => {
                self.get_signing_identity(&capability.account_id)
            }
            RadrootsNostrSignerCapability::RemoteSession(_) => Ok(None),
        }
    }

    pub fn upsert_identity(
        &self,
        identity: &RadrootsIdentity,
        label: Option<String>,
        make_default: bool,
    ) -> Result<RadrootsIdentityId, RadrootsNostrAccountsError> {
        let account_id = identity.id();
        let secret_key_hex = Zeroizing::new(identity.secret_key_hex());
        self.vault.store_secret(
            account_secret_slot(&account_id).as_str(),
            secret_key_hex.as_str(),
        )?;

        let public_identity = identity.to_public();
        self.upsert_public_identity(public_identity, label, make_default)
    }

    pub fn upsert_public_identity(
        &self,
        public_identity: RadrootsIdentityPublic,
        label: Option<String>,
        make_default: bool,
    ) -> Result<RadrootsIdentityId, RadrootsNostrAccountsError> {
        let updated_at_unix = now_unix_secs();
        let account_id = public_identity.id.clone();
        self.update_state(|state| {
            if public_identity.id.as_str() != public_identity.public_key_hex {
                return Err(RadrootsNostrAccountsError::InvalidState(
                    "public identity id does not match public key".into(),
                ));
            }
            if let Some(existing) = state
                .accounts
                .iter_mut()
                .find(|record| record.account_id == account_id)
            {
                existing.public_identity = public_identity.clone();
                if let Some(next_label) = label.clone() {
                    existing.label = Some(next_label);
                }
                existing.touch_updated(updated_at_unix);
            } else {
                state.accounts.push(RadrootsNostrAccountRecord::new(
                    public_identity.clone(),
                    label.clone(),
                    updated_at_unix,
                ));
            }

            if state.default_account_id.is_none() || make_default {
                state.default_account_id = Some(account_id.clone());
            }
            Ok(())
        })?;
        Ok(account_id)
    }

    pub fn generate_identity(
        &self,
        label: Option<String>,
        make_default: bool,
    ) -> Result<RadrootsIdentityId, RadrootsNostrAccountsError> {
        let identity = RadrootsIdentity::generate();
        self.upsert_identity(&identity, label, make_default)
    }

    pub fn set_default_account(
        &self,
        account_id: &RadrootsIdentityId,
    ) -> Result<(), RadrootsNostrAccountsError> {
        let account_id = account_id.clone();
        self.update_state(|state| {
            let exists = state
                .accounts
                .iter()
                .any(|record| record.account_id == account_id);
            if !exists {
                return Err(RadrootsNostrAccountsError::AccountNotFound(
                    account_id.to_string(),
                ));
            }
            state.default_account_id = Some(account_id);
            Ok(())
        })
    }

    pub fn clear_default_account(&self) -> Result<(), RadrootsNostrAccountsError> {
        self.update_state(|state| {
            state.default_account_id = None;
            Ok(())
        })
    }

    pub fn resolve_account_selector(
        &self,
        selector: &str,
    ) -> Result<RadrootsNostrAccountRecord, RadrootsNostrAccountsError> {
        let normalized = selector.trim();
        if normalized.is_empty() {
            return Err(RadrootsNostrAccountsError::InvalidAccountSelector(
                "account selector cannot be empty".to_owned(),
            ));
        }

        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        if let Some(record) = guard
            .accounts
            .iter()
            .find(|record| {
                record.account_id.as_str() == normalized
                    || record.public_identity.public_key_npub == normalized
            })
            .cloned()
        {
            return Ok(record);
        }

        let mut label_matches = guard
            .accounts
            .iter()
            .filter(|record| record.label.as_deref() == Some(normalized))
            .cloned();
        let Some(record) = label_matches.next() else {
            return Err(RadrootsNostrAccountsError::AccountNotFound(
                normalized.to_owned(),
            ));
        };
        if label_matches.next().is_some() {
            return Err(RadrootsNostrAccountsError::AmbiguousAccountSelector(
                normalized.to_owned(),
            ));
        }
        Ok(record)
    }

    pub fn remove_account(
        &self,
        account_id: &RadrootsIdentityId,
    ) -> Result<(), RadrootsNostrAccountsError> {
        let account_id = account_id.clone();
        self.update_state(|state| {
            let before = state.accounts.len();
            state
                .accounts
                .retain(|record| record.account_id != account_id);
            if state.accounts.len() == before {
                return Err(RadrootsNostrAccountsError::AccountNotFound(
                    account_id.to_string(),
                ));
            }

            if state.default_account_id.as_ref() == Some(&account_id) {
                state.default_account_id = None;
            }
            Ok(())
        })?;
        self.vault
            .remove_secret(account_secret_slot(&account_id).as_str())?;
        Ok(())
    }

    pub fn export_secret_hex(
        &self,
        account_id: &RadrootsIdentityId,
    ) -> Result<Option<String>, RadrootsNostrAccountsError> {
        self.vault
            .load_secret(account_secret_slot(account_id).as_str())
            .map_err(Into::into)
    }

    pub fn migrate_legacy_identity_file(
        &self,
        path: impl AsRef<Path>,
        label: Option<String>,
        make_default: bool,
    ) -> Result<RadrootsIdentityId, RadrootsNostrAccountsError> {
        let identity = RadrootsIdentity::load_from_path_auto(path)?;
        self.upsert_identity(&identity, label, make_default)
    }

    fn resolve_signing_identity(
        &self,
        record: RadrootsNostrAccountRecord,
    ) -> Result<Option<RadrootsIdentity>, RadrootsNostrAccountsError> {
        let Some(secret_key_hex) = self
            .vault
            .load_secret(account_secret_slot(&record.account_id).as_str())?
        else {
            return Ok(None);
        };
        let secret_key_hex = Zeroizing::new(secret_key_hex);
        let mut identity = RadrootsIdentity::from_secret_key_str(secret_key_hex.as_str())?;
        if identity.public_key_hex() != record.public_identity.public_key_hex {
            return Err(RadrootsNostrAccountsError::PublicKeyMismatch);
        }
        if let Some(profile) = record.public_identity.profile {
            identity.set_profile(profile);
        }
        Ok(Some(identity))
    }

    fn local_signer_capability(
        &self,
        record: RadrootsNostrAccountRecord,
    ) -> Result<RadrootsNostrSignerCapability, RadrootsNostrAccountsError> {
        let availability = self.local_signer_availability(&record)?;
        Ok(RadrootsNostrSignerCapability::LocalAccount(
            RadrootsNostrLocalSignerCapability::new(
                record.account_id,
                record.public_identity,
                availability,
            ),
        ))
    }

    fn local_signer_availability(
        &self,
        record: &RadrootsNostrAccountRecord,
    ) -> Result<RadrootsNostrLocalSignerAvailability, RadrootsNostrAccountsError> {
        let Some(secret_key_hex) = self
            .vault
            .load_secret(account_secret_slot(&record.account_id).as_str())?
        else {
            return Ok(RadrootsNostrLocalSignerAvailability::PublicOnly);
        };

        let secret_key_hex = Zeroizing::new(secret_key_hex);
        let identity = RadrootsIdentity::from_secret_key_str(secret_key_hex.as_str())?;
        if identity.public_key_hex() != record.public_identity.public_key_hex {
            return Err(RadrootsNostrAccountsError::PublicKeyMismatch);
        }
        Ok(RadrootsNostrLocalSignerAvailability::SecretBacked)
    }

    fn update_state(
        &self,
        update: impl FnOnce(
            &mut RadrootsNostrAccountStoreState,
        ) -> Result<(), RadrootsNostrAccountsError>,
    ) -> Result<(), RadrootsNostrAccountsError> {
        let mut guard = self.state.write().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        let mut next = guard.clone();
        update(&mut next)?;
        self.store.save(&next)?;
        *guard = next;
        Ok(())
    }
}

fn local_file_backed_secret_vault(
    backend: RadrootsSecretBackend,
    secrets_dir: &Path,
    _host_vault_service_name: String,
) -> Result<Arc<dyn RadrootsSecretVault>, RadrootsNostrAccountsError> {
    match backend {
        #[cfg(feature = "os-keyring")]
        RadrootsSecretBackend::HostVault(_) => Ok(Arc::new(
            RadrootsNostrSecretVaultOsKeyring::new(_host_vault_service_name),
        )),
        #[cfg(not(feature = "os-keyring"))]
        RadrootsSecretBackend::HostVault(_) => Err(RadrootsNostrAccountsError::Vault(
            "host_vault backend requires radroots_nostr_accounts os-keyring support".into(),
        )),
        RadrootsSecretBackend::EncryptedFile => {
            Ok(Arc::new(RadrootsProtectedFileSecretVault::new(secrets_dir)))
        }
        #[cfg(feature = "memory-vault")]
        RadrootsSecretBackend::Memory => Ok(Arc::new(RadrootsNostrSecretVaultMemory::new())),
        #[cfg(not(feature = "memory-vault"))]
        RadrootsSecretBackend::Memory => Err(RadrootsNostrAccountsError::Vault(
            "memory backend requires radroots_nostr_accounts memory-vault support".into(),
        )),
        RadrootsSecretBackend::ExternalCommand => Err(RadrootsNostrAccountsError::Vault(
            "external_command secret backend is not supported for local accounts".into(),
        )),
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{
        RadrootsNostrAccountStore, RadrootsNostrFileAccountStore, RadrootsNostrMemoryAccountStore,
    };
    use crate::vault::RadrootsNostrSecretVaultMemory;
    use crate::vault::RadrootsSecretVault;
    use radroots_identity::RadrootsIdentityProfile;
    use radroots_secret_vault::{
        RadrootsHostVaultCapabilities, RadrootsSecretBackend, RadrootsSecretBackendAvailability,
        RadrootsSecretBackendSelection,
    };
    use serde_json::json;
    use std::fs;
    use std::sync::Arc;
    use std::sync::RwLock;
    use std::thread;

    struct LoadErrorStore;

    impl RadrootsNostrAccountStore for LoadErrorStore {
        fn load(&self) -> Result<RadrootsNostrAccountStoreState, RadrootsNostrAccountsError> {
            Err(RadrootsNostrAccountsError::Store(
                "store load failed".into(),
            ))
        }

        fn save(
            &self,
            _state: &RadrootsNostrAccountStoreState,
        ) -> Result<(), RadrootsNostrAccountsError> {
            Ok(())
        }
    }

    struct SaveErrorStore {
        state: RwLock<RadrootsNostrAccountStoreState>,
    }

    impl SaveErrorStore {
        fn new(state: RadrootsNostrAccountStoreState) -> Self {
            Self {
                state: RwLock::new(state),
            }
        }
    }

    impl RadrootsNostrAccountStore for SaveErrorStore {
        fn load(&self) -> Result<RadrootsNostrAccountStoreState, RadrootsNostrAccountsError> {
            let guard = self.state.read().map_err(|_| {
                RadrootsNostrAccountsError::Store("save error store poisoned".into())
            })?;
            Ok(guard.clone())
        }

        fn save(
            &self,
            _state: &RadrootsNostrAccountStoreState,
        ) -> Result<(), RadrootsNostrAccountsError> {
            Err(RadrootsNostrAccountsError::Store(
                "store save failed".into(),
            ))
        }
    }

    struct VaultStoreError;

    impl RadrootsSecretVault for VaultStoreError {
        fn store_secret(
            &self,
            _slot: &str,
            _secret: &str,
        ) -> Result<(), radroots_secret_vault::RadrootsSecretVaultAccessError> {
            Err(
                radroots_secret_vault::RadrootsSecretVaultAccessError::Backend(
                    "vault store failed".into(),
                ),
            )
        }

        fn load_secret(
            &self,
            _slot: &str,
        ) -> Result<Option<String>, radroots_secret_vault::RadrootsSecretVaultAccessError> {
            Ok(None)
        }

        fn remove_secret(
            &self,
            _slot: &str,
        ) -> Result<(), radroots_secret_vault::RadrootsSecretVaultAccessError> {
            Ok(())
        }
    }

    struct VaultLoadError;

    impl RadrootsSecretVault for VaultLoadError {
        fn store_secret(
            &self,
            _slot: &str,
            _secret: &str,
        ) -> Result<(), radroots_secret_vault::RadrootsSecretVaultAccessError> {
            Ok(())
        }

        fn load_secret(
            &self,
            _slot: &str,
        ) -> Result<Option<String>, radroots_secret_vault::RadrootsSecretVaultAccessError> {
            Err(
                radroots_secret_vault::RadrootsSecretVaultAccessError::Backend(
                    "vault load failed".into(),
                ),
            )
        }

        fn remove_secret(
            &self,
            _slot: &str,
        ) -> Result<(), radroots_secret_vault::RadrootsSecretVaultAccessError> {
            Ok(())
        }
    }

    struct VaultInvalidSecret;

    impl RadrootsSecretVault for VaultInvalidSecret {
        fn store_secret(
            &self,
            _slot: &str,
            _secret: &str,
        ) -> Result<(), radroots_secret_vault::RadrootsSecretVaultAccessError> {
            Ok(())
        }

        fn load_secret(
            &self,
            _slot: &str,
        ) -> Result<Option<String>, radroots_secret_vault::RadrootsSecretVaultAccessError> {
            Ok(Some("invalid-secret".to_string()))
        }

        fn remove_secret(
            &self,
            _slot: &str,
        ) -> Result<(), radroots_secret_vault::RadrootsSecretVaultAccessError> {
            Ok(())
        }
    }

    struct VaultRemoveError;

    impl RadrootsSecretVault for VaultRemoveError {
        fn store_secret(
            &self,
            _slot: &str,
            _secret: &str,
        ) -> Result<(), radroots_secret_vault::RadrootsSecretVaultAccessError> {
            Ok(())
        }

        fn load_secret(
            &self,
            _slot: &str,
        ) -> Result<Option<String>, radroots_secret_vault::RadrootsSecretVaultAccessError> {
            Ok(None)
        }

        fn remove_secret(
            &self,
            _slot: &str,
        ) -> Result<(), radroots_secret_vault::RadrootsSecretVaultAccessError> {
            Err(
                radroots_secret_vault::RadrootsSecretVaultAccessError::Backend(
                    "vault remove failed".into(),
                ),
            )
        }
    }

    fn poison_manager_state(manager: &RadrootsNostrAccountsManager) {
        let state = manager.state.clone();
        let _ = thread::spawn(move || {
            let _guard = state.write().expect("write");
            panic!("poison manager state");
        })
        .join();
    }

    fn status_kind(status: &RadrootsNostrAccountStatus) -> &'static str {
        match status {
            RadrootsNostrAccountStatus::NotConfigured => "not-configured",
            RadrootsNostrAccountStatus::PublicOnly { .. } => "public-only",
            RadrootsNostrAccountStatus::Ready { .. } => "ready",
        }
    }

    fn status_account(status: &RadrootsNostrAccountStatus) -> Option<&RadrootsNostrAccountRecord> {
        match status {
            RadrootsNostrAccountStatus::NotConfigured => None,
            RadrootsNostrAccountStatus::PublicOnly { account }
            | RadrootsNostrAccountStatus::Ready { account } => Some(account),
        }
    }

    #[test]
    fn manager_persists_default_account_and_restores_signing_identity() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Arc::new(RadrootsNostrFileAccountStore::new(
            temp.path().join("accounts.json"),
        ));
        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());
        let manager =
            RadrootsNostrAccountsManager::new(store.clone(), vault.clone()).expect("manager");
        let created_id = manager
            .generate_identity(Some("primary".into()), true)
            .expect("create identity");

        let default_account_id = manager
            .default_account_id()
            .expect("default")
            .expect("default id");
        assert_eq!(default_account_id, created_id);

        let manager2 = RadrootsNostrAccountsManager::new(store, vault).expect("manager2");
        let default_account_id_2 = manager2
            .default_account_id()
            .expect("default2")
            .expect("default2 id");
        assert_eq!(default_account_id_2, created_id);
        assert!(
            manager2
                .default_signing_identity()
                .expect("signing")
                .is_some()
        );
    }

    #[test]
    fn new_file_backed_with_vault_persists_default_account() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("accounts.json");
        let manager = RadrootsNostrAccountsManager::new_file_backed_with_vault(
            &path,
            RadrootsNostrSecretVaultMemory::new(),
        )
        .expect("manager");
        let identity = RadrootsIdentity::generate();
        let account_id = manager
            .upsert_identity(&identity, Some("primary".into()), true)
            .expect("upsert");

        let reloaded = RadrootsNostrAccountsManager::new_file_backed_with_vault(
            &path,
            RadrootsNostrSecretVaultMemory::new(),
        )
        .expect("reloaded");

        assert_eq!(
            reloaded.default_account_id().expect("default"),
            Some(account_id)
        );
        assert_eq!(reloaded.list_accounts().expect("accounts").len(), 1);
    }

    #[test]
    fn new_migrates_legacy_store_file_to_default_account_semantics() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("accounts.json");
        let identity = RadrootsIdentity::generate();
        let public_identity = identity.to_public();
        let account_id = public_identity.id.clone();
        let legacy_record =
            RadrootsNostrAccountRecord::new(public_identity, Some("legacy".into()), 1);
        fs::write(
            &path,
            serde_json::to_vec_pretty(&json!({
                "version": 1,
                "selected_account_id": account_id,
                "accounts": [legacy_record],
            }))
            .expect("serialize legacy store"),
        )
        .expect("write legacy store");

        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());
        vault
            .store_secret(
                account_secret_slot(&account_id).as_str(),
                identity.secret_key_hex().as_str(),
            )
            .expect("store secret");

        let manager = RadrootsNostrAccountsManager::new(
            Arc::new(RadrootsNostrFileAccountStore::new(&path)),
            vault,
        )
        .expect("manager");

        assert_eq!(
            manager.default_account_id().expect("default"),
            Some(account_id.clone())
        );

        let migrated_store: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("read migrated store"))
                .expect("parse migrated store");
        assert_eq!(
            migrated_store["version"],
            serde_json::Value::from(crate::model::RADROOTS_NOSTR_ACCOUNTS_STORE_VERSION),
        );
        assert_eq!(
            migrated_store["default_account_id"],
            serde_json::Value::from(account_id.to_string()),
        );
        assert!(migrated_store.get("selected_account_id").is_none());
    }

    #[test]
    fn resolve_local_backend_applies_shared_fallback_policy() {
        let resolved = RadrootsNostrAccountsManager::resolve_local_backend(
            RadrootsSecretBackendSelection {
                primary: RadrootsSecretBackend::HostVault(
                    radroots_secret_vault::RadrootsHostVaultPolicy::desktop(),
                ),
                fallback: Some(RadrootsSecretBackend::EncryptedFile),
            },
            RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: true,
                external_command: false,
                memory: false,
            },
        )
        .expect("fallback resolves");

        assert_eq!(resolved.backend, RadrootsSecretBackend::EncryptedFile);
        assert!(resolved.used_fallback);
    }

    #[test]
    fn new_local_file_backed_rejects_external_command_backend() {
        let temp = tempfile::tempdir().expect("tempdir");
        let err = RadrootsNostrAccountsManager::new_local_file_backed(
            temp.path().join("accounts.json"),
            temp.path().join("secrets"),
            RadrootsSecretBackendSelection {
                primary: RadrootsSecretBackend::ExternalCommand,
                fallback: None,
            },
            RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: true,
                external_command: true,
                memory: false,
            },
            "org.radroots.test.local-account",
        )
        .err()
        .expect("external command must be rejected");

        assert_eq!(
            err.to_string(),
            "vault error: external_command secret backend is not supported for local accounts"
        );
    }

    #[test]
    fn watch_only_account_has_no_signing_identity() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Arc::new(RadrootsNostrFileAccountStore::new(
            temp.path().join("accounts.json"),
        ));
        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());
        let manager = RadrootsNostrAccountsManager::new(store, vault).expect("manager");

        let identity = RadrootsIdentity::generate();
        let public = identity.to_public();
        manager
            .upsert_public_identity(public, Some("watch".into()), true)
            .expect("watch");

        assert!(
            manager
                .default_signing_identity()
                .expect("signing")
                .is_none()
        );
        let status = manager
            .default_account_status()
            .expect("default account status");
        assert_eq!(status_kind(&status), "public-only");
        let account = status_account(&status).expect("account");
        assert_eq!(account.label.as_deref(), Some("watch"));
    }

    #[test]
    fn default_account_status_reports_ready_for_signing_identity() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let default_account_id = manager
            .generate_identity(Some("primary".into()), true)
            .expect("generate");

        let status = manager
            .default_account_status()
            .expect("default account status");
        assert_eq!(status_kind(&status), "ready");
        let account = status_account(&status).expect("account");
        assert_eq!(account.account_id, default_account_id);
        assert_eq!(account.label.as_deref(), Some("primary"));

        let signer = manager
            .default_signer_capability()
            .expect("default signer capability")
            .expect("signer capability");
        let local = signer.local_account().expect("local signer");
        assert_eq!(local.account_id, default_account_id);
        assert!(local.is_secret_backed());
    }

    #[test]
    fn migrate_legacy_identity_file_imports_identity() {
        let temp = tempfile::tempdir().expect("tempdir");
        let legacy_path = temp.path().join("legacy_identity.json");
        let legacy_identity = RadrootsIdentity::generate();
        legacy_identity
            .save_json(&legacy_path)
            .expect("legacy save");

        let store = Arc::new(RadrootsNostrFileAccountStore::new(
            temp.path().join("accounts.json"),
        ));
        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());
        let manager = RadrootsNostrAccountsManager::new(store, vault).expect("manager");
        let id = manager
            .migrate_legacy_identity_file(&legacy_path, Some("legacy".into()), true)
            .expect("migrate");
        assert_eq!(
            manager
                .default_account_id()
                .expect("default")
                .expect("default id"),
            id
        );
    }

    #[test]
    fn upsert_public_identity_without_label_preserves_existing_label() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let account_id = manager
            .generate_identity(Some("primary".into()), true)
            .expect("generate");

        let existing = manager
            .default_public_identity()
            .expect("default public")
            .expect("public identity");
        manager
            .upsert_public_identity(existing, None, false)
            .expect("upsert");

        let records = manager.list_accounts().expect("list");
        let record = records
            .into_iter()
            .find(|record| record.account_id == account_id)
            .expect("account");
        assert_eq!(record.label.as_deref(), Some("primary"));
    }

    #[test]
    fn new_rejects_unsupported_schema_version() {
        let store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());
        let mut state = RadrootsNostrAccountStoreState::default();
        state.version = crate::model::RADROOTS_NOSTR_ACCOUNTS_STORE_VERSION + 1;
        store.save(&state).expect("save");

        let err = RadrootsNostrAccountsManager::new(store, vault)
            .err()
            .expect("unsupported schema version");
        assert!(err.to_string().contains("invalid account state"));
    }

    #[test]
    fn new_clears_orphaned_default_account() {
        let store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());
        let mut state = RadrootsNostrAccountStoreState::default();
        state.default_account_id = Some(RadrootsIdentity::generate().id());
        store.save(&state).expect("save");

        let manager = RadrootsNostrAccountsManager::new(store, vault).expect("manager");
        assert!(manager.default_account_id().expect("default").is_none());
    }

    #[test]
    fn default_methods_return_none_when_state_is_empty() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        assert!(
            manager
                .default_account()
                .expect("default account")
                .is_none()
        );
        assert!(
            manager
                .default_public_identity()
                .expect("default public")
                .is_none()
        );
        assert!(
            manager
                .default_signing_identity()
                .expect("default signing")
                .is_none()
        );
        assert!(
            manager
                .default_signer_capability()
                .expect("default signer capability")
                .is_none()
        );
        let status = manager
            .default_account_status()
            .expect("default account status");
        assert_eq!(status_kind(&status), "not-configured");
        assert!(status_account(&status).is_none());

        let missing_id = RadrootsIdentity::generate().id();
        assert!(
            manager
                .get_signing_identity(&missing_id)
                .expect("signing")
                .is_none()
        );
        assert!(
            manager
                .get_signer_capability(&missing_id)
                .expect("signer capability")
                .is_none()
        );
    }

    #[test]
    fn default_account_status_propagates_secret_integrity_errors() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let account_id = manager
            .generate_identity(Some("primary".into()), true)
            .expect("generate");
        manager
            .vault
            .remove_secret(account_secret_slot(&account_id).as_str())
            .expect("remove secret");

        let status = manager
            .default_account_status()
            .expect("default account status");
        assert_eq!(status_kind(&status), "public-only");
        let account = status_account(&status).expect("account");
        assert_eq!(account.account_id, account_id);

        let wrong_identity = RadrootsIdentity::generate();
        manager
            .vault
            .store_secret(
                account_secret_slot(&account_id).as_str(),
                wrong_identity.secret_key_hex().as_str(),
            )
            .expect("store wrong secret");

        let err = manager
            .default_account_status()
            .expect_err("public key mismatch");
        assert_eq!(err.to_string(), "public key does not match secret key");
    }

    #[test]
    fn default_account_status_propagates_store_vault_and_secret_parse_errors() {
        let poisoned_manager = RadrootsNostrAccountsManager::new_in_memory();
        poison_manager_state(&poisoned_manager);
        let default_err = poisoned_manager
            .default_account_status()
            .expect_err("default status poisoned");
        assert!(default_err.to_string().starts_with("store error:"));

        let mut load_error_state = RadrootsNostrAccountStoreState::default();
        let load_error_public = RadrootsIdentity::generate().to_public();
        load_error_state
            .accounts
            .push(RadrootsNostrAccountRecord::new(
                load_error_public.clone(),
                Some("watch".into()),
                1,
            ));
        load_error_state.default_account_id = Some(load_error_public.id.clone());
        let load_error_store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        load_error_store
            .save(&load_error_state)
            .expect("save state");
        let vault_load_error_manager =
            RadrootsNostrAccountsManager::new(load_error_store, Arc::new(VaultLoadError))
                .expect("manager");
        let vault_load_error = vault_load_error_manager
            .default_account_status()
            .expect_err("vault load error");
        assert!(vault_load_error.to_string().starts_with("vault error:"));

        let mut invalid_secret_state = RadrootsNostrAccountStoreState::default();
        let invalid_secret_public = RadrootsIdentity::generate().to_public();
        invalid_secret_state
            .accounts
            .push(RadrootsNostrAccountRecord::new(
                invalid_secret_public.clone(),
                Some("invalid".into()),
                1,
            ));
        invalid_secret_state.default_account_id = Some(invalid_secret_public.id.clone());
        let invalid_secret_store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        invalid_secret_store
            .save(&invalid_secret_state)
            .expect("save state");
        let invalid_secret_manager =
            RadrootsNostrAccountsManager::new(invalid_secret_store, Arc::new(VaultInvalidSecret))
                .expect("manager");
        let invalid_secret = invalid_secret_manager
            .default_account_status()
            .expect_err("invalid secret");
        assert!(invalid_secret.to_string().starts_with("identity error:"));
    }

    #[test]
    fn signer_capability_paths_propagate_secret_parse_errors() {
        let mut invalid_secret_state = RadrootsNostrAccountStoreState::default();
        let invalid_secret_public = RadrootsIdentity::generate().to_public();
        invalid_secret_state
            .accounts
            .push(RadrootsNostrAccountRecord::new(
                invalid_secret_public.clone(),
                Some("invalid".into()),
                1,
            ));
        invalid_secret_state.default_account_id = Some(invalid_secret_public.id.clone());
        let invalid_secret_store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        invalid_secret_store
            .save(&invalid_secret_state)
            .expect("save state");
        let invalid_secret_manager =
            RadrootsNostrAccountsManager::new(invalid_secret_store, Arc::new(VaultInvalidSecret))
                .expect("manager");

        let default_signer_error = invalid_secret_manager
            .default_signer_capability()
            .expect_err("default signer invalid secret");
        assert!(
            default_signer_error
                .to_string()
                .starts_with("identity error:")
        );

        let signer_error = invalid_secret_manager
            .get_signer_capability(&invalid_secret_public.id)
            .expect_err("signer invalid secret");
        assert!(signer_error.to_string().starts_with("identity error:"));
    }

    #[test]
    fn select_remove_export_and_lookup_paths() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let first_id = manager
            .generate_identity(Some("first".into()), true)
            .expect("first");
        let second_id = manager
            .generate_identity(Some("second".into()), false)
            .expect("second");

        manager
            .set_default_account(&second_id)
            .expect("set default second");
        assert_eq!(
            manager.default_account_id().expect("default"),
            Some(second_id.clone())
        );
        assert!(
            manager
                .export_secret_hex(&second_id)
                .expect("export")
                .is_some()
        );
        assert!(
            manager
                .get_signing_identity(&second_id)
                .expect("signing")
                .is_some()
        );

        manager.remove_account(&second_id).expect("remove second");
        assert_eq!(manager.default_account_id().expect("default"), None);
        assert!(
            manager
                .export_secret_hex(&second_id)
                .expect("export after remove")
                .is_none()
        );
        assert!(
            manager
                .get_signing_identity(&first_id)
                .expect("first signing")
                .is_some()
        );

        let set_default_missing = manager
            .set_default_account(&second_id)
            .expect_err("missing default");
        assert!(
            set_default_missing
                .to_string()
                .contains("account not found")
        );
        let remove_missing = manager
            .remove_account(&second_id)
            .expect_err("missing remove");
        assert!(remove_missing.to_string().contains("account not found"));
    }

    #[test]
    fn upsert_public_identity_updates_label_and_respects_default_flag() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let original_default = manager
            .generate_identity(Some("primary".into()), true)
            .expect("generate");

        let existing = manager
            .default_public_identity()
            .expect("default public")
            .expect("public");
        manager
            .upsert_public_identity(existing.clone(), Some("renamed".into()), false)
            .expect("upsert existing");

        let renamed = manager
            .list_accounts()
            .expect("list")
            .into_iter()
            .find(|record| record.account_id == existing.id)
            .expect("record");
        assert_eq!(renamed.label.as_deref(), Some("renamed"));

        let watch_only = RadrootsIdentity::generate().to_public();
        let watch_id = watch_only.id.clone();
        manager
            .upsert_public_identity(watch_only.clone(), Some("watch".into()), false)
            .expect("upsert watch");
        assert_eq!(
            manager.default_account_id().expect("default"),
            Some(original_default.clone())
        );

        manager
            .upsert_public_identity(watch_only, Some("watch".into()), true)
            .expect("replace default");
        assert_eq!(
            manager.default_account_id().expect("default"),
            Some(watch_id)
        );
    }

    #[test]
    fn upsert_public_identity_rejects_mismatched_id() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let mut public_identity = RadrootsIdentity::generate().to_public();
        let other = RadrootsIdentity::generate().to_public();
        public_identity.id = other.id.clone();

        let err = manager
            .upsert_public_identity(public_identity, None, true)
            .expect_err("id mismatch");
        assert!(err.to_string().starts_with("invalid account state:"));
    }

    #[test]
    fn remove_non_default_account_keeps_current_default() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let default_account_id = manager
            .generate_identity(Some("selected".into()), true)
            .expect("default");
        let removable_id = manager
            .generate_identity(Some("removable".into()), false)
            .expect("removable");

        manager.remove_account(&removable_id).expect("remove");
        assert_eq!(
            manager.default_account_id().expect("default"),
            Some(default_account_id)
        );
    }

    #[test]
    fn clear_default_account_clears_default_without_removing_accounts() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        manager
            .generate_identity(Some("primary".into()), true)
            .expect("primary");
        manager
            .generate_identity(Some("secondary".into()), false)
            .expect("secondary");

        manager.clear_default_account().expect("clear default");

        assert!(manager.default_account_id().expect("default").is_none());
        assert_eq!(manager.list_accounts().expect("accounts").len(), 2);
    }

    #[test]
    fn resolve_account_selector_matches_exact_id_npub_and_unique_label() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let account_id = manager
            .generate_identity(Some("primary".into()), true)
            .expect("primary");
        let default_account = manager
            .default_account()
            .expect("default account")
            .expect("default record");
        let npub = default_account.public_identity.public_key_npub.clone();

        let resolved_by_id = manager
            .resolve_account_selector(account_id.as_str())
            .expect("resolve by id");
        assert_eq!(resolved_by_id.account_id, account_id);

        let resolved_by_npub = manager
            .resolve_account_selector(&npub)
            .expect("resolve by npub");
        assert_eq!(resolved_by_npub.account_id, account_id);

        let resolved_by_label = manager
            .resolve_account_selector("primary")
            .expect("resolve by label");
        assert_eq!(resolved_by_label.account_id, account_id);
    }

    #[test]
    fn resolve_account_selector_rejects_empty_and_ambiguous_labels() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        manager
            .generate_identity(Some("shared".into()), true)
            .expect("first");
        manager
            .generate_identity(Some("shared".into()), false)
            .expect("second");

        let empty = manager
            .resolve_account_selector("   ")
            .expect_err("empty selector");
        assert!(matches!(
            empty,
            RadrootsNostrAccountsError::InvalidAccountSelector(_)
        ));

        let ambiguous = manager
            .resolve_account_selector("shared")
            .expect_err("ambiguous selector");
        assert!(matches!(
            ambiguous,
            RadrootsNostrAccountsError::AmbiguousAccountSelector(_)
        ));
    }

    #[test]
    fn remove_account_propagates_vault_remove_error() {
        let store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        let vault = Arc::new(VaultRemoveError);
        let manager = RadrootsNostrAccountsManager::new(store, vault.clone()).expect("manager");
        let public = RadrootsIdentity::generate().to_public();
        let account_id = public.id.clone();
        vault
            .store_secret(account_secret_slot(&account_id).as_str(), "secret")
            .expect("vault store");
        assert!(
            vault
                .load_secret(account_secret_slot(&account_id).as_str())
                .expect("vault load")
                .is_none()
        );
        manager
            .upsert_public_identity(public, Some("remove".into()), true)
            .expect("upsert");

        let err = manager
            .remove_account(&account_id)
            .expect_err("remove error");
        assert!(err.to_string().starts_with("vault error:"));
    }

    #[test]
    fn resolve_signing_identity_mismatch_and_profile_paths() {
        let store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());
        let manager = RadrootsNostrAccountsManager::new(store, vault.clone()).expect("manager");

        let mismatch_public = RadrootsIdentity::generate().to_public();
        let mismatch_id = mismatch_public.id.clone();
        manager
            .upsert_public_identity(mismatch_public, Some("mismatch".into()), true)
            .expect("upsert mismatch");

        let wrong_identity = RadrootsIdentity::generate();
        vault
            .store_secret(
                account_secret_slot(&mismatch_id).as_str(),
                wrong_identity.secret_key_hex().as_str(),
            )
            .expect("vault store");

        let mismatch = manager
            .default_signing_identity()
            .expect_err("public key mismatch");
        assert!(
            mismatch
                .to_string()
                .contains("public key does not match secret key")
        );

        let mut with_profile = RadrootsIdentity::generate();
        let profile = RadrootsIdentityProfile {
            identifier: Some("profile-id".to_string()),
            ..RadrootsIdentityProfile::default()
        };
        with_profile.set_profile(profile);
        let profile_id = manager
            .upsert_identity(&with_profile, Some("profile".into()), true)
            .expect("upsert profile");
        let resolved = manager
            .get_signing_identity(&profile_id)
            .expect("resolve")
            .expect("identity");
        assert_eq!(
            resolved
                .profile()
                .and_then(|value| value.identifier.clone())
                .as_deref(),
            Some("profile-id")
        );

        let local_signer = manager
            .get_signer_capability(&profile_id)
            .expect("local signer capability")
            .expect("local signer");
        assert!(
            manager
                .resolve_signing_identity_for_signer(&local_signer)
                .expect("resolve local signer")
                .is_some()
        );

        let remote_signer = RadrootsNostrSignerCapability::RemoteSession(
            radroots_nostr_signer::prelude::RadrootsNostrRemoteSessionSignerCapability::new(
                radroots_nostr_signer::prelude::RadrootsNostrSignerConnectionId::new_v7(),
                RadrootsIdentity::generate().to_public(),
                RadrootsIdentity::generate().to_public(),
            ),
        );
        assert!(
            manager
                .resolve_signing_identity_for_signer(&remote_signer)
                .expect("resolve remote signer")
                .is_none()
        );
    }

    #[test]
    fn manager_propagates_store_and_vault_errors() {
        let load_error = RadrootsNostrAccountsManager::new(
            Arc::new(LoadErrorStore),
            Arc::new(RadrootsNostrSecretVaultMemory::new()),
        )
        .err()
        .expect("load error manager");
        assert!(load_error.to_string().starts_with("store error:"));

        let save_error_store = Arc::new(SaveErrorStore::new(
            RadrootsNostrAccountStoreState::default(),
        ));
        let save_error_manager = RadrootsNostrAccountsManager::new(
            save_error_store,
            Arc::new(RadrootsNostrSecretVaultMemory::new()),
        )
        .expect("manager");
        let save_error = save_error_manager
            .upsert_public_identity(RadrootsIdentity::generate().to_public(), None, true)
            .expect_err("save error");
        assert!(save_error.to_string().starts_with("store error:"));

        let vault_store_error_manager = RadrootsNostrAccountsManager::new(
            Arc::new(RadrootsNostrMemoryAccountStore::new()),
            Arc::new(VaultStoreError),
        )
        .expect("manager");
        let identity = RadrootsIdentity::generate();
        let vault_store_error = vault_store_error_manager
            .upsert_identity(&identity, None, true)
            .expect_err("vault store error");
        assert!(vault_store_error.to_string().starts_with("vault error:"));

        let mut load_error_state = RadrootsNostrAccountStoreState::default();
        let load_error_public = RadrootsIdentity::generate().to_public();
        load_error_state
            .accounts
            .push(RadrootsNostrAccountRecord::new(
                load_error_public.clone(),
                Some("watch".into()),
                1,
            ));
        load_error_state.default_account_id = Some(load_error_public.id.clone());
        let load_error_store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        load_error_store
            .save(&load_error_state)
            .expect("save state");
        let vault_load_error_manager =
            RadrootsNostrAccountsManager::new(load_error_store, Arc::new(VaultLoadError))
                .expect("manager");
        let vault_load_error = vault_load_error_manager
            .default_signing_identity()
            .expect_err("vault load error");
        assert!(vault_load_error.to_string().starts_with("vault error:"));

        let mut invalid_secret_state = RadrootsNostrAccountStoreState::default();
        let invalid_secret_public = RadrootsIdentity::generate().to_public();
        invalid_secret_state
            .accounts
            .push(RadrootsNostrAccountRecord::new(
                invalid_secret_public.clone(),
                Some("invalid".into()),
                1,
            ));
        invalid_secret_state.default_account_id = Some(invalid_secret_public.id.clone());
        let invalid_secret_store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        invalid_secret_store
            .save(&invalid_secret_state)
            .expect("save state");
        let invalid_secret_manager =
            RadrootsNostrAccountsManager::new(invalid_secret_store, Arc::new(VaultInvalidSecret))
                .expect("manager");
        let invalid_secret = invalid_secret_manager
            .default_signing_identity()
            .expect_err("invalid secret");
        assert!(invalid_secret.to_string().starts_with("identity error:"));
    }

    #[test]
    fn migrate_legacy_identity_file_returns_error_for_missing_path() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let temp = tempfile::tempdir().expect("tempdir");
        let missing = temp.path().join("missing_legacy.json");
        let migrated = manager
            .migrate_legacy_identity_file(&missing, None, false)
            .expect_err("missing legacy");
        assert!(migrated.to_string().starts_with("identity error:"));
    }

    #[test]
    fn manager_reports_poisoned_state_locks() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        poison_manager_state(&manager);

        let list_err = manager.list_accounts().expect_err("list poisoned");
        assert!(list_err.to_string().starts_with("store error:"));
        let default_id_err = manager
            .default_account_id()
            .expect_err("default id poisoned");
        assert!(default_id_err.to_string().starts_with("store error:"));
        let default_err = manager.default_account().expect_err("default poisoned");
        assert!(default_err.to_string().starts_with("store error:"));
        let default_public_err = manager
            .default_public_identity()
            .expect_err("default public poisoned");
        assert!(default_public_err.to_string().starts_with("store error:"));
        let default_signing_err = manager
            .default_signing_identity()
            .expect_err("default signing poisoned");
        assert!(default_signing_err.to_string().starts_with("store error:"));
        let default_signer_err = manager
            .default_signer_capability()
            .expect_err("default signer poisoned");
        assert!(default_signer_err.to_string().starts_with("store error:"));

        let account_id = RadrootsIdentity::generate().id();
        let signing_err = manager
            .get_signing_identity(&account_id)
            .expect_err("signing poisoned");
        assert!(signing_err.to_string().starts_with("store error:"));
        let signer_err = manager
            .get_signer_capability(&account_id)
            .expect_err("signer poisoned");
        assert!(signer_err.to_string().starts_with("store error:"));
        let set_default_err = manager
            .set_default_account(&account_id)
            .expect_err("default poisoned");
        assert!(set_default_err.to_string().starts_with("store error:"));
        let remove_err = manager
            .remove_account(&account_id)
            .expect_err("remove poisoned");
        assert!(remove_err.to_string().starts_with("store error:"));
        let upsert_err = manager
            .upsert_public_identity(RadrootsIdentity::generate().to_public(), None, false)
            .expect_err("upsert poisoned");
        assert!(upsert_err.to_string().starts_with("store error:"));
    }

    #[test]
    fn stub_store_and_vault_methods_are_exercised() {
        let load_error_store = LoadErrorStore;
        let load_error_store_result =
            load_error_store.save(&RadrootsNostrAccountStoreState::default());
        assert!(load_error_store_result.is_ok());

        let save_error_store = SaveErrorStore::new(RadrootsNostrAccountStoreState::default());
        let loaded = save_error_store.load().expect("load");
        assert_eq!(
            loaded.version,
            RadrootsNostrAccountStoreState::default().version
        );
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = save_error_store.state.write().expect("write");
            panic!("poison save error store");
        }));
        let poisoned_load = save_error_store.load().expect_err("poisoned load");
        assert!(poisoned_load.to_string().starts_with("store error:"));

        let account_id = RadrootsIdentity::generate().id();
        let vault_store_error = VaultStoreError;
        assert!(
            vault_store_error
                .load_secret(account_secret_slot(&account_id).as_str())
                .expect("load")
                .is_none()
        );
        vault_store_error
            .remove_secret(account_secret_slot(&account_id).as_str())
            .expect("remove");

        let vault_load_error = VaultLoadError;
        vault_load_error
            .store_secret(account_secret_slot(&account_id).as_str(), "secret")
            .expect("store");
        vault_load_error
            .remove_secret(account_secret_slot(&account_id).as_str())
            .expect("remove");

        let vault_invalid_secret = VaultInvalidSecret;
        vault_invalid_secret
            .store_secret(account_secret_slot(&account_id).as_str(), "secret")
            .expect("store");
        vault_invalid_secret
            .remove_secret(account_secret_slot(&account_id).as_str())
            .expect("remove");
    }
}
