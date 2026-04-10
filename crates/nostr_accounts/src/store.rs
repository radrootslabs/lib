use crate::error::RadrootsNostrAccountsError;
use crate::model::RadrootsNostrAccountStoreState;
use radroots_runtime::json::{JsonFile, JsonWriteOptions};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

pub trait RadrootsNostrAccountStore: Send + Sync {
    fn load(&self) -> Result<RadrootsNostrAccountStoreState, RadrootsNostrAccountsError>;
    fn save(
        &self,
        state: &RadrootsNostrAccountStoreState,
    ) -> Result<(), RadrootsNostrAccountsError>;
}

#[derive(Debug, Clone)]
pub struct RadrootsNostrFileAccountStore {
    path: PathBuf,
}

impl RadrootsNostrFileAccountStore {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
}

#[derive(Debug, Clone, Default)]
pub struct RadrootsNostrMemoryAccountStore {
    state: Arc<RwLock<RadrootsNostrAccountStoreState>>,
}

impl RadrootsNostrMemoryAccountStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RadrootsNostrAccountStore for RadrootsNostrFileAccountStore {
    fn load(&self) -> Result<RadrootsNostrAccountStoreState, RadrootsNostrAccountsError> {
        if !self.path.exists() {
            return Ok(RadrootsNostrAccountStoreState::default());
        }
        let file = JsonFile::<RadrootsNostrAccountStoreState>::load(self.path.as_path())?;
        Ok(file.value)
    }

    fn save(
        &self,
        state: &RadrootsNostrAccountStoreState,
    ) -> Result<(), RadrootsNostrAccountsError> {
        let mut file = JsonFile::load_or_create_with(self.path.as_path(), || state.clone())?;
        file.set_options(JsonWriteOptions {
            pretty: true,
            mode_unix: Some(0o600),
        });
        file.value = state.clone();
        if let Err(err) = file.save() {
            return Err(err.into());
        }
        Ok(())
    }
}

impl RadrootsNostrAccountStore for RadrootsNostrMemoryAccountStore {
    fn load(&self) -> Result<RadrootsNostrAccountStoreState, RadrootsNostrAccountsError> {
        let guard = self
            .state
            .read()
            .map_err(|_| RadrootsNostrAccountsError::Store("memory store lock poisoned".into()))?;
        Ok(guard.clone())
    }

    fn save(
        &self,
        state: &RadrootsNostrAccountStoreState,
    ) -> Result<(), RadrootsNostrAccountsError> {
        let mut guard = self
            .state
            .write()
            .map_err(|_| RadrootsNostrAccountsError::Store("memory store lock poisoned".into()))?;
        *guard = state.clone();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn file_store_round_trip() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("accounts.json");
        let store = RadrootsNostrFileAccountStore::new(path.as_path());

        let state = RadrootsNostrAccountStoreState::default();
        store.save(&state).expect("save");
        let loaded = store.load().expect("load");
        assert_eq!(loaded.version, state.version);
        assert!(loaded.accounts.is_empty());
    }

    #[test]
    fn file_store_load_missing_and_path_accessor() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("missing.json");
        let store = RadrootsNostrFileAccountStore::new(path.as_path());

        assert_eq!(store.path(), path.as_path());
        let loaded = store.load().expect("load");
        assert_eq!(
            loaded.version,
            RadrootsNostrAccountStoreState::default().version
        );
        assert!(loaded.accounts.is_empty());
    }

    #[test]
    fn file_store_load_reports_parse_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("invalid.json");
        std::fs::write(&path, "{").expect("write invalid json");
        let store = RadrootsNostrFileAccountStore::new(path.as_path());

        let err = store.load().expect_err("invalid json");
        assert!(err.to_string().starts_with("store error:"));
    }

    #[test]
    fn file_store_save_reports_parse_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("invalid.json");
        std::fs::write(&path, "{").expect("write invalid json");
        let store = RadrootsNostrFileAccountStore::new(path.as_path());

        let err = store
            .save(&RadrootsNostrAccountStoreState::default())
            .expect_err("invalid json save");
        assert!(err.to_string().starts_with("store error:"));
    }

    #[cfg(unix)]
    #[test]
    fn file_store_save_reports_write_error() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("accounts.json");
        let json =
            serde_json::to_string(&RadrootsNostrAccountStoreState::default()).expect("serialize");
        std::fs::write(&path, json).expect("write json");
        let store = RadrootsNostrFileAccountStore::new(path.as_path());

        let mut perms = std::fs::metadata(temp.path())
            .expect("dir metadata")
            .permissions();
        perms.set_mode(0o500);
        std::fs::set_permissions(temp.path(), perms).expect("set perms");

        let err = store
            .save(&RadrootsNostrAccountStoreState::default())
            .expect_err("read-only save");
        assert!(err.to_string().starts_with("store error:"));

        let mut perms = std::fs::metadata(temp.path())
            .expect("dir metadata")
            .permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(temp.path(), perms).expect("restore perms");
    }

    #[test]
    fn memory_store_round_trip() {
        let store = RadrootsNostrMemoryAccountStore::new();
        let state = RadrootsNostrAccountStoreState::default();
        store.save(&state).expect("save");

        let loaded = store.load().expect("load");
        assert_eq!(loaded.version, state.version);
        assert_eq!(loaded.selected_account_id, state.selected_account_id);
    }

    #[test]
    fn memory_store_reports_poisoned_lock() {
        let store = RadrootsNostrMemoryAccountStore::new();
        let shared = store.state.clone();
        let _ = thread::spawn(move || {
            let _guard = shared.write().expect("write");
            panic!("poison memory store");
        })
        .join();

        let load = store.load().expect_err("poisoned load");
        assert!(load.to_string().contains("memory store lock poisoned"));

        let save = store
            .save(&RadrootsNostrAccountStoreState::default())
            .expect_err("poisoned save");
        assert!(save.to_string().contains("memory store lock poisoned"));
    }
}
