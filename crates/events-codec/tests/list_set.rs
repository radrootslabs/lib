use radroots_events::{
    kinds::{KIND_LIST_SET_FOLLOW, KIND_POST},
    list::RadrootsListEntry,
    list_set::RadrootsListSet,
};
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::list_set::decode::{
    index_from_event, list_set_from_tags, metadata_from_event,
};
use radroots_events_codec::list_set::encode::{list_set_build_tags, to_wire_parts_with_kind};

fn sample_list_set() -> RadrootsListSet {
    RadrootsListSet {
        d_tag: "members.owners".to_string(),
        content: "private".to_string(),
        entries: vec![
            RadrootsListEntry {
                tag: "p".to_string(),
                values: vec!["owner".to_string()],
            },
            RadrootsListEntry {
                tag: "t".to_string(),
                values: vec!["orchard".to_string()],
            },
        ],
        title: Some("owners".to_string()),
        description: Some("core team".to_string()),
        image: Some("https://example.com/team.png".to_string()),
    }
}

#[test]
fn list_set_build_tags_and_decode_roundtrip() {
    let list_set = sample_list_set();
    let tags = list_set_build_tags(&list_set).unwrap();
    let decoded =
        list_set_from_tags(KIND_LIST_SET_FOLLOW, list_set.content.clone(), &tags).unwrap();
    assert_eq!(decoded.d_tag, list_set.d_tag);
    assert_eq!(decoded.title, list_set.title);
    assert_eq!(decoded.description, list_set.description);
    assert_eq!(decoded.image, list_set.image);
    assert_eq!(decoded.entries.len(), list_set.entries.len());
    assert_eq!(decoded.entries[0].tag, list_set.entries[0].tag);
    assert_eq!(decoded.entries[0].values, list_set.entries[0].values);
    assert_eq!(decoded.entries[1].tag, list_set.entries[1].tag);
    assert_eq!(decoded.entries[1].values, list_set.entries[1].values);
}

#[test]
fn list_set_encode_and_decode_reject_invalid_inputs() {
    let invalid = RadrootsListSet {
        d_tag: " ".to_string(),
        content: "".to_string(),
        entries: Vec::new(),
        title: None,
        description: None,
        image: None,
    };
    let err = list_set_build_tags(&invalid).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));

    let invalid = RadrootsListSet {
        d_tag: "farm:invalid:owners".to_string(),
        content: "".to_string(),
        entries: Vec::new(),
        title: None,
        description: None,
        image: None,
    };
    let err = list_set_build_tags(&invalid).unwrap_err();
    assert!(matches!(err, EventEncodeError::InvalidField("d_tag")));

    let err = to_wire_parts_with_kind(&sample_list_set(), KIND_POST).unwrap_err();
    assert!(matches!(err, EventEncodeError::InvalidKind(KIND_POST)));

    let err = list_set_from_tags(KIND_POST, "".to_string(), &[]).unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "nip51 list set kind",
            got: KIND_POST
        }
    ));
}

#[test]
fn list_set_decode_rejects_invalid_tag_shapes() {
    let err = list_set_from_tags(KIND_LIST_SET_FOLLOW, "".to_string(), &[]).unwrap_err();
    assert!(matches!(err, EventParseError::MissingTag("d")));

    let err = list_set_from_tags(
        KIND_LIST_SET_FOLLOW,
        "".to_string(),
        &[vec!["d".to_string(), " ".to_string()]],
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("d")));

    let err = list_set_from_tags(
        KIND_LIST_SET_FOLLOW,
        "".to_string(),
        &[vec!["".to_string(), "value".to_string()]],
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("tag")));
}

#[test]
fn list_set_metadata_and_index_from_event_roundtrip() {
    let list_set = sample_list_set();
    let parts = to_wire_parts_with_kind(&list_set, KIND_LIST_SET_FOLLOW).unwrap();

    let metadata = metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        44,
        KIND_LIST_SET_FOLLOW,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(metadata.id, "id");
    assert_eq!(metadata.author, "author");
    assert_eq!(metadata.published_at, 44);
    assert_eq!(metadata.kind, KIND_LIST_SET_FOLLOW);
    assert_eq!(metadata.list_set.d_tag, "members.owners");

    let index = index_from_event(
        "id".to_string(),
        "author".to_string(),
        44,
        KIND_LIST_SET_FOLLOW,
        parts.content,
        parts.tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(index.event.kind, KIND_LIST_SET_FOLLOW);
    assert_eq!(index.event.sig, "sig");
    assert_eq!(index.metadata.list_set.entries.len(), 2);
}

#[test]
fn list_set_decode_keeps_first_optional_display_tags() {
    let tags = vec![
        vec!["d".to_string(), "members.owners".to_string()],
        vec!["title".to_string(), "owners".to_string()],
        vec!["title".to_string(), "ignored".to_string()],
        vec!["description".to_string(), "team".to_string()],
        vec!["description".to_string(), "ignored".to_string()],
        vec!["image".to_string(), "https://example.com/a.png".to_string()],
        vec!["image".to_string(), "https://example.com/b.png".to_string()],
        vec!["p".to_string(), "owner".to_string()],
    ];
    let decoded = list_set_from_tags(KIND_LIST_SET_FOLLOW, "private".to_string(), &tags).unwrap();
    assert_eq!(decoded.title.as_deref(), Some("owners"));
    assert_eq!(decoded.description.as_deref(), Some("team"));
    assert_eq!(decoded.image.as_deref(), Some("https://example.com/a.png"));
}
