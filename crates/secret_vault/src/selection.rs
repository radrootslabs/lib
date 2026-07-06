use crate::backend::RadrootsSecretBackend;
use crate::error::RadrootsSecretVaultError;
use crate::policy::RadrootsHostVaultCapabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RadrootsSecretBackendSelection {
    pub primary: RadrootsSecretBackend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RadrootsSecretBackendAvailability {
    pub host_vault: RadrootsHostVaultCapabilities,
    pub encrypted_file: bool,
    pub external_command: bool,
    pub memory: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RadrootsResolvedSecretBackend {
    pub backend: RadrootsSecretBackend,
}

impl RadrootsSecretBackendSelection {
    pub fn resolve(
        self,
        availability: RadrootsSecretBackendAvailability,
    ) -> Result<RadrootsResolvedSecretBackend, RadrootsSecretVaultError> {
        availability.supports(self.primary)?;
        Ok(RadrootsResolvedSecretBackend {
            backend: self.primary,
        })
    }
}

impl RadrootsSecretBackendAvailability {
    fn supports(self, backend: RadrootsSecretBackend) -> Result<(), RadrootsSecretVaultError> {
        match backend {
            RadrootsSecretBackend::HostVault(policy) => self.host_vault.validate(policy),
            RadrootsSecretBackend::EncryptedFile if self.encrypted_file => Ok(()),
            RadrootsSecretBackend::ExternalCommand if self.external_command => Ok(()),
            RadrootsSecretBackend::Memory if self.memory => Ok(()),
            _ => Err(RadrootsSecretVaultError::BackendUnavailable {
                backend: backend.kind(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::RadrootsSecretBackendKind;
    use crate::error::RadrootsHostVaultRequirement;
    use crate::policy::{
        RadrootsHostVaultHardwarePolicy, RadrootsHostVaultPolicy, RadrootsHostVaultResidency,
        RadrootsHostVaultUserPresencePolicy,
    };

    #[test]
    fn host_vault_is_selected_when_available() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::HostVault(RadrootsHostVaultPolicy::desktop()),
        };

        let resolved = selection
            .resolve(RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::desktop_keyring(),
                encrypted_file: true,
                external_command: false,
                memory: false,
            })
            .expect("host vault resolves");

        assert_eq!(
            resolved,
            RadrootsResolvedSecretBackend {
                backend: RadrootsSecretBackend::HostVault(RadrootsHostVaultPolicy::desktop()),
            }
        );
    }

    #[test]
    fn host_vault_unavailable_fails_closed() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::HostVault(RadrootsHostVaultPolicy::desktop()),
        };

        let err = selection
            .resolve(RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: true,
                external_command: false,
                memory: false,
            })
            .expect_err("unavailable primary must fail");

        assert_eq!(
            err,
            RadrootsSecretVaultError::BackendUnavailable {
                backend: RadrootsSecretBackendKind::HostVault,
            }
        );
    }

    #[test]
    fn encrypted_file_unavailable_fails_closed() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::EncryptedFile,
        };

        let err = selection
            .resolve(RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: false,
                external_command: false,
                memory: false,
            })
            .expect_err("unavailable primary must fail");

        assert_eq!(
            err,
            RadrootsSecretVaultError::BackendUnavailable {
                backend: RadrootsSecretBackendKind::EncryptedFile,
            }
        );
    }

    #[test]
    fn encrypted_file_resolves_when_available() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::EncryptedFile,
        };

        let resolved = selection
            .resolve(RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: true,
                external_command: false,
                memory: false,
            })
            .expect("encrypted file resolves");

        assert_eq!(
            resolved,
            RadrootsResolvedSecretBackend {
                backend: RadrootsSecretBackend::EncryptedFile,
            }
        );
    }

    #[test]
    fn unsupported_host_vault_policy_fails_closed() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::HostVault(RadrootsHostVaultPolicy {
                residency: RadrootsHostVaultResidency::DeviceLocalOnly,
                user_presence: RadrootsHostVaultUserPresencePolicy::Required,
                hardware: RadrootsHostVaultHardwarePolicy::RequireHardwareBacked,
            }),
        };

        let err = selection
            .resolve(RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::desktop_keyring(),
                encrypted_file: true,
                external_command: false,
                memory: false,
            })
            .expect_err("unsupported host policy must fail");

        assert_eq!(
            err,
            RadrootsSecretVaultError::HostVaultPolicyUnsupported {
                requirement: RadrootsHostVaultRequirement::DeviceLocalOnly,
            }
        );
    }

    #[test]
    fn external_command_unavailable_fails_closed() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::ExternalCommand,
        };

        let err = selection
            .resolve(RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: true,
                external_command: false,
                memory: false,
            })
            .expect_err("unavailable primary must fail");

        assert_eq!(
            err,
            RadrootsSecretVaultError::BackendUnavailable {
                backend: RadrootsSecretBackendKind::ExternalCommand,
            }
        );
    }

    #[test]
    fn external_command_resolves_when_available() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::ExternalCommand,
        };

        let resolved = selection
            .resolve(RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: false,
                external_command: true,
                memory: false,
            })
            .expect("external command resolves");

        assert_eq!(
            resolved,
            RadrootsResolvedSecretBackend {
                backend: RadrootsSecretBackend::ExternalCommand,
            }
        );
    }

    #[test]
    fn memory_backend_must_be_selected_explicitly() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::Memory,
        };

        let resolved = selection
            .resolve(RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: false,
                external_command: false,
                memory: true,
            })
            .expect("memory backend resolves");

        assert_eq!(
            resolved,
            RadrootsResolvedSecretBackend {
                backend: RadrootsSecretBackend::Memory,
            }
        );

        let err = selection
            .resolve(RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: false,
                external_command: false,
                memory: false,
            })
            .expect_err("unavailable memory backend must fail");

        assert_eq!(
            err,
            RadrootsSecretVaultError::BackendUnavailable {
                backend: RadrootsSecretBackendKind::Memory,
            }
        );
    }
}
