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
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| {
            RadrootsRuntimeManagerError::CreateRegistryParent {
                path: parent.to_path_buf(),
                source,
            }
        })?;
    }

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
