use crate::error::RadrootsNostrAccountsError;
use crate::model::RadrootsNostrAccountStoreState;
#[cfg(feature = "memory-vault")]
use crate::store::RadrootsNostrMemoryAccountStore;
use crate::store::{RadrootsNostrAccountStore, RadrootsNostrFileAccountStore};
#[cfg(feature = "memory-vault")]
use crate::vault::RadrootsNostrSecretVaultMemory;
#[cfg(feature = "os-keyring")]
use crate::vault::RadrootsNostrSecretVaultOsKeyring;
use crate::vault::{RadrootsSecretVault, account_secret_slot};
use nostr::{Keys, PublicKey as NostrPublicKey, SecretKey};
use radroots_identity::{
    AccountId, PublicIdentity, PublicKey as IdentityPublicKey,
    account::{Record as AccountRecord, Status as AccountStatus},
};
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
        if state.version != crate::model::RADROOTS_NOSTR_ACCOUNTS_STORE_VERSION {
            return Err(RadrootsNostrAccountsError::InvalidState(format!(
                "unsupported accounts schema version {}",
                state.version
            )));
        }

        let mut state_dirty = false;
        if let Some(default_account_id) = state.default_account_id.clone() {
            let exists = state
                .accounts
                .iter()
                .any(|record| record.id() == default_account_id);
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

    pub fn list_accounts(&self) -> Result<Vec<AccountRecord>, RadrootsNostrAccountsError> {
        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        Ok(guard.accounts.clone())
    }

    pub fn default_account_id(&self) -> Result<Option<AccountId>, RadrootsNostrAccountsError> {
        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        Ok(guard.default_account_id.clone())
    }

    pub fn default_account(&self) -> Result<Option<AccountRecord>, RadrootsNostrAccountsError> {
        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        let Some(default_account_id) = guard.default_account_id.as_ref() else {
            return Ok(None);
        };
        Ok(guard
            .accounts
            .iter()
            .find(|record| record.id() == *default_account_id)
            .cloned())
    }

    pub fn default_public_identity(
        &self,
    ) -> Result<Option<PublicIdentity>, RadrootsNostrAccountsError> {
        Ok(self
            .default_account()?
            .map(|record| record.public_identity().clone()))
    }

    pub fn default_account_status(&self) -> Result<AccountStatus, RadrootsNostrAccountsError> {
        let Some(record) = self.default_account()? else {
            return Ok(AccountStatus::NotConfigured);
        };

        Ok(match self.local_signer_availability(&record)? {
            RadrootsNostrLocalSignerAvailability::PublicOnly => {
                AccountStatus::PublicOnly { account: record }
            }
            RadrootsNostrLocalSignerAvailability::SecretBacked => {
                AccountStatus::Ready { account: record }
            }
        })
    }

    pub fn default_signing_keys(&self) -> Result<Option<Keys>, RadrootsNostrAccountsError> {
        let Some(record) = self.default_account()? else {
            return Ok(None);
        };
        self.resolve_signing_keys(record)
    }

    pub fn get_signing_keys(
        &self,
        account_id: &AccountId,
    ) -> Result<Option<Keys>, RadrootsNostrAccountsError> {
        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        let Some(record) = guard
            .accounts
            .iter()
            .find(|record| record.id() == *account_id)
            .cloned()
        else {
            return Ok(None);
        };
        drop(guard);
        self.resolve_signing_keys(record)
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
        account_id: &AccountId,
    ) -> Result<Option<RadrootsNostrSignerCapability>, RadrootsNostrAccountsError> {
        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        let Some(record) = guard
            .accounts
            .iter()
            .find(|record| record.id() == *account_id)
            .cloned()
        else {
            return Ok(None);
        };
        drop(guard);
        Ok(Some(self.local_signer_capability(record)?))
    }

    pub fn resolve_signing_keys_for_signer(
        &self,
        signer: &RadrootsNostrSignerCapability,
    ) -> Result<Option<Keys>, RadrootsNostrAccountsError> {
        match signer {
            RadrootsNostrSignerCapability::LocalAccount(capability) => {
                self.get_signing_keys(&capability.account_id)
            }
            RadrootsNostrSignerCapability::RemoteSession(_) => Ok(None),
        }
    }

    pub fn upsert_keys(
        &self,
        keys: &Keys,
        label: Option<String>,
        make_default: bool,
    ) -> Result<AccountId, RadrootsNostrAccountsError> {
        let public_identity = public_identity_from_keys(keys)?;
        let account_id = AccountId::from_public_identity(&public_identity);
        let secret_key_hex = Zeroizing::new(keys.secret_key().to_secret_hex());
        self.vault.store_secret(
            account_secret_slot(&account_id).as_str(),
            secret_key_hex.as_str(),
        )?;

        self.upsert_public_identity(public_identity, label, make_default)
    }

    /// Attaches matching secret material to an existing account without import semantics.
    pub fn attach_secret_keys(
        &self,
        account_id: &AccountId,
        keys: &Keys,
        make_default: bool,
    ) -> Result<AccountRecord, RadrootsNostrAccountsError> {
        let account_id = *account_id;
        let public_key_hex = keys.public_key().to_hex();
        let updated_at_unix = now_unix_secs();
        let mut guard = self.state.write().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        let mut next = guard.clone();
        let Some(record) = next
            .accounts
            .iter_mut()
            .find(|record| record.id() == account_id)
        else {
            return Err(RadrootsNostrAccountsError::AccountNotFound(
                account_id.to_string(),
            ));
        };
        if record.public_identity().public_key().to_hex() != public_key_hex {
            return Err(RadrootsNostrAccountsError::PublicKeyMismatch);
        }

        let secret_key_hex = Zeroizing::new(keys.secret_key().to_secret_hex());
        self.vault.store_secret(
            account_secret_slot(&account_id).as_str(),
            secret_key_hex.as_str(),
        )?;

        record.touch_updated(updated_at_unix)?;
        let updated_record = record.clone();
        if make_default {
            next.default_account_id = Some(account_id);
        }
        self.store.save(&next)?;
        *guard = next;
        Ok(updated_record)
    }

    pub fn upsert_public_identity(
        &self,
        public_identity: PublicIdentity,
        label: Option<String>,
        make_default: bool,
    ) -> Result<AccountId, RadrootsNostrAccountsError> {
        let updated_at_unix = now_unix_secs();
        let account_id = AccountId::from_public_identity(&public_identity);
        self.update_state(|state| {
            if let Some(existing) = state
                .accounts
                .iter_mut()
                .find(|record| record.id() == account_id)
            {
                let next_label = label
                    .clone()
                    .or_else(|| existing.label().map(ToOwned::to_owned));
                *existing = AccountRecord::try_from_parts(
                    existing.id(),
                    public_identity.clone(),
                    next_label,
                    existing.created_at_unix(),
                    updated_at_unix,
                )?;
            } else {
                state.accounts.push(AccountRecord::new(
                    public_identity.clone(),
                    label.clone(),
                    updated_at_unix,
                ));
            }

            if state.default_account_id.is_none() || make_default {
                state.default_account_id = Some(account_id);
            }
            Ok(())
        })?;
        Ok(account_id)
    }

    pub fn generate_keys(
        &self,
        label: Option<String>,
        make_default: bool,
    ) -> Result<AccountId, RadrootsNostrAccountsError> {
        let keys = Keys::generate();
        self.upsert_keys(&keys, label, make_default)
    }

    pub fn set_default_account(
        &self,
        account_id: &AccountId,
    ) -> Result<(), RadrootsNostrAccountsError> {
        let account_id = *account_id;
        self.update_state(|state| {
            let exists = state
                .accounts
                .iter()
                .any(|record| record.id() == account_id);
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
    ) -> Result<AccountRecord, RadrootsNostrAccountsError> {
        let normalized = selector.trim();
        if normalized.is_empty() {
            return Err(RadrootsNostrAccountsError::InvalidAccountSelector(
                "account selector cannot be empty".to_owned(),
            ));
        }

        let selector_public_key = NostrPublicKey::parse(normalized).ok();
        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        if let Some(record) = guard
            .accounts
            .iter()
            .find(|record| {
                record.id().to_hex() == normalized
                    || selector_public_key.is_some_and(|key| {
                        record.public_identity().public_key().to_hex() == key.to_hex()
                    })
            })
            .cloned()
        {
            return Ok(record);
        }

        let mut label_matches = guard
            .accounts
            .iter()
            .filter(|record| record.label() == Some(normalized))
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

    pub fn remove_account(&self, account_id: &AccountId) -> Result<(), RadrootsNostrAccountsError> {
        let account_id = *account_id;
        self.update_state(|state| {
            let before = state.accounts.len();
            state.accounts.retain(|record| record.id() != account_id);
            if state.accounts.len() == before {
                return Err(RadrootsNostrAccountsError::AccountNotFound(
                    account_id.to_string(),
                ));
            }

            if state.default_account_id == Some(account_id) {
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
        account_id: &AccountId,
    ) -> Result<Option<String>, RadrootsNostrAccountsError> {
        self.vault
            .load_secret(account_secret_slot(account_id).as_str())
            .map_err(Into::into)
    }

    fn resolve_signing_keys(
        &self,
        record: AccountRecord,
    ) -> Result<Option<Keys>, RadrootsNostrAccountsError> {
        let Some(secret_key_hex) = self
            .vault
            .load_secret(account_secret_slot(&record.id()).as_str())?
        else {
            return Ok(None);
        };
        let secret_key_hex = Zeroizing::new(secret_key_hex);
        let keys = keys_from_secret_hex(secret_key_hex.as_str())?;
        if keys.public_key().to_hex() != record.public_identity().public_key().to_hex() {
            return Err(RadrootsNostrAccountsError::PublicKeyMismatch);
        }
        Ok(Some(keys))
    }

    fn local_signer_capability(
        &self,
        record: AccountRecord,
    ) -> Result<RadrootsNostrSignerCapability, RadrootsNostrAccountsError> {
        let availability = self.local_signer_availability(&record)?;
        Ok(RadrootsNostrSignerCapability::LocalAccount(Box::new(
            RadrootsNostrLocalSignerCapability::new(
                record.id(),
                record.public_identity().clone(),
                availability,
            ),
        )))
    }

    fn local_signer_availability(
        &self,
        record: &AccountRecord,
    ) -> Result<RadrootsNostrLocalSignerAvailability, RadrootsNostrAccountsError> {
        let Some(secret_key_hex) = self
            .vault
            .load_secret(account_secret_slot(&record.id()).as_str())?
        else {
            return Ok(RadrootsNostrLocalSignerAvailability::PublicOnly);
        };

        let secret_key_hex = Zeroizing::new(secret_key_hex);
        let keys = keys_from_secret_hex(secret_key_hex.as_str())?;
        if keys.public_key().to_hex() != record.public_identity().public_key().to_hex() {
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

fn public_identity_from_keys(keys: &Keys) -> Result<PublicIdentity, RadrootsNostrAccountsError> {
    let public_key = IdentityPublicKey::from_hex(&keys.public_key().to_hex())?;
    Ok(PublicIdentity::new(public_key))
}

fn keys_from_secret_hex(secret_key_hex: &str) -> Result<Keys, RadrootsNostrAccountsError> {
    let secret_key = SecretKey::from_hex(secret_key_hex)
        .map_err(|error| RadrootsNostrAccountsError::Identity(error.to_string()))?;
    Ok(Keys::new(secret_key))
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
    use nostr::{Keys, ToBech32};
    use radroots_identity::{Profile, Username};
    use radroots_secret_vault::{
        RadrootsHostVaultCapabilities, RadrootsSecretBackend, RadrootsSecretBackendAvailability,
        RadrootsSecretBackendSelection,
    };
    use serde_json::json;
    use std::fs;
    use std::sync::Arc;
    use std::sync::RwLock;
    use std::thread;

    trait TestKeysExt {
        fn id(&self) -> AccountId;
        fn to_public(&self) -> PublicIdentity;
        fn public_key_hex(&self) -> String;
        fn secret_key_hex(&self) -> String;
    }

    impl TestKeysExt for Keys {
        fn id(&self) -> AccountId {
            AccountId::from_public_identity(&self.to_public())
        }

        fn to_public(&self) -> PublicIdentity {
            public_identity_from_keys(self).expect("public identity")
        }

        fn public_key_hex(&self) -> String {
            self.public_key().to_hex()
        }

        fn secret_key_hex(&self) -> String {
            self.secret_key().to_secret_hex()
        }
    }

    mod removed_surface_fixtures {
        pub const MIGRATE_LEGACY_IDENTITY_FILE: &str = "migrate_legacy_identity_file";
        pub const SELECTED_ACCOUNT_ID: &str = "selected_account_id";
        pub const SERDE_ALIAS_ATTRIBUTE: &str = "serde(alias";
    }

    fn production_manager_source() -> &'static str {
        include_str!("manager.rs")
            .split_once("#[cfg(test)]\nmod tests")
            .expect("manager tests boundary")
            .0
    }

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

    fn status_kind(status: &AccountStatus) -> &'static str {
        match status {
            AccountStatus::NotConfigured => "not-configured",
            AccountStatus::PublicOnly { .. } => "public-only",
            AccountStatus::Ready { .. } => "ready",
            _ => "unknown",
        }
    }

    fn status_account(status: &AccountStatus) -> Option<&AccountRecord> {
        match status {
            AccountStatus::NotConfigured => None,
            AccountStatus::PublicOnly { account } | AccountStatus::Ready { account } => {
                Some(account)
            }
            _ => None,
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
            .generate_keys(Some("primary".into()), true)
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
        assert!(manager2.default_signing_keys().expect("signing").is_some());
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
        let identity = Keys::generate();
        let account_id = manager
            .upsert_keys(&identity, Some("primary".into()), true)
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
    fn new_rejects_non_current_store_file_schema() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("accounts.json");
        fs::write(
            &path,
            serde_json::to_vec_pretty(&json!({
                "version": 1,
                "default_account_id": Keys::generate().id(),
                "accounts": [],
            }))
            .expect("serialize store"),
        )
        .expect("write store");

        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());
        let err = match RadrootsNostrAccountsManager::new(
            Arc::new(RadrootsNostrFileAccountStore::new(&path)),
            vault,
        ) {
            Ok(_) => panic!("unsupported schema was accepted"),
            Err(error) => error,
        };

        assert!(
            err.to_string()
                .contains("unsupported accounts schema version 1")
        );
    }

    #[test]
    fn new_reports_save_error_when_dirty_state_requires_rewrite() {
        let state = RadrootsNostrAccountStoreState {
            default_account_id: Some(Keys::generate().id()),
            ..Default::default()
        };
        let store = Arc::new(SaveErrorStore::new(state));
        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());

        let err = match RadrootsNostrAccountsManager::new(store, vault) {
            Ok(_) => panic!("dirty state save error"),
            Err(err) => err,
        };

        assert_eq!(err.to_string(), "store error: store save failed");
    }

    #[test]
    fn resolve_local_backend_fails_when_primary_is_unavailable() {
        let err = RadrootsNostrAccountsManager::resolve_local_backend(
            RadrootsSecretBackendSelection {
                primary: RadrootsSecretBackend::HostVault(
                    radroots_secret_vault::RadrootsHostVaultPolicy::desktop(),
                ),
            },
            RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: true,
                external_command: false,
                memory: false,
            },
        )
        .expect_err("unavailable primary fails");

        assert_eq!(err.to_string(), "secret backend host_vault is unavailable");
    }

    #[test]
    fn new_local_file_backed_rejects_external_command_backend() {
        let temp = tempfile::tempdir().expect("tempdir");
        let err = match RadrootsNostrAccountsManager::new_local_file_backed(
            temp.path().join("accounts.json"),
            temp.path().join("secrets"),
            RadrootsSecretBackendSelection {
                primary: RadrootsSecretBackend::ExternalCommand,
            },
            RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: true,
                external_command: true,
                memory: false,
            },
            "org.radroots.test.local-account",
        ) {
            Ok(_) => panic!("external command must be rejected"),
            Err(err) => err,
        };

        assert_eq!(
            err.to_string(),
            "vault error: external_command secret backend is not supported for local accounts"
        );
    }

    #[test]
    fn new_local_file_backed_reports_backend_resolution_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        let err = match RadrootsNostrAccountsManager::new_local_file_backed(
            temp.path().join("accounts.json"),
            temp.path().join("secrets"),
            RadrootsSecretBackendSelection {
                primary: RadrootsSecretBackend::HostVault(
                    radroots_secret_vault::RadrootsHostVaultPolicy::desktop(),
                ),
            },
            RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: false,
                external_command: false,
                memory: false,
            },
            "org.radroots.test.local-account",
        ) {
            Ok(_) => panic!("backend resolution error"),
            Err(err) => err,
        };

        assert_eq!(
            err.to_string(),
            "vault error: secret backend host_vault is unavailable"
        );
    }

    #[test]
    fn new_local_file_backed_reports_store_load_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        let err = match RadrootsNostrAccountsManager::new_local_file_backed(
            temp.path(),
            temp.path().join("secrets"),
            RadrootsSecretBackendSelection {
                primary: RadrootsSecretBackend::EncryptedFile,
            },
            RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: true,
                external_command: false,
                memory: false,
            },
            "org.radroots.test.local-account",
        ) {
            Ok(_) => panic!("store load error"),
            Err(err) => err,
        };

        assert!(err.to_string().starts_with("store error:"));
    }

    #[test]
    fn new_local_file_backed_resolves_encrypted_file_backend() {
        let temp = tempfile::tempdir().expect("tempdir");
        let (manager, resolved) = RadrootsNostrAccountsManager::new_local_file_backed(
            temp.path().join("accounts.json"),
            temp.path().join("secrets"),
            RadrootsSecretBackendSelection {
                primary: RadrootsSecretBackend::EncryptedFile,
            },
            RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: true,
                external_command: false,
                memory: false,
            },
            "org.radroots.test.local-account",
        )
        .expect("encrypted file manager");

        assert_eq!(resolved.backend, RadrootsSecretBackend::EncryptedFile);
        assert!(manager.list_accounts().expect("accounts").is_empty());
    }

    #[test]
    #[cfg(not(feature = "os-keyring"))]
    fn local_file_backed_secret_vault_rejects_host_vault_without_feature() {
        let temp = tempfile::tempdir().expect("tempdir");
        let err = match local_file_backed_secret_vault(
            RadrootsSecretBackend::HostVault(
                radroots_secret_vault::RadrootsHostVaultPolicy::desktop(),
            ),
            temp.path(),
            "org.radroots.test.local-account".into(),
        ) {
            Ok(_) => panic!("host vault requires feature"),
            Err(err) => err,
        };

        assert_eq!(
            err.to_string(),
            "vault error: host_vault backend requires radroots_nostr_accounts os-keyring support"
        );
    }

    #[test]
    #[cfg(feature = "memory-vault")]
    fn local_file_backed_secret_vault_resolves_memory_backend() {
        let temp = tempfile::tempdir().expect("tempdir");
        let vault = local_file_backed_secret_vault(
            RadrootsSecretBackend::Memory,
            temp.path(),
            "org.radroots.test.local-account".into(),
        )
        .expect("memory vault");

        vault.store_secret("slot", "secret").expect("store");
        assert_eq!(
            vault.load_secret("slot").expect("load").as_deref(),
            Some("secret")
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

        let identity = Keys::generate();
        let public = identity.to_public();
        manager
            .upsert_public_identity(public, Some("watch".into()), true)
            .expect("watch");

        assert!(manager.default_signing_keys().expect("signing").is_none());
        let status = manager
            .default_account_status()
            .expect("default account status");
        assert_eq!(status_kind(&status), "public-only");
        let account = status_account(&status).expect("account");
        assert_eq!(account.label(), Some("watch"));
    }

    #[test]
    fn attach_secret_keys_upgrades_existing_watch_only_account() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let identity = Keys::generate();
        let account_id = manager
            .upsert_public_identity(identity.to_public(), Some("watch".into()), false)
            .expect("watch");
        manager.clear_default_account().expect("clear default");

        let attached = manager
            .attach_secret_keys(&account_id, &identity, false)
            .expect("attach secret");

        assert_eq!(attached.id(), account_id);
        assert_eq!(attached.label(), Some("watch"));
        assert_eq!(
            attached.public_identity().public_key().to_hex(),
            identity.public_key_hex()
        );
        assert_eq!(manager.list_accounts().expect("list").len(), 1);
        let signing_identity = manager
            .get_signing_keys(&account_id)
            .expect("signing")
            .expect("secret backed");
        assert_eq!(signing_identity.public_key_hex(), identity.public_key_hex());
        assert_eq!(manager.default_account_id().expect("default"), None);
    }

    #[test]
    fn attach_secret_keys_preserves_existing_default_when_not_requested() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let default_account_id = manager
            .generate_keys(Some("primary".into()), true)
            .expect("primary");
        let identity = Keys::generate();
        let account_id = manager
            .upsert_public_identity(identity.to_public(), Some("watch".into()), false)
            .expect("watch");

        manager
            .attach_secret_keys(&account_id, &identity, false)
            .expect("attach secret");

        assert_eq!(
            manager.default_account_id().expect("default"),
            Some(default_account_id)
        );
    }

    #[test]
    fn attach_secret_keys_can_explicitly_make_default() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        manager
            .generate_keys(Some("primary".into()), true)
            .expect("primary");
        let identity = Keys::generate();
        let account_id = manager
            .upsert_public_identity(identity.to_public(), Some("watch".into()), false)
            .expect("watch");

        manager
            .attach_secret_keys(&account_id, &identity, true)
            .expect("attach secret");

        assert_eq!(
            manager.default_account_id().expect("default"),
            Some(account_id)
        );
    }

    #[test]
    fn attach_secret_keys_rejects_missing_account_without_storing_secret() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let identity = Keys::generate();
        let missing_id = identity.id();

        let err = manager
            .attach_secret_keys(&missing_id, &identity, false)
            .expect_err("missing account");

        assert_eq!(err.to_string(), format!("account not found: {missing_id}"));
        assert!(
            manager
                .export_secret_hex(&missing_id)
                .expect("export")
                .is_none()
        );
        assert!(manager.list_accounts().expect("list").is_empty());
    }

    #[test]
    fn attach_secret_keys_rejects_public_key_mismatch_without_storing_secret() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let public_identity = Keys::generate();
        let account_id = manager
            .upsert_public_identity(public_identity.to_public(), Some("watch".into()), false)
            .expect("watch");
        manager.clear_default_account().expect("clear default");
        let mismatched_identity = Keys::generate();

        let err = manager
            .attach_secret_keys(&account_id, &mismatched_identity, false)
            .expect_err("public key mismatch");

        assert_eq!(err.to_string(), "public key does not match secret key");
        assert!(
            manager
                .export_secret_hex(&account_id)
                .expect("export")
                .is_none()
        );
        assert!(
            manager
                .get_signing_keys(&account_id)
                .expect("signing")
                .is_none()
        );
        assert_eq!(manager.default_account_id().expect("default"), None);
    }

    #[test]
    fn attach_secret_keys_reports_vault_store_error() {
        let manager = RadrootsNostrAccountsManager::new(
            Arc::new(RadrootsNostrMemoryAccountStore::new()),
            Arc::new(VaultStoreError),
        )
        .expect("manager");
        let identity = Keys::generate();
        let account_id = manager
            .upsert_public_identity(identity.to_public(), Some("watch".into()), false)
            .expect("watch");

        let err = manager
            .attach_secret_keys(&account_id, &identity, false)
            .expect_err("vault store error");

        assert!(err.to_string().starts_with("vault error:"));
    }

    #[test]
    fn attach_secret_keys_reports_store_save_error_after_secret_store() {
        let identity = Keys::generate();
        let public_identity = identity.to_public();
        let account_id = AccountId::from_public_identity(&public_identity);
        let mut state = RadrootsNostrAccountStoreState::default();
        state
            .accounts
            .push(AccountRecord::new(public_identity, Some("watch".into()), 1));
        let manager = RadrootsNostrAccountsManager::new(
            Arc::new(SaveErrorStore::new(state)),
            Arc::new(RadrootsNostrSecretVaultMemory::new()),
        )
        .expect("manager");

        let err = manager
            .attach_secret_keys(&account_id, &identity, false)
            .expect_err("store save error");

        assert_eq!(err.to_string(), "store error: store save failed");
    }

    #[test]
    fn default_account_status_reports_ready_for_signing_identity() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let default_account_id = manager
            .generate_keys(Some("primary".into()), true)
            .expect("generate");

        let status = manager
            .default_account_status()
            .expect("default account status");
        assert_eq!(status_kind(&status), "ready");
        let account = status_account(&status).expect("account");
        assert_eq!(account.id(), default_account_id);
        assert_eq!(account.label(), Some("primary"));

        let signer = manager
            .default_signer_capability()
            .expect("default signer capability")
            .expect("signer capability");
        let local = signer.local_account().expect("local signer");
        assert_eq!(local.account_id, default_account_id);
        assert!(local.is_secret_backed());
    }

    #[test]
    fn manager_source_rejects_identity_migration_api() {
        assert!(
            !production_manager_source()
                .contains(removed_surface_fixtures::MIGRATE_LEGACY_IDENTITY_FILE),
            "nostr accounts manager must not expose removed identity migration API"
        );
    }

    #[test]
    fn model_source_rejects_account_store_aliases() {
        let source = include_str!("model.rs");
        assert!(
            !source.contains(removed_surface_fixtures::SELECTED_ACCOUNT_ID),
            "nostr account store model must not accept removed account-store field aliases"
        );
        assert!(
            !source.contains(removed_surface_fixtures::SERDE_ALIAS_ATTRIBUTE),
            "nostr account store model must not expose serde alias compatibility"
        );
    }

    #[test]
    fn upsert_public_identity_without_label_preserves_existing_label() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let account_id = manager
            .generate_keys(Some("primary".into()), true)
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
            .find(|record| record.id() == account_id)
            .expect("account");
        assert_eq!(record.label(), Some("primary"));
    }

    #[test]
    fn new_rejects_unsupported_schema_version() {
        let store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());
        let state = RadrootsNostrAccountStoreState {
            version: crate::model::RADROOTS_NOSTR_ACCOUNTS_STORE_VERSION + 1,
            ..Default::default()
        };
        store.save(&state).expect("save");

        let err = match RadrootsNostrAccountsManager::new(store, vault) {
            Ok(_) => panic!("unsupported schema version"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("invalid account state"));
    }

    #[test]
    fn new_clears_orphaned_default_account() {
        let store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());
        let state = RadrootsNostrAccountStoreState {
            default_account_id: Some(Keys::generate().id()),
            ..Default::default()
        };
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
                .default_signing_keys()
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

        let missing_id = Keys::generate().id();
        assert!(
            manager
                .get_signing_keys(&missing_id)
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
            .generate_keys(Some("primary".into()), true)
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
        assert_eq!(account.id(), account_id);

        let wrong_identity = Keys::generate();
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
        let load_error_public = Keys::generate().to_public();
        load_error_state.accounts.push(AccountRecord::new(
            load_error_public.clone(),
            Some("watch".into()),
            1,
        ));
        load_error_state.default_account_id =
            Some(AccountId::from_public_identity(&load_error_public));
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
        let invalid_secret_public = Keys::generate().to_public();
        invalid_secret_state.accounts.push(AccountRecord::new(
            invalid_secret_public.clone(),
            Some("invalid".into()),
            1,
        ));
        invalid_secret_state.default_account_id =
            Some(AccountId::from_public_identity(&invalid_secret_public));
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
        let invalid_secret_public = Keys::generate().to_public();
        invalid_secret_state.accounts.push(AccountRecord::new(
            invalid_secret_public.clone(),
            Some("invalid".into()),
            1,
        ));
        invalid_secret_state.default_account_id =
            Some(AccountId::from_public_identity(&invalid_secret_public));
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
            .get_signer_capability(&AccountId::from_public_identity(&invalid_secret_public))
            .expect_err("signer invalid secret");
        assert!(signer_error.to_string().starts_with("identity error:"));
    }

    #[test]
    fn select_remove_export_and_lookup_paths() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let first_id = manager
            .generate_keys(Some("first".into()), true)
            .expect("first");
        let second_id = manager
            .generate_keys(Some("second".into()), false)
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
                .get_signing_keys(&second_id)
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
                .get_signing_keys(&first_id)
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
            .generate_keys(Some("primary".into()), true)
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
            .find(|record| record.id() == AccountId::from_public_identity(&existing))
            .expect("record");
        assert_eq!(renamed.label(), Some("renamed"));

        let watch_only = Keys::generate().to_public();
        let watch_id = AccountId::from_public_identity(&watch_only);
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
    fn remove_non_default_account_keeps_current_default() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let default_account_id = manager
            .generate_keys(Some("selected".into()), true)
            .expect("default");
        let removable_id = manager
            .generate_keys(Some("removable".into()), false)
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
            .generate_keys(Some("primary".into()), true)
            .expect("primary");
        manager
            .generate_keys(Some("secondary".into()), false)
            .expect("secondary");

        manager.clear_default_account().expect("clear default");

        assert!(manager.default_account_id().expect("default").is_none());
        assert_eq!(manager.list_accounts().expect("accounts").len(), 2);
    }

    #[test]
    fn resolve_account_selector_matches_exact_id_npub_and_unique_label() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let account_id = manager
            .generate_keys(Some("primary".into()), true)
            .expect("primary");
        let default_account = manager
            .default_account()
            .expect("default account")
            .expect("default record");
        let npub =
            NostrPublicKey::from_hex(&default_account.public_identity().public_key().to_hex())
                .expect("Nostr public key")
                .to_bech32()
                .expect("npub");

        let resolved_by_id = manager
            .resolve_account_selector(&account_id.to_hex())
            .expect("resolve by id");
        assert_eq!(resolved_by_id.id(), account_id);

        let resolved_by_npub = manager
            .resolve_account_selector(&npub)
            .expect("resolve by npub");
        assert_eq!(resolved_by_npub.id(), account_id);

        let resolved_by_label = manager
            .resolve_account_selector("primary")
            .expect("resolve by label");
        assert_eq!(resolved_by_label.id(), account_id);
    }

    #[test]
    fn resolve_account_selector_rejects_empty_and_ambiguous_labels() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        manager
            .generate_keys(Some("shared".into()), true)
            .expect("first");
        manager
            .generate_keys(Some("shared".into()), false)
            .expect("second");

        let empty = manager
            .resolve_account_selector("   ")
            .expect_err("empty selector");
        assert!(empty.to_string().starts_with("invalid account selector:"));

        let ambiguous = manager
            .resolve_account_selector("shared")
            .expect_err("ambiguous selector");
        assert!(
            ambiguous
                .to_string()
                .starts_with("account selector is ambiguous:")
        );

        let missing = manager
            .resolve_account_selector("missing")
            .expect_err("missing selector");
        assert_eq!(missing.to_string(), "account not found: missing");
    }

    #[test]
    fn remove_account_propagates_vault_remove_error() {
        let store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        let vault = Arc::new(VaultRemoveError);
        let manager = RadrootsNostrAccountsManager::new(store, vault.clone()).expect("manager");
        let public = Keys::generate().to_public();
        let account_id = AccountId::from_public_identity(&public);
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
    fn resolve_signing_keys_mismatch_and_public_profile_paths() {
        let store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());
        let manager = RadrootsNostrAccountsManager::new(store, vault.clone()).expect("manager");

        let mismatch_public = Keys::generate().to_public();
        let mismatch_id = AccountId::from_public_identity(&mismatch_public);
        manager
            .upsert_public_identity(mismatch_public, Some("mismatch".into()), true)
            .expect("upsert mismatch");

        let wrong_identity = Keys::generate();
        vault
            .store_secret(
                account_secret_slot(&mismatch_id).as_str(),
                wrong_identity.secret_key_hex().as_str(),
            )
            .expect("vault store");

        let mismatch = manager
            .default_signing_keys()
            .expect_err("public key mismatch");
        assert!(
            mismatch
                .to_string()
                .contains("public key does not match secret key")
        );

        let with_profile = Keys::generate();
        let profiled_identity = with_profile.to_public().with_profile(
            Profile::new().with_username(Username::parse("profile-id").expect("username")),
        );
        let profile_id = manager
            .upsert_public_identity(profiled_identity, Some("profile".into()), true)
            .expect("upsert profile");
        manager
            .attach_secret_keys(&profile_id, &with_profile, true)
            .expect("attach profile keys");
        let resolved = manager
            .get_signing_keys(&profile_id)
            .expect("resolve")
            .expect("identity");
        assert_eq!(resolved.public_key(), with_profile.public_key());
        let stored_profile = manager
            .default_public_identity()
            .expect("default public identity")
            .expect("public identity");
        assert_eq!(
            stored_profile
                .profile()
                .and_then(Profile::username)
                .map(Username::as_str),
            Some("profile-id")
        );

        let local_signer = manager
            .get_signer_capability(&profile_id)
            .expect("local signer capability")
            .expect("local signer");
        assert!(
            manager
                .resolve_signing_keys_for_signer(&local_signer)
                .expect("resolve local signer")
                .is_some()
        );

        let remote_signer = RadrootsNostrSignerCapability::RemoteSession(Box::new(
            radroots_nostr_signer::prelude::RadrootsNostrRemoteSessionSignerCapability::new(
                radroots_nostr_signer::prelude::RadrootsNostrSignerConnectionId::new_v7(),
                Keys::generate().to_public(),
                Keys::generate().to_public(),
            ),
        ));
        assert!(
            manager
                .resolve_signing_keys_for_signer(&remote_signer)
                .expect("resolve remote signer")
                .is_none()
        );
    }

    #[test]
    fn manager_propagates_store_and_vault_errors() {
        let load_error = match RadrootsNostrAccountsManager::new(
            Arc::new(LoadErrorStore),
            Arc::new(RadrootsNostrSecretVaultMemory::new()),
        ) {
            Ok(_) => panic!("load error manager"),
            Err(err) => err,
        };
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
            .upsert_public_identity(Keys::generate().to_public(), None, true)
            .expect_err("save error");
        assert!(save_error.to_string().starts_with("store error:"));

        let vault_store_error_manager = RadrootsNostrAccountsManager::new(
            Arc::new(RadrootsNostrMemoryAccountStore::new()),
            Arc::new(VaultStoreError),
        )
        .expect("manager");
        let identity = Keys::generate();
        let vault_store_error = vault_store_error_manager
            .upsert_keys(&identity, None, true)
            .expect_err("vault store error");
        assert!(vault_store_error.to_string().starts_with("vault error:"));

        let mut load_error_state = RadrootsNostrAccountStoreState::default();
        let load_error_public = Keys::generate().to_public();
        load_error_state.accounts.push(AccountRecord::new(
            load_error_public.clone(),
            Some("watch".into()),
            1,
        ));
        load_error_state.default_account_id =
            Some(AccountId::from_public_identity(&load_error_public));
        let load_error_store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        load_error_store
            .save(&load_error_state)
            .expect("save state");
        let vault_load_error_manager =
            RadrootsNostrAccountsManager::new(load_error_store, Arc::new(VaultLoadError))
                .expect("manager");
        let vault_load_error = vault_load_error_manager
            .default_signing_keys()
            .expect_err("vault load error");
        assert!(vault_load_error.to_string().starts_with("vault error:"));

        let mut invalid_secret_state = RadrootsNostrAccountStoreState::default();
        let invalid_secret_public = Keys::generate().to_public();
        invalid_secret_state.accounts.push(AccountRecord::new(
            invalid_secret_public.clone(),
            Some("invalid".into()),
            1,
        ));
        invalid_secret_state.default_account_id =
            Some(AccountId::from_public_identity(&invalid_secret_public));
        let invalid_secret_store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        invalid_secret_store
            .save(&invalid_secret_state)
            .expect("save state");
        let invalid_secret_manager =
            RadrootsNostrAccountsManager::new(invalid_secret_store, Arc::new(VaultInvalidSecret))
                .expect("manager");
        let invalid_secret = invalid_secret_manager
            .default_signing_keys()
            .expect_err("invalid secret");
        assert!(invalid_secret.to_string().starts_with("identity error:"));
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
            .default_signing_keys()
            .expect_err("default signing poisoned");
        assert!(default_signing_err.to_string().starts_with("store error:"));
        let default_signer_err = manager
            .default_signer_capability()
            .expect_err("default signer poisoned");
        assert!(default_signer_err.to_string().starts_with("store error:"));

        let account_id = Keys::generate().id();
        let signing_err = manager
            .get_signing_keys(&account_id)
            .expect_err("signing poisoned");
        assert!(signing_err.to_string().starts_with("store error:"));
        let attach_identity = Keys::generate();
        let attach_err = manager
            .attach_secret_keys(&account_id, &attach_identity, false)
            .expect_err("attach poisoned");
        assert!(attach_err.to_string().starts_with("store error:"));
        let signer_err = manager
            .get_signer_capability(&account_id)
            .expect_err("signer poisoned");
        assert!(signer_err.to_string().starts_with("store error:"));
        let selector_err = manager
            .resolve_account_selector("missing")
            .expect_err("selector poisoned");
        assert!(selector_err.to_string().starts_with("store error:"));
        let clear_default_err = manager
            .clear_default_account()
            .expect_err("clear default poisoned");
        assert!(clear_default_err.to_string().starts_with("store error:"));
        let set_default_err = manager
            .set_default_account(&account_id)
            .expect_err("default poisoned");
        assert!(set_default_err.to_string().starts_with("store error:"));
        let remove_err = manager
            .remove_account(&account_id)
            .expect_err("remove poisoned");
        assert!(remove_err.to_string().starts_with("store error:"));
        let upsert_err = manager
            .upsert_public_identity(Keys::generate().to_public(), None, false)
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

        let account_id = Keys::generate().id();
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
