use crate::error::{RadrootsHostVaultRequirement, RadrootsSecretVaultError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadrootsHostVaultResidency {
    UserProfile,
    DeviceLocalOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadrootsHostVaultUserPresencePolicy {
    NotRequired,
    Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadrootsHostVaultHardwarePolicy {
    Any,
    PreferHardwareBacked,
    RequireHardwareBacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RadrootsHostVaultPolicy {
    pub residency: RadrootsHostVaultResidency,
    pub user_presence: RadrootsHostVaultUserPresencePolicy,
    pub hardware: RadrootsHostVaultHardwarePolicy,
}

impl RadrootsHostVaultPolicy {
    #[must_use]
    pub const fn desktop() -> Self {
        Self {
            residency: RadrootsHostVaultResidency::UserProfile,
            user_presence: RadrootsHostVaultUserPresencePolicy::NotRequired,
            hardware: RadrootsHostVaultHardwarePolicy::Any,
        }
    }

    #[must_use]
    pub const fn device_local() -> Self {
        Self {
            residency: RadrootsHostVaultResidency::DeviceLocalOnly,
            user_presence: RadrootsHostVaultUserPresencePolicy::NotRequired,
            hardware: RadrootsHostVaultHardwarePolicy::Any,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RadrootsHostVaultCapabilities {
    pub available: bool,
    pub supports_device_local_only: bool,
    pub supports_user_presence: bool,
    pub supports_hardware_backed: bool,
}

impl RadrootsHostVaultCapabilities {
    #[must_use]
    pub const fn unavailable() -> Self {
        Self {
            available: false,
            supports_device_local_only: false,
            supports_user_presence: false,
            supports_hardware_backed: false,
        }
    }

    #[must_use]
    pub const fn desktop_keyring() -> Self {
        Self {
            available: true,
            supports_device_local_only: false,
            supports_user_presence: false,
            supports_hardware_backed: false,
        }
    }

    #[must_use]
    pub const fn secure_device() -> Self {
        Self {
            available: true,
            supports_device_local_only: true,
            supports_user_presence: true,
            supports_hardware_backed: true,
        }
    }

    pub const fn validate(
        self,
        policy: RadrootsHostVaultPolicy,
    ) -> Result<(), RadrootsSecretVaultError> {
        if !self.available {
            return Err(RadrootsSecretVaultError::BackendUnavailable {
                backend: crate::backend::RadrootsSecretBackendKind::HostVault,
            });
        }

        if matches!(
            policy.residency,
            RadrootsHostVaultResidency::DeviceLocalOnly
        ) && !self.supports_device_local_only
        {
            return Err(RadrootsSecretVaultError::HostVaultPolicyUnsupported {
                requirement: RadrootsHostVaultRequirement::DeviceLocalOnly,
            });
        }

        if matches!(
            policy.user_presence,
            RadrootsHostVaultUserPresencePolicy::Required
        ) && !self.supports_user_presence
        {
            return Err(RadrootsSecretVaultError::HostVaultPolicyUnsupported {
                requirement: RadrootsHostVaultRequirement::UserPresence,
            });
        }

        if matches!(
            policy.hardware,
            RadrootsHostVaultHardwarePolicy::RequireHardwareBacked
        ) && !self.supports_hardware_backed
        {
            return Err(RadrootsSecretVaultError::HostVaultPolicyUnsupported {
                requirement: RadrootsHostVaultRequirement::HardwareBacked,
            });
        }

        Ok(())
    }
}
