#![forbid(unsafe_code)]
#![no_std]

extern crate alloc;
#[cfg(any(feature = "std", test))]
extern crate std;

pub mod backend;
pub mod error;
pub mod policy;
pub mod selection;
#[cfg(feature = "std")]
pub mod vault;
pub mod wrap;

pub mod prelude {
    pub use crate::backend::{RadrootsSecretBackend, RadrootsSecretBackendKind};
    #[cfg(feature = "std")]
    pub use crate::error::RadrootsSecretVaultAccessError;
    pub use crate::error::{RadrootsHostVaultRequirement, RadrootsSecretVaultError};
    pub use crate::policy::{
        RadrootsHostVaultCapabilities, RadrootsHostVaultHardwarePolicy, RadrootsHostVaultPolicy,
        RadrootsHostVaultResidency, RadrootsHostVaultUserPresencePolicy,
    };
    pub use crate::selection::{
        RadrootsResolvedSecretBackend, RadrootsSecretBackendAvailability,
        RadrootsSecretBackendSelection,
    };
    #[cfg(feature = "std")]
    pub use crate::vault::RadrootsSecretVault;
    #[cfg(feature = "memory-vault")]
    pub use crate::vault::RadrootsSecretVaultMemory;
    #[cfg(feature = "os-keyring")]
    pub use crate::vault::RadrootsSecretVaultOsKeyring;
    pub use crate::wrap::RadrootsSecretKeyWrapping;
}

pub use backend::{RadrootsSecretBackend, RadrootsSecretBackendKind};
#[cfg(feature = "std")]
pub use error::RadrootsSecretVaultAccessError;
pub use error::{RadrootsHostVaultRequirement, RadrootsSecretVaultError};
pub use policy::{
    RadrootsHostVaultCapabilities, RadrootsHostVaultHardwarePolicy, RadrootsHostVaultPolicy,
    RadrootsHostVaultResidency, RadrootsHostVaultUserPresencePolicy,
};
pub use selection::{
    RadrootsResolvedSecretBackend, RadrootsSecretBackendAvailability,
    RadrootsSecretBackendSelection,
};
#[cfg(feature = "std")]
pub use vault::RadrootsSecretVault;
#[cfg(feature = "memory-vault")]
pub use vault::RadrootsSecretVaultMemory;
#[cfg(feature = "os-keyring")]
pub use vault::RadrootsSecretVaultOsKeyring;
pub use wrap::RadrootsSecretKeyWrapping;
