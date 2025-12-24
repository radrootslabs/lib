mod common;

use radroots_events::reaction::RadrootsReaction;
use radroots_events::tags::TAG_E_ROOT;

use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::event_ref::{build_event_ref_tag, push_nip10_ref_tags};
use radroots_events_codec::reaction::decode::reaction_from_tags;
use radroots_events_codec::reaction::encode::{reaction_build_tags, to_wire_parts};

#[test]
fn reaction_build_tags_requires_root_fields() {
    let reaction = RadrootsReaction {
        root: common::event_ref("", "author", 1),
        content: "like".to_string(),
    };

    let err = reaction_build_tags(&reaction).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("root.id")
    ));
}

#[test]
fn reaction_to_wire_parts_requires_content() {
    let reaction = RadrootsReaction {
        root: common::event_ref("root", "author", 1),
        content: "   ".to_string(),
    };

    let err = to_wire_parts(&reaction).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("content")
    ));
}

#[test]
fn reaction_from_tags_requires_root_tag() {
    let tags = vec![vec!["p".to_string(), "x".to_string()]];
    let err = reaction_from_tags(7, &tags, "+").unwrap_err();
    assert!(matches!(err, EventParseError::MissingTag("e")));
}

#[test]
fn reaction_roundtrip_from_tags() {
    let root = common::event_ref_with_d(
        "root",
        "author",
        1,
        "note-1",
        Some(vec!["wss://relay".to_string()]),
    );
    let mut tags = Vec::new();
    push_nip10_ref_tags(&mut tags, &root, "e", "p", "k", "a");

    let reaction = reaction_from_tags(7, &tags, "+").unwrap();

    assert_eq!(reaction.root.id, root.id);
    assert_eq!(reaction.root.author, root.author);
    assert_eq!(reaction.root.kind, root.kind);
    assert_eq!(reaction.root.d_tag, root.d_tag);
    assert_eq!(reaction.root.relays, root.relays);
    assert_eq!(reaction.content, "+");
}

#[test]
fn reaction_roundtrip_from_legacy_tags() {
    let root = common::event_ref("root", "author", 1);
    let tags = vec![build_event_ref_tag(TAG_E_ROOT, &root)];

    let reaction = reaction_from_tags(7, &tags, "+").unwrap();

    assert_eq!(reaction.root.id, root.id);
    assert_eq!(reaction.root.author, root.author);
    assert_eq!(reaction.root.kind, root.kind);
    assert_eq!(reaction.content, "+");
}
