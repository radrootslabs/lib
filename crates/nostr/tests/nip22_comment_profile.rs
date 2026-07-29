use radroots_event::{
    envelope::kind::{
        KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_TIME_EVENT, KIND_CLASSIFIED_LISTING, KIND_COMMENT,
    },
    post::comment::{
        RadrootsAuthoredNip22Comment, RadrootsNip22AddressRootReference,
        RadrootsNip22CommentParentReference, RadrootsNip22EventRootReference,
    },
};
use radroots_event_codec::comment::{
    admission::verify_and_admit_nip22_comment_event, inbound::RadrootsInboundNip22CommentPosition,
};
use radroots_nostr::prelude::{
    RadrootsNostrError, RadrootsNostrGenericEventBuilder, RadrootsNostrKeys, RadrootsNostrKind,
    RadrootsNostrSecretKey, RadrootsNostrTimestamp, radroots_event_from_nostr,
    radroots_nostr_build_nip22_comment_event,
};
use radroots_test_fixtures::{
    FIXTURE_ALICE_SECRET_KEY_HEX, FIXTURE_BOB_PUBLIC_KEY_HEX, FIXTURE_CAROL_PUBLIC_KEY_HEX,
    RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS,
};

#[cfg(feature = "client")]
use radroots_nostr::prelude::RadrootsNostrClient;

const CREATED_AT: u64 = 1_784_347_200;
const ROOT_EVENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ADDRESS_REVISION_ID: &str =
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const NESTED_ROOT_EVENT_ID: &str =
    "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const PARENT_EVENT_ID: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

#[test]
fn typed_nip22_comment_builders_sign_and_admit_all_exact_shapes() {
    let keys = fixture_keys();
    let created_at = RadrootsNostrTimestamp::from_secs(CREATED_AT);

    let top_event = RadrootsAuthoredNip22Comment::top_level_event(
        "Are these carrots available Saturday?",
        RadrootsNip22EventRootReference::parse(
            ROOT_EVENT_ID,
            FIXTURE_BOB_PUBLIC_KEY_HEX,
            KIND_CLASSIFIED_LISTING,
            Some(RELAY_PRIMARY_WSS),
        )
        .expect("event root"),
    )
    .expect("top-level event Comment");
    let top_event_tags = vec![
        tag(&[
            "E",
            ROOT_EVENT_ID,
            RELAY_PRIMARY_WSS,
            FIXTURE_BOB_PUBLIC_KEY_HEX,
        ]),
        tag(&["K", "30402"]),
        tag(&["P", FIXTURE_BOB_PUBLIC_KEY_HEX, RELAY_PRIMARY_WSS]),
        tag(&[
            "e",
            ROOT_EVENT_ID,
            RELAY_PRIMARY_WSS,
            FIXTURE_BOB_PUBLIC_KEY_HEX,
        ]),
        tag(&["k", "30402"]),
        tag(&["p", FIXTURE_BOB_PUBLIC_KEY_HEX, RELAY_PRIMARY_WSS]),
    ];

    let address =
        format!("{KIND_CALENDAR_DATE_EVENT}:{FIXTURE_BOB_PUBLIC_KEY_HEX}:victoria-market");
    let top_address = RadrootsAuthoredNip22Comment::parse_top_level_address(
        "Looking forward to the Victoria market.",
        RadrootsNip22AddressRootReference::parse(&address, Some(RELAY_SECONDARY_WSS))
            .expect("address root"),
        ADDRESS_REVISION_ID,
    )
    .expect("top-level address Comment");
    let top_address_tags = vec![
        tag(&["A", &address, RELAY_SECONDARY_WSS]),
        tag(&["K", "31922"]),
        tag(&["P", FIXTURE_BOB_PUBLIC_KEY_HEX, RELAY_SECONDARY_WSS]),
        tag(&["a", &address, RELAY_SECONDARY_WSS]),
        tag(&["e", ADDRESS_REVISION_ID, RELAY_SECONDARY_WSS]),
        tag(&["k", "31922"]),
        tag(&["p", FIXTURE_BOB_PUBLIC_KEY_HEX, RELAY_SECONDARY_WSS]),
    ];

    let nested_event = RadrootsAuthoredNip22Comment::nested(
        "Harvest starts Friday morning.",
        RadrootsNip22EventRootReference::parse(
            NESTED_ROOT_EVENT_ID,
            FIXTURE_BOB_PUBLIC_KEY_HEX,
            KIND_CALENDAR_TIME_EVENT,
            None,
        )
        .expect("nested event root"),
        parent(Some(RELAY_SECONDARY_WSS)),
    )
    .expect("nested event Comment");
    let nested_event_tags = vec![
        tag(&["E", NESTED_ROOT_EVENT_ID, "", FIXTURE_BOB_PUBLIC_KEY_HEX]),
        tag(&["K", "31923"]),
        tag(&["P", FIXTURE_BOB_PUBLIC_KEY_HEX]),
        tag(&[
            "e",
            PARENT_EVENT_ID,
            RELAY_SECONDARY_WSS,
            FIXTURE_CAROL_PUBLIC_KEY_HEX,
        ]),
        tag(&["k", "1111"]),
        tag(&["p", FIXTURE_CAROL_PUBLIC_KEY_HEX, RELAY_SECONDARY_WSS]),
    ];

    let nested_address_value =
        format!("{KIND_CLASSIFIED_LISTING}:{FIXTURE_BOB_PUBLIC_KEY_HEX}:carrots");
    let nested_address = RadrootsAuthoredNip22Comment::nested(
        "I can pick up two bunches.",
        RadrootsNip22AddressRootReference::parse(&nested_address_value, Some(RELAY_PRIMARY_WSS))
            .expect("nested address root"),
        parent(None),
    )
    .expect("nested address Comment");
    let nested_address_tags = vec![
        tag(&["A", &nested_address_value, RELAY_PRIMARY_WSS]),
        tag(&["K", "30402"]),
        tag(&["P", FIXTURE_BOB_PUBLIC_KEY_HEX, RELAY_PRIMARY_WSS]),
        tag(&["e", PARENT_EVENT_ID, "", FIXTURE_CAROL_PUBLIC_KEY_HEX]),
        tag(&["k", "1111"]),
        tag(&["p", FIXTURE_CAROL_PUBLIC_KEY_HEX]),
    ];

    for (comment, expected_tags, expected_position) in [
        (top_event, top_event_tags, "top_event"),
        (top_address, top_address_tags, "top_address"),
        (nested_event, nested_event_tags, "nested"),
        (nested_address, nested_address_tags, "nested"),
    ] {
        let event = radroots_nostr_build_nip22_comment_event(&comment)
            .expect("typed Comment builder")
            .custom_created_at(created_at)
            .sign_with_keys(&keys)
            .expect("signed Comment");
        assert_eq!(event.kind.as_u16(), KIND_COMMENT as u16);
        assert_eq!(event.created_at, created_at);
        assert_eq!(event_tags(&event), expected_tags);
        event.verify().expect("valid Comment signature");

        let admitted = verify_and_admit_nip22_comment_event(
            radroots_event_from_nostr(&event).expect("Comment event adapter"),
        )
        .expect("Comment admission");
        assert_eq!(
            position_label(admitted.projection().position()),
            expected_position
        );
    }
}

#[test]
fn generic_kind_1111_builder_cannot_bypass_typed_comment_authoring() {
    let kind = RadrootsNostrKind::Custom(KIND_COMMENT as u16);
    let error = RadrootsNostrGenericEventBuilder::new(kind, "Raw Comment")
        .sign_with_keys(&fixture_keys())
        .expect_err("generic kind 1111 must be reserved");

    assert!(matches!(
        error,
        RadrootsNostrError::TypedAuthoringRequired { kind: actual }
            if actual == KIND_COMMENT as u16
    ));
}

#[cfg(feature = "client")]
#[tokio::test]
async fn typed_nip22_comment_builder_reaches_client_publication() {
    let client = RadrootsNostrClient::new(fixture_keys());
    let comment = RadrootsAuthoredNip22Comment::top_level_event(
        "Publish Comment",
        RadrootsNip22EventRootReference::parse(
            ROOT_EVENT_ID,
            FIXTURE_BOB_PUBLIC_KEY_HEX,
            KIND_CLASSIFIED_LISTING,
            None,
        )
        .expect("event root"),
    )
    .expect("Comment");
    let builder =
        radroots_nostr_build_nip22_comment_event(&comment).expect("typed Comment builder");

    let error = client
        .send_nip22_comment_event_builder(builder)
        .await
        .expect_err("no relay is configured");

    assert!(matches!(error, RadrootsNostrError::ClientError(_)));
}

fn fixture_keys() -> RadrootsNostrKeys {
    RadrootsNostrKeys::new(
        RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("fixture secret key"),
    )
}

fn parent(relay: Option<&str>) -> RadrootsNip22CommentParentReference {
    RadrootsNip22CommentParentReference::parse(PARENT_EVENT_ID, FIXTURE_CAROL_PUBLIC_KEY_HEX, relay)
        .expect("Comment parent")
}

fn event_tags(event: &nostr::Event) -> Vec<Vec<String>> {
    event
        .tags
        .iter()
        .map(|tag| tag.as_slice().to_vec())
        .collect()
}

fn position_label(position: &RadrootsInboundNip22CommentPosition) -> &'static str {
    match position {
        RadrootsInboundNip22CommentPosition::TopLevelEvent { .. } => "top_event",
        RadrootsInboundNip22CommentPosition::TopLevelAddress { .. } => "top_address",
        RadrootsInboundNip22CommentPosition::Nested { .. } => "nested",
    }
}

fn tag(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}
