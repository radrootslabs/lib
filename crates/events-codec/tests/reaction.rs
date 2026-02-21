mod common;

use radroots_events::tags::TAG_E_ROOT;
use radroots_events::{
    kinds::{KIND_POST, KIND_REACTION},
    reaction::RadrootsReaction,
};

use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::event_ref::{build_event_ref_tag, push_nip10_ref_tags};
use radroots_events_codec::reaction::decode::{
    index_from_event, metadata_from_event, reaction_from_tags,
};
use radroots_events_codec::reaction::encode::{reaction_build_tags, to_wire_parts};

#[test]
fn reaction_build_tags_requires_root_fields() {
    let reaction = RadrootsReaction {
        root: common::event_ref("", "author", KIND_POST),
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
        root: common::event_ref("root", "author", KIND_POST),
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
    let err = reaction_from_tags(KIND_REACTION, &tags, "+").unwrap_err();
    assert!(matches!(err, EventParseError::MissingTag("e")));
}

#[test]
fn reaction_roundtrip_from_tags() {
    let root = common::event_ref_with_d(
        "root",
        "author",
        KIND_POST,
        "note-1",
        Some(vec!["wss://relay".to_string()]),
    );
    let mut tags = Vec::new();
    push_nip10_ref_tags(&mut tags, &root, "e", "p", "k", "a");

    let reaction = reaction_from_tags(KIND_REACTION, &tags, "+").unwrap();

    assert_eq!(reaction.root.id, root.id);
    assert_eq!(reaction.root.author, root.author);
    assert_eq!(reaction.root.kind, root.kind);
    assert_eq!(reaction.root.d_tag, root.d_tag);
    assert_eq!(reaction.root.relays, root.relays);
    assert_eq!(reaction.content, "+");
}

#[test]
fn reaction_roundtrip_from_legacy_tags() {
    let root = common::event_ref("root", "author", KIND_POST);
    let tags = vec![build_event_ref_tag(TAG_E_ROOT, &root)];

    let reaction = reaction_from_tags(KIND_REACTION, &tags, "+").unwrap();

    assert_eq!(reaction.root.id, root.id);
    assert_eq!(reaction.root.author, root.author);
    assert_eq!(reaction.root.kind, root.kind);
    assert_eq!(reaction.content, "+");
}

#[test]
fn reaction_metadata_and_index_from_event_roundtrip() {
    let root = common::event_ref_with_d(
        "root",
        "author",
        KIND_POST,
        "note-1",
        Some(vec!["wss://relay".to_string()]),
    );
    let mut tags = Vec::new();
    push_nip10_ref_tags(&mut tags, &root, "e", "p", "k", "a");

    let metadata = metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        99,
        KIND_REACTION,
        "+".to_string(),
        tags.clone(),
    )
    .unwrap();
    assert_eq!(metadata.id, "id");
    assert_eq!(metadata.author, "author");
    assert_eq!(metadata.published_at, 99);
    assert_eq!(metadata.kind, KIND_REACTION);
    assert_eq!(metadata.reaction.content, "+");
    assert_eq!(metadata.reaction.root.id, root.id);

    let index = index_from_event(
        "id".to_string(),
        "author".to_string(),
        99,
        KIND_REACTION,
        "+".to_string(),
        tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(index.event.kind, KIND_REACTION);
    assert_eq!(index.event.sig, "sig");
    assert_eq!(index.metadata.reaction.content, "+");
}
