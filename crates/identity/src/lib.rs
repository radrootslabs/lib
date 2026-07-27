#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod account;
pub mod error;
#[cfg(all(feature = "std", feature = "serde"))]
pub mod identity;
pub mod key;
pub mod profile;
#[cfg(all(feature = "std", feature = "serde"))]
pub mod storage;
pub mod username;

pub use account::AccountId;
pub use error::Error;
#[cfg(feature = "std")]
pub use error::IdentityError;
#[cfg(all(feature = "std", feature = "serde"))]
pub use identity::{
    DEFAULT_IDENTITY_PATH, RadrootsIdentity, RadrootsIdentityFile, RadrootsIdentityId,
    RadrootsIdentityProfile, RadrootsIdentityPublic, RadrootsIdentitySecretKeyFormat,
};
#[cfg(all(feature = "std", feature = "serde", feature = "nip49"))]
pub use identity::{
    RadrootsIdentityEncryptedSecretKeyOptions, RadrootsIdentityEncryptedSecretKeySecurity,
};
pub use key::{IdentityId, PublicKey};
#[cfg(all(feature = "std", feature = "serde"))]
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
