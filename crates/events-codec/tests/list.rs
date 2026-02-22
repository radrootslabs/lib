use radroots_events::{
    kinds::{KIND_LIST_MUTE, KIND_POST},
    list::{RadrootsList, RadrootsListEntry},
};
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::list::decode::{
    index_from_event, list_entries_from_tags, list_from_tags, metadata_from_event,
};
use radroots_events_codec::list::encode::{list_build_tags, to_wire_parts_with_kind};

fn sample_list() -> RadrootsList {
    RadrootsList {
        content: "private".to_string(),
        entries: vec![
            RadrootsListEntry {
                tag: "p".to_string(),
                values: vec!["pubkey".to_string()],
            },
            RadrootsListEntry {
                tag: "t".to_string(),
                values: vec!["orchard".to_string()],
            },
        ],
    }
}

#[test]
fn list_build_tags_and_decode_roundtrip() {
    let list = sample_list();
    let tags = list_build_tags(&list).unwrap();
    let decoded = list_from_tags(KIND_LIST_MUTE, list.content.clone(), &tags).unwrap();
    assert_eq!(decoded.content, list.content);
    assert_eq!(decoded.entries.len(), list.entries.len());
    assert_eq!(decoded.entries[0].tag, list.entries[0].tag);
    assert_eq!(decoded.entries[0].values, list.entries[0].values);
    assert_eq!(decoded.entries[1].tag, list.entries[1].tag);
    assert_eq!(decoded.entries[1].values, list.entries[1].values);
}

#[test]
fn list_encode_and_decode_reject_invalid_inputs() {
    let invalid = RadrootsList {
        content: "".to_string(),
        entries: vec![RadrootsListEntry {
            tag: " ".to_string(),
            values: vec!["pubkey".to_string()],
        }],
    };
    let err = list_build_tags(&invalid).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("entry.tag")
    ));

    let invalid = RadrootsList {
        content: "".to_string(),
        entries: vec![RadrootsListEntry {
            tag: "p".to_string(),
            values: vec![" ".to_string()],
        }],
    };
    let err = list_build_tags(&invalid).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("entry.values")
    ));

    let err = to_wire_parts_with_kind(&sample_list(), KIND_POST).unwrap_err();
    assert!(matches!(err, EventEncodeError::InvalidKind(KIND_POST)));

    let err = list_from_tags(KIND_POST, "private".to_string(), &[]).unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "nip51 standard list kind",
            got: KIND_POST
        }
    ));
}

#[test]
fn list_entries_from_tags_rejects_empty_entry_fields() {
    let err = list_entries_from_tags(&[vec!["".to_string(), "x".to_string()]]).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("tag")));

    let err = list_entries_from_tags(&[vec!["p".to_string(), " ".to_string()]]).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("tag")));
}

#[test]
fn list_metadata_and_index_from_event_roundtrip() {
    let list = sample_list();
    let parts = to_wire_parts_with_kind(&list, KIND_LIST_MUTE).unwrap();

    let metadata = metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        44,
        KIND_LIST_MUTE,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(metadata.id, "id");
    assert_eq!(metadata.author, "author");
    assert_eq!(metadata.published_at, 44);
    assert_eq!(metadata.kind, KIND_LIST_MUTE);
    assert_eq!(metadata.list.content, list.content);
    assert_eq!(metadata.list.entries.len(), list.entries.len());
    assert_eq!(metadata.list.entries[0].tag, list.entries[0].tag);
    assert_eq!(metadata.list.entries[0].values, list.entries[0].values);

    let index = index_from_event(
        "id".to_string(),
        "author".to_string(),
        44,
        KIND_LIST_MUTE,
        parts.content,
        parts.tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(index.event.kind, KIND_LIST_MUTE);
    assert_eq!(index.event.sig, "sig");
    assert_eq!(index.metadata.list.entries.len(), 2);
}

#[test]
fn list_index_from_event_propagates_parse_errors() {
    let err = index_from_event(
        "id".to_string(),
        "author".to_string(),
        44,
        KIND_POST,
        "private".to_string(),
        Vec::new(),
        "sig".to_string(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "nip51 standard list kind",
            got: KIND_POST
        }
    ));
}
