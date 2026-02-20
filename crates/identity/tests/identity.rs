use radroots_events::profile::RadrootsProfile;
use radroots_identity::{
    IdentityError, RadrootsIdentity, RadrootsIdentityId, RadrootsIdentityProfile,
};

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
