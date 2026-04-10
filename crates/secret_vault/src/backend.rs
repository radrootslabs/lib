use crate::policy::RadrootsHostVaultPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadrootsSecretBackendKind {
    HostVault,
    EncryptedFile,
    ExternalCommand,
    Memory,
    PlaintextFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadrootsSecretBackend {
    HostVault(RadrootsHostVaultPolicy),
    EncryptedFile,
    ExternalCommand,
    Memory,
    PlaintextFile,
}

impl RadrootsSecretBackend {
    #[must_use]
    pub const fn kind(self) -> RadrootsSecretBackendKind {
        match self {
            Self::HostVault(_) => RadrootsSecretBackendKind::HostVault,
            Self::EncryptedFile => RadrootsSecretBackendKind::EncryptedFile,
            Self::ExternalCommand => RadrootsSecretBackendKind::ExternalCommand,
            Self::Memory => RadrootsSecretBackendKind::Memory,
            Self::PlaintextFile => RadrootsSecretBackendKind::PlaintextFile,
        }
    }
}
