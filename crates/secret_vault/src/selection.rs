use crate::backend::{RadrootsSecretBackend, RadrootsSecretBackendKind};
use crate::error::RadrootsSecretVaultError;
use crate::policy::RadrootsHostVaultCapabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RadrootsSecretBackendSelection {
    pub primary: RadrootsSecretBackend,
    pub fallback: Option<RadrootsSecretBackend>,
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
    pub used_fallback: bool,
}

impl RadrootsSecretBackendSelection {
    pub fn resolve(
        self,
        availability: RadrootsSecretBackendAvailability,
    ) -> Result<RadrootsResolvedSecretBackend, RadrootsSecretVaultError> {
        if availability.supports(self.primary).is_ok() {
            return Ok(RadrootsResolvedSecretBackend {
                backend: self.primary,
                used_fallback: false,
            });
        }

        if let RadrootsSecretBackend::HostVault(policy) = self.primary {
            if availability.host_vault.available {
                availability.host_vault.validate(policy)?;
            }
        }

        match self.fallback {
            Some(fallback) => {
                if !self.primary.allows_fallback_to(fallback.kind()) {
                    return Err(RadrootsSecretVaultError::FallbackDisallowed {
                        primary: self.primary.kind(),
                        fallback: fallback.kind(),
                    });
                }

                availability.supports(fallback).map_err(|_| {
                    RadrootsSecretVaultError::FallbackUnavailable {
                        primary: self.primary.kind(),
                        fallback: fallback.kind(),
                    }
                })?;

                Ok(RadrootsResolvedSecretBackend {
                    backend: fallback,
                    used_fallback: true,
                })
            }
            None => Err(RadrootsSecretVaultError::BackendUnavailable {
                backend: self.primary.kind(),
            }),
        }
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

impl RadrootsSecretBackend {
    const fn allows_fallback_to(self, fallback: RadrootsSecretBackendKind) -> bool {
        matches!(
            (self.kind(), fallback),
            (
                RadrootsSecretBackendKind::HostVault,
                RadrootsSecretBackendKind::EncryptedFile
            )
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::RadrootsHostVaultRequirement;
    use crate::policy::{
        RadrootsHostVaultHardwarePolicy, RadrootsHostVaultPolicy, RadrootsHostVaultResidency,
        RadrootsHostVaultUserPresencePolicy,
    };

    #[test]
    fn host_vault_is_selected_when_available() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::HostVault(RadrootsHostVaultPolicy::desktop()),
            fallback: Some(RadrootsSecretBackend::EncryptedFile),
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
                used_fallback: false,
            }
        );
    }

    #[test]
    fn host_vault_may_explicitly_fallback_to_encrypted_file() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::HostVault(RadrootsHostVaultPolicy::desktop()),
            fallback: Some(RadrootsSecretBackend::EncryptedFile),
        };

        let resolved = selection
            .resolve(RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: true,
                external_command: false,
                memory: false,
            })
            .expect("encrypted file fallback resolves");

        assert_eq!(
            resolved,
            RadrootsResolvedSecretBackend {
                backend: RadrootsSecretBackend::EncryptedFile,
                used_fallback: true,
            }
        );
    }

    #[test]
    fn host_vault_without_explicit_fallback_fails_closed() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::HostVault(RadrootsHostVaultPolicy::desktop()),
            fallback: None,
        };

        let err = selection
            .resolve(RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: true,
                external_command: false,
                memory: false,
            })
            .expect_err("missing fallback must fail");

        assert_eq!(
            err,
            RadrootsSecretVaultError::BackendUnavailable {
                backend: RadrootsSecretBackendKind::HostVault,
            }
        );
    }

    #[test]
    fn unsupported_host_vault_policy_fails_before_any_downgrade() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::HostVault(RadrootsHostVaultPolicy {
                residency: RadrootsHostVaultResidency::DeviceLocalOnly,
                user_presence: RadrootsHostVaultUserPresencePolicy::Required,
                hardware: RadrootsHostVaultHardwarePolicy::RequireHardwareBacked,
            }),
            fallback: Some(RadrootsSecretBackend::EncryptedFile),
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
    fn external_command_may_not_downgrade_to_encrypted_file() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::ExternalCommand,
            fallback: Some(RadrootsSecretBackend::EncryptedFile),
        };

        let err = selection
            .resolve(RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: true,
                external_command: false,
                memory: false,
            })
            .expect_err("external command downgrade must fail");

        assert_eq!(
            err,
            RadrootsSecretVaultError::FallbackDisallowed {
                primary: RadrootsSecretBackendKind::ExternalCommand,
                fallback: RadrootsSecretBackendKind::EncryptedFile,
            }
        );
    }

    #[test]
    fn memory_backend_must_be_selected_explicitly() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::Memory,
            fallback: None,
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
                used_fallback: false,
            }
        );
    }

    #[test]
    fn unavailable_explicit_fallback_reports_fallback_unavailable() {
        let selection = RadrootsSecretBackendSelection {
            primary: RadrootsSecretBackend::HostVault(RadrootsHostVaultPolicy::desktop()),
            fallback: Some(RadrootsSecretBackend::EncryptedFile),
        };

        let err = selection
            .resolve(RadrootsSecretBackendAvailability {
                host_vault: RadrootsHostVaultCapabilities::unavailable(),
                encrypted_file: false,
                external_command: false,
                memory: false,
            })
            .expect_err("unavailable fallback must fail");

        assert_eq!(
            err,
            RadrootsSecretVaultError::FallbackUnavailable {
                primary: RadrootsSecretBackendKind::HostVault,
                fallback: RadrootsSecretBackendKind::EncryptedFile,
            }
        );
    }
}
