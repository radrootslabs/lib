use core::fmt;
use std::path::Path;

use radroots_runtime_paths::{RadrootsPathProfile, RuntimeContext, RuntimeContextSource};

use crate::paths::{resolve_instance_paths, resolve_shared_paths};
use crate::registry::{remove_instance, upsert_instance};
use crate::{
    BootstrapRuntimeContract, ManagedRuntimeHealthState, ManagedRuntimeInstallState,
    ManagedRuntimeInstancePaths, ManagedRuntimeInstanceRecord, ManagedRuntimeInstanceRegistry,
    ManagementModeContract, RadrootsRuntimeManagementContract, RadrootsRuntimeManagerError,
    load_registry,
};

#[derive(Clone)]
pub struct ManagedRuntimeContext {
    contract: RadrootsRuntimeManagementContract,
    manager_context: RuntimeContext,
    shared_paths: crate::ManagedRuntimeSharedPaths,
    registry: ManagedRuntimeInstanceRegistry,
}

impl ManagedRuntimeContext {
    #[must_use]
    pub fn contract(&self) -> &RadrootsRuntimeManagementContract {
        &self.contract
    }

    #[must_use]
    pub fn manager_context(&self) -> &RuntimeContext {
        &self.manager_context
    }

    #[must_use]
    pub fn shared_paths(&self) -> &crate::ManagedRuntimeSharedPaths {
        &self.shared_paths
    }

    #[must_use]
    pub fn registry(&self) -> &ManagedRuntimeInstanceRegistry {
        &self.registry
    }

    pub fn register_instance(
        &mut self,
        runtime_context: &RuntimeContext,
        install_state: ManagedRuntimeInstallState,
    ) -> Result<(), RadrootsRuntimeManagerError> {
        ensure_context_scope(&self.manager_context, runtime_context)?;
        upsert_instance(
            &mut self.registry,
            ManagedRuntimeInstanceRecord::new(runtime_context, install_state),
        );
        Ok(())
    }

    pub fn remove_instance(
        &mut self,
        runtime_context: &RuntimeContext,
    ) -> Result<Option<ManagedRuntimeInstanceRecord>, RadrootsRuntimeManagerError> {
        ensure_context_scope(&self.manager_context, runtime_context)?;
        Ok(remove_instance(
            &mut self.registry,
            runtime_context.service(),
            runtime_context.instance(),
        ))
    }
}

impl fmt::Debug for ManagedRuntimeContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedRuntimeContext")
            .field("manager_context", &self.manager_context)
            .field("shared_paths", &self.shared_paths)
            .field("registry", &"[redacted]")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedRuntimeGroup {
    ActiveManagedTarget,
    DefinedManagedTarget,
    BootstrapOnly,
    Unknown,
}

impl ManagedRuntimeGroup {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ActiveManagedTarget => "active_managed_target",
            Self::DefinedManagedTarget => "defined_managed_target",
            Self::BootstrapOnly => "bootstrap_only",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub fn posture(self) -> &'static str {
        match self {
            Self::ActiveManagedTarget => "active_managed_target",
            Self::DefinedManagedTarget => "defined_future_target",
            Self::BootstrapOnly => "bootstrap_only_direct_binding",
            Self::Unknown => "unknown_runtime",
        }
    }
}

/// A sealed, internally cross-bound management target.
///
/// ```compile_fail
/// use radroots_runtime_manager::ManagedRuntimeTarget;
///
/// let _ = ManagedRuntimeTarget {
///     context: todo!(),
///     instance_source: todo!(),
///     runtime_group: todo!(),
///     management_mode: None,
///     mode_contract: None,
///     bootstrap: None,
///     instance_record: None,
///     predicted_paths: None,
/// };
/// ```
#[derive(Clone)]
pub struct ManagedRuntimeTarget {
    context: RuntimeContext,
    instance_source: RuntimeContextSource,
    runtime_group: ManagedRuntimeGroup,
    management_mode: Option<String>,
    mode_contract: Option<ManagementModeContract>,
    bootstrap: Option<BootstrapRuntimeContract>,
    instance_record: Option<ManagedRuntimeInstanceRecord>,
    predicted_paths: Option<ManagedRuntimeInstancePaths>,
}

impl ManagedRuntimeTarget {
    #[must_use]
    pub fn context(&self) -> &RuntimeContext {
        &self.context
    }

    #[must_use]
    pub fn instance_source(&self) -> RuntimeContextSource {
        self.instance_source
    }

    #[must_use]
    pub fn runtime_group(&self) -> ManagedRuntimeGroup {
        self.runtime_group
    }

    #[must_use]
    pub fn management_mode(&self) -> Option<&str> {
        self.management_mode.as_deref()
    }

    #[must_use]
    pub fn mode_contract(&self) -> Option<&ManagementModeContract> {
        self.mode_contract.as_ref()
    }

    #[must_use]
    pub fn bootstrap(&self) -> Option<&BootstrapRuntimeContract> {
        self.bootstrap.as_ref()
    }

    #[must_use]
    pub fn instance_record(&self) -> Option<&ManagedRuntimeInstanceRecord> {
        self.instance_record.as_ref()
    }

    #[must_use]
    pub fn predicted_paths(&self) -> Option<&ManagedRuntimeInstancePaths> {
        self.predicted_paths.as_ref()
    }
}

impl fmt::Debug for ManagedRuntimeTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedRuntimeTarget")
            .field("context", &self.context)
            .field("runtime_group", &self.runtime_group)
            .field("predicted_paths", &self.predicted_paths)
            .finish_non_exhaustive()
    }
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
    #[must_use]
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
    pub instance_source: RuntimeContextSource,
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
    pub lifecycle_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRuntimeLogsInspection {
    pub runtime_id: String,
    pub instance_id: String,
    pub instance_source: RuntimeContextSource,
    pub runtime_group: String,
    pub state: String,
    pub source: String,
    pub detail: String,
    pub stdout_log_present: bool,
    pub stderr_log_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRuntimeConfigInspection {
    pub runtime_id: String,
    pub instance_id: String,
    pub instance_source: RuntimeContextSource,
    pub runtime_group: String,
    pub state: String,
    pub source: String,
    pub detail: String,
    pub config_format: Option<String>,
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
    pub instance_source: RuntimeContextSource,
    pub runtime_group: String,
    pub state: String,
    pub source: String,
    pub detail: String,
    pub mutates_bindings: bool,
    pub next_step: Option<String>,
}

pub fn load_management_context(
    contract: RadrootsRuntimeManagementContract,
    manager_context: RuntimeContext,
) -> Result<ManagedRuntimeContext, RadrootsRuntimeManagerError> {
    active_management_mode_for_profile(&contract, manager_context.profile())?;
    let shared_paths = resolve_shared_paths(&manager_context);
    let registry = load_registry(shared_paths.instance_registry_path())?;
    Ok(ManagedRuntimeContext {
        contract,
        manager_context,
        shared_paths,
        registry,
    })
}

pub fn active_management_mode_for_profile(
    contract: &RadrootsRuntimeManagementContract,
    profile: RadrootsPathProfile,
) -> Result<&str, RadrootsRuntimeManagerError> {
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
        .ok_or(RadrootsRuntimeManagerError::UnsupportedProfile)
}

pub fn resolve_runtime_target(
    context: &ManagedRuntimeContext,
    runtime_context: RuntimeContext,
) -> Result<ManagedRuntimeTarget, RadrootsRuntimeManagerError> {
    ensure_context_scope(&context.manager_context, &runtime_context)?;
    let runtime_id = runtime_context.service().as_str();
    let runtime_group = runtime_group(&context.contract, runtime_id);
    let bootstrap = context.contract.bootstrap.get(runtime_id).cloned();
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
        .find(|record| record.matches_context(&runtime_context))
        .cloned();
    let predicted_paths = matches!(
        runtime_group,
        ManagedRuntimeGroup::ActiveManagedTarget | ManagedRuntimeGroup::DefinedManagedTarget
    )
    .then(|| resolve_instance_paths(&context.shared_paths, &runtime_context));

    Ok(ManagedRuntimeTarget {
        instance_source: runtime_context.sources().instance(),
        context: runtime_context,
        runtime_group,
        management_mode,
        mode_contract,
        bootstrap,
        instance_record,
        predicted_paths,
    })
}

fn ensure_context_scope(
    manager: &RuntimeContext,
    target: &RuntimeContext,
) -> Result<(), RadrootsRuntimeManagerError> {
    if manager.profile() != target.profile()
        || context_roots(manager)
            .iter()
            .zip(context_roots(target))
            .any(|(left, right)| left != &right)
    {
        return Err(RadrootsRuntimeManagerError::RuntimeContextMismatch);
    }
    Ok(())
}

fn context_roots(context: &RuntimeContext) -> [&Path; 6] {
    let paths = context.paths();
    [
        instance_root(paths.config()),
        instance_root(paths.state()),
        instance_root(paths.cache()),
        instance_root(paths.logs()),
        instance_root(paths.run()),
        instance_root(paths.secrets()),
    ]
}

fn instance_root(path: &Path) -> &Path {
    path.parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("RuntimeContext service paths contain the sealed services/service/instance suffix")
}

#[must_use]
pub fn inspect_runtime_status(
    target: &ManagedRuntimeTarget,
    lifecycle_actions: &[String],
) -> ManagedRuntimeInspection<ManagedRuntimeStatusInspection> {
    let availability = if target.runtime_group == ManagedRuntimeGroup::Unknown {
        ManagedRuntimeInspectionAvailability::Unconfigured
    } else {
        ManagedRuntimeInspectionAvailability::Success
    };
    let (health_state, health_source) = infer_health_state(target);

    ManagedRuntimeInspection {
        availability,
        view: ManagedRuntimeStatusInspection {
            runtime_id: target.context.service().to_string(),
            instance_id: target.context.instance().to_string(),
            instance_source: target.instance_source,
            runtime_group: target.runtime_group.as_str().to_owned(),
            management_posture: target.runtime_group.posture().to_owned(),
            state: status_state(target).to_owned(),
            source: "runtime management contract + typed instance registry".to_owned(),
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
                .map(|record| install_state_label(record.install_state()))
                .unwrap_or_else(|| install_state_label(ManagedRuntimeInstallState::NotInstalled))
                .to_owned(),
            health_state: health_state.to_owned(),
            health_source: health_source.to_owned(),
            lifecycle_actions: if target.runtime_group == ManagedRuntimeGroup::ActiveManagedTarget {
                lifecycle_actions.to_vec()
            } else {
                Vec::new()
            },
        },
    }
}

#[must_use]
pub fn inspect_runtime_logs(
    target: &ManagedRuntimeTarget,
) -> ManagedRuntimeInspection<ManagedRuntimeLogsInspection> {
    let availability = managed_inspection_availability(target);
    let stdout_log_present = target
        .predicted_paths
        .as_ref()
        .is_some_and(|paths| paths.stdout_log_path().exists());
    let stderr_log_present = target
        .predicted_paths
        .as_ref()
        .is_some_and(|paths| paths.stderr_log_path().exists());

    ManagedRuntimeInspection {
        availability,
        view: ManagedRuntimeLogsInspection {
            runtime_id: target.context.service().to_string(),
            instance_id: target.context.instance().to_string(),
            instance_source: target.instance_source,
            runtime_group: target.runtime_group.as_str().to_owned(),
            state: availability_state(availability),
            source: "runtime management contract + manager-owned tracking".to_owned(),
            detail: logs_detail(target),
            stdout_log_present,
            stderr_log_present,
        },
    }
}

#[must_use]
pub fn inspect_runtime_config(
    target: &ManagedRuntimeTarget,
) -> ManagedRuntimeInspection<ManagedRuntimeConfigInspection> {
    let availability = managed_inspection_availability(target);
    let config_path = target.instance_record.as_ref().and_then(|_| {
        target
            .predicted_paths
            .as_ref()
            .map(ManagedRuntimeInstancePaths::config_path)
    });
    let config_present = config_path.as_deref().is_some_and(Path::exists);

    ManagedRuntimeInspection {
        availability,
        view: ManagedRuntimeConfigInspection {
            runtime_id: target.context.service().to_string(),
            instance_id: target.context.instance().to_string(),
            instance_source: target.instance_source,
            runtime_group: target.runtime_group.as_str().to_owned(),
            state: match availability {
                ManagedRuntimeInspectionAvailability::Success if config_path.is_some() => {
                    "ready".to_owned()
                }
                ManagedRuntimeInspectionAvailability::Success => "not_installed".to_owned(),
                other => availability_state(other),
            },
            source: "runtime context + typed instance registry".to_owned(),
            detail: config_detail(target, config_path.is_some()),
            config_format: target
                .bootstrap
                .as_ref()
                .map(|entry| entry.config_format.clone()),
            config_present,
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

#[must_use]
pub fn inspect_runtime_action(
    target: &ManagedRuntimeTarget,
    action: ManagedRuntimeLifecycleAction,
) -> ManagedRuntimeInspection<ManagedRuntimeActionInspection> {
    let (availability, state, detail) = match target.runtime_group {
        ManagedRuntimeGroup::ActiveManagedTarget => (
            ManagedRuntimeInspectionAvailability::Unsupported,
            "deferred",
            format!(
                "runtime {} `{}` is not supported for this managed target",
                action.as_str().replace('_', " "),
                target.context.service()
            ),
        ),
        ManagedRuntimeGroup::DefinedManagedTarget => (
            ManagedRuntimeInspectionAvailability::Unsupported,
            "unsupported",
            format!(
                "runtime `{}` is only a defined future managed target; `{}` is not admitted in the current wave",
                target.context.service(),
                action.as_str().replace('_', " ")
            ),
        ),
        ManagedRuntimeGroup::BootstrapOnly => (
            ManagedRuntimeInspectionAvailability::Unsupported,
            "unsupported",
            format!(
                "runtime `{}` remains bootstrap_only; generic managed `{}` is not admitted",
                target.context.service(),
                action.as_str().replace('_', " ")
            ),
        ),
        ManagedRuntimeGroup::Unknown => (
            ManagedRuntimeInspectionAvailability::Unconfigured,
            "unknown_runtime",
            unknown_runtime_detail(target),
        ),
    };

    ManagedRuntimeInspection {
        availability,
        view: ManagedRuntimeActionInspection {
            action: action.as_str().to_owned(),
            runtime_id: target.context.service().to_string(),
            instance_id: target.context.instance().to_string(),
            instance_source: target.instance_source,
            runtime_group: target.runtime_group.as_str().to_owned(),
            state: state.to_owned(),
            source: "generic runtime-management command family".to_owned(),
            detail,
            mutates_bindings: false,
            next_step: None,
        },
    }
}

fn managed_inspection_availability(
    target: &ManagedRuntimeTarget,
) -> ManagedRuntimeInspectionAvailability {
    match target.runtime_group {
        ManagedRuntimeGroup::Unknown => ManagedRuntimeInspectionAvailability::Unconfigured,
        ManagedRuntimeGroup::ActiveManagedTarget => ManagedRuntimeInspectionAvailability::Success,
        ManagedRuntimeGroup::DefinedManagedTarget | ManagedRuntimeGroup::BootstrapOnly => {
            if target.instance_record.is_some() {
                ManagedRuntimeInspectionAvailability::Success
            } else {
                ManagedRuntimeInspectionAvailability::Unsupported
            }
        }
    }
}

fn availability_state(availability: ManagedRuntimeInspectionAvailability) -> String {
    match availability {
        ManagedRuntimeInspectionAvailability::Success => "ready",
        ManagedRuntimeInspectionAvailability::Unconfigured => "unknown_runtime",
        ManagedRuntimeInspectionAvailability::Unsupported => "unsupported",
    }
    .to_owned()
}

fn status_state(target: &ManagedRuntimeTarget) -> &'static str {
    match target.runtime_group {
        ManagedRuntimeGroup::ActiveManagedTarget => target
            .instance_record
            .as_ref()
            .map(|record| install_state_label(record.install_state()))
            .unwrap_or("not_installed"),
        ManagedRuntimeGroup::DefinedManagedTarget => "defined_not_active",
        ManagedRuntimeGroup::BootstrapOnly => "bootstrap_only",
        ManagedRuntimeGroup::Unknown => "unknown_runtime",
    }
}

fn status_detail(target: &ManagedRuntimeTarget) -> String {
    match target.runtime_group {
        ManagedRuntimeGroup::ActiveManagedTarget if target.instance_record.is_some() => format!(
            "managed runtime `{}` instance `{}` is registered",
            target.context.service(),
            target.context.instance()
        ),
        ManagedRuntimeGroup::ActiveManagedTarget => format!(
            "managed runtime `{}` has no registered instance `{}`",
            target.context.service(),
            target.context.instance()
        ),
        ManagedRuntimeGroup::DefinedManagedTarget => format!(
            "runtime `{}` is defined but not yet an active managed target",
            target.context.service()
        ),
        ManagedRuntimeGroup::BootstrapOnly => format!(
            "runtime `{}` is bootstrap_only in the management contract",
            target.context.service()
        ),
        ManagedRuntimeGroup::Unknown => unknown_runtime_detail(target),
    }
}

fn logs_detail(target: &ManagedRuntimeTarget) -> String {
    match target.runtime_group {
        ManagedRuntimeGroup::ActiveManagedTarget => {
            "runtime logs use manager-owned stdout/stderr tracking".to_owned()
        }
        ManagedRuntimeGroup::DefinedManagedTarget => format!(
            "runtime `{}` is a defined future managed target",
            target.context.service()
        ),
        ManagedRuntimeGroup::BootstrapOnly => format!(
            "runtime `{}` is bootstrap_only; generic managed logs are not admitted",
            target.context.service()
        ),
        ManagedRuntimeGroup::Unknown => unknown_runtime_detail(target),
    }
}

fn config_detail(target: &ManagedRuntimeTarget, registered: bool) -> String {
    match target.runtime_group {
        ManagedRuntimeGroup::ActiveManagedTarget if registered => {
            "runtime config is derived from the validated service context".to_owned()
        }
        ManagedRuntimeGroup::ActiveManagedTarget => format!(
            "managed runtime `{}` has no registered instance config",
            target.context.service()
        ),
        ManagedRuntimeGroup::DefinedManagedTarget => format!(
            "runtime `{}` is a defined future managed target",
            target.context.service()
        ),
        ManagedRuntimeGroup::BootstrapOnly => format!(
            "runtime `{}` is bootstrap_only; generic managed config is not admitted",
            target.context.service()
        ),
        ManagedRuntimeGroup::Unknown => unknown_runtime_detail(target),
    }
}

fn unknown_runtime_detail(target: &ManagedRuntimeTarget) -> String {
    format!(
        "runtime `{}` is not present in the current runtime-management contract",
        target.context.service()
    )
}

fn infer_health_state(target: &ManagedRuntimeTarget) -> (&'static str, &'static str) {
    let Some(record) = &target.instance_record else {
        return (
            health_state_label(ManagedRuntimeHealthState::NotInstalled),
            "registry_absent",
        );
    };
    if record.install_state() == ManagedRuntimeInstallState::Failed {
        return (
            health_state_label(ManagedRuntimeHealthState::Failed),
            "registry_install_state",
        );
    }
    if target
        .predicted_paths
        .as_ref()
        .is_some_and(|paths| crate::process_running(paths).unwrap_or(false))
    {
        return (
            health_state_label(ManagedRuntimeHealthState::Running),
            "process_probe",
        );
    }
    if record.install_state() == ManagedRuntimeInstallState::NotInstalled {
        (
            health_state_label(ManagedRuntimeHealthState::NotInstalled),
            "registry_install_state",
        )
    } else {
        (
            health_state_label(ManagedRuntimeHealthState::Stopped),
            "pid_file_absent",
        )
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

#[must_use]
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
    use std::fs;

    use radroots_runtime_paths::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
    };
    use tempfile::tempdir;

    use super::{
        ManagedRuntimeGroup, ManagedRuntimeInspectionAvailability, ManagedRuntimeLifecycleAction,
        active_management_mode_for_profile, inspect_runtime_action, inspect_runtime_config,
        inspect_runtime_logs, inspect_runtime_status, load_management_context,
        resolve_runtime_target, runtime_group,
    };
    use crate::{ManagedRuntimeInstallState, RadrootsRuntimeManagerError, parse_contract_str};

    const CONTRACT: &str = r#"
schema = "radroots-runtime-management"
schema_version = 1
owner_doc = "owner"
runtime_registry = "registry.toml"
distribution_contract = "distribution.toml"
capabilities_contract = "capabilities.toml"

[defaults]
instance_cardinality = "multiple"
managed_runtime_lookup = "typed_instance_registry"
explicit_runtime_endpoint_overrides_precede_managed_instance_binding = true
global_path_mutation_forbidden = true

[management_clients]
active = ["cli"]

[managed_runtime_targets]
active = ["radrootsd"]
defined = ["myc", "rhi"]
bootstrap_only = ["hyf"]

[lifecycle]
actions = ["install", "start"]
health_states = ["not_installed", "running"]

[mode.interactive_user_managed]
contract_state = "active"
platforms = ["linux"]
supported_profiles = ["repo_local"]
service_manager_integration = false
uses_absolute_binary_paths = true
default_instance_cardinality = "multiple"

[mode.service_host_managed]
contract_state = "defined"
platforms = ["linux"]
supported_profiles = ["service_host"]
service_manager_integration = true
uses_absolute_binary_paths = true
default_instance_cardinality = "multiple"

[paths.interactive_user_managed]
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

    fn context(service: &str, instance: &str, root: &std::path::Path) -> RuntimeContext {
        RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(root.to_path_buf()),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new(service).expect("service"),
            InstanceId::new(instance).expect("instance"),
        )
        .expect("context")
    }

    fn manager(root: &std::path::Path) -> super::ManagedRuntimeContext {
        load_management_context(
            parse_contract_str(CONTRACT).expect("contract"),
            context("runtime-manager", "default", root),
        )
        .expect("manager")
    }

    #[test]
    fn manager_loads_from_its_context_and_rejects_inactive_profiles() {
        let dir = tempdir().expect("tempdir");
        let manager = manager(dir.path());
        assert_eq!(
            manager.manager_context().service().as_str(),
            "runtime-manager"
        );
        assert!(manager.registry().instances.is_empty());
        assert!(
            manager
                .shared_paths()
                .instance_registry_path()
                .ends_with("services/runtime-manager/default/instances.toml")
        );

        let contract = parse_contract_str(CONTRACT).expect("contract");
        assert_eq!(
            active_management_mode_for_profile(&contract, RadrootsPathProfile::RepoLocal)
                .expect("active mode"),
            "interactive_user_managed"
        );
        assert!(
            active_management_mode_for_profile(&contract, RadrootsPathProfile::ServiceHost)
                .is_err()
        );
    }

    #[test]
    fn targets_bind_exact_typed_contexts_and_multi_instance_records() {
        let dir = tempdir().expect("tempdir");
        let mut manager = manager(dir.path());
        let primary = context("myc", "primary", dir.path());
        let secondary = context("myc", "secondary", dir.path());
        manager
            .register_instance(&primary, ManagedRuntimeInstallState::Configured)
            .expect("register primary");

        let primary_target = resolve_runtime_target(&manager, primary.clone()).expect("primary");
        let secondary_target =
            resolve_runtime_target(&manager, secondary.clone()).expect("secondary");
        assert!(primary_target.instance_record.is_some());
        assert!(secondary_target.instance_record.is_none());
        assert_eq!(primary_target.context, primary);
        assert_eq!(secondary_target.context, secondary);
        assert_ne!(
            primary_target.predicted_paths,
            secondary_target.predicted_paths
        );
        assert_eq!(
            primary_target.runtime_group,
            ManagedRuntimeGroup::DefinedManagedTarget
        );
        assert!(primary_target.predicted_paths.is_some());
    }

    #[test]
    fn manager_rejects_same_identity_from_a_different_root_scope() {
        let first = tempdir().expect("first");
        let second = tempdir().expect("second");
        let mut manager = manager(first.path());
        let mismatched = context("myc", "primary", second.path());

        assert!(matches!(
            manager.register_instance(&mismatched, ManagedRuntimeInstallState::Configured),
            Err(RadrootsRuntimeManagerError::RuntimeContextMismatch)
        ));
        assert!(matches!(
            resolve_runtime_target(&manager, mismatched),
            Err(RadrootsRuntimeManagerError::RuntimeContextMismatch)
        ));
        assert!(manager.registry().instances().is_empty());
    }

    #[test]
    fn groups_and_unknown_targets_remain_contract_controlled() {
        let dir = tempdir().expect("tempdir");
        let manager = manager(dir.path());
        let contract = manager.contract();
        assert_eq!(
            runtime_group(contract, "radrootsd"),
            ManagedRuntimeGroup::ActiveManagedTarget
        );
        assert_eq!(
            runtime_group(contract, "myc"),
            ManagedRuntimeGroup::DefinedManagedTarget
        );
        assert_eq!(
            runtime_group(contract, "hyf"),
            ManagedRuntimeGroup::BootstrapOnly
        );
        assert_eq!(
            runtime_group(contract, "unknown"),
            ManagedRuntimeGroup::Unknown
        );

        let unknown = resolve_runtime_target(&manager, context("unknown", "default", dir.path()))
            .expect("unknown target");
        assert!(unknown.predicted_paths.is_none());
        assert_eq!(
            inspect_runtime_status(&unknown, &[]).availability,
            ManagedRuntimeInspectionAvailability::Unconfigured
        );
    }

    #[test]
    fn status_uses_manager_tracking_without_disclosing_paths() {
        let dir = tempdir().expect("tempdir");
        let mut manager = manager(dir.path());
        let service = context("radrootsd", "local", dir.path());
        manager
            .register_instance(&service, ManagedRuntimeInstallState::Configured)
            .expect("register service");
        let target = resolve_runtime_target(&manager, service).expect("target");
        let paths = target.predicted_paths.as_ref().expect("paths");
        fs::create_dir_all(paths.run_dir()).expect("run dir");
        fs::write(paths.pid_file_path(), std::process::id().to_string()).expect("pid");

        let status = inspect_runtime_status(&target, &["start".to_owned()]);
        assert_eq!(status.view.health_state, "running");
        assert_eq!(status.view.health_source, "process_probe");
        assert_eq!(
            status.view.instance_source,
            RuntimeContextSource::BootstrapCli
        );
        let rendered = format!("{status:?}");
        assert!(!rendered.contains(dir.path().to_string_lossy().as_ref()));
        assert!(!status.view.detail.contains('/'));
    }

    #[test]
    fn log_and_config_inspections_use_manager_and_service_context_paths() {
        let dir = tempdir().expect("tempdir");
        let mut manager = manager(dir.path());
        let service = context("radrootsd", "local", dir.path());
        manager
            .register_instance(&service, ManagedRuntimeInstallState::Configured)
            .expect("register service");
        let target = resolve_runtime_target(&manager, service).expect("target");
        let paths = target.predicted_paths.as_ref().expect("paths");
        fs::create_dir_all(paths.logs_dir()).expect("logs");
        fs::write(paths.stdout_log_path(), "stdout").expect("stdout");
        let config_path = paths.config_path();
        fs::create_dir_all(config_path.parent().expect("config parent")).expect("config parent");
        fs::write(&config_path, "enabled = true").expect("config");

        let logs = inspect_runtime_logs(&target);
        assert!(logs.view.stdout_log_present);
        assert!(!logs.view.stderr_log_present);
        let config = inspect_runtime_config(&target);
        assert!(config.view.config_present);
        assert_eq!(config.view.config_format.as_deref(), Some("toml"));
        for rendered in [format!("{logs:?}"), format!("{config:?}")] {
            assert!(!rendered.contains(dir.path().to_string_lossy().as_ref()));
        }
    }

    #[test]
    fn actions_do_not_mutate_bindings_for_any_group() {
        let dir = tempdir().expect("tempdir");
        let manager = manager(dir.path());
        for (service, expected) in [
            (
                "radrootsd",
                ManagedRuntimeInspectionAvailability::Unsupported,
            ),
            ("myc", ManagedRuntimeInspectionAvailability::Unsupported),
            ("hyf", ManagedRuntimeInspectionAvailability::Unsupported),
            (
                "unknown",
                ManagedRuntimeInspectionAvailability::Unconfigured,
            ),
        ] {
            let target = resolve_runtime_target(&manager, context(service, "default", dir.path()))
                .expect("target");
            let action = inspect_runtime_action(&target, ManagedRuntimeLifecycleAction::ConfigSet);
            assert_eq!(action.availability, expected);
            assert!(!action.view.mutates_bindings);
            assert!(action.view.next_step.is_none());
        }
    }
}
