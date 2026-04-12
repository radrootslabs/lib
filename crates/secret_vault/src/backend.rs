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
