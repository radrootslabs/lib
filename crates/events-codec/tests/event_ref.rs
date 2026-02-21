mod common;

use radroots_events::kinds::KIND_POST;
use radroots_events_codec::error::EventParseError;
use radroots_events_codec::event_ref::{
    build_event_ref_tag, find_event_ref_tag, parse_event_ref_tag, parse_nip10_ref_tags,
    push_nip10_ref_tags,
};

#[test]
fn build_and_parse_roundtrip_with_d_tag_and_relays() {
    let event = common::event_ref_with_d(
        "id",
        "author",
        42,
        "d-tag",
        Some(vec!["wss://relay".to_string()]),
    );

    let tag = build_event_ref_tag("e", &event);
    let parsed = parse_event_ref_tag(&tag, "e").unwrap();

    assert_eq!(parsed.id, event.id);
    assert_eq!(parsed.author, event.author);
    assert_eq!(parsed.kind, event.kind);
    assert_eq!(parsed.d_tag, event.d_tag);
    assert_eq!(parsed.relays, event.relays);
}

#[test]
fn build_and_parse_roundtrip_without_d_tag_or_relays() {
    let event = common::event_ref("id", "author", KIND_POST);
    let tag = build_event_ref_tag("e", &event);

    assert_eq!(tag.len(), 5);
    assert_eq!(tag[4], "");

    let parsed = parse_event_ref_tag(&tag, "e").unwrap();
    assert_eq!(parsed.id, event.id);
    assert_eq!(parsed.author, event.author);
    assert_eq!(parsed.kind, event.kind);
    assert!(parsed.d_tag.is_none());
    assert!(parsed.relays.is_none());
}

#[test]
fn parse_event_ref_tag_allows_relay_only_fifth_entry() {
    let tag = vec![
        "e".to_string(),
        "id".to_string(),
        "author".to_string(),
        KIND_POST.to_string(),
        "wss://relay".to_string(),
    ];

    let parsed = parse_event_ref_tag(&tag, "e").unwrap();
    assert!(parsed.d_tag.is_none());
    assert_eq!(parsed.relays, Some(vec!["wss://relay".to_string()]));
}

#[test]
fn parse_event_ref_tag_rejects_invalid_kind() {
    let tag = vec![
        "e".to_string(),
        "id".to_string(),
        "author".to_string(),
        "bad".to_string(),
    ];

    let err = parse_event_ref_tag(&tag, "e").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidNumber("e", _)));
}

#[test]
fn find_event_ref_tag_locates_first_match() {
    let event = common::event_ref("id", "author", KIND_POST);
    let tags = vec![
        vec!["p".to_string(), "pubkey".to_string()],
        build_event_ref_tag("e", &event),
    ];

    let found = find_event_ref_tag(&tags, "e").unwrap();
    assert_eq!(found[0], "e");
    assert_eq!(found[1], "id");
}

#[test]
fn push_and_parse_nip10_ref_tags_roundtrip_with_and_without_a_tag() {
    let event = common::event_ref_with_d(
        "id",
        "author",
        KIND_POST,
        "AAAAAAAAAAAAAAAAAAAAAA",
        Some(vec!["wss://relay.example.com".to_string()]),
    );
    let mut tags = Vec::new();
    push_nip10_ref_tags(&mut tags, &event, "e", "p", "k", "a");
    let parsed = parse_nip10_ref_tags(&tags, "e", "p", "k", "a").unwrap();
    assert_eq!(parsed.id, event.id);
    assert_eq!(parsed.author, event.author);
    assert_eq!(parsed.kind, event.kind);
    assert_eq!(parsed.d_tag, event.d_tag);
    assert_eq!(parsed.relays, event.relays);

    let event = common::event_ref("id2", "author2", KIND_POST);
    let mut tags = Vec::new();
    push_nip10_ref_tags(&mut tags, &event, "e", "p", "k", "a");
    let parsed = parse_nip10_ref_tags(&tags, "e", "p", "k", "a").unwrap();
    assert_eq!(parsed.id, event.id);
    assert_eq!(parsed.author, event.author);
    assert_eq!(parsed.kind, event.kind);
    assert!(parsed.d_tag.is_none());
}

#[test]
fn parse_nip10_ref_tags_rejects_missing_or_invalid_required_tags() {
    let err = parse_nip10_ref_tags(&[], "e", "p", "k", "a").unwrap_err();
    assert!(matches!(err, EventParseError::MissingTag("e")));

    let tags = vec![
        vec!["e".to_string(), "".to_string()],
        vec!["p".to_string(), "author".to_string()],
        vec!["k".to_string(), KIND_POST.to_string()],
    ];
    let err = parse_nip10_ref_tags(&tags, "e", "p", "k", "a").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("e")));

    let tags = vec![
        vec!["e".to_string(), "id".to_string()],
        vec!["p".to_string(), "".to_string()],
        vec!["k".to_string(), KIND_POST.to_string()],
    ];
    let err = parse_nip10_ref_tags(&tags, "e", "p", "k", "a").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("p")));

    let tags = vec![
        vec!["e".to_string(), "id".to_string()],
        vec!["p".to_string(), "author".to_string()],
        vec!["k".to_string(), "bad-kind".to_string()],
    ];
    let err = parse_nip10_ref_tags(&tags, "e", "p", "k", "a").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidNumber("k", _)));
}

#[test]
fn parse_nip10_ref_tags_prefers_e_relays_and_can_fall_back_to_a_relays() {
    let tags = vec![
        vec!["e".to_string(), "id".to_string()],
        vec!["p".to_string(), "author".to_string()],
        vec!["k".to_string(), KIND_POST.to_string()],
        vec![
            "a".to_string(),
            format!("{}:{}:{}", KIND_POST, "author", "AAAAAAAAAAAAAAAAAAAAAA"),
            "wss://relay.a.example.com".to_string(),
        ],
    ];
    let parsed = parse_nip10_ref_tags(&tags, "e", "p", "k", "a").unwrap();
    assert_eq!(parsed.d_tag.as_deref(), Some("AAAAAAAAAAAAAAAAAAAAAA"));
    assert_eq!(
        parsed.relays,
        Some(vec!["wss://relay.a.example.com".to_string()])
    );

    let tags = vec![
        vec![
            "e".to_string(),
            "id".to_string(),
            "wss://relay.e.example.com".to_string(),
        ],
        vec!["p".to_string(), "author".to_string()],
        vec!["k".to_string(), KIND_POST.to_string()],
        vec![
            "a".to_string(),
            format!("{}:{}:{}", KIND_POST, "author", "AAAAAAAAAAAAAAAAAAAAAA"),
            "wss://relay.a.example.com".to_string(),
        ],
    ];
    let parsed = parse_nip10_ref_tags(&tags, "e", "p", "k", "a").unwrap();
    assert_eq!(
        parsed.relays,
        Some(vec!["wss://relay.e.example.com".to_string()])
    );
}
