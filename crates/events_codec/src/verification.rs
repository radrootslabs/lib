#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

use core::fmt;
#[cfg(feature = "nostr")]
use core::str::FromStr;

use radroots_events::RadrootsNostrEvent;
use radroots_events::contract::{
    RadrootsContractValidationError, RadrootsEventContract,
    validate_event_contract as validate_radroots_event_contract,
};
use radroots_events::draft::compute_nip01_event_id;
use radroots_events::ids::RadrootsEventId;
use radroots_events::knowledge::{
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsIdVerifiedEvent {
    event: RadrootsNostrEvent,
}

impl RadrootsIdVerifiedEvent {
    pub fn event(&self) -> &RadrootsNostrEvent {
        &self.event
    }

    pub fn into_event(self) -> RadrootsNostrEvent {
        self.event
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSignatureVerifiedEvent {
    event: RadrootsNostrEvent,
}

impl RadrootsSignatureVerifiedEvent {
    pub fn event(&self) -> &RadrootsNostrEvent {
        &self.event
    }

    pub fn into_event(self) -> RadrootsNostrEvent {
        self.event
    }
}

/// A NIP-01 verified event whose Radroots contract shape has been validated.
///
/// This stage has checked contract-level kind, discriminator, content schema,
/// schema/schema_version markers where required, and tag cardinality/value
/// shape. It has not yet returned the typed payload semantics; those are
/// checked by `decode_validated_event`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsContractValidatedEvent {
    event: RadrootsNostrEvent,
    contract: &'static RadrootsEventContract,
}

impl RadrootsContractValidatedEvent {
    pub fn event(&self) -> &RadrootsNostrEvent {
        &self.event
    }

    pub fn contract(&self) -> &'static RadrootsEventContract {
        self.contract
    }

    pub fn contract_id(&self) -> &'static str {
        self.contract.id
    }

    pub fn into_event(self) -> RadrootsNostrEvent {
        self.event
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsNip01VerificationError {
    MalformedEnvelope,
    IdMismatch { expected: String, actual: String },
    SignatureInvalid,
    SignatureVerificationUnavailable,
}

impl RadrootsNip01VerificationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MalformedEnvelope => "malformed_envelope",
            Self::IdMismatch { .. } => "id_mismatch",
            Self::SignatureInvalid => "signature_invalid",
            Self::SignatureVerificationUnavailable => "signature_verification_unavailable",
        }
    }
}

impl fmt::Display for RadrootsNip01VerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedEnvelope => formatter.write_str("malformed NIP-01 event envelope"),
            Self::IdMismatch { expected, actual } => {
                write!(
                    formatter,
                    "NIP-01 event id mismatch: expected {expected}, got {actual}"
                )
            }
            Self::SignatureInvalid => formatter.write_str("invalid NIP-01 event signature"),
            Self::SignatureVerificationUnavailable => {
                formatter.write_str("NIP-01 signature verification requires the nostr feature")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsNip01VerificationError {}

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
    pub fn event(&self) -> &RadrootsNostrEvent {
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

pub fn verify_event_id(
    event: RadrootsNostrEvent,
) -> Result<RadrootsIdVerifiedEvent, RadrootsNip01VerificationError> {
    RadrootsEventId::parse(event.id.as_str())
        .map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?;
    let expected = compute_nip01_event_id(
        event.author.as_str(),
        event.created_at,
        event.kind,
        &event.tags,
        event.content.as_str(),
    )
    .map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?
    .into_string();
    if event.id != expected {
        return Err(RadrootsNip01VerificationError::IdMismatch {
            expected,
            actual: event.id,
        });
    }
    Ok(RadrootsIdVerifiedEvent { event })
}

#[cfg(feature = "nostr")]
pub fn verify_event_signature(
    event: RadrootsIdVerifiedEvent,
) -> Result<RadrootsSignatureVerifiedEvent, RadrootsNip01VerificationError> {
    verify_event_id(event.event.clone())?;
    let raw_event = raw_event_from_radroots(&event.event)?;
    if raw_event.verify_signature() {
        Ok(RadrootsSignatureVerifiedEvent { event: event.event })
    } else {
        Err(RadrootsNip01VerificationError::SignatureInvalid)
    }
}

#[cfg(not(feature = "nostr"))]
pub fn verify_event_signature(
    _event: RadrootsIdVerifiedEvent,
) -> Result<RadrootsSignatureVerifiedEvent, RadrootsNip01VerificationError> {
    Err(RadrootsNip01VerificationError::SignatureVerificationUnavailable)
}

/// Validate the Radroots event contract after NIP-01 id and signature checks.
///
/// The successful result is `RadrootsContractValidatedEvent`, which preserves
/// the raw event plus the matched contract metadata. It means the event matched
/// a known Radroots contract shape, not that a typed domain payload has already
/// been returned.
pub fn validate_event_contract(
    event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsContractValidatedEvent, RadrootsContractValidationError> {
    let contract = validate_radroots_event_contract(&event.event)?;
    Ok(RadrootsContractValidatedEvent {
        event: event.event,
        contract,
    })
}

/// Decode a contract-validated event into its typed Radroots event variant.
///
/// This is the stage that turns `RadrootsContractValidatedEvent` into
/// `RadrootsDecodedEvent` and runs the typed decoder/semantic validation for
/// the matched contract. Unsupported contract ids still fail here, even after
/// the generic contract shape was valid.
pub fn decode_validated_event(
    event: RadrootsContractValidatedEvent,
) -> Result<RadrootsDecodedEvent, RadrootsDecodeError> {
    match event.contract.id {
        "radroots.wiki.article.v1" => Ok(RadrootsDecodedEvent::WikiArticle(
            wiki_article_from_event(event.event)?,
        )),
        "radroots.wiki.redirect.v1" => Ok(RadrootsDecodedEvent::WikiRedirect(
            wiki_redirect_from_event(event.event)?,
        )),
        "radroots.wiki.merge_request.v1" => Ok(RadrootsDecodedEvent::WikiMergeRequest(
            wiki_merge_request_from_event(event.event)?,
        )),
        "radroots.knowledge.source.v1" => Ok(RadrootsDecodedEvent::KnowledgeSource(
            knowledge_source_from_event(event.event)?,
        )),
        "radroots.knowledge.claim.v1" => Ok(RadrootsDecodedEvent::KnowledgeClaim(
            knowledge_claim_from_event(event.event)?,
        )),
        "radroots.knowledge.relation.v1" => Ok(RadrootsDecodedEvent::KnowledgeRelation(
            knowledge_relation_from_event(event.event)?,
        )),
        "radroots.knowledge.review.v1" => Ok(RadrootsDecodedEvent::KnowledgeReview(
            knowledge_review_from_event(event.event)?,
        )),
        "radroots.knowledge.field_report.v1" => Ok(RadrootsDecodedEvent::KnowledgeFieldReport(
            knowledge_field_report_from_event(event.event)?,
        )),
        "radroots.knowledge.evidence_bounty.v1" => Ok(RadrootsDecodedEvent::EvidenceBounty(
            evidence_bounty_from_event(event.event)?,
        )),
        "radroots.knowledge.change_proposal.v1" => {
            Ok(RadrootsDecodedEvent::KnowledgeChangeProposal(
                knowledge_change_proposal_from_event(event.event)?,
            ))
        }
        "radroots.knowledge.contribution_attestation.v1" => {
            Ok(RadrootsDecodedEvent::ContributionAttestation(
                contribution_attestation_from_event(event.event)?,
            ))
        }
        contract_id => Err(RadrootsDecodeError::UnsupportedContract {
            contract_id: contract_id.to_string(),
        }),
    }
}

/// Verify NIP-01 identity, validate the Radroots contract, and decode the event.
///
/// The pipeline is:
/// `RadrootsNostrEvent -> verify_event_id -> RadrootsIdVerifiedEvent ->
/// verify_event_signature -> RadrootsSignatureVerifiedEvent ->
/// validate_event_contract -> RadrootsContractValidatedEvent ->
/// decode_validated_event -> RadrootsDecodedEvent`.
pub fn verify_and_decode_radroots_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsDecodedEvent, RadrootsDecodeError> {
    let id_verified = verify_event_id(event)?;
    let signature_verified = verify_event_signature(id_verified)?;
    let contract_validated = validate_event_contract(signature_verified)?;
    decode_validated_event(contract_validated)
}

#[cfg(feature = "nostr")]
fn raw_event_from_radroots(
    event: &RadrootsNostrEvent,
) -> Result<nostr::Event, RadrootsNip01VerificationError> {
    let id = nostr::EventId::from_hex(event.id.as_str())
        .map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?;
    let public_key = nostr::PublicKey::from_hex(event.author.as_str())
        .map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?;
    let kind_u16 =
        u16::try_from(event.kind).map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?;
    let mut tags = Vec::with_capacity(event.tags.len());
    for tag in event.tags.iter().cloned() {
        tags.push(
            nostr::Tag::parse(tag)
                .map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?,
        );
    }
    let sig = nostr::secp256k1::schnorr::Signature::from_str(event.sig.as_str())
        .map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?;
    Ok(nostr::Event::new(
        id,
        public_key,
        nostr::Timestamp::from_secs(u64::from(event.created_at)),
        nostr::Kind::Custom(kind_u16),
        tags,
        event.content.clone(),
        sig,
    ))
}
