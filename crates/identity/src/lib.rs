#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod error;
pub mod identity;
#[cfg(feature = "std")]
pub mod storage;
pub mod username;

pub use error::IdentityError;
pub use identity::{
    DEFAULT_IDENTITY_PATH, RadrootsIdentity, RadrootsIdentityFile, RadrootsIdentityId,
    RadrootsIdentityProfile, RadrootsIdentityPublic, RadrootsIdentitySecretKeyFormat,
};
#[cfg(feature = "nip49")]
pub use identity::{
    RadrootsIdentityEncryptedSecretKeyOptions, RadrootsIdentityEncryptedSecretKeySecurity,
};
#[cfg(feature = "std")]
pub use storage::{
    RADROOTS_ENCRYPTED_IDENTITY_DEFAULT_KEY_SLOT, RADROOTS_ENCRYPTED_IDENTITY_KEY_SUFFIX,
    RadrootsEncryptedIdentityFile, encrypted_identity_wrapping_key_path, load_encrypted_identity,
    load_encrypted_identity_with_key_slot, load_identity_profile, rotate_encrypted_identity,
    rotate_encrypted_identity_with_key_slot, store_encrypted_identity,
    store_encrypted_identity_with_key_slot, store_identity_profile,
};
pub use username::{
    RADROOTS_USERNAME_MAX_LEN, RADROOTS_USERNAME_MIN_LEN, RADROOTS_USERNAME_REGEX,
    radroots_username_is_valid, radroots_username_normalize,
};
