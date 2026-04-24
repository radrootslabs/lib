use std::path::PathBuf;

use radroots_runtime_paths::{
    RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver, RadrootsRuntimePathSelection,
};

use crate::{
    BootstrapRuntimeContract, ManagedRuntimeHealthState, ManagedRuntimeInstallState,
    ManagedRuntimeInstancePaths, ManagedRuntimeInstanceRecord, ManagedRuntimeInstanceRegistry,
    ManagementModeContract, RadrootsRuntimeManagementContract, RadrootsRuntimeManagerError,
    load_registry, resolve_instance_paths, resolve_shared_paths,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedRuntimeInspectionAvailability {
    Success,
    Unconfigured,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRuntimeInspection<T> {
    pub availability: ManagedRuntimeInspectionAvailability,
    pub view: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedRuntimeLifecycleAction {
    Install,
    Uninstall,
    Start,
    Stop,
    Restart,
    ConfigSet,
}

impl ManagedRuntimeLifecycleAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Uninstall => "uninstall",
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::ConfigSet => "config_set",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRuntimeStatusInspection {
    pub runtime_id: String,
    pub instance_id: String,
    pub instance_source: String,
    pub runtime_group: String,
    pub management_posture: String,
    pub state: String,
    pub source: String,
    pub detail: String,
    pub management_mode: Option<String>,
    pub service_manager_integration: Option<bool>,
    pub uses_absolute_binary_paths: Option<bool>,
    pub preferred_cli_binding: Option<bool>,
    pub install_state: String,
    pub health_state: String,
    pub health_source: String,
    pub registry_path: PathBuf,
    pub lifecycle_actions: Vec<String>,
    pub instance_paths: Option<ManagedRuntimeInstancePaths>,
    pub instance_record: Option<ManagedRuntimeInstanceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRuntimeLogsInspection {
    pub runtime_id: String,
    pub instance_id: String,
    pub instance_source: String,
    pub runtime_group: String,
    pub state: String,
    pub source: String,
    pub detail: String,
    pub stdout_log_path: Option<PathBuf>,
    pub stderr_log_path: Option<PathBuf>,
    pub stdout_log_present: bool,
    pub stderr_log_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRuntimeConfigInspection {
    pub runtime_id: String,
    pub instance_id: String,
    pub instance_source: String,
    pub runtime_group: String,
    pub state: String,
    pub source: String,
    pub detail: String,
    pub config_format: Option<String>,
    pub config_path: Option<PathBuf>,
    pub config_present: bool,
    pub requires_bootstrap_secret: Option<bool>,
    pub requires_config_bootstrap: Option<bool>,
    pub requires_signer_provider: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRuntimeActionInspection {
    pub action: String,
    pub runtime_id: String,
    pub instance_id: String,
    pub instance_source: String,
    pub runtime_group: String,
    pub state: String,
    pub source: String,
    pub detail: String,
    pub mutates_bindings: bool,
    pub next_step: Option<String>,
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

pub fn load_management_context_with_selection(
    contract: RadrootsRuntimeManagementContract,
    resolver: &RadrootsPathResolver,
    selection: &RadrootsRuntimePathSelection,
) -> Result<ManagedRuntimeContext, RadrootsRuntimeManagerError> {
    let overrides = selection.caller_overrides()?;
    load_management_context(contract, resolver, selection.profile, &overrides)
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

pub fn inspect_runtime_status(
    target: &ManagedRuntimeTarget,
    lifecycle_actions: &[String],
) -> ManagedRuntimeInspection<ManagedRuntimeStatusInspection> {
    let availability = if target.runtime_group == ManagedRuntimeGroup::Unknown {
        ManagedRuntimeInspectionAvailability::Unconfigured
    } else {
        ManagedRuntimeInspectionAvailability::Success
    };

    ManagedRuntimeInspection {
        availability,
        view: ManagedRuntimeStatusInspection {
            runtime_id: target.runtime_id.clone(),
            instance_id: target.instance_id.clone(),
            instance_source: target.instance_source.clone(),
            runtime_group: target.runtime_group.as_str().to_owned(),
            management_posture: target.runtime_group.posture().to_owned(),
            state: status_state(target).to_owned(),
            source: "runtime management contract + shared instance registry".to_owned(),
            detail: status_detail(target),
            management_mode: target.management_mode.clone(),
            service_manager_integration: target
                .mode_contract
                .as_ref()
                .map(|mode| mode.service_manager_integration),
            uses_absolute_binary_paths: target
                .mode_contract
                .as_ref()
                .map(|mode| mode.uses_absolute_binary_paths),
            preferred_cli_binding: target
                .bootstrap
                .as_ref()
                .map(|entry| entry.preferred_cli_binding),
            install_state: target
                .instance_record
                .as_ref()
                .map(|record| install_state_label(record.install_state))
                .unwrap_or_else(|| install_state_label(ManagedRuntimeInstallState::NotInstalled))
                .to_owned(),
            health_state: infer_health_state(target).0.to_owned(),
            health_source: infer_health_state(target).1.to_owned(),
            registry_path: target.registry_path.clone(),
            lifecycle_actions: if target.runtime_group == ManagedRuntimeGroup::ActiveManagedTarget {
                lifecycle_actions.to_vec()
            } else {
                Vec::new()
            },
            instance_paths: target.predicted_paths.clone(),
            instance_record: target.instance_record.clone(),
        },
    }
}

pub fn inspect_runtime_logs(
    target: &ManagedRuntimeTarget,
) -> ManagedRuntimeInspection<ManagedRuntimeLogsInspection> {
    let stdout_log_path = target
        .predicted_paths
        .as_ref()
        .map(|paths| paths.stdout_log_path.clone());
    let stderr_log_path = target
        .predicted_paths
        .as_ref()
        .map(|paths| paths.stderr_log_path.clone());
    let availability = match target.runtime_group {
        ManagedRuntimeGroup::Unknown => ManagedRuntimeInspectionAvailability::Unconfigured,
        ManagedRuntimeGroup::ActiveManagedTarget => ManagedRuntimeInspectionAvailability::Success,
        ManagedRuntimeGroup::DefinedManagedTarget | ManagedRuntimeGroup::BootstrapOnly => {
            if target.instance_record.is_some() {
                ManagedRuntimeInspectionAvailability::Success
            } else {
                ManagedRuntimeInspectionAvailability::Unsupported
            }
        }
    };
    let detail = match target.runtime_group {
        ManagedRuntimeGroup::ActiveManagedTarget => {
            "runtime logs report the managed stdout/stderr locations for the active managed instance"
                .to_owned()
        }
        ManagedRuntimeGroup::DefinedManagedTarget => format!(
            "runtime `{}` is only a defined future managed target; no active generic logs surface exists without a registered instance",
            target.runtime_id
        ),
        ManagedRuntimeGroup::BootstrapOnly => format!(
            "runtime `{}` remains bootstrap_only and direct-bindable in this wave; generic managed logs are not admitted",
            target.runtime_id
        ),
        ManagedRuntimeGroup::Unknown => unknown_runtime_detail(target),
    };

    ManagedRuntimeInspection {
        availability,
        view: ManagedRuntimeLogsInspection {
            runtime_id: target.runtime_id.clone(),
            instance_id: target.instance_id.clone(),
            instance_source: target.instance_source.clone(),
            runtime_group: target.runtime_group.as_str().to_owned(),
            state: match availability {
                ManagedRuntimeInspectionAvailability::Success => "ready".to_owned(),
                ManagedRuntimeInspectionAvailability::Unconfigured => "unknown_runtime".to_owned(),
                ManagedRuntimeInspectionAvailability::Unsupported => "unsupported".to_owned(),
            },
            source: "runtime management contract + shared instance registry".to_owned(),
            detail,
            stdout_log_path: stdout_log_path.clone(),
            stderr_log_path: stderr_log_path.clone(),
            stdout_log_present: path_present(stdout_log_path.as_ref()).unwrap_or_else(|| {
                target
                    .instance_record
                    .as_ref()
                    .is_some_and(|record| record.logs_path.join("stdout.log").exists())
            }),
            stderr_log_present: path_present(stderr_log_path.as_ref()).unwrap_or_else(|| {
                target
                    .instance_record
                    .as_ref()
                    .is_some_and(|record| record.logs_path.join("stderr.log").exists())
            }),
        },
    }
}

pub fn inspect_runtime_config(
    target: &ManagedRuntimeTarget,
) -> ManagedRuntimeInspection<ManagedRuntimeConfigInspection> {
    let availability = match target.runtime_group {
        ManagedRuntimeGroup::Unknown => ManagedRuntimeInspectionAvailability::Unconfigured,
        ManagedRuntimeGroup::ActiveManagedTarget => ManagedRuntimeInspectionAvailability::Success,
        ManagedRuntimeGroup::DefinedManagedTarget | ManagedRuntimeGroup::BootstrapOnly => {
            if target.instance_record.is_some() {
                ManagedRuntimeInspectionAvailability::Success
            } else {
                ManagedRuntimeInspectionAvailability::Unsupported
            }
        }
    };
    let config_path = target
        .instance_record
        .as_ref()
        .map(|record| record.config_path.clone());
    let detail = match target.runtime_group {
        ManagedRuntimeGroup::ActiveManagedTarget => {
            if config_path.is_some() {
                "runtime config show reports the managed config location without mutating bindings"
                    .to_owned()
            } else {
                format!(
                    "managed runtime `{}` has no registered instance config yet",
                    target.runtime_id
                )
            }
        }
        ManagedRuntimeGroup::DefinedManagedTarget => format!(
            "runtime `{}` is only a defined future managed target; generic config surfaces are not admitted without a registered instance",
            target.runtime_id
        ),
        ManagedRuntimeGroup::BootstrapOnly => format!(
            "runtime `{}` remains bootstrap_only and direct-bindable in this wave; generic managed config is not admitted",
            target.runtime_id
        ),
        ManagedRuntimeGroup::Unknown => unknown_runtime_detail(target),
    };

    ManagedRuntimeInspection {
        availability,
        view: ManagedRuntimeConfigInspection {
            runtime_id: target.runtime_id.clone(),
            instance_id: target.instance_id.clone(),
            instance_source: target.instance_source.clone(),
            runtime_group: target.runtime_group.as_str().to_owned(),
            state: match availability {
                ManagedRuntimeInspectionAvailability::Success => {
                    if config_path.is_some() {
                        "ready".to_owned()
                    } else {
                        "not_installed".to_owned()
                    }
                }
                ManagedRuntimeInspectionAvailability::Unconfigured => "unknown_runtime".to_owned(),
                ManagedRuntimeInspectionAvailability::Unsupported => "unsupported".to_owned(),
            },
            source: "runtime management contract + shared instance registry".to_owned(),
            detail,
            config_format: target
                .bootstrap
                .as_ref()
                .map(|entry| entry.config_format.clone()),
            config_path: config_path.clone(),
            config_present: config_path.as_ref().is_some_and(|path| path.exists()),
            requires_bootstrap_secret: target
                .bootstrap
                .as_ref()
                .map(|entry| entry.requires_bootstrap_secret),
            requires_config_bootstrap: target
                .bootstrap
                .as_ref()
                .map(|entry| entry.requires_config_bootstrap),
            requires_signer_provider: target
                .bootstrap
                .as_ref()
                .map(|entry| entry.requires_signer_provider),
        },
    }
}

pub fn inspect_runtime_action(
    target: &ManagedRuntimeTarget,
    action: ManagedRuntimeLifecycleAction,
    detail_override: Option<String>,
) -> ManagedRuntimeInspection<ManagedRuntimeActionInspection> {
    let (availability, state, detail, next_step) = match target.runtime_group {
        ManagedRuntimeGroup::ActiveManagedTarget => (
            ManagedRuntimeInspectionAvailability::Unsupported,
            "deferred",
            detail_override.unwrap_or_else(|| {
                format!(
                    "runtime {} `{}` is not supported for this managed target",
                    action.as_str().replace('_', " "),
                    target.runtime_id
                )
            }),
            None,
        ),
        ManagedRuntimeGroup::DefinedManagedTarget => (
            ManagedRuntimeInspectionAvailability::Unsupported,
            "unsupported",
            detail_override.unwrap_or_else(|| {
                format!(
                    "runtime `{}` is only a defined future managed target; `{}` is not admitted in the current wave",
                    target.runtime_id,
                    action.as_str().replace('_', " ")
                )
            }),
            None,
        ),
        ManagedRuntimeGroup::BootstrapOnly => (
            ManagedRuntimeInspectionAvailability::Unsupported,
            "unsupported",
            detail_override.unwrap_or_else(|| {
                format!(
                    "runtime `{}` remains bootstrap_only and direct-bindable in this wave; generic managed `{}` is not admitted",
                    target.runtime_id,
                    action.as_str().replace('_', " ")
                )
            }),
            None,
        ),
        ManagedRuntimeGroup::Unknown => (
            ManagedRuntimeInspectionAvailability::Unconfigured,
            "unknown_runtime",
            detail_override.unwrap_or_else(|| unknown_runtime_detail(target)),
            None,
        ),
    };

    ManagedRuntimeInspection {
        availability,
        view: ManagedRuntimeActionInspection {
            action: action.as_str().to_owned(),
            runtime_id: target.runtime_id.clone(),
            instance_id: target.instance_id.clone(),
            instance_source: target.instance_source.clone(),
            runtime_group: target.runtime_group.as_str().to_owned(),
            state: state.to_owned(),
            source: "generic runtime-management command family".to_owned(),
            detail,
            mutates_bindings: false,
            next_step,
        },
    }
}

fn status_state(target: &ManagedRuntimeTarget) -> &'static str {
    match target.runtime_group {
        ManagedRuntimeGroup::ActiveManagedTarget => match target.instance_record.as_ref() {
            Some(record) => install_state_label(record.install_state),
            None => "not_installed",
        },
        ManagedRuntimeGroup::DefinedManagedTarget => "defined_not_active",
        ManagedRuntimeGroup::BootstrapOnly => "bootstrap_only",
        ManagedRuntimeGroup::Unknown => "unknown_runtime",
    }
}

fn status_detail(target: &ManagedRuntimeTarget) -> String {
    match target.runtime_group {
        ManagedRuntimeGroup::ActiveManagedTarget => match &target.instance_record {
            Some(record) => format!(
                "managed runtime `{}` instance `{}` is registered with config at {}",
                target.runtime_id,
                target.instance_id,
                record.config_path.display()
            ),
            None => format!(
                "managed runtime `{}` has no registered instance `{}` in {}",
                target.runtime_id,
                target.instance_id,
                target.registry_path.display()
            ),
        },
        ManagedRuntimeGroup::DefinedManagedTarget => format!(
            "runtime `{}` is defined in the management contract but not yet admitted as an active managed target",
            target.runtime_id
        ),
        ManagedRuntimeGroup::BootstrapOnly => format!(
            "runtime `{}` is bootstrap_only in the management contract and remains direct-bindable outside managed lifecycle in this wave",
            target.runtime_id
        ),
        ManagedRuntimeGroup::Unknown => unknown_runtime_detail(target),
    }
}

fn unknown_runtime_detail(target: &ManagedRuntimeTarget) -> String {
    format!(
        "runtime `{}` is not present in the current runtime-management contract",
        target.runtime_id
    )
}

fn infer_health_state(target: &ManagedRuntimeTarget) -> (&'static str, &'static str) {
    let Some(record) = &target.instance_record else {
        return (
            health_state_label(ManagedRuntimeHealthState::NotInstalled),
            "registry_absent",
        );
    };
    if record.install_state == ManagedRuntimeInstallState::Failed {
        return (
            health_state_label(ManagedRuntimeHealthState::Failed),
            "registry_install_state",
        );
    }

    if let Some(paths) = target.predicted_paths.as_ref() {
        if crate::process_running(paths).unwrap_or(false) {
            return (
                health_state_label(ManagedRuntimeHealthState::Running),
                "process_probe",
            );
        }
    } else if record.run_path.join("runtime.pid").exists() {
        return (
            health_state_label(ManagedRuntimeHealthState::Running),
            "pid_file_presence",
        );
    }

    match record.install_state {
        ManagedRuntimeInstallState::NotInstalled => (
            health_state_label(ManagedRuntimeHealthState::NotInstalled),
            "registry_install_state",
        ),
        ManagedRuntimeInstallState::Installed | ManagedRuntimeInstallState::Configured => (
            health_state_label(ManagedRuntimeHealthState::Stopped),
            "pid_file_absent",
        ),
        ManagedRuntimeInstallState::Failed => (
            health_state_label(ManagedRuntimeHealthState::Failed),
            "registry_install_state",
        ),
    }
}

fn install_state_label(state: ManagedRuntimeInstallState) -> &'static str {
    match state {
        ManagedRuntimeInstallState::NotInstalled => "not_installed",
        ManagedRuntimeInstallState::Installed => "installed",
        ManagedRuntimeInstallState::Configured => "configured",
        ManagedRuntimeInstallState::Failed => "failed",
    }
}

fn health_state_label(state: ManagedRuntimeHealthState) -> &'static str {
    match state {
        ManagedRuntimeHealthState::NotInstalled => "not_installed",
        ManagedRuntimeHealthState::Stopped => "stopped",
        ManagedRuntimeHealthState::Starting => "starting",
        ManagedRuntimeHealthState::Running => "running",
        ManagedRuntimeHealthState::Degraded => "degraded",
        ManagedRuntimeHealthState::Failed => "failed",
    }
}

fn path_present(path: Option<&PathBuf>) -> Option<bool> {
    path.map(|value| value.exists())
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
        ManagedRuntimeGroup, active_management_mode_for_profile, load_management_context,
        resolve_runtime_target,
    };
    use crate::{ManagedRuntimeInstallState, ManagedRuntimeInstanceRecord, parse_contract_str};

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
