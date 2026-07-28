#![cfg(all(feature = "knowledge", feature = "nostr"))]

use radroots_event::contract::validate_event_contract_shape;
use radroots_event::kinds::{
    KIND_KNOWLEDGE_CLAIM, KIND_KNOWLEDGE_REVIEW, KIND_KNOWLEDGE_SOURCE, KIND_WIKI_ARTICLE,
};
use radroots_event::knowledge::{
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
    RadrootsWikiArticleVersionRef, RadrootsWikiMergeRequest, RadrootsWikiRedirect,
};
use radroots_event::wire::RadrootsNip01EventWireParts;
use radroots_event::{RadrootsEventEnvelope, RadrootsEventEnvelopeParts};
use radroots_event_codec::error::{EventEncodeError, EventParseError};
use radroots_event_codec::knowledge::{
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
use radroots_event_codec::verification::{
    RadrootsDecodeError, RadrootsDecodedEvent, RadrootsNip01VerificationError,
    verify_and_decode_radroots_event,
};

fn hex_64(character: char) -> String {
    core::iter::repeat_n(character, 64).collect()
}

fn event_ref(character: char, kind: u32) -> radroots_event::RadrootsEventRef {
    radroots_event::RadrootsEventRef {
        id: hex_64(character),
        author: hex_64('a'),
        kind,
        d_tag: None,
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

fn article_version_ref() -> RadrootsWikiArticleVersionRef {
    article_version_ref_for('b', "soil-health")
}

fn article_version_ref_for(event_id_character: char, d_tag: &str) -> RadrootsWikiArticleVersionRef {
    let mut address_ref = address_ref();
    address_ref.d_tag = d_tag.to_string();
    RadrootsWikiArticleVersionRef {
        event_id: hex_64(event_id_character),
        address_ref,
    }
}

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

fn event_with_parts(
    event: &RadrootsEventEnvelope,
    tags: Vec<Vec<String>>,
    content: String,
) -> RadrootsEventEnvelope {
    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: event.id_str().to_string(),
        author: event.author().to_hex().to_string(),
        created_at: event.created_at_u64(),
        kind: event.kind_u32(),
        tags,
        content,
        sig: event.sig_str().to_string(),
    })
    .unwrap()
}

fn mutate_tags(event: &mut RadrootsEventEnvelope, update: impl FnOnce(&mut Vec<Vec<String>>)) {
    let mut tags = event.tags_as_vec();
    update(&mut tags);
    *event = event_with_parts(event, tags, event.content().to_string());
}

fn replace_content(event: &mut RadrootsEventEnvelope, content: String) {
    *event = event_with_parts(event, event.tags_as_vec(), content);
}

fn replace_first_tag_value(event: &mut RadrootsEventEnvelope, name: &str, value: String) {
    mutate_tags(event, |tags| {
        let tag = tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some(name))
            .expect("tag");
        let tag_value = tag.get_mut(1).expect("tag value");
        *tag_value = value;
    });
}

fn marked_tag(tag: &[String], name: &str, marker: &str) -> bool {
    tag.first().map(String::as_str) == Some(name) && tag.last().map(String::as_str) == Some(marker)
}

fn marked_tag_index(event: &RadrootsEventEnvelope, name: &str, marker: &str) -> usize {
    event
        .tags_as_vec()
        .iter()
        .position(|tag| marked_tag(tag, name, marker))
        .expect("marked tag")
}

fn assert_parse_error(actual: EventParseError, expected: EventParseError) {
    match (actual, expected) {
        (EventParseError::MissingTag(actual), EventParseError::MissingTag(expected))
        | (EventParseError::InvalidTag(actual), EventParseError::InvalidTag(expected))
        | (EventParseError::InvalidJson(actual), EventParseError::InvalidJson(expected)) => {
            assert_eq!(actual, expected);
        }
        (
            EventParseError::InvalidKind {
                expected: actual_expected,
                got: actual_got,
            },
            EventParseError::InvalidKind { expected, got },
        ) => {
            assert_eq!(actual_expected, expected);
            assert_eq!(actual_got, got);
        }
        (actual, expected) => panic!("expected {expected:?}, got {actual:?}"),
    }
}

fn assert_encode_error(actual: EventEncodeError, expected: EventEncodeError) {
    match (actual, expected) {
        (
            EventEncodeError::EmptyRequiredField(actual),
            EventEncodeError::EmptyRequiredField(expected),
        )
        | (EventEncodeError::InvalidField(actual), EventEncodeError::InvalidField(expected)) => {
            assert_eq!(actual, expected);
        }
        (actual, expected) => panic!("expected {expected:?}, got {actual:?}"),
    }
}

fn invalid_relay() -> String {
    "http://relay.radroots.example".to_string()
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

fn wiki_article() -> RadrootsWikiArticle {
    RadrootsWikiArticle {
        d_tag: "soil-health".to_string(),
        title: Some("Soil health".to_string()),
        content_djot: "# Soil health".to_string(),
        summary: Some("Living soil basics".to_string()),
        topics: vec!["soil".to_string(), "health".to_string()],
        references: vec![event_ref('2', KIND_KNOWLEDGE_SOURCE)],
        forked_from: vec![article_version_ref()],
        deferred_to: Some(article_version_ref()),
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

fn bounty() -> RadrootsEvidenceBounty {
    RadrootsEvidenceBounty {
        schema: RADROOTS_EVIDENCE_BOUNTY_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        d_tag: "soil-bounty".to_string(),
        title: "Soil bounty".to_string(),
        summary: None,
        topics: vec!["soil".to_string()],
        target_refs: vec![event_ref('a', KIND_KNOWLEDGE_CLAIM)],
        reward_note: None,
        closes_at: None,
    }
}

fn proposal() -> RadrootsKnowledgeChangeProposal {
    RadrootsKnowledgeChangeProposal {
        schema: RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        target: event_ref('b', KIND_KNOWLEDGE_CLAIM),
        proposal_type: "amend".to_string(),
        summary: "Clarify scope".to_string(),
        rationale: None,
        evidence_refs: vec![event_ref('c', KIND_KNOWLEDGE_SOURCE)],
        supersedes: Vec::new(),
    }
}

fn attestation() -> RadrootsContributionAttestation {
    RadrootsContributionAttestation {
        schema: RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        contributor_pubkey: hex_64('a'),
        contribution_type: "review".to_string(),
        subject_refs: vec![event_ref('d', KIND_KNOWLEDGE_REVIEW)],
        summary: "Reviewed synthetic claim".to_string(),
        evidence_refs: vec![event_ref('e', KIND_KNOWLEDGE_REVIEW)],
    }
}

#[test]
fn knowledge_codecs_roundtrip_all_contracts() {
    let article_event = event_from_parts(wiki_article_to_wire_parts(&wiki_article()).unwrap());
    validate_event_contract_shape(&article_event, "radroots.wiki.article.v1").unwrap();
    assert!(article_event.tags_as_vec().iter().any(|tag| tag
        == &vec![
            "a".to_string(),
            format!("30818:{}:soil-health", hex_64('a')),
            "wss://relay.radroots.example".to_string(),
            "fork".to_string()
        ]));
    assert!(article_event.tags_as_vec().iter().any(|tag| tag
        == &vec![
            "e".to_string(),
            hex_64('b'),
            "wss://relay.radroots.example".to_string(),
            "fork".to_string()
        ]));
    assert!(article_event.tags_as_vec().iter().any(|tag| tag
        == &vec![
            "a".to_string(),
            format!("30818:{}:soil-health", hex_64('a')),
            "wss://relay.radroots.example".to_string(),
            "defer".to_string()
        ]));
    assert!(article_event.tags_as_vec().iter().any(|tag| tag
        == &vec![
            "e".to_string(),
            hex_64('b'),
            "wss://relay.radroots.example".to_string(),
            "defer".to_string()
        ]));
    assert_eq!(
        wiki_article_from_event(article_event)
            .unwrap()
            .data
            .data
            .title,
        Some("Soil health".to_string())
    );

    let redirect = RadrootsWikiRedirect {
        d_tag: "soil".to_string(),
        target: address_ref(),
    };
    let redirect_event = event_from_parts(wiki_redirect_to_wire_parts(&redirect).unwrap());
    validate_event_contract_shape(&redirect_event, "radroots.wiki.redirect.v1").unwrap();
    assert!(redirect_event.tags_as_vec().iter().any(|tag| tag
        == &vec![
            "a".to_string(),
            format!("30818:{}:soil-health", hex_64('a')),
            "wss://relay.radroots.example".to_string()
        ]));
    assert_eq!(
        wiki_redirect_from_event(redirect_event)
            .unwrap()
            .data
            .data
            .target
            .d_tag,
        "soil-health"
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
    assert_eq!(merge_event.content(), "Merge synthetic source");
    assert!(
        merge_event
            .tags_as_vec()
            .iter()
            .any(|tag| tag == &vec!["e".to_string(), hex_64('e'), String::new()])
    );
    assert!(merge_event.tags_as_vec().iter().any(|tag| tag
        == &vec![
            "e".to_string(),
            hex_64('f'),
            String::new(),
            "source".to_string()
        ]));
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
fn knowledge_decode_rejects_mirrored_tag_content_drift() {
    let mut source_event = event_from_parts(knowledge_source_to_wire_parts(&source()).unwrap());
    replace_first_tag_value(&mut source_event, "source", hex_64('4'));
    assert_parse_error(
        knowledge_source_from_event(source_event).unwrap_err(),
        EventParseError::InvalidTag("source"),
    );

    let mut bounty_event = event_from_parts(evidence_bounty_to_wire_parts(&bounty()).unwrap());
    replace_first_tag_value(&mut bounty_event, "evidence", hex_64('b'));
    assert_parse_error(
        evidence_bounty_from_event(bounty_event).unwrap_err(),
        EventParseError::InvalidTag("evidence"),
    );

    let mut claim_source_event = event_from_parts(knowledge_claim_to_wire_parts(&claim()).unwrap());
    replace_first_tag_value(&mut claim_source_event, "source", hex_64('6'));
    assert_parse_error(
        knowledge_claim_from_event(claim_source_event).unwrap_err(),
        EventParseError::InvalidTag("source"),
    );

    let mut claim_citation_event =
        event_from_parts(knowledge_claim_to_wire_parts(&claim()).unwrap());
    replace_first_tag_value(&mut claim_citation_event, "citation", hex_64('6'));
    assert_parse_error(
        knowledge_claim_from_event(claim_citation_event).unwrap_err(),
        EventParseError::InvalidTag("citation"),
    );

    let mut relation_event =
        event_from_parts(knowledge_relation_to_wire_parts(&relation()).unwrap());
    replace_first_tag_value(&mut relation_event, "source", hex_64('8'));
    assert_parse_error(
        knowledge_relation_from_event(relation_event).unwrap_err(),
        EventParseError::InvalidTag("source"),
    );

    let mut review_target_event =
        event_from_parts(knowledge_review_to_wire_parts(&review()).unwrap());
    replace_first_tag_value(&mut review_target_event, "review_target", hex_64('9'));
    assert_parse_error(
        knowledge_review_from_event(review_target_event).unwrap_err(),
        EventParseError::InvalidTag("review_target"),
    );

    let mut report_geohash_event =
        event_from_parts(knowledge_field_report_to_wire_parts(&field_report()).unwrap());
    replace_first_tag_value(&mut report_geohash_event, "g", "c24".to_string());
    assert_parse_error(
        knowledge_field_report_from_event(report_geohash_event).unwrap_err(),
        EventParseError::InvalidTag("g"),
    );

    let mut report_topic_event =
        event_from_parts(knowledge_field_report_to_wire_parts(&field_report()).unwrap());
    replace_first_tag_value(&mut report_topic_event, "t", "soil".to_string());
    assert_parse_error(
        knowledge_field_report_from_event(report_topic_event).unwrap_err(),
        EventParseError::InvalidTag("t"),
    );

    let mut report_evidence_event =
        event_from_parts(knowledge_field_report_to_wire_parts(&field_report()).unwrap());
    replace_first_tag_value(&mut report_evidence_event, "evidence", hex_64('e'));
    assert_parse_error(
        knowledge_field_report_from_event(report_evidence_event).unwrap_err(),
        EventParseError::InvalidTag("evidence"),
    );

    let mut proposal_event =
        event_from_parts(knowledge_change_proposal_to_wire_parts(&proposal()).unwrap());
    replace_first_tag_value(&mut proposal_event, "evidence", hex_64('d'));
    assert_parse_error(
        knowledge_change_proposal_from_event(proposal_event).unwrap_err(),
        EventParseError::InvalidTag("evidence"),
    );

    let mut attestation_event =
        event_from_parts(contribution_attestation_to_wire_parts(&attestation()).unwrap());
    replace_first_tag_value(&mut attestation_event, "evidence", hex_64('f'));
    assert_parse_error(
        contribution_attestation_from_event(attestation_event).unwrap_err(),
        EventParseError::InvalidTag("evidence"),
    );
}

#[test]
fn verified_decode_accepts_signed_claim_and_rejects_mutation() {
    let signed = sign_parts(knowledge_claim_to_wire_parts(&claim()).unwrap());
    let decoded = verify_and_decode_radroots_event(signed.clone()).unwrap();
    assert!(matches!(decoded, RadrootsDecodedEvent::KnowledgeClaim(_)));

    let mut mutated = signed;
    let mutated_content = mutated.content().replace("Cover crops", "Compost");
    replace_content(&mut mutated, mutated_content);
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
    mutate_tags(&mut missing_contract, |tags| {
        tags.retain(|tag| tag.first().map(|value| value.as_str()) != Some("contract"));
    });
    let signed = sign_parts(RadrootsNip01EventWireParts {
        kind: missing_contract.kind_u32(),
        content: missing_contract.content().to_string(),
        tags: missing_contract.tags_as_vec(),
    });
    let error = verify_and_decode_radroots_event(signed).unwrap_err();
    assert_eq!(error.code(), "contract_validation");

    let mut report =
        event_from_parts(knowledge_field_report_to_wire_parts(&field_report()).unwrap());
    let mut value: serde_json::Value = serde_json::from_str(report.content()).unwrap();
    value["context"]["latitude"] = serde_json::Value::from("45.0000");
    replace_content(&mut report, serde_json::to_string(&value).unwrap());
    let parsed_error = knowledge_field_report_from_event(report).unwrap_err();
    assert_eq!(parsed_error.code(), "invalid_json");
}

#[test]
fn malformed_nip54_wiki_shapes_are_rejected() {
    let mut redirect = event_from_parts(
        wiki_redirect_to_wire_parts(&RadrootsWikiRedirect {
            d_tag: "soil".to_string(),
            target: address_ref(),
        })
        .unwrap(),
    );
    mutate_tags(&mut redirect, |tags| {
        for tag in tags {
            if tag.first().map(|value| value.as_str()) == Some("a") {
                tag[1] = format!("30023:{}:soil-health", hex_64('a'));
            }
        }
    });
    assert_parse_error(
        wiki_redirect_from_event(redirect).unwrap_err(),
        EventParseError::InvalidTag("a"),
    );

    let merge = RadrootsWikiMergeRequest {
        target_article: address_ref(),
        destination_pubkey: hex_64('a'),
        base_version_event_id: Some(hex_64('e')),
        source_version_event_id: hex_64('f'),
        explanation: Some("Merge synthetic source".to_string()),
    };
    let mut missing_target = event_from_parts(wiki_merge_request_to_wire_parts(&merge).unwrap());
    mutate_tags(&mut missing_target, |tags| {
        tags.retain(|tag| tag.first().map(|value| value.as_str()) != Some("a"));
    });
    assert_parse_error(
        wiki_merge_request_from_event(missing_target).unwrap_err(),
        EventParseError::MissingTag("a"),
    );

    let mut missing_destination =
        event_from_parts(wiki_merge_request_to_wire_parts(&merge).unwrap());
    mutate_tags(&mut missing_destination, |tags| {
        tags.retain(|tag| tag.first().map(|value| value.as_str()) != Some("p"));
    });
    assert_parse_error(
        wiki_merge_request_from_event(missing_destination).unwrap_err(),
        EventParseError::MissingTag("p"),
    );

    let mut missing_source = event_from_parts(wiki_merge_request_to_wire_parts(&merge).unwrap());
    mutate_tags(&mut missing_source, |tags| {
        tags.retain(|tag| {
            !(tag.first().map(|value| value.as_str()) == Some("e")
                && tag.last().map(|value| value.as_str()) == Some("source"))
        });
    });
    assert_parse_error(
        wiki_merge_request_from_event(missing_source).unwrap_err(),
        EventParseError::InvalidTag("e"),
    );

    let mut duplicate_source = event_from_parts(wiki_merge_request_to_wire_parts(&merge).unwrap());
    mutate_tags(&mut duplicate_source, |tags| {
        tags.push(vec![
            "e".to_string(),
            hex_64('a'),
            String::new(),
            "source".to_string(),
        ]);
    });
    assert_parse_error(
        wiki_merge_request_from_event(duplicate_source).unwrap_err(),
        EventParseError::InvalidTag("e"),
    );

    let mut wrong_merge_target =
        event_from_parts(wiki_merge_request_to_wire_parts(&merge).unwrap());
    mutate_tags(&mut wrong_merge_target, |tags| {
        for tag in tags {
            if tag.first().map(|value| value.as_str()) == Some("a") {
                tag[1] = format!("30023:{}:soil-health", hex_64('a'));
            }
        }
    });
    assert_parse_error(
        wiki_merge_request_from_event(wrong_merge_target).unwrap_err(),
        EventParseError::InvalidTag("a"),
    );

    let mut orphan_fork = event_from_parts(wiki_article_to_wire_parts(&wiki_article()).unwrap());
    let mut removed_fork_event = false;
    mutate_tags(&mut orphan_fork, |tags| {
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
    assert_parse_error(
        wiki_article_from_event(orphan_fork).unwrap_err(),
        EventParseError::InvalidTag("a"),
    );

    let mut duplicate_defer =
        event_from_parts(wiki_article_to_wire_parts(&wiki_article()).unwrap());
    mutate_tags(&mut duplicate_defer, |tags| {
        tags.extend([
            vec![
                "a".to_string(),
                format!("30818:{}:compost", hex_64('a')),
                String::new(),
                "defer".to_string(),
            ],
            vec![
                "e".to_string(),
                hex_64('c'),
                String::new(),
                "defer".to_string(),
            ],
        ]);
    });
    assert_parse_error(
        wiki_article_from_event(duplicate_defer).unwrap_err(),
        EventParseError::InvalidTag("a"),
    );
}

#[test]
fn wiki_article_version_refs_require_adjacent_marked_pairs() {
    let article_event = event_from_parts(wiki_article_to_wire_parts(&wiki_article()).unwrap());
    let decoded = wiki_article_from_event(article_event).unwrap();
    assert_eq!(decoded.data.data.forked_from[0], article_version_ref());
    assert_eq!(decoded.data.data.deferred_to, Some(article_version_ref()));

    let mut two_forks = wiki_article();
    two_forks.forked_from = vec![
        article_version_ref_for('b', "soil-health"),
        article_version_ref_for('c', "compost"),
    ];
    two_forks.deferred_to = None;
    let two_forks_event = event_from_parts(wiki_article_to_wire_parts(&two_forks).unwrap());
    let decoded = wiki_article_from_event(two_forks_event.clone()).unwrap();
    assert_eq!(
        decoded.data.data.forked_from,
        vec![
            article_version_ref_for('b', "soil-health"),
            article_version_ref_for('c', "compost")
        ]
    );

    let mut grouped = two_forks_event;
    let fork_addresses = grouped
        .tags_as_vec()
        .iter()
        .filter(|tag| marked_tag(tag, "a", "fork"))
        .cloned()
        .collect::<Vec<_>>();
    let fork_events = grouped
        .tags_as_vec()
        .iter()
        .filter(|tag| marked_tag(tag, "e", "fork"))
        .cloned()
        .collect::<Vec<_>>();
    mutate_tags(&mut grouped, |tags| {
        tags.retain(|tag| !marked_tag(tag, "a", "fork") && !marked_tag(tag, "e", "fork"));
        tags.extend(fork_addresses);
        tags.extend(fork_events);
    });
    assert_parse_error(
        wiki_article_from_event(grouped).unwrap_err(),
        EventParseError::InvalidTag("a"),
    );

    let mut reversed = event_from_parts(wiki_article_to_wire_parts(&wiki_article()).unwrap());
    let fork_address_index = marked_tag_index(&reversed, "a", "fork");
    let fork_event_index = marked_tag_index(&reversed, "e", "fork");
    mutate_tags(&mut reversed, |tags| {
        tags.swap(fork_address_index, fork_event_index);
    });
    assert_parse_error(
        wiki_article_from_event(reversed).unwrap_err(),
        EventParseError::InvalidTag("e"),
    );

    let mut relay_mismatch = event_from_parts(wiki_article_to_wire_parts(&wiki_article()).unwrap());
    let fork_event_index = marked_tag_index(&relay_mismatch, "e", "fork");
    mutate_tags(&mut relay_mismatch, |tags| {
        tags[fork_event_index][2] = "wss://other.radroots.example".to_string();
    });
    assert_parse_error(
        wiki_article_from_event(relay_mismatch).unwrap_err(),
        EventParseError::InvalidTag("e"),
    );

    let mut missing_partner =
        event_from_parts(wiki_article_to_wire_parts(&wiki_article()).unwrap());
    let fork_event_index = marked_tag_index(&missing_partner, "e", "fork");
    mutate_tags(&mut missing_partner, |tags| {
        tags.remove(fork_event_index);
    });
    assert_parse_error(
        wiki_article_from_event(missing_partner).unwrap_err(),
        EventParseError::InvalidTag("a"),
    );

    let mut misplaced_marker =
        event_from_parts(wiki_article_to_wire_parts(&wiki_article()).unwrap());
    let fork_address_index = marked_tag_index(&misplaced_marker, "a", "fork");
    mutate_tags(&mut misplaced_marker, |tags| {
        tags[fork_address_index].insert(2, "fork".to_string());
    });
    assert_parse_error(
        wiki_article_from_event(misplaced_marker).unwrap_err(),
        EventParseError::InvalidTag("a"),
    );

    let mut wrong_kind = event_from_parts(wiki_article_to_wire_parts(&wiki_article()).unwrap());
    let fork_address_index = marked_tag_index(&wrong_kind, "a", "fork");
    mutate_tags(&mut wrong_kind, |tags| {
        tags[fork_address_index][1] = format!("30023:{}:soil-health", hex_64('a'));
    });
    assert_parse_error(
        wiki_article_from_event(wrong_kind).unwrap_err(),
        EventParseError::InvalidTag("a"),
    );
}

#[test]
fn wiki_article_codec_accepts_missing_title_tag() {
    let mut article = wiki_article();
    article.title = None;
    let article_event = event_from_parts(wiki_article_to_wire_parts(&article).unwrap());
    assert!(
        !article_event
            .tags_as_vec()
            .iter()
            .any(|tag| tag.first().map(String::as_str) == Some("title"))
    );
    let decoded = wiki_article_from_event(article_event).unwrap();
    assert_eq!(decoded.data.data.title, None);
}

#[test]
fn semantic_validation_rejects_invalid_encode_models() {
    let mut article = wiki_article();
    article.content_djot = String::new();
    assert_encode_error(
        wiki_article_to_wire_parts(&article).unwrap_err(),
        EventEncodeError::EmptyRequiredField("content_djot"),
    );

    let mut redirect = RadrootsWikiRedirect {
        d_tag: "soil".to_string(),
        target: address_ref(),
    };
    redirect.target.kind = 30023;
    assert_encode_error(
        wiki_redirect_to_wire_parts(&redirect).unwrap_err(),
        EventEncodeError::InvalidField("wiki_redirect.target"),
    );

    let mut merge = RadrootsWikiMergeRequest {
        target_article: address_ref(),
        destination_pubkey: "bad".to_string(),
        base_version_event_id: Some(hex_64('e')),
        source_version_event_id: hex_64('f'),
        explanation: None,
    };
    assert_encode_error(
        wiki_merge_request_to_wire_parts(&merge).unwrap_err(),
        EventEncodeError::InvalidField("destination_pubkey"),
    );
    merge.destination_pubkey = hex_64('a');
    merge.base_version_event_id = Some("bad".to_string());
    assert_encode_error(
        wiki_merge_request_to_wire_parts(&merge).unwrap_err(),
        EventEncodeError::InvalidField("base_version_event_id"),
    );

    let mut source = source();
    source.title = " ".to_string();
    assert_encode_error(
        knowledge_source_to_wire_parts(&source).unwrap_err(),
        EventEncodeError::EmptyRequiredField("title"),
    );

    let mut claim = claim();
    claim.citation_spans[0].quote_hash = Some("bad".to_string());
    assert_encode_error(
        knowledge_claim_to_wire_parts(&claim).unwrap_err(),
        EventEncodeError::InvalidField("citation_spans"),
    );

    let mut relation = relation();
    relation.subject.external_id = Some("cover-crops".to_string());
    assert_encode_error(
        knowledge_relation_to_wire_parts(&relation).unwrap_err(),
        EventEncodeError::InvalidField("subject"),
    );

    let mut review = review();
    review.scores[0].value = String::new();
    assert_encode_error(
        knowledge_review_to_wire_parts(&review).unwrap_err(),
        EventEncodeError::EmptyRequiredField("scores"),
    );

    let mut report = field_report();
    report.observations.clear();
    assert_encode_error(
        knowledge_field_report_to_wire_parts(&report).unwrap_err(),
        EventEncodeError::EmptyRequiredField("observations"),
    );

    let bounty = RadrootsEvidenceBounty {
        schema: RADROOTS_EVIDENCE_BOUNTY_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        d_tag: "soil-bounty".to_string(),
        title: "Soil bounty".to_string(),
        summary: None,
        topics: vec!["soil".to_string()],
        target_refs: Vec::new(),
        reward_note: None,
        closes_at: None,
    };
    assert_encode_error(
        evidence_bounty_to_wire_parts(&bounty).unwrap_err(),
        EventEncodeError::EmptyRequiredField("target_refs"),
    );

    let proposal = RadrootsKnowledgeChangeProposal {
        schema: RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        target: event_ref('b', KIND_KNOWLEDGE_CLAIM),
        proposal_type: "amend".to_string(),
        summary: String::new(),
        rationale: None,
        evidence_refs: vec![event_ref('c', KIND_KNOWLEDGE_SOURCE)],
        supersedes: Vec::new(),
    };
    assert_encode_error(
        knowledge_change_proposal_to_wire_parts(&proposal).unwrap_err(),
        EventEncodeError::EmptyRequiredField("summary"),
    );

    let attestation = RadrootsContributionAttestation {
        schema: RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        contributor_pubkey: hex_64('a'),
        contribution_type: "review".to_string(),
        subject_refs: Vec::new(),
        summary: "Reviewed synthetic claim".to_string(),
        evidence_refs: vec![event_ref('e', KIND_KNOWLEDGE_REVIEW)],
    };
    assert_encode_error(
        contribution_attestation_to_wire_parts(&attestation).unwrap_err(),
        EventEncodeError::EmptyRequiredField("subject_refs"),
    );
}

#[test]
fn semantic_validation_rejects_invalid_relay_models() {
    let mut source = source();
    source.artifact_refs[0].relays = Some(vec![invalid_relay()]);
    assert_encode_error(
        knowledge_source_to_wire_parts(&source).unwrap_err(),
        EventEncodeError::InvalidField("artifact_refs"),
    );

    let mut redirect = RadrootsWikiRedirect {
        d_tag: "soil".to_string(),
        target: address_ref(),
    };
    redirect.target.relays = vec![invalid_relay()];
    assert_encode_error(
        wiki_redirect_to_wire_parts(&redirect).unwrap_err(),
        EventEncodeError::InvalidField("wiki_redirect.target"),
    );

    let mut review = review();
    review.target.relays = vec![invalid_relay()];
    assert_encode_error(
        knowledge_review_to_wire_parts(&review).unwrap_err(),
        EventEncodeError::InvalidField("review_target"),
    );

    let mut report = field_report();
    report.context.location_precision = RadrootsKnowledgeLocationPrecision::ExactPrivateReference;
    let mut private_location_ref = event_ref('f', KIND_KNOWLEDGE_SOURCE);
    private_location_ref.relays = Some(vec![invalid_relay()]);
    report.context.private_location_ref = Some(private_location_ref);
    assert_encode_error(
        knowledge_field_report_to_wire_parts(&report).unwrap_err(),
        EventEncodeError::InvalidField("private_location_ref"),
    );
}

#[test]
fn knowledge_claim_encode_enforces_citation_rules() {
    let mut model = claim();
    model.citation_spans.clear();
    assert_encode_error(
        knowledge_claim_to_wire_parts(&model).unwrap_err(),
        EventEncodeError::EmptyRequiredField("citation_spans"),
    );

    assert!(knowledge_claim_to_wire_parts(&claim()).is_ok());

    for claim_type in ["hypothesis", "observation", "question"] {
        let mut uncited = claim();
        uncited.claim_type = claim_type.to_string();
        uncited.citation_spans.clear();
        assert!(knowledge_claim_to_wire_parts(&uncited).is_ok());
    }

    let mut capitalized = claim();
    capitalized.claim_type = "Hypothesis".to_string();
    capitalized.citation_spans.clear();
    assert_encode_error(
        knowledge_claim_to_wire_parts(&capitalized).unwrap_err(),
        EventEncodeError::EmptyRequiredField("citation_spans"),
    );
}

#[test]
fn semantic_validation_rejects_invalid_decoded_content() {
    let mut article = event_from_parts(wiki_article_to_wire_parts(&wiki_article()).unwrap());
    replace_content(&mut article, String::new());
    assert_parse_error(
        wiki_article_from_event(article).unwrap_err(),
        EventParseError::InvalidJson("content_djot"),
    );

    let mut source_event = event_from_parts(knowledge_source_to_wire_parts(&source()).unwrap());
    let mut value: serde_json::Value = serde_json::from_str(source_event.content()).unwrap();
    value["title"] = serde_json::Value::String(String::new());
    replace_content(&mut source_event, serde_json::to_string(&value).unwrap());
    assert_parse_error(
        knowledge_source_from_event(source_event).unwrap_err(),
        EventParseError::InvalidJson("title"),
    );

    let mut claim_event = event_from_parts(knowledge_claim_to_wire_parts(&claim()).unwrap());
    let mut value: serde_json::Value = serde_json::from_str(claim_event.content()).unwrap();
    value["citation_spans"][0]["quote_hash"] = serde_json::Value::String("bad".to_string());
    replace_content(&mut claim_event, serde_json::to_string(&value).unwrap());
    assert_parse_error(
        knowledge_claim_from_event(claim_event).unwrap_err(),
        EventParseError::InvalidJson("citation_spans"),
    );

    let mut relation_event =
        event_from_parts(knowledge_relation_to_wire_parts(&relation()).unwrap());
    let mut value: serde_json::Value = serde_json::from_str(relation_event.content()).unwrap();
    value["subject"]["external_id"] = serde_json::Value::String("cover-crops".to_string());
    replace_content(&mut relation_event, serde_json::to_string(&value).unwrap());
    assert_parse_error(
        knowledge_relation_from_event(relation_event).unwrap_err(),
        EventParseError::InvalidJson("subject"),
    );

    let mut review_event = event_from_parts(knowledge_review_to_wire_parts(&review()).unwrap());
    let mut value: serde_json::Value = serde_json::from_str(review_event.content()).unwrap();
    value["target"]["kind"] = serde_json::Value::from(0);
    replace_content(&mut review_event, serde_json::to_string(&value).unwrap());
    assert_parse_error(
        knowledge_review_from_event(review_event).unwrap_err(),
        EventParseError::InvalidJson("review_target"),
    );

    let mut report_event =
        event_from_parts(knowledge_field_report_to_wire_parts(&field_report()).unwrap());
    let mut value: serde_json::Value = serde_json::from_str(report_event.content()).unwrap();
    value["observations"] = serde_json::Value::Array(Vec::new());
    replace_content(&mut report_event, serde_json::to_string(&value).unwrap());
    assert_parse_error(
        knowledge_field_report_from_event(report_event).unwrap_err(),
        EventParseError::InvalidJson("observations"),
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
    let mut bounty_event = event_from_parts(evidence_bounty_to_wire_parts(&bounty).unwrap());
    let mut value: serde_json::Value = serde_json::from_str(bounty_event.content()).unwrap();
    value["target_refs"] = serde_json::Value::Array(Vec::new());
    replace_content(&mut bounty_event, serde_json::to_string(&value).unwrap());
    assert_parse_error(
        evidence_bounty_from_event(bounty_event).unwrap_err(),
        EventParseError::InvalidJson("target_refs"),
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
    let mut proposal_event =
        event_from_parts(knowledge_change_proposal_to_wire_parts(&proposal).unwrap());
    let mut value: serde_json::Value = serde_json::from_str(proposal_event.content()).unwrap();
    value["summary"] = serde_json::Value::String(String::new());
    replace_content(&mut proposal_event, serde_json::to_string(&value).unwrap());
    assert_parse_error(
        knowledge_change_proposal_from_event(proposal_event).unwrap_err(),
        EventParseError::InvalidJson("summary"),
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
    let mut attestation_event =
        event_from_parts(contribution_attestation_to_wire_parts(&attestation).unwrap());
    let mut value: serde_json::Value = serde_json::from_str(attestation_event.content()).unwrap();
    value["subject_refs"] = serde_json::Value::Array(Vec::new());
    replace_content(
        &mut attestation_event,
        serde_json::to_string(&value).unwrap(),
    );
    assert_parse_error(
        contribution_attestation_from_event(attestation_event).unwrap_err(),
        EventParseError::InvalidJson("subject_refs"),
    );
}

#[test]
fn knowledge_decode_rejects_invalid_relay_values() {
    let mut article_event = event_from_parts(wiki_article_to_wire_parts(&wiki_article()).unwrap());
    mutate_tags(&mut article_event, |tags| {
        let source_tag = tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some("source"))
            .expect("source tag");
        source_tag[5] = invalid_relay();
    });
    assert_parse_error(
        wiki_article_from_event(article_event).unwrap_err(),
        EventParseError::InvalidTag("source"),
    );

    let mut redirect_event = event_from_parts(
        wiki_redirect_to_wire_parts(&RadrootsWikiRedirect {
            d_tag: "soil".to_string(),
            target: address_ref(),
        })
        .unwrap(),
    );
    mutate_tags(&mut redirect_event, |tags| {
        let target_tag = tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some("a"))
            .expect("target tag");
        target_tag[2] = invalid_relay();
    });
    assert_parse_error(
        wiki_redirect_from_event(redirect_event).unwrap_err(),
        EventParseError::InvalidTag("a"),
    );

    let mut source_event = event_from_parts(knowledge_source_to_wire_parts(&source()).unwrap());
    let mut value: serde_json::Value = serde_json::from_str(source_event.content()).unwrap();
    value["artifact_refs"][0]["relays"] =
        serde_json::Value::Array(vec![serde_json::Value::String(invalid_relay())]);
    replace_content(&mut source_event, serde_json::to_string(&value).unwrap());
    assert_parse_error(
        knowledge_source_from_event(source_event).unwrap_err(),
        EventParseError::InvalidJson("artifact_refs"),
    );

    let mut review_event = event_from_parts(knowledge_review_to_wire_parts(&review()).unwrap());
    let mut value: serde_json::Value = serde_json::from_str(review_event.content()).unwrap();
    value["target"]["relays"] =
        serde_json::Value::Array(vec![serde_json::Value::String(invalid_relay())]);
    replace_content(&mut review_event, serde_json::to_string(&value).unwrap());
    assert_parse_error(
        knowledge_review_from_event(review_event).unwrap_err(),
        EventParseError::InvalidJson("review_target"),
    );

    let mut report = field_report();
    report.context.location_precision = RadrootsKnowledgeLocationPrecision::ExactPrivateReference;
    report.context.private_location_ref = Some(event_ref('f', KIND_KNOWLEDGE_SOURCE));
    let mut report_event = event_from_parts(knowledge_field_report_to_wire_parts(&report).unwrap());
    let mut value: serde_json::Value = serde_json::from_str(report_event.content()).unwrap();
    value["context"]["private_location_ref"]["relays"] =
        serde_json::Value::Array(vec![serde_json::Value::String(invalid_relay())]);
    replace_content(&mut report_event, serde_json::to_string(&value).unwrap());
    assert_parse_error(
        knowledge_field_report_from_event(report_event).unwrap_err(),
        EventParseError::InvalidJson("private_location_ref"),
    );
}

#[test]
fn knowledge_claim_decode_enforces_citation_rules() {
    let mut claim_event = event_from_parts(knowledge_claim_to_wire_parts(&claim()).unwrap());
    let mut value: serde_json::Value = serde_json::from_str(claim_event.content()).unwrap();
    value["citation_spans"] = serde_json::Value::Array(Vec::new());
    replace_content(&mut claim_event, serde_json::to_string(&value).unwrap());
    assert_parse_error(
        knowledge_claim_from_event(claim_event).unwrap_err(),
        EventParseError::InvalidJson("citation_spans"),
    );

    for claim_type in ["hypothesis", "observation", "question"] {
        let mut uncited = claim();
        uncited.claim_type = claim_type.to_string();
        uncited.citation_spans.clear();
        let decoded = knowledge_claim_from_event(event_from_parts(
            knowledge_claim_to_wire_parts(&uncited).unwrap(),
        ))
        .unwrap();
        assert_eq!(decoded.data.data.claim_type, claim_type);
        assert!(decoded.data.data.citation_spans.is_empty());
    }
}
