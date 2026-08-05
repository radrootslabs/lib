use radroots_event::GenericEventDraft;
use radroots_event::draft::DraftError;
use radroots_event::envelope::kind::{KIND_GEOCHAT, KIND_PROFILE};
use radroots_event::wire::Nip01EventWireParts;
use radroots_event_codec::decode::wire::{canonicalize_tags, empty_content};

#[test]
fn wire_canonicalize_tags_trims_sorts_and_dedups() {
    let mut tags = vec![
        vec![" z ".to_string(), "b".to_string()],
        vec!["t".to_string(), "a".to_string()],
        vec!["".to_string(), "x".to_string()],
        vec![" t ".to_string(), "a ".to_string()],
        vec!["t".to_string(), "a".to_string()],
    ];

    canonicalize_tags(&mut tags);

    assert_eq!(
        tags,
        vec![
            vec!["t".to_string(), "a".to_string()],
            vec!["z".to_string(), "b".to_string()],
        ]
    );
}

#[test]
fn wire_parts_are_canonical_event_owned_payload_parts() {
    let parts = Nip01EventWireParts {
        kind: KIND_GEOCHAT,
        content: "hello".to_string(),
        tags: vec![vec!["t".to_string(), "a".to_string()]],
    };
    let json = serde_json::to_string(&parts).expect("json");
    let decoded: Nip01EventWireParts = serde_json::from_str(&json).expect("decoded");

    assert_eq!(decoded, parts);
}

#[test]
fn draft_validation_accepts_wire_parts_without_signed_envelope() {
    let parts = Nip01EventWireParts {
        kind: KIND_GEOCHAT,
        content: "hello".to_string(),
        tags: vec![vec!["t".to_string(), "a".to_string()]],
    };

    let draft = GenericEventDraft::new(
        "radroots.social.geochat.v1",
        parts.kind,
        99,
        parts.tags,
        parts.content,
        "a".repeat(64),
    )
    .expect("draft");

    assert_eq!(draft.kind_u32(), KIND_GEOCHAT);
    assert_eq!(draft.created_at_u64(), 99);
    assert_eq!(draft.expected_pubkey().to_hex(), "a".repeat(64));
    assert_eq!(draft.content(), "hello");
    assert_eq!(draft.tags().len(), 1);
    assert_eq!(draft.expected_event_id_hex().len(), 64);
}

#[test]
fn generic_draft_rejects_typed_only_contracts() {
    let typed_only = Nip01EventWireParts {
        kind: KIND_PROFILE,
        content: r#"{"name":"Alice"}"#.to_string(),
        tags: Vec::new(),
    };
    let error = GenericEventDraft::new(
        "radroots.profile.metadata.v1",
        typed_only.kind,
        99,
        typed_only.tags,
        typed_only.content,
        "a".repeat(64),
    )
    .expect_err("typed-only contract");
    assert!(matches!(
        error,
        DraftError::ContractNotDraftAuthorable { .. }
    ));
}

#[test]
fn wire_empty_content_is_empty_string() {
    let content = empty_content();
    assert!(content.is_empty());
}
