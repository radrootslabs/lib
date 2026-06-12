use radroots_events::{
    kinds::{KIND_ARTICLE, KIND_POST, KIND_REACTION},
    reaction::RadrootsReaction,
    social::RadrootsSocialTarget,
    tags::TAG_E_ROOT,
};
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::reaction::decode::{
    data_from_event, parsed_from_event, reaction_from_tags,
};
use radroots_events_codec::reaction::encode::{
    reaction_build_tags, to_wire_parts, to_wire_parts_with_kind,
};

const EVENT_ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const AUTHOR: &str = "author_pubkey";
const D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";

fn event_target() -> RadrootsSocialTarget {
    RadrootsSocialTarget::Event {
        id: EVENT_ID.to_string(),
        author: Some(AUTHOR.to_string()),
        event_kind: Some(KIND_ARTICLE),
        relays: Some(vec!["wss://relay.example.test".to_string()]),
    }
}

fn address_target() -> RadrootsSocialTarget {
    RadrootsSocialTarget::Address {
        address: format!("{}:{AUTHOR}:{D_TAG}", KIND_ARTICLE),
        author: Some(AUTHOR.to_string()),
        event_kind: Some(KIND_ARTICLE),
        relays: Some(vec!["wss://relay2.example.test".to_string()]),
    }
}

fn assert_event_target(target: &RadrootsSocialTarget) {
    match target {
        RadrootsSocialTarget::Event {
            id,
            author,
            event_kind,
            relays,
        } => {
            assert_eq!(id, EVENT_ID);
            assert_eq!(author.as_deref(), Some(AUTHOR));
            assert_eq!(*event_kind, Some(KIND_ARTICLE));
            assert_eq!(relays.as_ref().map(Vec::len), Some(1));
        }
        _ => panic!("expected event target"),
    }
}

fn assert_address_target(target: &RadrootsSocialTarget) {
    match target {
        RadrootsSocialTarget::Address {
            address,
            author,
            event_kind,
            relays,
        } => {
            assert_eq!(address, &format!("{}:{AUTHOR}:{D_TAG}", KIND_ARTICLE));
            assert_eq!(author.as_deref(), Some(AUTHOR));
            assert_eq!(*event_kind, Some(KIND_ARTICLE));
            assert_eq!(relays.as_ref().map(Vec::len), Some(1));
        }
        _ => panic!("expected address target"),
    }
}

#[test]
fn reaction_build_tags_requires_valid_event_or_address_target() {
    let reaction = RadrootsReaction {
        target: RadrootsSocialTarget::Event {
            id: "not-hex".to_string(),
            author: Some(AUTHOR.to_string()),
            event_kind: Some(KIND_ARTICLE),
            relays: None,
        },
        content: "+".to_string(),
    };
    assert!(matches!(
        reaction_build_tags(&reaction),
        Err(EventEncodeError::InvalidField("target.id"))
    ));

    let reaction = RadrootsReaction {
        target: RadrootsSocialTarget::External {
            id: "https://example.test".to_string(),
            external_kind: "web".to_string(),
            hint: None,
        },
        content: "+".to_string(),
    };
    assert!(matches!(
        reaction_build_tags(&reaction),
        Err(EventEncodeError::InvalidField("target"))
    ));
}

#[test]
fn reaction_to_wire_parts_accepts_empty_plus_minus_emoji_and_custom_content() {
    for content in ["", "+", "-", "🔥", "harvest"] {
        let reaction = RadrootsReaction {
            target: event_target(),
            content: content.to_string(),
        };
        let parts = to_wire_parts(&reaction).unwrap();
        assert_eq!(parts.kind, KIND_REACTION);
        assert_eq!(parts.content, content);
        assert!(parts.tags.iter().any(|tag| tag[0] == "e"));
    }
}

#[test]
fn reaction_to_wire_parts_with_kind_keeps_requested_kind() {
    let reaction = RadrootsReaction {
        target: event_target(),
        content: "+".to_string(),
    };
    let parts = to_wire_parts_with_kind(&reaction, KIND_POST).unwrap();
    assert_eq!(parts.kind, KIND_POST);
    assert_eq!(parts.content, "+");
}

#[test]
fn reaction_roundtrips_event_target() {
    let reaction = RadrootsReaction {
        target: event_target(),
        content: "+".to_string(),
    };
    let parts = to_wire_parts(&reaction).unwrap();
    let parsed = reaction_from_tags(parts.kind, &parts.tags, &parts.content).unwrap();

    assert_event_target(&parsed.target);
    assert_eq!(parsed.content, "+");
}

#[test]
fn reaction_roundtrips_address_target() {
    let reaction = RadrootsReaction {
        target: address_target(),
        content: "".to_string(),
    };
    let parts = to_wire_parts(&reaction).unwrap();
    let parsed = reaction_from_tags(parts.kind, &parts.tags, &parts.content).unwrap();

    assert_address_target(&parsed.target);
    assert_eq!(parsed.content, "");
}

#[test]
fn reaction_from_tags_rejects_missing_legacy_and_mismatched_targets() {
    assert!(matches!(
        reaction_from_tags(
            KIND_REACTION,
            &[vec!["p".to_string(), AUTHOR.to_string()]],
            "+"
        ),
        Err(EventParseError::MissingTag("e"))
    ));

    assert!(matches!(
        reaction_from_tags(
            KIND_REACTION,
            &[vec![TAG_E_ROOT.to_string(), EVENT_ID.to_string()]],
            "+"
        ),
        Err(EventParseError::InvalidTag(TAG_E_ROOT))
    ));

    assert!(matches!(
        reaction_from_tags(
            KIND_REACTION,
            &[
                vec!["e".to_string(), EVENT_ID.to_string()],
                vec![
                    "a".to_string(),
                    format!("{}:{AUTHOR}:{D_TAG}", KIND_ARTICLE)
                ]
            ],
            "+"
        ),
        Err(EventParseError::InvalidTag("e"))
    ));

    assert!(matches!(
        reaction_from_tags(
            KIND_REACTION,
            &[
                vec![
                    "a".to_string(),
                    format!("{}:{AUTHOR}:{D_TAG}", KIND_ARTICLE)
                ],
                vec!["p".to_string(), "other_author".to_string()]
            ],
            "+"
        ),
        Err(EventParseError::InvalidTag("p"))
    ));
}

#[test]
fn reaction_from_tags_rejects_invalid_kind() {
    let tags = reaction_build_tags(&RadrootsReaction {
        target: event_target(),
        content: "+".to_string(),
    })
    .unwrap();

    assert!(matches!(
        reaction_from_tags(KIND_POST, &tags, "+"),
        Err(EventParseError::InvalidKind {
            expected: "7",
            got: KIND_POST
        })
    ));
}

#[test]
fn reaction_metadata_and_index_from_event_roundtrip() {
    let parts = to_wire_parts(&RadrootsReaction {
        target: event_target(),
        content: "".to_string(),
    })
    .unwrap();

    let metadata = data_from_event(
        "id".to_string(),
        "author".to_string(),
        99,
        KIND_REACTION,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(metadata.id, "id");
    assert_eq!(metadata.kind, KIND_REACTION);
    assert_event_target(&metadata.data.target);
    assert_eq!(metadata.data.content, "");

    let index = parsed_from_event(
        "id".to_string(),
        "author".to_string(),
        99,
        KIND_REACTION,
        parts.content,
        parts.tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(index.event.kind, KIND_REACTION);
    assert_eq!(index.event.sig, "sig");
    assert_event_target(&index.data.data.target);
}
