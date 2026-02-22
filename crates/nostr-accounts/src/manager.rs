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
    use crate::store::RadrootsNostrFileAccountStore;
    use crate::vault::RadrootsNostrSecretVaultMemory;
    use std::sync::Arc;

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
}
