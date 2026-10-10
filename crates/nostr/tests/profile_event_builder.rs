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

#[test]
fn profile_complete_edit_external_signing_request_completes_exact_preserved_replacement() {
    use nostr::JsonUtil;
    use radroots_event::{
        profile::edit::{ProfileEdit, ProfileFieldChange},
        wire::Nip01EventWire,
    };
    use radroots_event_codec::{
        admission::profile::verify_and_admit_profile_event, authoring::AuthoredEventPlan,
    };
    use radroots_nostr::event::ExternalSigningRequest;
    let keys = RadrootsNostrKeys::new(
        RadrootsNostrSecretKey::from_hex(test_fixtures::FIXTURE_ALICE_SECRET_KEY_HEX).unwrap(),
    );
    let signed = nostr::EventBuilder::new(
        nostr::Kind::Metadata,
        r#"{"name":"farm","website":"https://farm.example","custom":{"x":1}}"#,
    )
    .custom_created_at(RadrootsNostrTimestamp::from_secs(100))
    .sign_with_keys(&keys)
    .unwrap();
    let baseline = verify_and_admit_profile_event(
        Nip01EventWire::parse_json(&signed.as_json())
            .unwrap()
            .into_envelope()
            .unwrap(),
    )
    .unwrap();
    let plan = AuthoredEventPlan::from_profile_edit(
        &baseline,
        &baseline,
        &ProfileEdit {
            display_name: ProfileFieldChange::Set("Updated farm".into()),
            ..Default::default()
        },
        101,
        keys.public_key().to_hex(),
    )
    .unwrap();
    let request = ExternalSigningRequest::from_authored_plan(plan.clone()).unwrap();
    assert_eq!(request.authored_plan(), Some(&plan));
    let unsigned: nostr::UnsignedEvent =
        serde_json::from_value(serde_json::to_value(&request).unwrap()).unwrap();
    let completed = request
        .complete(unsigned.sign_with_keys(&keys).unwrap())
        .unwrap();
    assert_eq!(completed.id.to_hex(), plan.expected_event_id().to_hex());
    let admitted = verify_and_admit_profile_event(
        Nip01EventWire::parse_json(&completed.as_json())
            .unwrap()
            .into_envelope()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(admitted.metadata().display_name(), Some("Updated farm"));
    assert_eq!(
        admitted.metadata().raw_fields()["website"],
        "https://farm.example"
    );
    assert_eq!(
        admitted.metadata().raw_fields()["custom"],
        serde_json::json!({"x":1})
    );
}
