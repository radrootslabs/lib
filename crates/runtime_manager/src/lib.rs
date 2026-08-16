#![forbid(unsafe_code)]

mod error;
mod managed;
mod model;

pub use error::RadrootsRuntimeManagerError;
pub use managed::{ManagedRuntimeContext, ManagedRuntimeTarget, resolve_runtime_target};
pub use model::{
    BootstrapRuntimeContract, InstanceMetadataContract, LifecycleContract, ManagementDefaults,
    ManagementModeContract, ManagementPathContract, RadrootsRuntimeManagementContract,
    RuntimeGroups,
};

pub const RUNTIME_MANAGEMENT_SCHEMA: &str = "radroots-runtime-management";
pub const RUNTIME_MANAGEMENT_SCHEMA_VERSION: u32 = 1;
pub const RUNTIME_MANAGEMENT_CONTRACT_MAX_UTF8_BYTES: usize = 1_048_576;

pub(crate) const HARDENED_MANAGEMENT_CONTRACT: &str =
    include_str!("../tests/fixtures/hardened_service_management.v1.toml");

pub fn parse_contract_str(
    raw: &str,
) -> Result<RadrootsRuntimeManagementContract, RadrootsRuntimeManagerError> {
    if raw.len() > RUNTIME_MANAGEMENT_CONTRACT_MAX_UTF8_BYTES {
        return Err(RadrootsRuntimeManagerError::ContractTooLarge);
    }
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

    use super::{
        HARDENED_MANAGEMENT_CONTRACT, RUNTIME_MANAGEMENT_CONTRACT_MAX_UTF8_BYTES,
        RUNTIME_MANAGEMENT_SCHEMA, RadrootsRuntimeManagerError, parse_contract_str,
    };

    const CONTRACT: &str = HARDENED_MANAGEMENT_CONTRACT;

    #[test]
    fn contract_parser_accepts_only_the_exact_static_inventory() {
        let contract = parse_contract_str(CONTRACT).expect("contract");
        assert_eq!(contract.schema, RUNTIME_MANAGEMENT_SCHEMA);
        assert_eq!(contract.service_targets.len(), 2);
        assert!(contract.bootstrap.is_empty());

        for raw in [
            CONTRACT.replace("schema_version = 1", "schema_version = 2"),
            CONTRACT.replace(
                "defined = [\"myc\", \"rhi\"]",
                "active = [\"myc\"]\ndefined = [\"rhi\"]",
            ),
            CONTRACT.replace("actions = []", "actions = [\"start\"]"),
            format!("{CONTRACT}\nunknown = true\n"),
        ] {
            assert!(parse_contract_str(&raw).is_err());
        }
        assert!(parse_contract_str("schema = [").is_err());
    }

    #[test]
    fn contract_parser_caps_the_complete_document_before_toml_parsing() {
        let mut exact = CONTRACT.to_owned();
        exact.push('#');
        exact.extend(std::iter::repeat_n(
            'x',
            RUNTIME_MANAGEMENT_CONTRACT_MAX_UTF8_BYTES - exact.len(),
        ));
        assert_eq!(exact.len(), RUNTIME_MANAGEMENT_CONTRACT_MAX_UTF8_BYTES);
        parse_contract_str(&exact).expect("exact maximum contract remains admissible");

        exact.push('x');
        assert_eq!(
            parse_contract_str(&exact),
            Err(RadrootsRuntimeManagerError::ContractTooLarge)
        );

        let very_large = format!("{}#{}", CONTRACT, "x".repeat(4 * 1024 * 1024));
        assert_eq!(
            parse_contract_str(&very_large),
            Err(RadrootsRuntimeManagerError::ContractTooLarge)
        );
    }

    #[test]
    fn contract_errors_are_value_free_and_source_free() {
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
            let error = parse_contract_str(&raw).expect_err("invalid contract");
            let rendered = format!("{error} {error:?}");
            assert!(!rendered.contains(secret));
            assert!(error.source().is_none());
        }
    }

    #[test]
    fn root_surface_exposes_no_legacy_runtime_authority() {
        let source = include_str!("lib.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("production source");
        for forbidden in [
            "mod lifecycle",
            "mod paths",
            "mod registry",
            "ManagedRuntimeArtifactName",
            "ManagedRuntimeInstancePaths",
            "ManagedRuntimeSharedPaths",
            "ManagedRuntimeInstanceRegistry",
            "ManagedRuntimeInstanceRecord",
            "ManagedRuntimeLifecycleAction",
            "load_registry",
            "save_registry",
            "start_process",
            "stop_process",
            "install_binary",
            "extract_binary_archive",
            "remove_instance_artifacts",
            "write_instance_config",
        ] {
            assert!(!source.contains(forbidden), "root retained `{forbidden}`");
        }
    }
}
