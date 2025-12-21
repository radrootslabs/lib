use radroots_events::post::RadrootsPost;
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::post::decode::post_from_content;
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
    assert_eq!(parts.kind, 1);
    assert_eq!(parts.content, "hello");
    assert!(parts.tags.is_empty());
}

#[test]
fn post_from_content_requires_kind_and_content() {
    let err = post_from_content(2, "hello").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind { expected: "1", got: 2 }
    ));

    let err = post_from_content(1, "   ").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("content")));
}
