use crate::backend::RadrootsSecretBackendKind;
use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadrootsHostVaultRequirement {
    DeviceLocalOnly,
    UserPresence,
    HardwareBacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadrootsSecretVaultError {
    BackendUnavailable {
        backend: RadrootsSecretBackendKind,
    },
    FallbackDisallowed {
        primary: RadrootsSecretBackendKind,
        fallback: RadrootsSecretBackendKind,
    },
    FallbackUnavailable {
        primary: RadrootsSecretBackendKind,
        fallback: RadrootsSecretBackendKind,
    },
    HostVaultPolicyUnsupported {
        requirement: RadrootsHostVaultRequirement,
    },
}

impl fmt::Display for RadrootsHostVaultRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::DeviceLocalOnly => "device_local_only",
            Self::UserPresence => "user_presence",
            Self::HardwareBacked => "hardware_backed",
        };
        f.write_str(value)
    }
}

impl fmt::Display for RadrootsSecretVaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendUnavailable { backend } => {
                write!(f, "secret backend {backend} is unavailable")
            }
            Self::FallbackDisallowed { primary, fallback } => write!(
                f,
                "secret backend {primary} may not silently downgrade to {fallback}"
            ),
            Self::FallbackUnavailable { primary, fallback } => write!(
                f,
                "secret backend {primary} fallback {fallback} is unavailable"
            ),
            Self::HostVaultPolicyUnsupported { requirement } => write!(
                f,
                "host vault does not satisfy the required {requirement} policy"
            ),
        }
    }
}

impl fmt::Display for RadrootsSecretBackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::HostVault => "host_vault",
            Self::EncryptedFile => "encrypted_file",
            Self::ExternalCommand => "external_command",
            Self::Memory => "memory",
            Self::PlaintextFile => "plaintext_file",
        };
        f.write_str(value)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSecretVaultError {}
