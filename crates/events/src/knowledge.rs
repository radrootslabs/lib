#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use core::fmt;

use crate::RadrootsNostrEventRef;

pub const RADROOTS_KNOWLEDGE_SCHEMA_VERSION: u16 = 1;
pub const RADROOTS_WIKI_D_TAG_MAX_LEN: usize = 512;
pub const RADROOTS_KNOWLEDGE_SOURCE_SCHEMA: &str = "radroots.knowledge.source.v1";
pub const RADROOTS_KNOWLEDGE_CLAIM_SCHEMA: &str = "radroots.knowledge.claim.v1";
pub const RADROOTS_KNOWLEDGE_RELATION_SCHEMA: &str = "radroots.knowledge.relation.v1";
pub const RADROOTS_KNOWLEDGE_REVIEW_SCHEMA: &str = "radroots.knowledge.review.v1";
pub const RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA: &str = "radroots.knowledge.field_report.v1";
pub const RADROOTS_EVIDENCE_BOUNTY_SCHEMA: &str = "radroots.knowledge.evidence_bounty.v1";
pub const RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA: &str = "radroots.knowledge.change_proposal.v1";
pub const RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA: &str =
    "radroots.knowledge.contribution_attestation.v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsWikiDTagError {
    Empty,
    TooLong { max: usize, actual: usize },
    NotNormalized { normalized: String },
}

impl RadrootsWikiDTagError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::TooLong { .. } => "too_long",
            Self::NotNormalized { .. } => "not_normalized",
        }
    }
}

impl fmt::Display for RadrootsWikiDTagError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("wiki d tag must not be empty"),
            Self::TooLong { max, actual } => {
                write!(
                    formatter,
                    "wiki d tag length {actual} exceeds maximum length {max}"
                )
            }
            Self::NotNormalized { normalized } => {
                write!(
                    formatter,
                    "wiki d tag must match normalized value {normalized}"
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsWikiDTagError {}

pub fn normalize_wiki_d_tag(input: &str) -> String {
    let mut normalized = String::new();
    let mut pending_separator = false;

    for raw in input.trim().chars() {
        for character in raw.to_lowercase() {
            if character.is_alphanumeric() {
                if pending_separator && !normalized.is_empty() {
                    normalized.push('-');
                }
                normalized.push(character);
                pending_separator = false;
            } else if character.is_whitespace() || character == '-' {
                pending_separator = true;
            }
        }
    }

    normalized
}

pub fn validate_wiki_d_tag(value: &str) -> Result<(), RadrootsWikiDTagError> {
    if value.is_empty() {
        return Err(RadrootsWikiDTagError::Empty);
    }

    let actual = value.chars().count();
    if actual > RADROOTS_WIKI_D_TAG_MAX_LEN {
        return Err(RadrootsWikiDTagError::TooLong {
            max: RADROOTS_WIKI_D_TAG_MAX_LEN,
            actual,
        });
    }

    let normalized = normalize_wiki_d_tag(value);
    if normalized != value {
        return Err(RadrootsWikiDTagError::NotNormalized { normalized });
    }

    Ok(())
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAddressableRef {
    pub kind: u32,
    pub pubkey: String,
    pub d_tag: String,
    pub relays: Vec<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRightsAssertion {
    pub assertion: String,
    pub holder: Option<String>,
    pub license: Option<String>,
    pub url: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsWikiArticle {
    pub d_tag: String,
    pub title: String,
    pub content_djot: String,
    pub summary: Option<String>,
    pub topics: Vec<String>,
    pub references: Vec<RadrootsNostrEventRef>,
    pub forked_from: Vec<RadrootsNostrEventRef>,
    pub deferred_to: Option<RadrootsNostrEventRef>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsWikiRedirect {
    pub d_tag: String,
    pub target: RadrootsNostrEventRef,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsWikiMergeRequest {
    pub target_article: RadrootsAddressableRef,
    pub destination_pubkey: String,
    pub base_version_event_id: Option<String>,
    pub source_version_event_id: String,
    pub explanation: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeSource {
    pub schema: String,
    pub schema_version: u16,
    pub d_tag: String,
    pub title: String,
    pub source_type: String,
    pub authors: Vec<String>,
    pub publisher: Option<String>,
    pub publication_year: Option<u16>,
    pub edition: Option<String>,
    pub canonical_url: Option<String>,
    pub artifact_refs: Vec<RadrootsNostrEventRef>,
    pub author_asserted_rights: Option<RadrootsRightsAssertion>,
    pub topics: Vec<String>,
    pub summary: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeClaim {
    pub schema: String,
    pub schema_version: u16,
    pub claim_type: String,
    pub text: String,
    pub citation_spans: Vec<RadrootsKnowledgeCitationSpan>,
    pub topics: Vec<String>,
    pub applies_to: Vec<String>,
    pub author_asserted_confidence: Option<String>,
    pub supersedes: Vec<RadrootsNostrEventRef>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeCitationSpan {
    pub source_ref: RadrootsNostrEventRef,
    pub artifact_ref: Option<RadrootsNostrEventRef>,
    pub page_start: Option<u32>,
    pub page_end: Option<u32>,
    pub section_path: Vec<String>,
    pub quote_hash: Option<String>,
    pub chunk_id: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeNodeRef {
    pub node_type: String,
    pub event_ref: Option<RadrootsNostrEventRef>,
    pub address_ref: Option<RadrootsAddressableRef>,
    pub external_id: Option<String>,
    pub label: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeRelation {
    pub schema: String,
    pub schema_version: u16,
    pub subject: RadrootsKnowledgeNodeRef,
    pub predicate: String,
    pub object: RadrootsKnowledgeNodeRef,
    pub support_refs: Vec<RadrootsNostrEventRef>,
    pub author_asserted_confidence: Option<String>,
    pub supersedes: Vec<RadrootsNostrEventRef>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeReview {
    pub schema: String,
    pub schema_version: u16,
    pub target: RadrootsKnowledgeReviewTarget,
    pub reviewer_role: String,
    pub verdict: String,
    pub scores: Vec<RadrootsKnowledgeReviewScore>,
    pub notes: Option<String>,
    pub evidence_refs: Vec<RadrootsNostrEventRef>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeReviewTarget {
    pub event_id: String,
    pub author_pubkey: String,
    pub kind: u32,
    pub address: Option<String>,
    pub relays: Vec<String>,
    pub review_scope: RadrootsKnowledgeReviewScope,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsKnowledgeReviewScope {
    SpecificVersion,
    AddressableCoordinateAtPublishedAt,
    PolicyLatest,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeReviewScore {
    pub dimension: String,
    pub value: String,
    pub note: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeFieldReport {
    pub schema: String,
    pub schema_version: u16,
    pub report_type: String,
    pub title: String,
    pub summary: Option<String>,
    pub context: RadrootsKnowledgeFieldContext,
    pub observations: Vec<RadrootsKnowledgeObservation>,
    pub artifact_refs: Vec<RadrootsNostrEventRef>,
    pub related_refs: Vec<RadrootsNostrEventRef>,
    pub limitations: Vec<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeFieldContext {
    pub location_precision: RadrootsKnowledgeLocationPrecision,
    pub public_location: Option<RadrootsKnowledgeLocation>,
    pub private_location_ref: Option<RadrootsNostrEventRef>,
    pub topics: Vec<String>,
    pub context_tags: Vec<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsKnowledgeLocationPrecision {
    None,
    Region,
    Locality,
    CoarseGeohash,
    ExactPublic,
    ExactPrivateReference,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeLocation {
    pub label: Option<String>,
    pub region: Option<String>,
    pub locality: Option<String>,
    pub geohash: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeObservation {
    pub observation_type: String,
    pub text: String,
    pub observed_at: Option<String>,
    pub values: Vec<RadrootsKnowledgeObservationValue>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeObservationValue {
    pub key: String,
    pub value: String,
    pub unit: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEvidenceBounty {
    pub schema: String,
    pub schema_version: u16,
    pub d_tag: String,
    pub title: String,
    pub summary: Option<String>,
    pub topics: Vec<String>,
    pub target_refs: Vec<RadrootsNostrEventRef>,
    pub reward_note: Option<String>,
    pub closes_at: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeChangeProposal {
    pub schema: String,
    pub schema_version: u16,
    pub target: RadrootsNostrEventRef,
    pub proposal_type: String,
    pub summary: String,
    pub rationale: Option<String>,
    pub evidence_refs: Vec<RadrootsNostrEventRef>,
    pub supersedes: Vec<RadrootsNostrEventRef>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsContributionAttestation {
    pub schema: String,
    pub schema_version: u16,
    pub contributor_pubkey: String,
    pub contribution_type: String,
    pub subject_refs: Vec<RadrootsNostrEventRef>,
    pub summary: String,
    pub evidence_refs: Vec<RadrootsNostrEventRef>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_ref() -> RadrootsNostrEventRef {
        RadrootsNostrEventRef {
            id: "0".repeat(64),
            author: "1".repeat(64),
            kind: 1,
            d_tag: None,
            relays: None,
        }
    }

    #[test]
    fn normalizes_wiki_d_tags() {
        assert_eq!(
            normalize_wiki_d_tag(" Soil Health  Basics "),
            "soil-health-basics"
        );
        assert_eq!(
            normalize_wiki_d_tag("Crème Brûlée 2026"),
            "crème-brûlée-2026"
        );
        assert_eq!(normalize_wiki_d_tag("土壌 健康 101"), "土壌-健康-101");
        assert_eq!(
            normalize_wiki_d_tag("Почва Здоровье 101"),
            "почва-здоровье-101"
        );
        assert_eq!(normalize_wiki_d_tag("El Niño y Suelo"), "el-niño-y-suelo");
        assert_eq!(
            normalize_wiki_d_tag("Field: Water & Soil!"),
            "field-water-soil"
        );
        assert_eq!(normalize_wiki_d_tag("--Field---Notes--"), "field-notes");
        assert_eq!(normalize_wiki_d_tag("中文 農業 101"), "中文-農業-101");
        assert_eq!(normalize_wiki_d_tag("soil 🌱 health"), "soil-health");
        assert_eq!(normalize_wiki_d_tag("!!! 🌱 !!!"), "");
        assert_eq!(
            normalize_wiki_d_tag(&normalize_wiki_d_tag("Soil Health Basics")),
            "soil-health-basics"
        );
    }

    #[test]
    fn validates_normalized_wiki_d_tags() {
        assert_eq!(validate_wiki_d_tag("soil-health-basics"), Ok(()));
        assert_eq!(validate_wiki_d_tag(""), Err(RadrootsWikiDTagError::Empty));
        assert_eq!(
            validate_wiki_d_tag("Soil Health"),
            Err(RadrootsWikiDTagError::NotNormalized {
                normalized: "soil-health".to_string()
            })
        );
    }

    #[test]
    fn rejects_oversized_wiki_d_tags() {
        let value = "a".repeat(RADROOTS_WIKI_D_TAG_MAX_LEN + 1);

        assert_eq!(
            validate_wiki_d_tag(&value),
            Err(RadrootsWikiDTagError::TooLong {
                max: RADROOTS_WIKI_D_TAG_MAX_LEN,
                actual: RADROOTS_WIKI_D_TAG_MAX_LEN + 1,
            })
        );
    }

    #[test]
    fn exposes_knowledge_schema_ids() {
        assert_eq!(RADROOTS_KNOWLEDGE_SCHEMA_VERSION, 1);
        assert_eq!(
            RADROOTS_KNOWLEDGE_SOURCE_SCHEMA,
            "radroots.knowledge.source.v1"
        );
        assert_eq!(
            RADROOTS_KNOWLEDGE_CLAIM_SCHEMA,
            "radroots.knowledge.claim.v1"
        );
        assert_eq!(
            RADROOTS_KNOWLEDGE_RELATION_SCHEMA,
            "radroots.knowledge.relation.v1"
        );
        assert_eq!(
            RADROOTS_KNOWLEDGE_REVIEW_SCHEMA,
            "radroots.knowledge.review.v1"
        );
        assert_eq!(
            RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA,
            "radroots.knowledge.field_report.v1"
        );
        assert_eq!(
            RADROOTS_EVIDENCE_BOUNTY_SCHEMA,
            "radroots.knowledge.evidence_bounty.v1"
        );
        assert_eq!(
            RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA,
            "radroots.knowledge.change_proposal.v1"
        );
        assert_eq!(
            RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA,
            "radroots.knowledge.contribution_attestation.v1"
        );
    }

    #[test]
    fn models_wiki_article_as_djot_payload() {
        let article = RadrootsWikiArticle {
            d_tag: "soil-health".to_string(),
            title: "Soil health".to_string(),
            content_djot: "# Soil health".to_string(),
            summary: Some("Living soil basics".to_string()),
            topics: vec!["soil".to_string()],
            references: vec![event_ref()],
            forked_from: Vec::new(),
            deferred_to: None,
        };

        assert_eq!(article.content_djot, "# Soil health");
        assert_eq!(article.topics, vec!["soil"]);
        assert_eq!(article.references.len(), 1);
    }

    #[test]
    fn models_author_asserted_rights_without_trusted_status() {
        let source = RadrootsKnowledgeSource {
            schema: RADROOTS_KNOWLEDGE_SOURCE_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            d_tag: "soil-source".to_string(),
            title: "Soil Source".to_string(),
            source_type: "book".to_string(),
            authors: vec!["Ada Example".to_string()],
            publisher: None,
            publication_year: Some(2026),
            edition: None,
            canonical_url: None,
            artifact_refs: Vec::new(),
            author_asserted_rights: Some(RadrootsRightsAssertion {
                assertion: "author_asserted_public_domain".to_string(),
                holder: None,
                license: None,
                url: None,
            }),
            topics: vec!["soil".to_string()],
            summary: None,
        };

        assert_eq!(source.schema_version, 1);
        assert_eq!(
            source
                .author_asserted_rights
                .as_ref()
                .expect("rights")
                .assertion,
            "author_asserted_public_domain"
        );
    }

    #[test]
    fn models_field_context_without_exact_private_coordinates() {
        let context = RadrootsKnowledgeFieldContext {
            location_precision: RadrootsKnowledgeLocationPrecision::ExactPrivateReference,
            public_location: Some(RadrootsKnowledgeLocation {
                label: Some("watershed edge".to_string()),
                region: Some("sample-region".to_string()),
                locality: None,
                geohash: Some("c23".to_string()),
            }),
            private_location_ref: Some(event_ref()),
            topics: vec!["water".to_string()],
            context_tags: vec!["field".to_string()],
        };

        assert_eq!(
            context.location_precision,
            RadrootsKnowledgeLocationPrecision::ExactPrivateReference
        );
        assert!(context.private_location_ref.is_some());
        assert_eq!(
            context.public_location.as_ref().expect("location").geohash,
            Some("c23".to_string())
        );
    }
}
