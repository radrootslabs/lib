use crate::error::RadrootsNostrAccountsError;
use crate::model::{RadrootsNostrAccountRecord, RadrootsNostrAccountStoreState};
use crate::store::{RadrootsNostrAccountStore, RadrootsNostrMemoryAccountStore};
use crate::vault::{RadrootsNostrSecretVault, RadrootsNostrSecretVaultMemory};
use radroots_identity::{RadrootsIdentity, RadrootsIdentityId, RadrootsIdentityPublic};
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct RadrootsNostrAccountsManager {
    store: Arc<dyn RadrootsNostrAccountStore>,
    vault: Arc<dyn RadrootsNostrSecretVault>,
    state: Arc<RwLock<RadrootsNostrAccountStoreState>>,
}

impl RadrootsNostrAccountsManager {
    pub fn new_in_memory() -> Self {
        Self {
            store: Arc::new(RadrootsNostrMemoryAccountStore::new()),
            vault: Arc::new(RadrootsNostrSecretVaultMemory::new()),
            state: Arc::new(RwLock::new(RadrootsNostrAccountStoreState::default())),
        }
    }

    pub fn new(
        store: Arc<dyn RadrootsNostrAccountStore>,
        vault: Arc<dyn RadrootsNostrSecretVault>,
    ) -> Result<Self, RadrootsNostrAccountsError> {
        let mut state = store.load()?;
        if state.version != crate::model::RADROOTS_NOSTR_ACCOUNTS_STORE_VERSION {
            return Err(RadrootsNostrAccountsError::InvalidState(format!(
                "unsupported accounts schema version {}",
                state.version
            )));
        }

        if let Some(selected) = state.selected_account_id.clone() {
            let exists = state
                .accounts
                .iter()
                .any(|record| record.account_id == selected);
            if !exists {
                state.selected_account_id = None;
            }
        }

        Ok(Self {
            store,
            vault,
            state: Arc::new(RwLock::new(state)),
        })
    }

    pub fn list_accounts(
        &self,
    ) -> Result<Vec<RadrootsNostrAccountRecord>, RadrootsNostrAccountsError> {
        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        Ok(guard.accounts.clone())
    }

    pub fn selected_account_id(
        &self,
    ) -> Result<Option<RadrootsIdentityId>, RadrootsNostrAccountsError> {
        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        Ok(guard.selected_account_id.clone())
    }

    pub fn selected_account(
        &self,
    ) -> Result<Option<RadrootsNostrAccountRecord>, RadrootsNostrAccountsError> {
        let guard = self.state.read().map_err(|_| {
            RadrootsNostrAccountsError::Store("accounts state lock poisoned".into())
        })?;
        let Some(selected) = guard.selected_account_id.as_ref() else {
            return Ok(None);
        };
        Ok(guard
            .accounts
            .iter()
            .find(|record| &record.account_id == selected)
            .cloned())
    }

    pub fn selected_public_identity(
        &self,
    ) -> Result<Option<RadrootsIdentityPublic>, RadrootsNostrAccountsError> {
        Ok(self
            .selected_account()?
            .map(|record| record.public_identity.clone()))
    }

    pub fn selected_signing_identity(
        &self,
    ) -> Result<Option<RadrootsIdentity>, RadrootsNostrAccountsError> {
        let Some(record) = self.selected_account()? else {
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

    pub fn upsert_identity(
        &self,
        identity: &RadrootsIdentity,
        label: Option<String>,
        make_selected: bool,
    ) -> Result<RadrootsIdentityId, RadrootsNostrAccountsError> {
        let account_id = identity.id();
        self.vault
            .store_secret_hex(&account_id, identity.secret_key_hex().as_str())?;

        let public_identity = identity.to_public();
        self.upsert_public_identity(public_identity, label, make_selected)
    }

    pub fn upsert_public_identity(
        &self,
        public_identity: RadrootsIdentityPublic,
        label: Option<String>,
        make_selected: bool,
    ) -> Result<RadrootsIdentityId, RadrootsNostrAccountsError> {
        let updated_at_unix = now_unix_secs();
        let account_id = public_identity.id.clone();
        self.update_state(|state| {
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

            if state.selected_account_id.is_none() || make_selected {
                state.selected_account_id = Some(account_id.clone());
            }
            Ok(())
        })?;
        Ok(account_id)
    }

    pub fn generate_identity(
        &self,
        label: Option<String>,
        make_selected: bool,
    ) -> Result<RadrootsIdentityId, RadrootsNostrAccountsError> {
        let identity = RadrootsIdentity::generate();
        self.upsert_identity(&identity, label, make_selected)
    }

    pub fn select_account(
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
            state.selected_account_id = Some(account_id);
            Ok(())
        })
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

            if state.selected_account_id.as_ref() == Some(&account_id) {
                state.selected_account_id = state
                    .accounts
                    .first()
                    .map(|record| record.account_id.clone());
            }
            Ok(())
        })?;
        self.vault.remove_secret(&account_id)?;
        Ok(())
    }

    pub fn export_secret_hex(
        &self,
        account_id: &RadrootsIdentityId,
    ) -> Result<Option<String>, RadrootsNostrAccountsError> {
        self.vault.load_secret_hex(account_id)
    }

    pub fn migrate_legacy_identity_file(
        &self,
        path: impl AsRef<Path>,
        label: Option<String>,
        make_selected: bool,
    ) -> Result<RadrootsIdentityId, RadrootsNostrAccountsError> {
        let identity = RadrootsIdentity::load_from_path_auto(path)?;
        self.upsert_identity(&identity, label, make_selected)
    }

    fn resolve_signing_identity(
        &self,
        record: RadrootsNostrAccountRecord,
    ) -> Result<Option<RadrootsIdentity>, RadrootsNostrAccountsError> {
        let Some(secret_key_hex) = self.vault.load_secret_hex(&record.account_id)? else {
            return Ok(None);
        };
        let mut identity = RadrootsIdentity::from_secret_key_str(secret_key_hex.as_str())?;
        if identity.public_key_hex() != record.public_identity.public_key_hex {
            return Err(RadrootsNostrAccountsError::PublicKeyMismatch);
        }
        if let Some(profile) = record.public_identity.profile {
            identity.set_profile(profile);
        }
        Ok(Some(identity))
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
    use crate::vault::RadrootsNostrSecretVault;
    use crate::vault::RadrootsNostrSecretVaultMemory;
    use radroots_identity::RadrootsIdentityProfile;
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

    impl RadrootsNostrSecretVault for VaultStoreError {
        fn store_secret_hex(
            &self,
            _account_id: &RadrootsIdentityId,
            _secret_key_hex: &str,
        ) -> Result<(), RadrootsNostrAccountsError> {
            Err(RadrootsNostrAccountsError::Vault(
                "vault store failed".into(),
            ))
        }

        fn load_secret_hex(
            &self,
            _account_id: &RadrootsIdentityId,
        ) -> Result<Option<String>, RadrootsNostrAccountsError> {
            Ok(None)
        }

        fn remove_secret(
            &self,
            _account_id: &RadrootsIdentityId,
        ) -> Result<(), RadrootsNostrAccountsError> {
            Ok(())
        }
    }

    struct VaultLoadError;

    impl RadrootsNostrSecretVault for VaultLoadError {
        fn store_secret_hex(
            &self,
            _account_id: &RadrootsIdentityId,
            _secret_key_hex: &str,
        ) -> Result<(), RadrootsNostrAccountsError> {
            Ok(())
        }

        fn load_secret_hex(
            &self,
            _account_id: &RadrootsIdentityId,
        ) -> Result<Option<String>, RadrootsNostrAccountsError> {
            Err(RadrootsNostrAccountsError::Vault(
                "vault load failed".into(),
            ))
        }

        fn remove_secret(
            &self,
            _account_id: &RadrootsIdentityId,
        ) -> Result<(), RadrootsNostrAccountsError> {
            Ok(())
        }
    }

    struct VaultInvalidSecret;

    impl RadrootsNostrSecretVault for VaultInvalidSecret {
        fn store_secret_hex(
            &self,
            _account_id: &RadrootsIdentityId,
            _secret_key_hex: &str,
        ) -> Result<(), RadrootsNostrAccountsError> {
            Ok(())
        }

        fn load_secret_hex(
            &self,
            _account_id: &RadrootsIdentityId,
        ) -> Result<Option<String>, RadrootsNostrAccountsError> {
            Ok(Some("invalid-secret".to_string()))
        }

        fn remove_secret(
            &self,
            _account_id: &RadrootsIdentityId,
        ) -> Result<(), RadrootsNostrAccountsError> {
            Ok(())
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

    #[test]
    fn manager_persists_selection_and_restores_signing_identity() {
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

        let selected = manager
            .selected_account_id()
            .expect("selected")
            .expect("selected id");
        assert_eq!(selected, created_id);

        let manager2 = RadrootsNostrAccountsManager::new(store, vault).expect("manager2");
        let selected2 = manager2
            .selected_account_id()
            .expect("selected2")
            .expect("selected2 id");
        assert_eq!(selected2, created_id);
        assert!(
            manager2
                .selected_signing_identity()
                .expect("signing")
                .is_some()
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
                .selected_signing_identity()
                .expect("signing")
                .is_none()
        );
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
                .selected_account_id()
                .expect("selected")
                .expect("selected id"),
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
            .selected_public_identity()
            .expect("selected public")
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

        let result = RadrootsNostrAccountsManager::new(store, vault);
        assert!(matches!(
            result,
            Err(RadrootsNostrAccountsError::InvalidState(_))
        ));
    }

    #[test]
    fn new_clears_orphaned_selected_account() {
        let store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());
        let mut state = RadrootsNostrAccountStoreState::default();
        state.selected_account_id = Some(RadrootsIdentity::generate().id());
        store.save(&state).expect("save");

        let manager = RadrootsNostrAccountsManager::new(store, vault).expect("manager");
        assert!(manager.selected_account_id().expect("selected").is_none());
    }

    #[test]
    fn selected_methods_return_none_when_state_is_empty() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        assert!(
            manager
                .selected_account()
                .expect("selected account")
                .is_none()
        );
        assert!(
            manager
                .selected_public_identity()
                .expect("selected public")
                .is_none()
        );
        assert!(
            manager
                .selected_signing_identity()
                .expect("selected signing")
                .is_none()
        );

        let missing_id = RadrootsIdentity::generate().id();
        assert!(
            manager
                .get_signing_identity(&missing_id)
                .expect("signing")
                .is_none()
        );
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

        manager.select_account(&second_id).expect("select second");
        assert_eq!(
            manager.selected_account_id().expect("selected"),
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
        assert_eq!(
            manager.selected_account_id().expect("selected"),
            Some(first_id)
        );
        assert!(
            manager
                .export_secret_hex(&second_id)
                .expect("export after remove")
                .is_none()
        );

        let select_missing = manager.select_account(&second_id);
        assert!(matches!(
            select_missing,
            Err(RadrootsNostrAccountsError::AccountNotFound(_))
        ));
        let remove_missing = manager.remove_account(&second_id);
        assert!(matches!(
            remove_missing,
            Err(RadrootsNostrAccountsError::AccountNotFound(_))
        ));
    }

    #[test]
    fn upsert_public_identity_updates_label_and_respects_selection_flag() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        manager
            .generate_identity(Some("primary".into()), true)
            .expect("generate");

        let existing = manager
            .selected_public_identity()
            .expect("selected public")
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
        let make_selected = manager.selected_account_id().expect("selected").is_some();
        manager
            .upsert_public_identity(watch_only, Some("watch".into()), make_selected)
            .expect("upsert watch");
        assert_eq!(
            manager.selected_account_id().expect("selected"),
            Some(watch_id)
        );
    }

    #[test]
    fn remove_non_selected_account_keeps_current_selection() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let selected_id = manager
            .generate_identity(Some("selected".into()), true)
            .expect("selected");
        let removable_id = manager
            .generate_identity(Some("removable".into()), false)
            .expect("removable");

        manager.remove_account(&removable_id).expect("remove");
        assert_eq!(
            manager.selected_account_id().expect("selected"),
            Some(selected_id)
        );
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
            .store_secret_hex(&mismatch_id, wrong_identity.secret_key_hex().as_str())
            .expect("vault store");

        let mismatch = manager.selected_signing_identity();
        assert!(matches!(
            mismatch,
            Err(RadrootsNostrAccountsError::PublicKeyMismatch)
        ));

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
    }

    #[test]
    fn manager_propagates_store_and_vault_errors() {
        let load_error = RadrootsNostrAccountsManager::new(
            Arc::new(LoadErrorStore),
            Arc::new(RadrootsNostrSecretVaultMemory::new()),
        );
        assert!(matches!(
            load_error,
            Err(RadrootsNostrAccountsError::Store(_))
        ));

        let save_error_store = Arc::new(SaveErrorStore::new(
            RadrootsNostrAccountStoreState::default(),
        ));
        let save_error_manager = RadrootsNostrAccountsManager::new(
            save_error_store,
            Arc::new(RadrootsNostrSecretVaultMemory::new()),
        )
        .expect("manager");
        let save_error = save_error_manager.upsert_public_identity(
            RadrootsIdentity::generate().to_public(),
            None,
            true,
        );
        assert!(matches!(
            save_error,
            Err(RadrootsNostrAccountsError::Store(_))
        ));

        let vault_store_error_manager = RadrootsNostrAccountsManager::new(
            Arc::new(RadrootsNostrMemoryAccountStore::new()),
            Arc::new(VaultStoreError),
        )
        .expect("manager");
        let identity = RadrootsIdentity::generate();
        let vault_store_error = vault_store_error_manager.upsert_identity(&identity, None, true);
        assert!(matches!(
            vault_store_error,
            Err(RadrootsNostrAccountsError::Vault(_))
        ));

        let mut load_error_state = RadrootsNostrAccountStoreState::default();
        let load_error_public = RadrootsIdentity::generate().to_public();
        load_error_state
            .accounts
            .push(RadrootsNostrAccountRecord::new(
                load_error_public.clone(),
                Some("watch".into()),
                1,
            ));
        load_error_state.selected_account_id = Some(load_error_public.id.clone());
        let load_error_store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        load_error_store
            .save(&load_error_state)
            .expect("save state");
        let vault_load_error_manager =
            RadrootsNostrAccountsManager::new(load_error_store, Arc::new(VaultLoadError))
                .expect("manager");
        let vault_load_error = vault_load_error_manager.selected_signing_identity();
        assert!(matches!(
            vault_load_error,
            Err(RadrootsNostrAccountsError::Vault(_))
        ));

        let mut invalid_secret_state = RadrootsNostrAccountStoreState::default();
        let invalid_secret_public = RadrootsIdentity::generate().to_public();
        invalid_secret_state
            .accounts
            .push(RadrootsNostrAccountRecord::new(
                invalid_secret_public.clone(),
                Some("invalid".into()),
                1,
            ));
        invalid_secret_state.selected_account_id = Some(invalid_secret_public.id.clone());
        let invalid_secret_store = Arc::new(RadrootsNostrMemoryAccountStore::new());
        invalid_secret_store
            .save(&invalid_secret_state)
            .expect("save state");
        let invalid_secret_manager =
            RadrootsNostrAccountsManager::new(invalid_secret_store, Arc::new(VaultInvalidSecret))
                .expect("manager");
        let invalid_secret = invalid_secret_manager.selected_signing_identity();
        assert!(matches!(
            invalid_secret,
            Err(RadrootsNostrAccountsError::Identity(_))
        ));
    }

    #[test]
    fn migrate_legacy_identity_file_returns_error_for_missing_path() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        let temp = tempfile::tempdir().expect("tempdir");
        let missing = temp.path().join("missing_legacy.json");
        let migrated = manager.migrate_legacy_identity_file(&missing, None, false);
        assert!(matches!(
            migrated,
            Err(RadrootsNostrAccountsError::Identity(_))
        ));
    }

    #[test]
    fn manager_reports_poisoned_state_locks() {
        let manager = RadrootsNostrAccountsManager::new_in_memory();
        poison_manager_state(&manager);

        assert!(matches!(
            manager.list_accounts(),
            Err(RadrootsNostrAccountsError::Store(_))
        ));
        assert!(matches!(
            manager.selected_account_id(),
            Err(RadrootsNostrAccountsError::Store(_))
        ));
        assert!(matches!(
            manager.selected_account(),
            Err(RadrootsNostrAccountsError::Store(_))
        ));

        let account_id = RadrootsIdentity::generate().id();
        assert!(matches!(
            manager.get_signing_identity(&account_id),
            Err(RadrootsNostrAccountsError::Store(_))
        ));
        assert!(matches!(
            manager.select_account(&account_id),
            Err(RadrootsNostrAccountsError::Store(_))
        ));
        assert!(matches!(
            manager.remove_account(&account_id),
            Err(RadrootsNostrAccountsError::Store(_))
        ));
        assert!(matches!(
            manager.upsert_public_identity(RadrootsIdentity::generate().to_public(), None, false),
            Err(RadrootsNostrAccountsError::Store(_))
        ));
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
        let poisoned_load = save_error_store.load();
        assert!(matches!(
            poisoned_load,
            Err(RadrootsNostrAccountsError::Store(_))
        ));

        let account_id = RadrootsIdentity::generate().id();
        let vault_store_error = VaultStoreError;
        assert!(
            vault_store_error
                .load_secret_hex(&account_id)
                .expect("load")
                .is_none()
        );
        vault_store_error
            .remove_secret(&account_id)
            .expect("remove");

        let vault_load_error = VaultLoadError;
        vault_load_error
            .store_secret_hex(&account_id, "secret")
            .expect("store");
        vault_load_error.remove_secret(&account_id).expect("remove");

        let vault_invalid_secret = VaultInvalidSecret;
        vault_invalid_secret
            .store_secret_hex(&account_id, "secret")
            .expect("store");
        vault_invalid_secret
            .remove_secret(&account_id)
            .expect("remove");
    }
}
