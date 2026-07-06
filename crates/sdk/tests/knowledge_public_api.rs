#![cfg(feature = "knowledge")]

use nostr::{EventBuilder, Keys, Kind, Tag, Timestamp};
use radroots_sdk::knowledge::prelude::*;

const SECRET_KEY_HEX: &str = "0101010101010101010101010101010101010101010101010101010101010101";
const CREATED_AT: u32 = 1_800_000_000;
const RELAY: &str = "wss://relay.radroots.example";

#[test]
fn knowledge_prelude_builds_mvp_wire_parts_without_codec_imports() {
    let article = article_builder().build_event().expect("article");
    let redirect = redirect_builder().build_event().expect("redirect");
    let merge_request = merge_request_builder()
        .build_event()
        .expect("merge request");
    let source = source_builder().build_event().expect("source");
    let claim = claim_builder().build_event().expect("claim");
    let relation = relation_builder().build_event().expect("relation");
    let review = review_builder().build_event().expect("review");
    let field_report = field_report_builder().build_event().expect("field report");

    assert_eq!(article.kind, KIND_WIKI_ARTICLE);
    assert_eq!(redirect.kind, KIND_WIKI_REDIRECT);
    assert_eq!(merge_request.kind, KIND_WIKI_MERGE_REQUEST);
    assert_eq!(source.kind, KIND_KNOWLEDGE_SOURCE);
    assert_eq!(claim.kind, KIND_KNOWLEDGE_CLAIM);
    assert_eq!(relation.kind, KIND_KNOWLEDGE_RELATION);
    assert_eq!(review.kind, KIND_KNOWLEDGE_REVIEW);
    assert_eq!(field_report.kind, KIND_KNOWLEDGE_FIELD_REPORT);
}

#[test]
fn fluent_builders_auto_fill_schema_and_prepare_drafts() {
    let article = article_builder().build().expect("article");
    let redirect = redirect_builder().build().expect("redirect");
    let merge_request = merge_request_builder().build().expect("merge request");
    let source = source_builder().build().expect("source");
    let claim = claim_builder().build().expect("claim");
    let relation = relation_builder().build().expect("relation");
    let review = review_builder().build().expect("review");
    let field_report = field_report_builder().build().expect("field report");

    assert_eq!(article.d_tag, "soil-health");
    assert_eq!(redirect.target, address_ref());
    assert_eq!(
        merge_request.explanation.as_deref(),
        Some("Merge synthetic soil article updates")
    );
    assert_eq!(source.schema, RADROOTS_KNOWLEDGE_SOURCE_SCHEMA);
    assert_eq!(claim.schema, RADROOTS_KNOWLEDGE_CLAIM_SCHEMA);
    assert_eq!(relation.schema, RADROOTS_KNOWLEDGE_RELATION_SCHEMA);
    assert_eq!(review.schema, RADROOTS_KNOWLEDGE_REVIEW_SCHEMA);
    assert_eq!(field_report.schema, RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA);
    assert_eq!(claim.schema_version, RADROOTS_KNOWLEDGE_SCHEMA_VERSION);

    let article_draft = article_builder()
        .build_draft(public_key_hex(), CREATED_AT)
        .expect("article draft");
    let claim_draft = claim_builder()
        .build_draft(public_key_hex(), CREATED_AT)
        .expect("claim draft");
    assert_eq!(article_draft.contract_id, WIKI_ARTICLE_CONTRACT_ID);
    assert_eq!(claim_draft.contract_id, KNOWLEDGE_CLAIM_CONTRACT_ID);
}

#[test]
fn fluent_builders_reject_missing_and_invalid_required_fields() {
    let missing_text = RadrootsKnowledgeClaimBuilder::new()
        .claim_type("practice_effect")
        .build()
        .expect_err("missing text");
    assert_eq!(
        missing_text,
        RadrootsKnowledgeBuilderError::MissingField("text")
    );

    let invalid_d_tag = RadrootsWikiArticleBuilder::new("Soil Health")
        .title("Soil health")
        .content_djot("# Soil health")
        .build()
        .expect_err("invalid d tag");
    assert_eq!(
        invalid_d_tag,
        RadrootsKnowledgeBuilderError::InvalidField("d_tag")
    );
}

#[test]
fn knowledge_claim_builder_enforces_citation_rules() {
    let missing_citations = RadrootsKnowledgeClaimBuilder::new()
        .claim_type("practice_effect")
        .text("Cover crops improve soil structure.")
        .build()
        .expect_err("missing citations");
    assert_eq!(
        missing_citations,
        RadrootsKnowledgeBuilderError::MissingField("citation_spans")
    );

    assert!(claim_builder().build().is_ok());

    for claim_type in ["hypothesis", "observation", "question"] {
        let claim = RadrootsKnowledgeClaimBuilder::new()
            .claim_type(claim_type)
            .text("Synthetic uncited claim.")
            .build()
            .expect("uncited claim");
        assert_eq!(claim.claim_type, claim_type);
        assert!(claim.citation_spans.is_empty());
    }

    let capitalized = RadrootsKnowledgeClaimBuilder::new()
        .claim_type("Hypothesis")
        .text("Synthetic uncited claim.")
        .build()
        .expect_err("capitalized claim type requires citations");
    assert_eq!(
        capitalized,
        RadrootsKnowledgeBuilderError::MissingField("citation_spans")
    );
}

#[test]
fn knowledge_draft_builder_freezes_mvp_drafts_without_runtime() {
    let draft_builder = KnowledgeDraftBuilder::new(public_key_hex(), CREATED_AT);

    let article = draft_builder
        .wiki_article(&wiki_article())
        .expect("article draft");
    let redirect = draft_builder
        .wiki_redirect(&wiki_redirect())
        .expect("redirect draft");
    let merge_request = draft_builder
        .wiki_merge_request(&wiki_merge_request())
        .expect("merge request draft");
    let source = draft_builder
        .knowledge_source(&knowledge_source())
        .expect("source draft");
    let claim = draft_builder
        .knowledge_claim(&knowledge_claim())
        .expect("claim draft");
    let relation = draft_builder
        .knowledge_relation(&knowledge_relation())
        .expect("relation draft");
    let review = draft_builder
        .knowledge_review(&knowledge_review())
        .expect("review draft");
    let field_report = draft_builder
        .knowledge_field_report(&knowledge_field_report())
        .expect("field report draft");

    assert_eq!(article.contract_id, WIKI_ARTICLE_CONTRACT_ID);
    assert_eq!(redirect.contract_id, WIKI_REDIRECT_CONTRACT_ID);
    assert_eq!(merge_request.contract_id, WIKI_MERGE_REQUEST_CONTRACT_ID);
    assert_eq!(source.contract_id, KNOWLEDGE_SOURCE_CONTRACT_ID);
    assert_eq!(claim.contract_id, KNOWLEDGE_CLAIM_CONTRACT_ID);
    assert_eq!(relation.contract_id, KNOWLEDGE_RELATION_CONTRACT_ID);
    assert_eq!(review.contract_id, KNOWLEDGE_REVIEW_CONTRACT_ID);
    assert_eq!(field_report.contract_id, KNOWLEDGE_FIELD_REPORT_CONTRACT_ID);
    assert_eq!(claim.expected_pubkey, public_key_hex());
    assert_eq!(claim.created_at, CREATED_AT);
    assert_eq!(claim.kind, KIND_KNOWLEDGE_CLAIM);
}

#[test]
fn knowledge_codec_exposes_manifest_and_verified_decode() {
    let codec = KnowledgeCodec::new();
    let manifest = codec.contract_manifest();

    assert_eq!(
        manifest.schema_version,
        RADROOTS_KNOWLEDGE_CONTRACT_MANIFEST_SCHEMA_VERSION
    );
    assert_eq!(manifest.contract_count, 11);
    assert!(
        manifest
            .contracts
            .iter()
            .any(|contract| contract.contract_id == KNOWLEDGE_CLAIM_CONTRACT_ID)
    );
    let claim_contract = manifest
        .contracts
        .iter()
        .find(|contract| contract.contract_id == KNOWLEDGE_CLAIM_CONTRACT_ID)
        .expect("claim manifest contract");
    assert!(claim_contract.sdk_builder_support);
    assert!(claim_contract.sdk_draft_support);
    assert!(claim_contract.wasm_tag_builder_support);
    assert!(claim_contract.wasm_verified_decode_support);

    let signed = sign_parts(claim_builder().build_event().expect("claim parts"));
    let decoded = codec
        .verify_and_decode_radroots_event(signed)
        .expect("decoded claim");

    match decoded {
        RadrootsDecodedEvent::KnowledgeClaim(parsed) => {
            assert_eq!(parsed.data.data.text, "Cover crops improve soil structure.");
        }
        _ => panic!("expected knowledge claim"),
    }

    let sha256 = codec.contract_manifest_sha256().expect("manifest sha256");
    assert_eq!(sha256.len(), 64);
}

#[test]
fn knowledge_errors_expose_stable_codes() {
    let mut article = wiki_article();
    article.d_tag = "Soil Health".to_owned();
    let error = build_wiki_article_event(&article).expect_err("invalid d tag");

    assert_eq!(error.code(), "knowledge_encode");
    assert_eq!(error.inner_code(), "invalid_field");
    assert!(!error.to_string().contains(article.content_djot.as_str()));
}

fn sign_parts(parts: WireEventParts) -> RadrootsNostrEvent {
    let tags = parts
        .tags
        .into_iter()
        .map(Tag::parse)
        .collect::<Result<Vec<_>, _>>()
        .expect("tags");
    let keys = Keys::parse(SECRET_KEY_HEX).expect("keys");
    let event = EventBuilder::new(Kind::Custom(parts.kind as u16), parts.content)
        .tags(tags)
        .custom_created_at(Timestamp::from_secs(u64::from(CREATED_AT)))
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

fn public_key_hex() -> String {
    Keys::parse(SECRET_KEY_HEX)
        .expect("keys")
        .public_key()
        .to_hex()
}

fn hex_64(character: char) -> String {
    character.to_string().repeat(64)
}

fn event_ref(character: char, kind: u32) -> RadrootsNostrEventRef {
    RadrootsNostrEventRef {
        id: hex_64(character),
        author: hex_64('a'),
        kind,
        d_tag: None,
        relays: Some(vec![RELAY.to_owned()]),
    }
}

fn address_ref() -> RadrootsAddressableRef {
    RadrootsAddressableRef {
        kind: KIND_WIKI_ARTICLE,
        pubkey: hex_64('a'),
        d_tag: "soil-health".to_owned(),
        relays: vec![RELAY.to_owned()],
    }
}

fn wiki_article() -> RadrootsWikiArticle {
    RadrootsWikiArticle {
        d_tag: "soil-health".to_owned(),
        title: "Soil health".to_owned(),
        content_djot: "# Soil health".to_owned(),
        summary: Some("Living soil basics".to_owned()),
        topics: vec!["soil".to_owned(), "local-food".to_owned()],
        references: vec![event_ref('1', KIND_KNOWLEDGE_SOURCE)],
        forked_from: Vec::new(),
        deferred_to: None,
    }
}

fn wiki_redirect() -> RadrootsWikiRedirect {
    RadrootsWikiRedirect {
        d_tag: "soil".to_owned(),
        target: address_ref(),
    }
}

fn article_version_ref() -> RadrootsWikiArticleVersionRef {
    RadrootsWikiArticleVersionRef {
        event_id: hex_64('b'),
        address_ref: address_ref(),
    }
}

fn wiki_merge_request() -> RadrootsWikiMergeRequest {
    RadrootsWikiMergeRequest {
        target_article: address_ref(),
        destination_pubkey: hex_64('a'),
        base_version_event_id: Some(hex_64('e')),
        source_version_event_id: hex_64('f'),
        explanation: Some("Merge synthetic soil article updates".to_owned()),
    }
}

fn article_builder() -> RadrootsWikiArticleBuilder {
    RadrootsWikiArticleBuilder::new("soil-health")
        .title("Soil health")
        .content_djot("# Soil health")
        .summary("Living soil basics")
        .topic("soil")
        .topic("local-food")
        .reference(event_ref('1', KIND_KNOWLEDGE_SOURCE))
        .forked_from(article_version_ref())
}

fn redirect_builder() -> RadrootsWikiRedirectBuilder {
    RadrootsWikiRedirectBuilder::new("soil").target(address_ref())
}

fn merge_request_builder() -> RadrootsWikiMergeRequestBuilder {
    RadrootsWikiMergeRequestBuilder::new()
        .target_article(address_ref())
        .destination_pubkey(hex_64('a'))
        .base_version_event_id(hex_64('e'))
        .source_version_event_id(hex_64('f'))
        .explanation("Merge synthetic soil article updates")
}

fn source_builder() -> RadrootsKnowledgeSourceBuilder {
    RadrootsKnowledgeSourceBuilder::new("soil-source")
        .title("Soil Source")
        .source_type("book")
        .author("A. Example")
        .publisher("Radroots Synthetic Press")
        .publication_year(2026)
        .canonical_url("https://source.example.test/soil-source")
        .artifact_ref(event_ref('3', KIND_FILE_METADATA))
        .topic("soil")
        .summary("Synthetic source for SDK coverage")
}

fn claim_builder() -> RadrootsKnowledgeClaimBuilder {
    RadrootsKnowledgeClaimBuilder::new()
        .claim_type("practice_effect")
        .text("Cover crops improve soil structure.")
        .citation_span(RadrootsKnowledgeCitationSpan {
            source_ref: event_ref('4', KIND_KNOWLEDGE_SOURCE),
            artifact_ref: None,
            page_start: Some(12),
            page_end: Some(13),
            section_path: vec!["chapter-1".to_owned()],
            quote_hash: Some(hex_64('5')),
            chunk_id: Some("chunk-1".to_owned()),
        })
        .topic("cover-crops")
        .applies_to("local-food")
        .author_asserted_confidence("medium")
}

fn relation_builder() -> RadrootsKnowledgeRelationBuilder {
    RadrootsKnowledgeRelationBuilder::new()
        .subject(knowledge_node_ref("cover crops"))
        .predicate("supports")
        .object(knowledge_node_ref("soil structure"))
        .support_ref(event_ref('7', KIND_KNOWLEDGE_CLAIM))
        .author_asserted_confidence("medium")
}

fn review_builder() -> RadrootsKnowledgeReviewBuilder {
    RadrootsKnowledgeReviewBuilder::new()
        .target(RadrootsKnowledgeReviewTarget {
            event_id: hex_64('8'),
            author_pubkey: hex_64('a'),
            kind: KIND_KNOWLEDGE_CLAIM,
            address: None,
            relays: vec![RELAY.to_owned()],
            review_scope: RadrootsKnowledgeReviewScope::SpecificVersion,
        })
        .reviewer_role("peer")
        .verdict("needs_more_evidence")
        .score(RadrootsKnowledgeReviewScore {
            dimension: "evidence".to_owned(),
            value: "partial".to_owned(),
            note: None,
        })
        .notes("Synthetic review")
        .evidence_ref(event_ref('9', KIND_KNOWLEDGE_SOURCE))
}

fn field_report_builder() -> RadrootsKnowledgeFieldReportBuilder {
    RadrootsKnowledgeFieldReportBuilder::new()
        .report_type("observation")
        .title("Field observation")
        .summary("Observed cover crop residue.")
        .context(RadrootsKnowledgeFieldContext {
            location_precision: RadrootsKnowledgeLocationPrecision::CoarseGeohash,
            public_location: Some(RadrootsKnowledgeLocation {
                label: Some("watershed".to_owned()),
                region: Some("synthetic-region".to_owned()),
                locality: None,
                geohash: Some("c23".to_owned()),
            }),
            private_location_ref: None,
            topics: vec!["field".to_owned()],
            context_tags: vec!["observation".to_owned()],
        })
        .observation(RadrootsKnowledgeObservation {
            observation_type: "residue".to_owned(),
            text: "Residue was visible across beds.".to_owned(),
            observed_at: Some("2026-07-05".to_owned()),
            values: vec![RadrootsKnowledgeObservationValue {
                key: "coverage".to_owned(),
                value: "medium".to_owned(),
                unit: None,
            }],
        })
        .artifact_ref(event_ref('c', KIND_FILE_METADATA))
        .related_ref(event_ref('d', KIND_KNOWLEDGE_CLAIM))
        .limitation("single observer")
}

fn knowledge_source() -> RadrootsKnowledgeSource {
    RadrootsKnowledgeSource {
        schema: RADROOTS_KNOWLEDGE_SOURCE_SCHEMA.to_owned(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        d_tag: "soil-source".to_owned(),
        title: "Soil Source".to_owned(),
        source_type: "book".to_owned(),
        authors: vec!["A. Example".to_owned()],
        publisher: Some("Radroots Synthetic Press".to_owned()),
        publication_year: Some(2026),
        edition: None,
        canonical_url: Some("https://source.example.test/soil-source".to_owned()),
        artifact_refs: vec![event_ref('3', KIND_FILE_METADATA)],
        author_asserted_rights: None,
        topics: vec!["soil".to_owned()],
        summary: Some("Synthetic source for SDK coverage".to_owned()),
    }
}

fn knowledge_claim() -> RadrootsKnowledgeClaim {
    RadrootsKnowledgeClaim {
        schema: RADROOTS_KNOWLEDGE_CLAIM_SCHEMA.to_owned(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        claim_type: "practice_effect".to_owned(),
        text: "Cover crops improve soil structure.".to_owned(),
        citation_spans: vec![RadrootsKnowledgeCitationSpan {
            source_ref: event_ref('4', KIND_KNOWLEDGE_SOURCE),
            artifact_ref: None,
            page_start: Some(12),
            page_end: Some(13),
            section_path: vec!["chapter-1".to_owned()],
            quote_hash: Some(hex_64('5')),
            chunk_id: Some("chunk-1".to_owned()),
        }],
        topics: vec!["cover-crops".to_owned()],
        applies_to: vec!["local-food".to_owned()],
        author_asserted_confidence: Some("medium".to_owned()),
        supersedes: Vec::new(),
    }
}

fn knowledge_node_ref(label: &str) -> RadrootsKnowledgeNodeRef {
    RadrootsKnowledgeNodeRef {
        node_type: "event".to_owned(),
        event_ref: Some(event_ref('6', KIND_KNOWLEDGE_CLAIM)),
        address_ref: None,
        external_id: None,
        label: Some(label.to_owned()),
    }
}

fn knowledge_relation() -> RadrootsKnowledgeRelation {
    RadrootsKnowledgeRelation {
        schema: RADROOTS_KNOWLEDGE_RELATION_SCHEMA.to_owned(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        subject: knowledge_node_ref("cover crops"),
        predicate: "supports".to_owned(),
        object: knowledge_node_ref("soil structure"),
        support_refs: vec![event_ref('7', KIND_KNOWLEDGE_CLAIM)],
        author_asserted_confidence: Some("medium".to_owned()),
        supersedes: Vec::new(),
    }
}

fn knowledge_review() -> RadrootsKnowledgeReview {
    RadrootsKnowledgeReview {
        schema: RADROOTS_KNOWLEDGE_REVIEW_SCHEMA.to_owned(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        target: RadrootsKnowledgeReviewTarget {
            event_id: hex_64('8'),
            author_pubkey: hex_64('a'),
            kind: KIND_KNOWLEDGE_CLAIM,
            address: None,
            relays: vec![RELAY.to_owned()],
            review_scope: RadrootsKnowledgeReviewScope::SpecificVersion,
        },
        reviewer_role: "peer".to_owned(),
        verdict: "needs_more_evidence".to_owned(),
        scores: vec![RadrootsKnowledgeReviewScore {
            dimension: "evidence".to_owned(),
            value: "partial".to_owned(),
            note: None,
        }],
        notes: Some("Synthetic review".to_owned()),
        evidence_refs: vec![event_ref('9', KIND_KNOWLEDGE_SOURCE)],
    }
}

fn knowledge_field_report() -> RadrootsKnowledgeFieldReport {
    RadrootsKnowledgeFieldReport {
        schema: RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA.to_owned(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        report_type: "observation".to_owned(),
        title: "Field observation".to_owned(),
        summary: Some("Observed cover crop residue.".to_owned()),
        context: RadrootsKnowledgeFieldContext {
            location_precision: RadrootsKnowledgeLocationPrecision::CoarseGeohash,
            public_location: Some(RadrootsKnowledgeLocation {
                label: Some("watershed".to_owned()),
                region: Some("synthetic-region".to_owned()),
                locality: None,
                geohash: Some("c23".to_owned()),
            }),
            private_location_ref: None,
            topics: vec!["field".to_owned()],
            context_tags: vec!["observation".to_owned()],
        },
        observations: vec![RadrootsKnowledgeObservation {
            observation_type: "residue".to_owned(),
            text: "Residue was visible across beds.".to_owned(),
            observed_at: Some("2026-07-05".to_owned()),
            values: vec![RadrootsKnowledgeObservationValue {
                key: "coverage".to_owned(),
                value: "medium".to_owned(),
                unit: None,
            }],
        }],
        artifact_refs: vec![event_ref('c', KIND_FILE_METADATA)],
        related_refs: vec![event_ref('d', KIND_KNOWLEDGE_CLAIM)],
        limitations: vec!["single observer".to_owned()],
    }
}
