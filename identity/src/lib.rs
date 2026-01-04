#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod error;
pub mod identity;
pub mod username;

pub use error::IdentityError;
pub use identity::{
    RadrootsIdentity, RadrootsIdentityFile, RadrootsIdentityProfile,
    RadrootsIdentitySecretKeyFormat, DEFAULT_IDENTITY_PATH,
};
pub use username::{
    radroots_username_is_valid, radroots_username_normalize, RADROOTS_USERNAME_MAX_LEN,
    RADROOTS_USERNAME_MIN_LEN, RADROOTS_USERNAME_REGEX,
};
