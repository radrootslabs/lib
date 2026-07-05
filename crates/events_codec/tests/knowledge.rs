#![cfg(all(feature = "knowledge", feature = "nostr"))]

use radroots_events::RadrootsNostrEvent;
use radroots_events::contract::validate_event_contract_shape;
use radroots_events::kinds::{
    KIND_KNOWLEDGE_CLAIM, KIND_KNOWLEDGE_REVIEW, KIND_KNOWLEDGE_SOURCE, KIND_WIKI_ARTICLE,
};
use radroots_events::knowledge::{
    RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA, RADROOTS_EVIDENCE_BOUNTY_SCHEMA,
    RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA, RADROOTS_KNOWLEDGE_CLAIM_SCHEMA,
    RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA, RADROOTS_KNOWLEDGE_RELATION_SCHEMA,
    RADROOTS_KNOWLEDGE_REVIEW_SCHEMA, RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
    RADROOTS_KNOWLEDGE_SOURCE_SCHEMA, RadrootsAddressableRef, RadrootsContributionAttestation,
    RadrootsEvidenceBounty, RadrootsKnowledgeChangeProposal, RadrootsKnowledgeCitationSpan,
    RadrootsKnowledgeClaim, RadrootsKnowledgeFieldContext, RadrootsKnowledgeFieldReport,
    RadrootsKnowledgeLocation, RadrootsKnowledgeLocationPrecision, RadrootsKnowledgeNodeRef,
    RadrootsKnowledgeObservation, RadrootsKnowledgeObservationValue, RadrootsKnowledgeRelation,
    RadrootsKnowledgeReview, RadrootsKnowledgeReviewScope, RadrootsKnowledgeReviewScore,
    RadrootsKnowledgeReviewTarget, RadrootsKnowledgeSource, RadrootsWikiArticle,
    RadrootsWikiMergeRequest, RadrootsWikiRedirect,
};
use radroots_events_codec::knowledge::{
    contribution_attestation_from_event, contribution_attestation_to_wire_parts,
    evidence_bounty_from_event, evidence_bounty_to_wire_parts,
    knowledge_change_proposal_from_event, knowledge_change_proposal_to_wire_parts,
    knowledge_claim_from_event, knowledge_claim_to_wire_parts, knowledge_field_report_from_event,
    knowledge_field_report_to_wire_parts, knowledge_relation_from_event,
    knowledge_relation_to_wire_parts, knowledge_review_from_event, knowledge_review_to_wire_parts,
    knowledge_source_from_event, knowledge_source_to_wire_parts, wiki_article_from_event,
    wiki_article_to_wire_parts, wiki_merge_request_from_event, wiki_merge_request_to_wire_parts,
    wiki_redirect_from_event, wiki_redirect_to_wire_parts,
};
use radroots_events_codec::verification::{
    RadrootsDecodeError, RadrootsDecodedEvent, RadrootsNip01VerificationError,
    verify_and_decode_radroots_event,
};
use radroots_events_codec::wire::WireEventParts;

fn hex_64(character: char) -> String {
    core::iter::repeat_n(character, 64).collect()
}

fn event_ref(character: char, kind: u32) -> radroots_events::RadrootsNostrEventRef {
    radroots_events::RadrootsNostrEventRef {
        id: hex_64(character),
        author: hex_64('a'),
        kind,
        d_tag: None,
        relays: Some(vec!["wss://relay.radroots.example".to_string()]),
    }
}

fn article_ref() -> radroots_events::RadrootsNostrEventRef {
    radroots_events::RadrootsNostrEventRef {
        id: hex_64('b'),
        author: hex_64('a'),
        kind: KIND_WIKI_ARTICLE,
        d_tag: Some("soil-health".to_string()),
        relays: Some(vec!["wss://relay.radroots.example".to_string()]),
    }
}

fn address_ref() -> RadrootsAddressableRef {
    RadrootsAddressableRef {
        kind: KIND_WIKI_ARTICLE,
        pubkey: hex_64('a'),
        d_tag: "soil-health".to_string(),
        relays: vec!["wss://relay.radroots.example".to_string()],
    }
}

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

fn wiki_article() -> RadrootsWikiArticle {
    RadrootsWikiArticle {
        d_tag: "soil-health".to_string(),
        title: "Soil health".to_string(),
        content_djot: "# Soil health".to_string(),
        summary: Some("Living soil basics".to_string()),
        topics: vec!["soil".to_string(), "health".to_string()],
        references: vec![event_ref('2', KIND_KNOWLEDGE_SOURCE)],
        forked_from: Vec::new(),
        deferred_to: None,
    }
}

fn source() -> RadrootsKnowledgeSource {
    RadrootsKnowledgeSource {
        schema: RADROOTS_KNOWLEDGE_SOURCE_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        d_tag: "soil-source".to_string(),
        title: "Soil Source".to_string(),
        source_type: "book".to_string(),
        authors: vec!["A. Example".to_string()],
        publisher: Some("Radroots Synthetic Press".to_string()),
        publication_year: Some(2026),
        edition: None,
        canonical_url: None,
        artifact_refs: vec![event_ref('3', 1063)],
        author_asserted_rights: None,
        topics: vec!["soil".to_string()],
        summary: Some("Synthetic source".to_string()),
    }
}

fn claim() -> RadrootsKnowledgeClaim {
    RadrootsKnowledgeClaim {
        schema: RADROOTS_KNOWLEDGE_CLAIM_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        claim_type: "practice_effect".to_string(),
        text: "Cover crops improve soil structure.".to_string(),
        citation_spans: vec![RadrootsKnowledgeCitationSpan {
            source_ref: event_ref('4', KIND_KNOWLEDGE_SOURCE),
            artifact_ref: None,
            page_start: Some(12),
            page_end: Some(13),
            section_path: vec!["chapter-1".to_string()],
            quote_hash: Some(hex_64('5')),
            chunk_id: Some("chunk-1".to_string()),
        }],
        topics: vec!["cover-crops".to_string()],
        applies_to: vec!["local-food".to_string()],
        author_asserted_confidence: Some("medium".to_string()),
        supersedes: Vec::new(),
    }
}

fn node_ref(label: &str) -> RadrootsKnowledgeNodeRef {
    RadrootsKnowledgeNodeRef {
        node_type: "event".to_string(),
        event_ref: Some(event_ref('6', KIND_KNOWLEDGE_CLAIM)),
        address_ref: None,
        external_id: None,
        label: Some(label.to_string()),
    }
}

fn relation() -> RadrootsKnowledgeRelation {
    RadrootsKnowledgeRelation {
        schema: RADROOTS_KNOWLEDGE_RELATION_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        subject: node_ref("cover crops"),
        predicate: "supports".to_string(),
        object: node_ref("soil structure"),
        support_refs: vec![event_ref('7', KIND_KNOWLEDGE_CLAIM)],
        author_asserted_confidence: Some("medium".to_string()),
        supersedes: Vec::new(),
    }
}

fn review() -> RadrootsKnowledgeReview {
    RadrootsKnowledgeReview {
        schema: RADROOTS_KNOWLEDGE_REVIEW_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        target: RadrootsKnowledgeReviewTarget {
            event_id: hex_64('8'),
            author_pubkey: hex_64('a'),
            kind: KIND_KNOWLEDGE_CLAIM,
            address: None,
            relays: vec!["wss://relay.radroots.example".to_string()],
            review_scope: RadrootsKnowledgeReviewScope::SpecificVersion,
        },
        reviewer_role: "peer".to_string(),
        verdict: "needs_more_evidence".to_string(),
        scores: vec![RadrootsKnowledgeReviewScore {
            dimension: "evidence".to_string(),
            value: "partial".to_string(),
            note: None,
        }],
        notes: Some("Synthetic review".to_string()),
        evidence_refs: vec![event_ref('9', KIND_KNOWLEDGE_SOURCE)],
    }
}

fn field_report() -> RadrootsKnowledgeFieldReport {
    RadrootsKnowledgeFieldReport {
        schema: RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        report_type: "observation".to_string(),
        title: "Field observation".to_string(),
        summary: Some("Observed cover crop residue.".to_string()),
        context: RadrootsKnowledgeFieldContext {
            location_precision: RadrootsKnowledgeLocationPrecision::CoarseGeohash,
            public_location: Some(RadrootsKnowledgeLocation {
                label: Some("watershed".to_string()),
                region: Some("synthetic-region".to_string()),
                locality: None,
                geohash: Some("c23".to_string()),
            }),
            private_location_ref: None,
            topics: vec!["field".to_string()],
            context_tags: vec!["observation".to_string()],
        },
        observations: vec![RadrootsKnowledgeObservation {
            observation_type: "residue".to_string(),
            text: "Residue was visible across beds.".to_string(),
            observed_at: Some("2026-07-05".to_string()),
            values: vec![RadrootsKnowledgeObservationValue {
                key: "coverage".to_string(),
                value: "medium".to_string(),
                unit: None,
            }],
        }],
        artifact_refs: vec![event_ref('c', 1063)],
        related_refs: vec![event_ref('d', KIND_KNOWLEDGE_CLAIM)],
        limitations: vec!["single observer".to_string()],
    }
}

#[test]
fn knowledge_codecs_roundtrip_all_contracts() {
    let article_event = event_from_parts(wiki_article_to_wire_parts(&wiki_article()).unwrap());
    validate_event_contract_shape(&article_event, "radroots.wiki.article.v1").unwrap();
    assert_eq!(
        wiki_article_from_event(article_event)
            .unwrap()
            .data
            .data
            .title,
        "Soil health"
    );

    let redirect = RadrootsWikiRedirect {
        d_tag: "soil".to_string(),
        target: article_ref(),
    };
    let redirect_event = event_from_parts(wiki_redirect_to_wire_parts(&redirect).unwrap());
    validate_event_contract_shape(&redirect_event, "radroots.wiki.redirect.v1").unwrap();
    assert_eq!(
        wiki_redirect_from_event(redirect_event)
            .unwrap()
            .data
            .data
            .target
            .d_tag,
        Some("soil-health".to_string())
    );

    let merge = RadrootsWikiMergeRequest {
        target_article: address_ref(),
        destination_pubkey: hex_64('a'),
        base_version_event_id: Some(hex_64('e')),
        source_version_event_id: hex_64('f'),
        explanation: Some("Merge synthetic source".to_string()),
    };
    let merge_event = event_from_parts(wiki_merge_request_to_wire_parts(&merge).unwrap());
    validate_event_contract_shape(&merge_event, "radroots.wiki.merge_request.v1").unwrap();
    assert_eq!(
        wiki_merge_request_from_event(merge_event)
            .unwrap()
            .data
            .data
            .source_version_event_id,
        hex_64('f')
    );

    let source_event = event_from_parts(knowledge_source_to_wire_parts(&source()).unwrap());
    validate_event_contract_shape(&source_event, RADROOTS_KNOWLEDGE_SOURCE_SCHEMA).unwrap();
    assert_eq!(
        knowledge_source_from_event(source_event)
            .unwrap()
            .data
            .data
            .source_type,
        "book"
    );

    let claim_event = event_from_parts(knowledge_claim_to_wire_parts(&claim()).unwrap());
    validate_event_contract_shape(&claim_event, RADROOTS_KNOWLEDGE_CLAIM_SCHEMA).unwrap();
    assert_eq!(
        knowledge_claim_from_event(claim_event)
            .unwrap()
            .data
            .data
            .claim_type,
        "practice_effect"
    );

    let relation_event = event_from_parts(knowledge_relation_to_wire_parts(&relation()).unwrap());
    validate_event_contract_shape(&relation_event, RADROOTS_KNOWLEDGE_RELATION_SCHEMA).unwrap();
    assert_eq!(
        knowledge_relation_from_event(relation_event)
            .unwrap()
            .data
            .data
            .predicate,
        "supports"
    );

    let review_event = event_from_parts(knowledge_review_to_wire_parts(&review()).unwrap());
    validate_event_contract_shape(&review_event, RADROOTS_KNOWLEDGE_REVIEW_SCHEMA).unwrap();
    assert_eq!(
        knowledge_review_from_event(review_event)
            .unwrap()
            .data
            .data
            .verdict,
        "needs_more_evidence"
    );

    let report_event =
        event_from_parts(knowledge_field_report_to_wire_parts(&field_report()).unwrap());
    validate_event_contract_shape(&report_event, RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA).unwrap();
    assert_eq!(
        knowledge_field_report_from_event(report_event)
            .unwrap()
            .data
            .data
            .report_type,
        "observation"
    );

    let bounty = RadrootsEvidenceBounty {
        schema: RADROOTS_EVIDENCE_BOUNTY_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        d_tag: "soil-bounty".to_string(),
        title: "Soil bounty".to_string(),
        summary: None,
        topics: vec!["soil".to_string()],
        target_refs: vec![event_ref('a', KIND_KNOWLEDGE_CLAIM)],
        reward_note: None,
        closes_at: None,
    };
    let bounty_event = event_from_parts(evidence_bounty_to_wire_parts(&bounty).unwrap());
    validate_event_contract_shape(&bounty_event, RADROOTS_EVIDENCE_BOUNTY_SCHEMA).unwrap();
    assert_eq!(
        evidence_bounty_from_event(bounty_event)
            .unwrap()
            .data
            .data
            .title,
        "Soil bounty"
    );

    let proposal = RadrootsKnowledgeChangeProposal {
        schema: RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        target: event_ref('b', KIND_KNOWLEDGE_CLAIM),
        proposal_type: "amend".to_string(),
        summary: "Clarify scope".to_string(),
        rationale: None,
        evidence_refs: vec![event_ref('c', KIND_KNOWLEDGE_SOURCE)],
        supersedes: Vec::new(),
    };
    let proposal_event =
        event_from_parts(knowledge_change_proposal_to_wire_parts(&proposal).unwrap());
    validate_event_contract_shape(&proposal_event, RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA)
        .unwrap();
    assert_eq!(
        knowledge_change_proposal_from_event(proposal_event)
            .unwrap()
            .data
            .data
            .proposal_type,
        "amend"
    );

    let attestation = RadrootsContributionAttestation {
        schema: RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        contributor_pubkey: hex_64('a'),
        contribution_type: "review".to_string(),
        subject_refs: vec![event_ref('d', KIND_KNOWLEDGE_REVIEW)],
        summary: "Reviewed synthetic claim".to_string(),
        evidence_refs: vec![event_ref('e', KIND_KNOWLEDGE_REVIEW)],
    };
    let attestation_event =
        event_from_parts(contribution_attestation_to_wire_parts(&attestation).unwrap());
    validate_event_contract_shape(&attestation_event, RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA)
        .unwrap();
    assert_eq!(
        contribution_attestation_from_event(attestation_event)
            .unwrap()
            .data
            .data
            .contribution_type,
        "review"
    );
}

#[test]
fn verified_decode_accepts_signed_claim_and_rejects_mutation() {
    let signed = sign_parts(knowledge_claim_to_wire_parts(&claim()).unwrap());
    let decoded = verify_and_decode_radroots_event(signed.clone()).unwrap();
    assert!(matches!(decoded, RadrootsDecodedEvent::KnowledgeClaim(_)));

    let mut mutated = signed;
    mutated.content = mutated.content.replace("Cover crops", "Compost");
    let err = verify_and_decode_radroots_event(mutated).unwrap_err();
    assert_eq!(err.code(), "nip01_verification");
    assert!(matches!(
        err,
        RadrootsDecodeError::Nip01Verification(RadrootsNip01VerificationError::IdMismatch { .. })
    ));
}

#[test]
fn malformed_knowledge_events_return_stable_decode_codes() {
    let mut missing_contract = event_from_parts(knowledge_claim_to_wire_parts(&claim()).unwrap());
    missing_contract
        .tags
        .retain(|tag| tag.first().map(|value| value.as_str()) != Some("contract"));
    let signed = sign_parts(WireEventParts {
        kind: missing_contract.kind,
        content: missing_contract.content,
        tags: missing_contract.tags,
    });
    let error = verify_and_decode_radroots_event(signed).unwrap_err();
    assert_eq!(error.code(), "contract_validation");

    let mut report =
        event_from_parts(knowledge_field_report_to_wire_parts(&field_report()).unwrap());
    let mut value: serde_json::Value = serde_json::from_str(&report.content).unwrap();
    value["context"]["latitude"] = serde_json::Value::from("45.0000");
    report.content = serde_json::to_string(&value).unwrap();
    let parsed_error = knowledge_field_report_from_event(report).unwrap_err();
    assert_eq!(parsed_error.code(), "invalid_json");
}
