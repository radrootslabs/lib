#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use nostr::{Keys as RadrootsNostrKeys, SecretKey as RadrootsNostrSecretKey};
use radroots_event::profile::AuthoredProfile;
use radroots_nostr::event::Timestamp as RadrootsNostrTimestamp;
use radroots_nostr::event::build_profile as build_profile_event;

#[test]
fn typed_profile_builder_preserves_the_strict_replacement_snapshot() {
    let keys = RadrootsNostrKeys::new(
        RadrootsNostrSecretKey::from_hex(test_fixtures::FIXTURE_ALICE_SECRET_KEY_HEX).unwrap(),
    );
    let created_at = RadrootsNostrTimestamp::from_secs(1_784_347_200);
    let profile = AuthoredProfile::new("Alice")
        .unwrap()
        .with_display_name("Alice's Orchard")
        .with_about("Tree fruit")
        .with_bot(false);

    let event = build_profile_event(&profile)
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

#[test]
fn typed_profile_builder_finalizes_the_same_plan_for_an_external_signer() {
    let keys = RadrootsNostrKeys::new(
        RadrootsNostrSecretKey::from_hex(test_fixtures::FIXTURE_ALICE_SECRET_KEY_HEX).unwrap(),
    );
    let created_at = RadrootsNostrTimestamp::from_secs(1_784_347_200);
    let profile = AuthoredProfile::new("Alice").unwrap().with_bot(false);
    let request = build_profile_event(&profile)
        .unwrap()
        .custom_created_at(created_at)
        .into_external_signing_request(keys.public_key())
        .expect("typed external request");
    let plan = request.authored_plan().expect("typed request plan").clone();
    assert_eq!(
        plan.body().contract().contract_id().as_str(),
        "radroots.profile.metadata.v1"
    );
    assert_eq!(plan.created_at(), created_at.as_secs());

    let unsigned: nostr::UnsignedEvent =
        serde_json::from_value(serde_json::to_value(&request).unwrap()).unwrap();
    let event = unsigned.sign_with_keys(&keys).unwrap();
    let completed = request.complete(event).expect("exact typed completion");
    assert_eq!(completed.id.to_hex(), plan.expected_event_id().to_hex());
}
