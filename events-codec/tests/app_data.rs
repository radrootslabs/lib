use radroots_events::app_data::{RadrootsAppData, KIND_APP_DATA};
use radroots_events_codec::app_data::decode::app_data_from_tags;
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
    let err = app_data_from_tags(1, &tags, "payload").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "30078",
            got: 1
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
