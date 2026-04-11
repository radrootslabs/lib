use std::fs;
use std::path::Path;

use crate::error::RadrootsRuntimeManagerError;
use crate::model::{ManagedRuntimeInstanceRecord, ManagedRuntimeInstanceRegistry};

pub fn load_registry(
    path: impl AsRef<Path>,
) -> Result<ManagedRuntimeInstanceRegistry, RadrootsRuntimeManagerError> {
    let path = path.as_ref();
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ManagedRuntimeInstanceRegistry::default());
        }
        Err(source) => {
            return Err(RadrootsRuntimeManagerError::ReadRegistry {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    toml::from_str::<ManagedRuntimeInstanceRegistry>(&raw).map_err(|source| {
        RadrootsRuntimeManagerError::ParseRegistry {
            path: path.to_path_buf(),
            details: source.to_string(),
        }
    })
}

pub fn save_registry(
    path: impl AsRef<Path>,
    registry: &ManagedRuntimeInstanceRegistry,
) -> Result<(), RadrootsRuntimeManagerError> {
    let path = path.as_ref();
    ensure_registry_parent(path)?;

    let raw = toml::to_string_pretty(registry)
        .map_err(|err| RadrootsRuntimeManagerError::SerializeRegistry(err.to_string()))?;
    fs::write(path, raw).map_err(|source| RadrootsRuntimeManagerError::WriteRegistry {
        path: path.to_path_buf(),
        source,
    })
}

pub fn upsert_instance(
    registry: &mut ManagedRuntimeInstanceRegistry,
    record: ManagedRuntimeInstanceRecord,
) {
    if let Some(existing) = registry.instances.iter_mut().find(|existing| {
        existing.runtime_id == record.runtime_id && existing.instance_id == record.instance_id
    }) {
        *existing = record;
    } else {
        registry.instances.push(record);
        registry.instances.sort_by(|left, right| {
            left.runtime_id
                .cmp(&right.runtime_id)
                .then_with(|| left.instance_id.cmp(&right.instance_id))
        });
    }
}

pub fn instance<'a>(
    registry: &'a ManagedRuntimeInstanceRegistry,
    runtime_id: &str,
    instance_id: &str,
) -> Option<&'a ManagedRuntimeInstanceRecord> {
    registry
        .instances
        .iter()
        .find(|record| record.runtime_id == runtime_id && record.instance_id == instance_id)
}

pub fn remove_instance(
    registry: &mut ManagedRuntimeInstanceRegistry,
    runtime_id: &str,
    instance_id: &str,
) -> Option<ManagedRuntimeInstanceRecord> {
    let index = registry
        .instances
        .iter()
        .position(|record| record.runtime_id == runtime_id && record.instance_id == instance_id)?;
    Some(registry.instances.remove(index))
}

fn ensure_registry_parent(path: &Path) -> Result<(), RadrootsRuntimeManagerError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }
    fs::create_dir_all(parent).map_err(|source| RadrootsRuntimeManagerError::CreateRegistryParent {
        path: parent.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use super::{
        ensure_registry_parent, instance, load_registry, remove_instance, save_registry,
        upsert_instance,
    };
    use crate::{
        ManagedRuntimeInstallState, ManagedRuntimeInstanceRecord, ManagedRuntimeInstanceRegistry,
        RadrootsRuntimeManagerError,
    };

    fn sample_record(runtime_id: &str, instance_id: &str) -> ManagedRuntimeInstanceRecord {
        ManagedRuntimeInstanceRecord {
            runtime_id: runtime_id.to_string(),
            instance_id: instance_id.to_string(),
            management_mode: "interactive_user_managed".to_string(),
            install_state: ManagedRuntimeInstallState::Configured,
            binary_path: PathBuf::from("/tmp/radrootsd"),
            config_path: PathBuf::from("/tmp/config.toml"),
            logs_path: PathBuf::from("/tmp/logs"),
            run_path: PathBuf::from("/tmp/run"),
            installed_version: "0.1.0-alpha.1".to_string(),
            health_endpoint: Some("jsonrpc_status".to_string()),
            secret_material_ref: None,
            last_started_at: None,
            last_stopped_at: None,
            notes: Some("test".to_string()),
        }
    }

    fn assert_error_contains(err: &RadrootsRuntimeManagerError, parts: &[&str]) {
        let rendered = err.to_string();
        for part in parts {
            assert!(
                rendered.contains(part),
                "expected `{rendered}` to contain `{part}`"
            );
        }
    }

    #[test]
    fn load_registry_returns_default_when_file_is_missing() {
        let dir = tempdir().expect("tempdir");
        let registry = load_registry(dir.path().join("missing.toml")).expect("missing registry");
        assert_eq!(registry, ManagedRuntimeInstanceRegistry::default());
    }

    #[test]
    fn load_registry_reports_read_errors() {
        let dir = tempdir().expect("tempdir");
        let err = load_registry(dir.path()).expect_err("directory should fail");
        assert_error_contains(
            &err,
            &[
                dir.path().to_string_lossy().as_ref(),
                "read runtime instance registry",
            ],
        );
    }

    #[test]
    fn load_registry_reports_parse_errors() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("instances.toml");
        fs::write(&path, "not = [valid").expect("write invalid registry");

        let err = load_registry(&path).expect_err("invalid registry should fail");
        assert_error_contains(
            &err,
            &[
                path.to_string_lossy().as_ref(),
                "parse runtime instance registry",
            ],
        );
    }

    #[test]
    fn save_registry_reports_write_errors() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("registry-dir");
        fs::create_dir(&path).expect("create directory target");

        let err = save_registry(&path, &ManagedRuntimeInstanceRegistry::default())
            .expect_err("directory path should fail");
        assert_error_contains(
            &err,
            &[
                path.to_string_lossy().as_ref(),
                "write runtime instance registry",
            ],
        );
    }

    #[test]
    fn save_registry_reports_parent_creation_errors() {
        let dir = tempdir().expect("tempdir");
        let file_parent = dir.path().join("occupied");
        fs::write(&file_parent, "file").expect("occupied parent");
        let path = file_parent.join("instances.toml");

        let err = save_registry(&path, &ManagedRuntimeInstanceRegistry::default())
            .expect_err("file parent should fail");
        assert_error_contains(
            &err,
            &[
                file_parent.to_string_lossy().as_ref(),
                "create runtime instance registry parent",
            ],
        );
    }

    #[test]
    fn ensure_registry_parent_accepts_parentless_relative_paths() {
        ensure_registry_parent(Path::new("instances.toml")).expect("relative path parentless");
        ensure_registry_parent(Path::new("/")).expect("root path parentless");
    }

    #[test]
    fn upsert_instance_replaces_existing_and_sorts_new_records() {
        let mut registry = ManagedRuntimeInstanceRegistry::default();
        upsert_instance(&mut registry, sample_record("radrootsd", "b"));
        upsert_instance(&mut registry, sample_record("myc", "a"));

        let mut replacement = sample_record("radrootsd", "b");
        replacement.installed_version = "0.2.0".to_string();
        upsert_instance(&mut registry, replacement);

        assert_eq!(registry.instances.len(), 2);
        assert_eq!(registry.instances[0].runtime_id, "myc");
        assert_eq!(registry.instances[1].runtime_id, "radrootsd");
        assert_eq!(registry.instances[1].installed_version, "0.2.0");
    }

    #[test]
    fn instance_and_remove_instance_handle_missing_and_present_rows() {
        let mut registry = ManagedRuntimeInstanceRegistry::default();
        upsert_instance(&mut registry, sample_record("radrootsd", "local"));

        assert!(instance(&registry, "myc", "local").is_none());
        assert!(remove_instance(&mut registry, "myc", "local").is_none());

        let removed = remove_instance(&mut registry, "radrootsd", "local").expect("remove");
        assert_eq!(removed.runtime_id, "radrootsd");
        assert!(registry.instances.is_empty());
    }
}
