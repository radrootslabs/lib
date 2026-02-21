use radroots_events::{
    geochat::RadrootsGeoChat,
    kinds::{KIND_GEOCHAT, KIND_POST},
};
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::geochat::decode::{
    geochat_from_tags, index_from_event, metadata_from_event,
};
use radroots_events_codec::geochat::encode::{geochat_build_tags, to_wire_parts};

#[test]
fn geochat_build_tags_requires_geohash() {
    let geochat = RadrootsGeoChat {
        geohash: "  ".to_string(),
        content: "hello".to_string(),
        nickname: None,
        teleported: false,
    };

    let err = geochat_build_tags(&geochat).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("geohash")
    ));
}

#[test]
fn geochat_build_tags_requires_nickname_if_present() {
    let geochat = RadrootsGeoChat {
        geohash: "dr5rsj7".to_string(),
        content: "hello".to_string(),
        nickname: Some(" ".to_string()),
        teleported: false,
    };

    let err = geochat_build_tags(&geochat).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("nickname")
    ));
}

#[test]
fn geochat_to_wire_parts_requires_content() {
    let geochat = RadrootsGeoChat {
        geohash: "dr5rsj7".to_string(),
        content: "  ".to_string(),
        nickname: None,
        teleported: false,
    };

    let err = to_wire_parts(&geochat).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("content")
    ));
}

#[test]
fn geochat_to_wire_parts_sets_tags() {
    let geochat = RadrootsGeoChat {
        geohash: "dr5rsj7".to_string(),
        content: "hello".to_string(),
        nickname: Some("alex".to_string()),
        teleported: true,
    };

    let parts = to_wire_parts(&geochat).unwrap();
    assert_eq!(parts.kind, KIND_GEOCHAT);
    assert_eq!(parts.content, "hello");
    assert_eq!(
        parts.tags,
        vec![
            vec!["g".to_string(), "dr5rsj7".to_string()],
            vec!["n".to_string(), "alex".to_string()],
            vec!["t".to_string(), "teleport".to_string()],
        ]
    );
}

#[test]
fn geochat_from_tags_requires_kind_geohash_and_content() {
    let tags = vec![vec!["g".to_string(), "dr5rsj7".to_string()]];
    let err = geochat_from_tags(KIND_POST, &tags, "hello").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "20000",
            got: KIND_POST
        }
    ));

    let err = geochat_from_tags(KIND_GEOCHAT, &tags, "  ").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("content")));

    let err = geochat_from_tags(KIND_GEOCHAT, &[], "hello").unwrap_err();
    assert!(matches!(err, EventParseError::MissingTag("g")));
}

#[test]
fn geochat_roundtrip_from_tags() {
    let tags = vec![
        vec!["g".to_string(), "dr5rsj7".to_string()],
        vec!["n".to_string(), "alex".to_string()],
        vec!["t".to_string(), "teleport".to_string()],
    ];

    let geochat = geochat_from_tags(KIND_GEOCHAT, &tags, "hello").unwrap();

    assert_eq!(geochat.geohash, "dr5rsj7");
    assert_eq!(geochat.content, "hello");
    assert_eq!(geochat.nickname.as_deref(), Some("alex"));
    assert!(geochat.teleported);
}

#[test]
fn geochat_from_tags_rejects_invalid_optional_tags() {
    let err = geochat_from_tags(
        KIND_GEOCHAT,
        &[
            vec!["g".to_string(), "dr5rsj7".to_string()],
            vec!["n".to_string(), " ".to_string()],
        ],
        "hello",
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("n")));

    let err = geochat_from_tags(
        KIND_GEOCHAT,
        &[
            vec!["g".to_string(), "dr5rsj7".to_string()],
            vec!["t".to_string(), " ".to_string()],
        ],
        "hello",
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("t")));
}

#[test]
fn geochat_metadata_and_index_from_event_roundtrip() {
    let tags = vec![
        vec!["g".to_string(), "dr5rsj7".to_string()],
        vec!["n".to_string(), "alex".to_string()],
        vec!["t".to_string(), "teleport".to_string()],
    ];
    let metadata = metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        77,
        KIND_GEOCHAT,
        "hello".to_string(),
        tags.clone(),
    )
    .unwrap();
    assert_eq!(metadata.id, "id");
    assert_eq!(metadata.author, "author");
    assert_eq!(metadata.published_at, 77);
    assert_eq!(metadata.kind, KIND_GEOCHAT);
    assert_eq!(metadata.geochat.geohash, "dr5rsj7");
    assert!(metadata.geochat.teleported);

    let index = index_from_event(
        "id".to_string(),
        "author".to_string(),
        77,
        KIND_GEOCHAT,
        "hello".to_string(),
        tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(index.event.kind, KIND_GEOCHAT);
    assert_eq!(index.event.sig, "sig");
    assert_eq!(index.metadata.geochat.geohash, "dr5rsj7");
}
