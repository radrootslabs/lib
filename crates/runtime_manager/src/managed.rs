use core::fmt;

use radroots_runtime_distribution::HardenedServiceTarget;
use radroots_runtime_paths::{InstanceId, RadrootsPathProfile, RuntimeContext, ServiceId};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use radroots_service_host::AdminTransportLimits;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::ManagedRuntimeStatusClient;
use crate::{
    ManagedCliCommand, ManagedCliInvocation, ManagementModeContract,
    RadrootsRuntimeManagementContract, RadrootsRuntimeManagerError,
};

/// Validated frozen runtime-management contract.
///
/// Instance identity, profile, and canonical paths are deliberately absent;
/// those values enter only through a sealed [`RuntimeContext`] when a target is
/// resolved.
///
/// ```compile_fail
/// use radroots_runtime_manager::ManagedRuntimeContext;
///
/// let _ = ManagedRuntimeContext { contract: todo!() };
/// ```
#[derive(Clone)]
pub struct ManagedRuntimeContext {
    contract: RadrootsRuntimeManagementContract,
}

impl ManagedRuntimeContext {
    pub fn new(
        contract: RadrootsRuntimeManagementContract,
    ) -> Result<Self, RadrootsRuntimeManagerError> {
        crate::validate_hardened_management_contract(&contract)?;
        Ok(Self { contract })
    }

    #[must_use]
    pub fn contract(&self) -> &RadrootsRuntimeManagementContract {
        &self.contract
    }
}

impl fmt::Debug for ManagedRuntimeContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedRuntimeContext")
            .field("schema", &self.contract.schema)
            .field("schema_version", &self.contract.schema_version)
            .finish_non_exhaustive()
    }
}

/// Sealed management capability for one validated Myc or RHI runtime context.
///
/// The target owns the sole service-instance identity/profile/path authority.
/// It exposes typed CLI-v1 and status-v1 integration but no filesystem,
/// process, PID, log, credential, or distribution-artifact mutation surface.
///
/// ```compile_fail
/// use radroots_runtime_manager::ManagedRuntimeTarget;
///
/// let _ = ManagedRuntimeTarget {
///     context: todo!(),
///     service_target: todo!(),
///     management_mode: String::new(),
///     mode_contract: todo!(),
/// };
/// ```
#[derive(Clone)]
pub struct ManagedRuntimeTarget {
    context: RuntimeContext,
    service_target: HardenedServiceTarget,
    management_mode: String,
    mode_contract: ManagementModeContract,
}

impl ManagedRuntimeTarget {
    #[must_use]
    pub fn runtime_context(&self) -> &RuntimeContext {
        &self.context
    }

    #[must_use]
    pub fn service_id(&self) -> &ServiceId {
        self.context.service()
    }

    #[must_use]
    pub fn instance_id(&self) -> &InstanceId {
        self.context.instance()
    }

    #[must_use]
    pub fn profile(&self) -> RadrootsPathProfile {
        self.context.profile()
    }

    #[must_use]
    pub fn service_target(&self) -> &HardenedServiceTarget {
        &self.service_target
    }

    #[must_use]
    pub fn management_mode(&self) -> &str {
        &self.management_mode
    }

    #[must_use]
    pub fn mode_contract(&self) -> &ManagementModeContract {
        &self.mode_contract
    }

    pub fn cli_invocation(
        &self,
        command: ManagedCliCommand,
    ) -> Result<ManagedCliInvocation, RadrootsRuntimeManagerError> {
        ManagedCliInvocation::for_context(&self.context, command)
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub fn status_client(
        &self,
        limits: AdminTransportLimits,
    ) -> Result<ManagedRuntimeStatusClient, RadrootsRuntimeManagerError> {
        ManagedRuntimeStatusClient::for_context(&self.context, limits)
    }
}

impl fmt::Debug for ManagedRuntimeTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedRuntimeTarget")
            .field("service_id", self.context.service())
            .field("instance_id", self.context.instance())
            .field("profile", &self.context.profile())
            .field("management_mode", &self.management_mode)
            .field("paths", &"[redacted]")
            .finish_non_exhaustive()
    }
}

pub fn resolve_runtime_target(
    context: &ManagedRuntimeContext,
    runtime_context: RuntimeContext,
) -> Result<ManagedRuntimeTarget, RadrootsRuntimeManagerError> {
    let service_target = context
        .contract
        .service_targets
        .get(runtime_context.service())
        .cloned()
        .ok_or(RadrootsRuntimeManagerError::UnsupportedServiceTarget)?;
    let management_mode =
        active_management_mode_for_profile(&context.contract, runtime_context.profile())?;
    let mode_contract = context
        .contract
        .mode
        .get(management_mode)
        .cloned()
        .ok_or(RadrootsRuntimeManagerError::InvalidContract)?;
    Ok(ManagedRuntimeTarget {
        context: runtime_context,
        service_target,
        management_mode: management_mode.to_owned(),
        mode_contract,
    })
}

fn active_management_mode_for_profile(
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use radroots_runtime_paths::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
    };
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use radroots_service_host::AdminTransportLimits;

    use super::{
        ManagedRuntimeContext, active_management_mode_for_profile, resolve_runtime_target,
    };
    use crate::{
        HARDENED_MANAGEMENT_CONTRACT, ManagedCliCommand, RadrootsRuntimeManagerError,
        parse_contract_str,
    };

    fn management_context() -> ManagedRuntimeContext {
        ManagedRuntimeContext::new(
            parse_contract_str(HARDENED_MANAGEMENT_CONTRACT).expect("contract"),
        )
        .expect("management context")
    }

    fn runtime_context(
        profile: RadrootsPathProfile,
        service: &str,
        instance: &str,
    ) -> RuntimeContext {
        let root = (profile == RadrootsPathProfile::RepoLocal)
            .then(|| PathBuf::from("/sensitive/project-root"));
        RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                profile,
                root,
                if profile == RadrootsPathProfile::RepoLocal {
                    RuntimeContextSource::BootstrapCli
                } else {
                    RuntimeContextSource::SafeDefault
                },
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new(service).expect("service"),
            InstanceId::new(instance).expect("instance"),
        )
        .expect("runtime context")
    }

    #[test]
    fn context_accepts_only_the_exact_contract() {
        let context = management_context();
        assert_eq!(context.contract().service_targets.len(), 2);

        let mut direct = parse_contract_str(HARDENED_MANAGEMENT_CONTRACT).expect("contract");
        direct.lifecycle.actions.push("start".to_owned());
        assert!(matches!(
            ManagedRuntimeContext::new(direct),
            Err(RadrootsRuntimeManagerError::InvalidContract)
        ));
    }

    #[test]
    fn exact_myc_and_rhi_contexts_resolve_without_identity_duplication() {
        let management = management_context();
        let myc = resolve_runtime_target(
            &management,
            runtime_context(RadrootsPathProfile::RepoLocal, "myc", "primary"),
        )
        .expect("myc target");
        let rhi = resolve_runtime_target(
            &management,
            runtime_context(RadrootsPathProfile::ServiceHost, "rhi", "secondary"),
        )
        .expect("rhi target");

        assert_eq!(myc.service_id().as_str(), "myc");
        assert_eq!(myc.instance_id().as_str(), "primary");
        assert_eq!(myc.profile(), RadrootsPathProfile::RepoLocal);
        assert_eq!(myc.service_target().service_id(), myc.service_id());
        assert_eq!(myc.management_mode(), "interactive_user_managed");
        assert!(!myc.mode_contract().service_manager_integration);
        let invocation = myc
            .cli_invocation(ManagedCliCommand::Doctor)
            .expect("Myc doctor invocation");
        assert_eq!(invocation.profile(), RadrootsPathProfile::RepoLocal);
        assert_eq!(invocation.command(), ManagedCliCommand::Doctor);

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let status_client = myc
                .status_client(AdminTransportLimits::DEFAULT)
                .expect("Myc status client");
            assert!(!format!("{status_client:?}").contains("sensitive"));
        }

        assert_eq!(rhi.service_id().as_str(), "rhi");
        assert_eq!(rhi.instance_id().as_str(), "secondary");
        assert_eq!(rhi.management_mode(), "service_host_managed");
        assert!(rhi.mode_contract().service_manager_integration);
        assert_eq!(rhi.runtime_context().service(), rhi.service_id());

        for rendered in [
            format!("{management:?}"),
            format!("{myc:?}"),
            format!("{rhi:?}"),
        ] {
            assert!(!rendered.contains("sensitive"));
            assert!(!rendered.contains("state.sqlite"));
            assert!(!rendered.contains("admin.sock"));
        }
    }

    #[test]
    fn unsupported_service_and_profile_fail_without_fallback() {
        let management = management_context();
        let unsupported = runtime_context(RadrootsPathProfile::ServiceHost, "radrootsd", "default");
        assert!(matches!(
            resolve_runtime_target(&management, unsupported),
            Err(RadrootsRuntimeManagerError::UnsupportedServiceTarget)
        ));

        let mobile = RuntimeContextBootstrap::new(
            RadrootsPathProfile::MobileNative,
            None,
            RuntimeContextSource::SafeDefault,
            RuntimeContextSource::BootstrapCli,
        );
        assert!(mobile.is_err());
    }

    #[test]
    fn inactive_management_mode_never_matches_a_supported_profile() {
        let mut contract = parse_contract_str(HARDENED_MANAGEMENT_CONTRACT).expect("contract");
        contract
            .mode
            .get_mut("interactive_user_managed")
            .expect("interactive mode")
            .contract_state = "inactive".to_owned();

        assert_eq!(
            active_management_mode_for_profile(&contract, RadrootsPathProfile::RepoLocal),
            Err(RadrootsRuntimeManagerError::UnsupportedProfile)
        );
    }

    #[test]
    fn production_manager_has_no_direct_io_process_or_artifact_authority() {
        let source = include_str!("managed.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("production source");
        for forbidden in [
            "std::fs",
            "std::process",
            "load_registry",
            "save_registry",
            "register_instance",
            "remove_instance",
            "ManagedRuntimeInstancePaths",
            "ManagedRuntimeArtifactName",
            "inspect_runtime_",
            "start_process",
            "stop_process",
            "extract_binary_archive",
        ] {
            assert!(
                !source.contains(forbidden),
                "manager retained `{forbidden}`"
            );
        }
    }
}
