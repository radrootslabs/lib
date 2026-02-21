use radroots_events::{
    kinds::{KIND_COMMENT, KIND_POST},
    post::RadrootsPost,
};
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::post::decode::{
    index_from_event, metadata_from_event, post_from_content,
};
use radroots_events_codec::post::encode::to_wire_parts;

#[test]
fn post_to_wire_parts_requires_content() {
    let post = RadrootsPost {
        content: "   ".to_string(),
    };

    let err = to_wire_parts(&post).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("content")
    ));
}

#[test]
fn post_to_wire_parts_sets_kind_and_content() {
    let post = RadrootsPost {
        content: "hello".to_string(),
    };

    let parts = to_wire_parts(&post).unwrap();
    assert_eq!(parts.kind, KIND_POST);
    assert_eq!(parts.content, "hello");
    assert!(parts.tags.is_empty());
}

#[test]
fn post_from_content_requires_kind_and_content() {
    let err = post_from_content(KIND_COMMENT, "hello").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "1",
            got: KIND_COMMENT
        }
    ));

    let err = post_from_content(KIND_POST, "   ").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("content")));
}

#[test]
fn post_metadata_and_index_from_event_roundtrip() {
    let metadata = metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        77,
        KIND_POST,
        "hello".to_string(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(metadata.id, "id");
    assert_eq!(metadata.author, "author");
    assert_eq!(metadata.published_at, 77);
    assert_eq!(metadata.kind, KIND_POST);
    assert_eq!(metadata.post.content, "hello");

    let index = index_from_event(
        "id".to_string(),
        "author".to_string(),
        77,
        KIND_POST,
        "hello".to_string(),
        Vec::new(),
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(index.event.id, "id");
    assert_eq!(index.event.author, "author");
    assert_eq!(index.event.created_at, 77);
    assert_eq!(index.event.kind, KIND_POST);
    assert_eq!(index.event.content, "hello");
    assert_eq!(index.event.sig, "sig");
    assert_eq!(index.metadata.post.content, "hello");
}
