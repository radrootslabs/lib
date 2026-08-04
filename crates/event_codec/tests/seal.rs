use radroots_event::envelope::kind::{KIND_MESSAGE, KIND_SEAL};
use radroots_event::social::seal::Seal;

mod common;

use common::{AUTHOR, EVENT_ID, EVENT_SIG};
use radroots_event_codec::decode::EventParseError;
use radroots_event_codec::decode::seal::{data_from_event, parsed_from_event, seal_from_parts};
use radroots_event_codec::encode::EventEncodeError;
use radroots_event_codec::encode::seal::{seal_build_tags, to_wire_parts, to_wire_parts_with_kind};

#[test]
fn seal_to_wire_parts_requires_content() {
    let seal = Seal {
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
    let seal = Seal {
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
    let metadata = data_from_event(
        EVENT_ID.to_string(),
        AUTHOR.to_string(),
        14,
        KIND_SEAL,
        "payload".to_string(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(metadata.id, EVENT_ID);
    assert_eq!(metadata.author, AUTHOR);
    assert_eq!(metadata.published_at, 14);
    assert_eq!(metadata.kind, KIND_SEAL);
    assert_eq!(metadata.data.content, "payload");

    let index = parsed_from_event(
        EVENT_ID.to_string(),
        AUTHOR.to_string(),
        14,
        KIND_SEAL,
        "payload".to_string(),
        Vec::new(),
        EVENT_SIG.to_string(),
    )
    .unwrap();
    assert_eq!(index.event.kind_u32(), KIND_SEAL);
    assert_eq!(index.event.signature_hex(), EVENT_SIG);
    assert_eq!(index.data.data.content, "payload");
}

#[test]
fn seal_index_from_event_propagates_parse_errors() {
    let err = parsed_from_event(
        "id".to_string(),
        "author".to_string(),
        14,
        KIND_MESSAGE,
        "payload".to_string(),
        Vec::new(),
        "sig".to_string(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "13",
            got: KIND_MESSAGE
        }
    ));
}

#[test]
fn seal_build_tags_and_kind_validation_cover_paths() {
    let seal = Seal {
        content: "payload".to_string(),
    };
    assert!(seal_build_tags(&seal).unwrap().is_empty());

    let parts = to_wire_parts_with_kind(&seal, KIND_SEAL).unwrap();
    assert_eq!(parts.kind, KIND_SEAL);
    assert_eq!(parts.content, "payload");

    let err = to_wire_parts_with_kind(&seal, KIND_MESSAGE).unwrap_err();
    assert!(matches!(err, EventEncodeError::InvalidKind(KIND_MESSAGE)));
}
