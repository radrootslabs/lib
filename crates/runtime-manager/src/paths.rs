use std::path::PathBuf;

use radroots_runtime_paths::{
    RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver, RadrootsPaths,
};

use crate::error::RadrootsRuntimeManagerError;
use crate::model::RadrootsRuntimeManagementContract;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRuntimeSharedPaths {
    pub instance_registry_path: PathBuf,
    pub artifact_cache_dir: PathBuf,
    pub install_root: PathBuf,
    pub state_root: PathBuf,
    pub logs_root: PathBuf,
    pub run_root: PathBuf,
    pub secrets_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRuntimeInstancePaths {
    pub install_dir: PathBuf,
    pub state_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub run_dir: PathBuf,
    pub secrets_dir: PathBuf,
    pub pid_file_path: PathBuf,
    pub stdout_log_path: PathBuf,
    pub stderr_log_path: PathBuf,
    pub metadata_path: PathBuf,
}

pub fn resolve_shared_paths(
    contract: &RadrootsRuntimeManagementContract,
    resolver: &RadrootsPathResolver,
    profile: RadrootsPathProfile,
    overrides: &RadrootsPathOverrides,
    mode_id: &str,
) -> Result<ManagedRuntimeSharedPaths, RadrootsRuntimeManagerError> {
    ensure_profile_supported(contract, mode_id, profile)?;
    let roots = resolver.resolve(profile, overrides)?;
    let path_spec = contract
        .paths
        .get(mode_id)
        .ok_or_else(|| RadrootsRuntimeManagerError::MissingPathSpec(mode_id.to_string()))?;

    Ok(ManagedRuntimeSharedPaths {
        instance_registry_path: root_class_path(
            &roots,
            &path_spec.instance_registry_root_class,
            &path_spec.instance_registry_rel,
        )?,
        artifact_cache_dir: root_class_path(
            &roots,
            &path_spec.artifact_cache_root_class,
            &path_spec.artifact_cache_rel,
        )?,
        install_root: root_class_path(
            &roots,
            &path_spec.install_root_class,
            &path_spec.install_root_rel,
        )?,
        state_root: root_class_path(
            &roots,
            &path_spec.state_root_class,
            &path_spec.state_root_rel,
        )?,
        logs_root: root_class_path(&roots, &path_spec.logs_root_class, &path_spec.logs_root_rel)?,
        run_root: root_class_path(&roots, &path_spec.run_root_class, &path_spec.run_root_rel)?,
        secrets_root: root_class_path(
            &roots,
            &path_spec.secrets_root_class,
            &path_spec.secrets_namespace_rel,
        )?,
    })
}

pub fn resolve_instance_paths(
    shared: &ManagedRuntimeSharedPaths,
    runtime_id: &str,
    instance_id: &str,
) -> ManagedRuntimeInstancePaths {
    let suffix = PathBuf::from(runtime_id).join(instance_id);
    let install_dir = shared.install_root.join(&suffix);
    let state_dir = shared.state_root.join(&suffix);
    let logs_dir = shared.logs_root.join(&suffix);
    let run_dir = shared.run_root.join(&suffix);
    let secrets_dir = shared.secrets_root.join(&suffix);

    ManagedRuntimeInstancePaths {
        install_dir,
        state_dir: state_dir.clone(),
        logs_dir: logs_dir.clone(),
        run_dir: run_dir.clone(),
        secrets_dir,
        pid_file_path: run_dir.join("runtime.pid"),
        stdout_log_path: logs_dir.join("stdout.log"),
        stderr_log_path: logs_dir.join("stderr.log"),
        metadata_path: state_dir.join("instance.toml"),
    }
}

pub fn bootstrap_runtime<'a>(
    contract: &'a RadrootsRuntimeManagementContract,
    runtime_id: &str,
) -> Result<&'a crate::model::BootstrapRuntimeContract, RadrootsRuntimeManagerError> {
    contract
        .bootstrap
        .get(runtime_id)
        .ok_or_else(|| RadrootsRuntimeManagerError::UnknownBootstrapRuntime(runtime_id.to_string()))
}

fn ensure_profile_supported(
    contract: &RadrootsRuntimeManagementContract,
    mode_id: &str,
    profile: RadrootsPathProfile,
) -> Result<(), RadrootsRuntimeManagerError> {
    let mode = contract
        .mode
        .get(mode_id)
        .ok_or_else(|| RadrootsRuntimeManagerError::UnknownManagementMode(mode_id.to_string()))?;
    let profile_id = profile.to_string();
    if mode
        .supported_profiles
        .iter()
        .any(|entry| entry == &profile_id)
    {
        Ok(())
    } else {
        Err(RadrootsRuntimeManagerError::UnsupportedProfile {
            mode_id: mode_id.to_string(),
            profile: profile_id,
        })
    }
}

fn root_class_path(
    roots: &RadrootsPaths,
    root_class: &str,
    rel: &str,
) -> Result<PathBuf, RadrootsRuntimeManagerError> {
    let base = match root_class {
        "config" => &roots.config,
        "data" => &roots.data,
        "cache" => &roots.cache,
        "logs" => &roots.logs,
        "run" => &roots.run,
        "secrets" => &roots.secrets,
        other => {
            return Err(RadrootsRuntimeManagerError::UnknownRootClass(
                other.to_string(),
            ));
        }
    };
    Ok(base.join(rel))
}
