use radroots_event::reply::{RadrootsAuthoredNip10Reply, RadrootsNip10ReplyReference};
use radroots_event_codec::{
    post::admission::{RadrootsPostAdmissionOutcome, verify_and_admit_post_event},
    reply::{
        admission::{admit_thread_excluded_post_candidate, verify_and_admit_nip10_reply_event},
        inbound::RadrootsNip10ReplyStyle,
    },
};
use radroots_nostr::prelude::{
    RadrootsNostrError, RadrootsNostrGenericEventBuilder, RadrootsNostrKeys, RadrootsNostrKind,
    RadrootsNostrSecretKey, RadrootsNostrTag, RadrootsNostrTimestamp, radroots_event_from_nostr,
    radroots_nostr_build_nip10_reply_event,
};
use radroots_test_fixtures::{
    FIXTURE_ALICE_SECRET_KEY_HEX, FIXTURE_BOB_PUBLIC_KEY_HEX, FIXTURE_CAROL_PUBLIC_KEY_HEX,
    RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS,
};

#[cfg(feature = "client")]
use radroots_nostr::prelude::RadrootsNostrClient;

const CREATED_AT: u64 = 1_784_347_200;
const ROOT_EVENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PARENT_EVENT_ID: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

#[test]
fn typed_nip10_reply_builders_sign_exact_marked_direct_and_nested_profiles() {
    let keys = fixture_keys();
    let created_at = RadrootsNostrTimestamp::from_secs(CREATED_AT);
    let root = reference(
        ROOT_EVENT_ID,
        FIXTURE_BOB_PUBLIC_KEY_HEX,
        Some(RELAY_PRIMARY_WSS),
    );

    let direct = RadrootsAuthoredNip10Reply::direct("Direct reply", root.clone())
        .expect("authored direct reply");
    let direct_event = radroots_nostr_build_nip10_reply_event(&direct)
        .expect("typed direct Reply builder")
        .custom_created_at(created_at)
        .sign_with_keys(&keys)
        .expect("signed direct Reply");

    assert_eq!(direct_event.kind, RadrootsNostrKind::TextNote);
    assert_eq!(direct_event.created_at, created_at);
    assert_eq!(
        event_tags(&direct_event),
        vec![
            tag(&["e", ROOT_EVENT_ID, RELAY_PRIMARY_WSS, "root"]),
            tag(&["p", FIXTURE_BOB_PUBLIC_KEY_HEX]),
        ]
    );
    direct_event.verify().expect("valid direct Reply signature");

    let parent = reference(
        PARENT_EVENT_ID,
        FIXTURE_CAROL_PUBLIC_KEY_HEX,
        Some(RELAY_SECONDARY_WSS),
    );
    let nested = RadrootsAuthoredNip10Reply::nested("Nested reply", root, parent)
        .expect("authored nested reply");
    let nested_event = radroots_nostr_build_nip10_reply_event(&nested)
        .expect("typed nested Reply builder")
        .custom_created_at(created_at)
        .sign_with_keys(&keys)
        .expect("signed nested Reply");

    assert_eq!(
        event_tags(&nested_event),
        vec![
            tag(&["e", ROOT_EVENT_ID, RELAY_PRIMARY_WSS, "root"]),
            tag(&["e", PARENT_EVENT_ID, RELAY_SECONDARY_WSS, "reply"]),
            tag(&["p", FIXTURE_BOB_PUBLIC_KEY_HEX]),
            tag(&["p", FIXTURE_CAROL_PUBLIC_KEY_HEX]),
        ]
    );
    nested_event.verify().expect("valid nested Reply signature");
    let admitted = verify_and_admit_nip10_reply_event(
        radroots_event_from_nostr(&nested_event).expect("nested Reply adapter"),
    )
    .expect("nested Reply admission");
    assert!(!admitted.projection().is_direct());
    assert_eq!(
        admitted
            .projection()
            .reply_reference()
            .expect("nested parent")
            .event_id()
            .to_hex(),
        PARENT_EVENT_ID
    );
}

#[test]
fn signed_nip10_reply_is_thread_excluded_before_semantic_reply_admission() {
    let reply = RadrootsAuthoredNip10Reply::direct(
        "Thread reply",
        reference(
            ROOT_EVENT_ID,
            FIXTURE_BOB_PUBLIC_KEY_HEX,
            Some(RELAY_PRIMARY_WSS),
        ),
    )
    .expect("authored direct reply");
    let event = radroots_nostr_build_nip10_reply_event(&reply)
        .expect("typed Reply builder")
        .custom_created_at(RadrootsNostrTimestamp::from_secs(CREATED_AT))
        .sign_with_keys(&fixture_keys())
        .expect("signed Reply");
    let envelope = radroots_event_from_nostr(&event).expect("Radroots event adapter");

    let candidate = match verify_and_admit_post_event(envelope).expect("post admission") {
        RadrootsPostAdmissionOutcome::ThreadExcluded(candidate) => candidate,
        RadrootsPostAdmissionOutcome::Root(_) => panic!("Reply must never become a root card"),
        _ => panic!("unexpected post admission outcome"),
    };
    let admitted =
        admit_thread_excluded_post_candidate(candidate).expect("semantic Reply admission");

    assert_eq!(admitted.contract().id, "radroots.social.reply.v1");
    assert_eq!(
        admitted.projection().style(),
        RadrootsNip10ReplyStyle::Marked
    );
    assert!(admitted.projection().is_direct());
    assert_eq!(
        admitted.projection().root().event_id().to_hex(),
        ROOT_EVENT_ID
    );
}

#[test]
fn verified_legacy_positional_nip10_reply_remains_tolerated_inbound() {
    let keys = fixture_keys();
    let event = nostr::EventBuilder::text_note("Legacy positional reply")
        .tags([
            RadrootsNostrTag::parse(["e", ROOT_EVENT_ID, RELAY_PRIMARY_WSS])
                .expect("positional root reference"),
            RadrootsNostrTag::parse(["p", FIXTURE_BOB_PUBLIC_KEY_HEX])
                .expect("root author reference"),
        ])
        .custom_created_at(RadrootsNostrTimestamp::from_secs(CREATED_AT))
        .sign_with_keys(&keys)
        .expect("signed inbound fixture");
    let envelope = radroots_event_from_nostr(&event).expect("Radroots event adapter");
    let admitted =
        verify_and_admit_nip10_reply_event(envelope).expect("legacy positional Reply admission");

    assert_eq!(
        admitted.projection().style(),
        RadrootsNip10ReplyStyle::LegacyPositional
    );
    assert!(admitted.projection().is_direct());
    assert_eq!(
        admitted.projection().root().event_id().to_hex(),
        ROOT_EVENT_ID
    );
}

#[test]
fn generic_kind_one_builder_cannot_bypass_typed_nip10_reply_authoring() {
    let keys = fixture_keys();
    let marked_reply = RadrootsNostrGenericEventBuilder::text_note("Raw marked reply").tags([
        RadrootsNostrTag::parse(["e", ROOT_EVENT_ID, RELAY_PRIMARY_WSS, "root"])
            .expect("root reference"),
        RadrootsNostrTag::parse(["p", FIXTURE_BOB_PUBLIC_KEY_HEX]).expect("root author reference"),
    ]);

    for builder in [
        RadrootsNostrGenericEventBuilder::text_note("Raw root"),
        marked_reply,
    ] {
        assert!(matches!(
            builder.sign_with_keys(&keys),
            Err(RadrootsNostrError::TypedAuthoringRequired { kind })
                if kind == RadrootsNostrKind::TextNote.as_u16()
        ));
    }
}

#[cfg(feature = "client")]
#[tokio::test]
async fn typed_nip10_reply_builder_reaches_client_publication() {
    let client = RadrootsNostrClient::new(fixture_keys());
    let reply = RadrootsAuthoredNip10Reply::direct(
        "Publish Reply",
        reference(
            ROOT_EVENT_ID,
            FIXTURE_BOB_PUBLIC_KEY_HEX,
            Some(RELAY_PRIMARY_WSS),
        ),
    )
    .expect("authored direct reply");
    let builder =
        radroots_nostr_build_nip10_reply_event(&reply).expect("typed NIP-10 Reply builder");

    let error = client
        .send_nip10_reply_event_builder(builder)
        .await
        .expect_err("no relay is configured");

    assert!(matches!(error, RadrootsNostrError::ClientError(_)));
}

fn fixture_keys() -> RadrootsNostrKeys {
    RadrootsNostrKeys::new(
        RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("fixture secret key"),
    )
}

fn reference(event_id: &str, author: &str, relay: Option<&str>) -> RadrootsNip10ReplyReference {
    RadrootsNip10ReplyReference::parse(event_id, author, relay).expect("valid Reply reference")
}

fn event_tags(event: &nostr::Event) -> Vec<Vec<String>> {
    event
        .tags
        .iter()
        .map(|tag| tag.as_slice().to_vec())
        .collect()
}

fn tag(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}
