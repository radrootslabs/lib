#![cfg(feature = "serde_json")]

use radroots_events::{
    kinds::{KIND_ARTICLE, KIND_GENERIC_REPOST, KIND_POST, KIND_REPOST},
    repost::{RadrootsGenericRepost, RadrootsRepost},
    social::RadrootsSocialTarget,
    tags::{TAG_A, TAG_E, TAG_K, TAG_P},
};
use radroots_events_codec::{
    error::{EventEncodeError, EventParseError},
    repost::{
        decode::{
            generic_repost_data_from_event, generic_repost_from_event,
            generic_repost_parsed_from_event, repost_data_from_event, repost_from_event,
            repost_parsed_from_event,
        },
        encode::{
            generic_repost_build_tags, generic_repost_to_wire_parts,
            generic_repost_to_wire_parts_with_kind, repost_build_tags, repost_to_wire_parts,
            repost_to_wire_parts_with_kind,
        },
    },
};

const EVENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const AUTHOR: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ARTICLE_D_TAG: &str = "DDDDDDDDDDDDDDDDDDDDDA";

fn note_repost() -> RadrootsRepost {
    RadrootsRepost {
        target: RadrootsSocialTarget::Event {
            id: EVENT_ID.to_string(),
            author: Some(AUTHOR.to_string()),
            event_kind: Some(KIND_POST),
            relays: Some(vec!["wss://relay.example.test".to_string()]),
        },
        content: None,
    }
}

fn generic_article_repost() -> RadrootsGenericRepost {
    RadrootsGenericRepost {
        target: RadrootsSocialTarget::Address {
            address: format!("{KIND_ARTICLE}:{AUTHOR}:{ARTICLE_D_TAG}"),
            author: Some(AUTHOR.to_string()),
            event_kind: Some(KIND_ARTICLE),
            relays: Some(vec!["wss://relay.example.test".to_string()]),
        },
        target_kind: KIND_ARTICLE,
        content: Some("{\"kind\":30023}".to_string()),
    }
}

fn has_tag(tags: &[Vec<String>], key: &str, value: &str) -> bool {
    tags.iter().any(|tag| {
        tag.first().map(|entry| entry.as_str()) == Some(key)
            && tag.get(1).map(|entry| entry.as_str()) == Some(value)
    })
}

#[test]
fn repost_to_wire_parts_roundtrips_kind_one_target() {
    let repost = note_repost();
    let parts = repost_to_wire_parts(&repost).unwrap();

    assert_eq!(parts.kind, KIND_REPOST);
    assert!(parts.content.is_empty());
    assert!(has_tag(&parts.tags, TAG_E, EVENT_ID));
    assert!(has_tag(&parts.tags, TAG_P, AUTHOR));

    let decoded = repost_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert!(matches!(
        decoded.target,
        RadrootsSocialTarget::Event {
            event_kind: Some(KIND_POST),
            ..
        }
    ));
    assert!(decoded.content.is_none());
}

#[test]
fn generic_repost_to_wire_parts_roundtrips_address_target() {
    let repost = generic_article_repost();
    let parts = generic_repost_to_wire_parts(&repost).unwrap();

    assert_eq!(parts.kind, KIND_GENERIC_REPOST);
    assert_eq!(parts.content, "{\"kind\":30023}");
    assert!(has_tag(
        &parts.tags,
        TAG_A,
        format!("{KIND_ARTICLE}:{AUTHOR}:{ARTICLE_D_TAG}").as_str()
    ));
    assert!(has_tag(
        &parts.tags,
        TAG_K,
        KIND_ARTICLE.to_string().as_str()
    ));

    let decoded = generic_repost_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.target_kind, KIND_ARTICLE);
    assert!(matches!(
        decoded.target,
        RadrootsSocialTarget::Address {
            event_kind: Some(KIND_ARTICLE),
            ..
        }
    ));
    assert_eq!(decoded.content.as_deref(), Some("{\"kind\":30023}"));
}

#[test]
fn repost_codecs_reject_wrong_kind_and_wrong_target_kind() {
    assert!(matches!(
        repost_to_wire_parts_with_kind(&note_repost(), KIND_GENERIC_REPOST),
        Err(EventEncodeError::InvalidKind(KIND_GENERIC_REPOST))
    ));
    assert!(matches!(
        generic_repost_to_wire_parts_with_kind(&generic_article_repost(), KIND_REPOST),
        Err(EventEncodeError::InvalidKind(KIND_REPOST))
    ));

    let mut repost = note_repost();
    if let RadrootsSocialTarget::Event { event_kind, .. } = &mut repost.target {
        *event_kind = Some(KIND_ARTICLE);
    }
    assert!(matches!(
        repost_build_tags(&repost),
        Err(EventEncodeError::InvalidField("target_kind"))
    ));

    let mut generic = generic_article_repost();
    generic.target_kind = KIND_POST;
    assert!(matches!(
        generic_repost_build_tags(&generic),
        Err(EventEncodeError::InvalidField("target_kind"))
    ));

    let err = repost_from_event(KIND_GENERIC_REPOST, &[], "").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "6",
            got: KIND_GENERIC_REPOST
        }
    ));

    let err = generic_repost_from_event(KIND_GENERIC_REPOST, &[], "").unwrap_err();
    assert!(matches!(err, EventParseError::MissingTag(TAG_K)));
}

#[test]
fn repost_wrappers_preserve_event_metadata() {
    let parts = repost_to_wire_parts(&note_repost()).unwrap();
    let data = repost_data_from_event(
        "repost_id".to_string(),
        "author".to_string(),
        10,
        parts.kind,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(data.kind, KIND_REPOST);
    assert_eq!(data.published_at, 10);

    let parsed = repost_parsed_from_event(
        "repost_id".to_string(),
        "author".to_string(),
        10,
        parts.kind,
        parts.content,
        parts.tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(parsed.event.sig, "sig");

    let generic_parts = generic_repost_to_wire_parts(&generic_article_repost()).unwrap();
    let generic_data = generic_repost_data_from_event(
        "generic_id".to_string(),
        "author".to_string(),
        11,
        generic_parts.kind,
        generic_parts.content.clone(),
        generic_parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(generic_data.data.target_kind, KIND_ARTICLE);

    let generic_parsed = generic_repost_parsed_from_event(
        "generic_id".to_string(),
        "author".to_string(),
        11,
        generic_parts.kind,
        generic_parts.content,
        generic_parts.tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(generic_parsed.event.created_at, 11);
}
