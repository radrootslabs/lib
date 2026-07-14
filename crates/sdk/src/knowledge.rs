#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
#[cfg(feature = "std")]
use std::{
    string::{String, ToString},
    vec::Vec,
};

use core::fmt;

pub use radroots_event::wire::RadrootsNip01EventWireParts;
pub use radroots_event::{
    RadrootsEventEnvelope, RadrootsEventRef,
    draft::{RadrootsDraftError, RadrootsEventDraft},
    kinds::{
        KIND_FILE_METADATA, KIND_KNOWLEDGE_CLAIM, KIND_KNOWLEDGE_FIELD_REPORT,
        KIND_KNOWLEDGE_RELATION, KIND_KNOWLEDGE_REVIEW, KIND_KNOWLEDGE_SOURCE, KIND_WIKI_ARTICLE,
        KIND_WIKI_MERGE_REQUEST, KIND_WIKI_REDIRECT,
    },
    knowledge::{
        RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA, RADROOTS_EVIDENCE_BOUNTY_SCHEMA,
        RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA, RADROOTS_KNOWLEDGE_CLAIM_SCHEMA,
        RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA, RADROOTS_KNOWLEDGE_RELATION_SCHEMA,
        RADROOTS_KNOWLEDGE_REVIEW_SCHEMA, RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        RADROOTS_KNOWLEDGE_SOURCE_SCHEMA, RADROOTS_WIKI_D_TAG_MAX_LEN, RadrootsAddressableRef,
        RadrootsContributionAttestation, RadrootsEvidenceBounty, RadrootsKnowledgeChangeProposal,
        RadrootsKnowledgeCitationSpan, RadrootsKnowledgeClaim, RadrootsKnowledgeFieldContext,
        RadrootsKnowledgeFieldReport, RadrootsKnowledgeLocation,
        RadrootsKnowledgeLocationPrecision, RadrootsKnowledgeNodeRef, RadrootsKnowledgeObservation,
        RadrootsKnowledgeObservationValue, RadrootsKnowledgeRelation, RadrootsKnowledgeReview,
        RadrootsKnowledgeReviewScope, RadrootsKnowledgeReviewScore, RadrootsKnowledgeReviewTarget,
        RadrootsKnowledgeSource, RadrootsKnowledgeValidationError, RadrootsRightsAssertion,
        RadrootsWikiArticle, RadrootsWikiArticleVersionRef, RadrootsWikiDTagError,
        RadrootsWikiMergeRequest, RadrootsWikiRedirect, normalize_wiki_d_tag,
        validate_knowledge_claim, validate_wiki_article, validate_wiki_d_tag,
    },
};
pub use radroots_event_codec::{
    RADROOTS_KNOWLEDGE_CONTRACT_MANIFEST_SCHEMA_VERSION,
    error::RadrootsEncodeError,
    manifest::{
        RadrootsKnowledgeContractManifest, RadrootsKnowledgeContractManifestEntry,
        RadrootsKnowledgeManifestCodecSupport, RadrootsKnowledgeManifestDiscriminator,
        RadrootsKnowledgeManifestTagContract,
    },
    verification::{
        RadrootsContractValidatedEvent, RadrootsDecodeError, RadrootsDecodedEvent,
        RadrootsIdVerifiedEvent, RadrootsNip01VerificationError, RadrootsSignatureVerifiedEvent,
    },
};

use radroots_event::knowledge::{
    validate_knowledge_field_report, validate_knowledge_relation, validate_knowledge_review,
    validate_knowledge_source, validate_wiki_merge_request, validate_wiki_redirect,
};
use radroots_event_codec::{
    contract_manifest_json as codec_contract_manifest_json,
    contract_manifest_sha256 as codec_contract_manifest_sha256,
    knowledge::{
        knowledge_claim_to_wire_parts, knowledge_field_report_to_wire_parts,
        knowledge_relation_to_wire_parts, knowledge_review_to_wire_parts,
        knowledge_source_to_wire_parts, wiki_article_to_wire_parts,
        wiki_merge_request_to_wire_parts, wiki_redirect_to_wire_parts,
    },
    knowledge_contract_manifest, verify_and_decode_radroots_event as codec_verify_and_decode,
};

pub const WIKI_ARTICLE_CONTRACT_ID: &str = "radroots.wiki.article.v1";
pub const WIKI_REDIRECT_CONTRACT_ID: &str = "radroots.wiki.redirect.v1";
pub const WIKI_MERGE_REQUEST_CONTRACT_ID: &str = "radroots.wiki.merge_request.v1";
pub const KNOWLEDGE_SOURCE_CONTRACT_ID: &str = RADROOTS_KNOWLEDGE_SOURCE_SCHEMA;
pub const KNOWLEDGE_CLAIM_CONTRACT_ID: &str = RADROOTS_KNOWLEDGE_CLAIM_SCHEMA;
pub const KNOWLEDGE_RELATION_CONTRACT_ID: &str = RADROOTS_KNOWLEDGE_RELATION_SCHEMA;
pub const KNOWLEDGE_REVIEW_CONTRACT_ID: &str = RADROOTS_KNOWLEDGE_REVIEW_SCHEMA;
pub const KNOWLEDGE_FIELD_REPORT_CONTRACT_ID: &str = RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA;

#[derive(Debug)]
pub enum RadrootsSdkKnowledgeError {
    Builder(RadrootsKnowledgeBuilderError),
    Encode(RadrootsEncodeError),
    Draft(RadrootsDraftError),
    Decode(RadrootsDecodeError),
    Manifest(serde_json::Error),
}

impl RadrootsSdkKnowledgeError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Builder(_) => "knowledge_builder",
            Self::Encode(_) => "knowledge_encode",
            Self::Draft(_) => "knowledge_draft",
            Self::Decode(_) => "knowledge_decode",
            Self::Manifest(_) => "knowledge_manifest",
        }
    }

    pub fn inner_code(&self) -> &'static str {
        match self {
            Self::Builder(error) => error.code(),
            Self::Encode(error) => error.code(),
            Self::Draft(error) => draft_error_code(error),
            Self::Decode(error) => error.code(),
            Self::Manifest(_) => "json",
        }
    }
}

impl fmt::Display for RadrootsSdkKnowledgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Builder(_) => formatter.write_str("knowledge event builder failed"),
            Self::Encode(_) => formatter.write_str("knowledge event encoding failed"),
            Self::Draft(_) => formatter.write_str("knowledge event draft preparation failed"),
            Self::Decode(_) => formatter.write_str("knowledge event verification or decode failed"),
            Self::Manifest(_) => {
                formatter.write_str("knowledge contract manifest rendering failed")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSdkKnowledgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Builder(error) => Some(error),
            Self::Encode(error) => Some(error),
            Self::Draft(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Manifest(error) => Some(error),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsKnowledgeBuilderError {
    MissingField(&'static str),
    InvalidField(&'static str),
}

impl RadrootsKnowledgeBuilderError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MissingField(_) => "missing_field",
            Self::InvalidField(_) => "invalid_field",
        }
    }

    pub const fn field(&self) -> &'static str {
        match self {
            Self::MissingField(field) | Self::InvalidField(field) => field,
        }
    }
}

impl fmt::Display for RadrootsKnowledgeBuilderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(field) => write!(
                formatter,
                "knowledge builder missing required field {field}"
            ),
            Self::InvalidField(field) => {
                write!(formatter, "knowledge builder invalid field {field}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsKnowledgeBuilderError {}

impl From<RadrootsKnowledgeBuilderError> for RadrootsSdkKnowledgeError {
    fn from(value: RadrootsKnowledgeBuilderError) -> Self {
        Self::Builder(value)
    }
}

impl From<RadrootsEncodeError> for RadrootsSdkKnowledgeError {
    fn from(value: RadrootsEncodeError) -> Self {
        Self::Encode(value)
    }
}

impl From<RadrootsDraftError> for RadrootsSdkKnowledgeError {
    fn from(value: RadrootsDraftError) -> Self {
        Self::Draft(value)
    }
}

impl From<RadrootsDecodeError> for RadrootsSdkKnowledgeError {
    fn from(value: RadrootsDecodeError) -> Self {
        Self::Decode(value)
    }
}

impl From<serde_json::Error> for RadrootsSdkKnowledgeError {
    fn from(value: serde_json::Error) -> Self {
        Self::Manifest(value)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsWikiArticleBuilder {
    d_tag: String,
    title: Option<String>,
    content_djot: Option<String>,
    summary: Option<String>,
    topics: Vec<String>,
    references: Vec<RadrootsEventRef>,
    forked_from: Vec<RadrootsWikiArticleVersionRef>,
    deferred_to: Option<RadrootsWikiArticleVersionRef>,
}

impl RadrootsWikiArticleBuilder {
    pub fn new(d_tag: impl Into<String>) -> Self {
        Self {
            d_tag: d_tag.into(),
            ..Self::default()
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn content_djot(mut self, content_djot: impl Into<String>) -> Self {
        self.content_djot = Some(content_djot.into());
        self
    }

    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.topics.push(topic.into());
        self
    }

    pub fn reference(mut self, reference: RadrootsEventRef) -> Self {
        self.references.push(reference);
        self
    }

    pub fn forked_from(mut self, version_ref: RadrootsWikiArticleVersionRef) -> Self {
        self.forked_from.push(version_ref);
        self
    }

    pub fn deferred_to(mut self, version_ref: RadrootsWikiArticleVersionRef) -> Self {
        self.deferred_to = Some(version_ref);
        self
    }

    pub fn build(self) -> Result<RadrootsWikiArticle, RadrootsKnowledgeBuilderError> {
        let article = RadrootsWikiArticle {
            d_tag: self.d_tag,
            title: self.title,
            content_djot: builder_required_string(self.content_djot, "content_djot")?,
            summary: self.summary,
            topics: self.topics,
            references: self.references,
            forked_from: self.forked_from,
            deferred_to: self.deferred_to,
        };
        builder_validated(article, validate_wiki_article)
    }

    pub fn build_event(self) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        Ok(build_wiki_article_event(&self.build()?)?)
    }

    pub fn build_draft(
        self,
        expected_pubkey: impl AsRef<str>,
        created_at: u32,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        Ok(prepare_wiki_article_draft(
            &self.build()?,
            expected_pubkey,
            created_at,
        )?)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsWikiRedirectBuilder {
    d_tag: String,
    target: Option<RadrootsAddressableRef>,
}

impl RadrootsWikiRedirectBuilder {
    pub fn new(d_tag: impl Into<String>) -> Self {
        Self {
            d_tag: d_tag.into(),
            target: None,
        }
    }

    pub fn target(mut self, target: RadrootsAddressableRef) -> Self {
        self.target = Some(target);
        self
    }

    pub fn build(self) -> Result<RadrootsWikiRedirect, RadrootsKnowledgeBuilderError> {
        let redirect = RadrootsWikiRedirect {
            d_tag: self.d_tag,
            target: builder_required(self.target, "target")?,
        };
        builder_validated(redirect, validate_wiki_redirect)
    }

    pub fn build_event(self) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        Ok(build_wiki_redirect_event(&self.build()?)?)
    }

    pub fn build_draft(
        self,
        expected_pubkey: impl AsRef<str>,
        created_at: u32,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        Ok(prepare_wiki_redirect_draft(
            &self.build()?,
            expected_pubkey,
            created_at,
        )?)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsWikiMergeRequestBuilder {
    target_article: Option<RadrootsAddressableRef>,
    destination_pubkey: Option<String>,
    base_version_event_id: Option<String>,
    source_version_event_id: Option<String>,
    explanation: Option<String>,
}

impl RadrootsWikiMergeRequestBuilder {
    pub fn new() -> Self {
        Self {
            target_article: None,
            destination_pubkey: None,
            base_version_event_id: None,
            source_version_event_id: None,
            explanation: None,
        }
    }

    pub fn target_article(mut self, target_article: RadrootsAddressableRef) -> Self {
        self.target_article = Some(target_article);
        self
    }

    pub fn destination_pubkey(mut self, destination_pubkey: impl Into<String>) -> Self {
        self.destination_pubkey = Some(destination_pubkey.into());
        self
    }

    pub fn base_version_event_id(mut self, base_version_event_id: impl Into<String>) -> Self {
        self.base_version_event_id = Some(base_version_event_id.into());
        self
    }

    pub fn source_version_event_id(mut self, source_version_event_id: impl Into<String>) -> Self {
        self.source_version_event_id = Some(source_version_event_id.into());
        self
    }

    pub fn explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = Some(explanation.into());
        self
    }

    pub fn build(self) -> Result<RadrootsWikiMergeRequest, RadrootsKnowledgeBuilderError> {
        let request = RadrootsWikiMergeRequest {
            target_article: builder_required(self.target_article, "target_article")?,
            destination_pubkey: builder_required_string(
                self.destination_pubkey,
                "destination_pubkey",
            )?,
            base_version_event_id: self.base_version_event_id,
            source_version_event_id: builder_required_string(
                self.source_version_event_id,
                "source_version_event_id",
            )?,
            explanation: self.explanation,
        };
        builder_validated(request, validate_wiki_merge_request)
    }

    pub fn build_event(self) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        Ok(build_wiki_merge_request_event(&self.build()?)?)
    }

    pub fn build_draft(
        self,
        expected_pubkey: impl AsRef<str>,
        created_at: u32,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        Ok(prepare_wiki_merge_request_draft(
            &self.build()?,
            expected_pubkey,
            created_at,
        )?)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsKnowledgeSourceBuilder {
    d_tag: String,
    title: Option<String>,
    source_type: Option<String>,
    authors: Vec<String>,
    publisher: Option<String>,
    publication_year: Option<u16>,
    edition: Option<String>,
    canonical_url: Option<String>,
    artifact_refs: Vec<RadrootsEventRef>,
    author_asserted_rights: Option<RadrootsRightsAssertion>,
    topics: Vec<String>,
    summary: Option<String>,
}

impl RadrootsKnowledgeSourceBuilder {
    pub fn new(d_tag: impl Into<String>) -> Self {
        Self {
            d_tag: d_tag.into(),
            ..Self::default()
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn source_type(mut self, source_type: impl Into<String>) -> Self {
        self.source_type = Some(source_type.into());
        self
    }

    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.authors.push(author.into());
        self
    }

    pub fn publisher(mut self, publisher: impl Into<String>) -> Self {
        self.publisher = Some(publisher.into());
        self
    }

    pub fn publication_year(mut self, publication_year: u16) -> Self {
        self.publication_year = Some(publication_year);
        self
    }

    pub fn edition(mut self, edition: impl Into<String>) -> Self {
        self.edition = Some(edition.into());
        self
    }

    pub fn canonical_url(mut self, canonical_url: impl Into<String>) -> Self {
        self.canonical_url = Some(canonical_url.into());
        self
    }

    pub fn artifact_ref(mut self, artifact_ref: RadrootsEventRef) -> Self {
        self.artifact_refs.push(artifact_ref);
        self
    }

    pub fn author_asserted_rights(mut self, rights: RadrootsRightsAssertion) -> Self {
        self.author_asserted_rights = Some(rights);
        self
    }

    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.topics.push(topic.into());
        self
    }

    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    pub fn build(self) -> Result<RadrootsKnowledgeSource, RadrootsKnowledgeBuilderError> {
        let source = RadrootsKnowledgeSource {
            schema: RADROOTS_KNOWLEDGE_SOURCE_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            d_tag: self.d_tag,
            title: builder_required_string(self.title, "title")?,
            source_type: builder_required_string(self.source_type, "source_type")?,
            authors: self.authors,
            publisher: self.publisher,
            publication_year: self.publication_year,
            edition: self.edition,
            canonical_url: self.canonical_url,
            artifact_refs: self.artifact_refs,
            author_asserted_rights: self.author_asserted_rights,
            topics: self.topics,
            summary: self.summary,
        };
        builder_validated(source, validate_knowledge_source)
    }

    pub fn build_event(self) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        Ok(build_knowledge_source_event(&self.build()?)?)
    }

    pub fn build_draft(
        self,
        expected_pubkey: impl AsRef<str>,
        created_at: u32,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        Ok(prepare_knowledge_source_draft(
            &self.build()?,
            expected_pubkey,
            created_at,
        )?)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsKnowledgeClaimBuilder {
    claim_type: Option<String>,
    text: Option<String>,
    citation_spans: Vec<RadrootsKnowledgeCitationSpan>,
    topics: Vec<String>,
    applies_to: Vec<String>,
    author_asserted_confidence: Option<String>,
    supersedes: Vec<RadrootsEventRef>,
}

impl RadrootsKnowledgeClaimBuilder {
    pub fn new() -> Self {
        Self {
            claim_type: None,
            text: None,
            citation_spans: Vec::new(),
            topics: Vec::new(),
            applies_to: Vec::new(),
            author_asserted_confidence: None,
            supersedes: Vec::new(),
        }
    }

    pub fn claim_type(mut self, claim_type: impl Into<String>) -> Self {
        self.claim_type = Some(claim_type.into());
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn citation_span(mut self, citation_span: RadrootsKnowledgeCitationSpan) -> Self {
        self.citation_spans.push(citation_span);
        self
    }

    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.topics.push(topic.into());
        self
    }

    pub fn applies_to(mut self, applies_to: impl Into<String>) -> Self {
        self.applies_to.push(applies_to.into());
        self
    }

    pub fn author_asserted_confidence(
        mut self,
        author_asserted_confidence: impl Into<String>,
    ) -> Self {
        self.author_asserted_confidence = Some(author_asserted_confidence.into());
        self
    }

    pub fn supersedes(mut self, supersedes: RadrootsEventRef) -> Self {
        self.supersedes.push(supersedes);
        self
    }

    pub fn build(self) -> Result<RadrootsKnowledgeClaim, RadrootsKnowledgeBuilderError> {
        let claim = RadrootsKnowledgeClaim {
            schema: RADROOTS_KNOWLEDGE_CLAIM_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            claim_type: builder_required_string(self.claim_type, "claim_type")?,
            text: builder_required_string(self.text, "text")?,
            citation_spans: self.citation_spans,
            topics: self.topics,
            applies_to: self.applies_to,
            author_asserted_confidence: self.author_asserted_confidence,
            supersedes: self.supersedes,
        };
        builder_validated(claim, validate_knowledge_claim)
    }

    pub fn build_event(self) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        Ok(build_knowledge_claim_event(&self.build()?)?)
    }

    pub fn build_draft(
        self,
        expected_pubkey: impl AsRef<str>,
        created_at: u32,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        Ok(prepare_knowledge_claim_draft(
            &self.build()?,
            expected_pubkey,
            created_at,
        )?)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsKnowledgeRelationBuilder {
    subject: Option<RadrootsKnowledgeNodeRef>,
    predicate: Option<String>,
    object: Option<RadrootsKnowledgeNodeRef>,
    support_refs: Vec<RadrootsEventRef>,
    author_asserted_confidence: Option<String>,
    supersedes: Vec<RadrootsEventRef>,
}

impl RadrootsKnowledgeRelationBuilder {
    pub fn new() -> Self {
        Self {
            subject: None,
            predicate: None,
            object: None,
            support_refs: Vec::new(),
            author_asserted_confidence: None,
            supersedes: Vec::new(),
        }
    }

    pub fn subject(mut self, subject: RadrootsKnowledgeNodeRef) -> Self {
        self.subject = Some(subject);
        self
    }

    pub fn predicate(mut self, predicate: impl Into<String>) -> Self {
        self.predicate = Some(predicate.into());
        self
    }

    pub fn object(mut self, object: RadrootsKnowledgeNodeRef) -> Self {
        self.object = Some(object);
        self
    }

    pub fn support_ref(mut self, support_ref: RadrootsEventRef) -> Self {
        self.support_refs.push(support_ref);
        self
    }

    pub fn author_asserted_confidence(
        mut self,
        author_asserted_confidence: impl Into<String>,
    ) -> Self {
        self.author_asserted_confidence = Some(author_asserted_confidence.into());
        self
    }

    pub fn supersedes(mut self, supersedes: RadrootsEventRef) -> Self {
        self.supersedes.push(supersedes);
        self
    }

    pub fn build(self) -> Result<RadrootsKnowledgeRelation, RadrootsKnowledgeBuilderError> {
        let relation = RadrootsKnowledgeRelation {
            schema: RADROOTS_KNOWLEDGE_RELATION_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            subject: builder_required(self.subject, "subject")?,
            predicate: builder_required_string(self.predicate, "predicate")?,
            object: builder_required(self.object, "object")?,
            support_refs: self.support_refs,
            author_asserted_confidence: self.author_asserted_confidence,
            supersedes: self.supersedes,
        };
        builder_validated(relation, validate_knowledge_relation)
    }

    pub fn build_event(self) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        Ok(build_knowledge_relation_event(&self.build()?)?)
    }

    pub fn build_draft(
        self,
        expected_pubkey: impl AsRef<str>,
        created_at: u32,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        Ok(prepare_knowledge_relation_draft(
            &self.build()?,
            expected_pubkey,
            created_at,
        )?)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsKnowledgeReviewBuilder {
    target: Option<RadrootsKnowledgeReviewTarget>,
    reviewer_role: Option<String>,
    verdict: Option<String>,
    scores: Vec<RadrootsKnowledgeReviewScore>,
    notes: Option<String>,
    evidence_refs: Vec<RadrootsEventRef>,
}

impl RadrootsKnowledgeReviewBuilder {
    pub fn new() -> Self {
        Self {
            target: None,
            reviewer_role: None,
            verdict: None,
            scores: Vec::new(),
            notes: None,
            evidence_refs: Vec::new(),
        }
    }

    pub fn target(mut self, target: RadrootsKnowledgeReviewTarget) -> Self {
        self.target = Some(target);
        self
    }

    pub fn reviewer_role(mut self, reviewer_role: impl Into<String>) -> Self {
        self.reviewer_role = Some(reviewer_role.into());
        self
    }

    pub fn verdict(mut self, verdict: impl Into<String>) -> Self {
        self.verdict = Some(verdict.into());
        self
    }

    pub fn score(mut self, score: RadrootsKnowledgeReviewScore) -> Self {
        self.scores.push(score);
        self
    }

    pub fn notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    pub fn evidence_ref(mut self, evidence_ref: RadrootsEventRef) -> Self {
        self.evidence_refs.push(evidence_ref);
        self
    }

    pub fn build(self) -> Result<RadrootsKnowledgeReview, RadrootsKnowledgeBuilderError> {
        let review = RadrootsKnowledgeReview {
            schema: RADROOTS_KNOWLEDGE_REVIEW_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            target: builder_required(self.target, "target")?,
            reviewer_role: builder_required_string(self.reviewer_role, "reviewer_role")?,
            verdict: builder_required_string(self.verdict, "verdict")?,
            scores: self.scores,
            notes: self.notes,
            evidence_refs: self.evidence_refs,
        };
        builder_validated(review, validate_knowledge_review)
    }

    pub fn build_event(self) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        Ok(build_knowledge_review_event(&self.build()?)?)
    }

    pub fn build_draft(
        self,
        expected_pubkey: impl AsRef<str>,
        created_at: u32,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        Ok(prepare_knowledge_review_draft(
            &self.build()?,
            expected_pubkey,
            created_at,
        )?)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsKnowledgeFieldReportBuilder {
    report_type: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    context: Option<RadrootsKnowledgeFieldContext>,
    observations: Vec<RadrootsKnowledgeObservation>,
    artifact_refs: Vec<RadrootsEventRef>,
    related_refs: Vec<RadrootsEventRef>,
    limitations: Vec<String>,
}

impl RadrootsKnowledgeFieldReportBuilder {
    pub fn new() -> Self {
        Self {
            report_type: None,
            title: None,
            summary: None,
            context: None,
            observations: Vec::new(),
            artifact_refs: Vec::new(),
            related_refs: Vec::new(),
            limitations: Vec::new(),
        }
    }

    pub fn report_type(mut self, report_type: impl Into<String>) -> Self {
        self.report_type = Some(report_type.into());
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    pub fn context(mut self, context: RadrootsKnowledgeFieldContext) -> Self {
        self.context = Some(context);
        self
    }

    pub fn observation(mut self, observation: RadrootsKnowledgeObservation) -> Self {
        self.observations.push(observation);
        self
    }

    pub fn artifact_ref(mut self, artifact_ref: RadrootsEventRef) -> Self {
        self.artifact_refs.push(artifact_ref);
        self
    }

    pub fn related_ref(mut self, related_ref: RadrootsEventRef) -> Self {
        self.related_refs.push(related_ref);
        self
    }

    pub fn limitation(mut self, limitation: impl Into<String>) -> Self {
        self.limitations.push(limitation.into());
        self
    }

    pub fn build(self) -> Result<RadrootsKnowledgeFieldReport, RadrootsKnowledgeBuilderError> {
        let report = RadrootsKnowledgeFieldReport {
            schema: RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            report_type: builder_required_string(self.report_type, "report_type")?,
            title: builder_required_string(self.title, "title")?,
            summary: self.summary,
            context: builder_required(self.context, "context")?,
            observations: self.observations,
            artifact_refs: self.artifact_refs,
            related_refs: self.related_refs,
            limitations: self.limitations,
        };
        builder_validated(report, validate_knowledge_field_report)
    }

    pub fn build_event(self) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        Ok(build_knowledge_field_report_event(&self.build()?)?)
    }

    pub fn build_draft(
        self,
        expected_pubkey: impl AsRef<str>,
        created_at: u32,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        Ok(prepare_knowledge_field_report_draft(
            &self.build()?,
            expected_pubkey,
            created_at,
        )?)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KnowledgeEventBuilder;

impl KnowledgeEventBuilder {
    pub const fn new() -> Self {
        Self
    }

    pub fn wiki_article(
        &self,
        article: &RadrootsWikiArticle,
    ) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        build_wiki_article_event(article)
    }

    pub fn wiki_redirect(
        &self,
        redirect: &RadrootsWikiRedirect,
    ) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        build_wiki_redirect_event(redirect)
    }

    pub fn wiki_merge_request(
        &self,
        request: &RadrootsWikiMergeRequest,
    ) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        build_wiki_merge_request_event(request)
    }

    pub fn knowledge_source(
        &self,
        source: &RadrootsKnowledgeSource,
    ) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        build_knowledge_source_event(source)
    }

    pub fn knowledge_claim(
        &self,
        claim: &RadrootsKnowledgeClaim,
    ) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        build_knowledge_claim_event(claim)
    }

    pub fn knowledge_relation(
        &self,
        relation: &RadrootsKnowledgeRelation,
    ) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        build_knowledge_relation_event(relation)
    }

    pub fn knowledge_review(
        &self,
        review: &RadrootsKnowledgeReview,
    ) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        build_knowledge_review_event(review)
    }

    pub fn knowledge_field_report(
        &self,
        report: &RadrootsKnowledgeFieldReport,
    ) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
        build_knowledge_field_report_event(report)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnowledgeDraftBuilder {
    expected_pubkey: String,
    created_at: u32,
}

impl KnowledgeDraftBuilder {
    pub fn new(expected_pubkey: impl AsRef<str>, created_at: u32) -> Self {
        Self {
            expected_pubkey: expected_pubkey.as_ref().to_string(),
            created_at,
        }
    }

    pub fn expected_pubkey(&self) -> &str {
        self.expected_pubkey.as_str()
    }

    pub const fn created_at(&self) -> u32 {
        self.created_at
    }

    pub fn wiki_article(
        &self,
        article: &RadrootsWikiArticle,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        prepare_wiki_article_draft(article, self.expected_pubkey(), self.created_at)
    }

    pub fn wiki_redirect(
        &self,
        redirect: &RadrootsWikiRedirect,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        prepare_wiki_redirect_draft(redirect, self.expected_pubkey(), self.created_at)
    }

    pub fn wiki_merge_request(
        &self,
        request: &RadrootsWikiMergeRequest,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        prepare_wiki_merge_request_draft(request, self.expected_pubkey(), self.created_at)
    }

    pub fn knowledge_source(
        &self,
        source: &RadrootsKnowledgeSource,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        prepare_knowledge_source_draft(source, self.expected_pubkey(), self.created_at)
    }

    pub fn knowledge_claim(
        &self,
        claim: &RadrootsKnowledgeClaim,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        prepare_knowledge_claim_draft(claim, self.expected_pubkey(), self.created_at)
    }

    pub fn knowledge_relation(
        &self,
        relation: &RadrootsKnowledgeRelation,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        prepare_knowledge_relation_draft(relation, self.expected_pubkey(), self.created_at)
    }

    pub fn knowledge_review(
        &self,
        review: &RadrootsKnowledgeReview,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        prepare_knowledge_review_draft(review, self.expected_pubkey(), self.created_at)
    }

    pub fn knowledge_field_report(
        &self,
        report: &RadrootsKnowledgeFieldReport,
    ) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
        prepare_knowledge_field_report_draft(report, self.expected_pubkey(), self.created_at)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KnowledgeCodec;

impl KnowledgeCodec {
    pub const fn new() -> Self {
        Self
    }

    pub fn verify_and_decode_radroots_event(
        &self,
        event: RadrootsEventEnvelope,
    ) -> Result<RadrootsDecodedEvent, RadrootsSdkKnowledgeError> {
        verify_and_decode_radroots_event(event)
    }

    pub fn contract_manifest(&self) -> RadrootsKnowledgeContractManifest {
        contract_manifest()
    }

    pub fn contract_manifest_json(&self) -> Result<String, RadrootsSdkKnowledgeError> {
        contract_manifest_json()
    }

    pub fn contract_manifest_sha256(&self) -> Result<String, RadrootsSdkKnowledgeError> {
        contract_manifest_sha256()
    }
}

pub fn build_wiki_article_event(
    article: &RadrootsWikiArticle,
) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
    Ok(wiki_article_to_wire_parts(article)?)
}

pub fn build_wiki_redirect_event(
    redirect: &RadrootsWikiRedirect,
) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
    Ok(wiki_redirect_to_wire_parts(redirect)?)
}

pub fn build_wiki_merge_request_event(
    request: &RadrootsWikiMergeRequest,
) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
    Ok(wiki_merge_request_to_wire_parts(request)?)
}

pub fn build_knowledge_source_event(
    source: &RadrootsKnowledgeSource,
) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
    Ok(knowledge_source_to_wire_parts(source)?)
}

pub fn build_knowledge_claim_event(
    claim: &RadrootsKnowledgeClaim,
) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
    Ok(knowledge_claim_to_wire_parts(claim)?)
}

pub fn build_knowledge_relation_event(
    relation: &RadrootsKnowledgeRelation,
) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
    Ok(knowledge_relation_to_wire_parts(relation)?)
}

pub fn build_knowledge_review_event(
    review: &RadrootsKnowledgeReview,
) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
    Ok(knowledge_review_to_wire_parts(review)?)
}

pub fn build_knowledge_field_report_event(
    report: &RadrootsKnowledgeFieldReport,
) -> Result<RadrootsNip01EventWireParts, RadrootsSdkKnowledgeError> {
    Ok(knowledge_field_report_to_wire_parts(report)?)
}

pub fn prepare_wiki_article_draft(
    article: &RadrootsWikiArticle,
    expected_pubkey: impl AsRef<str>,
    created_at: u32,
) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
    prepare_draft(
        build_wiki_article_event(article)?,
        WIKI_ARTICLE_CONTRACT_ID,
        expected_pubkey,
        created_at,
    )
}

pub fn prepare_wiki_redirect_draft(
    redirect: &RadrootsWikiRedirect,
    expected_pubkey: impl AsRef<str>,
    created_at: u32,
) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
    prepare_draft(
        build_wiki_redirect_event(redirect)?,
        WIKI_REDIRECT_CONTRACT_ID,
        expected_pubkey,
        created_at,
    )
}

pub fn prepare_wiki_merge_request_draft(
    request: &RadrootsWikiMergeRequest,
    expected_pubkey: impl AsRef<str>,
    created_at: u32,
) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
    prepare_draft(
        build_wiki_merge_request_event(request)?,
        WIKI_MERGE_REQUEST_CONTRACT_ID,
        expected_pubkey,
        created_at,
    )
}

pub fn prepare_knowledge_source_draft(
    source: &RadrootsKnowledgeSource,
    expected_pubkey: impl AsRef<str>,
    created_at: u32,
) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
    prepare_draft(
        build_knowledge_source_event(source)?,
        KNOWLEDGE_SOURCE_CONTRACT_ID,
        expected_pubkey,
        created_at,
    )
}

pub fn prepare_knowledge_claim_draft(
    claim: &RadrootsKnowledgeClaim,
    expected_pubkey: impl AsRef<str>,
    created_at: u32,
) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
    prepare_draft(
        build_knowledge_claim_event(claim)?,
        KNOWLEDGE_CLAIM_CONTRACT_ID,
        expected_pubkey,
        created_at,
    )
}

pub fn prepare_knowledge_relation_draft(
    relation: &RadrootsKnowledgeRelation,
    expected_pubkey: impl AsRef<str>,
    created_at: u32,
) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
    prepare_draft(
        build_knowledge_relation_event(relation)?,
        KNOWLEDGE_RELATION_CONTRACT_ID,
        expected_pubkey,
        created_at,
    )
}

pub fn prepare_knowledge_review_draft(
    review: &RadrootsKnowledgeReview,
    expected_pubkey: impl AsRef<str>,
    created_at: u32,
) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
    prepare_draft(
        build_knowledge_review_event(review)?,
        KNOWLEDGE_REVIEW_CONTRACT_ID,
        expected_pubkey,
        created_at,
    )
}

pub fn prepare_knowledge_field_report_draft(
    report: &RadrootsKnowledgeFieldReport,
    expected_pubkey: impl AsRef<str>,
    created_at: u32,
) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
    prepare_draft(
        build_knowledge_field_report_event(report)?,
        KNOWLEDGE_FIELD_REPORT_CONTRACT_ID,
        expected_pubkey,
        created_at,
    )
}

pub fn verify_and_decode_radroots_event(
    event: RadrootsEventEnvelope,
) -> Result<RadrootsDecodedEvent, RadrootsSdkKnowledgeError> {
    Ok(codec_verify_and_decode(event)?)
}

pub fn contract_manifest() -> RadrootsKnowledgeContractManifest {
    knowledge_contract_manifest()
}

pub fn contract_manifest_json() -> Result<String, RadrootsSdkKnowledgeError> {
    Ok(codec_contract_manifest_json()?)
}

pub fn contract_manifest_sha256() -> Result<String, RadrootsSdkKnowledgeError> {
    Ok(codec_contract_manifest_sha256()?)
}

fn prepare_draft(
    parts: RadrootsNip01EventWireParts,
    contract_id: &'static str,
    expected_pubkey: impl AsRef<str>,
    created_at: u32,
) -> Result<RadrootsEventDraft, RadrootsSdkKnowledgeError> {
    Ok(RadrootsEventDraft::new(
        contract_id,
        parts.kind,
        u64::from(created_at),
        parts.tags,
        parts.content,
        expected_pubkey.as_ref(),
    )?)
}

fn draft_error_code(error: &RadrootsDraftError) -> &'static str {
    match error {
        RadrootsDraftError::UnknownContract(_) => "unknown_contract",
        RadrootsDraftError::ContractKindMismatch { .. } => "contract_kind_mismatch",
        RadrootsDraftError::ContractShape { error, .. } => error.code(),
        RadrootsDraftError::SignedEventPubkeyMismatch { .. } => "signed_event_pubkey_mismatch",
        RadrootsDraftError::SignedEventIdMismatch { .. } => "signed_event_id_mismatch",
        RadrootsDraftError::SignedEventCreatedAtMismatch { .. } => {
            "signed_event_created_at_mismatch"
        }
        RadrootsDraftError::SignedEventKindMismatch { .. } => "signed_event_kind_mismatch",
        RadrootsDraftError::SignedEventTagsMismatch { .. } => "signed_event_tags_mismatch",
        RadrootsDraftError::SignedEventContentMismatch { .. } => "signed_event_content_mismatch",
        RadrootsDraftError::SignedEventComputedIdMismatch { .. } => {
            "signed_event_computed_id_mismatch"
        }
        RadrootsDraftError::IdParse(_) => "id_parse",
        RadrootsDraftError::CanonicalEventId(_) => "canonical_event_id",
        RadrootsDraftError::Envelope(_) => "event_envelope",
        RadrootsDraftError::SignedEvent(_) => "signed_event",
    }
}

fn builder_required<T>(
    value: Option<T>,
    field: &'static str,
) -> Result<T, RadrootsKnowledgeBuilderError> {
    value.ok_or(RadrootsKnowledgeBuilderError::MissingField(field))
}

fn builder_required_string(
    value: Option<String>,
    field: &'static str,
) -> Result<String, RadrootsKnowledgeBuilderError> {
    builder_non_empty_string(builder_required(value, field)?, field)
}

fn builder_non_empty_string(
    value: String,
    field: &'static str,
) -> Result<String, RadrootsKnowledgeBuilderError> {
    if value.trim().is_empty() {
        Err(RadrootsKnowledgeBuilderError::MissingField(field))
    } else {
        Ok(value)
    }
}

fn builder_validation_error(
    error: RadrootsKnowledgeValidationError,
) -> RadrootsKnowledgeBuilderError {
    match error {
        RadrootsKnowledgeValidationError::EmptyField(field) => {
            RadrootsKnowledgeBuilderError::MissingField(field)
        }
        RadrootsKnowledgeValidationError::InvalidField(field) => {
            RadrootsKnowledgeBuilderError::InvalidField(field)
        }
    }
}

fn builder_validated<T>(
    value: T,
    validate: fn(&T) -> Result<(), RadrootsKnowledgeValidationError>,
) -> Result<T, RadrootsKnowledgeBuilderError> {
    validate(&value).map_err(builder_validation_error)?;
    Ok(value)
}

pub mod prelude {
    pub use super::{
        KIND_FILE_METADATA, KIND_KNOWLEDGE_CLAIM, KIND_KNOWLEDGE_FIELD_REPORT,
        KIND_KNOWLEDGE_RELATION, KIND_KNOWLEDGE_REVIEW, KIND_KNOWLEDGE_SOURCE, KIND_WIKI_ARTICLE,
        KIND_WIKI_MERGE_REQUEST, KIND_WIKI_REDIRECT, KNOWLEDGE_CLAIM_CONTRACT_ID,
        KNOWLEDGE_FIELD_REPORT_CONTRACT_ID, KNOWLEDGE_RELATION_CONTRACT_ID,
        KNOWLEDGE_REVIEW_CONTRACT_ID, KNOWLEDGE_SOURCE_CONTRACT_ID, KnowledgeCodec,
        KnowledgeDraftBuilder, KnowledgeEventBuilder, RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA,
        RADROOTS_EVIDENCE_BOUNTY_SCHEMA, RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA,
        RADROOTS_KNOWLEDGE_CLAIM_SCHEMA, RADROOTS_KNOWLEDGE_CONTRACT_MANIFEST_SCHEMA_VERSION,
        RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA, RADROOTS_KNOWLEDGE_RELATION_SCHEMA,
        RADROOTS_KNOWLEDGE_REVIEW_SCHEMA, RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        RADROOTS_KNOWLEDGE_SOURCE_SCHEMA, RADROOTS_WIKI_D_TAG_MAX_LEN, RadrootsAddressableRef,
        RadrootsContractValidatedEvent, RadrootsDecodeError, RadrootsDecodedEvent,
        RadrootsDraftError, RadrootsEncodeError, RadrootsEventDraft, RadrootsEventEnvelope,
        RadrootsEventRef, RadrootsIdVerifiedEvent, RadrootsKnowledgeBuilderError,
        RadrootsKnowledgeChangeProposal, RadrootsKnowledgeCitationSpan, RadrootsKnowledgeClaim,
        RadrootsKnowledgeClaimBuilder, RadrootsKnowledgeContractManifest,
        RadrootsKnowledgeContractManifestEntry, RadrootsKnowledgeFieldContext,
        RadrootsKnowledgeFieldReport, RadrootsKnowledgeFieldReportBuilder,
        RadrootsKnowledgeLocation, RadrootsKnowledgeLocationPrecision,
        RadrootsKnowledgeManifestCodecSupport, RadrootsKnowledgeManifestDiscriminator,
        RadrootsKnowledgeManifestTagContract, RadrootsKnowledgeNodeRef,
        RadrootsKnowledgeObservation, RadrootsKnowledgeObservationValue, RadrootsKnowledgeRelation,
        RadrootsKnowledgeRelationBuilder, RadrootsKnowledgeReview, RadrootsKnowledgeReviewBuilder,
        RadrootsKnowledgeReviewScope, RadrootsKnowledgeReviewScore, RadrootsKnowledgeReviewTarget,
        RadrootsKnowledgeSource, RadrootsKnowledgeSourceBuilder, RadrootsKnowledgeValidationError,
        RadrootsNip01EventWireParts, RadrootsNip01VerificationError, RadrootsRightsAssertion,
        RadrootsSdkKnowledgeError, RadrootsSignatureVerifiedEvent, RadrootsWikiArticle,
        RadrootsWikiArticleBuilder, RadrootsWikiArticleVersionRef, RadrootsWikiDTagError,
        RadrootsWikiMergeRequest, RadrootsWikiMergeRequestBuilder, RadrootsWikiRedirect,
        RadrootsWikiRedirectBuilder, WIKI_ARTICLE_CONTRACT_ID, WIKI_MERGE_REQUEST_CONTRACT_ID,
        WIKI_REDIRECT_CONTRACT_ID, build_knowledge_claim_event, build_knowledge_field_report_event,
        build_knowledge_relation_event, build_knowledge_review_event, build_knowledge_source_event,
        build_wiki_article_event, build_wiki_merge_request_event, build_wiki_redirect_event,
        contract_manifest, contract_manifest_json, contract_manifest_sha256, normalize_wiki_d_tag,
        prepare_knowledge_claim_draft, prepare_knowledge_field_report_draft,
        prepare_knowledge_relation_draft, prepare_knowledge_review_draft,
        prepare_knowledge_source_draft, prepare_wiki_article_draft,
        prepare_wiki_merge_request_draft, prepare_wiki_redirect_draft, validate_knowledge_claim,
        validate_wiki_article, validate_wiki_d_tag, verify_and_decode_radroots_event,
    };
}
