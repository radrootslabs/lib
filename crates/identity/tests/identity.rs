use radroots_events::profile::RadrootsProfile;
use radroots_identity::{
    DEFAULT_IDENTITY_PATH, IdentityError, RadrootsIdentity, RadrootsIdentityId,
    RadrootsIdentityProfile, RadrootsIdentityPublic, RadrootsIdentitySecretKeyFormat,
};
#[cfg(feature = "nip49")]
use radroots_identity::{
    RadrootsIdentityEncryptedSecretKeyOptions, RadrootsIdentityEncryptedSecretKeySecurity,
};
use std::path::PathBuf;

fn profile_with_identifier(value: &str) -> RadrootsIdentityProfile {
    RadrootsIdentityProfile {
        identifier: Some(value.to_string()),
        ..Default::default()
    }
}

fn sample_event(content: &str) -> nostr::Event {
    nostr::EventBuilder::text_note(content)
        .sign_with_keys(&nostr::Keys::generate())
        .unwrap()
}

#[test]
fn load_from_json_file_hex() {
    let keys = nostr::Keys::generate();
    let identity = RadrootsIdentity::new(keys.clone());
    let json = serde_json::to_string(&identity.to_file()).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");
    std::fs::write(&path, json).unwrap();

    let loaded = RadrootsIdentity::load_from_path_auto(&path).unwrap();
    assert_eq!(loaded.public_key(), keys.public_key());
}

#[test]
fn load_from_json_file_profile() {
    let keys = nostr::Keys::generate();
    let mut identity = RadrootsIdentity::new(keys.clone());
    let profile = RadrootsProfile {
        name: "relay-agent".to_string(),
        display_name: Some("Relay Agent".to_string()),
        nip05: None,
        about: Some("hello".to_string()),
        website: None,
        picture: None,
        banner: None,
        lud06: None,
        lud16: None,
        bot: None,
    };
    identity.set_profile(RadrootsIdentityProfile {
        profile: Some(profile),
        ..Default::default()
    });
    let json = serde_json::to_string(&identity.to_file()).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");
    std::fs::write(&path, json).unwrap();

    let loaded = RadrootsIdentity::load_from_path_auto(&path).unwrap();
    let loaded_profile = loaded.profile().and_then(|p| p.profile.as_ref()).unwrap();
    assert_eq!(loaded_profile.name, "relay-agent");
    assert_eq!(loaded_profile.display_name.as_deref(), Some("Relay Agent"));
    assert_eq!(loaded_profile.about.as_deref(), Some("hello"));
}

#[test]
fn load_from_text_file_hex() {
    let keys = nostr::Keys::generate();
    let identity = RadrootsIdentity::new(keys.clone());
    let secret = identity.secret_key_hex();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.txt");
    std::fs::write(&path, secret).unwrap();

    let loaded = RadrootsIdentity::load_from_path_auto(&path).unwrap();
    assert_eq!(loaded.public_key(), keys.public_key());
}

#[test]
fn load_from_text_file_nsec() {
    let keys = nostr::Keys::generate();
    let identity = RadrootsIdentity::new(keys.clone());
    let secret = identity.secret_key_nsec();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.txt");
    std::fs::write(&path, secret).unwrap();

    let loaded = RadrootsIdentity::load_from_path_auto(&path).unwrap();
    assert_eq!(loaded.public_key(), keys.public_key());
}

#[test]
fn load_from_binary_file() {
    let keys = nostr::Keys::generate();
    let identity = RadrootsIdentity::new(keys.clone());
    let secret = identity.secret_key_bytes();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.key");
    std::fs::write(&path, secret).unwrap();

    let loaded = RadrootsIdentity::load_from_path_auto(&path).unwrap();
    assert_eq!(loaded.public_key(), keys.public_key());
}

#[test]
fn load_or_generate_missing_disallowed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");

    let err = RadrootsIdentity::load_or_generate(Some(&path), false).unwrap_err();
    assert!(matches!(err, IdentityError::GenerationNotAllowed(p) if p == path));
}

#[test]
fn load_or_generate_missing_allowed_creates_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");

    let identity = RadrootsIdentity::load_or_generate(Some(&path), true).unwrap();
    assert!(path.exists());

    let loaded = RadrootsIdentity::load_from_path_auto(&path).unwrap();
    assert_eq!(loaded.public_key(), identity.public_key());
}

#[test]
fn load_from_json_file_public_key_npub() {
    let keys = nostr::Keys::generate();
    let identity = RadrootsIdentity::new(keys.clone());
    let mut file = identity.to_file();
    file.public_key = Some(identity.public_key_npub());
    let json = serde_json::to_string(&file).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");
    std::fs::write(&path, json).unwrap();

    let loaded = RadrootsIdentity::load_from_path_auto(&path).unwrap();
    assert_eq!(loaded.public_key(), keys.public_key());
}

#[test]
fn load_from_json_file_public_key_mismatch() {
    let keys = nostr::Keys::generate();
    let identity = RadrootsIdentity::new(keys);
    let other_keys = nostr::Keys::generate();
    let mut file = identity.to_file();
    file.public_key = Some(other_keys.public_key().to_hex());
    let json = serde_json::to_string(&file).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");
    std::fs::write(&path, json).unwrap();

    let err = RadrootsIdentity::load_from_path_auto(&path).unwrap_err();
    assert!(matches!(err, IdentityError::PublicKeyMismatch));
}

#[test]
fn identity_id_matches_public_key_hex() {
    let keys = nostr::Keys::generate();
    let identity = RadrootsIdentity::new(keys.clone());

    let id = identity.id();
    assert_eq!(id.as_str(), keys.public_key().to_hex());
}

#[test]
fn identity_id_parses_hex_and_npub() {
    use nostr::nips::nip19::ToBech32;

    let keys = nostr::Keys::generate();
    let public_key = keys.public_key();
    let hex = public_key.to_hex();
    let npub = public_key.to_bech32().unwrap();

    let from_hex = RadrootsIdentityId::parse(hex.as_str()).unwrap();
    let from_npub = RadrootsIdentityId::parse(npub.as_str()).unwrap();
    assert_eq!(from_hex.as_str(), hex);
    assert_eq!(from_npub.as_str(), hex);
}

#[test]
fn to_public_projection_excludes_secret_key_fields() {
    let keys = nostr::Keys::generate();
    let identity = RadrootsIdentity::new(keys.clone());
    let public = identity.to_public();

    assert_eq!(public.id.as_str(), keys.public_key().to_hex());
    assert_eq!(public.public_key_hex, keys.public_key().to_hex());
    assert!(public.profile.is_none());

    let json = serde_json::to_string(&public).unwrap();
    assert!(!json.contains("secret_key"));
    assert!(!json.contains(&identity.secret_key_hex()));
}

#[test]
fn identity_id_trait_paths_and_string_conversions() {
    let keys = nostr::Keys::generate();
    let public_key = keys.public_key();
    let public_key_hex = public_key.to_hex();

    let from_impl = RadrootsIdentityId::from(public_key);
    assert_eq!(from_impl.as_ref(), public_key_hex);

    let from_try = RadrootsIdentityId::try_from(public_key_hex.as_str()).unwrap();
    assert_eq!(from_try.to_string(), public_key_hex);
    assert_eq!(from_try.clone().into_string(), public_key_hex);
}

#[test]
fn identity_profile_state_mutation_paths() {
    let keys = nostr::Keys::generate();
    let mut identity =
        RadrootsIdentity::with_profile(keys.clone(), RadrootsIdentityProfile::default());
    assert!(identity.profile().is_none());

    identity.set_profile(RadrootsIdentityProfile::default());
    assert!(identity.profile().is_none());

    let profile = profile_with_identifier("radroots-user");
    identity.set_profile(profile.clone());
    assert!(identity.profile().is_some());

    let profile_mut = identity.profile_mut().unwrap();
    profile_mut.identifier = Some("radroots-user-updated".to_string());
    assert_eq!(
        identity.profile().and_then(|p| p.identifier.as_deref()),
        Some("radroots-user-updated")
    );

    let public = identity.to_public();
    assert!(public.profile.is_some());

    identity.clear_profile();
    assert!(identity.profile().is_none());

    let public_without_profile = RadrootsIdentityPublic::new(keys.public_key())
        .with_profile(RadrootsIdentityProfile::default());
    assert!(public_without_profile.profile.is_none());

    let public_with_profile = RadrootsIdentityPublic::new(keys.public_key()).with_profile(profile);
    assert!(public_with_profile.profile.is_some());
}

#[test]
fn identity_accessor_paths_and_secret_formats() {
    let keys = nostr::Keys::generate();
    let identity = RadrootsIdentity::new(keys.clone());

    assert_eq!(identity.keys().public_key(), keys.public_key());
    assert_eq!(identity.public_key(), keys.public_key());
    assert!(identity.npub().starts_with("npub1"));
    assert!(identity.nsec().starts_with("nsec1"));

    let file_nsec = identity.to_file_with_secret_format(RadrootsIdentitySecretKeyFormat::Nsec);
    assert!(file_nsec.secret_key.starts_with("nsec1"));

    let from_keys: RadrootsIdentity = keys.clone().into();
    let roundtrip_keys = from_keys.clone().into_keys();
    assert_eq!(roundtrip_keys.public_key(), keys.public_key());
}

#[cfg(feature = "nip49")]
#[test]
fn encrypted_secret_key_round_trips_to_identity() {
    let identity = RadrootsIdentity::generate();
    let encrypted = identity
        .encrypt_secret_key_ncryptsec("fixture-password")
        .unwrap();
    assert!(encrypted.starts_with("ncryptsec1"));

    let decrypted =
        RadrootsIdentity::from_encrypted_secret_key_str(&encrypted, "fixture-password").unwrap();
    assert_eq!(decrypted.public_key(), identity.public_key());
}

#[cfg(feature = "nip49")]
#[test]
fn encrypted_secret_key_options_propagate_to_output() {
    use nostr::nips::nip19::FromBech32;
    use nostr::nips::nip49::{EncryptedSecretKey, KeySecurity};

    let identity = RadrootsIdentity::generate();
    let encrypted = identity
        .encrypt_secret_key_ncryptsec_with_options(
            "fixture-password",
            RadrootsIdentityEncryptedSecretKeyOptions {
                log_n: 15,
                key_security: RadrootsIdentityEncryptedSecretKeySecurity::Medium,
            },
        )
        .unwrap();
    let parsed = EncryptedSecretKey::from_bech32(&encrypted).unwrap();
    assert_eq!(parsed.log_n(), 15);
    assert_eq!(parsed.key_security(), KeySecurity::Medium);
}

#[cfg(feature = "nip49")]
#[test]
fn encrypted_secret_key_rejects_invalid_and_wrong_password_inputs() {
    let identity = RadrootsIdentity::generate();
    let encrypted = identity
        .encrypt_secret_key_ncryptsec("fixture-password")
        .unwrap();

    let invalid =
        RadrootsIdentity::from_encrypted_secret_key_str("not-an-encrypted-secret", "password")
            .unwrap_err();
    assert!(matches!(
        invalid,
        IdentityError::InvalidEncryptedSecretKey(_)
    ));

    let wrong_password =
        RadrootsIdentity::from_encrypted_secret_key_str(&encrypted, "wrong-password").unwrap_err();
    assert!(matches!(
        wrong_password,
        IdentityError::DecryptEncryptedSecretKey(_)
    ));
}

#[test]
fn parse_failures_cover_public_key_errors() {
    let err_empty = RadrootsIdentityId::parse("   ").unwrap_err();
    assert!(matches!(err_empty, IdentityError::InvalidPublicKey(_)));

    let err_invalid = RadrootsIdentityId::parse("invalid-public-key-value").unwrap_err();
    assert!(matches!(err_invalid, IdentityError::InvalidPublicKey(_)));
}

#[test]
fn from_secret_key_bytes_rejects_wrong_length() {
    let err = RadrootsIdentity::from_secret_key_bytes(&[1, 2, 3]).unwrap_err();
    assert!(matches!(err, IdentityError::InvalidIdentityFormat));
}

#[test]
fn from_secret_key_str_rejects_invalid_secret() {
    let err = RadrootsIdentity::from_secret_key_str("not-a-secret-key").unwrap_err();
    assert!(matches!(err, IdentityError::InvalidSecretKey(_)));
}

#[test]
fn from_secret_key_bytes_rejects_invalid_scalar() {
    let err = RadrootsIdentity::from_secret_key_bytes(&[0u8; 32]).unwrap_err();
    assert!(matches!(err, IdentityError::InvalidSecretKey(_)));
}

#[test]
fn load_from_path_reports_not_found_and_read_errors() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing-identity.json");
    let not_found = RadrootsIdentity::load_from_path_auto(&missing).unwrap_err();
    assert!(matches!(not_found, IdentityError::NotFound(path) if path == missing));

    let read_error = RadrootsIdentity::load_from_path_auto(dir.path()).unwrap_err();
    assert!(matches!(read_error, IdentityError::Read(path, _) if path == dir.path()));
}

#[test]
fn load_from_path_rejects_invalid_payloads() {
    let dir = tempfile::tempdir().unwrap();

    let blank_path = dir.path().join("identity-blank.txt");
    std::fs::write(&blank_path, "   \n\t ").unwrap();
    let blank_err = RadrootsIdentity::load_from_path_auto(&blank_path).unwrap_err();
    assert!(matches!(blank_err, IdentityError::InvalidIdentityFormat));

    let invalid_utf8_path = dir.path().join("identity-invalid-utf8.bin");
    std::fs::write(&invalid_utf8_path, [0xff, 0xfe, 0xfd]).unwrap();
    let utf8_err = RadrootsIdentity::load_from_path_auto(&invalid_utf8_path).unwrap_err();
    assert!(matches!(utf8_err, IdentityError::InvalidIdentityFormat));

    let invalid_json_path = dir.path().join("identity-invalid-json.json");
    std::fs::write(&invalid_json_path, "{invalid").unwrap();
    let json_err = RadrootsIdentity::load_from_path_auto(&invalid_json_path).unwrap_err();
    assert!(matches!(json_err, IdentityError::InvalidJson(_)));
}

#[test]
fn load_from_json_file_without_public_key_succeeds() {
    let keys = nostr::Keys::generate();
    let identity = RadrootsIdentity::new(keys.clone());
    let mut file = identity.to_file();
    file.public_key = None;
    let json = serde_json::to_string(&file).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");
    std::fs::write(&path, json).unwrap();

    let loaded = RadrootsIdentity::load_from_path_auto(&path).unwrap();
    assert_eq!(loaded.public_key(), keys.public_key());
}

#[test]
fn load_from_json_file_rejects_invalid_secret_key_string() {
    let payload = serde_json::json!({
        "secret_key": "invalid-secret-key",
        "public_key": null,
    });
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");
    std::fs::write(&path, payload.to_string()).unwrap();

    let err = RadrootsIdentity::load_from_path_auto(&path).unwrap_err();
    assert!(matches!(err, IdentityError::InvalidSecretKey(_)));
}

#[test]
fn load_from_json_file_rejects_invalid_public_key_value() {
    let keys = nostr::Keys::generate();
    let identity = RadrootsIdentity::new(keys);
    let mut file = identity.to_file();
    file.public_key = Some("invalid-public-key".to_string());
    let json = serde_json::to_string(&file).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");
    std::fs::write(&path, json).unwrap();

    let err = RadrootsIdentity::load_from_path_auto(&path).unwrap_err();
    assert!(matches!(err, IdentityError::InvalidPublicKey(_)));
}

#[test]
fn save_json_rejects_directory_target() {
    let identity = RadrootsIdentity::generate();
    let dir = tempfile::tempdir().unwrap();
    let err = identity.save_json(dir.path()).unwrap_err();
    assert!(matches!(err, IdentityError::Store(_)));
}

#[cfg(unix)]
#[test]
fn save_json_reports_write_failure_on_read_only_directory() {
    use std::os::unix::fs::PermissionsExt;

    let identity = RadrootsIdentity::generate();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");
    identity.save_json(path.as_path()).unwrap();

    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
    let err_path = identity.save_json(path.as_path()).unwrap_err();
    assert!(matches!(err_path, IdentityError::Store(_)));
    let err_path_buf = identity.save_json(&path).unwrap_err();
    assert!(matches!(err_path_buf, IdentityError::Store(_)));
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
}

#[cfg(unix)]
#[test]
fn load_or_generate_reports_save_failure_when_parent_not_writable() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("readonly");
    std::fs::create_dir(&parent).unwrap();
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o500)).unwrap();

    let path = parent.join("identity.json");
    let err = RadrootsIdentity::load_or_generate::<&std::path::Path>(Some(path.as_path()), true)
        .unwrap_err();
    assert!(matches!(err, IdentityError::Store(_)));
    let err_path_buf = RadrootsIdentity::load_or_generate(Some(&path), true).unwrap_err();
    assert!(matches!(err_path_buf, IdentityError::Store(_)));
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn load_or_generate_uses_default_path_when_missing() {
    let original = std::env::current_dir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let denied = RadrootsIdentity::load_or_generate::<&std::path::Path>(None, false).unwrap_err();
    assert!(
        matches!(denied, IdentityError::GenerationNotAllowed(path) if path == PathBuf::from(DEFAULT_IDENTITY_PATH))
    );

    let generated = RadrootsIdentity::load_or_generate::<&std::path::Path>(None, true).unwrap();
    let default_path = dir.path().join(DEFAULT_IDENTITY_PATH);
    assert!(default_path.exists());

    let loaded = RadrootsIdentity::load_from_path_auto(&default_path).unwrap();
    assert_eq!(generated.public_key(), loaded.public_key());

    std::env::set_current_dir(original).unwrap();
}

#[test]
fn load_or_generate_prefers_existing_path() {
    let keys = nostr::Keys::generate();
    let identity = RadrootsIdentity::new(keys.clone());
    let payload = serde_json::to_string(&identity.to_file()).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");
    std::fs::write(&path, payload).unwrap();

    let loaded = RadrootsIdentity::load_or_generate(Some(&path), false).unwrap();
    assert_eq!(loaded.public_key(), keys.public_key());
}

#[test]
fn path_ref_variants_cover_success_paths() {
    let identity = RadrootsIdentity::generate();
    let dir = tempfile::tempdir().unwrap();

    let saved_path = dir.path().join("saved.json");
    identity.save_json(saved_path.as_path()).unwrap();
    let loaded = RadrootsIdentity::load_from_path_auto(saved_path.as_path()).unwrap();
    assert_eq!(loaded.public_key(), identity.public_key());

    let generated_path = dir.path().join("generated.json");
    let generated =
        RadrootsIdentity::load_or_generate(Some(generated_path.as_path()), true).unwrap();
    assert!(generated_path.exists());
    let roundtrip = RadrootsIdentity::load_from_path_auto(generated_path.as_path()).unwrap();
    assert_eq!(generated.public_key(), roundtrip.public_key());
}

#[test]
fn generate_with_profile_retains_profile() {
    let profile = profile_with_identifier("runtime-user");
    let identity = RadrootsIdentity::generate_with_profile(profile);
    assert_eq!(
        identity.profile().and_then(|p| p.identifier.as_deref()),
        Some("runtime-user")
    );
}

#[test]
fn identity_profile_is_empty_checks_metadata_and_application_handler() {
    let profile_with_metadata = RadrootsIdentityProfile {
        metadata: Some(sample_event("metadata")),
        ..Default::default()
    };
    assert!(!profile_with_metadata.is_empty());

    let profile_with_handler = RadrootsIdentityProfile {
        application_handler: Some(sample_event("handler")),
        ..Default::default()
    };
    assert!(!profile_with_handler.is_empty());
}

#[cfg(feature = "secrecy")]
#[test]
fn secret_key_hex_secret_returns_secret_string() {
    use secrecy::ExposeSecret;

    let keys = nostr::Keys::generate();
    let identity = RadrootsIdentity::new(keys);
    let secret = identity.secret_key_hex_secret();
    assert_eq!(secret.expose_secret(), &identity.secret_key_hex());
}

#[cfg(feature = "zeroize")]
#[test]
fn secret_key_zeroizing_bytes_matches_raw_secret() {
    let keys = nostr::Keys::generate();
    let identity = RadrootsIdentity::new(keys);
    let raw = identity.secret_key_bytes();
    let protected = identity.secret_key_bytes_zeroizing();
    assert_eq!(&*protected, &raw);
}
