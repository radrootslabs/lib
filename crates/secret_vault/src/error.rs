use crate::backend::RadrootsSecretBackendKind;
use alloc::string::String;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSecretVaultAccessError {
    Backend(String),
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

impl fmt::Display for RadrootsSecretVaultAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(message) => write!(f, "secret vault access error: {message}"),
        }
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
        };
        f.write_str(value)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSecretVaultError {}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSecretVaultAccessError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::RadrootsSecretBackendKind;
    use alloc::string::ToString;

    #[test]
    fn display_formats_requirements_backend_kinds_and_errors() {
        assert_eq!(
            RadrootsHostVaultRequirement::DeviceLocalOnly.to_string(),
            "device_local_only"
        );
        assert_eq!(
            RadrootsHostVaultRequirement::UserPresence.to_string(),
            "user_presence"
        );
        assert_eq!(
            RadrootsHostVaultRequirement::HardwareBacked.to_string(),
            "hardware_backed"
        );

        assert_eq!(
            RadrootsSecretBackendKind::HostVault.to_string(),
            "host_vault"
        );
        assert_eq!(
            RadrootsSecretBackendKind::EncryptedFile.to_string(),
            "encrypted_file"
        );
        assert_eq!(
            RadrootsSecretBackendKind::ExternalCommand.to_string(),
            "external_command"
        );
        assert_eq!(RadrootsSecretBackendKind::Memory.to_string(), "memory");

        assert_eq!(
            RadrootsSecretVaultAccessError::Backend("backend offline".into()).to_string(),
            "secret vault access error: backend offline"
        );
        assert_eq!(
            RadrootsSecretVaultError::BackendUnavailable {
                backend: RadrootsSecretBackendKind::HostVault,
            }
            .to_string(),
            "secret backend host_vault is unavailable"
        );
        assert_eq!(
            RadrootsSecretVaultError::FallbackDisallowed {
                primary: RadrootsSecretBackendKind::ExternalCommand,
                fallback: RadrootsSecretBackendKind::EncryptedFile,
            }
            .to_string(),
            "secret backend external_command may not silently downgrade to encrypted_file"
        );
        assert_eq!(
            RadrootsSecretVaultError::FallbackUnavailable {
                primary: RadrootsSecretBackendKind::HostVault,
                fallback: RadrootsSecretBackendKind::EncryptedFile,
            }
            .to_string(),
            "secret backend host_vault fallback encrypted_file is unavailable"
        );
        assert_eq!(
            RadrootsSecretVaultError::HostVaultPolicyUnsupported {
                requirement: RadrootsHostVaultRequirement::HardwareBacked,
            }
            .to_string(),
            "host vault does not satisfy the required hardware_backed policy"
        );
    }
}
