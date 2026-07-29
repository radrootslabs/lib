#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

use core::fmt;

use radroots_event::contract::RadrootsContractValidationError;
use radroots_event::envelope::RadrootsEventEnvelope;
use radroots_event::knowledge::{
    RadrootsContributionAttestation, RadrootsEvidenceBounty, RadrootsKnowledgeChangeProposal,
    RadrootsKnowledgeClaim, RadrootsKnowledgeFieldReport, RadrootsKnowledgeRelation,
    RadrootsKnowledgeReview, RadrootsKnowledgeSource, RadrootsWikiArticle,
    RadrootsWikiMergeRequest, RadrootsWikiRedirect,
};

use crate::error::EventParseError;
use crate::knowledge::decode::{
    contribution_attestation_from_event, evidence_bounty_from_event,
    knowledge_change_proposal_from_event, knowledge_claim_from_event,
    knowledge_field_report_from_event, knowledge_relation_from_event, knowledge_review_from_event,
    knowledge_source_from_event, wiki_article_from_event, wiki_merge_request_from_event,
    wiki_redirect_from_event,
};
use crate::parsed::RadrootsParsedEvent;
pub use crate::verification::{RadrootsContractValidatedEvent, validate_event_contract};
use crate::verification::{RadrootsNip01VerificationError, verify_nip01_event};

#[derive(Debug)]
pub enum RadrootsDecodeError {
    Nip01Verification(RadrootsNip01VerificationError),
    ContractValidation(RadrootsContractValidationError),
    EventParse(EventParseError),
    UnsupportedContract { contract_id: String },
}

impl RadrootsDecodeError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Nip01Verification(_) => "nip01_verification",
            Self::ContractValidation(_) => "contract_validation",
            Self::EventParse(_) => "event_parse",
            Self::UnsupportedContract { .. } => "unsupported_contract",
        }
    }
}

impl fmt::Display for RadrootsDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nip01Verification(error) => write!(formatter, "{error}"),
            Self::ContractValidation(error) => {
                write!(
                    formatter,
                    "contract validation failed with code {}",
                    error.code()
                )
            }
            Self::EventParse(error) => write!(formatter, "{error}"),
            Self::UnsupportedContract { contract_id } => {
                write!(formatter, "unsupported event contract `{contract_id}`")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsDecodeError {}

impl From<EventParseError> for RadrootsDecodeError {
    fn from(value: EventParseError) -> Self {
        Self::EventParse(value)
    }
}

impl From<RadrootsNip01VerificationError> for RadrootsDecodeError {
    fn from(value: RadrootsNip01VerificationError) -> Self {
        Self::Nip01Verification(value)
    }
}

impl From<RadrootsContractValidationError> for RadrootsDecodeError {
    fn from(value: RadrootsContractValidationError) -> Self {
        Self::ContractValidation(value)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum RadrootsDecodedEvent {
    WikiArticle(RadrootsParsedEvent<RadrootsWikiArticle>),
    WikiRedirect(RadrootsParsedEvent<RadrootsWikiRedirect>),
    WikiMergeRequest(RadrootsParsedEvent<RadrootsWikiMergeRequest>),
    KnowledgeSource(RadrootsParsedEvent<RadrootsKnowledgeSource>),
    KnowledgeClaim(RadrootsParsedEvent<RadrootsKnowledgeClaim>),
    KnowledgeRelation(RadrootsParsedEvent<RadrootsKnowledgeRelation>),
    KnowledgeReview(RadrootsParsedEvent<RadrootsKnowledgeReview>),
    KnowledgeFieldReport(RadrootsParsedEvent<RadrootsKnowledgeFieldReport>),
    EvidenceBounty(RadrootsParsedEvent<RadrootsEvidenceBounty>),
    KnowledgeChangeProposal(RadrootsParsedEvent<RadrootsKnowledgeChangeProposal>),
    ContributionAttestation(RadrootsParsedEvent<RadrootsContributionAttestation>),
}

impl RadrootsDecodedEvent {
    pub fn event(&self) -> &RadrootsEventEnvelope {
        match self {
            Self::WikiArticle(parsed) => &parsed.event,
            Self::WikiRedirect(parsed) => &parsed.event,
            Self::WikiMergeRequest(parsed) => &parsed.event,
            Self::KnowledgeSource(parsed) => &parsed.event,
            Self::KnowledgeClaim(parsed) => &parsed.event,
            Self::KnowledgeRelation(parsed) => &parsed.event,
            Self::KnowledgeReview(parsed) => &parsed.event,
            Self::KnowledgeFieldReport(parsed) => &parsed.event,
            Self::EvidenceBounty(parsed) => &parsed.event,
            Self::KnowledgeChangeProposal(parsed) => &parsed.event,
            Self::ContributionAttestation(parsed) => &parsed.event,
        }
    }
}

pub fn decode_validated_event(
    event: RadrootsContractValidatedEvent,
) -> Result<RadrootsDecodedEvent, RadrootsDecodeError> {
    let contract_id = event.contract_id();
    let event = event.into_event();
    match contract_id {
        "radroots.wiki.article.v1" => Ok(RadrootsDecodedEvent::WikiArticle(
            wiki_article_from_event(event)?,
        )),
        "radroots.wiki.redirect.v1" => Ok(RadrootsDecodedEvent::WikiRedirect(
            wiki_redirect_from_event(event)?,
        )),
        "radroots.wiki.merge_request.v1" => Ok(RadrootsDecodedEvent::WikiMergeRequest(
            wiki_merge_request_from_event(event)?,
        )),
        "radroots.knowledge.source.v1" => Ok(RadrootsDecodedEvent::KnowledgeSource(
            knowledge_source_from_event(event)?,
        )),
        "radroots.knowledge.claim.v1" => Ok(RadrootsDecodedEvent::KnowledgeClaim(
            knowledge_claim_from_event(event)?,
        )),
        "radroots.knowledge.relation.v1" => Ok(RadrootsDecodedEvent::KnowledgeRelation(
            knowledge_relation_from_event(event)?,
        )),
        "radroots.knowledge.review.v1" => Ok(RadrootsDecodedEvent::KnowledgeReview(
            knowledge_review_from_event(event)?,
        )),
        "radroots.knowledge.field_report.v1" => Ok(RadrootsDecodedEvent::KnowledgeFieldReport(
            knowledge_field_report_from_event(event)?,
        )),
        "radroots.knowledge.evidence_bounty.v1" => Ok(RadrootsDecodedEvent::EvidenceBounty(
            evidence_bounty_from_event(event)?,
        )),
        "radroots.knowledge.change_proposal.v1" => {
            Ok(RadrootsDecodedEvent::KnowledgeChangeProposal(
                knowledge_change_proposal_from_event(event)?,
            ))
        }
        "radroots.knowledge.contribution_attestation.v1" => {
            Ok(RadrootsDecodedEvent::ContributionAttestation(
                contribution_attestation_from_event(event)?,
            ))
        }
        contract_id => Err(RadrootsDecodeError::UnsupportedContract {
            contract_id: contract_id.to_string(),
        }),
    }
}

/// Verifies NIP-01 identity before applying the knowledge contract and decoder.
pub fn verify_and_decode_radroots_event(
    event: RadrootsEventEnvelope,
) -> Result<RadrootsDecodedEvent, RadrootsDecodeError> {
    let verified = verify_nip01_event(event)?;
    let contract_validated = validate_event_contract(verified)?;
    decode_validated_event(contract_validated)
}
