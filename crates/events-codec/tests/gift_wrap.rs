use radroots_events::gift_wrap::{RadrootsGiftWrap, RadrootsGiftWrapRecipient};
use radroots_events::kinds::{KIND_GIFT_WRAP, KIND_MESSAGE};

use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::gift_wrap::decode::{
    gift_wrap_from_tags, index_from_event, metadata_from_event,
};
use radroots_events_codec::gift_wrap::encode::{gift_wrap_build_tags, to_wire_parts};

fn sample_gift_wrap() -> RadrootsGiftWrap {
    RadrootsGiftWrap {
        recipient: RadrootsGiftWrapRecipient {
            public_key: "pubkey".to_string(),
            relay_url: Some("wss://relay.example".to_string()),
        },
        content: "encrypted".to_string(),
        expiration: Some(1700000000),
    }
}

#[test]
fn gift_wrap_build_tags_requires_recipient() {
    let mut gift_wrap = sample_gift_wrap();
    gift_wrap.recipient.public_key = "  ".to_string();

    let err = gift_wrap_build_tags(&gift_wrap).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("recipient.public_key")
    ));
}

#[test]
fn gift_wrap_to_wire_parts_sets_kind_content_and_tags() {
    let gift_wrap = sample_gift_wrap();
    let parts = to_wire_parts(&gift_wrap).unwrap();

    assert_eq!(parts.kind, KIND_GIFT_WRAP);
    assert_eq!(parts.content, "encrypted");
    assert_eq!(
        parts.tags,
        vec![
            vec![
                "p".to_string(),
                "pubkey".to_string(),
                "wss://relay.example".to_string()
            ],
            vec!["expiration".to_string(), "1700000000".to_string()],
        ]
    );
}

#[test]
fn gift_wrap_from_tags_rejects_wrong_kind() {
    let gift_wrap = sample_gift_wrap();
    let parts = to_wire_parts(&gift_wrap).unwrap();

    let err = gift_wrap_from_tags(KIND_MESSAGE, &parts.tags, &parts.content).unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "1059",
            got: KIND_MESSAGE
        }
    ));
}

#[test]
fn gift_wrap_from_tags_requires_p_tag() {
    let err = gift_wrap_from_tags(KIND_GIFT_WRAP, &[], "payload").unwrap_err();
    assert!(matches!(err, EventParseError::MissingTag("p")));
}

#[test]
fn gift_wrap_from_tags_rejects_invalid_expiration_and_relay() {
    let err = gift_wrap_from_tags(
        KIND_GIFT_WRAP,
        &[
            vec![
                "p".to_string(),
                "pubkey".to_string(),
                "wss://relay.example".to_string(),
            ],
            vec!["expiration".to_string(), " ".to_string()],
        ],
        "payload",
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("expiration")));

    let err = gift_wrap_from_tags(
        KIND_GIFT_WRAP,
        &[
            vec![
                "p".to_string(),
                "pubkey".to_string(),
                "wss://relay.example".to_string(),
            ],
            vec!["expiration".to_string(), "invalid".to_string()],
        ],
        "payload",
    )
    .unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidNumber("expiration", _)
    ));

    let err = gift_wrap_from_tags(
        KIND_GIFT_WRAP,
        &[vec!["p".to_string(), "pubkey".to_string(), " ".to_string()]],
        "payload",
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("p")));
}

#[test]
fn gift_wrap_metadata_and_index_from_event_roundtrip() {
    let gift_wrap = sample_gift_wrap();
    let parts = to_wire_parts(&gift_wrap).unwrap();

    let metadata = metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        11,
        parts.kind,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(metadata.id, "id");
    assert_eq!(metadata.author, "author");
    assert_eq!(metadata.published_at, 11);
    assert_eq!(
        metadata.gift_wrap.recipient.public_key,
        gift_wrap.recipient.public_key
    );
    assert_eq!(
        metadata.gift_wrap.recipient.relay_url,
        gift_wrap.recipient.relay_url
    );
    assert_eq!(metadata.gift_wrap.content, gift_wrap.content);
    assert_eq!(metadata.gift_wrap.expiration, gift_wrap.expiration);

    let index = index_from_event(
        "id".to_string(),
        "author".to_string(),
        11,
        parts.kind,
        parts.content,
        parts.tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(index.event.kind, KIND_GIFT_WRAP);
    assert_eq!(index.event.sig, "sig");
    assert_eq!(index.metadata.gift_wrap.recipient.public_key, "pubkey");
}
