use radroots_event::contract::ContractValidationError;
use radroots_event::draft::{DraftError, EventDraft};
use radroots_event::envelope::kind::{KIND_GEOCHAT, KIND_KNOWLEDGE_CLAIM, KIND_KNOWLEDGE_SOURCE};
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

    let draft = EventDraft::new(
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
fn draft_validation_rejects_contract_shape_errors() {
    let missing_contract_tag = Nip01EventWireParts {
        kind: KIND_KNOWLEDGE_CLAIM,
        content: r#"{"schema":"radroots.knowledge.claim.v1","schema_version":1}"#.to_string(),
        tags: Vec::new(),
    };
    let error = EventDraft::new(
        "radroots.knowledge.claim.v1",
        missing_contract_tag.kind,
        99,
        missing_contract_tag.tags,
        missing_contract_tag.content,
        "a".repeat(64),
    )
    .expect_err("missing contract tag");
    assert!(matches!(
        error,
        DraftError::ContractShape {
            error: ContractValidationError::MissingTag {
                name: "contract",
                ..
            },
            ..
        }
    ));

    let invalid_relay = Nip01EventWireParts {
        kind: KIND_KNOWLEDGE_CLAIM,
        content: r#"{"schema":"radroots.knowledge.claim.v1","schema_version":1}"#.to_string(),
        tags: vec![
            vec![
                "contract".to_string(),
                "radroots.knowledge.claim.v1".to_string(),
            ],
            vec![
                "source".to_string(),
                "b".repeat(64),
                "a".repeat(64),
                KIND_KNOWLEDGE_SOURCE.to_string(),
                String::new(),
                "http://relay.radroots.example".to_string(),
            ],
        ],
    };
    let error = EventDraft::new(
        "radroots.knowledge.claim.v1",
        invalid_relay.kind,
        99,
        invalid_relay.tags,
        invalid_relay.content,
        "a".repeat(64),
    )
    .expect_err("invalid relay");
    assert!(matches!(
        error,
        DraftError::ContractShape {
            error: ContractValidationError::TagValueMismatch { name: "source", .. },
            ..
        }
    ));
}

#[test]
fn wire_empty_content_is_empty_string() {
    let content = empty_content();
    assert!(content.is_empty());
}
