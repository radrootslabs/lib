#![forbid(unsafe_code)]

pub mod error;
pub mod lifecycle;
pub mod managed;
pub mod model;
pub mod paths;
pub mod registry;

pub use error::RadrootsRuntimeManagerError;
pub use lifecycle::{
    ensure_instance_layout, extract_binary_archive, install_binary, process_running,
    read_secret_file, remove_instance_artifacts, start_process, stop_process,
    write_instance_metadata, write_managed_file, write_secret_file,
};
pub use managed::{
    ManagedRuntimeActionInspection, ManagedRuntimeConfigInspection, ManagedRuntimeContext,
    ManagedRuntimeGroup, ManagedRuntimeInspection, ManagedRuntimeInspectionAvailability,
    ManagedRuntimeLifecycleAction, ManagedRuntimeLogsInspection, ManagedRuntimeStatusInspection,
    ManagedRuntimeTarget, active_management_mode_for_profile, inspect_runtime_action,
    inspect_runtime_config, inspect_runtime_logs, inspect_runtime_status, load_management_context,
    load_management_context_with_selection, resolve_runtime_target, runtime_group,
};
pub use model::{
    BootstrapRuntimeContract, LifecycleContract, ManagedRuntimeHealthState,
    ManagedRuntimeInstallState, ManagedRuntimeInstanceRecord, ManagedRuntimeInstanceRegistry,
    ManagementDefaults, ManagementModeContract, ManagementPathContract,
    RadrootsRuntimeManagementContract, RuntimeGroups,
};
pub use paths::{
    ManagedRuntimeInstancePaths, ManagedRuntimeSharedPaths, bootstrap_runtime,
    resolve_instance_paths, resolve_shared_paths,
};
pub use registry::{instance, load_registry, remove_instance, save_registry, upsert_instance};

pub const RUNTIME_MANAGEMENT_SCHEMA: &str = "radroots-runtime-management";

pub fn parse_contract_str(
    raw: &str,
) -> Result<RadrootsRuntimeManagementContract, RadrootsRuntimeManagerError> {
    let contract = toml::from_str::<RadrootsRuntimeManagementContract>(raw)
        .map_err(|err| RadrootsRuntimeManagerError::Parse(err.to_string()))?;
    if contract.schema != RUNTIME_MANAGEMENT_SCHEMA {
        return Err(RadrootsRuntimeManagerError::UnexpectedSchema {
            expected: RUNTIME_MANAGEMENT_SCHEMA,
            found: contract.schema.clone(),
        });
    }
    Ok(contract)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use radroots_runtime_paths::{
        RadrootsHostEnvironment, RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform,
    };
    use tempfile::tempdir;

    use crate::{
        ManagedRuntimeHealthState, ManagedRuntimeInstallState, ManagedRuntimeInstanceRecord,
        bootstrap_runtime, instance, load_registry, parse_contract_str, resolve_instance_paths,
        resolve_shared_paths, save_registry, upsert_instance,
    };

    fn assert_error_contains(err: &crate::RadrootsRuntimeManagerError, parts: &[&str]) {
        let rendered = err.to_string();
        for part in parts {
            assert!(
                rendered.contains(part),
                "expected `{rendered}` to contain `{part}`"
            );
        }
    }

    const CONTRACT: &str = r#"
schema = "radroots-runtime-management"
schema_version = 1
owner_doc = "docs/execution/rcl/radroots-modular-runtime-management-bootstrap-rcl.md"
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
actions = ["install", "uninstall", "start", "stop", "restart", "status", "logs", "config_show", "config_set"]
destructive_actions = ["uninstall"]
health_states = ["not_installed", "stopped", "starting", "running", "degraded", "failed"]

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
required_fields = [
  "runtime_id",
  "instance_id",
  "management_mode",
  "install_state",
  "binary_path",
  "config_path",
  "logs_path",
  "run_path",
  "installed_version",
]
optional_fields = [
  "health_endpoint",
  "secret_material_ref",
  "last_started_at",
  "last_stopped_at",
  "notes",
]

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

    #[test]
    fn parse_contract_accepts_expected_schema() {
        let contract = parse_contract_str(CONTRACT).expect("parse contract");
        assert_eq!(contract.schema, crate::RUNTIME_MANAGEMENT_SCHEMA);
        assert!(contract.mode.contains_key("interactive_user_managed"));
    }

    #[test]
    fn parse_contract_reports_invalid_toml() {
        let err = parse_contract_str("schema = [").expect_err("invalid toml should fail");
        assert_error_contains(&err, &["parse runtime management contract"]);
    }

    #[test]
    fn parse_contract_rejects_unexpected_schema() {
        let err = parse_contract_str(&CONTRACT.replace(
            "schema = \"radroots-runtime-management\"",
            "schema = \"wrong-schema\"",
        ))
        .expect_err("unexpected schema should fail");
        assert_error_contains(&err, &["wrong-schema", crate::RUNTIME_MANAGEMENT_SCHEMA]);
    }

    #[test]
    fn resolve_shared_paths_uses_interactive_user_roots() {
        let contract = parse_contract_str(CONTRACT).expect("parse contract");
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                ..RadrootsHostEnvironment::default()
            },
        );

        let paths = resolve_shared_paths(
            &contract,
            &resolver,
            RadrootsPathProfile::InteractiveUser,
            &RadrootsPathOverrides::default(),
            "interactive_user_managed",
        )
        .expect("resolve shared manager paths");

        assert_eq!(
            paths.instance_registry_path,
            PathBuf::from("/home/treesap/.radroots/config/shared/runtime-manager/instances.toml")
        );
        assert_eq!(
            paths.install_root,
            PathBuf::from("/home/treesap/.radroots/data/shared/runtime-manager/installs")
        );
        assert_eq!(
            paths.artifact_cache_dir,
            PathBuf::from("/home/treesap/.radroots/cache/shared/runtime-manager/artifacts")
        );
        assert_eq!(
            paths.state_root,
            PathBuf::from("/home/treesap/.radroots/data/shared/runtime-manager/state")
        );
        assert_eq!(
            paths.logs_root,
            PathBuf::from("/home/treesap/.radroots/logs/shared/runtime-manager")
        );
        assert_eq!(
            paths.run_root,
            PathBuf::from("/home/treesap/.radroots/run/shared/runtime-manager")
        );
        assert_eq!(
            paths.secrets_root,
            PathBuf::from("/home/treesap/.radroots/secrets/shared/runtime-manager")
        );
    }

    #[test]
    fn resolve_repo_local_paths_uses_explicit_base_root() {
        let contract = parse_contract_str(CONTRACT).expect("parse contract");
        let resolver =
            RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default());

        let paths = resolve_shared_paths(
            &contract,
            &resolver,
            RadrootsPathProfile::RepoLocal,
            &RadrootsPathOverrides::repo_local("/repo/.local/radroots"),
            "interactive_user_managed",
        )
        .expect("resolve repo local manager paths");

        assert_eq!(
            paths.state_root,
            PathBuf::from("/repo/.local/radroots/data/shared/runtime-manager/state")
        );
    }

    #[test]
    fn resolve_instance_paths_builds_per_runtime_layout() {
        let contract = parse_contract_str(CONTRACT).expect("parse contract");
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Macos,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/Users/treesap")),
                ..RadrootsHostEnvironment::default()
            },
        );
        let shared = resolve_shared_paths(
            &contract,
            &resolver,
            RadrootsPathProfile::InteractiveUser,
            &RadrootsPathOverrides::default(),
            "interactive_user_managed",
        )
        .expect("resolve shared manager paths");

        let instance_paths = resolve_instance_paths(&shared, "radrootsd", "local");
        assert_eq!(
            instance_paths.install_dir,
            PathBuf::from(
                "/Users/treesap/.radroots/data/shared/runtime-manager/installs/radrootsd/local"
            )
        );
        assert_eq!(
            instance_paths.pid_file_path,
            PathBuf::from(
                "/Users/treesap/.radroots/run/shared/runtime-manager/radrootsd/local/runtime.pid"
            )
        );
        assert_eq!(
            instance_paths.metadata_path,
            PathBuf::from(
                "/Users/treesap/.radroots/data/shared/runtime-manager/state/radrootsd/local/instance.toml"
            )
        );
    }

    #[test]
    fn registry_round_trip_persists_and_reloads_instances() {
        let dir = tempdir().expect("tempdir");
        let registry_path = dir.path().join("instances.toml");
        let mut registry = crate::ManagedRuntimeInstanceRegistry::default();
        upsert_instance(
            &mut registry,
            ManagedRuntimeInstanceRecord {
                runtime_id: "radrootsd".to_string(),
                instance_id: "local".to_string(),
                management_mode: "interactive_user_managed".to_string(),
                install_state: ManagedRuntimeInstallState::Configured,
                binary_path: PathBuf::from("/tmp/radrootsd"),
                config_path: PathBuf::from("/tmp/config.toml"),
                logs_path: PathBuf::from("/tmp/logs"),
                run_path: PathBuf::from("/tmp/run"),
                installed_version: "0.1.0-alpha.2".to_string(),
                health_endpoint: Some("jsonrpc_status".to_string()),
                secret_material_ref: Some(
                    "shared/runtime-manager/radrootsd/local/token".to_string(),
                ),
                last_started_at: Some("2026-04-08T00:00:00Z".to_string()),
                last_stopped_at: None,
                notes: Some("test".to_string()),
            },
        );

        save_registry(&registry_path, &registry).expect("save registry");
        let reloaded = load_registry(&registry_path).expect("load registry");
        let record = instance(&reloaded, "radrootsd", "local").expect("instance record");
        assert_eq!(record.install_state, ManagedRuntimeInstallState::Configured);
        assert_eq!(record.health_endpoint.as_deref(), Some("jsonrpc_status"));
    }

    #[test]
    fn bootstrap_lookup_returns_radrootsd_contract() {
        let contract = parse_contract_str(CONTRACT).expect("parse contract");
        let bootstrap = bootstrap_runtime(&contract, "radrootsd").expect("bootstrap contract");
        assert_eq!(bootstrap.default_instance_id, "local");
        assert_eq!(bootstrap.health_surface, "jsonrpc_status");
        assert!(bootstrap.preferred_cli_binding);
    }

    #[test]
    fn install_and_health_state_surface_is_typed() {
        assert_eq!(
            ManagedRuntimeInstallState::Installed,
            ManagedRuntimeInstallState::Installed
        );
        assert_eq!(
            ManagedRuntimeHealthState::Degraded,
            ManagedRuntimeHealthState::Degraded
        );
    }
}
