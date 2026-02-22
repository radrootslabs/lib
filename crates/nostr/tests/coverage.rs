use std::borrow::Cow;

use nostr::nips::nip04;
use radroots_nostr::error::RadrootsNostrTagsResolveError;
use radroots_nostr::events::jobs::{
    radroots_nostr_build_event_job_feedback, radroots_nostr_build_event_job_result,
};
use radroots_nostr::events::metadata::radroots_nostr_build_metadata_event;
use radroots_nostr::events::post::{
    radroots_nostr_build_post_event, radroots_nostr_build_post_reply_event,
    radroots_nostr_post_events_filter,
};
use radroots_nostr::events::radroots_nostr_build_event;
use radroots_nostr::filter::{
    radroots_nostr_filter_kind, radroots_nostr_filter_new_events, radroots_nostr_filter_tag,
    radroots_nostr_kind,
};
use radroots_nostr::parse::{radroots_nostr_parse_pubkey, radroots_nostr_parse_pubkeys};
use radroots_nostr::tags::{
    radroots_nostr_tag_at_value, radroots_nostr_tag_first_value, radroots_nostr_tag_match_geohash,
    radroots_nostr_tag_match_l, radroots_nostr_tag_match_location,
    radroots_nostr_tag_match_summary, radroots_nostr_tag_match_title,
    radroots_nostr_tag_relays_parse, radroots_nostr_tag_slice, radroots_nostr_tags_match,
    radroots_nostr_tags_resolve,
};
use radroots_nostr::types::{
    RadrootsNostrEventBuilder, RadrootsNostrKeys, RadrootsNostrKind, RadrootsNostrMetadata,
    RadrootsNostrRelayUrl, RadrootsNostrTag, RadrootsNostrTagKind, RadrootsNostrTagStandard,
    RadrootsNostrTimestamp,
};
use radroots_nostr::util::{
    created_at_u32_saturating, event_created_at_u32_saturating, radroots_nostr_npub_string,
};

fn make_keys() -> RadrootsNostrKeys {
    RadrootsNostrKeys::generate()
}

fn text_event_with_tags(keys: &RadrootsNostrKeys, tags: Vec<RadrootsNostrTag>) -> nostr::Event {
    RadrootsNostrEventBuilder::new(RadrootsNostrKind::TextNote, "content")
        .tags(tags)
        .sign_with_keys(keys)
        .expect("sign event")
}

fn encrypted_event_with_p_tag(
    sender_keys: &RadrootsNostrKeys,
    content: impl Into<String>,
    recipient_hex: &str,
) -> nostr::Event {
    RadrootsNostrEventBuilder::new(RadrootsNostrKind::TextNote, content.into())
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
fn build_event_skips_empty_tag_slices() {
    let keys = make_keys();
    let pubkey_hex = keys.public_key().to_hex();
    let builder = radroots_nostr_build_event(
        1,
        "test",
        vec![vec![], vec!["p".to_string(), pubkey_hex.clone()]],
    )
    .expect("builder");
    let event = builder.build(keys.public_key());
    let has_self_p_tag = event.tags.iter().any(|tag| {
        tag.kind() == RadrootsNostrTagKind::p() && tag.content() == Some(pubkey_hex.as_str())
    });
    assert!(has_self_p_tag);

    let builder_string = radroots_nostr_build_event(
        1,
        String::from("test"),
        vec![vec![], vec!["x".to_string(), "v".to_string()]],
    )
    .expect("builder string");
    let event_string = builder_string.build(keys.public_key());
    assert_eq!(event_string.tags.len(), 1);
}

#[test]
fn job_event_builders_are_callable() {
    let keys = make_keys();
    let job_request = RadrootsNostrEventBuilder::new(RadrootsNostrKind::Custom(5001), "job")
        .sign_with_keys(&keys)
        .expect("job request");
    let non_job_request = RadrootsNostrEventBuilder::new(RadrootsNostrKind::TextNote, "job")
        .sign_with_keys(&keys)
        .expect("non-job request");

    let job_result = radroots_nostr_build_event_job_result(
        &job_request,
        "ok",
        1,
        Some("bolt11".to_string()),
        Some(Vec::new()),
    )
    .expect("job result builder");
    let _ = job_result.build(keys.public_key());

    let feedback_ok = radroots_nostr_build_event_job_feedback(
        &job_request,
        "success",
        Some("extra".to_string()),
        Some(Vec::new()),
    )
    .expect("job feedback builder");
    let _ = feedback_ok.build(keys.public_key());

    let feedback_invalid =
        radroots_nostr_build_event_job_feedback(&job_request, "invalid-status", None, None)
            .expect("job feedback fallback builder");
    let _ = feedback_invalid.build(keys.public_key());

    let invalid_job_result = radroots_nostr_build_event_job_result(
        &non_job_request,
        "ok",
        1,
        Some("bolt11".to_string()),
        Some(Vec::new()),
    );
    assert!(invalid_job_result.is_err());
}

#[test]
fn metadata_builder_is_callable() {
    let keys = make_keys();
    let metadata = RadrootsNostrMetadata::default();
    let builder = radroots_nostr_build_metadata_event(&metadata);
    let _ = builder.build(keys.public_key());
}

#[test]
fn post_helpers_cover_success_and_error_paths() {
    let keys = make_keys();
    let parent = text_event_with_tags(&keys, Vec::new());
    let parent_id_hex = parent.id.to_hex();
    let author_hex = parent.pubkey.to_hex();
    let root_id_hex = parent.id.to_hex();

    let post_builder = radroots_nostr_build_post_event("hello");
    let _ = post_builder.build(keys.public_key());

    let _ = radroots_nostr_post_events_filter(None, None);
    let _ = radroots_nostr_post_events_filter(Some(10), Some(1_700_000_000));

    let reply_ok = radroots_nostr_build_post_reply_event(
        &parent_id_hex,
        &author_hex,
        "reply",
        Some(root_id_hex.as_str()),
    )
    .expect("reply event builder");
    let _ = reply_ok.build(keys.public_key());

    let reply_invalid_root = radroots_nostr_build_post_reply_event(
        &parent_id_hex,
        &author_hex,
        "reply",
        Some("not-hex-root"),
    )
    .expect("reply builder with invalid optional root");
    let _ = reply_invalid_root.build(keys.public_key());
    let reply_empty_root =
        radroots_nostr_build_post_reply_event(&parent_id_hex, &author_hex, "reply", Some(""))
            .expect("reply builder with empty optional root");
    let _ = reply_empty_root.build(keys.public_key());
    let reply_none_root =
        radroots_nostr_build_post_reply_event(&parent_id_hex, &author_hex, "reply", None)
            .expect("reply builder without optional root");
    let _ = reply_none_root.build(keys.public_key());

    let invalid_parent = radroots_nostr_build_post_reply_event("bad", &author_hex, "reply", None);
    assert!(invalid_parent.is_err());

    let invalid_author =
        radroots_nostr_build_post_reply_event(&parent_id_hex, "bad", "reply", None);
    assert!(invalid_author.is_err());
}

#[test]
fn filter_helpers_cover_all_paths() {
    let filter = radroots_nostr_filter_kind(1);
    let filtered = radroots_nostr_filter_tag(filter, "p", vec!["x".to_string()]);
    assert!(filtered.is_ok());

    let empty_tag =
        radroots_nostr_filter_tag(radroots_nostr_filter_kind(1), "", vec!["x".to_string()]);
    assert!(empty_tag.is_err());

    let multi_tag =
        radroots_nostr_filter_tag(radroots_nostr_filter_kind(1), "pp", vec!["x".to_string()]);
    assert!(multi_tag.is_err());

    let invalid_tag =
        radroots_nostr_filter_tag(radroots_nostr_filter_kind(1), "1", vec!["x".to_string()]);
    assert!(invalid_tag.is_err());

    let _ = radroots_nostr_kind(30000);
    let _ = radroots_nostr_filter_new_events(radroots_nostr_filter_kind(1));
}

#[test]
fn parse_helpers_cover_success_and_failure() {
    let keys = make_keys();
    let pubkey_hex = keys.public_key().to_hex();
    let ok = radroots_nostr_parse_pubkey(pubkey_hex.as_str());
    assert!(ok.is_ok());

    let invalid = radroots_nostr_parse_pubkey("invalid");
    assert!(invalid.is_err());

    let parsed = radroots_nostr_parse_pubkeys(&[pubkey_hex.clone()]);
    assert!(parsed.is_ok());

    let parse_err = radroots_nostr_parse_pubkeys(&[pubkey_hex, "invalid".to_string()]);
    assert!(parse_err.is_err());
}

#[test]
fn tag_helpers_cover_matchers_and_resolve_paths() {
    let keys = make_keys();
    let other = make_keys();

    let custom_tag = RadrootsNostrTag::custom(
        RadrootsNostrTagKind::Custom(Cow::Borrowed("x")),
        vec!["v1".to_string(), "v2".to_string()],
    );
    assert_eq!(
        radroots_nostr_tag_first_value(&custom_tag, "x"),
        Some("v1".to_string())
    );
    assert_eq!(radroots_nostr_tag_first_value(&custom_tag, "y"), None);
    assert_eq!(
        radroots_nostr_tag_at_value(&custom_tag, 0),
        Some("x".to_string())
    );
    assert_eq!(radroots_nostr_tag_at_value(&custom_tag, 9), None);
    assert_eq!(
        radroots_nostr_tag_slice(&custom_tag, 1),
        Some(vec!["v1".to_string(), "v2".to_string()])
    );
    assert_eq!(radroots_nostr_tag_slice(&custom_tag, 9), None);
    let matched = radroots_nostr_tags_match(&custom_tag).expect("custom match");
    assert_eq!(matched.0, "x");
    assert_eq!(matched.1, ["v1".to_string(), "v2".to_string()]);

    let relays_tag = RadrootsNostrTag::from_standardized(RadrootsNostrTagStandard::Relays(vec![
        RadrootsNostrRelayUrl::parse("wss://relay.example.com").expect("relay"),
    ]));
    assert!(radroots_nostr_tag_relays_parse(&relays_tag).is_some());
    let relays_non_match =
        RadrootsNostrTag::from_standardized(RadrootsNostrTagStandard::Title("x".to_string()));
    assert!(radroots_nostr_tag_relays_parse(&relays_non_match).is_none());
    assert!(radroots_nostr_tag_relays_parse(&custom_tag).is_none());

    let l_tag = RadrootsNostrTag::custom(
        RadrootsNostrTagKind::Custom(Cow::Borrowed("l")),
        vec!["12.5".to_string(), "kg".to_string()],
    );
    assert_eq!(radroots_nostr_tag_match_l(&l_tag), Some(("kg", 12.5)));
    let bad_l_tag = RadrootsNostrTag::custom(
        RadrootsNostrTagKind::Custom(Cow::Borrowed("l")),
        vec!["abc".to_string(), "kg".to_string()],
    );
    assert_eq!(radroots_nostr_tag_match_l(&bad_l_tag), None);
    assert_eq!(radroots_nostr_tag_match_l(&custom_tag), None);
    let short_l_tag = RadrootsNostrTag::custom(
        RadrootsNostrTagKind::Custom(Cow::Borrowed("l")),
        vec!["12.5".to_string()],
    );
    assert_eq!(radroots_nostr_tag_match_l(&short_l_tag), None);

    let location_tag = RadrootsNostrTag::custom(
        RadrootsNostrTagKind::Custom(Cow::Borrowed("location")),
        vec![
            "se".to_string(),
            "stockholm".to_string(),
            "city".to_string(),
        ],
    );
    assert_eq!(
        radroots_nostr_tag_match_location(&location_tag),
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
    assert_eq!(radroots_nostr_tag_match_location(&location_non_match), None);
    assert_eq!(radroots_nostr_tag_match_location(&custom_tag), None);

    let geohash_tag =
        RadrootsNostrTag::from_standardized(RadrootsNostrTagStandard::Geohash("u4pr".to_string()));
    assert_eq!(
        radroots_nostr_tag_match_geohash(&geohash_tag),
        Some("u4pr".to_string())
    );
    let title_tag =
        RadrootsNostrTag::from_standardized(RadrootsNostrTagStandard::Title("title".to_string()));
    assert_eq!(radroots_nostr_tag_match_geohash(&title_tag), None);
    assert_eq!(radroots_nostr_tag_match_geohash(&custom_tag), None);

    assert_eq!(
        radroots_nostr_tag_match_title(&title_tag),
        Some("title".to_string())
    );
    let summary_tag = RadrootsNostrTag::from_standardized(RadrootsNostrTagStandard::Summary(
        "summary".to_string(),
    ));
    assert_eq!(radroots_nostr_tag_match_title(&summary_tag), None);
    assert_eq!(radroots_nostr_tag_match_title(&custom_tag), None);

    assert_eq!(
        radroots_nostr_tag_match_summary(&summary_tag),
        Some("summary".to_string())
    );
    assert_eq!(radroots_nostr_tag_match_summary(&geohash_tag), None);
    assert_eq!(radroots_nostr_tag_match_summary(&custom_tag), None);

    let clear_event = text_event_with_tags(
        &keys,
        vec![RadrootsNostrTag::custom(
            RadrootsNostrTagKind::Custom(Cow::Borrowed("x")),
            vec!["x".to_string(), "v".to_string()],
        )],
    );
    let resolved = radroots_nostr_tags_resolve(&clear_event, &keys).expect("clear tags");
    assert_eq!(resolved.len(), 1);

    let encrypted_missing_p = text_event_with_tags(
        &keys,
        vec![RadrootsNostrTag::custom(
            RadrootsNostrTagKind::Encrypted,
            vec!["encrypted".to_string()],
        )],
    );
    let missing_p = radroots_nostr_tags_resolve(&encrypted_missing_p, &keys);
    assert!(matches!(
        missing_p,
        Err(RadrootsNostrTagsResolveError::MissingPTag(_))
    ));

    let sender = make_keys();
    let encrypted_invalid_p = encrypted_event_with_p_tag(&sender, "cipher", "not-a-pubkey");
    let invalid_p = radroots_nostr_tags_resolve(&encrypted_invalid_p, &keys);
    assert!(matches!(
        invalid_p,
        Err(RadrootsNostrTagsResolveError::MissingPTag(_))
    ));

    let encrypted_not_recipient =
        encrypted_event_with_p_tag(&sender, "cipher", &other.public_key().to_hex());
    let not_recipient = radroots_nostr_tags_resolve(&encrypted_not_recipient, &keys);
    assert!(matches!(
        not_recipient,
        Err(RadrootsNostrTagsResolveError::NotRecipient)
    ));

    let encrypted_bad_cipher =
        encrypted_event_with_p_tag(&sender, "not-ciphertext", &keys.public_key().to_hex());
    let bad_cipher = radroots_nostr_tags_resolve(&encrypted_bad_cipher, &keys);
    assert!(matches!(
        bad_cipher,
        Err(RadrootsNostrTagsResolveError::DecryptionError(_))
    ));

    let encrypted_cleartext = nip04::encrypt(sender.secret_key(), &keys.public_key(), "[]")
        .expect("encrypt cleartext tags");
    let encrypted_ok =
        encrypted_event_with_p_tag(&sender, encrypted_cleartext, &keys.public_key().to_hex());
    let resolved_encrypted =
        radroots_nostr_tags_resolve(&encrypted_ok, &keys).expect("resolve tags");
    assert!(resolved_encrypted.is_empty());
}

#[test]
fn util_helpers_cover_conversion_paths() {
    let keys = make_keys();
    let npub = radroots_nostr_npub_string(&keys.public_key());
    assert!(npub.is_some());

    let max = RadrootsNostrTimestamp::from(u64::from(u32::MAX));
    let overflow = RadrootsNostrTimestamp::from(u64::from(u32::MAX) + 1);
    assert_eq!(created_at_u32_saturating(max), u32::MAX);
    assert_eq!(created_at_u32_saturating(overflow), u32::MAX);

    let event = text_event_with_tags(&keys, Vec::new());
    let _ = event_created_at_u32_saturating(&event);
}
