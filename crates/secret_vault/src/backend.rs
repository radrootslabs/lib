use crate::policy::RadrootsHostVaultPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadrootsSecretBackendKind {
    HostVault,
    EncryptedFile,
    ExternalCommand,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadrootsSecretBackend {
    HostVault(RadrootsHostVaultPolicy),
    EncryptedFile,
    ExternalCommand,
    Memory,
}

impl RadrootsSecretBackend {
    #[must_use]
    pub const fn kind(self) -> RadrootsSecretBackendKind {
        match self {
            Self::HostVault(_) => RadrootsSecretBackendKind::HostVault,
            Self::EncryptedFile => RadrootsSecretBackendKind::EncryptedFile,
            Self::ExternalCommand => RadrootsSecretBackendKind::ExternalCommand,
            Self::Memory => RadrootsSecretBackendKind::Memory,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_kind_maps_all_backend_variants() {
        assert_eq!(
            RadrootsSecretBackend::HostVault(RadrootsHostVaultPolicy::desktop()).kind(),
            RadrootsSecretBackendKind::HostVault
        );
        assert_eq!(
            RadrootsSecretBackend::EncryptedFile.kind(),
            RadrootsSecretBackendKind::EncryptedFile
        );
        assert_eq!(
            RadrootsSecretBackend::ExternalCommand.kind(),
            RadrootsSecretBackendKind::ExternalCommand
        );
        assert_eq!(
            RadrootsSecretBackend::Memory.kind(),
            RadrootsSecretBackendKind::Memory
        );
    }
}
