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
    let root = |root_class: &str, rel: &str| root_class_path(&roots, root_class, rel);

    let instance_registry_path = root(
        &path_spec.instance_registry_root_class,
        &path_spec.instance_registry_rel,
    )?;
    let artifact_cache_dir = root(
        &path_spec.artifact_cache_root_class,
        &path_spec.artifact_cache_rel,
    )?;
    let install_root = root(&path_spec.install_root_class, &path_spec.install_root_rel)?;
    let state_root = root(&path_spec.state_root_class, &path_spec.state_root_rel)?;
    let logs_root = root(&path_spec.logs_root_class, &path_spec.logs_root_rel)?;
    let run_root = root(&path_spec.run_root_class, &path_spec.run_root_rel)?;
    let secrets_root = root(
        &path_spec.secrets_root_class,
        &path_spec.secrets_namespace_rel,
    )?;

    Ok(ManagedRuntimeSharedPaths {
        instance_registry_path,
        artifact_cache_dir,
        install_root,
        state_root,
        logs_root,
        run_root,
        secrets_root,
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use radroots_runtime_paths::{
        RadrootsHostEnvironment, RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPaths, RadrootsPlatform,
    };

    use super::{bootstrap_runtime, resolve_shared_paths, root_class_path};
    use crate::{
        RadrootsRuntimeManagerError, model::RadrootsRuntimeManagementContract, parse_contract_str,
    };

    const CONTRACT: &str = r#"
schema = "radroots-runtime-management"
schema_version = 1
owner_doc = "docs/migration/radroots-modular-runtime-management-bootstrap-rcl.md"
runtime_registry = "registry.toml"
distribution_contract = "distribution.toml"
capabilities_contract = "capabilities.toml"

[defaults]
instance_cardinality = "single_default_instance"
managed_runtime_lookup = "shared_instance_registry"
explicit_runtime_endpoint_overrides_precede_managed_instance_binding = true
global_path_mutation_forbidden = true

[management_clients]
active = ["cli"]
defined = ["community-app-desktop"]

[managed_runtime_targets]
active = ["radrootsd"]
defined = ["myc", "rhi"]
bootstrap_only = ["hyf"]

[lifecycle]
actions = ["install", "uninstall", "start"]
destructive_actions = ["uninstall"]
health_states = ["not_installed", "running"]

[mode.interactive_user_managed]
contract_state = "active"
platforms = ["linux", "macos", "windows"]
supported_profiles = ["interactive_user", "repo_local"]
service_manager_integration = false
uses_absolute_binary_paths = true
requires_explicit_pid_tracking = true
requires_explicit_log_tracking = true
default_instance_cardinality = "single_default_instance"

[mode.service_host_managed]
contract_state = "defined"
platforms = ["linux", "macos", "windows"]
supported_profiles = ["service_host"]
service_manager_integration = true
uses_absolute_binary_paths = true
default_instance_cardinality = "single_default_instance"

[paths.interactive_user_managed]
shared_namespace = "shared/runtime-manager"
instance_registry_root_class = "config"
instance_registry_rel = "shared/runtime-manager/instances.toml"
artifact_cache_root_class = "cache"
artifact_cache_rel = "shared/runtime-manager/artifacts"
install_root_class = "data"
install_root_rel = "shared/runtime-manager/installs"
state_root_class = "data"
state_root_rel = "shared/runtime-manager/state"
logs_root_class = "logs"
logs_root_rel = "shared/runtime-manager"
run_root_class = "run"
run_root_rel = "shared/runtime-manager"
secrets_root_class = "secrets"
secrets_namespace_rel = "shared/runtime-manager"

[instance_metadata]
required_fields = ["runtime_id"]
optional_fields = ["notes"]

[bootstrap.radrootsd]
runtime_id = "radrootsd"
management_mode = "interactive_user_managed"
default_instance_id = "local"
install_strategy = "archive_unpack"
config_format = "toml"
requires_bootstrap_secret = true
requires_config_bootstrap = true
requires_signer_provider = false
health_surface = "jsonrpc_status"
preferred_cli_binding = true
"#;

    fn contract() -> RadrootsRuntimeManagementContract {
        parse_contract_str(CONTRACT).expect("parse contract")
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

    fn linux_resolver() -> RadrootsPathResolver {
        RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                ..RadrootsHostEnvironment::default()
            },
        )
    }

    #[test]
    fn bootstrap_lookup_reports_unknown_runtime() {
        let err = bootstrap_runtime(&contract(), "missing-runtime").expect_err("missing runtime");
        assert_error_contains(&err, &["missing-runtime", "no bootstrap entry"]);
    }

    #[test]
    fn resolve_shared_paths_reports_unknown_management_mode() {
        let err = resolve_shared_paths(
            &contract(),
            &linux_resolver(),
            RadrootsPathProfile::InteractiveUser,
            &RadrootsPathOverrides::default(),
            "missing-mode",
        )
        .expect_err("missing mode should fail");
        assert_error_contains(&err, &["management mode `missing-mode`"]);
    }

    #[test]
    fn resolve_shared_paths_reports_unsupported_profile() {
        let err = resolve_shared_paths(
            &contract(),
            &linux_resolver(),
            RadrootsPathProfile::ServiceHost,
            &RadrootsPathOverrides::default(),
            "interactive_user_managed",
        )
        .expect_err("service_host should be unsupported for interactive mode");
        assert_error_contains(&err, &["interactive_user_managed", "service_host"]);
    }

    #[test]
    fn resolve_shared_paths_reports_missing_path_spec() {
        let mut contract = contract();
        contract.paths.remove("interactive_user_managed");

        let err = resolve_shared_paths(
            &contract,
            &linux_resolver(),
            RadrootsPathProfile::InteractiveUser,
            &RadrootsPathOverrides::default(),
            "interactive_user_managed",
        )
        .expect_err("missing path spec should fail");
        assert_error_contains(
            &err,
            &["interactive_user_managed", "no shared path specification"],
        );
    }

    #[test]
    fn resolve_shared_paths_reports_unknown_root_class() {
        let mut contract = contract();
        contract
            .paths
            .get_mut("interactive_user_managed")
            .expect("path spec")
            .instance_registry_root_class = "bogus".to_string();

        let err = resolve_shared_paths(
            &contract,
            &linux_resolver(),
            RadrootsPathProfile::InteractiveUser,
            &RadrootsPathOverrides::default(),
            "interactive_user_managed",
        )
        .expect_err("unknown root class should fail");
        assert_error_contains(&err, &["unknown root class `bogus`"]);
    }

    #[test]
    fn root_class_path_maps_all_known_classes() {
        let roots = RadrootsPaths {
            config: PathBuf::from("/roots/config"),
            data: PathBuf::from("/roots/data"),
            cache: PathBuf::from("/roots/cache"),
            logs: PathBuf::from("/roots/logs"),
            run: PathBuf::from("/roots/run"),
            secrets: PathBuf::from("/roots/secrets"),
        };

        assert_eq!(
            root_class_path(&roots, "config", "a/b").expect("config root"),
            PathBuf::from("/roots/config/a/b")
        );
        assert_eq!(
            root_class_path(&roots, "data", "a/b").expect("data root"),
            PathBuf::from("/roots/data/a/b")
        );
        assert_eq!(
            root_class_path(&roots, "cache", "a/b").expect("cache root"),
            PathBuf::from("/roots/cache/a/b")
        );
        assert_eq!(
            root_class_path(&roots, "logs", "a/b").expect("logs root"),
            PathBuf::from("/roots/logs/a/b")
        );
        assert_eq!(
            root_class_path(&roots, "run", "a/b").expect("run root"),
            PathBuf::from("/roots/run/a/b")
        );
        assert_eq!(
            root_class_path(&roots, "secrets", "a/b").expect("secrets root"),
            PathBuf::from("/roots/secrets/a/b")
        );
    }
}
