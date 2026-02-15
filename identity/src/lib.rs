#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod error;
pub mod identity;
pub mod username;

pub use error::IdentityError;
pub use identity::{
    DEFAULT_IDENTITY_PATH, RadrootsIdentity, RadrootsIdentityFile, RadrootsIdentityProfile,
    RadrootsIdentitySecretKeyFormat,
};
pub use username::{
    RADROOTS_USERNAME_MAX_LEN, RADROOTS_USERNAME_MIN_LEN, RADROOTS_USERNAME_REGEX,
    radroots_username_is_valid, radroots_username_normalize,
};
