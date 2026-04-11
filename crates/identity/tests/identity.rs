#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use radroots_events::profile::RadrootsProfile;
use radroots_identity::{
    DEFAULT_IDENTITY_PATH, IdentityError, RadrootsIdentity, RadrootsIdentityId,
    RadrootsIdentityProfile, RadrootsIdentityPublic, RadrootsIdentitySecretKeyFormat,
};
#[cfg(feature = "nip49")]
use radroots_identity::{
    RadrootsIdentityEncryptedSecretKeyOptions, RadrootsIdentityEncryptedSecretKeySecurity,
};
use radroots_runtime_paths::{
    RadrootsHostEnvironment, RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver,
    RadrootsPlatform,
};
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
use test_fixtures::{ApprovedFixtureIdentity, FIXTURE_ALICE, FIXTURE_BOB};

fn home_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn remove(key: &'static str) -> Self {
        let previous = std::env::var_os(key);
        unsafe { std::env::remove_var(key) };
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(value) = self.previous.as_ref() {
            unsafe { std::env::set_var(self.key, value) };
        } else {
            unsafe { std::env::remove_var(self.key) };
        }
    }
}

fn fixture_keys(fixture: ApprovedFixtureIdentity) -> nostr::Keys {
    let secret = nostr::SecretKey::from_hex(fixture.secret_key_hex).unwrap();
    nostr::Keys::new(secret)
}

fn fixture_identity(fixture: ApprovedFixtureIdentity) -> RadrootsIdentity {
    RadrootsIdentity::from_secret_key_str(fixture.secret_key_hex).unwrap()
}

fn profile_with_identifier(value: &str) -> RadrootsIdentityProfile {
    RadrootsIdentityProfile {
        identifier: Some(value.to_string()),
        ..Default::default()
    }
}

fn sample_event(content: &str) -> nostr::Event {
    nostr::EventBuilder::text_note(content)
        .sign_with_keys(&fixture_keys(FIXTURE_ALICE))
        .unwrap()
}

#[test]
fn load_from_json_file_hex() {
    let identity = fixture_identity(FIXTURE_ALICE);
    let json = serde_json::to_string(&identity.to_file()).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");
    std::fs::write(&path, json).unwrap();

    let loaded = RadrootsIdentity::load_from_path_auto(&path).unwrap();
    assert_eq!(loaded.public_key().to_hex(), FIXTURE_ALICE.public_key_hex);
}

#[test]
fn load_from_json_file_profile() {
    let mut identity = fixture_identity(FIXTURE_ALICE);
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
    let identity = fixture_identity(FIXTURE_ALICE);
    let secret = identity.secret_key_hex();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.txt");
    std::fs::write(&path, secret).unwrap();

    let loaded = RadrootsIdentity::load_from_path_auto(&path).unwrap();
    assert_eq!(loaded.public_key().to_hex(), FIXTURE_ALICE.public_key_hex);
}

#[test]
fn load_from_text_file_nsec() {
    let identity = fixture_identity(FIXTURE_ALICE);
    let secret = identity.secret_key_nsec();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.txt");
    std::fs::write(&path, secret).unwrap();

    let loaded = RadrootsIdentity::load_from_path_auto(&path).unwrap();
    assert_eq!(loaded.public_key().to_hex(), FIXTURE_ALICE.public_key_hex);
}

#[test]
fn load_from_binary_file() {
    let identity = fixture_identity(FIXTURE_ALICE);
    let secret = identity.secret_key_bytes();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.key");
    std::fs::write(&path, secret).unwrap();

    let loaded = RadrootsIdentity::load_from_path_auto(&path).unwrap();
    assert_eq!(loaded.public_key().to_hex(), FIXTURE_ALICE.public_key_hex);
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
    let identity = fixture_identity(FIXTURE_ALICE);
    let mut file = identity.to_file();
    file.public_key = Some(identity.public_key_npub());
    let json = serde_json::to_string(&file).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");
    std::fs::write(&path, json).unwrap();

    let loaded = RadrootsIdentity::load_from_path_auto(&path).unwrap();
    assert_eq!(loaded.public_key().to_hex(), FIXTURE_ALICE.public_key_hex);
}

#[test]
fn load_from_json_file_public_key_mismatch() {
    let identity = fixture_identity(FIXTURE_ALICE);
    let mut file = identity.to_file();
    file.public_key = Some(FIXTURE_BOB.public_key_hex.to_string());
    let json = serde_json::to_string(&file).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");
    std::fs::write(&path, json).unwrap();

    let err = RadrootsIdentity::load_from_path_auto(&path).unwrap_err();
    assert!(matches!(err, IdentityError::PublicKeyMismatch));
}

#[test]
fn identity_id_matches_public_key_hex() {
    let identity = fixture_identity(FIXTURE_ALICE);

    let id = identity.id();
    assert_eq!(id.as_str(), FIXTURE_ALICE.public_key_hex);
}

#[test]
fn identity_id_parses_hex_and_npub() {
    let from_hex = RadrootsIdentityId::parse(FIXTURE_ALICE.public_key_hex).unwrap();
    let from_npub = RadrootsIdentityId::parse(FIXTURE_ALICE.npub).unwrap();
    assert_eq!(from_hex.as_str(), FIXTURE_ALICE.public_key_hex);
    assert_eq!(from_npub.as_str(), FIXTURE_ALICE.public_key_hex);
}

#[test]
fn to_public_projection_excludes_secret_key_fields() {
    let identity = fixture_identity(FIXTURE_ALICE);
    let public = identity.to_public();

    assert_eq!(public.id.as_str(), FIXTURE_ALICE.public_key_hex);
    assert_eq!(public.public_key_hex, FIXTURE_ALICE.public_key_hex);
    assert_eq!(public.public_key_npub, FIXTURE_ALICE.npub);
    assert!(public.profile.is_none());

    let json = serde_json::to_string(&public).unwrap();
    assert!(!json.contains("secret_key"));
    assert!(!json.contains(&identity.secret_key_hex()));
}

#[test]
fn identity_id_trait_paths_and_string_conversions() {
    let public_key = fixture_identity(FIXTURE_ALICE).public_key();
    let public_key_hex = FIXTURE_ALICE.public_key_hex.to_string();

    let from_impl = RadrootsIdentityId::from(public_key);
    assert_eq!(from_impl.as_ref(), public_key_hex);

    let from_try = RadrootsIdentityId::try_from(public_key_hex.as_str()).unwrap();
    assert_eq!(from_try.to_string(), public_key_hex);
    assert_eq!(from_try.clone().into_string(), public_key_hex);
}

#[test]
fn identity_profile_state_mutation_paths() {
    let mut identity = RadrootsIdentity::with_profile(
        fixture_keys(FIXTURE_ALICE),
        RadrootsIdentityProfile::default(),
    );
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

    let public_without_profile = RadrootsIdentityPublic::new(identity.public_key())
        .with_profile(RadrootsIdentityProfile::default());
    assert!(public_without_profile.profile.is_none());

    let public_with_profile =
        RadrootsIdentityPublic::new(identity.public_key()).with_profile(profile);
    assert!(public_with_profile.profile.is_some());
}

#[test]
fn identity_accessor_paths_and_secret_formats() {
    let identity = fixture_identity(FIXTURE_ALICE);

    assert_eq!(
        identity.keys().public_key().to_hex(),
        FIXTURE_ALICE.public_key_hex
    );
    assert_eq!(identity.public_key().to_hex(), FIXTURE_ALICE.public_key_hex);
    assert_eq!(identity.npub(), FIXTURE_ALICE.npub);
    assert_eq!(identity.nsec(), FIXTURE_ALICE.nsec);

    let file_nsec = identity.to_file_with_secret_format(RadrootsIdentitySecretKeyFormat::Nsec);
    assert_eq!(file_nsec.secret_key, FIXTURE_ALICE.nsec);

    let from_keys: RadrootsIdentity = fixture_keys(FIXTURE_ALICE).into();
    let roundtrip_keys = from_keys.clone().into_keys();
    assert_eq!(
        roundtrip_keys.public_key().to_hex(),
        FIXTURE_ALICE.public_key_hex
    );
}

#[cfg(feature = "nip49")]
#[test]
fn encrypted_secret_key_round_trips_to_identity() {
    let identity = fixture_identity(FIXTURE_ALICE);
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

    let identity = fixture_identity(FIXTURE_ALICE);
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
fn encrypted_secret_key_weak_security_and_invalid_log_n_paths() {
    use nostr::nips::nip49::KeySecurity;

    assert_eq!(
        KeySecurity::from(RadrootsIdentityEncryptedSecretKeySecurity::Weak),
        KeySecurity::Weak
    );

    let identity = fixture_identity(FIXTURE_ALICE);
    let err = identity
        .encrypt_secret_key_ncryptsec_with_options(
            "fixture-password",
            RadrootsIdentityEncryptedSecretKeyOptions {
                log_n: 255,
                key_security: RadrootsIdentityEncryptedSecretKeySecurity::Weak,
            },
        )
        .unwrap_err();
    assert!(matches!(err, IdentityError::EncryptSecretKey(_)));
}

#[cfg(feature = "nip49")]
#[test]
fn encrypted_secret_key_rejects_invalid_and_wrong_password_inputs() {
    let identity = fixture_identity(FIXTURE_ALICE);
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

#[cfg(feature = "nip49")]
#[test]
fn load_from_path_auto_rejects_nip49_export_format() {
    let identity = fixture_identity(FIXTURE_ALICE);
    let encrypted = identity
        .encrypt_secret_key_ncryptsec("fixture-password")
        .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.ncryptsec");
    std::fs::write(&path, encrypted).unwrap();

    let err = RadrootsIdentity::load_from_path_auto(&path).unwrap_err();
    assert!(matches!(err, IdentityError::InvalidSecretKey(_)));
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
    let identity = fixture_identity(FIXTURE_ALICE);
    let mut file = identity.to_file();
    file.public_key = None;
    let json = serde_json::to_string(&file).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");
    std::fs::write(&path, json).unwrap();

    let loaded = RadrootsIdentity::load_from_path_auto(&path).unwrap();
    assert_eq!(loaded.public_key().to_hex(), FIXTURE_ALICE.public_key_hex);
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
    let identity = fixture_identity(FIXTURE_ALICE);
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
    let identity = fixture_identity(FIXTURE_ALICE);
    let dir = tempfile::tempdir().unwrap();
    let err = identity.save_json(dir.path()).unwrap_err();
    assert!(matches!(err, IdentityError::Store(_)));
}

#[cfg(unix)]
#[test]
fn save_json_reports_write_failure_on_read_only_directory() {
    use std::os::unix::fs::PermissionsExt;

    let identity = fixture_identity(FIXTURE_ALICE);
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
    let resolver = RadrootsPathResolver::new(
        RadrootsPlatform::Linux,
        RadrootsHostEnvironment {
            home_dir: Some(PathBuf::from("/home/treesap")),
            ..RadrootsHostEnvironment::default()
        },
    );
    let default_path = RadrootsIdentity::default_path_for(
        &resolver,
        RadrootsPathProfile::InteractiveUser,
        &RadrootsPathOverrides::default(),
    )
    .unwrap();

    let denied = RadrootsIdentity::load_or_generate::<&std::path::Path>(Some(&default_path), false)
        .unwrap_err();
    assert!(matches!(denied, IdentityError::GenerationNotAllowed(path) if path == default_path));
    assert_eq!(
        default_path.file_name().and_then(std::ffi::OsStr::to_str),
        Some(DEFAULT_IDENTITY_PATH)
    );
    assert_eq!(
        default_path,
        PathBuf::from("/home/treesap/.radroots/secrets/shared/identities/default.json")
    );
}

#[test]
fn default_path_matches_current_resolver_default_path() {
    let expected = RadrootsIdentity::default_path_for(
        &RadrootsPathResolver::current(),
        RadrootsPathProfile::InteractiveUser,
        &RadrootsPathOverrides::default(),
    )
    .unwrap();

    assert_eq!(RadrootsIdentity::default_path().unwrap(), expected);
}

#[test]
fn default_path_for_reports_missing_home_dir() {
    let resolver =
        RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default());
    let err = RadrootsIdentity::default_path_for(
        &resolver,
        RadrootsPathProfile::InteractiveUser,
        &RadrootsPathOverrides::default(),
    )
    .unwrap_err();
    assert!(matches!(err, IdentityError::Paths(_)));
}

#[test]
fn load_or_generate_without_explicit_path_propagates_default_path_errors() {
    let _lock = home_env_lock().lock().unwrap();
    let _guard = EnvVarGuard::remove("HOME");

    let err = RadrootsIdentity::load_or_generate::<&std::path::Path>(None, false).unwrap_err();
    assert!(matches!(err, IdentityError::Paths(_)));
}

#[test]
fn load_or_generate_creates_at_explicit_default_path() {
    let dir = tempfile::tempdir().unwrap();
    let default_path = dir.path().join(DEFAULT_IDENTITY_PATH);
    let generated =
        RadrootsIdentity::load_or_generate::<&std::path::Path>(Some(&default_path), true).unwrap();
    assert!(default_path.exists());

    let loaded = RadrootsIdentity::load_from_path_auto(&default_path).unwrap();
    assert_eq!(generated.public_key(), loaded.public_key());
}

#[test]
fn load_or_generate_prefers_existing_path() {
    let identity = fixture_identity(FIXTURE_ALICE);
    let payload = serde_json::to_string(&identity.to_file()).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.json");
    std::fs::write(&path, payload).unwrap();

    let loaded = RadrootsIdentity::load_or_generate(Some(&path), false).unwrap();
    assert_eq!(loaded.public_key().to_hex(), FIXTURE_ALICE.public_key_hex);
}

#[test]
fn path_ref_variants_cover_success_paths() {
    let identity = fixture_identity(FIXTURE_ALICE);
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

#[test]
fn identity_error_display_variants_are_exercised() {
    let missing_path = PathBuf::from("/tmp/missing-identity.json");
    assert_eq!(
        IdentityError::NotFound(missing_path.clone()).to_string(),
        format!("identity file missing at {}", missing_path.display())
    );
    assert_eq!(
        IdentityError::GenerationNotAllowed(missing_path.clone()).to_string(),
        format!(
            "identity file missing at {} and generation is not permitted (pass --allow-generate-identity)",
            missing_path.display()
        )
    );
    assert!(
        IdentityError::Read(missing_path.clone(), std::io::Error::other("boom"))
            .to_string()
            .contains("failed to read identity file")
    );

    let json_err = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
    assert!(
        IdentityError::InvalidJson(json_err)
            .to_string()
            .contains("invalid identity JSON")
    );

    let secret_err = nostr::Keys::parse("not-a-secret-key").unwrap_err();
    assert!(
        IdentityError::InvalidSecretKey(secret_err)
            .to_string()
            .contains("invalid secret key")
    );

    #[cfg(feature = "nip49")]
    {
        assert_eq!(
            IdentityError::EncryptSecretKey("encrypt failed".into()).to_string(),
            "failed to encrypt secret key: encrypt failed"
        );
        assert_eq!(
            IdentityError::InvalidEncryptedSecretKey("bad payload".into()).to_string(),
            "invalid encrypted secret key: bad payload"
        );
        assert_eq!(
            IdentityError::DecryptEncryptedSecretKey("bad password".into()).to_string(),
            "failed to decrypt encrypted secret key: bad password"
        );
    }

    assert_eq!(
        IdentityError::InvalidPublicKey("bad-pubkey".into()).to_string(),
        "invalid public key: bad-pubkey"
    );
    assert_eq!(
        IdentityError::PublicKeyMismatch.to_string(),
        "public key does not match secret key"
    );
    assert_eq!(
        IdentityError::InvalidIdentityFormat.to_string(),
        "unsupported identity file format"
    );

    #[cfg(all(feature = "std", feature = "json-file"))]
    {
        let store_err = fixture_identity(FIXTURE_ALICE)
            .save_json(tempfile::tempdir().unwrap().path())
            .unwrap_err();
        assert!(!store_err.to_string().is_empty());
    }

    let paths_err = IdentityError::from(
        radroots_runtime_paths::RadrootsRuntimePathsError::MissingHomeDir {
            platform: RadrootsPlatform::Linux,
        },
    );
    assert_eq!(
        paths_err.to_string(),
        "interactive_user on linux requires a home directory"
    );
}

#[cfg(feature = "secrecy")]
#[test]
fn secret_key_hex_secret_returns_secret_string() {
    use secrecy::ExposeSecret;

    let identity = fixture_identity(FIXTURE_ALICE);
    let secret = identity.secret_key_hex_secret();
    assert_eq!(secret.expose_secret(), &identity.secret_key_hex());
}

#[cfg(feature = "zeroize")]
#[test]
fn secret_key_zeroizing_bytes_matches_raw_secret() {
    let identity = fixture_identity(FIXTURE_ALICE);
    let raw = identity.secret_key_bytes();
    let protected = identity.secret_key_bytes_zeroizing();
    assert_eq!(&*protected, &raw);
}
