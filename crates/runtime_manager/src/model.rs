use std::collections::BTreeMap;

use radroots_runtime_distribution::HardenedServiceTargets;
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
    pub instance_metadata: InstanceMetadataContract,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManagementDefaults {
    pub instance_cardinality: String,
    pub runtime_binding: String,
    pub admin_endpoint: String,
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
    pub default_instance_cardinality: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstanceMetadataContract {
    #[serde(default)]
    pub required_fields: Vec<String>,
    #[serde(default)]
    pub optional_fields: Vec<String>,
}
