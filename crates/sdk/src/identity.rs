pub use radroots_identity::{
    DEFAULT_IDENTITY_PATH, IdentityError, RADROOTS_USERNAME_MAX_LEN, RADROOTS_USERNAME_MIN_LEN,
    RADROOTS_USERNAME_REGEX, RadrootsIdentity, RadrootsIdentityFile, RadrootsIdentityId,
    RadrootsIdentityProfile, RadrootsIdentityPublic, RadrootsIdentitySecretKeyFormat,
    radroots_username_is_valid, radroots_username_normalize,
};

#[cfg(feature = "identity-storage")]
pub use radroots_identity::{
    RADROOTS_ENCRYPTED_IDENTITY_DEFAULT_KEY_SLOT, RADROOTS_ENCRYPTED_IDENTITY_KEY_SUFFIX,
    RadrootsEncryptedIdentityFile, encrypted_identity_wrapping_key_path, load_encrypted_identity,
    load_encrypted_identity_with_key_slot, load_identity_profile, rotate_encrypted_identity,
    rotate_encrypted_identity_with_key_slot, store_encrypted_identity,
    store_encrypted_identity_with_key_slot, store_identity_profile,
};

#[cfg(all(feature = "identity-models", feature = "identity-storage"))]
#[cfg(test)]
#[path = "../tests/unit/identity_tests.rs"]
mod tests;
