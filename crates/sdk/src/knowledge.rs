#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(feature = "std")]
use std::string::{String, ToString};

use core::fmt;

pub use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventRef,
    draft::{RadrootsDraftError, RadrootsFrozenEventDraft},
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
        RadrootsKnowledgeSource, RadrootsRightsAssertion, RadrootsWikiArticle,
        RadrootsWikiDTagError, RadrootsWikiMergeRequest, RadrootsWikiRedirect,
        normalize_wiki_d_tag, validate_wiki_d_tag,
    },
};
pub use radroots_events_codec::{
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
    wire::WireEventParts,
};

use radroots_events_codec::{
    contract_manifest_json as codec_contract_manifest_json,
    contract_manifest_sha256 as codec_contract_manifest_sha256,
    knowledge::{
        knowledge_claim_to_wire_parts, knowledge_field_report_to_wire_parts,
        knowledge_relation_to_wire_parts, knowledge_review_to_wire_parts,
        knowledge_source_to_wire_parts, wiki_article_to_wire_parts,
        wiki_merge_request_to_wire_parts, wiki_redirect_to_wire_parts,
    },
    knowledge_contract_manifest, verify_and_decode_radroots_event as codec_verify_and_decode,
    wire::to_frozen_draft,
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
    Encode(RadrootsEncodeError),
    Draft(RadrootsDraftError),
    Decode(RadrootsDecodeError),
    Manifest(serde_json::Error),
}

impl RadrootsSdkKnowledgeError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Encode(_) => "knowledge_encode",
            Self::Draft(_) => "knowledge_draft",
            Self::Decode(_) => "knowledge_decode",
            Self::Manifest(_) => "knowledge_manifest",
        }
    }

    pub fn inner_code(&self) -> &'static str {
        match self {
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
            Self::Encode(error) => Some(error),
            Self::Draft(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Manifest(error) => Some(error),
        }
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KnowledgeEventBuilder;

impl KnowledgeEventBuilder {
    pub const fn new() -> Self {
        Self
    }

    pub fn wiki_article(
        &self,
        article: &RadrootsWikiArticle,
    ) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
        build_wiki_article_event(article)
    }

    pub fn wiki_redirect(
        &self,
        redirect: &RadrootsWikiRedirect,
    ) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
        build_wiki_redirect_event(redirect)
    }

    pub fn wiki_merge_request(
        &self,
        request: &RadrootsWikiMergeRequest,
    ) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
        build_wiki_merge_request_event(request)
    }

    pub fn knowledge_source(
        &self,
        source: &RadrootsKnowledgeSource,
    ) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
        build_knowledge_source_event(source)
    }

    pub fn knowledge_claim(
        &self,
        claim: &RadrootsKnowledgeClaim,
    ) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
        build_knowledge_claim_event(claim)
    }

    pub fn knowledge_relation(
        &self,
        relation: &RadrootsKnowledgeRelation,
    ) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
        build_knowledge_relation_event(relation)
    }

    pub fn knowledge_review(
        &self,
        review: &RadrootsKnowledgeReview,
    ) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
        build_knowledge_review_event(review)
    }

    pub fn knowledge_field_report(
        &self,
        report: &RadrootsKnowledgeFieldReport,
    ) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
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
    ) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
        prepare_wiki_article_draft(article, self.expected_pubkey(), self.created_at)
    }

    pub fn wiki_redirect(
        &self,
        redirect: &RadrootsWikiRedirect,
    ) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
        prepare_wiki_redirect_draft(redirect, self.expected_pubkey(), self.created_at)
    }

    pub fn wiki_merge_request(
        &self,
        request: &RadrootsWikiMergeRequest,
    ) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
        prepare_wiki_merge_request_draft(request, self.expected_pubkey(), self.created_at)
    }

    pub fn knowledge_source(
        &self,
        source: &RadrootsKnowledgeSource,
    ) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
        prepare_knowledge_source_draft(source, self.expected_pubkey(), self.created_at)
    }

    pub fn knowledge_claim(
        &self,
        claim: &RadrootsKnowledgeClaim,
    ) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
        prepare_knowledge_claim_draft(claim, self.expected_pubkey(), self.created_at)
    }

    pub fn knowledge_relation(
        &self,
        relation: &RadrootsKnowledgeRelation,
    ) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
        prepare_knowledge_relation_draft(relation, self.expected_pubkey(), self.created_at)
    }

    pub fn knowledge_review(
        &self,
        review: &RadrootsKnowledgeReview,
    ) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
        prepare_knowledge_review_draft(review, self.expected_pubkey(), self.created_at)
    }

    pub fn knowledge_field_report(
        &self,
        report: &RadrootsKnowledgeFieldReport,
    ) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
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
        event: RadrootsNostrEvent,
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
) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
    Ok(wiki_article_to_wire_parts(article)?)
}

pub fn build_wiki_redirect_event(
    redirect: &RadrootsWikiRedirect,
) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
    Ok(wiki_redirect_to_wire_parts(redirect)?)
}

pub fn build_wiki_merge_request_event(
    request: &RadrootsWikiMergeRequest,
) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
    Ok(wiki_merge_request_to_wire_parts(request)?)
}

pub fn build_knowledge_source_event(
    source: &RadrootsKnowledgeSource,
) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
    Ok(knowledge_source_to_wire_parts(source)?)
}

pub fn build_knowledge_claim_event(
    claim: &RadrootsKnowledgeClaim,
) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
    Ok(knowledge_claim_to_wire_parts(claim)?)
}

pub fn build_knowledge_relation_event(
    relation: &RadrootsKnowledgeRelation,
) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
    Ok(knowledge_relation_to_wire_parts(relation)?)
}

pub fn build_knowledge_review_event(
    review: &RadrootsKnowledgeReview,
) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
    Ok(knowledge_review_to_wire_parts(review)?)
}

pub fn build_knowledge_field_report_event(
    report: &RadrootsKnowledgeFieldReport,
) -> Result<WireEventParts, RadrootsSdkKnowledgeError> {
    Ok(knowledge_field_report_to_wire_parts(report)?)
}

pub fn prepare_wiki_article_draft(
    article: &RadrootsWikiArticle,
    expected_pubkey: impl AsRef<str>,
    created_at: u32,
) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
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
) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
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
) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
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
) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
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
) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
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
) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
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
) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
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
) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
    prepare_draft(
        build_knowledge_field_report_event(report)?,
        KNOWLEDGE_FIELD_REPORT_CONTRACT_ID,
        expected_pubkey,
        created_at,
    )
}

pub fn verify_and_decode_radroots_event(
    event: RadrootsNostrEvent,
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
    parts: WireEventParts,
    contract_id: &'static str,
    expected_pubkey: impl AsRef<str>,
    created_at: u32,
) -> Result<RadrootsFrozenEventDraft, RadrootsSdkKnowledgeError> {
    Ok(to_frozen_draft(
        parts,
        contract_id,
        expected_pubkey,
        created_at,
    )?)
}

fn draft_error_code(error: &RadrootsDraftError) -> &'static str {
    match error {
        RadrootsDraftError::UnknownContract(_) => "unknown_contract",
        RadrootsDraftError::ContractKindMismatch { .. } => "contract_kind_mismatch",
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
        RadrootsDraftError::JsonString(_) => "json_string",
    }
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
        RadrootsDraftError, RadrootsEncodeError, RadrootsFrozenEventDraft, RadrootsIdVerifiedEvent,
        RadrootsKnowledgeChangeProposal, RadrootsKnowledgeCitationSpan, RadrootsKnowledgeClaim,
        RadrootsKnowledgeContractManifest, RadrootsKnowledgeContractManifestEntry,
        RadrootsKnowledgeFieldContext, RadrootsKnowledgeFieldReport, RadrootsKnowledgeLocation,
        RadrootsKnowledgeLocationPrecision, RadrootsKnowledgeManifestCodecSupport,
        RadrootsKnowledgeManifestDiscriminator, RadrootsKnowledgeManifestTagContract,
        RadrootsKnowledgeNodeRef, RadrootsKnowledgeObservation, RadrootsKnowledgeObservationValue,
        RadrootsKnowledgeRelation, RadrootsKnowledgeReview, RadrootsKnowledgeReviewScope,
        RadrootsKnowledgeReviewScore, RadrootsKnowledgeReviewTarget, RadrootsKnowledgeSource,
        RadrootsNip01VerificationError, RadrootsNostrEvent, RadrootsNostrEventRef,
        RadrootsRightsAssertion, RadrootsSdkKnowledgeError, RadrootsSignatureVerifiedEvent,
        RadrootsWikiArticle, RadrootsWikiDTagError, RadrootsWikiMergeRequest, RadrootsWikiRedirect,
        WIKI_ARTICLE_CONTRACT_ID, WIKI_MERGE_REQUEST_CONTRACT_ID, WIKI_REDIRECT_CONTRACT_ID,
        WireEventParts, build_knowledge_claim_event, build_knowledge_field_report_event,
        build_knowledge_relation_event, build_knowledge_review_event, build_knowledge_source_event,
        build_wiki_article_event, build_wiki_merge_request_event, build_wiki_redirect_event,
        contract_manifest, contract_manifest_json, contract_manifest_sha256, normalize_wiki_d_tag,
        prepare_knowledge_claim_draft, prepare_knowledge_field_report_draft,
        prepare_knowledge_relation_draft, prepare_knowledge_review_draft,
        prepare_knowledge_source_draft, prepare_wiki_article_draft,
        prepare_wiki_merge_request_draft, prepare_wiki_redirect_draft, validate_wiki_d_tag,
        verify_and_decode_radroots_event,
    };
}
