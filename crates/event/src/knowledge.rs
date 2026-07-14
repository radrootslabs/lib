#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use core::fmt;

use crate::RadrootsEventRef;
use crate::ids::{
    RadrootsAddressableCoordinate, RadrootsDTag, RadrootsEventId, RadrootsPublicKey,
    RadrootsRelayUrl,
};
use crate::kinds::KIND_WIKI_ARTICLE;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsKnowledgeValidationError {
    EmptyField(&'static str),
    InvalidField(&'static str),
}

impl RadrootsKnowledgeValidationError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::EmptyField(_) => "empty_field",
            Self::InvalidField(_) => "invalid_field",
        }
    }

    pub const fn field(self) -> &'static str {
        match self {
            Self::EmptyField(field) | Self::InvalidField(field) => field,
        }
    }
}

impl fmt::Display for RadrootsKnowledgeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(formatter, "empty knowledge field: {field}"),
            Self::InvalidField(field) => write!(formatter, "invalid knowledge field: {field}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsKnowledgeValidationError {}

fn require_non_empty(
    value: &str,
    field: &'static str,
) -> Result<(), RadrootsKnowledgeValidationError> {
    if value.trim().is_empty() {
        Err(RadrootsKnowledgeValidationError::EmptyField(field))
    } else {
        Ok(())
    }
}

fn require_schema(
    schema: &str,
    schema_version: u16,
    expected_schema: &'static str,
) -> Result<(), RadrootsKnowledgeValidationError> {
    if schema != expected_schema {
        return Err(RadrootsKnowledgeValidationError::InvalidField("schema"));
    }
    if schema_version != RADROOTS_KNOWLEDGE_SCHEMA_VERSION {
        return Err(RadrootsKnowledgeValidationError::InvalidField(
            "schema_version",
        ));
    }
    Ok(())
}

fn validate_event_id(
    value: &str,
    field: &'static str,
) -> Result<(), RadrootsKnowledgeValidationError> {
    if value.trim().is_empty() {
        return Err(RadrootsKnowledgeValidationError::EmptyField(field));
    }
    RadrootsEventId::parse(value)
        .map(|_| ())
        .map_err(|_| RadrootsKnowledgeValidationError::InvalidField(field))
}

fn validate_pubkey(
    value: &str,
    field: &'static str,
) -> Result<(), RadrootsKnowledgeValidationError> {
    if value.trim().is_empty() {
        return Err(RadrootsKnowledgeValidationError::EmptyField(field));
    }
    RadrootsPublicKey::parse(value)
        .map(|_| ())
        .map_err(|_| RadrootsKnowledgeValidationError::InvalidField(field))
}

fn validate_relays(
    relays: &[String],
    field: &'static str,
) -> Result<(), RadrootsKnowledgeValidationError> {
    for relay in relays {
        RadrootsRelayUrl::parse(relay)
            .map(|_| ())
            .map_err(|_| RadrootsKnowledgeValidationError::InvalidField(field))?;
    }
    Ok(())
}

fn validate_event_ref(
    event_ref: &RadrootsEventRef,
    field: &'static str,
) -> Result<(), RadrootsKnowledgeValidationError> {
    validate_event_id(&event_ref.id, field)?;
    validate_pubkey(&event_ref.author, field)?;
    if let Some(d_tag) = event_ref.d_tag.as_deref() {
        RadrootsDTag::parse(d_tag)
            .map(|_| ())
            .map_err(|_| RadrootsKnowledgeValidationError::InvalidField(field))?;
    }
    if let Some(relays) = event_ref.relays.as_deref() {
        validate_relays(relays, field)?;
    }
    Ok(())
}

fn validate_event_refs(
    refs: &[RadrootsEventRef],
    field: &'static str,
) -> Result<(), RadrootsKnowledgeValidationError> {
    for event_ref in refs {
        validate_event_ref(event_ref, field)?;
    }
    Ok(())
}

fn validate_address_ref(
    address: &RadrootsAddressableRef,
    field: &'static str,
) -> Result<(), RadrootsKnowledgeValidationError> {
    if address.kind == 0 {
        return Err(RadrootsKnowledgeValidationError::InvalidField(field));
    }
    validate_pubkey(&address.pubkey, field)?;
    RadrootsDTag::parse(address.d_tag.as_str())
        .map(|_| ())
        .map_err(|_| RadrootsKnowledgeValidationError::InvalidField(field))?;
    validate_relays(&address.relays, field)
}

fn validate_wiki_article_address_ref(
    address: &RadrootsAddressableRef,
    field: &'static str,
) -> Result<(), RadrootsKnowledgeValidationError> {
    if address.kind != KIND_WIKI_ARTICLE {
        return Err(RadrootsKnowledgeValidationError::InvalidField(field));
    }
    validate_pubkey(&address.pubkey, field)?;
    validate_wiki_d_tag(address.d_tag.as_str())
        .map(|_| ())
        .map_err(|_| RadrootsKnowledgeValidationError::InvalidField(field))?;
    validate_relays(&address.relays, field)
}

fn validate_wiki_article_version_ref(
    version_ref: &RadrootsWikiArticleVersionRef,
    field: &'static str,
) -> Result<(), RadrootsKnowledgeValidationError> {
    validate_event_id(&version_ref.event_id, field)?;
    validate_wiki_article_address_ref(&version_ref.address_ref, field)
}

fn validate_node_ref(
    node_ref: &RadrootsKnowledgeNodeRef,
    field: &'static str,
) -> Result<(), RadrootsKnowledgeValidationError> {
    let mut populated = 0u8;
    if let Some(event_ref) = &node_ref.event_ref {
        populated += 1;
        validate_event_ref(event_ref, field)?;
    }
    if let Some(address_ref) = &node_ref.address_ref {
        populated += 1;
        validate_address_ref(address_ref, field)?;
    }
    if let Some(external_id) = node_ref.external_id.as_deref() {
        populated += 1;
        require_non_empty(external_id, field)?;
    }
    if populated == 1 {
        Ok(())
    } else {
        Err(RadrootsKnowledgeValidationError::InvalidField(field))
    }
}

pub fn validate_wiki_article(
    article: &RadrootsWikiArticle,
) -> Result<(), RadrootsKnowledgeValidationError> {
    validate_wiki_d_tag(article.d_tag.as_str())
        .map(|_| ())
        .map_err(|_| RadrootsKnowledgeValidationError::InvalidField("d_tag"))?;
    if let Some(title) = article.title.as_deref() {
        require_non_empty(title, "title")?;
    }
    require_non_empty(article.content_djot.as_str(), "content_djot")?;
    validate_event_refs(&article.references, "references")?;
    for version_ref in &article.forked_from {
        validate_wiki_article_version_ref(version_ref, "forked_from")?;
    }
    if let Some(version_ref) = &article.deferred_to {
        validate_wiki_article_version_ref(version_ref, "deferred_to")?;
    }
    Ok(())
}

pub fn validate_wiki_redirect(
    redirect: &RadrootsWikiRedirect,
) -> Result<(), RadrootsKnowledgeValidationError> {
    validate_wiki_d_tag(redirect.d_tag.as_str())
        .map(|_| ())
        .map_err(|_| RadrootsKnowledgeValidationError::InvalidField("d_tag"))?;
    validate_wiki_article_address_ref(&redirect.target, "wiki_redirect.target")
}

pub fn validate_wiki_merge_request(
    request: &RadrootsWikiMergeRequest,
) -> Result<(), RadrootsKnowledgeValidationError> {
    validate_wiki_article_address_ref(&request.target_article, "target_article")?;
    validate_pubkey(&request.destination_pubkey, "destination_pubkey")?;
    if let Some(base) = request.base_version_event_id.as_deref() {
        validate_event_id(base, "base_version_event_id")?;
    }
    validate_event_id(
        request.source_version_event_id.as_str(),
        "source_version_event_id",
    )
}

pub fn validate_knowledge_source(
    source: &RadrootsKnowledgeSource,
) -> Result<(), RadrootsKnowledgeValidationError> {
    require_schema(
        source.schema.as_str(),
        source.schema_version,
        RADROOTS_KNOWLEDGE_SOURCE_SCHEMA,
    )?;
    validate_wiki_d_tag(source.d_tag.as_str())
        .map(|_| ())
        .map_err(|_| RadrootsKnowledgeValidationError::InvalidField("d_tag"))?;
    require_non_empty(source.title.as_str(), "title")?;
    require_non_empty(source.source_type.as_str(), "source_type")?;
    for author in &source.authors {
        require_non_empty(author.as_str(), "authors")?;
    }
    validate_event_refs(&source.artifact_refs, "artifact_refs")
}

pub fn is_uncited_knowledge_claim_type(claim_type: &str) -> bool {
    matches!(claim_type, "hypothesis" | "observation" | "question")
}

pub fn validate_knowledge_claim(
    claim: &RadrootsKnowledgeClaim,
) -> Result<(), RadrootsKnowledgeValidationError> {
    require_schema(
        claim.schema.as_str(),
        claim.schema_version,
        RADROOTS_KNOWLEDGE_CLAIM_SCHEMA,
    )?;
    require_non_empty(claim.claim_type.as_str(), "claim_type")?;
    require_non_empty(claim.text.as_str(), "text")?;
    if claim.citation_spans.is_empty()
        && !is_uncited_knowledge_claim_type(claim.claim_type.as_str())
    {
        return Err(RadrootsKnowledgeValidationError::EmptyField(
            "citation_spans",
        ));
    }
    for citation in &claim.citation_spans {
        if citation.source_ref.kind != crate::kinds::KIND_KNOWLEDGE_SOURCE {
            return Err(RadrootsKnowledgeValidationError::InvalidField(
                "citation_spans",
            ));
        }
        validate_event_ref(&citation.source_ref, "citation_spans")?;
        if let Some(artifact_ref) = &citation.artifact_ref {
            validate_event_ref(artifact_ref, "citation_spans")?;
        }
        if let Some(quote_hash) = citation.quote_hash.as_deref() {
            validate_event_id(quote_hash, "citation_spans")?;
        }
    }
    validate_event_refs(&claim.supersedes, "supersedes")
}

pub fn validate_knowledge_relation(
    relation: &RadrootsKnowledgeRelation,
) -> Result<(), RadrootsKnowledgeValidationError> {
    require_schema(
        relation.schema.as_str(),
        relation.schema_version,
        RADROOTS_KNOWLEDGE_RELATION_SCHEMA,
    )?;
    validate_node_ref(&relation.subject, "subject")?;
    require_non_empty(relation.predicate.as_str(), "predicate")?;
    validate_node_ref(&relation.object, "object")?;
    validate_event_refs(&relation.support_refs, "support_refs")?;
    validate_event_refs(&relation.supersedes, "supersedes")
}

pub fn validate_knowledge_review(
    review: &RadrootsKnowledgeReview,
) -> Result<(), RadrootsKnowledgeValidationError> {
    require_schema(
        review.schema.as_str(),
        review.schema_version,
        RADROOTS_KNOWLEDGE_REVIEW_SCHEMA,
    )?;
    validate_event_id(review.target.event_id.as_str(), "review_target")?;
    validate_pubkey(review.target.author_pubkey.as_str(), "review_target")?;
    if review.target.kind == 0 {
        return Err(RadrootsKnowledgeValidationError::InvalidField(
            "review_target",
        ));
    }
    if let Some(address) = review.target.address.as_deref() {
        RadrootsAddressableCoordinate::parse(address)
            .map(|_| ())
            .map_err(|_| RadrootsKnowledgeValidationError::InvalidField("review_target"))?;
    }
    validate_relays(&review.target.relays, "review_target")?;
    require_non_empty(review.reviewer_role.as_str(), "reviewer_role")?;
    require_non_empty(review.verdict.as_str(), "verdict")?;
    for score in &review.scores {
        require_non_empty(score.dimension.as_str(), "scores")?;
        require_non_empty(score.value.as_str(), "scores")?;
    }
    validate_event_refs(&review.evidence_refs, "evidence_refs")
}

pub fn validate_knowledge_field_report(
    report: &RadrootsKnowledgeFieldReport,
) -> Result<(), RadrootsKnowledgeValidationError> {
    require_schema(
        report.schema.as_str(),
        report.schema_version,
        RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA,
    )?;
    require_non_empty(report.report_type.as_str(), "report_type")?;
    require_non_empty(report.title.as_str(), "title")?;
    if report.context.location_precision
        == RadrootsKnowledgeLocationPrecision::ExactPrivateReference
        && report.context.private_location_ref.is_none()
    {
        return Err(RadrootsKnowledgeValidationError::EmptyField(
            "private_location_ref",
        ));
    }
    if let Some(private_location_ref) = &report.context.private_location_ref {
        validate_event_ref(private_location_ref, "private_location_ref")?;
    }
    if report.observations.is_empty() {
        return Err(RadrootsKnowledgeValidationError::EmptyField("observations"));
    }
    for observation in &report.observations {
        require_non_empty(observation.observation_type.as_str(), "observations")?;
        require_non_empty(observation.text.as_str(), "observations")?;
        for value in &observation.values {
            require_non_empty(value.key.as_str(), "observation_values")?;
            require_non_empty(value.value.as_str(), "observation_values")?;
        }
    }
    validate_event_refs(&report.artifact_refs, "artifact_refs")?;
    validate_event_refs(&report.related_refs, "related_refs")
}

pub fn validate_evidence_bounty(
    bounty: &RadrootsEvidenceBounty,
) -> Result<(), RadrootsKnowledgeValidationError> {
    require_schema(
        bounty.schema.as_str(),
        bounty.schema_version,
        RADROOTS_EVIDENCE_BOUNTY_SCHEMA,
    )?;
    validate_wiki_d_tag(bounty.d_tag.as_str())
        .map(|_| ())
        .map_err(|_| RadrootsKnowledgeValidationError::InvalidField("d_tag"))?;
    require_non_empty(bounty.title.as_str(), "title")?;
    if bounty.target_refs.is_empty() {
        return Err(RadrootsKnowledgeValidationError::EmptyField("target_refs"));
    }
    validate_event_refs(&bounty.target_refs, "target_refs")
}

pub fn validate_knowledge_change_proposal(
    proposal: &RadrootsKnowledgeChangeProposal,
) -> Result<(), RadrootsKnowledgeValidationError> {
    require_schema(
        proposal.schema.as_str(),
        proposal.schema_version,
        RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA,
    )?;
    validate_event_ref(&proposal.target, "target")?;
    require_non_empty(proposal.proposal_type.as_str(), "proposal_type")?;
    require_non_empty(proposal.summary.as_str(), "summary")?;
    validate_event_refs(&proposal.evidence_refs, "evidence_refs")?;
    validate_event_refs(&proposal.supersedes, "supersedes")
}

pub fn validate_contribution_attestation(
    attestation: &RadrootsContributionAttestation,
) -> Result<(), RadrootsKnowledgeValidationError> {
    require_schema(
        attestation.schema.as_str(),
        attestation.schema_version,
        RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA,
    )?;
    validate_pubkey(
        attestation.contributor_pubkey.as_str(),
        "contributor_pubkey",
    )?;
    require_non_empty(attestation.contribution_type.as_str(), "contribution_type")?;
    require_non_empty(attestation.summary.as_str(), "summary")?;
    if attestation.subject_refs.is_empty() {
        return Err(RadrootsKnowledgeValidationError::EmptyField("subject_refs"));
    }
    validate_event_refs(&attestation.subject_refs, "subject_refs")?;
    validate_event_refs(&attestation.evidence_refs, "evidence_refs")
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAddressableRef {
    pub kind: u32,
    pub pubkey: String,
    pub d_tag: String,
    pub relays: Vec<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRightsAssertion {
    pub assertion: String,
    pub holder: Option<String>,
    pub license: Option<String>,
    pub url: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsWikiArticle {
    pub d_tag: String,
    pub title: Option<String>,
    pub content_djot: String,
    pub summary: Option<String>,
    pub topics: Vec<String>,
    pub references: Vec<RadrootsEventRef>,
    pub forked_from: Vec<RadrootsWikiArticleVersionRef>,
    pub deferred_to: Option<RadrootsWikiArticleVersionRef>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsWikiArticleVersionRef {
    pub event_id: String,
    pub address_ref: RadrootsAddressableRef,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsWikiRedirect {
    pub d_tag: String,
    pub target: RadrootsAddressableRef,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
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

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
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
    pub artifact_refs: Vec<RadrootsEventRef>,
    pub author_asserted_rights: Option<RadrootsRightsAssertion>,
    pub topics: Vec<String>,
    pub summary: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
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
    pub supersedes: Vec<RadrootsEventRef>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeCitationSpan {
    pub source_ref: RadrootsEventRef,
    pub artifact_ref: Option<RadrootsEventRef>,
    pub page_start: Option<u32>,
    pub page_end: Option<u32>,
    pub section_path: Vec<String>,
    pub quote_hash: Option<String>,
    pub chunk_id: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeNodeRef {
    pub node_type: String,
    pub event_ref: Option<RadrootsEventRef>,
    pub address_ref: Option<RadrootsAddressableRef>,
    pub external_id: Option<String>,
    pub label: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeRelation {
    pub schema: String,
    pub schema_version: u16,
    pub subject: RadrootsKnowledgeNodeRef,
    pub predicate: String,
    pub object: RadrootsKnowledgeNodeRef,
    pub support_refs: Vec<RadrootsEventRef>,
    pub author_asserted_confidence: Option<String>,
    pub supersedes: Vec<RadrootsEventRef>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
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
    pub evidence_refs: Vec<RadrootsEventRef>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
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

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsKnowledgeReviewScope {
    SpecificVersion,
    AddressableCoordinateAtPublishedAt,
    PolicyLatest,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeReviewScore {
    pub dimension: String,
    pub value: String,
    pub note: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
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
    pub artifact_refs: Vec<RadrootsEventRef>,
    pub related_refs: Vec<RadrootsEventRef>,
    pub limitations: Vec<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeFieldContext {
    pub location_precision: RadrootsKnowledgeLocationPrecision,
    pub public_location: Option<RadrootsKnowledgeLocation>,
    pub private_location_ref: Option<RadrootsEventRef>,
    pub topics: Vec<String>,
    pub context_tags: Vec<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsKnowledgeLocationPrecision {
    None,
    Region,
    Locality,
    CoarseGeohash,
    ExactPublic,
    ExactPrivateReference,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeLocation {
    pub label: Option<String>,
    pub region: Option<String>,
    pub locality: Option<String>,
    pub geohash: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeObservation {
    pub observation_type: String,
    pub text: String,
    pub observed_at: Option<String>,
    pub values: Vec<RadrootsKnowledgeObservationValue>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeObservationValue {
    pub key: String,
    pub value: String,
    pub unit: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
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
    pub target_refs: Vec<RadrootsEventRef>,
    pub reward_note: Option<String>,
    pub closes_at: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsKnowledgeChangeProposal {
    pub schema: String,
    pub schema_version: u16,
    pub target: RadrootsEventRef,
    pub proposal_type: String,
    pub summary: String,
    pub rationale: Option<String>,
    pub evidence_refs: Vec<RadrootsEventRef>,
    pub supersedes: Vec<RadrootsEventRef>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsContributionAttestation {
    pub schema: String,
    pub schema_version: u16,
    pub contributor_pubkey: String,
    pub contribution_type: String,
    pub subject_refs: Vec<RadrootsEventRef>,
    pub summary: String,
    pub evidence_refs: Vec<RadrootsEventRef>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_64(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
    }

    fn event_ref() -> RadrootsEventRef {
        event_ref_with_kind(1)
    }

    fn event_ref_with_kind(kind: u32) -> RadrootsEventRef {
        RadrootsEventRef {
            id: "0".repeat(64),
            author: "1".repeat(64),
            kind,
            d_tag: None,
            relays: None,
        }
    }

    fn article_address_ref() -> RadrootsAddressableRef {
        RadrootsAddressableRef {
            kind: KIND_WIKI_ARTICLE,
            pubkey: hex_64('a'),
            d_tag: "soil-health".to_string(),
            relays: Vec::new(),
        }
    }

    fn article_version_ref() -> RadrootsWikiArticleVersionRef {
        RadrootsWikiArticleVersionRef {
            event_id: hex_64('b'),
            address_ref: article_address_ref(),
        }
    }

    fn wiki_article() -> RadrootsWikiArticle {
        RadrootsWikiArticle {
            d_tag: "soil-health".to_string(),
            title: Some("Soil health".to_string()),
            content_djot: "# Soil health".to_string(),
            summary: None,
            topics: Vec::new(),
            references: vec![event_ref()],
            forked_from: vec![article_version_ref()],
            deferred_to: Some(article_version_ref()),
        }
    }

    fn invalid_relay() -> String {
        "http://relay.radroots.example".to_string()
    }

    fn knowledge_source() -> RadrootsKnowledgeSource {
        RadrootsKnowledgeSource {
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
            artifact_refs: vec![event_ref()],
            author_asserted_rights: None,
            topics: vec!["soil".to_string()],
            summary: None,
        }
    }

    fn citation_span() -> RadrootsKnowledgeCitationSpan {
        RadrootsKnowledgeCitationSpan {
            source_ref: event_ref_with_kind(crate::kinds::KIND_KNOWLEDGE_SOURCE),
            artifact_ref: Some(event_ref()),
            page_start: None,
            page_end: None,
            section_path: Vec::new(),
            quote_hash: Some(hex_64('c')),
            chunk_id: None,
        }
    }

    fn knowledge_claim() -> RadrootsKnowledgeClaim {
        RadrootsKnowledgeClaim {
            schema: RADROOTS_KNOWLEDGE_CLAIM_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            claim_type: "practice_effect".to_string(),
            text: "Cover crops improve soil structure.".to_string(),
            citation_spans: vec![citation_span()],
            topics: Vec::new(),
            applies_to: Vec::new(),
            author_asserted_confidence: None,
            supersedes: vec![event_ref()],
        }
    }

    fn node_ref() -> RadrootsKnowledgeNodeRef {
        RadrootsKnowledgeNodeRef {
            node_type: "event".to_string(),
            event_ref: Some(event_ref()),
            address_ref: None,
            external_id: None,
            label: None,
        }
    }

    fn knowledge_relation() -> RadrootsKnowledgeRelation {
        RadrootsKnowledgeRelation {
            schema: RADROOTS_KNOWLEDGE_RELATION_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            subject: node_ref(),
            predicate: "supports".to_string(),
            object: node_ref(),
            support_refs: vec![event_ref()],
            author_asserted_confidence: None,
            supersedes: Vec::new(),
        }
    }

    fn knowledge_review() -> RadrootsKnowledgeReview {
        RadrootsKnowledgeReview {
            schema: RADROOTS_KNOWLEDGE_REVIEW_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            target: RadrootsKnowledgeReviewTarget {
                event_id: hex_64('d'),
                author_pubkey: hex_64('a'),
                kind: crate::kinds::KIND_KNOWLEDGE_CLAIM,
                address: None,
                relays: Vec::new(),
                review_scope: RadrootsKnowledgeReviewScope::SpecificVersion,
            },
            reviewer_role: "peer".to_string(),
            verdict: "needs_more_evidence".to_string(),
            scores: vec![RadrootsKnowledgeReviewScore {
                dimension: "evidence".to_string(),
                value: "partial".to_string(),
                note: None,
            }],
            notes: None,
            evidence_refs: vec![event_ref()],
        }
    }

    fn field_report() -> RadrootsKnowledgeFieldReport {
        RadrootsKnowledgeFieldReport {
            schema: RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            report_type: "observation".to_string(),
            title: "Field observation".to_string(),
            summary: None,
            context: RadrootsKnowledgeFieldContext {
                location_precision: RadrootsKnowledgeLocationPrecision::CoarseGeohash,
                public_location: None,
                private_location_ref: None,
                topics: Vec::new(),
                context_tags: Vec::new(),
            },
            observations: vec![RadrootsKnowledgeObservation {
                observation_type: "residue".to_string(),
                text: "Residue was visible.".to_string(),
                observed_at: None,
                values: vec![RadrootsKnowledgeObservationValue {
                    key: "coverage".to_string(),
                    value: "medium".to_string(),
                    unit: None,
                }],
            }],
            artifact_refs: vec![event_ref()],
            related_refs: vec![event_ref()],
            limitations: Vec::new(),
        }
    }

    fn evidence_bounty() -> RadrootsEvidenceBounty {
        RadrootsEvidenceBounty {
            schema: RADROOTS_EVIDENCE_BOUNTY_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            d_tag: "soil-bounty".to_string(),
            title: "Soil bounty".to_string(),
            summary: None,
            topics: Vec::new(),
            target_refs: vec![event_ref()],
            reward_note: None,
            closes_at: None,
        }
    }

    fn knowledge_change_proposal() -> RadrootsKnowledgeChangeProposal {
        RadrootsKnowledgeChangeProposal {
            schema: RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            target: event_ref(),
            proposal_type: "amend".to_string(),
            summary: "Clarify scope".to_string(),
            rationale: None,
            evidence_refs: vec![event_ref()],
            supersedes: Vec::new(),
        }
    }

    fn contribution_attestation() -> RadrootsContributionAttestation {
        RadrootsContributionAttestation {
            schema: RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            contributor_pubkey: hex_64('a'),
            contribution_type: "review".to_string(),
            subject_refs: vec![event_ref()],
            summary: "Reviewed claim".to_string(),
            evidence_refs: vec![event_ref()],
        }
    }

    fn assert_validation_error(
        result: Result<(), RadrootsKnowledgeValidationError>,
        expected: RadrootsKnowledgeValidationError,
    ) {
        assert_eq!(result, Err(expected));
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
            title: Some("Soil health".to_string()),
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

    #[test]
    fn knowledge_validators_accept_valid_models() {
        assert_eq!(validate_wiki_article(&wiki_article()), Ok(()));
        assert_eq!(
            validate_wiki_redirect(&RadrootsWikiRedirect {
                d_tag: "soil".to_string(),
                target: article_address_ref(),
            }),
            Ok(())
        );
        assert_eq!(
            validate_wiki_merge_request(&RadrootsWikiMergeRequest {
                target_article: article_address_ref(),
                destination_pubkey: hex_64('a'),
                base_version_event_id: Some(hex_64('e')),
                source_version_event_id: hex_64('f'),
                explanation: None,
            }),
            Ok(())
        );
        assert_eq!(validate_knowledge_source(&knowledge_source()), Ok(()));
        assert_eq!(validate_knowledge_claim(&knowledge_claim()), Ok(()));
        assert_eq!(validate_knowledge_relation(&knowledge_relation()), Ok(()));
        assert_eq!(validate_knowledge_review(&knowledge_review()), Ok(()));
        assert_eq!(validate_knowledge_field_report(&field_report()), Ok(()));
        assert_eq!(validate_evidence_bounty(&evidence_bounty()), Ok(()));
        assert_eq!(
            validate_knowledge_change_proposal(&knowledge_change_proposal()),
            Ok(())
        );
        assert_eq!(
            validate_contribution_attestation(&contribution_attestation()),
            Ok(())
        );
    }

    #[test]
    fn knowledge_validators_reject_noncanonical_relay_values() {
        let mut article = wiki_article();
        article.references[0].relays = Some(vec![invalid_relay()]);
        assert_validation_error(
            validate_wiki_article(&article),
            RadrootsKnowledgeValidationError::InvalidField("references"),
        );

        let mut article = wiki_article();
        article.forked_from[0].address_ref.relays = vec![invalid_relay()];
        assert_validation_error(
            validate_wiki_article(&article),
            RadrootsKnowledgeValidationError::InvalidField("forked_from"),
        );

        let mut article = wiki_article();
        article
            .deferred_to
            .as_mut()
            .expect("deferred")
            .address_ref
            .relays = vec![invalid_relay()];
        assert_validation_error(
            validate_wiki_article(&article),
            RadrootsKnowledgeValidationError::InvalidField("deferred_to"),
        );

        let mut redirect = RadrootsWikiRedirect {
            d_tag: "soil".to_string(),
            target: article_address_ref(),
        };
        redirect.target.relays = vec![invalid_relay()];
        assert_validation_error(
            validate_wiki_redirect(&redirect),
            RadrootsKnowledgeValidationError::InvalidField("wiki_redirect.target"),
        );

        let mut merge = RadrootsWikiMergeRequest {
            target_article: article_address_ref(),
            destination_pubkey: hex_64('a'),
            base_version_event_id: Some(hex_64('e')),
            source_version_event_id: hex_64('f'),
            explanation: None,
        };
        merge.target_article.relays = vec![invalid_relay()];
        assert_validation_error(
            validate_wiki_merge_request(&merge),
            RadrootsKnowledgeValidationError::InvalidField("target_article"),
        );

        let mut source = knowledge_source();
        source.artifact_refs[0].relays = Some(vec![invalid_relay()]);
        assert_validation_error(
            validate_knowledge_source(&source),
            RadrootsKnowledgeValidationError::InvalidField("artifact_refs"),
        );

        let mut claim = knowledge_claim();
        claim.citation_spans[0].source_ref.relays = Some(vec![invalid_relay()]);
        assert_validation_error(
            validate_knowledge_claim(&claim),
            RadrootsKnowledgeValidationError::InvalidField("citation_spans"),
        );

        let mut relation = knowledge_relation();
        relation.support_refs[0].relays = Some(vec![invalid_relay()]);
        assert_validation_error(
            validate_knowledge_relation(&relation),
            RadrootsKnowledgeValidationError::InvalidField("support_refs"),
        );

        let mut review = knowledge_review();
        review.target.relays = vec![invalid_relay()];
        assert_validation_error(
            validate_knowledge_review(&review),
            RadrootsKnowledgeValidationError::InvalidField("review_target"),
        );

        let mut report = field_report();
        report.context.location_precision =
            RadrootsKnowledgeLocationPrecision::ExactPrivateReference;
        let mut private_location_ref = event_ref();
        private_location_ref.relays = Some(vec![invalid_relay()]);
        report.context.private_location_ref = Some(private_location_ref);
        assert_validation_error(
            validate_knowledge_field_report(&report),
            RadrootsKnowledgeValidationError::InvalidField("private_location_ref"),
        );

        let mut bounty = evidence_bounty();
        bounty.target_refs[0].relays = Some(vec![invalid_relay()]);
        assert_validation_error(
            validate_evidence_bounty(&bounty),
            RadrootsKnowledgeValidationError::InvalidField("target_refs"),
        );

        let mut proposal = knowledge_change_proposal();
        proposal.target.relays = Some(vec![invalid_relay()]);
        assert_validation_error(
            validate_knowledge_change_proposal(&proposal),
            RadrootsKnowledgeValidationError::InvalidField("target"),
        );

        let mut attestation = contribution_attestation();
        attestation.subject_refs[0].relays = Some(vec![invalid_relay()]);
        assert_validation_error(
            validate_contribution_attestation(&attestation),
            RadrootsKnowledgeValidationError::InvalidField("subject_refs"),
        );
    }

    #[test]
    fn wiki_article_title_is_optional_but_not_blank() {
        let mut article = RadrootsWikiArticle {
            d_tag: "soil-health".to_string(),
            title: None,
            content_djot: "# Soil health".to_string(),
            summary: None,
            topics: Vec::new(),
            references: Vec::new(),
            forked_from: Vec::new(),
            deferred_to: None,
        };
        assert_eq!(validate_wiki_article(&article), Ok(()));

        article.title = Some(" ".to_string());
        assert_validation_error(
            validate_wiki_article(&article),
            RadrootsKnowledgeValidationError::EmptyField("title"),
        );
    }

    #[test]
    fn knowledge_claims_require_citations_except_exact_uncited_types() {
        let mut claim = knowledge_claim();
        claim.citation_spans.clear();
        assert_validation_error(
            validate_knowledge_claim(&claim),
            RadrootsKnowledgeValidationError::EmptyField("citation_spans"),
        );

        claim.citation_spans.push(citation_span());
        assert_eq!(validate_knowledge_claim(&claim), Ok(()));

        for claim_type in ["hypothesis", "observation", "question"] {
            let mut uncited = knowledge_claim();
            uncited.claim_type = claim_type.to_string();
            uncited.citation_spans.clear();
            assert_eq!(validate_knowledge_claim(&uncited), Ok(()));
            assert!(is_uncited_knowledge_claim_type(claim_type));
        }

        let mut capitalized = knowledge_claim();
        capitalized.claim_type = "Hypothesis".to_string();
        capitalized.citation_spans.clear();
        assert_validation_error(
            validate_knowledge_claim(&capitalized),
            RadrootsKnowledgeValidationError::EmptyField("citation_spans"),
        );
        assert!(!is_uncited_knowledge_claim_type("Hypothesis"));
    }

    #[test]
    fn knowledge_validators_reject_representative_invalid_models() {
        let mut article = RadrootsWikiArticle {
            d_tag: "soil-health".to_string(),
            title: Some("Soil health".to_string()),
            content_djot: " ".to_string(),
            summary: None,
            topics: Vec::new(),
            references: Vec::new(),
            forked_from: Vec::new(),
            deferred_to: None,
        };
        assert_validation_error(
            validate_wiki_article(&article),
            RadrootsKnowledgeValidationError::EmptyField("content_djot"),
        );
        article.content_djot = "# Soil health".to_string();
        article.forked_from.push(RadrootsWikiArticleVersionRef {
            event_id: "bad".to_string(),
            address_ref: article_address_ref(),
        });
        assert_validation_error(
            validate_wiki_article(&article),
            RadrootsKnowledgeValidationError::InvalidField("forked_from"),
        );

        let mut redirect = RadrootsWikiRedirect {
            d_tag: "soil".to_string(),
            target: article_address_ref(),
        };
        redirect.target.kind = 30023;
        assert_validation_error(
            validate_wiki_redirect(&redirect),
            RadrootsKnowledgeValidationError::InvalidField("wiki_redirect.target"),
        );

        let mut merge = RadrootsWikiMergeRequest {
            target_article: article_address_ref(),
            destination_pubkey: "bad".to_string(),
            base_version_event_id: None,
            source_version_event_id: hex_64('f'),
            explanation: None,
        };
        assert_validation_error(
            validate_wiki_merge_request(&merge),
            RadrootsKnowledgeValidationError::InvalidField("destination_pubkey"),
        );
        merge.destination_pubkey = hex_64('a');
        merge.source_version_event_id = String::new();
        assert_validation_error(
            validate_wiki_merge_request(&merge),
            RadrootsKnowledgeValidationError::EmptyField("source_version_event_id"),
        );

        let mut source = knowledge_source();
        source.authors.push(" ".to_string());
        assert_validation_error(
            validate_knowledge_source(&source),
            RadrootsKnowledgeValidationError::EmptyField("authors"),
        );

        let mut claim = knowledge_claim();
        claim.citation_spans[0].quote_hash = Some("bad".to_string());
        assert_validation_error(
            validate_knowledge_claim(&claim),
            RadrootsKnowledgeValidationError::InvalidField("citation_spans"),
        );

        let mut relation = knowledge_relation();
        relation.subject.external_id = Some("external".to_string());
        assert_validation_error(
            validate_knowledge_relation(&relation),
            RadrootsKnowledgeValidationError::InvalidField("subject"),
        );

        let mut review = knowledge_review();
        review.target.kind = 0;
        assert_validation_error(
            validate_knowledge_review(&review),
            RadrootsKnowledgeValidationError::InvalidField("review_target"),
        );

        let mut report = field_report();
        report.context.location_precision =
            RadrootsKnowledgeLocationPrecision::ExactPrivateReference;
        assert_validation_error(
            validate_knowledge_field_report(&report),
            RadrootsKnowledgeValidationError::EmptyField("private_location_ref"),
        );
        report.context.location_precision = RadrootsKnowledgeLocationPrecision::CoarseGeohash;
        report.observations[0].values[0].value = String::new();
        assert_validation_error(
            validate_knowledge_field_report(&report),
            RadrootsKnowledgeValidationError::EmptyField("observation_values"),
        );

        let mut bounty = evidence_bounty();
        bounty.target_refs.clear();
        assert_validation_error(
            validate_evidence_bounty(&bounty),
            RadrootsKnowledgeValidationError::EmptyField("target_refs"),
        );

        let mut proposal = knowledge_change_proposal();
        proposal.summary = " ".to_string();
        assert_validation_error(
            validate_knowledge_change_proposal(&proposal),
            RadrootsKnowledgeValidationError::EmptyField("summary"),
        );

        let mut attestation = contribution_attestation();
        attestation.contributor_pubkey = "bad".to_string();
        assert_validation_error(
            validate_contribution_attestation(&attestation),
            RadrootsKnowledgeValidationError::InvalidField("contributor_pubkey"),
        );
    }
}
