use radroots_events::kinds::{KIND_MESSAGE, KIND_SEAL};
use radroots_events::seal::RadrootsSeal;

use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::seal::decode::{index_from_event, metadata_from_event, seal_from_parts};
use radroots_events_codec::seal::encode::to_wire_parts;

#[test]
fn seal_to_wire_parts_requires_content() {
    let seal = RadrootsSeal {
        content: "  ".to_string(),
    };

    let err = to_wire_parts(&seal).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("content")
    ));
}

#[test]
fn seal_to_wire_parts_sets_kind_and_content() {
    let seal = RadrootsSeal {
        content: "payload".to_string(),
    };

    let parts = to_wire_parts(&seal).unwrap();
    assert_eq!(parts.kind, KIND_SEAL);
    assert_eq!(parts.content, "payload");
    assert!(parts.tags.is_empty());
}

#[test]
fn seal_from_parts_rejects_wrong_kind() {
    let err = seal_from_parts(KIND_MESSAGE, &[], "payload").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "13",
            got: KIND_MESSAGE
        }
    ));
}

#[test]
fn seal_from_parts_requires_empty_tags() {
    let err = seal_from_parts(
        KIND_SEAL,
        &[vec!["p".to_string(), "x".to_string()]],
        "payload",
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("tags")));
}

#[test]
fn seal_from_parts_requires_content() {
    let err = seal_from_parts(KIND_SEAL, &[], " ").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("content")));
}

#[test]
fn seal_metadata_and_index_from_event_roundtrip() {
    let metadata = metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        14,
        KIND_SEAL,
        "payload".to_string(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(metadata.id, "id");
    assert_eq!(metadata.author, "author");
    assert_eq!(metadata.published_at, 14);
    assert_eq!(metadata.kind, KIND_SEAL);
    assert_eq!(metadata.seal.content, "payload");

    let index = index_from_event(
        "id".to_string(),
        "author".to_string(),
        14,
        KIND_SEAL,
        "payload".to_string(),
        Vec::new(),
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(index.event.kind, KIND_SEAL);
    assert_eq!(index.event.sig, "sig");
    assert_eq!(index.metadata.seal.content, "payload");
}
