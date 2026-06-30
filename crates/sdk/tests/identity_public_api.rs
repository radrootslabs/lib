#![cfg(feature = "identity-models")]

#[test]
fn identity_models_are_public_through_identity_module() {
    use radroots_sdk::identity::{
        DEFAULT_IDENTITY_PATH, IdentityError, RADROOTS_USERNAME_MAX_LEN, RADROOTS_USERNAME_MIN_LEN,
        RADROOTS_USERNAME_REGEX, RadrootsIdentity, RadrootsIdentityFile, RadrootsIdentityId,
        RadrootsIdentityProfile, RadrootsIdentityPublic, RadrootsIdentitySecretKeyFormat,
        radroots_username_is_valid, radroots_username_normalize,
    };

    assert_eq!(DEFAULT_IDENTITY_PATH, "default.json");
    assert!(RADROOTS_USERNAME_MIN_LEN <= RADROOTS_USERNAME_MAX_LEN);
    assert!(RADROOTS_USERNAME_REGEX.contains("[a-z0-9._-]"));

    let normalized = radroots_username_normalize(" Field_User ").expect("normalized username");
    assert!(radroots_username_is_valid(normalized.as_str()));

    let identity = RadrootsIdentity::generate();
    let identity_id = RadrootsIdentityId::parse(identity.public_key_hex().as_str())
        .expect("identity id parses from public key");
    let public_identity = RadrootsIdentityPublic::new(identity.public_key());
    let empty_profile = RadrootsIdentityProfile::default();
    let identity_file = identity.to_file_with_secret_format(RadrootsIdentitySecretKeyFormat::Hex);

    assert_eq!(identity_id.as_str(), identity.public_key_hex());
    assert_eq!(public_identity.public_key_hex, identity.public_key_hex());
    assert!(empty_profile.is_empty());
    assert!(!identity_file.secret_key.is_empty());
    assert!(matches!(
        RadrootsIdentityId::parse("not-a-public-key"),
        Err(IdentityError::InvalidPublicKey(_))
    ));
    let _: RadrootsIdentityFile = identity_file;
}

#[cfg(feature = "identity-storage")]
#[test]
fn identity_storage_is_public_through_identity_module() {
    use radroots_sdk::identity::{
        RADROOTS_ENCRYPTED_IDENTITY_DEFAULT_KEY_SLOT, RADROOTS_ENCRYPTED_IDENTITY_KEY_SUFFIX,
        RadrootsEncryptedIdentityFile, RadrootsIdentity, encrypted_identity_wrapping_key_path,
        load_encrypted_identity, load_encrypted_identity_with_key_slot, load_identity_profile,
        rotate_encrypted_identity, rotate_encrypted_identity_with_key_slot,
        store_encrypted_identity, store_encrypted_identity_with_key_slot, store_identity_profile,
    };

    let temp = tempfile::tempdir().expect("tempdir");
    let encrypted_path = temp.path().join("sdk-identity.enc.json");
    let profile_path = temp.path().join("sdk-profile.json");
    let identity = RadrootsIdentity::generate();

    let encrypted_file = RadrootsEncryptedIdentityFile::new(encrypted_path.clone());
    encrypted_file.store(&identity).expect("store identity");
    assert_eq!(
        encrypted_file
            .load()
            .expect("load identity")
            .public_key_hex(),
        identity.public_key_hex()
    );

    store_encrypted_identity(encrypted_path.as_path(), &identity).expect("store encrypted");
    assert_eq!(
        load_encrypted_identity(encrypted_path.as_path())
            .expect("load encrypted")
            .public_key_hex(),
        identity.public_key_hex()
    );

    store_encrypted_identity_with_key_slot(encrypted_path.as_path(), "sdk-api", &identity)
        .expect("store encrypted with key slot");
    assert_eq!(
        load_encrypted_identity_with_key_slot(encrypted_path.as_path(), "sdk-api")
            .expect("load encrypted with key slot")
            .public_key_hex(),
        identity.public_key_hex()
    );

    rotate_encrypted_identity(encrypted_path.as_path()).expect("rotate default key slot");
    rotate_encrypted_identity_with_key_slot(encrypted_path.as_path(), "sdk-api")
        .expect("rotate named key slot");
    store_identity_profile(profile_path.as_path(), &identity).expect("store profile");

    assert_eq!(
        load_identity_profile(profile_path.as_path())
            .expect("load profile")
            .public_key_hex,
        identity.public_key_hex()
    );
    assert_eq!(
        RADROOTS_ENCRYPTED_IDENTITY_DEFAULT_KEY_SLOT,
        "radroots_identity"
    );
    assert_eq!(RADROOTS_ENCRYPTED_IDENTITY_KEY_SUFFIX, ".key");
    assert_eq!(
        encrypted_identity_wrapping_key_path(encrypted_path.as_path())
            .file_name()
            .and_then(|name| name.to_str()),
        Some("sdk-identity.enc.json.key")
    );
}
