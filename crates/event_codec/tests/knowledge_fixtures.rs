#![cfg(all(feature = "knowledge", feature = "nostr"))]

use std::collections::BTreeSet;

use radroots_event::contract::{
    RadrootsContractValidationError, RadrootsEventClass, all_event_contracts,
    validate_event_contract_shape,
};
use radroots_event::kinds::{
    KIND_CONTRIBUTION_ATTESTATION, KIND_KNOWLEDGE_CHANGE_PROPOSAL, KIND_KNOWLEDGE_CLAIM,
    KIND_KNOWLEDGE_FIELD_REPORT, KIND_KNOWLEDGE_RELATION, KIND_KNOWLEDGE_REVIEW, KIND_WIKI_ARTICLE,
};
use radroots_event::knowledge::{
    RADROOTS_KNOWLEDGE_CLAIM_SCHEMA, RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA, RadrootsWikiArticle,
};
use radroots_event::wire::RadrootsNip01EventWireParts;
use radroots_event::{RadrootsEventEnvelope, RadrootsEventEnvelopeParts};
use radroots_event_codec::error::{EventEncodeError, EventParseError};
use radroots_event_codec::knowledge::{
    contribution_attestation_to_wire_parts, evidence_bounty_to_wire_parts,
    knowledge_change_proposal_to_wire_parts, knowledge_claim_to_wire_parts,
    knowledge_field_report_to_wire_parts, knowledge_relation_to_wire_parts,
    knowledge_review_to_wire_parts, knowledge_source_to_wire_parts, wiki_article_from_event,
    wiki_article_to_wire_parts, wiki_merge_request_from_event, wiki_merge_request_to_wire_parts,
    wiki_redirect_to_wire_parts,
};
use radroots_event_codec::verification::{
    RadrootsDecodeError, RadrootsDecodedEvent, RadrootsNip01VerificationError,
    verify_and_decode_radroots_event,
};
use radroots_test_fixtures::RELAY_PRIMARY_WSS;
use radroots_test_fixtures::knowledge::{
    RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES, RADROOTS_KNOWLEDGE_VALID_CONTRACT_IDS,
    RadrootsKnowledgeFixture, RadrootsKnowledgeFixtureCase, hex_64, knowledge_claim,
    knowledge_field_report, knowledge_valid_fixtures, wiki_article, wiki_merge_request,
    wiki_redirect,
};

fn event_from_parts(parts: RadrootsNip01EventWireParts) -> RadrootsEventEnvelope {
    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: hex_64('0'),
        author: hex_64('a'),
        created_at: 1_800_000_000,
        kind: parts.kind,
        tags: parts.tags,
        content: parts.content,
        sig: "1".repeat(128),
    })
    .unwrap()
}

fn sign_parts(parts: RadrootsNip01EventWireParts) -> RadrootsEventEnvelope {
    let tags = parts
        .tags
        .into_iter()
        .map(nostr::Tag::parse)
        .collect::<Result<Vec<_>, _>>()
        .expect("tags");
    let keys =
        nostr::Keys::parse("0101010101010101010101010101010101010101010101010101010101010101")
            .expect("keys");
    let kind = u16::try_from(parts.kind).expect("knowledge event kind must fit NIP-01");
    let event = nostr::EventBuilder::new(nostr::Kind::Custom(kind), parts.content)
        .tags(tags)
        .custom_created_at(nostr::Timestamp::from_secs(1_800_000_000))
        .sign_with_keys(&keys)
        .expect("signed event");
    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: event.id.to_hex(),
        author: event.pubkey.to_hex(),
        created_at: event.created_at.as_secs(),
        kind: u32::from(event.kind.as_u16()),
        tags: event
            .tags
            .as_slice()
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect(),
        content: event.content,
        sig: event.sig.to_string(),
    })
    .unwrap()
}

fn event_with_parts(
    event: &RadrootsEventEnvelope,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
) -> RadrootsEventEnvelope {
    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: event.id_str().to_string(),
        author: event.author().to_hex().to_string(),
        created_at: event.created_at_u64(),
        kind: event.kind_u32(),
        tags,
        content,
        sig,
    })
    .unwrap()
}

fn mutate_tags(event: &mut RadrootsEventEnvelope, update: impl FnOnce(&mut Vec<Vec<String>>)) {
    let mut tags = event.tags_as_vec();
    update(&mut tags);
    *event = event_with_parts(
        event,
        tags,
        event.content().to_string(),
        event.sig_str().to_string(),
    );
}

fn replace_content(event: &mut RadrootsEventEnvelope, content: String) {
    *event = event_with_parts(
        event,
        event.tags_as_vec(),
        content,
        event.sig_str().to_string(),
    );
}

fn replace_sig(event: &mut RadrootsEventEnvelope, sig: String) {
    *event = event_with_parts(event, event.tags_as_vec(), event.content().to_string(), sig);
}

fn parts_for_fixture(fixture: &RadrootsKnowledgeFixture) -> RadrootsNip01EventWireParts {
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

fn fixture_by_id<'a>(
    fixtures: &'a [RadrootsKnowledgeFixtureCase],
    id: &str,
) -> &'a RadrootsKnowledgeFixtureCase {
    fixtures
        .iter()
        .find(|fixture| fixture.id == id)
        .unwrap_or_else(|| panic!("missing fixture {id}"))
}

fn has_exact_tag(tags: &[Vec<String>], expected: &[&str]) -> bool {
    tags.iter().any(|tag| {
        tag.iter()
            .map(|entry| entry.as_str())
            .eq(expected.iter().copied())
    })
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
        .filter(|contract| RADROOTS_KNOWLEDGE_VALID_CONTRACT_IDS.contains(&contract.id))
        .map(|contract| contract.id)
        .collect::<BTreeSet<_>>();

    assert_eq!(fixture_contracts, registry_contracts);

    for fixture in &fixtures {
        let event = event_from_parts(parts_for_fixture(&fixture.data));
        validate_event_contract_shape(&event, fixture.contract_id).unwrap();
        assert_eq!(event.kind_u32(), fixture.kind, "{}", fixture.id);
    }

    let article_parts = parts_for_fixture(&fixture_by_id(&fixtures, "wiki_article_valid").data);
    let fork_address = format!("{}:{}:soil-health", KIND_WIKI_ARTICLE, hex_64('a'));
    let defer_address = format!("{}:{}:soil-health-v2", KIND_WIKI_ARTICLE, hex_64('a'));
    assert!(has_exact_tag(
        &article_parts.tags,
        &["a", fork_address.as_str(), RELAY_PRIMARY_WSS, "fork",]
    ));
    assert!(has_exact_tag(
        &article_parts.tags,
        &["e", hex_64('b').as_str(), RELAY_PRIMARY_WSS, "fork"]
    ));
    assert!(has_exact_tag(
        &article_parts.tags,
        &["a", defer_address.as_str(), RELAY_PRIMARY_WSS, "defer",]
    ));
    assert!(has_exact_tag(
        &article_parts.tags,
        &["e", hex_64('c').as_str(), RELAY_PRIMARY_WSS, "defer"]
    ));

    let merge_without_base =
        parts_for_fixture(&fixture_by_id(&fixtures, "wiki_merge_request_without_base_valid").data);
    assert!(has_exact_tag(
        &merge_without_base.tags,
        &["e", hex_64('f').as_str(), "", "source"]
    ));
    assert!(
        !merge_without_base
            .tags
            .iter()
            .any(|tag| tag == &vec!["e".to_string(), hex_64('e'), String::new()])
    );
}

#[test]
fn adversarial_knowledge_fixtures_reject_at_expected_stages() {
    let malformed = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "malformed_tags")
        .unwrap();
    let mut malformed_event =
        event_from_parts(knowledge_claim_to_wire_parts(&knowledge_claim()).unwrap());
    mutate_tags(&mut malformed_event, |tags| {
        tags.push(vec![
            "contract".to_string(),
            RADROOTS_KNOWLEDGE_CLAIM_SCHEMA.to_string(),
        ]);
    });
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
        serde_json::from_str(wrong_schema_event.content()).unwrap();
    wrong_schema_value["schema"] = serde_json::Value::from("radroots.knowledge.relation.v1");
    replace_content(
        &mut wrong_schema_event,
        serde_json::to_string(&wrong_schema_value).unwrap(),
    );
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
    mutate_tags(&mut missing_contract_event, |tags| {
        tags.retain(|tag| tag.first().map(|value| value.as_str()) != Some("contract"));
    });
    let signed = sign_parts(RadrootsNip01EventWireParts {
        kind: missing_contract_event.kind_u32(),
        content: missing_contract_event.content().to_string(),
        tags: missing_contract_event.tags_as_vec(),
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
        serde_json::from_str(private_event.content()).unwrap();
    private_value["context"]["latitude"] = serde_json::Value::from("45.0000");
    replace_content(
        &mut private_event,
        serde_json::to_string(&private_value).unwrap(),
    );
    let signed = sign_parts(RadrootsNip01EventWireParts {
        kind: private_event.kind_u32(),
        content: private_event.content().to_string(),
        tags: private_event.tags_as_vec(),
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
    mutate_tags(&mut unsupported_event, |tags| {
        for tag in tags {
            if tag.first().map(|value| value.as_str()) == Some("contract") {
                tag[1] = "radroots.knowledge.unsupported.v1".to_string();
            }
        }
    });
    let signed = sign_parts(RadrootsNip01EventWireParts {
        kind: unsupported_event.kind_u32(),
        content: unsupported_event.content().to_string(),
        tags: unsupported_event.tags_as_vec(),
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

    let invalid_redirect = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "invalid_redirect_target_kind")
        .unwrap();
    let mut redirect = wiki_redirect();
    redirect.target.kind = 30023;
    let error = wiki_redirect_to_wire_parts(&redirect).unwrap_err();
    assert_eq!(invalid_redirect.pipeline_stage, "encode");
    assert_eq!(invalid_redirect.expected_error_code, error.code());
    assert!(matches!(
        error,
        EventEncodeError::InvalidField("wiki_redirect.target")
    ));

    let missing_source = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "merge_request_missing_source_marker")
        .unwrap();
    let mut missing_source_event =
        event_from_parts(wiki_merge_request_to_wire_parts(&wiki_merge_request()).unwrap());
    mutate_tags(&mut missing_source_event, |tags| {
        tags.retain(|tag| {
            !(tag.first().map(|value| value.as_str()) == Some("e")
                && tag.last().map(|value| value.as_str()) == Some("source"))
        });
    });
    let error = wiki_merge_request_from_event(missing_source_event).unwrap_err();
    assert_eq!(missing_source.pipeline_stage, "event_parse");
    assert_eq!(missing_source.expected_error_code, error.code());
    assert!(matches!(error, EventParseError::InvalidTag("e")));

    let json_guard = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "merge_request_json_content_guard")
        .unwrap();
    let merge_parts = wiki_merge_request_to_wire_parts(&wiki_merge_request()).unwrap();
    assert_eq!(json_guard.pipeline_stage, "wire_shape");
    assert_eq!(json_guard.expected_error_code, "plain_text_content");
    assert_eq!(merge_parts.content, "Merge synthetic soil article updates");
    assert!(serde_json::from_str::<serde_json::Value>(&merge_parts.content).is_err());
    assert!(!merge_parts.content.contains("target_article"));

    let orphan_fork = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "orphan_fork_marker")
        .unwrap();
    let mut orphan_fork_event =
        event_from_parts(wiki_article_to_wire_parts(&wiki_article()).unwrap());
    let mut removed_fork_event = false;
    mutate_tags(&mut orphan_fork_event, |tags| {
        tags.retain(|tag| {
            if !removed_fork_event
                && tag.first().map(|value| value.as_str()) == Some("e")
                && tag.last().map(|value| value.as_str()) == Some("fork")
            {
                removed_fork_event = true;
                false
            } else {
                true
            }
        });
    });
    let error = wiki_article_from_event(orphan_fork_event).unwrap_err();
    assert_eq!(orphan_fork.pipeline_stage, "event_parse");
    assert_eq!(orphan_fork.expected_error_code, error.code());
    assert!(matches!(error, EventParseError::InvalidTag("a")));

    let orphan_defer = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "orphan_defer_marker")
        .unwrap();
    let mut orphan_defer_event =
        event_from_parts(wiki_article_to_wire_parts(&wiki_article()).unwrap());
    let mut removed_defer_address = false;
    mutate_tags(&mut orphan_defer_event, |tags| {
        tags.retain(|tag| {
            if !removed_defer_address
                && tag.first().map(|value| value.as_str()) == Some("a")
                && tag.last().map(|value| value.as_str()) == Some("defer")
            {
                removed_defer_address = true;
                false
            } else {
                true
            }
        });
    });
    let error = wiki_article_from_event(orphan_defer_event).unwrap_err();
    assert_eq!(orphan_defer.pipeline_stage, "event_parse");
    assert_eq!(orphan_defer.expected_error_code, error.code());
    assert!(matches!(error, EventParseError::InvalidTag("e")));

    let id_mismatch = RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "id_mismatch")
        .unwrap();
    let signed = sign_parts(knowledge_claim_to_wire_parts(&knowledge_claim()).unwrap());
    let mut mutated = signed.clone();
    let mutated_content = mutated.content().replace("Cover crops", "Compost");
    replace_content(&mut mutated, mutated_content);
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
    replace_sig(&mut bad_signature, "0".repeat(128));
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
        let mut value: serde_json::Value = serde_json::from_str(event.content()).unwrap();
        value[field] = serde_json::Value::from("approved");
        replace_content(&mut event, serde_json::to_string(&value).unwrap());
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
fn verified_decode_exposes_representative_public_surface_events() {
    let fixture_ids = [
        "wiki_article_valid",
        "wiki_redirect_valid",
        "wiki_merge_request_without_base_valid",
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
            | RadrootsDecodedEvent::WikiRedirect(_)
            | RadrootsDecodedEvent::WikiMergeRequest(_)
            | RadrootsDecodedEvent::KnowledgeSource(_)
            | RadrootsDecodedEvent::KnowledgeClaim(_)
            | RadrootsDecodedEvent::KnowledgeReview(_)
            | RadrootsDecodedEvent::KnowledgeFieldReport(_) => {}
            decoded => panic!("{decoded:?}"),
        }
    }
}
