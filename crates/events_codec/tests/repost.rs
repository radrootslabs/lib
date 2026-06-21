#![cfg(feature = "serde_json")]

use radroots_events::{
    kinds::{KIND_ARTICLE, KIND_GENERIC_REPOST, KIND_POST, KIND_REACTION, KIND_REPOST},
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

fn replace_tag_value(tags: &mut [Vec<String>], key: &str, value: &str) {
    let tag = tags
        .iter_mut()
        .find(|tag| tag.first().map(|entry| entry.as_str()) == Some(key))
        .expect("tag");
    tag[1] = value.to_string();
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
fn repost_event_target_codecs_cover_optional_and_error_edges() {
    let mut no_relay = note_repost();
    no_relay.content = Some("fresh note".to_string());
    if let RadrootsSocialTarget::Event { author, relays, .. } = &mut no_relay.target {
        *author = None;
        *relays = None;
    }
    let parts = repost_to_wire_parts(&no_relay).unwrap();
    assert_eq!(parts.content, "fresh note");
    assert!(!parts.tags.iter().any(|tag| {
        tag.first().map(|entry| entry.as_str()) == Some(TAG_P)
            || tag
                .get(2)
                .map(|entry| !entry.trim().is_empty())
                .unwrap_or(false)
    }));
    let decoded = repost_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.content.as_deref(), Some("fresh note"));
    assert!(matches!(
        decoded.target,
        RadrootsSocialTarget::Event { relays: None, .. }
    ));

    let mut invalid_target = note_repost();
    invalid_target.target = RadrootsSocialTarget::Address {
        address: format!("{KIND_ARTICLE}:{AUTHOR}:{ARTICLE_D_TAG}"),
        author: Some(AUTHOR.to_string()),
        event_kind: Some(KIND_ARTICLE),
        relays: None,
    };
    assert!(matches!(
        repost_build_tags(&invalid_target),
        Err(EventEncodeError::InvalidField("target"))
    ));

    let mut invalid_id = note_repost();
    if let RadrootsSocialTarget::Event { id, .. } = &mut invalid_id.target {
        *id = "not-a-lowercase-hex-id".to_string();
    }
    assert!(matches!(
        repost_build_tags(&invalid_id),
        Err(EventEncodeError::InvalidField("target.id"))
    ));

    let mut invalid_author = note_repost();
    if let RadrootsSocialTarget::Event { author, .. } = &mut invalid_author.target {
        *author = Some(" ".to_string());
    }
    assert!(matches!(
        repost_build_tags(&invalid_author),
        Err(EventEncodeError::EmptyRequiredField("target.author"))
    ));

    let mut tags = repost_build_tags(&note_repost()).unwrap();
    let event_tag = tags
        .iter_mut()
        .find(|tag| tag.first().map(String::as_str) == Some(TAG_E))
        .expect("event tag");
    event_tag.truncate(1);
    assert!(matches!(
        repost_from_event(KIND_REPOST, &tags, ""),
        Err(EventParseError::InvalidTag(TAG_E))
    ));

    let mut tags = repost_build_tags(&note_repost()).unwrap();
    replace_tag_value(&mut tags, TAG_E, "not-a-lowercase-hex-id");
    assert!(matches!(
        repost_from_event(KIND_REPOST, &tags, ""),
        Err(EventParseError::InvalidTag(TAG_E))
    ));
}

#[test]
fn generic_repost_codecs_cover_event_targets_and_error_edges() {
    let generic = RadrootsGenericRepost {
        target: RadrootsSocialTarget::Event {
            id: EVENT_ID.to_string(),
            author: Some(AUTHOR.to_string()),
            event_kind: Some(KIND_REACTION),
            relays: Some(vec![
                " ".to_string(),
                "wss://relay.example.test".to_string(),
            ]),
        },
        target_kind: KIND_REACTION,
        content: None,
    };
    let parts = generic_repost_to_wire_parts(&generic).unwrap();
    assert!(has_tag(&parts.tags, TAG_E, EVENT_ID));
    assert!(has_tag(
        &parts.tags,
        TAG_K,
        KIND_REACTION.to_string().as_str()
    ));
    let decoded = generic_repost_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert!(decoded.content.is_none());
    assert!(matches!(
        decoded.target,
        RadrootsSocialTarget::Event {
            event_kind: Some(KIND_REACTION),
            ..
        }
    ));

    let wrong_kind = generic_repost_from_event(KIND_REPOST, &parts.tags, "").unwrap_err();
    assert!(matches!(
        wrong_kind,
        EventParseError::InvalidKind {
            expected: "16",
            got: KIND_REPOST
        }
    ));

    let mut tags = parts.tags.clone();
    replace_tag_value(&mut tags, TAG_K, KIND_POST.to_string().as_str());
    assert!(matches!(
        generic_repost_from_event(KIND_GENERIC_REPOST, &tags, ""),
        Err(EventParseError::InvalidTag(TAG_K))
    ));

    let mut tags = parts.tags.clone();
    replace_tag_value(&mut tags, TAG_K, "not-a-number");
    assert!(matches!(
        generic_repost_from_event(KIND_GENERIC_REPOST, &tags, ""),
        Err(EventParseError::InvalidNumber(TAG_K, _))
    ));

    let tags = vec![vec![TAG_K.to_string(), KIND_REACTION.to_string()]];
    assert!(matches!(
        generic_repost_from_event(KIND_GENERIC_REPOST, &tags, ""),
        Err(EventParseError::MissingTag(TAG_E))
    ));

    let mut tags = generic_repost_build_tags(&generic_article_repost()).unwrap();
    replace_tag_value(
        &mut tags,
        TAG_A,
        format!("{KIND_REACTION}:{AUTHOR}:{ARTICLE_D_TAG}").as_str(),
    );
    assert!(matches!(
        generic_repost_from_event(KIND_GENERIC_REPOST, &tags, ""),
        Err(EventParseError::InvalidTag(TAG_A))
    ));

    let mut tags = generic_repost_build_tags(&generic).unwrap();
    let event_tag = tags
        .iter_mut()
        .find(|tag| tag.first().map(String::as_str) == Some(TAG_E))
        .expect("event tag");
    event_tag.truncate(1);
    assert!(matches!(
        generic_repost_from_event(KIND_GENERIC_REPOST, &tags, ""),
        Err(EventParseError::InvalidTag(TAG_E))
    ));

    let mut tags = generic_repost_build_tags(&generic).unwrap();
    replace_tag_value(&mut tags, TAG_E, "not-a-lowercase-hex-id");
    assert!(matches!(
        generic_repost_from_event(KIND_GENERIC_REPOST, &tags, ""),
        Err(EventParseError::InvalidTag(TAG_E))
    ));

    let mut generic = generic_article_repost();
    generic.target_kind = KIND_REACTION;
    assert!(matches!(
        generic_repost_build_tags(&generic),
        Err(EventEncodeError::InvalidField("target_kind"))
    ));

    let mut generic = generic_article_repost();
    if let RadrootsSocialTarget::Address { author, relays, .. } = &mut generic.target {
        *author = Some(" ".to_string());
        *relays = None;
    }
    assert!(matches!(
        generic_repost_build_tags(&generic),
        Err(EventEncodeError::EmptyRequiredField("target.author"))
    ));

    let mut generic = generic_article_repost();
    generic.target = RadrootsSocialTarget::External {
        id: "https://example.test/repost-target".to_string(),
        external_kind: "web".to_string(),
        hint: None,
    };
    assert!(matches!(
        generic_repost_build_tags(&generic),
        Err(EventEncodeError::InvalidField("target"))
    ));

    let mut generic = RadrootsGenericRepost {
        target: RadrootsSocialTarget::Event {
            id: EVENT_ID.to_string(),
            author: None,
            event_kind: None,
            relays: None,
        },
        target_kind: KIND_REACTION,
        content: None,
    };
    assert!(matches!(
        generic_repost_build_tags(&generic),
        Err(EventEncodeError::InvalidField("target_kind"))
    ));

    if let RadrootsSocialTarget::Event { event_kind, .. } = &mut generic.target {
        *event_kind = Some(KIND_REACTION);
    }
    generic.target_kind = KIND_POST;
    assert!(matches!(
        generic_repost_build_tags(&generic),
        Err(EventEncodeError::InvalidField("target_kind"))
    ));
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
