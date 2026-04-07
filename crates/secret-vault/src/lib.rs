#![forbid(unsafe_code)]
#![no_std]

#[cfg(any(feature = "std", test))]
extern crate std;

pub mod backend;
pub mod error;
pub mod policy;
pub mod selection;

pub mod prelude {
    pub use crate::backend::{RadrootsSecretBackend, RadrootsSecretBackendKind};
    pub use crate::error::{RadrootsHostVaultRequirement, RadrootsSecretVaultError};
    pub use crate::policy::{
        RadrootsHostVaultCapabilities, RadrootsHostVaultHardwarePolicy, RadrootsHostVaultPolicy,
        RadrootsHostVaultResidency, RadrootsHostVaultUserPresencePolicy,
    };
    pub use crate::selection::{
        RadrootsResolvedSecretBackend, RadrootsSecretBackendAvailability,
        RadrootsSecretBackendSelection,
    };
}

pub use backend::{RadrootsSecretBackend, RadrootsSecretBackendKind};
pub use error::{RadrootsHostVaultRequirement, RadrootsSecretVaultError};
pub use policy::{
    RadrootsHostVaultCapabilities, RadrootsHostVaultHardwarePolicy, RadrootsHostVaultPolicy,
    RadrootsHostVaultResidency, RadrootsHostVaultUserPresencePolicy,
};
pub use selection::{
    RadrootsResolvedSecretBackend, RadrootsSecretBackendAvailability,
    RadrootsSecretBackendSelection,
};
