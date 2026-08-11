use std::fs;
use std::path::Path;

use radroots_runtime_paths::{InstanceId, ServiceId};

use crate::error::RadrootsRuntimeManagerError;
use crate::model::{
    ManagedRuntimeInstanceRecord, ManagedRuntimeInstanceRegistry, RUNTIME_INSTANCE_REGISTRY_SCHEMA,
    RUNTIME_INSTANCE_REGISTRY_VERSION,
};

pub fn load_registry(
    path: impl AsRef<Path>,
) -> Result<ManagedRuntimeInstanceRegistry, RadrootsRuntimeManagerError> {
    load_registry_path(path.as_ref())
}

fn load_registry_path(
    path: &Path,
) -> Result<ManagedRuntimeInstanceRegistry, RadrootsRuntimeManagerError> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ManagedRuntimeInstanceRegistry::default());
        }
        Err(source) => {
            return Err(RadrootsRuntimeManagerError::ReadRegistry {
                kind: source.kind(),
            });
        }
    };

    let registry = toml::from_str::<ManagedRuntimeInstanceRegistry>(&raw)
        .map_err(|_| RadrootsRuntimeManagerError::ParseRegistry)?;
    normalize_registry(registry)
}

pub fn save_registry(
    path: impl AsRef<Path>,
    registry: &ManagedRuntimeInstanceRegistry,
) -> Result<(), RadrootsRuntimeManagerError> {
    save_registry_path(path.as_ref(), registry)
}

fn save_registry_path(
    path: &Path,
    registry: &ManagedRuntimeInstanceRegistry,
) -> Result<(), RadrootsRuntimeManagerError> {
    save_registry_path_with(path, registry, toml::to_string_pretty)
}

fn save_registry_path_with(
    path: &Path,
    registry: &ManagedRuntimeInstanceRegistry,
    serializer: fn(&ManagedRuntimeInstanceRegistry) -> Result<String, toml::ser::Error>,
) -> Result<(), RadrootsRuntimeManagerError> {
    ensure_registry_parent(path)?;

    let normalized = normalize_registry(registry.clone())?;
    let raw =
        serializer(&normalized).map_err(|_| RadrootsRuntimeManagerError::SerializeRegistry)?;
    fs::write(path, raw).map_err(|source| RadrootsRuntimeManagerError::WriteRegistry {
        kind: source.kind(),
    })
}

fn normalize_registry(
    mut registry: ManagedRuntimeInstanceRegistry,
) -> Result<ManagedRuntimeInstanceRegistry, RadrootsRuntimeManagerError> {
    if registry.schema != RUNTIME_INSTANCE_REGISTRY_SCHEMA {
        return Err(RadrootsRuntimeManagerError::UnexpectedRegistrySchema);
    }
    if registry.schema_version != RUNTIME_INSTANCE_REGISTRY_VERSION {
        return Err(RadrootsRuntimeManagerError::UnexpectedRegistryVersion);
    }
    registry.instances.sort_by(|left, right| {
        left.service_id()
            .cmp(right.service_id())
            .then_with(|| left.instance_id().cmp(right.instance_id()))
    });
    if registry.instances.windows(2).any(|pair| {
        pair[0].service_id() == pair[1].service_id()
            && pair[0].instance_id() == pair[1].instance_id()
    }) {
        return Err(RadrootsRuntimeManagerError::DuplicateRegistryInstance);
    }
    Ok(registry)
}

pub(crate) fn upsert_instance(
    registry: &mut ManagedRuntimeInstanceRegistry,
    record: ManagedRuntimeInstanceRecord,
) {
    if let Some(existing) = registry.instances.iter_mut().find(|existing| {
        existing.service_id() == record.service_id()
            && existing.instance_id() == record.instance_id()
    }) {
        *existing = record;
    } else {
        registry.instances.push(record);
        registry.instances.sort_by(|left, right| {
            left.service_id()
                .cmp(right.service_id())
                .then_with(|| left.instance_id().cmp(right.instance_id()))
        });
    }
}

pub fn instance<'a>(
    registry: &'a ManagedRuntimeInstanceRegistry,
    service_id: &ServiceId,
    instance_id: &InstanceId,
) -> Option<&'a ManagedRuntimeInstanceRecord> {
    registry
        .instances
        .iter()
        .find(|record| record.service_id() == service_id && record.instance_id() == instance_id)
}

pub(crate) fn remove_instance(
    registry: &mut ManagedRuntimeInstanceRegistry,
    service_id: &ServiceId,
    instance_id: &InstanceId,
) -> Option<ManagedRuntimeInstanceRecord> {
    let index = registry.instances.iter().position(|record| {
        record.service_id() == service_id && record.instance_id() == instance_id
    })?;
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
        kind: source.kind(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use radroots_runtime_paths::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
    };
    use serde::ser::Error as _;
    use tempfile::tempdir;

    use super::{
        ensure_registry_parent, instance, load_registry, remove_instance, save_registry,
        save_registry_path_with, upsert_instance,
    };
    use crate::{
        ManagedRuntimeInstallState, ManagedRuntimeInstanceRecord, ManagedRuntimeInstanceRegistry,
        RadrootsRuntimeManagerError,
    };

    fn runtime_context(service_id: &str, instance_id: &str) -> RuntimeContext {
        RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(PathBuf::from("/repo/.radroots")),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new(service_id).expect("service"),
            InstanceId::new(instance_id).expect("instance"),
        )
        .expect("context")
    }

    fn sample_record(service_id: &str, instance_id: &str) -> ManagedRuntimeInstanceRecord {
        let context = runtime_context(service_id, instance_id);
        ManagedRuntimeInstanceRecord::new(&context, ManagedRuntimeInstallState::Configured)
    }

    fn assert_error_contains(err: &RadrootsRuntimeManagerError, parts: &[&str]) {
        use std::error::Error as _;

        let rendered = err.to_string();
        for part in parts {
            assert!(
                rendered.contains(part),
                "expected `{rendered}` to contain `{part}`"
            );
        }
        assert!(err.source().is_none());
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
        assert_error_contains(&err, &["read runtime instance registry", "is a directory"]);
    }

    #[test]
    fn load_registry_reports_parse_errors() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("instances.toml");
        fs::write(&path, "credential = 'secret-value'\nnot = [valid")
            .expect("write invalid registry");

        let err = load_registry(&path).expect_err("invalid registry should fail");
        assert_error_contains(&err, &["parse runtime instance registry"]);
        let rendered = format!("{err} {err:?}");
        assert!(!rendered.contains("secret-value"));
        assert!(!rendered.contains(path.to_string_lossy().as_ref()));
    }

    #[test]
    fn save_registry_reports_write_errors() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("registry-dir");
        fs::create_dir(&path).expect("create directory target");

        let err = save_registry(&path, &ManagedRuntimeInstanceRegistry::default())
            .expect_err("directory path should fail");
        assert_error_contains(&err, &["write runtime instance registry", "is a directory"]);
    }

    #[test]
    fn save_registry_reports_parent_creation_errors() {
        let dir = tempdir().expect("tempdir");
        let file_parent = dir.path().join("occupied");
        fs::write(&file_parent, "file").expect("occupied parent");
        let path = file_parent.join("instances.toml");

        let err = save_registry(&path, &ManagedRuntimeInstanceRegistry::default())
            .expect_err("file parent should fail");
        assert_error_contains(&err, &["create runtime instance registry parent"]);
    }

    #[test]
    fn save_registry_reports_serializer_errors() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("instances.toml");

        let err =
            save_registry_path_with(&path, &ManagedRuntimeInstanceRegistry::default(), |_| {
                Err(toml::ser::Error::custom(
                    "forced registry serializer failure",
                ))
            })
            .expect_err("serializer should fail");

        assert_error_contains(&err, &["serialize runtime instance registry"]);
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
        upsert_instance(&mut registry, sample_record("radrootsd", "a"));
        upsert_instance(&mut registry, sample_record("myc", "a"));

        let replacement = ManagedRuntimeInstanceRecord::new(
            &runtime_context("radrootsd", "b"),
            ManagedRuntimeInstallState::Failed,
        );
        upsert_instance(&mut registry, replacement);

        assert_eq!(registry.instances.len(), 3);
        assert_eq!(registry.instances[0].service_id().as_str(), "myc");
        assert_eq!(registry.instances[1].instance_id().as_str(), "a");
        assert_eq!(registry.instances[2].service_id().as_str(), "radrootsd");
        assert_eq!(registry.instances[2].instance_id().as_str(), "b");
        assert_eq!(
            registry.instances[2].install_state(),
            ManagedRuntimeInstallState::Failed
        );
    }

    #[test]
    fn instance_and_remove_instance_handle_missing_and_present_rows() {
        let mut registry = ManagedRuntimeInstanceRegistry::default();
        upsert_instance(&mut registry, sample_record("radrootsd", "local"));

        let myc = ServiceId::new("myc").expect("service");
        let radrootsd = ServiceId::new("radrootsd").expect("service");
        let local = InstanceId::new("local").expect("instance");

        assert!(instance(&registry, &myc, &local).is_none());
        assert!(remove_instance(&mut registry, &myc, &local).is_none());

        let removed = remove_instance(&mut registry, &radrootsd, &local).expect("remove");
        assert_eq!(removed.service_id().as_str(), "radrootsd");
        assert!(registry.instances.is_empty());
    }

    #[test]
    fn registry_round_trip_is_typed_sorted_and_contains_no_service_paths_or_secrets() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("instances.toml");
        let mut registry = ManagedRuntimeInstanceRegistry::default();
        upsert_instance(&mut registry, sample_record("rhi", "secondary"));
        upsert_instance(&mut registry, sample_record("myc", "primary"));

        save_registry(&path, &registry).expect("save registry");
        let raw = fs::read_to_string(&path).expect("read registry");
        for forbidden in [
            "binary_path",
            "config_path",
            "logs_path",
            "run_path",
            "secrets_path",
            "secret_material_ref",
            "/repo/",
            "/etc/",
        ] {
            assert!(!raw.contains(forbidden), "registry leaked `{forbidden}`");
        }

        let loaded = load_registry(&path).expect("load registry");
        assert_eq!(loaded, registry);
        assert_eq!(loaded.instances[0].service_id().as_str(), "myc");
        assert_eq!(loaded.instances[1].service_id().as_str(), "rhi");
    }

    #[test]
    fn registry_rejects_schema_version_unknown_fields_and_duplicate_keys() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("instances.toml");
        let registry = ManagedRuntimeInstanceRegistry {
            instances: vec![sample_record("myc", "primary")],
            ..ManagedRuntimeInstanceRegistry::default()
        };
        save_registry(&path, &registry).expect("save registry");
        let raw = fs::read_to_string(&path).expect("read registry");

        fs::write(
            &path,
            raw.replace("radroots.service-instance-registry", "wrong"),
        )
        .expect("write wrong schema");
        assert!(matches!(
            load_registry(&path),
            Err(RadrootsRuntimeManagerError::UnexpectedRegistrySchema)
        ));

        fs::write(
            &path,
            raw.replace("schema_version = 1", "schema_version = 2"),
        )
        .expect("write wrong version");
        assert!(matches!(
            load_registry(&path),
            Err(RadrootsRuntimeManagerError::UnexpectedRegistryVersion)
        ));

        fs::write(&path, format!("{raw}\nunknown = true\n")).expect("write unknown field");
        assert!(matches!(
            load_registry(&path),
            Err(RadrootsRuntimeManagerError::ParseRegistry)
        ));

        let duplicate = ManagedRuntimeInstanceRegistry {
            instances: vec![
                sample_record("myc", "primary"),
                sample_record("myc", "primary"),
            ],
            ..ManagedRuntimeInstanceRegistry::default()
        };
        assert!(matches!(
            save_registry(&path, &duplicate),
            Err(RadrootsRuntimeManagerError::DuplicateRegistryInstance)
        ));
    }
}
