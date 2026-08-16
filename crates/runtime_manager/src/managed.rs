use core::fmt;

use radroots_runtime_distribution::HardenedServiceTarget;
use radroots_runtime_paths::{InstanceId, RadrootsPathProfile, ServiceId};

use crate::{
    ManagementModeContract, RadrootsRuntimeManagementContract, RadrootsRuntimeManagerError,
};

/// Validated metadata-only runtime-management context.
///
/// The context owns no filesystem path, registry, process, artifact, or
/// lifecycle capability. Its fields are private so a caller cannot bypass the
/// exact contract validation performed by [`ManagedRuntimeContext::new`].
///
/// ```compile_fail
/// use radroots_runtime_manager::ManagedRuntimeContext;
///
/// let _ = ManagedRuntimeContext {
///     contract: todo!(),
///     profile: todo!(),
///     management_mode: String::new(),
/// };
/// ```
#[derive(Clone)]
pub struct ManagedRuntimeContext {
    contract: RadrootsRuntimeManagementContract,
    profile: RadrootsPathProfile,
    management_mode: String,
}

impl ManagedRuntimeContext {
    pub fn new(
        contract: RadrootsRuntimeManagementContract,
        profile: RadrootsPathProfile,
    ) -> Result<Self, RadrootsRuntimeManagerError> {
        crate::validate_hardened_management_contract(&contract)?;
        let management_mode = active_management_mode_for_profile(&contract, profile)?.to_owned();
        Ok(Self {
            contract,
            profile,
            management_mode,
        })
    }

    #[must_use]
    pub fn contract(&self) -> &RadrootsRuntimeManagementContract {
        &self.contract
    }

    #[must_use]
    pub const fn profile(&self) -> RadrootsPathProfile {
        self.profile
    }

    #[must_use]
    pub fn management_mode(&self) -> &str {
        &self.management_mode
    }
}

impl fmt::Debug for ManagedRuntimeContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedRuntimeContext")
            .field("profile", &self.profile)
            .field("management_mode", &self.management_mode)
            .finish_non_exhaustive()
    }
}

/// Sealed metadata for one explicitly selected Myc or RHI instance.
///
/// The target deliberately carries no resolved runtime paths and exposes no
/// lifecycle, registry, filesystem, process, or artifact capability.
///
/// ```compile_fail
/// use radroots_runtime_manager::ManagedRuntimeTarget;
///
/// let _ = ManagedRuntimeTarget {
///     service_id: todo!(),
///     instance_id: todo!(),
///     profile: todo!(),
///     service_target: todo!(),
///     management_mode: String::new(),
///     mode_contract: todo!(),
/// };
/// ```
#[derive(Clone)]
pub struct ManagedRuntimeTarget {
    service_id: ServiceId,
    instance_id: InstanceId,
    profile: RadrootsPathProfile,
    service_target: HardenedServiceTarget,
    management_mode: String,
    mode_contract: ManagementModeContract,
}

impl ManagedRuntimeTarget {
    #[must_use]
    pub fn service_id(&self) -> &ServiceId {
        &self.service_id
    }

    #[must_use]
    pub fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    #[must_use]
    pub const fn profile(&self) -> RadrootsPathProfile {
        self.profile
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
}

impl fmt::Debug for ManagedRuntimeTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedRuntimeTarget")
            .field("service_id", &self.service_id)
            .field("instance_id", &self.instance_id)
            .field("profile", &self.profile)
            .field("management_mode", &self.management_mode)
            .finish_non_exhaustive()
    }
}

pub fn resolve_runtime_target(
    context: &ManagedRuntimeContext,
    service_id: ServiceId,
    instance_id: InstanceId,
) -> Result<ManagedRuntimeTarget, RadrootsRuntimeManagerError> {
    let service_target = context
        .contract
        .service_targets
        .get(&service_id)
        .cloned()
        .ok_or(RadrootsRuntimeManagerError::UnsupportedServiceTarget)?;
    let mode_contract = context
        .contract
        .mode
        .get(&context.management_mode)
        .cloned()
        .ok_or(RadrootsRuntimeManagerError::InvalidContract)?;

    Ok(ManagedRuntimeTarget {
        service_id,
        instance_id,
        profile: context.profile,
        service_target,
        management_mode: context.management_mode.clone(),
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
    use radroots_runtime_paths::{InstanceId, RadrootsPathProfile, ServiceId};

    use super::{ManagedRuntimeContext, resolve_runtime_target};
    use crate::{HARDENED_MANAGEMENT_CONTRACT, RadrootsRuntimeManagerError, parse_contract_str};

    fn context(profile: RadrootsPathProfile) -> ManagedRuntimeContext {
        ManagedRuntimeContext::new(
            parse_contract_str(HARDENED_MANAGEMENT_CONTRACT).expect("contract"),
            profile,
        )
        .expect("management context")
    }

    fn target(
        context: &ManagedRuntimeContext,
        service: &str,
        instance: &str,
    ) -> super::ManagedRuntimeTarget {
        resolve_runtime_target(
            context,
            ServiceId::new(service).expect("service"),
            InstanceId::new(instance).expect("instance"),
        )
        .expect("target")
    }

    #[test]
    fn contexts_accept_only_exact_contracts_and_supported_profiles() {
        for (profile, mode) in [
            (RadrootsPathProfile::RepoLocal, "interactive_user_managed"),
            (RadrootsPathProfile::ServiceHost, "service_host_managed"),
        ] {
            let context = context(profile);
            assert_eq!(context.profile(), profile);
            assert_eq!(context.management_mode(), mode);
            assert_eq!(context.contract().service_targets.len(), 2);
        }

        for profile in [
            RadrootsPathProfile::InteractiveUser,
            RadrootsPathProfile::MobileNative,
        ] {
            let contract = parse_contract_str(HARDENED_MANAGEMENT_CONTRACT).expect("contract");
            assert!(matches!(
                ManagedRuntimeContext::new(contract, profile),
                Err(RadrootsRuntimeManagerError::UnsupportedProfile)
            ));
        }

        let mut direct = parse_contract_str(HARDENED_MANAGEMENT_CONTRACT).expect("contract");
        direct.lifecycle.actions.push("start".to_owned());
        assert!(matches!(
            ManagedRuntimeContext::new(direct, RadrootsPathProfile::RepoLocal),
            Err(RadrootsRuntimeManagerError::InvalidContract)
        ));
    }

    #[test]
    fn exact_myc_and_rhi_instances_resolve_to_static_metadata_only() {
        let context = context(RadrootsPathProfile::RepoLocal);
        let myc = target(&context, "myc", "primary");
        let rhi = target(&context, "rhi", "secondary");

        assert_eq!(myc.service_id().as_str(), "myc");
        assert_eq!(myc.instance_id().as_str(), "primary");
        assert_eq!(myc.profile(), RadrootsPathProfile::RepoLocal);
        assert_eq!(myc.service_target().service_id(), myc.service_id());
        assert_eq!(myc.management_mode(), "interactive_user_managed");
        assert!(!myc.mode_contract().service_manager_integration);

        assert_eq!(rhi.service_id().as_str(), "rhi");
        assert_eq!(rhi.instance_id().as_str(), "secondary");
        assert_eq!(rhi.service_target().service_id(), rhi.service_id());

        for rendered in [
            format!("{context:?}"),
            format!("{myc:?}"),
            format!("{rhi:?}"),
        ] {
            assert!(!rendered.contains('/'));
            assert!(!rendered.contains("state.sqlite"));
            assert!(!rendered.contains("instances.toml"));
        }
    }

    #[test]
    fn unsupported_service_ids_fail_without_fallback_or_effects() {
        let context = context(RadrootsPathProfile::ServiceHost);
        let error = resolve_runtime_target(
            &context,
            ServiceId::new("radrootsd").expect("service"),
            InstanceId::new("default").expect("instance"),
        )
        .expect_err("unsupported target");
        assert_eq!(error, RadrootsRuntimeManagerError::UnsupportedServiceTarget);
    }

    #[test]
    fn production_manager_is_metadata_only_and_contains_no_io_authority() {
        let source = include_str!("managed.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("production source");
        for forbidden in [
            "std::fs",
            "std::process",
            "std::path",
            "radroots_runtime_paths::RuntimeContext",
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
                "metadata-only manager retained `{forbidden}`"
            );
        }
    }
}
