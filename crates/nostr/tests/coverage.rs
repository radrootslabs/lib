#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use std::borrow::Cow;

use nostr::{Keys as RadrootsNostrKeys, RelayUrl as RadrootsNostrRelayUrl, nips::nip04};
#[cfg(feature = "events")]
use radroots_event::post::reply::{AuthoredNip10Reply, Nip10ReplyReference};
#[cfg(feature = "events")]
use radroots_event_codec::decode::job::{JobEventBorrow, JobEventLike};
#[cfg(feature = "events")]
use radroots_nostr::event::build_nip10_reply as build_nip10_reply_event;
#[cfg(feature = "events")]
use radroots_nostr::event::{
    ApplicationHandlerSpec, EventAdapter, build_application_handler, metadata_has_fields,
    to_job_feedback_index, to_job_feedback_metadata, to_job_request_index, to_job_request_metadata,
    to_job_result_index, to_job_result_metadata,
};
use radroots_nostr::event::{Kind as RadrootsNostrKind, Timestamp as RadrootsNostrTimestamp};
use radroots_nostr::event::{
    build_job_feedback as build_event_job_feedback, build_job_result as build_event_job_result,
    created_at_u32_saturating, event_created_at_u32_saturating, post_filter as post_events_filter,
};
use radroots_nostr::filter::{
    for_kind as filter_kind, kind, since_now as filter_new_events, with_tag as filter_tag,
};
use radroots_nostr::key::{parse_public_key, public_key_from_nostr, public_key_to_npub};
use radroots_nostr::tag::{
    ResolveError, Tag as RadrootsNostrTag, TagKind as RadrootsNostrTagKind,
    TagStandard as RadrootsNostrTagStandard, first_value as tag_first_value,
    match_geohash as tag_match_geohash, match_location as tag_match_location,
    match_location_coordinate as tag_match_l, match_parts as tags_match,
    match_summary as tag_match_summary, match_title as tag_match_title,
    relay_urls as tag_relays_parse, resolve as tags_resolve, value_at as tag_at_value,
    values_from as tag_slice,
};
use test_fixtures::RELAY_PRIMARY_WSS;

fn make_keys() -> RadrootsNostrKeys {
    RadrootsNostrKeys::generate()
}

fn text_event_with_tags(keys: &RadrootsNostrKeys, tags: Vec<RadrootsNostrTag>) -> nostr::Event {
    nostr::EventBuilder::new(RadrootsNostrKind::TextNote, "content")
        .tags(tags)
        .sign_with_keys(keys)
        .expect("sign event")
}

fn encrypted_event_with_p_tag(
    sender_keys: &RadrootsNostrKeys,
    content: impl Into<String>,
    recipient_hex: &str,
) -> nostr::Event {
    nostr::EventBuilder::new(RadrootsNostrKind::TextNote, content.into())
        .tags(vec![
            RadrootsNostrTag::custom(
                RadrootsNostrTagKind::Encrypted,
                vec!["encrypted".to_string()],
            ),
            RadrootsNostrTag::custom(RadrootsNostrTagKind::p(), vec![recipient_hex.to_string()]),
        ])
        .sign_with_keys(sender_keys)
        .expect("sign encrypted event")
}

#[test]
fn job_event_builders_are_callable() {
    let keys = make_keys();
    let job_request = nostr::EventBuilder::new(RadrootsNostrKind::Custom(5001), "job")
        .sign_with_keys(&keys)
        .expect("job request");
    let non_job_request = nostr::EventBuilder::new(RadrootsNostrKind::TextNote, "job")
        .sign_with_keys(&keys)
        .expect("non-job request");

    let job_result = build_event_job_result(
        &job_request,
        "ok",
        1,
        Some("bolt11".to_string()),
        Some(Vec::new()),
    )
    .expect("job result builder");
    let _ = job_result
        .sign_with_keys(&keys)
        .expect("job result signs through the generic boundary");

    let feedback_ok = build_event_job_feedback(
        &job_request,
        "success",
        Some("extra".to_string()),
        Some(Vec::new()),
    )
    .expect("job feedback builder");
    let _ = feedback_ok
        .sign_with_keys(&keys)
        .expect("job feedback signs through the generic boundary");

    let feedback_invalid = build_event_job_feedback(&job_request, "invalid-status", None, None)
        .expect("job feedback fallback builder");
    let _ = feedback_invalid
        .sign_with_keys(&keys)
        .expect("fallback job feedback signs through the generic boundary");

    let invalid_job_result = build_event_job_result(
        &non_job_request,
        "ok",
        1,
        Some("bolt11".to_string()),
        Some(Vec::new()),
    );
    assert!(invalid_job_result.is_err());
}

#[test]
fn post_helpers_cover_success_and_error_paths() {
    let _ = post_events_filter(None, None);
    let _ = post_events_filter(Some(10), Some(1_700_000_000));

    #[cfg(feature = "events")]
    {
        let keys = make_keys();
        let root = nostr::EventBuilder::text_note("root")
            .sign_with_keys(&keys)
            .expect("root");
        let parent = nostr::EventBuilder::text_note("parent")
            .sign_with_keys(&keys)
            .expect("parent");
        let author_hex = root.pubkey.to_hex();

        let root_reference =
            Nip10ReplyReference::parse(root.id.to_hex(), &author_hex, Some(RELAY_PRIMARY_WSS))
                .expect("root reference");
        let direct = AuthoredNip10Reply::direct("direct reply", root_reference.clone())
            .expect("direct reply");
        let direct_builder = build_nip10_reply_event(&direct).expect("direct reply builder");
        let _ = direct_builder
            .sign_with_keys(&keys)
            .expect("direct reply signs through the typed boundary");

        let parent_reference = Nip10ReplyReference::parse(parent.id.to_hex(), &author_hex, None)
            .expect("parent reference");
        let nested = AuthoredNip10Reply::nested("nested reply", root_reference, parent_reference)
            .expect("nested reply");
        let nested_builder = build_nip10_reply_event(&nested).expect("nested reply builder");
        let _ = nested_builder
            .sign_with_keys(&keys)
            .expect("nested reply signs through the typed boundary");

        assert!(Nip10ReplyReference::parse("bad", &author_hex, None).is_err());
        assert!(Nip10ReplyReference::parse(root.id.to_hex(), "bad", None).is_err());
        assert!(
            Nip10ReplyReference::parse(
                root.id.to_hex(),
                &author_hex,
                Some("https://relay.example"),
            )
            .is_err()
        );
        assert!(AuthoredNip10Reply::direct(" ", nested.root().clone()).is_err());
    }
}

#[test]
fn filter_helpers_cover_all_paths() {
    let filter = filter_kind(1);
    let filtered = filter_tag(filter, "p", vec!["x".to_string()]);
    assert!(filtered.is_ok());

    let empty_tag = filter_tag(filter_kind(1), "", vec!["x".to_string()]);
    assert!(empty_tag.is_err());

    let multi_tag = filter_tag(filter_kind(1), "pp", vec!["x".to_string()]);
    assert!(multi_tag.is_err());

    let invalid_tag = filter_tag(filter_kind(1), "1", vec!["x".to_string()]);
    assert!(invalid_tag.is_err());

    let _ = kind(30000);
    let _ = filter_new_events(filter_kind(1));
}

#[test]
fn parse_helpers_cover_success_and_failure() {
    let keys = make_keys();
    let pubkey_hex = keys.public_key().to_hex();
    let ok = parse_public_key(pubkey_hex.as_str());
    assert!(ok.is_ok());

    let invalid = parse_public_key("invalid");
    assert!(invalid.is_err());

    let npub = public_key_to_npub(ok.expect("public key")).expect("npub");
    assert!(parse_public_key(&npub).is_ok());
}

#[test]
fn tag_helpers_cover_matchers_and_resolve_paths() {
    let keys = make_keys();
    let other = make_keys();

    let custom_tag = RadrootsNostrTag::custom(
        RadrootsNostrTagKind::Custom(Cow::Borrowed("x")),
        vec!["v1".to_string(), "v2".to_string()],
    );
    assert_eq!(tag_first_value(&custom_tag, "x"), Some("v1".to_string()));
    assert_eq!(tag_first_value(&custom_tag, "y"), None);
    assert_eq!(tag_at_value(&custom_tag, 0), Some("x".to_string()));
    assert_eq!(tag_at_value(&custom_tag, 9), None);
    assert_eq!(
        tag_slice(&custom_tag, 1),
        Some(vec!["v1".to_string(), "v2".to_string()])
    );
    assert_eq!(tag_slice(&custom_tag, 9), None);
    let matched = tags_match(&custom_tag).expect("custom match");
    assert_eq!(matched.0, "x");
    assert_eq!(matched.1, ["v1".to_string(), "v2".to_string()]);

    let relays_tag = RadrootsNostrTag::from_standardized(RadrootsNostrTagStandard::Relays(vec![
        RadrootsNostrRelayUrl::parse(RELAY_PRIMARY_WSS).expect("relay"),
    ]));
    assert!(tag_relays_parse(&relays_tag).is_some());
    let relays_non_match =
        RadrootsNostrTag::from_standardized(RadrootsNostrTagStandard::Title("x".to_string()));
    assert!(tag_relays_parse(&relays_non_match).is_none());
    assert!(tag_relays_parse(&custom_tag).is_none());

    let l_tag = RadrootsNostrTag::custom(
        RadrootsNostrTagKind::Custom(Cow::Borrowed("l")),
        vec!["12.5".to_string(), "kg".to_string()],
    );
    assert_eq!(tag_match_l(&l_tag), Some(("kg", 12.5)));
    let bad_l_tag = RadrootsNostrTag::custom(
        RadrootsNostrTagKind::Custom(Cow::Borrowed("l")),
        vec!["abc".to_string(), "kg".to_string()],
    );
    assert_eq!(tag_match_l(&bad_l_tag), None);
    assert_eq!(tag_match_l(&custom_tag), None);
    let short_l_tag = RadrootsNostrTag::custom(
        RadrootsNostrTagKind::Custom(Cow::Borrowed("l")),
        vec!["12.5".to_string()],
    );
    assert_eq!(tag_match_l(&short_l_tag), None);

    let location_tag = RadrootsNostrTag::custom(
        RadrootsNostrTagKind::Custom(Cow::Borrowed("location")),
        vec![
            "se".to_string(),
            "stockholm".to_string(),
            "city".to_string(),
        ],
    );
    assert_eq!(
        tag_match_location(&location_tag),
        Some(("se", "stockholm", "city"))
    );
    let location_non_match = RadrootsNostrTag::custom(
        RadrootsNostrTagKind::Custom(Cow::Borrowed("x")),
        vec![
            "se".to_string(),
            "stockholm".to_string(),
            "city".to_string(),
        ],
    );
    assert_eq!(tag_match_location(&location_non_match), None);
    assert_eq!(tag_match_location(&custom_tag), None);

    let geohash_tag =
        RadrootsNostrTag::from_standardized(RadrootsNostrTagStandard::Geohash("u4pr".to_string()));
    assert_eq!(tag_match_geohash(&geohash_tag), Some("u4pr".to_string()));
    let title_tag =
        RadrootsNostrTag::from_standardized(RadrootsNostrTagStandard::Title("title".to_string()));
    assert_eq!(tag_match_geohash(&title_tag), None);
    assert_eq!(tag_match_geohash(&custom_tag), None);

    assert_eq!(tag_match_title(&title_tag), Some("title".to_string()));
    let summary_tag = RadrootsNostrTag::from_standardized(RadrootsNostrTagStandard::Summary(
        "summary".to_string(),
    ));
    assert_eq!(tag_match_title(&summary_tag), None);
    assert_eq!(tag_match_title(&custom_tag), None);

    assert_eq!(tag_match_summary(&summary_tag), Some("summary".to_string()));
    assert_eq!(tag_match_summary(&geohash_tag), None);
    assert_eq!(tag_match_summary(&custom_tag), None);

    let clear_event = text_event_with_tags(
        &keys,
        vec![RadrootsNostrTag::custom(
            RadrootsNostrTagKind::Custom(Cow::Borrowed("x")),
            vec!["x".to_string(), "v".to_string()],
        )],
    );
    let resolved = tags_resolve(&clear_event, &keys).expect("clear tags");
    assert_eq!(resolved.len(), 1);

    let encrypted_missing_p = text_event_with_tags(
        &keys,
        vec![RadrootsNostrTag::custom(
            RadrootsNostrTagKind::Encrypted,
            vec!["encrypted".to_string()],
        )],
    );
    let missing_p = tags_resolve(&encrypted_missing_p, &keys);
    assert!(matches!(missing_p, Err(ResolveError::MissingPTag(_))));

    let sender = make_keys();
    let encrypted_invalid_p = encrypted_event_with_p_tag(&sender, "cipher", "not-a-pubkey");
    let invalid_p = tags_resolve(&encrypted_invalid_p, &keys);
    assert!(matches!(invalid_p, Err(ResolveError::MissingPTag(_))));

    let encrypted_empty_p_content = nostr::EventBuilder::new(RadrootsNostrKind::TextNote, "cipher")
        .tags(vec![
            RadrootsNostrTag::custom(
                RadrootsNostrTagKind::Encrypted,
                vec!["encrypted".to_string()],
            ),
            RadrootsNostrTag::custom(RadrootsNostrTagKind::p(), Vec::<String>::new()),
        ])
        .sign_with_keys(&sender)
        .expect("sign encrypted event with empty p tag");
    let empty_p_content = tags_resolve(&encrypted_empty_p_content, &keys);
    assert!(matches!(empty_p_content, Err(ResolveError::MissingPTag(_))));

    let encrypted_not_recipient =
        encrypted_event_with_p_tag(&sender, "cipher", &other.public_key().to_hex());
    let not_recipient = tags_resolve(&encrypted_not_recipient, &keys);
    assert!(matches!(not_recipient, Err(ResolveError::NotRecipient)));

    let encrypted_bad_cipher =
        encrypted_event_with_p_tag(&sender, "not-ciphertext", &keys.public_key().to_hex());
    let bad_cipher = tags_resolve(&encrypted_bad_cipher, &keys);
    assert!(matches!(bad_cipher, Err(ResolveError::DecryptionError(_))));

    let encrypted_cleartext = nip04::encrypt(sender.secret_key(), &keys.public_key(), "[]")
        .expect("encrypt cleartext tags");
    let encrypted_ok =
        encrypted_event_with_p_tag(&sender, encrypted_cleartext, &keys.public_key().to_hex());
    let resolved_encrypted = tags_resolve(&encrypted_ok, &keys).expect("resolve tags");
    assert!(resolved_encrypted.is_empty());

    let encrypted_bad_json = nip04::encrypt(sender.secret_key(), &keys.public_key(), "not-json")
        .expect("encrypt invalid tags payload");
    let encrypted_bad_json_event =
        encrypted_event_with_p_tag(&sender, encrypted_bad_json, &keys.public_key().to_hex());
    let bad_json = tags_resolve(&encrypted_bad_json_event, &keys);
    assert!(matches!(bad_json, Err(ResolveError::ParseError(_))));
}

#[test]
fn util_helpers_cover_conversion_paths() {
    let keys = make_keys();
    let native = public_key_from_nostr(keys.public_key()).expect("native public key");
    let npub = public_key_to_npub(native).expect("npub");
    assert!(npub.starts_with("npub1"));

    let max = RadrootsNostrTimestamp::from(u64::from(u32::MAX));
    let overflow = RadrootsNostrTimestamp::from(u64::from(u32::MAX) + 1);
    assert_eq!(created_at_u32_saturating(max), u32::MAX);
    assert_eq!(created_at_u32_saturating(overflow), u32::MAX);

    let event = text_event_with_tags(&keys, Vec::new());
    let _ = event_created_at_u32_saturating(&event);
}

#[cfg(feature = "events")]
#[test]
fn event_and_job_adapters_cover_native_value_boundaries() {
    let keys = make_keys();
    let event = nostr::EventBuilder::new(RadrootsNostrKind::Custom(5_001), "job")
        .tags(vec![RadrootsNostrTag::custom(
            RadrootsNostrTagKind::Custom(Cow::Borrowed("i")),
            vec!["input".to_string()],
        )])
        .sign_with_keys(&keys)
        .unwrap();

    let adapter = EventAdapter::new(&event);
    assert_eq!(JobEventLike::raw_id(&adapter), event.id.to_hex());
    assert_eq!(JobEventLike::raw_author(&adapter), event.pubkey.to_hex());
    assert_eq!(
        JobEventLike::raw_published_at(&adapter),
        event.created_at.as_secs()
    );
    assert_eq!(JobEventLike::raw_kind(&adapter), 5_001);
    assert_eq!(JobEventLike::raw_content(&adapter), "job");
    assert_eq!(JobEventLike::raw_tags(&adapter).len(), 1);
    assert_eq!(JobEventLike::raw_sig(&adapter), event.sig.to_string());
    assert_eq!(JobEventBorrow::raw_id(&adapter), event.id.to_hex());
    assert_eq!(JobEventBorrow::raw_author(&adapter), event.pubkey.to_hex());
    assert_eq!(JobEventBorrow::raw_content(&adapter), "job");
    assert_eq!(JobEventBorrow::raw_kind(&adapter), 5_001);

    let profile_event = nostr::EventBuilder::metadata(&nostr::Metadata::new().name("Alice"))
        .sign_with_keys(&keys)
        .unwrap();
    let ordinary_adapter = EventAdapter::new(&profile_event);
    assert_eq!(JobEventLike::raw_kind(&ordinary_adapter), 0);
    assert_eq!(JobEventBorrow::raw_kind(&ordinary_adapter), 0);
    let _ = to_job_request_metadata(&event);
    let _ = to_job_request_index(&event);
    let _ = to_job_result_metadata(&event);
    let _ = to_job_result_index(&event);
    let _ = to_job_feedback_metadata(&event);
    let _ = to_job_feedback_index(&event);
}

#[cfg(feature = "events")]
#[test]
fn application_handler_builder_covers_optional_metadata_and_tag_filters() {
    assert!(build_application_handler(&ApplicationHandlerSpec::new(Vec::new())).is_err());
    let empty = nostr::Metadata::new();
    assert!(!metadata_has_fields(&empty));
    assert!(
        build_application_handler(
            &ApplicationHandlerSpec::new(vec![1]).with_metadata(empty.clone())
        )
        .is_ok()
    );
    assert!(build_application_handler(&ApplicationHandlerSpec::new(vec![1])).is_ok());
    let metadata = nostr::Metadata::new().name("Market app");
    assert!(metadata_has_fields(&metadata));
    let spec = ApplicationHandlerSpec::new(vec![1, 30_001])
        .with_identifier("market-app")
        .with_metadata(metadata)
        .with_relays(vec![" ".into(), RELAY_PRIMARY_WSS.into()])
        .with_nostr_connect_url(" nostrconnect://app ")
        .with_extra_tags(vec![Vec::new(), vec!["x".into(), "value".into()]]);
    assert_eq!(spec.kinds(), [1, 30_001]);
    assert_eq!(spec.identifier(), Some("market-app"));
    assert!(spec.metadata().is_some());
    assert_eq!(spec.extra_tags().len(), 2);
    assert_eq!(spec.relays().len(), 2);
    assert_eq!(spec.nostr_connect_url(), Some(" nostrconnect://app "));
    let builder = build_application_handler(&spec).unwrap();
    let event = builder.sign_with_keys(&make_keys()).unwrap();
    assert_eq!(event.kind, RadrootsNostrKind::Custom(31_990));
}
