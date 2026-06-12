use radroots_events::{
    comment::RadrootsComment,
    kinds::{KIND_ARTICLE, KIND_COMMENT, KIND_POST},
    social::RadrootsSocialTarget,
    tags::{TAG_E_PREV, TAG_E_ROOT},
};
use radroots_events_codec::comment::decode::{
    comment_from_tags, data_from_event, parsed_from_event,
};
use radroots_events_codec::comment::encode::{
    comment_build_tags, to_wire_parts, to_wire_parts_with_kind,
};
use radroots_events_codec::error::{EventEncodeError, EventParseError};

const ROOT_ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const PARENT_ID: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
const AUTHOR: &str = "author_pubkey";
const PARENT_AUTHOR: &str = "parent_pubkey";
const D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";

fn event_target(id: &str, author: &str, kind: u32) -> RadrootsSocialTarget {
    RadrootsSocialTarget::Event {
        id: id.to_string(),
        author: Some(author.to_string()),
        event_kind: Some(kind),
        relays: Some(vec!["wss://relay.example.test".to_string()]),
    }
}

fn address_target(author: &str, kind: u32, d_tag: &str) -> RadrootsSocialTarget {
    RadrootsSocialTarget::Address {
        address: format!("{kind}:{author}:{d_tag}"),
        author: Some(author.to_string()),
        event_kind: Some(kind),
        relays: Some(vec!["wss://relay2.example.test".to_string()]),
    }
}

fn external_target(id: &str, kind: &str) -> RadrootsSocialTarget {
    RadrootsSocialTarget::External {
        id: id.to_string(),
        external_kind: kind.to_string(),
        hint: Some("https://example.test/object".to_string()),
    }
}

fn assert_event_target(target: &RadrootsSocialTarget, id: &str, author: &str, kind: u32) {
    match target {
        RadrootsSocialTarget::Event {
            id: actual_id,
            author: actual_author,
            event_kind,
            relays,
        } => {
            assert_eq!(actual_id, id);
            assert_eq!(actual_author.as_deref(), Some(author));
            assert_eq!(*event_kind, Some(kind));
            assert_eq!(relays.as_ref().map(Vec::len), Some(1));
        }
        _ => panic!("expected event target"),
    }
}

fn assert_address_target(target: &RadrootsSocialTarget, author: &str, kind: u32, d_tag: &str) {
    match target {
        RadrootsSocialTarget::Address {
            address,
            author: actual_author,
            event_kind,
            relays,
        } => {
            assert_eq!(address, &format!("{kind}:{author}:{d_tag}"));
            assert_eq!(actual_author.as_deref(), Some(author));
            assert_eq!(*event_kind, Some(kind));
            assert_eq!(relays.as_ref().map(Vec::len), Some(1));
        }
        _ => panic!("expected address target"),
    }
}

#[test]
fn comment_build_tags_requires_strict_nip22_target_fields() {
    let comment = RadrootsComment {
        root: RadrootsSocialTarget::Event {
            id: "not-hex".to_string(),
            author: Some(AUTHOR.to_string()),
            event_kind: Some(KIND_ARTICLE),
            relays: None,
        },
        parent: event_target(PARENT_ID, PARENT_AUTHOR, KIND_ARTICLE),
        content: "hello".to_string(),
    };
    assert!(matches!(
        comment_build_tags(&comment),
        Err(EventEncodeError::InvalidField("root"))
    ));

    let comment = RadrootsComment {
        root: event_target(ROOT_ID, AUTHOR, KIND_POST),
        parent: event_target(PARENT_ID, PARENT_AUTHOR, KIND_ARTICLE),
        content: "hello".to_string(),
    };
    assert!(matches!(
        comment_build_tags(&comment),
        Err(EventEncodeError::InvalidField("root"))
    ));
}

#[test]
fn comment_to_wire_parts_requires_content() {
    let comment = RadrootsComment {
        root: event_target(ROOT_ID, AUTHOR, KIND_ARTICLE),
        parent: event_target(PARENT_ID, PARENT_AUTHOR, KIND_ARTICLE),
        content: "   ".to_string(),
    };

    let err = to_wire_parts(&comment).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("content")
    ));
}

#[test]
fn comment_roundtrips_event_and_address_targets() {
    let comment = RadrootsComment {
        root: event_target(ROOT_ID, AUTHOR, KIND_ARTICLE),
        parent: address_target(PARENT_AUTHOR, KIND_ARTICLE, D_TAG),
        content: "hello".to_string(),
    };
    let parts = to_wire_parts(&comment).unwrap();

    assert_eq!(parts.kind, KIND_COMMENT);
    assert!(parts.tags.iter().any(|tag| tag[0] == "E"));
    assert!(parts.tags.iter().any(|tag| tag[0] == "P"));
    assert!(parts.tags.iter().any(|tag| tag[0] == "K"));
    assert!(parts.tags.iter().any(|tag| tag[0] == "a"));
    assert!(parts.tags.iter().any(|tag| tag[0] == "p"));
    assert!(parts.tags.iter().any(|tag| tag[0] == "k"));

    let parsed = comment_from_tags(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_event_target(&parsed.root, ROOT_ID, AUTHOR, KIND_ARTICLE);
    assert_address_target(&parsed.parent, PARENT_AUTHOR, KIND_ARTICLE, D_TAG);
    assert_eq!(parsed.content, "hello");

    let custom_parts = to_wire_parts_with_kind(&comment, KIND_POST).unwrap();
    assert_eq!(custom_parts.kind, KIND_POST);
}

#[test]
fn comment_roundtrips_external_targets() {
    let comment = RadrootsComment {
        root: external_target("https://example.test/root", "web"),
        parent: external_target("https://example.test/parent", "web"),
        content: "external comment".to_string(),
    };
    let parts = to_wire_parts(&comment).unwrap();
    let parsed = comment_from_tags(parts.kind, &parts.tags, &parts.content).unwrap();

    match parsed.root {
        RadrootsSocialTarget::External {
            id,
            external_kind,
            hint,
        } => {
            assert_eq!(id, "https://example.test/root");
            assert_eq!(external_kind, "web");
            assert_eq!(hint.as_deref(), Some("https://example.test/object"));
        }
        _ => panic!("expected external root"),
    }
    match parsed.parent {
        RadrootsSocialTarget::External { id, .. } => {
            assert_eq!(id, "https://example.test/parent");
        }
        _ => panic!("expected external parent"),
    }
}

#[test]
fn comment_from_tags_rejects_legacy_missing_and_kind1_shapes() {
    let legacy_tags = vec![vec![
        TAG_E_ROOT.to_string(),
        ROOT_ID.to_string(),
        AUTHOR.to_string(),
        KIND_ARTICLE.to_string(),
    ]];
    assert!(matches!(
        comment_from_tags(KIND_COMMENT, &legacy_tags, "hello"),
        Err(EventParseError::InvalidTag(TAG_E_ROOT))
    ));

    let legacy_parent_tags = vec![
        vec!["E".to_string(), ROOT_ID.to_string()],
        vec!["P".to_string(), AUTHOR.to_string()],
        vec!["K".to_string(), KIND_ARTICLE.to_string()],
        vec![
            TAG_E_PREV.to_string(),
            PARENT_ID.to_string(),
            PARENT_AUTHOR.to_string(),
            KIND_ARTICLE.to_string(),
        ],
    ];
    assert!(matches!(
        comment_from_tags(KIND_COMMENT, &legacy_parent_tags, "hello"),
        Err(EventParseError::InvalidTag(TAG_E_PREV))
    ));

    let missing_parent_tags = vec![
        vec!["E".to_string(), ROOT_ID.to_string()],
        vec!["P".to_string(), AUTHOR.to_string()],
        vec!["K".to_string(), KIND_ARTICLE.to_string()],
    ];
    assert!(matches!(
        comment_from_tags(KIND_COMMENT, &missing_parent_tags, "hello"),
        Err(EventParseError::MissingTag("e"))
    ));

    let kind1_tags = vec![
        vec!["E".to_string(), ROOT_ID.to_string()],
        vec!["P".to_string(), AUTHOR.to_string()],
        vec!["K".to_string(), KIND_POST.to_string()],
        vec!["e".to_string(), PARENT_ID.to_string()],
        vec!["p".to_string(), PARENT_AUTHOR.to_string()],
        vec!["k".to_string(), KIND_ARTICLE.to_string()],
    ];
    assert!(matches!(
        comment_from_tags(KIND_COMMENT, &kind1_tags, "hello"),
        Err(EventParseError::InvalidTag("K"))
    ));
}

#[test]
fn comment_from_tags_rejects_empty_content_and_wrong_kind() {
    let tags = comment_build_tags(&RadrootsComment {
        root: event_target(ROOT_ID, AUTHOR, KIND_ARTICLE),
        parent: event_target(PARENT_ID, PARENT_AUTHOR, KIND_ARTICLE),
        content: "hello".to_string(),
    })
    .unwrap();

    assert!(matches!(
        comment_from_tags(KIND_COMMENT, &tags, "   "),
        Err(EventParseError::InvalidTag("content"))
    ));
    assert!(matches!(
        comment_from_tags(KIND_POST, &tags, "hello"),
        Err(EventParseError::InvalidKind {
            expected: "1111",
            got: KIND_POST
        })
    ));
}

#[test]
fn comment_metadata_and_index_from_event_roundtrip() {
    let parts = to_wire_parts(&RadrootsComment {
        root: event_target(ROOT_ID, AUTHOR, KIND_ARTICLE),
        parent: address_target(PARENT_AUTHOR, KIND_ARTICLE, D_TAG),
        content: "hello".to_string(),
    })
    .unwrap();

    let metadata = data_from_event(
        "id".to_string(),
        "author".to_string(),
        77,
        KIND_COMMENT,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(metadata.id, "id");
    assert_eq!(metadata.published_at, 77);
    assert_event_target(&metadata.data.root, ROOT_ID, AUTHOR, KIND_ARTICLE);

    let index = parsed_from_event(
        "id".to_string(),
        "author".to_string(),
        77,
        KIND_COMMENT,
        parts.content,
        parts.tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(index.event.created_at, 77);
    assert_eq!(index.event.sig, "sig");
    assert_address_target(&index.data.data.parent, PARENT_AUTHOR, KIND_ARTICLE, D_TAG);
}
