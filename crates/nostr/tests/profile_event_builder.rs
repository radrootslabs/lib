#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use radroots_event::profile::RadrootsAuthoredProfile;
use radroots_nostr::prelude::{
    RadrootsNostrKeys, RadrootsNostrSecretKey, RadrootsNostrTimestamp,
    radroots_nostr_build_profile_event,
};

#[test]
fn typed_profile_builder_preserves_the_strict_replacement_snapshot() {
    let keys = RadrootsNostrKeys::new(
        RadrootsNostrSecretKey::from_hex(test_fixtures::FIXTURE_ALICE_SECRET_KEY_HEX).unwrap(),
    );
    let created_at = RadrootsNostrTimestamp::from_secs(1_784_347_200);
    let profile = RadrootsAuthoredProfile::new("Alice")
        .unwrap()
        .with_display_name("Alice's Orchard")
        .with_about("Tree fruit")
        .with_bot(false);

    let event = radroots_nostr_build_profile_event(&profile)
        .unwrap()
        .custom_created_at(created_at)
        .sign_with_keys(&keys)
        .unwrap();

    assert_eq!(event.kind.as_u16(), 0);
    assert_eq!(event.created_at, created_at);
    assert!(event.tags.is_empty());
    assert_eq!(
        event.content,
        r#"{"name":"Alice","display_name":"Alice's Orchard","about":"Tree fruit","bot":false}"#
    );
    assert!(event.verify().is_ok());
}
