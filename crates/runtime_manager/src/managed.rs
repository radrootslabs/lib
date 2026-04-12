use std::path::PathBuf;

use radroots_runtime_paths::{RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver};

use crate::{
    load_registry, resolve_instance_paths, resolve_shared_paths, BootstrapRuntimeContract,
    ManagedRuntimeInstancePaths, ManagedRuntimeInstanceRecord, ManagedRuntimeInstanceRegistry,
    ManagementModeContract, RadrootsRuntimeManagementContract, RadrootsRuntimeManagerError,
};

#[derive(Debug, Clone)]
pub struct ManagedRuntimeContext {
    pub contract: RadrootsRuntimeManagementContract,
    pub shared_paths: crate::ManagedRuntimeSharedPaths,
    pub registry: ManagedRuntimeInstanceRegistry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedRuntimeGroup {
    ActiveManagedTarget,
    DefinedManagedTarget,
    BootstrapOnly,
    Unknown,
}

impl ManagedRuntimeGroup {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ActiveManagedTarget => "active_managed_target",
            Self::DefinedManagedTarget => "defined_managed_target",
            Self::BootstrapOnly => "bootstrap_only",
            Self::Unknown => "unknown",
        }
    }

    pub fn posture(self) -> &'static str {
        match self {
            Self::ActiveManagedTarget => "active_managed_target",
            Self::DefinedManagedTarget => "defined_future_target",
            Self::BootstrapOnly => "bootstrap_only_direct_binding",
            Self::Unknown => "unknown_runtime",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ManagedRuntimeTarget {
    pub runtime_id: String,
    pub instance_id: String,
    pub instance_source: String,
    pub runtime_group: ManagedRuntimeGroup,
    pub management_mode: Option<String>,
    pub mode_contract: Option<ManagementModeContract>,
    pub bootstrap: Option<BootstrapRuntimeContract>,
    pub instance_record: Option<ManagedRuntimeInstanceRecord>,
    pub predicted_paths: Option<ManagedRuntimeInstancePaths>,
    pub registry_path: PathBuf,
}

pub fn load_management_context(
    contract: RadrootsRuntimeManagementContract,
    resolver: &RadrootsPathResolver,
    profile: RadrootsPathProfile,
    overrides: &RadrootsPathOverrides,
) -> Result<ManagedRuntimeContext, RadrootsRuntimeManagerError> {
    let mode_id = active_management_mode_for_profile(&contract, profile)?;
    let shared_paths = resolve_shared_paths(&contract, resolver, profile, overrides, mode_id)?;
    let registry = load_registry(&shared_paths.instance_registry_path)?;
    Ok(ManagedRuntimeContext {
        contract,
        shared_paths,
        registry,
    })
}

pub fn active_management_mode_for_profile<'a>(
    contract: &'a RadrootsRuntimeManagementContract,
    profile: RadrootsPathProfile,
) -> Result<&'a str, RadrootsRuntimeManagerError> {
    let profile_id = profile.to_string();
    contract
        .mode
        .iter()
        .find(|(_, mode)| {
            mode.contract_state == "active"
                && mode
                    .supported_profiles
                    .iter()
                    .any(|entry| entry == &profile_id)
        })
        .map(|(mode_id, _)| mode_id.as_str())
        .ok_or_else(|| RadrootsRuntimeManagerError::UnsupportedProfile {
            mode_id: "active".to_owned(),
            profile: profile_id,
        })
}

pub fn resolve_runtime_target(
    context: &ManagedRuntimeContext,
    runtime_id: &str,
    requested_instance_id: Option<&str>,
) -> ManagedRuntimeTarget {
    let runtime_group = runtime_group(&context.contract, runtime_id);
    let bootstrap = context.contract.bootstrap.get(runtime_id).cloned();
    let instance_id = requested_instance_id
        .map(ToOwned::to_owned)
        .or_else(|| {
            bootstrap
                .as_ref()
                .map(|entry| entry.default_instance_id.clone())
        })
        .unwrap_or_else(|| "default".to_owned());
    let instance_source = if requested_instance_id.is_some() {
        "command_arg".to_owned()
    } else if bootstrap.is_some() {
        "bootstrap_default".to_owned()
    } else {
        "implicit_default".to_owned()
    };
    let management_mode = bootstrap
        .as_ref()
        .map(|entry| entry.management_mode.clone());
    let mode_contract = management_mode
        .as_ref()
        .and_then(|mode_id| context.contract.mode.get(mode_id).cloned());
    let instance_record = context
        .registry
        .instances
        .iter()
        .find(|record| record.runtime_id == runtime_id && record.instance_id == instance_id)
        .cloned();
    let predicted_paths = if runtime_group == ManagedRuntimeGroup::ActiveManagedTarget {
        Some(resolve_instance_paths(
            &context.shared_paths,
            runtime_id,
            instance_id.as_str(),
        ))
    } else {
        None
    };

    ManagedRuntimeTarget {
        runtime_id: runtime_id.to_owned(),
        instance_id,
        instance_source,
        runtime_group,
        management_mode,
        mode_contract,
        bootstrap,
        instance_record,
        predicted_paths,
        registry_path: context.shared_paths.instance_registry_path.clone(),
    }
}

pub fn runtime_group(
    contract: &RadrootsRuntimeManagementContract,
    runtime_id: &str,
) -> ManagedRuntimeGroup {
    if contract
        .managed_runtime_targets
        .active
        .iter()
        .any(|entry| entry == runtime_id)
    {
        ManagedRuntimeGroup::ActiveManagedTarget
    } else if contract
        .managed_runtime_targets
        .defined
        .iter()
        .any(|entry| entry == runtime_id)
    {
        ManagedRuntimeGroup::DefinedManagedTarget
    } else if contract
        .managed_runtime_targets
        .bootstrap_only
        .iter()
        .any(|entry| entry == runtime_id)
    {
        ManagedRuntimeGroup::BootstrapOnly
    } else {
        ManagedRuntimeGroup::Unknown
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use radroots_runtime_paths::{
        RadrootsHostEnvironment, RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform,
    };

    use super::{
        active_management_mode_for_profile, load_management_context, resolve_runtime_target,
        ManagedRuntimeGroup,
    };
    use crate::{parse_contract_str, ManagedRuntimeInstallState, ManagedRuntimeInstanceRecord};

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
defined = []

[managed_runtime_targets]
active = ["radrootsd"]
defined = ["myc"]
bootstrap_only = ["hyf"]

[lifecycle]
actions = ["install", "start"]
destructive_actions = []
health_states = ["not_installed", "running"]

[mode.interactive_user_managed]
contract_state = "active"
platforms = ["linux"]
supported_profiles = ["interactive_user", "repo_local"]
service_manager_integration = false
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

    #[test]
    fn active_management_mode_matches_supported_profile() {
        let contract = parse_contract_str(CONTRACT).expect("contract");
        let mode_id =
            active_management_mode_for_profile(&contract, RadrootsPathProfile::InteractiveUser)
                .expect("mode");
        assert_eq!(mode_id, "interactive_user_managed");
    }

    #[test]
    fn resolve_runtime_target_uses_bootstrap_default_instance_id() {
        let contract = parse_contract_str(CONTRACT).expect("contract");
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                ..RadrootsHostEnvironment::default()
            },
        );
        let mut context = load_management_context(
            contract,
            &resolver,
            RadrootsPathProfile::InteractiveUser,
            &RadrootsPathOverrides::default(),
        )
        .expect("context");
        context
            .registry
            .instances
            .push(ManagedRuntimeInstanceRecord {
                runtime_id: "radrootsd".to_owned(),
                instance_id: "local".to_owned(),
                management_mode: "interactive_user_managed".to_owned(),
                install_state: ManagedRuntimeInstallState::Configured,
                binary_path: PathBuf::from("/tmp/bin/radrootsd"),
                config_path: PathBuf::from("/tmp/config.toml"),
                logs_path: PathBuf::from("/tmp/logs"),
                run_path: PathBuf::from("/tmp/run"),
                installed_version: "0.1.0-alpha.2".to_owned(),
                health_endpoint: None,
                secret_material_ref: None,
                last_started_at: None,
                last_stopped_at: None,
                notes: None,
            });

        let target = resolve_runtime_target(&context, "radrootsd", None);
        assert_eq!(target.instance_id, "local");
        assert_eq!(target.instance_source, "bootstrap_default");
        assert_eq!(
            target.runtime_group,
            ManagedRuntimeGroup::ActiveManagedTarget
        );
        assert!(target.predicted_paths.is_some());
    }
}
