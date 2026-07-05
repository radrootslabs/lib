#![cfg(all(feature = "knowledge", feature = "nostr"))]

use std::collections::BTreeSet;

use radroots_events::RadrootsNostrEvent;
use radroots_events::contract::{
    RadrootsContractValidationError, RadrootsEventClass, all_event_contracts,
    validate_event_contract_shape,
};
use radroots_events::kinds::{
    KIND_CONTRIBUTION_ATTESTATION, KIND_KNOWLEDGE_CHANGE_PROPOSAL, KIND_KNOWLEDGE_CLAIM,
    KIND_KNOWLEDGE_FIELD_REPORT, KIND_KNOWLEDGE_RELATION, KIND_KNOWLEDGE_REVIEW,
};
use radroots_events::knowledge::{
    RADROOTS_KNOWLEDGE_CLAIM_SCHEMA, RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA, RadrootsWikiArticle,
};
use radroots_events_codec::error::EventEncodeError;
use radroots_events_codec::knowledge::{
    contribution_attestation_to_wire_parts, evidence_bounty_to_wire_parts,
    knowledge_change_proposal_to_wire_parts, knowledge_claim_to_wire_parts,
    knowledge_field_report_to_wire_parts, knowledge_relation_to_wire_parts,
    knowledge_review_to_wire_parts, knowledge_source_to_wire_parts, wiki_article_to_wire_parts,
    wiki_merge_request_to_wire_parts, wiki_redirect_to_wire_parts,
};
use radroots_events_codec::verification::{
    RadrootsDecodeError, RadrootsDecodedEvent, RadrootsNip01VerificationError,
    verify_and_decode_radroots_event,
};
use radroots_events_codec::wire::WireEventParts;
use radroots_test_fixtures::knowledge::{
    RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES, RADROOTS_KNOWLEDGE_VALID_CONTRACT_IDS,
    RadrootsKnowledgeFixture, hex_64, knowledge_claim, knowledge_field_report,
    knowledge_valid_fixtures, wiki_article,
};

fn event_from_parts(parts: WireEventParts) -> RadrootsNostrEvent {
    RadrootsNostrEvent {
        id: hex_64('0'),
        author: hex_64('a'),
        created_at: 1_800_000_000,
        kind: parts.kind,
        tags: parts.tags,
        content: parts.content,
        sig: "1".repeat(128),
    }
}

fn sign_parts(parts: WireEventParts) -> RadrootsNostrEvent {
    let tags = parts
        .tags
        .into_iter()
        .map(nostr::Tag::parse)
        .collect::<Result<Vec<_>, _>>()
        .expect("tags");
    let keys =
        nostr::Keys::parse("0101010101010101010101010101010101010101010101010101010101010101")
            .expect("keys");
    let event = nostr::EventBuilder::new(nostr::Kind::Custom(parts.kind as u16), parts.content)
        .tags(tags)
        .custom_created_at(nostr::Timestamp::from_secs(1_800_000_000))
        .sign_with_keys(&keys)
        .expect("signed event");
    RadrootsNostrEvent {
        id: event.id.to_hex(),
        author: event.pubkey.to_hex(),
        created_at: event.created_at.as_secs() as u32,
        kind: u32::from(event.kind.as_u16()),
        tags: event
            .tags
            .as_slice()
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect(),
        content: event.content,
        sig: event.sig.to_string(),
    }
}

fn parts_for_fixture(fixture: &RadrootsKnowledgeFixture) -> WireEventParts {
    match fixture {
        RadrootsKnowledgeFixture::WikiArticle(value) => wiki_article_to_wire_parts(value).unwrap(),
        RadrootsKnowledgeFixture::WikiRedirect(value) => {
            wiki_redirect_to_wire_parts(value).unwrap()
        }
        RadrootsKnowledgeFixture::WikiMergeRequest(value) => {
            wiki_merge_request_to_wire_parts(value).unwrap()
        }
        RadrootsKnowledgeFixture::KnowledgeSource(value) => {
            knowledge_source_to_wire_parts(value).unwrap()
        }
        RadrootsKnowledgeFixture::KnowledgeClaim(value) => {
            knowledge_claim_to_wire_parts(value).unwrap()
        }
        RadrootsKnowledgeFixture::KnowledgeRelation(value) => {
            knowledge_relation_to_wire_parts(value).unwrap()
        }
        RadrootsKnowledgeFixture::KnowledgeReview(value) => {
            knowledge_review_to_wire_parts(value).unwrap()
        }
        RadrootsKnowledgeFixture::KnowledgeFieldReport(value) => {
            knowledge_field_report_to_wire_parts(value).unwrap()
        }
        RadrootsKnowledgeFixture::EvidenceBounty(value) => {
            evidence_bounty_to_wire_parts(value).unwrap()
        }
        RadrootsKnowledgeFixture::KnowledgeChangeProposal(value) => {
            knowledge_change_proposal_to_wire_parts(value).unwrap()
        }
        RadrootsKnowledgeFixture::ContributionAttestation(value) => {
            contribution_attestation_to_wire_parts(value).unwrap()
        }
    }
}

#[test]
fn golden_knowledge_fixtures_cover_every_contract() {
    let fixtures = knowledge_valid_fixtures();
    let fixture_contracts = fixtures
        .iter()
        .map(|fixture| fixture.contract_id)
        .collect::<BTreeSet<_>>();
    let registry_contracts = all_event_contracts()
        .iter()
        .filter(|contract| {
            RADROOTS_KNOWLEDGE_VALID_CONTRACT_IDS
                .iter()
                .any(|contract_id| *contract_id == contract.id)
        })
        .map(|contract| contract.id)
        .collect::<BTreeSet<_>>();

    assert_eq!(fixture_contracts, registry_contracts);

    for fixture in fixtures {
        let event = event_from_parts(parts_for_fixture(&fixture.data));
        validate_event_contract_shape(&event, fixture.contract_id).unwrap();
        assert_eq!(event.kind, fixture.kind, "{}", fixture.id);
    }
}

#[test]
fn adversarial_knowledge_fixtures_reject_at_expected_stages() {
    let malformed = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "malformed_tags")
        .unwrap();
    let mut malformed_event =
        event_from_parts(knowledge_claim_to_wire_parts(&knowledge_claim()).unwrap());
    malformed_event.tags.push(vec![
        "contract".to_string(),
        RADROOTS_KNOWLEDGE_CLAIM_SCHEMA.to_string(),
    ]);
    let error = validate_event_contract_shape(&malformed_event, RADROOTS_KNOWLEDGE_CLAIM_SCHEMA)
        .unwrap_err();
    assert_eq!(malformed.pipeline_stage, "contract_validation");
    assert_eq!(malformed.expected_error_code, error.code());

    let wrong_schema = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "wrong_schema")
        .unwrap();
    let mut wrong_schema_event =
        event_from_parts(knowledge_claim_to_wire_parts(&knowledge_claim()).unwrap());
    let mut wrong_schema_value: serde_json::Value =
        serde_json::from_str(&wrong_schema_event.content).unwrap();
    wrong_schema_value["schema"] = serde_json::Value::from("radroots.knowledge.relation.v1");
    wrong_schema_event.content = serde_json::to_string(&wrong_schema_value).unwrap();
    let error = validate_event_contract_shape(&wrong_schema_event, RADROOTS_KNOWLEDGE_CLAIM_SCHEMA)
        .unwrap_err();
    assert_eq!(wrong_schema.pipeline_stage, "contract_validation");
    assert_eq!(wrong_schema.expected_error_code, error.code());

    let missing_contract = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "missing_contract_id")
        .unwrap();
    let mut missing_contract_event =
        event_from_parts(knowledge_claim_to_wire_parts(&knowledge_claim()).unwrap());
    missing_contract_event
        .tags
        .retain(|tag| tag.first().map(|value| value.as_str()) != Some("contract"));
    let signed = sign_parts(WireEventParts {
        kind: missing_contract_event.kind,
        content: missing_contract_event.content,
        tags: missing_contract_event.tags,
    });
    let error = verify_and_decode_radroots_event(signed).unwrap_err();
    assert_eq!(missing_contract.pipeline_stage, error.code());
    assert!(matches!(error, RadrootsDecodeError::ContractValidation(_)));

    let private_coordinates = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "private_coordinate_leakage")
        .unwrap();
    let mut private_event =
        event_from_parts(knowledge_field_report_to_wire_parts(&knowledge_field_report()).unwrap());
    let mut private_value: serde_json::Value =
        serde_json::from_str(&private_event.content).unwrap();
    private_value["context"]["latitude"] = serde_json::Value::from("45.0000");
    private_event.content = serde_json::to_string(&private_value).unwrap();
    let signed = sign_parts(WireEventParts {
        kind: private_event.kind,
        content: private_event.content,
        tags: private_event.tags,
    });
    let error = verify_and_decode_radroots_event(signed).unwrap_err();
    assert_eq!(private_coordinates.pipeline_stage, error.code());
    match error {
        RadrootsDecodeError::EventParse(error) => {
            assert_eq!(private_coordinates.expected_error_code, error.code());
        }
        error => panic!("{error:?}"),
    }

    let unsupported = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "unsupported_contract_shape")
        .unwrap();
    let mut unsupported_event =
        event_from_parts(knowledge_claim_to_wire_parts(&knowledge_claim()).unwrap());
    for tag in &mut unsupported_event.tags {
        if tag.first().map(|value| value.as_str()) == Some("contract") {
            tag[1] = "radroots.knowledge.unsupported.v1".to_string();
        }
    }
    let signed = sign_parts(WireEventParts {
        kind: unsupported_event.kind,
        content: unsupported_event.content,
        tags: unsupported_event.tags,
    });
    let error = verify_and_decode_radroots_event(signed).unwrap_err();
    assert_eq!(unsupported.pipeline_stage, error.code());
    assert!(matches!(error, RadrootsDecodeError::ContractValidation(_)));
}

#[test]
fn nip54_and_signature_adversarial_fixtures_are_rejected() {
    let invalid_d_tag = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "invalid_nip54_d_tag")
        .unwrap();
    let mut article: RadrootsWikiArticle = wiki_article();
    article.d_tag = "Soil Health".to_string();
    let error = wiki_article_to_wire_parts(&article).unwrap_err();
    assert_eq!(invalid_d_tag.pipeline_stage, "encode");
    assert_eq!(invalid_d_tag.expected_error_code, error.code());
    assert!(matches!(error, EventEncodeError::InvalidField("d_tag")));

    let id_mismatch = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "id_mismatch")
        .unwrap();
    let signed = sign_parts(knowledge_claim_to_wire_parts(&knowledge_claim()).unwrap());
    let mut mutated = signed.clone();
    mutated.content = mutated.content.replace("Cover crops", "Compost");
    let error = verify_and_decode_radroots_event(mutated).unwrap_err();
    assert_eq!(id_mismatch.pipeline_stage, error.code());
    match error {
        RadrootsDecodeError::Nip01Verification(RadrootsNip01VerificationError::IdMismatch {
            ..
        }) => {}
        error => panic!("{error:?}"),
    }

    let signature_invalidity = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "signature_invalidity")
        .unwrap();
    let mut bad_signature = signed;
    bad_signature.sig = "0".repeat(128);
    let error = verify_and_decode_radroots_event(bad_signature).unwrap_err();
    assert_eq!(signature_invalidity.pipeline_stage, error.code());
    match error {
        RadrootsDecodeError::Nip01Verification(
            RadrootsNip01VerificationError::SignatureInvalid,
        ) => {}
        error => panic!("{error:?}"),
    }
}

#[test]
fn authoritative_knowledge_status_fields_are_rejected() {
    for field in [
        "review_status",
        "canon_status",
        "approved_for_canon",
        "rights_status",
        "trust_status",
        "trusted",
    ] {
        let mut event =
            event_from_parts(knowledge_claim_to_wire_parts(&knowledge_claim()).unwrap());
        let mut value: serde_json::Value = serde_json::from_str(&event.content).unwrap();
        value[field] = serde_json::Value::from("approved");
        event.content = serde_json::to_string(&value).unwrap();
        let error =
            validate_event_contract_shape(&event, RADROOTS_KNOWLEDGE_CLAIM_SCHEMA).unwrap_err();
        assert_eq!(
            error,
            RadrootsContractValidationError::ForbiddenContentField {
                contract_id: RADROOTS_KNOWLEDGE_CLAIM_SCHEMA,
                field,
            }
        );
    }
}

#[test]
fn immutable_knowledge_contracts_are_regular_events() {
    let regular_ids = [
        (RADROOTS_KNOWLEDGE_CLAIM_SCHEMA, KIND_KNOWLEDGE_CLAIM),
        ("radroots.knowledge.relation.v1", KIND_KNOWLEDGE_RELATION),
        ("radroots.knowledge.review.v1", KIND_KNOWLEDGE_REVIEW),
        (
            RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA,
            KIND_KNOWLEDGE_FIELD_REPORT,
        ),
        (
            "radroots.knowledge.change_proposal.v1",
            KIND_KNOWLEDGE_CHANGE_PROPOSAL,
        ),
        (
            "radroots.knowledge.contribution_attestation.v1",
            KIND_CONTRIBUTION_ATTESTATION,
        ),
    ];
    for (contract_id, kind) in regular_ids {
        let contract = all_event_contracts()
            .iter()
            .find(|contract| contract.id == contract_id)
            .unwrap();
        assert_eq!(contract.kind, kind);
        assert_eq!(contract.class, RadrootsEventClass::Regular);
    }
}

#[test]
fn verified_decode_exposes_representative_downstream_compatibility_events() {
    let fixture_ids = [
        "wiki_article_valid",
        "knowledge_source_valid",
        "knowledge_claim_valid",
        "knowledge_review_valid",
        "knowledge_field_report_valid",
    ];
    for fixture in knowledge_valid_fixtures()
        .into_iter()
        .filter(|fixture| fixture_ids.contains(&fixture.id))
    {
        let signed = sign_parts(parts_for_fixture(&fixture.data));
        let decoded = verify_and_decode_radroots_event(signed).unwrap();
        match decoded {
            RadrootsDecodedEvent::WikiArticle(_)
            | RadrootsDecodedEvent::KnowledgeSource(_)
            | RadrootsDecodedEvent::KnowledgeClaim(_)
            | RadrootsDecodedEvent::KnowledgeReview(_)
            | RadrootsDecodedEvent::KnowledgeFieldReport(_) => {}
            decoded => panic!("{decoded:?}"),
        }
    }
}
