use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
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
    pub lifecycle: LifecycleContract,
    pub mode: BTreeMap<String, ManagementModeContract>,
    pub paths: BTreeMap<String, ManagementPathContract>,
    pub instance_metadata: InstanceMetadataContract,
    pub bootstrap: BTreeMap<String, BootstrapRuntimeContract>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ManagementDefaults {
    pub instance_cardinality: String,
    pub managed_runtime_lookup: String,
    pub explicit_runtime_endpoint_overrides_precede_managed_instance_binding: bool,
    pub global_path_mutation_forbidden: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeGroups {
    #[serde(default)]
    pub active: Vec<String>,
    #[serde(default)]
    pub defined: Vec<String>,
    #[serde(default)]
    pub bootstrap_only: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct LifecycleContract {
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub destructive_actions: Vec<String>,
    #[serde(default)]
    pub health_states: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
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
pub struct InstanceMetadataContract {
    #[serde(default)]
    pub required_fields: Vec<String>,
    #[serde(default)]
    pub optional_fields: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BootstrapRuntimeContract {
    pub runtime_id: String,
    pub management_mode: String,
    pub default_instance_id: String,
    pub install_strategy: String,
    pub config_format: String,
    pub requires_bootstrap_secret: bool,
    pub requires_config_bootstrap: bool,
    pub requires_signer_provider: bool,
    pub health_surface: String,
    pub preferred_cli_binding: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManagedRuntimeInstallState {
    NotInstalled,
    Installed,
    Configured,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManagedRuntimeHealthState {
    NotInstalled,
    Stopped,
    Starting,
    Running,
    Degraded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagedRuntimeInstanceRecord {
    pub runtime_id: String,
    pub instance_id: String,
    pub management_mode: String,
    pub install_state: ManagedRuntimeInstallState,
    pub binary_path: PathBuf,
    pub config_path: PathBuf,
    pub logs_path: PathBuf,
    pub run_path: PathBuf,
    pub installed_version: String,
    pub health_endpoint: Option<String>,
    pub secret_material_ref: Option<String>,
    pub last_started_at: Option<String>,
    pub last_stopped_at: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagedRuntimeInstanceRegistry {
    pub schema: String,
    pub schema_version: u32,
    #[serde(default)]
    pub instances: Vec<ManagedRuntimeInstanceRecord>,
}

impl Default for ManagedRuntimeInstanceRegistry {
    fn default() -> Self {
        Self {
            schema: "radroots_runtime-instance-registry".to_string(),
            schema_version: 1,
            instances: Vec::new(),
        }
    }
}
