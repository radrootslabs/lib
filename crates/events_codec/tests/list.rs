use radroots_events::{
    kinds::{KIND_LIST_MUTE, KIND_LIST_READ_WRITE_RELAYS, KIND_LIST_SET_FOLLOW, KIND_POST},
    list::{RadrootsList, RadrootsListEntry},
    tags::TAG_R,
};
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::list::decode::{
    data_from_event, list_entries_from_tags, list_from_tags, parsed_from_event,
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

    let invalid = RadrootsList {
        content: "".to_string(),
        entries: vec![RadrootsListEntry {
            tag: "p".to_string(),
            values: Vec::new(),
        }],
    };
    let err = list_build_tags(&invalid).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("entry.values")
    ));

    let err = to_wire_parts_with_kind(&invalid, KIND_LIST_MUTE).unwrap_err();
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
            expected: "nip51 standard or list-set kind",
            got: KIND_POST
        }
    ));
}

#[test]
fn list_set_kind_roundtrips_generic_entries() {
    let list = sample_list();
    let parts = to_wire_parts_with_kind(&list, KIND_LIST_SET_FOLLOW).unwrap();

    assert_eq!(parts.kind, KIND_LIST_SET_FOLLOW);
    let decoded = list_from_tags(parts.kind, parts.content, &parts.tags).unwrap();
    assert_eq!(decoded.entries.len(), list.entries.len());
    assert_eq!(decoded.entries[0].tag, "p");
}

#[test]
fn list_entries_from_tags_rejects_empty_entry_fields() {
    let err = list_entries_from_tags(&[vec!["".to_string(), "x".to_string()]]).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("tag")));

    let err = list_entries_from_tags(&[vec!["p".to_string(), " ".to_string()]]).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("tag")));

    let err = list_from_tags(
        KIND_LIST_MUTE,
        "private".to_string(),
        &[vec!["".to_string(), "x".to_string()]],
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("tag")));
}

#[test]
fn list_metadata_and_index_from_event_roundtrip() {
    let list = sample_list();
    let parts = to_wire_parts_with_kind(&list, KIND_LIST_MUTE).unwrap();

    let metadata = data_from_event(
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
    assert_eq!(metadata.data.content, list.content);
    assert_eq!(metadata.data.entries.len(), list.entries.len());
    assert_eq!(metadata.data.entries[0].tag, list.entries[0].tag);
    assert_eq!(metadata.data.entries[0].values, list.entries[0].values);

    let index = parsed_from_event(
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
    assert_eq!(index.data.data.entries.len(), 2);
}

#[test]
fn list_index_from_event_propagates_parse_errors() {
    let err = parsed_from_event(
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
            expected: "nip51 standard or list-set kind",
            got: KIND_POST
        }
    ));
}

#[test]
fn relay_list_kind_roundtrips_nip65_r_tags() {
    let list = RadrootsList {
        content: String::new(),
        entries: vec![
            RadrootsListEntry {
                tag: TAG_R.to_string(),
                values: vec!["wss://read.example.test".to_string(), "read".to_string()],
            },
            RadrootsListEntry {
                tag: TAG_R.to_string(),
                values: vec!["wss://write.example.test".to_string(), "write".to_string()],
            },
            RadrootsListEntry {
                tag: TAG_R.to_string(),
                values: vec!["wss://both.example.test".to_string()],
            },
        ],
    };

    let parts = to_wire_parts_with_kind(&list, KIND_LIST_READ_WRITE_RELAYS).unwrap();
    assert_eq!(parts.kind, KIND_LIST_READ_WRITE_RELAYS);
    assert!(parts.content.is_empty());
    assert_eq!(parts.tags.len(), 3);

    let decoded = list_from_tags(parts.kind, parts.content, &parts.tags).unwrap();
    assert_eq!(decoded.entries.len(), 3);
    assert_eq!(decoded.entries[0].values[1], "read");
    assert_eq!(decoded.entries[1].values[1], "write");
    assert_eq!(decoded.entries[2].values.len(), 1);
}

#[test]
fn relay_list_kind_validates_url_shape_and_markers() {
    let invalid_url = RadrootsList {
        content: String::new(),
        entries: vec![RadrootsListEntry {
            tag: TAG_R.to_string(),
            values: vec!["https://relay.example.test".to_string()],
        }],
    };
    assert!(matches!(
        to_wire_parts_with_kind(&invalid_url, KIND_LIST_READ_WRITE_RELAYS),
        Err(EventEncodeError::InvalidField("relay.url"))
    ));
    assert!(matches!(
        list_from_tags(
            KIND_LIST_READ_WRITE_RELAYS,
            String::new(),
            &[vec![
                TAG_R.to_string(),
                "https://relay.example.test".to_string()
            ]]
        ),
        Err(EventParseError::InvalidTag(TAG_R))
    ));

    let invalid_marker = RadrootsList {
        content: String::new(),
        entries: vec![RadrootsListEntry {
            tag: TAG_R.to_string(),
            values: vec!["wss://relay.example.test".to_string(), "both".to_string()],
        }],
    };
    assert!(matches!(
        to_wire_parts_with_kind(&invalid_marker, KIND_LIST_READ_WRITE_RELAYS),
        Err(EventEncodeError::InvalidField("relay.marker"))
    ));
    assert!(matches!(
        list_from_tags(
            KIND_LIST_READ_WRITE_RELAYS,
            String::new(),
            &[vec![
                TAG_R.to_string(),
                "wss://relay.example.test".to_string(),
                "both".to_string()
            ]]
        ),
        Err(EventParseError::InvalidTag(TAG_R))
    ));

    let empty = RadrootsList {
        content: String::new(),
        entries: Vec::new(),
    };
    assert!(matches!(
        to_wire_parts_with_kind(&empty, KIND_LIST_READ_WRITE_RELAYS),
        Err(EventEncodeError::EmptyRequiredField("relay.entries"))
    ));
    assert!(matches!(
        list_from_tags(KIND_LIST_READ_WRITE_RELAYS, String::new(), &[]),
        Err(EventParseError::MissingTag(TAG_R))
    ));
}
