mod common;

use radroots_events::comment::RadrootsComment;
use radroots_events::tags::{TAG_E_PREV, TAG_E_ROOT};

use radroots_events_codec::comment::decode::comment_from_tags;
use radroots_events_codec::comment::encode::{comment_build_tags, to_wire_parts};
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::event_ref::{build_event_ref_tag, push_nip10_ref_tags};

fn assert_event_ref_fields(
    actual: &radroots_events::RadrootsNostrEventRef,
    expected: &radroots_events::RadrootsNostrEventRef,
) {
    assert_eq!(actual.id, expected.id);
    assert_eq!(actual.author, expected.author);
    assert_eq!(actual.kind, expected.kind);
    assert_eq!(actual.d_tag, expected.d_tag);
    assert_eq!(actual.relays, expected.relays);
}

#[test]
fn comment_build_tags_requires_root_id() {
    let comment = RadrootsComment {
        root: common::event_ref("", "author", 1),
        parent: common::event_ref("parent", "author", 1),
        content: "hello".to_string(),
    };

    let err = comment_build_tags(&comment).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("root.id")
    ));
}

#[test]
fn comment_build_tags_requires_parent_author() {
    let comment = RadrootsComment {
        root: common::event_ref("root", "author", 1),
        parent: common::event_ref("parent", "", 1),
        content: "hello".to_string(),
    };

    let err = comment_build_tags(&comment).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("parent.author")
    ));
}

#[test]
fn comment_to_wire_parts_requires_content() {
    let comment = RadrootsComment {
        root: common::event_ref("root", "author", 1),
        parent: common::event_ref("parent", "author", 1),
        content: "   ".to_string(),
    };

    let err = to_wire_parts(&comment).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("content")
    ));
}

#[test]
fn comment_roundtrip_from_tags_with_parent() {
    let root = common::event_ref_with_d(
        "root",
        "author",
        1,
        "root-d",
        Some(vec!["wss://relay".to_string()]),
    );
    let parent = common::event_ref_with_d(
        "parent",
        "author",
        1,
        "parent-d",
        Some(vec!["wss://relay-2".to_string()]),
    );

    let mut tags = Vec::new();
    push_nip10_ref_tags(&mut tags, &root, "E", "P", "K", "A");
    push_nip10_ref_tags(&mut tags, &parent, "e", "p", "k", "a");

    let comment = comment_from_tags(1, &tags, "hello").unwrap();

    assert_event_ref_fields(&comment.root, &root);
    assert_event_ref_fields(&comment.parent, &parent);
    assert_eq!(comment.content, "hello");
}

#[test]
fn comment_from_tags_defaults_parent_to_root() {
    let root = common::event_ref("root", "author", 1);
    let mut tags = Vec::new();
    push_nip10_ref_tags(&mut tags, &root, "E", "P", "K", "A");

    let comment = comment_from_tags(1, &tags, "hello").unwrap();

    assert_event_ref_fields(&comment.root, &root);
    assert_event_ref_fields(&comment.parent, &root);
}

#[test]
fn comment_roundtrip_from_legacy_tags() {
    let root = common::event_ref("root", "author", 1);
    let parent = common::event_ref("parent", "author", 1);

    let tags = vec![
        build_event_ref_tag(TAG_E_ROOT, &root),
        build_event_ref_tag(TAG_E_PREV, &parent),
    ];

    let comment = comment_from_tags(1, &tags, "hello").unwrap();

    assert_event_ref_fields(&comment.root, &root);
    assert_event_ref_fields(&comment.parent, &parent);
}

#[test]
fn comment_from_tags_requires_root_tag() {
    let tags = vec![vec!["p".to_string(), "x".to_string()]];

    let err = comment_from_tags(1, &tags, "hello").unwrap_err();
    assert!(matches!(err, EventParseError::MissingTag("E")));
}

#[test]
fn comment_from_tags_rejects_wrong_kind() {
    let tags = vec![vec!["e".to_string(), "x".to_string()]];
    let err = comment_from_tags(2, &tags, "hello").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind { expected: "1", got: 2 }
    ));
}
