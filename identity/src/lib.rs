#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod error;
pub mod identity;

pub use error::IdentityError;
pub use identity::{
    RadrootsIdentity, RadrootsIdentityFile, RadrootsIdentityProfile,
    RadrootsIdentitySecretKeyFormat, DEFAULT_IDENTITY_PATH,
};
