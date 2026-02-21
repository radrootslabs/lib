use radroots_events::{
    app_data::{RadrootsAppData, KIND_APP_DATA},
    kinds::KIND_POST,
};
use radroots_events_codec::app_data::decode::{
    app_data_from_tags, index_from_event, metadata_from_event,
};
use radroots_events_codec::app_data::encode::{app_data_build_tags, to_wire_parts};
use radroots_events_codec::error::{EventEncodeError, EventParseError};

#[test]
fn app_data_build_tags_requires_d_tag() {
    let app_data = RadrootsAppData {
        d_tag: "  ".to_string(),
        content: "payload".to_string(),
    };

    let err = app_data_build_tags(&app_data).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));
}

#[test]
fn app_data_to_wire_parts_sets_kind_tags_content() {
    let app_data = RadrootsAppData {
        d_tag: "radroots.app".to_string(),
        content: "payload".to_string(),
    };

    let parts = to_wire_parts(&app_data).unwrap();
    assert_eq!(parts.kind, KIND_APP_DATA);
    assert_eq!(parts.content, "payload");
    assert_eq!(
        parts.tags,
        vec![vec!["d".to_string(), "radroots.app".to_string()]]
    );
}

#[test]
fn app_data_from_tags_requires_kind() {
    let tags = vec![vec!["d".to_string(), "radroots.app".to_string()]];
    let err = app_data_from_tags(KIND_POST, &tags, "payload").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "30078",
            got: KIND_POST
        }
    ));
}

#[test]
fn app_data_from_tags_requires_d_tag() {
    let err = app_data_from_tags(KIND_APP_DATA, &[], "payload").unwrap_err();
    assert!(matches!(err, EventParseError::MissingTag("d")));
}

#[test]
fn app_data_roundtrip_from_tags() {
    let tags = vec![vec!["d".to_string(), "radroots.app".to_string()]];
    let app_data = app_data_from_tags(KIND_APP_DATA, &tags, "payload").unwrap();

    assert_eq!(app_data.d_tag, "radroots.app");
    assert_eq!(app_data.content, "payload");
}

#[test]
fn app_data_from_tags_rejects_invalid_d_tag_shape() {
    let err = app_data_from_tags(KIND_APP_DATA, &[vec!["d".to_string()]], "payload").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("d")));

    let err = app_data_from_tags(
        KIND_APP_DATA,
        &[vec!["d".to_string(), " ".to_string()]],
        "payload",
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("d")));
}

#[test]
fn app_data_metadata_and_index_from_event_roundtrip() {
    let tags = vec![vec!["d".to_string(), "radroots.app".to_string()]];
    let metadata = metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        42,
        KIND_APP_DATA,
        "payload".to_string(),
        tags.clone(),
    )
    .unwrap();
    assert_eq!(metadata.id, "id");
    assert_eq!(metadata.author, "author");
    assert_eq!(metadata.published_at, 42);
    assert_eq!(metadata.kind, KIND_APP_DATA);
    assert_eq!(metadata.app_data.d_tag, "radroots.app");
    assert_eq!(metadata.app_data.content, "payload");

    let index = index_from_event(
        "id".to_string(),
        "author".to_string(),
        42,
        KIND_APP_DATA,
        "payload".to_string(),
        tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(index.event.id, "id");
    assert_eq!(index.event.author, "author");
    assert_eq!(index.event.created_at, 42);
    assert_eq!(index.event.kind, KIND_APP_DATA);
    assert_eq!(index.event.content, "payload");
    assert_eq!(index.event.sig, "sig");
    assert_eq!(index.metadata.app_data.d_tag, "radroots.app");
}
