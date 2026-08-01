//! Secret-provider contracts and capability selection.

use crate::error::{Error, PolicyRequirement};
use crate::id::BackendKind;
use crate::wrapping::KeyWrapping;

/// Secret residency required by a host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResidencyPolicy {
    /// The provider may use its normal host-selected residency.
    Any,
    /// The provider must keep material local to the current device.
    DeviceLocal,
}

/// User-presence behavior required by a host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UserPresencePolicy {
    /// User presence is not required for the operation.
    NotRequired,
    /// The provider must require user presence.
    Required,
}

/// Hardware-backed behavior requested by a host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HardwarePolicy {
    /// Hardware-backed protection is not required.
    Any,
    /// Prefer hardware-backed protection when available.
    PreferHardwareBacked,
    /// Hardware-backed protection is mandatory.
    RequireHardwareBacked,
}

/// Provider residency support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResidencySupport {
    /// Volatile process-local storage.
    Volatile,
    /// Persistent storage associated with the host user profile.
    UserProfile,
    /// Persistent storage restricted to the current device.
    DeviceLocal,
}

/// Whether a provider supports an optional security property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CapabilitySupport {
    /// The property is unavailable.
    Unavailable,
    /// The property is supported.
    Supported,
}

/// Explicit host security requirements for provider selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessPolicy {
    residency: ResidencyPolicy,
    user_presence: UserPresencePolicy,
    hardware: HardwarePolicy,
}

impl AccessPolicy {
    /// Creates an explicit access policy.
    #[must_use]
    pub const fn new(
        residency: ResidencyPolicy,
        user_presence: UserPresencePolicy,
        hardware: HardwarePolicy,
    ) -> Self {
        Self {
            residency,
            user_presence,
            hardware,
        }
    }

    /// Returns a policy suitable for an explicitly selected local adapter.
    #[must_use]
    pub const fn standard() -> Self {
        Self::new(
            ResidencyPolicy::Any,
            UserPresencePolicy::NotRequired,
            HardwarePolicy::Any,
        )
    }
}

/// Security properties reported by a provider without performing access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretCapabilities {
    available: bool,
    residency: ResidencySupport,
    user_presence: CapabilitySupport,
    hardware_backed: CapabilitySupport,
}

impl SecretCapabilities {
    /// Reports an unavailable provider without probing or mutating it.
    #[must_use]
    pub const fn unavailable() -> Self {
        Self {
            available: false,
            residency: ResidencySupport::Volatile,
            user_presence: CapabilitySupport::Unavailable,
            hardware_backed: CapabilitySupport::Unavailable,
        }
    }

    /// Reports the static security properties of an available provider.
    #[must_use]
    pub const fn available(
        residency: ResidencySupport,
        user_presence: CapabilitySupport,
        hardware_backed: CapabilitySupport,
    ) -> Self {
        Self {
            available: true,
            residency,
            user_presence,
            hardware_backed,
        }
    }

    /// Returns whether the provider is available for explicit selection.
    #[must_use]
    pub const fn is_available(self) -> bool {
        self.available
    }

    /// Returns the strongest residency guarantee reported by the provider.
    #[must_use]
    pub const fn residency(self) -> ResidencySupport {
        self.residency
    }

    /// Returns user-presence support.
    #[must_use]
    pub const fn user_presence(self) -> CapabilitySupport {
        self.user_presence
    }

    /// Returns hardware-backed protection support.
    #[must_use]
    pub const fn hardware_backed(self) -> CapabilitySupport {
        self.hardware_backed
    }

    fn validate(self, backend: BackendKind, policy: AccessPolicy) -> Result<(), Error> {
        if !self.available {
            return Err(Error::BackendUnavailable { backend });
        }
        if matches!(policy.residency, ResidencyPolicy::DeviceLocal)
            && !matches!(self.residency, ResidencySupport::DeviceLocal)
        {
            return Err(Error::PolicyUnsupported {
                backend,
                requirement: PolicyRequirement::DeviceLocal,
            });
        }
        if matches!(policy.user_presence, UserPresencePolicy::Required)
            && !matches!(self.user_presence, CapabilitySupport::Supported)
        {
            return Err(Error::PolicyUnsupported {
                backend,
                requirement: PolicyRequirement::UserPresence,
            });
        }
        if matches!(policy.hardware, HardwarePolicy::RequireHardwareBacked)
            && !matches!(self.hardware_backed, CapabilitySupport::Supported)
        {
            return Err(Error::PolicyUnsupported {
                backend,
                requirement: PolicyRequirement::HardwareBacked,
            });
        }
        Ok(())
    }
}

/// A wrapping provider selected and owned by the host.
pub trait SecretProvider: KeyWrapping + Send + Sync {
    /// Returns the adapter family implemented by this provider.
    fn backend_kind(&self) -> BackendKind;

    /// Reports capabilities without accessing secret storage.
    fn capabilities(&self) -> SecretCapabilities;
}

/// Exact provider selection with no implicit fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionPolicy {
    backend: BackendKind,
    access: AccessPolicy,
}

impl SelectionPolicy {
    /// Selects one backend family and its mandatory security properties.
    #[must_use]
    pub const fn new(backend: BackendKind, access: AccessPolicy) -> Self {
        Self { backend, access }
    }

    /// Resolves the exact provider without probing a fallback backend.
    pub fn select<'a>(
        self,
        candidates: &'a [&'a dyn SecretProvider],
    ) -> Result<&'a dyn SecretProvider, Error> {
        let provider = candidates
            .iter()
            .copied()
            .find(|candidate| candidate.backend_kind() == self.backend)
            .ok_or(Error::BackendUnavailable {
                backend: self.backend,
            })?;
        provider
            .capabilities()
            .validate(self.backend, self.access)?;
        Ok(provider)
    }
}
