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
pub const RUNTIME_MANAGEMENT_SCHEMA_VERSION: u32 = 1;

pub(crate) const HARDENED_MANAGEMENT_CONTRACT: &str =
    include_str!("../tests/fixtures/hardened_service_management.v1.toml");

pub fn parse_contract_str(
    raw: &str,
) -> Result<RadrootsRuntimeManagementContract, RadrootsRuntimeManagerError> {
    let contract = toml::from_str::<RadrootsRuntimeManagementContract>(raw)
        .map_err(|_| RadrootsRuntimeManagerError::Parse)?;
    if contract.schema != RUNTIME_MANAGEMENT_SCHEMA {
        return Err(RadrootsRuntimeManagerError::UnexpectedSchema);
    }
    if contract.schema_version != RUNTIME_MANAGEMENT_SCHEMA_VERSION {
        return Err(RadrootsRuntimeManagerError::UnexpectedSchemaVersion);
    }
    validate_hardened_management_contract(&contract)?;
    Ok(contract)
}

pub(crate) fn validate_hardened_management_contract(
    contract: &RadrootsRuntimeManagementContract,
) -> Result<(), RadrootsRuntimeManagerError> {
    let expected =
        toml::from_str::<RadrootsRuntimeManagementContract>(HARDENED_MANAGEMENT_CONTRACT)
            .map_err(|_| RadrootsRuntimeManagerError::InvalidContract)?;
    if contract != &expected {
        return Err(RadrootsRuntimeManagerError::InvalidContract);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::{HARDENED_MANAGEMENT_CONTRACT, RUNTIME_MANAGEMENT_SCHEMA, parse_contract_str};

    const CONTRACT: &str = HARDENED_MANAGEMENT_CONTRACT;

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
