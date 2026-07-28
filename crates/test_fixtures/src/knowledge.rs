use radroots_event::RadrootsEventRef;
use radroots_event::kinds::{
    KIND_CONTRIBUTION_ATTESTATION, KIND_EVIDENCE_BOUNTY, KIND_FILE_METADATA,
    KIND_KNOWLEDGE_CHANGE_PROPOSAL, KIND_KNOWLEDGE_CLAIM, KIND_KNOWLEDGE_FIELD_REPORT,
    KIND_KNOWLEDGE_RELATION, KIND_KNOWLEDGE_REVIEW, KIND_KNOWLEDGE_SOURCE, KIND_WIKI_ARTICLE,
    KIND_WIKI_MERGE_REQUEST, KIND_WIKI_REDIRECT,
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
use radroots_identity::PublicKey;

use crate::RELAY_PRIMARY_WSS;

pub const RADROOTS_KNOWLEDGE_FIXTURE_NAMESPACE: &str = "radroots-knowledge-fixture-v1";

pub const RADROOTS_KNOWLEDGE_VALID_CONTRACT_IDS: [&str; 11] = [
    "radroots.wiki.article.v1",
    "radroots.wiki.redirect.v1",
    "radroots.wiki.merge_request.v1",
    RADROOTS_KNOWLEDGE_SOURCE_SCHEMA,
    RADROOTS_KNOWLEDGE_CLAIM_SCHEMA,
    RADROOTS_KNOWLEDGE_RELATION_SCHEMA,
    RADROOTS_KNOWLEDGE_REVIEW_SCHEMA,
    RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA,
    RADROOTS_EVIDENCE_BOUNTY_SCHEMA,
    RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA,
    RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeAdversarialFixture {
    pub id: &'static str,
    pub pipeline_stage: &'static str,
    pub expected_error_code: &'static str,
}

pub const RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES: [RadrootsKnowledgeAdversarialFixture; 13] = [
    RadrootsKnowledgeAdversarialFixture {
        id: "malformed_tags",
        pipeline_stage: "contract_validation",
        expected_error_code: "tag_cardinality_mismatch",
    },
    RadrootsKnowledgeAdversarialFixture {
        id: "wrong_schema",
        pipeline_stage: "contract_validation",
        expected_error_code: "content_field_mismatch",
    },
    RadrootsKnowledgeAdversarialFixture {
        id: "missing_contract_id",
        pipeline_stage: "contract_validation",
        expected_error_code: "contract_match",
    },
    RadrootsKnowledgeAdversarialFixture {
        id: "private_coordinate_leakage",
        pipeline_stage: "event_parse",
        expected_error_code: "invalid_json",
    },
    RadrootsKnowledgeAdversarialFixture {
        id: "unsupported_contract_shape",
        pipeline_stage: "contract_validation",
        expected_error_code: "contract_match",
    },
    RadrootsKnowledgeAdversarialFixture {
        id: "invalid_nip54_d_tag",
        pipeline_stage: "encode",
        expected_error_code: "invalid_field",
    },
    RadrootsKnowledgeAdversarialFixture {
        id: "invalid_redirect_target_kind",
        pipeline_stage: "encode",
        expected_error_code: "invalid_field",
    },
    RadrootsKnowledgeAdversarialFixture {
        id: "merge_request_missing_source_marker",
        pipeline_stage: "event_parse",
        expected_error_code: "invalid_tag",
    },
    RadrootsKnowledgeAdversarialFixture {
        id: "merge_request_json_content_guard",
        pipeline_stage: "wire_shape",
        expected_error_code: "plain_text_content",
    },
    RadrootsKnowledgeAdversarialFixture {
        id: "orphan_fork_marker",
        pipeline_stage: "event_parse",
        expected_error_code: "invalid_tag",
    },
    RadrootsKnowledgeAdversarialFixture {
        id: "orphan_defer_marker",
        pipeline_stage: "event_parse",
        expected_error_code: "invalid_tag",
    },
    RadrootsKnowledgeAdversarialFixture {
        id: "id_mismatch",
        pipeline_stage: "nip01_verification",
        expected_error_code: "id_mismatch",
    },
    RadrootsKnowledgeAdversarialFixture {
        id: "signature_invalidity",
        pipeline_stage: "nip01_verification",
        expected_error_code: "signature_invalid",
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsKnowledgeFixture {
    WikiArticle(RadrootsWikiArticle),
    WikiRedirect(RadrootsWikiRedirect),
    WikiMergeRequest(RadrootsWikiMergeRequest),
    KnowledgeSource(RadrootsKnowledgeSource),
    KnowledgeClaim(RadrootsKnowledgeClaim),
    KnowledgeRelation(RadrootsKnowledgeRelation),
    KnowledgeReview(RadrootsKnowledgeReview),
    KnowledgeFieldReport(RadrootsKnowledgeFieldReport),
    EvidenceBounty(RadrootsEvidenceBounty),
    KnowledgeChangeProposal(RadrootsKnowledgeChangeProposal),
    ContributionAttestation(RadrootsContributionAttestation),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeFixtureCase {
    pub id: &'static str,
    pub contract_id: &'static str,
    pub kind: u32,
    pub data: RadrootsKnowledgeFixture,
}

pub fn hex_64(character: char) -> String {
    core::iter::repeat_n(character, 64).collect()
}

pub fn event_ref(character: char, kind: u32) -> RadrootsEventRef {
    RadrootsEventRef {
        id: hex_64(character),
        author: PublicKey::from_hex(
            "585591529da0bab31b3b1f986611cf5f435dca84f978c89ee8a40cca7103df",
        )
        .expect("fixture public key is valid"),
        kind,
        d_tag: None,
        relays: Some(vec![RELAY_PRIMARY_WSS.to_string()]),
    }
}

pub fn address_ref() -> RadrootsAddressableRef {
    RadrootsAddressableRef {
        kind: KIND_WIKI_ARTICLE,
        pubkey: hex_64('a'),
        d_tag: "soil-health".to_string(),
        relays: vec![RELAY_PRIMARY_WSS.to_string()],
    }
}

pub fn deferred_address_ref() -> RadrootsAddressableRef {
    RadrootsAddressableRef {
        kind: KIND_WIKI_ARTICLE,
        pubkey: hex_64('a'),
        d_tag: "soil-health-v2".to_string(),
        relays: vec![RELAY_PRIMARY_WSS.to_string()],
    }
}

pub fn wiki_article_version_ref() -> RadrootsWikiArticleVersionRef {
    RadrootsWikiArticleVersionRef {
        event_id: hex_64('b'),
        address_ref: address_ref(),
    }
}

pub fn wiki_article_deferred_version_ref() -> RadrootsWikiArticleVersionRef {
    RadrootsWikiArticleVersionRef {
        event_id: hex_64('c'),
        address_ref: deferred_address_ref(),
    }
}

pub fn wiki_article() -> RadrootsWikiArticle {
    RadrootsWikiArticle {
        d_tag: "soil-health".to_string(),
        title: Some("Soil health".to_string()),
        content_djot: "# Soil health\n\nLiving soil supports resilient local food systems."
            .to_string(),
        summary: Some("Living soil basics".to_string()),
        topics: vec!["soil".to_string(), "local-food".to_string()],
        references: vec![event_ref('2', KIND_KNOWLEDGE_SOURCE)],
        forked_from: vec![wiki_article_version_ref()],
        deferred_to: Some(wiki_article_deferred_version_ref()),
    }
}

pub fn wiki_redirect() -> RadrootsWikiRedirect {
    RadrootsWikiRedirect {
        d_tag: "soil".to_string(),
        target: address_ref(),
    }
}

pub fn wiki_merge_request() -> RadrootsWikiMergeRequest {
    RadrootsWikiMergeRequest {
        target_article: address_ref(),
        destination_pubkey: hex_64('a'),
        base_version_event_id: Some(hex_64('e')),
        source_version_event_id: hex_64('f'),
        explanation: Some("Merge synthetic soil article updates".to_string()),
    }
}

pub fn wiki_merge_request_without_base_version() -> RadrootsWikiMergeRequest {
    RadrootsWikiMergeRequest {
        base_version_event_id: None,
        ..wiki_merge_request()
    }
}

pub fn knowledge_source() -> RadrootsKnowledgeSource {
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
        canonical_url: Some("https://source.example.test/soil-source".to_string()),
        artifact_refs: vec![event_ref('3', KIND_FILE_METADATA)],
        author_asserted_rights: None,
        topics: vec!["soil".to_string()],
        summary: Some("Synthetic source for knowledge fixture coverage".to_string()),
    }
}

pub fn knowledge_claim() -> RadrootsKnowledgeClaim {
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

pub fn knowledge_node_ref(label: &str) -> RadrootsKnowledgeNodeRef {
    RadrootsKnowledgeNodeRef {
        node_type: "event".to_string(),
        event_ref: Some(event_ref('6', KIND_KNOWLEDGE_CLAIM)),
        address_ref: None,
        external_id: None,
        label: Some(label.to_string()),
    }
}

pub fn knowledge_relation() -> RadrootsKnowledgeRelation {
    RadrootsKnowledgeRelation {
        schema: RADROOTS_KNOWLEDGE_RELATION_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        subject: knowledge_node_ref("cover crops"),
        predicate: "supports".to_string(),
        object: knowledge_node_ref("soil structure"),
        support_refs: vec![event_ref('7', KIND_KNOWLEDGE_CLAIM)],
        author_asserted_confidence: Some("medium".to_string()),
        supersedes: Vec::new(),
    }
}

pub fn knowledge_review() -> RadrootsKnowledgeReview {
    RadrootsKnowledgeReview {
        schema: RADROOTS_KNOWLEDGE_REVIEW_SCHEMA.to_string(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        target: RadrootsKnowledgeReviewTarget {
            event_id: hex_64('8'),
            author_pubkey: hex_64('a'),
            kind: KIND_KNOWLEDGE_CLAIM,
            address: None,
            relays: vec![RELAY_PRIMARY_WSS.to_string()],
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

pub fn knowledge_field_report() -> RadrootsKnowledgeFieldReport {
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
        artifact_refs: vec![event_ref('c', KIND_FILE_METADATA)],
        related_refs: vec![event_ref('d', KIND_KNOWLEDGE_CLAIM)],
        limitations: vec!["single observer".to_string()],
    }
}

pub fn evidence_bounty() -> RadrootsEvidenceBounty {
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

pub fn knowledge_change_proposal() -> RadrootsKnowledgeChangeProposal {
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

pub fn contribution_attestation() -> RadrootsContributionAttestation {
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

pub fn knowledge_valid_fixtures() -> Vec<RadrootsKnowledgeFixtureCase> {
    vec![
        RadrootsKnowledgeFixtureCase {
            id: "wiki_article_valid",
            contract_id: "radroots.wiki.article.v1",
            kind: KIND_WIKI_ARTICLE,
            data: RadrootsKnowledgeFixture::WikiArticle(wiki_article()),
        },
        RadrootsKnowledgeFixtureCase {
            id: "wiki_redirect_valid",
            contract_id: "radroots.wiki.redirect.v1",
            kind: KIND_WIKI_REDIRECT,
            data: RadrootsKnowledgeFixture::WikiRedirect(wiki_redirect()),
        },
        RadrootsKnowledgeFixtureCase {
            id: "wiki_merge_request_valid",
            contract_id: "radroots.wiki.merge_request.v1",
            kind: KIND_WIKI_MERGE_REQUEST,
            data: RadrootsKnowledgeFixture::WikiMergeRequest(wiki_merge_request()),
        },
        RadrootsKnowledgeFixtureCase {
            id: "wiki_merge_request_without_base_valid",
            contract_id: "radroots.wiki.merge_request.v1",
            kind: KIND_WIKI_MERGE_REQUEST,
            data: RadrootsKnowledgeFixture::WikiMergeRequest(
                wiki_merge_request_without_base_version(),
            ),
        },
        RadrootsKnowledgeFixtureCase {
            id: "knowledge_source_valid",
            contract_id: RADROOTS_KNOWLEDGE_SOURCE_SCHEMA,
            kind: KIND_KNOWLEDGE_SOURCE,
            data: RadrootsKnowledgeFixture::KnowledgeSource(knowledge_source()),
        },
        RadrootsKnowledgeFixtureCase {
            id: "knowledge_claim_valid",
            contract_id: RADROOTS_KNOWLEDGE_CLAIM_SCHEMA,
            kind: KIND_KNOWLEDGE_CLAIM,
            data: RadrootsKnowledgeFixture::KnowledgeClaim(knowledge_claim()),
        },
        RadrootsKnowledgeFixtureCase {
            id: "knowledge_relation_valid",
            contract_id: RADROOTS_KNOWLEDGE_RELATION_SCHEMA,
            kind: KIND_KNOWLEDGE_RELATION,
            data: RadrootsKnowledgeFixture::KnowledgeRelation(knowledge_relation()),
        },
        RadrootsKnowledgeFixtureCase {
            id: "knowledge_review_valid",
            contract_id: RADROOTS_KNOWLEDGE_REVIEW_SCHEMA,
            kind: KIND_KNOWLEDGE_REVIEW,
            data: RadrootsKnowledgeFixture::KnowledgeReview(knowledge_review()),
        },
        RadrootsKnowledgeFixtureCase {
            id: "knowledge_field_report_valid",
            contract_id: RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA,
            kind: KIND_KNOWLEDGE_FIELD_REPORT,
            data: RadrootsKnowledgeFixture::KnowledgeFieldReport(knowledge_field_report()),
        },
        RadrootsKnowledgeFixtureCase {
            id: "evidence_bounty_valid",
            contract_id: RADROOTS_EVIDENCE_BOUNTY_SCHEMA,
            kind: KIND_EVIDENCE_BOUNTY,
            data: RadrootsKnowledgeFixture::EvidenceBounty(evidence_bounty()),
        },
        RadrootsKnowledgeFixtureCase {
            id: "knowledge_change_proposal_valid",
            contract_id: RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA,
            kind: KIND_KNOWLEDGE_CHANGE_PROPOSAL,
            data: RadrootsKnowledgeFixture::KnowledgeChangeProposal(knowledge_change_proposal()),
        },
        RadrootsKnowledgeFixtureCase {
            id: "contribution_attestation_valid",
            contract_id: RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA,
            kind: KIND_CONTRIBUTION_ATTESTATION,
            data: RadrootsKnowledgeFixture::ContributionAttestation(contribution_attestation()),
        },
    ]
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::{
        RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES, RADROOTS_KNOWLEDGE_FIXTURE_NAMESPACE,
        RADROOTS_KNOWLEDGE_VALID_CONTRACT_IDS, knowledge_valid_fixtures,
    };

    #[test]
    fn valid_fixture_catalog_covers_all_contract_ids() {
        let fixtures = knowledge_valid_fixtures();
        assert!(fixtures.len() >= RADROOTS_KNOWLEDGE_VALID_CONTRACT_IDS.len());
        for contract_id in RADROOTS_KNOWLEDGE_VALID_CONTRACT_IDS {
            assert!(
                fixtures
                    .iter()
                    .any(|fixture| fixture.contract_id == contract_id),
                "{contract_id}"
            );
        }
        assert_eq!(
            RADROOTS_KNOWLEDGE_FIXTURE_NAMESPACE,
            "radroots-knowledge-fixture-v1"
        );
    }

    #[test]
    fn adversarial_fixture_catalog_covers_required_cases() {
        let expected = [
            "malformed_tags",
            "wrong_schema",
            "missing_contract_id",
            "private_coordinate_leakage",
            "unsupported_contract_shape",
            "invalid_nip54_d_tag",
            "invalid_redirect_target_kind",
            "merge_request_missing_source_marker",
            "merge_request_json_content_guard",
            "orphan_fork_marker",
            "orphan_defer_marker",
            "id_mismatch",
            "signature_invalidity",
        ];
        assert_eq!(
            RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES.len(),
            expected.len()
        );
        for id in expected {
            assert!(
                RADROOTS_KNOWLEDGE_ADVERSARIAL_FIXTURES
                    .iter()
                    .any(|fixture| fixture.id == id),
                "{id}"
            );
        }
    }
}
