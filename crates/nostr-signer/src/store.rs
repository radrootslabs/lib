use crate::error::RadrootsNostrSignerError;
use crate::model::RadrootsNostrSignerStoreState;
use radroots_runtime::json::{JsonFile, JsonWriteOptions};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

pub trait RadrootsNostrSignerStore: Send + Sync {
    fn load(&self) -> Result<RadrootsNostrSignerStoreState, RadrootsNostrSignerError>;
    fn save(&self, state: &RadrootsNostrSignerStoreState) -> Result<(), RadrootsNostrSignerError>;
}

#[derive(Debug, Clone)]
pub struct RadrootsNostrFileSignerStore {
    path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct RadrootsNostrMemorySignerStore {
    state: Arc<RwLock<RadrootsNostrSignerStoreState>>,
}

impl RadrootsNostrFileSignerStore {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
}

impl RadrootsNostrMemorySignerStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RadrootsNostrSignerStore for RadrootsNostrFileSignerStore {
    fn load(&self) -> Result<RadrootsNostrSignerStoreState, RadrootsNostrSignerError> {
        if !self.path.exists() {
            return Ok(RadrootsNostrSignerStoreState::default());
        }
        let file = JsonFile::<RadrootsNostrSignerStoreState>::load(self.path.as_path())?;
        Ok(file.value)
    }

    fn save(&self, state: &RadrootsNostrSignerStoreState) -> Result<(), RadrootsNostrSignerError> {
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

impl RadrootsNostrSignerStore for RadrootsNostrMemorySignerStore {
    fn load(&self) -> Result<RadrootsNostrSignerStoreState, RadrootsNostrSignerError> {
        let guard = self
            .state
            .read()
            .map_err(|_| RadrootsNostrSignerError::Store("memory store lock poisoned".into()))?;
        Ok(guard.clone())
    }

    fn save(&self, state: &RadrootsNostrSignerStoreState) -> Result<(), RadrootsNostrSignerError> {
        let mut guard = self
            .state
            .write()
            .map_err(|_| RadrootsNostrSignerError::Store("memory store lock poisoned".into()))?;
        *guard = state.clone();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn file_store_round_trip_and_path_accessor() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("signer.json");
        let store = RadrootsNostrFileSignerStore::new(path.as_path());

        assert_eq!(store.path(), path.as_path());
        store
            .save(&RadrootsNostrSignerStoreState::default())
            .expect("save");
        let loaded = store.load().expect("load");
        assert_eq!(
            loaded.version,
            RadrootsNostrSignerStoreState::default().version
        );
        assert!(loaded.connections.is_empty());
    }

    #[test]
    fn file_store_load_missing_and_reports_parse_errors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let missing = RadrootsNostrFileSignerStore::new(temp.path().join("missing.json"));
        let loaded = missing.load().expect("missing load");
        assert!(loaded.connections.is_empty());

        let path = temp.path().join("invalid.json");
        std::fs::write(&path, "{").expect("write invalid json");
        let store = RadrootsNostrFileSignerStore::new(path.as_path());
        let err = store.load().expect_err("invalid json");
        assert!(err.to_string().starts_with("store error:"));
    }

    #[test]
    fn file_store_save_reports_parse_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("invalid-save.json");
        std::fs::write(&path, "{").expect("write invalid json");
        let store = RadrootsNostrFileSignerStore::new(path.as_path());
        let err = store
            .save(&RadrootsNostrSignerStoreState::default())
            .expect_err("invalid save");
        assert!(err.to_string().starts_with("store error:"));
    }

    #[cfg(unix)]
    #[test]
    fn file_store_save_reports_write_error() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("signer.json");
        let json =
            serde_json::to_string(&RadrootsNostrSignerStoreState::default()).expect("serialize");
        std::fs::write(&path, json).expect("write json");
        let store = RadrootsNostrFileSignerStore::new(path.as_path());

        let mut perms = std::fs::metadata(temp.path())
            .expect("dir metadata")
            .permissions();
        perms.set_mode(0o500);
        std::fs::set_permissions(temp.path(), perms).expect("set perms");

        let err = store
            .save(&RadrootsNostrSignerStoreState::default())
            .expect_err("read-only save");
        assert!(err.to_string().starts_with("store error:"));

        let mut perms = std::fs::metadata(temp.path())
            .expect("dir metadata")
            .permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(temp.path(), perms).expect("restore perms");
    }

    #[test]
    fn memory_store_round_trip_and_poison_errors() {
        let store = RadrootsNostrMemorySignerStore::new();
        let state = RadrootsNostrSignerStoreState::default();
        store.save(&state).expect("save");
        let loaded = store.load().expect("load");
        assert_eq!(loaded.version, state.version);

        let shared = store.state.clone();
        let _ = thread::spawn(move || {
            let _guard = shared.write().expect("write");
            panic!("poison memory store");
        })
        .join();

        let load = store.load().expect_err("poisoned load");
        let save = store.save(&state).expect_err("poisoned save");
        assert!(load.to_string().contains("memory store lock poisoned"));
        assert!(save.to_string().contains("memory store lock poisoned"));
    }
}
