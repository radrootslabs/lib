use std::collections::BTreeMap;

use radroots_runtime_distribution::HardenedServiceTargets;
use radroots_runtime_paths::{InstanceId, ServiceId};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RadrootsRuntimeManagementContract {
    pub schema: String,
    pub schema_version: u32,
    pub owner_doc: String,
    pub runtime_registry: String,
    pub distribution_contract: String,
    pub capabilities_contract: String,
    pub defaults: ManagementDefaults,
    pub management_clients: RuntimeGroups,
    pub managed_runtime_targets: RuntimeGroups,
    pub service_targets: HardenedServiceTargets,
    pub lifecycle: LifecycleContract,
    pub mode: BTreeMap<String, ManagementModeContract>,
    pub paths: BTreeMap<String, ManagementPathContract>,
    pub instance_metadata: InstanceMetadataContract,
    pub bootstrap: BTreeMap<String, BootstrapRuntimeContract>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManagementDefaults {
    pub instance_cardinality: String,
    pub managed_runtime_lookup: String,
    pub explicit_runtime_endpoint_overrides_precede_managed_instance_binding: bool,
    pub global_path_mutation_forbidden: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct RuntimeGroups {
    #[serde(default)]
    pub active: Vec<String>,
    #[serde(default)]
    pub defined: Vec<String>,
    #[serde(default)]
    pub bootstrap_only: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LifecycleContract {
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub destructive_actions: Vec<String>,
    #[serde(default)]
    pub health_states: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManagementModeContract {
    pub contract_state: String,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub supported_profiles: Vec<String>,
    pub service_manager_integration: bool,
    pub uses_absolute_binary_paths: bool,
    pub default_instance_cardinality: String,
    pub requires_explicit_pid_tracking: Option<bool>,
    pub requires_explicit_log_tracking: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManagementPathContract {
    pub shared_namespace: String,
    pub instance_registry_root_class: String,
    pub instance_registry_rel: String,
    pub artifact_cache_root_class: String,
    pub artifact_cache_rel: String,
    pub install_root_class: String,
    pub install_root_rel: String,
    pub state_root_class: String,
    pub state_root_rel: String,
    pub logs_root_class: String,
    pub logs_root_rel: String,
    pub run_root_class: String,
    pub run_root_rel: String,
    pub secrets_root_class: String,
    pub secrets_namespace_rel: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstanceMetadataContract {
    #[serde(default)]
    pub required_fields: Vec<String>,
    #[serde(default)]
    pub optional_fields: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BootstrapRuntimeContract {
    service_id: ServiceId,
    default_instance_id: InstanceId,
    preferred_cli_binding: bool,
}

impl BootstrapRuntimeContract {
    #[must_use]
    pub fn service_id(&self) -> &ServiceId {
        &self.service_id
    }

    #[must_use]
    pub fn default_instance_id(&self) -> &InstanceId {
        &self.default_instance_id
    }

    #[must_use]
    pub const fn preferred_cli_binding(&self) -> bool {
        self.preferred_cli_binding
    }
}

#[cfg(test)]
mod tests {
    use super::BootstrapRuntimeContract;

    #[test]
    fn bootstrap_accessors_project_parsed_fields_without_selecting_a_default() {
        let contract = toml::from_str::<BootstrapRuntimeContract>(
            r#"
service_id = "myc"
default_instance_id = "explicit-test-instance"
preferred_cli_binding = false
"#,
        )
        .expect("bootstrap contract");

        assert_eq!(contract.service_id().as_str(), "myc");
        assert_eq!(
            contract.default_instance_id().as_str(),
            "explicit-test-instance"
        );
        assert!(!contract.preferred_cli_binding());
    }
}
