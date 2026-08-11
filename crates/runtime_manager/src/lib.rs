#![forbid(unsafe_code)]

pub mod error;
pub mod lifecycle;
pub mod managed;
pub mod model;
pub mod paths;
pub mod registry;

pub use error::RadrootsRuntimeManagerError;
pub use lifecycle::{
    ManagedRuntimeArtifactName, ensure_instance_layout, extract_binary_archive, install_binary,
    process_running, remove_instance_artifacts, start_process, stop_process, write_instance_config,
};
pub use managed::{
    ManagedRuntimeActionInspection, ManagedRuntimeConfigInspection, ManagedRuntimeContext,
    ManagedRuntimeGroup, ManagedRuntimeInspection, ManagedRuntimeInspectionAvailability,
    ManagedRuntimeLifecycleAction, ManagedRuntimeLogsInspection, ManagedRuntimeStatusInspection,
    ManagedRuntimeTarget, active_management_mode_for_profile, inspect_runtime_action,
    inspect_runtime_config, inspect_runtime_logs, inspect_runtime_status, load_management_context,
    resolve_runtime_target, runtime_group,
};
pub use model::{
    BootstrapRuntimeContract, LifecycleContract, ManagedRuntimeHealthState,
    ManagedRuntimeInstallState, ManagedRuntimeInstanceRecord, ManagedRuntimeInstanceRegistry,
    ManagementDefaults, ManagementModeContract, ManagementPathContract,
    RUNTIME_INSTANCE_REGISTRY_SCHEMA, RUNTIME_INSTANCE_REGISTRY_VERSION,
    RadrootsRuntimeManagementContract, RuntimeGroups,
};
pub use paths::{ManagedRuntimeInstancePaths, ManagedRuntimeSharedPaths, bootstrap_runtime};
pub use registry::{instance, load_registry, save_registry};

pub const RUNTIME_MANAGEMENT_SCHEMA: &str = "radroots-runtime-management";

pub fn parse_contract_str(
    raw: &str,
) -> Result<RadrootsRuntimeManagementContract, RadrootsRuntimeManagerError> {
    let contract = toml::from_str::<RadrootsRuntimeManagementContract>(raw)
        .map_err(|_| RadrootsRuntimeManagerError::Parse)?;
    if contract.schema != RUNTIME_MANAGEMENT_SCHEMA {
        return Err(RadrootsRuntimeManagerError::UnexpectedSchema);
    }
    Ok(contract)
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::{RUNTIME_MANAGEMENT_SCHEMA, parse_contract_str};

    const CONTRACT: &str = r#"
schema = "radroots-runtime-management"
schema_version = 1
owner_doc = "owner"
runtime_registry = "registry"
distribution_contract = "distribution"
capabilities_contract = "capabilities"

[defaults]
instance_cardinality = "multiple"
managed_runtime_lookup = "typed_instance_registry"
explicit_runtime_endpoint_overrides_precede_managed_instance_binding = true
global_path_mutation_forbidden = true

[management_clients]
active = ["cli"]

[managed_runtime_targets]
defined = ["myc", "rhi"]

[lifecycle]
actions = ["status"]
health_states = ["running"]

[mode.interactive]
contract_state = "active"
platforms = ["linux"]
supported_profiles = ["repo_local"]
service_manager_integration = false
uses_absolute_binary_paths = true
default_instance_cardinality = "multiple"

[paths.interactive]
shared_namespace = "obsolete"
instance_registry_root_class = "config"
instance_registry_rel = "obsolete"
artifact_cache_root_class = "cache"
artifact_cache_rel = "obsolete"
install_root_class = "data"
install_root_rel = "obsolete"
state_root_class = "data"
state_root_rel = "obsolete"
logs_root_class = "logs"
logs_root_rel = "obsolete"
run_root_class = "run"
run_root_rel = "obsolete"
secrets_root_class = "secrets"
secrets_namespace_rel = "obsolete"

[instance_metadata]
required_fields = ["service_id", "instance_id"]

[bootstrap]
"#;

    #[test]
    fn contract_parser_accepts_only_the_expected_schema() {
        let contract = parse_contract_str(CONTRACT).expect("contract");
        assert_eq!(contract.schema, RUNTIME_MANAGEMENT_SCHEMA);

        let wrong = CONTRACT.replace(
            "schema = \"radroots-runtime-management\"",
            "schema = \"wrong\"",
        );
        assert!(parse_contract_str(&wrong).is_err());
        assert!(parse_contract_str("schema = [").is_err());
    }

    #[test]
    fn contract_errors_redact_raw_schema_values_and_parser_causes() {
        for (raw, secret) in [
            (
                CONTRACT.replace(
                    "schema = \"radroots-runtime-management\"",
                    "schema = \"/sensitive/root/secret-schema\"",
                ),
                "/sensitive/root/secret-schema",
            ),
            (
                "credential = 'secret-value'\ninvalid = [".to_owned(),
                "secret-value",
            ),
        ] {
            let err = parse_contract_str(&raw).expect_err("invalid contract");
            let rendered = format!("{err} {err:?}");
            assert!(!rendered.contains(secret));
            assert!(err.source().is_none());
        }
    }

    #[test]
    fn manager_source_no_longer_consumes_legacy_path_selection_or_raw_identity_joins() {
        let sources = [
            include_str!("error.rs"),
            include_str!("lifecycle.rs"),
            include_str!("managed.rs"),
            include_str!("model.rs"),
            include_str!("paths.rs"),
            include_str!("registry.rs"),
        ];
        for forbidden in [
            "RadrootsRuntimePathSelection",
            "load_management_context_with_selection",
            "PathBuf::from(runtime_id).join(instance_id)",
            "record.config_path",
            "record.logs_path",
            "record.run_path",
            "workers/rhi",
            "pub fn read_secret_file",
            "pub fn write_secret_file",
            "pub fn write_managed_file",
            "pub fn registry_mut",
            "pub fn upsert_instance",
            "pub fn resolve_shared_paths",
            "pub fn resolve_instance_paths",
        ] {
            assert!(
                sources.iter().all(|source| !source.contains(forbidden)),
                "runtime manager retained forbidden source `{forbidden}`"
            );
        }
    }
}
