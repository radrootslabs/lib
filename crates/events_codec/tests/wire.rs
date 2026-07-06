use radroots_events::contract::RadrootsContractValidationError;
use radroots_events::draft::RadrootsDraftError;
use radroots_events::kinds::{KIND_KNOWLEDGE_CLAIM, KIND_KNOWLEDGE_SOURCE, KIND_POST};
use radroots_events_codec::wire::{
    WireEventParts, canonicalize_tags, empty_content, to_frozen_draft,
};

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
fn wire_to_frozen_draft_copies_fields_and_computes_expected_id() {
    let parts = WireEventParts {
        kind: KIND_POST,
        content: "hello".to_string(),
        tags: vec![vec!["t".to_string(), "a".to_string()]],
    };

    let draft =
        to_frozen_draft(parts, "radroots.social.post.v1", "a".repeat(64), 99).expect("draft");

    assert_eq!(draft.kind, KIND_POST);
    assert_eq!(draft.created_at, 99);
    assert_eq!(draft.expected_pubkey, "a".repeat(64));
    assert_eq!(draft.content, "hello");
    assert_eq!(draft.tags.len(), 1);
    assert_eq!(draft.expected_event_id.len(), 64);
}

#[test]
fn wire_to_frozen_draft_rejects_contract_shape_errors() {
    let missing_contract_tag = WireEventParts {
        kind: KIND_KNOWLEDGE_CLAIM,
        content: r#"{"schema":"radroots.knowledge.claim.v1","schema_version":1}"#.to_string(),
        tags: Vec::new(),
    };
    let error = to_frozen_draft(
        missing_contract_tag,
        "radroots.knowledge.claim.v1",
        "a".repeat(64),
        99,
    )
    .expect_err("missing contract tag");
    assert!(matches!(
        error,
        RadrootsDraftError::ContractShape {
            error: RadrootsContractValidationError::MissingTag {
                name: "contract",
                ..
            },
            ..
        }
    ));

    let invalid_relay = WireEventParts {
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
    let error = to_frozen_draft(
        invalid_relay,
        "radroots.knowledge.claim.v1",
        "a".repeat(64),
        99,
    )
    .expect_err("invalid relay");
    assert!(matches!(
        error,
        RadrootsDraftError::ContractShape {
            error: RadrootsContractValidationError::TagValueMismatch { name: "source", .. },
            ..
        }
    ));
}

#[test]
fn wire_empty_content_is_empty_string() {
    let content = empty_content();
    assert!(content.is_empty());
}
