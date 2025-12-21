#![cfg(all(feature = "nostr", feature = "serde_json"))]

use radroots_events::profile::RadrootsProfile;
use radroots_events_codec::profile::encode::{to_metadata, to_wire_parts};
use radroots_events_codec::profile::error::ProfileEncodeError;
use serde_json::Value;

#[test]
fn profile_to_metadata_rejects_invalid_url() {
    let profile = RadrootsProfile {
        name: "alice".to_string(),
        display_name: None,
        nip05: None,
        about: None,
        website: Some("not-a-url".to_string()),
        picture: None,
        banner: None,
        lud06: None,
        lud16: None,
        bot: None,
    };

    let err = to_metadata(&profile).unwrap_err();
    assert!(matches!(
        err,
        ProfileEncodeError::InvalidUrl("website", _)
    ));
}

#[test]
fn profile_to_wire_parts_writes_json_content() {
    let profile = RadrootsProfile {
        name: "alice".to_string(),
        display_name: Some("Alice".to_string()),
        nip05: None,
        about: None,
        website: None,
        picture: None,
        banner: None,
        lud06: None,
        lud16: None,
        bot: None,
    };

    let parts = to_wire_parts(&profile).unwrap();
    assert_eq!(parts.kind, 0);

    let value: Value = serde_json::from_str(&parts.content).unwrap();
    assert_eq!(value.get("name").and_then(|v| v.as_str()), Some("alice"));
}
