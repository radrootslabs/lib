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
        file.save()?;
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
}
