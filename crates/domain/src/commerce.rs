//! Carrier-independent private record claims. These values provide no content
//! hash, signature, authenticated parent coverage, consent or installed effects.
use crate::{OrderId, PublicKey, RecordId, Sequence, SourceId, Terms, ThreadId, UnixSeconds};
use alloc::{boxed::Box, vec::Vec};
use core::fmt;

mod value;
pub use value::{FinalMeasurement, IssueCode, IssueReport, LineQuantity, Resolution};
mod payload;
pub use payload::{
    AgreementCommitment, CancellationProposal, HandoverClaim, ProposalAuthorization,
    ReadinessClaim, ReceiptObservation, RecordReason, ResidualRelease, ResolutionCommitment,
    ResolutionProposal, TermsAuthorization, TermsProposal, TermsQuote, TermsRequest,
    WithdrawalAcknowledgement,
};

/// Exact declared common record data; construction authenticates no participant.
#[derive(Clone, Eq, PartialEq)]
pub struct CommerceHeader {
    thread_id: ThreadId,
    order_id: Option<OrderId>,
    author: PublicKey,
    buyer: PublicKey,
    seller: PublicKey,
    created_at: UnixSeconds,
    author_sequence: Sequence,
    author_previous: Option<RecordId>,
    parents: Box<[SourceId]>,
}
impl CommerceHeader {
    /// Largest supported direct causal-parent set. Payload/source limits apply
    /// jointly; referenced data must never be silently omitted to fit this cap.
    pub const MAX_PARENTS: usize = 8;
    /// Validates declared participants, positive sequence/previous shape and a
    /// sorted unique parent set before capture. Previous author/thread/order,
    /// source signatures, sequence continuity and actual coverage need admission.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        thread_id: ThreadId,
        order_id: Option<OrderId>,
        author: PublicKey,
        buyer: PublicKey,
        seller: PublicKey,
        created_at: UnixSeconds,
        author_sequence: Sequence,
        author_previous: Option<RecordId>,
        parents: Vec<SourceId>,
    ) -> Result<Self, CommerceError> {
        if buyer == seller {
            return Err(CommerceError::PartiesCoincide);
        }
        if author != buyer && author != seller {
            return Err(CommerceError::InvalidAuthor);
        }
        if author_sequence.get() == 0 {
            return Err(CommerceError::InvalidSequence);
        }
        if (author_sequence.get() == 1) != author_previous.is_none() {
            return Err(CommerceError::InvalidPrevious);
        }
        if parents.len() > Self::MAX_PARENTS || parents.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(CommerceError::InvalidParents);
        }
        if let Some(previous) = author_previous
            && !parents.contains(&source(previous))
        {
            return Err(CommerceError::MissingParent);
        }
        Ok(Self {
            thread_id,
            order_id,
            author,
            buyer,
            seller,
            created_at,
            author_sequence,
            author_previous,
            parents: parents.into_boxed_slice(),
        })
    }
    /// Returns the exact application thread claim.
    pub const fn thread_id(&self) -> ThreadId {
        self.thread_id
    }
    /// Returns the explicit order claim; only quotes may omit it in content.
    pub const fn order_id(&self) -> Option<OrderId> {
        self.order_id
    }
    /// Returns the claimed author, without signer or delegation proof.
    pub const fn author(&self) -> PublicKey {
        self.author
    }
    /// Returns the distinct buyer claim.
    pub const fn buyer(&self) -> PublicKey {
        self.buyer
    }
    /// Returns the distinct seller claim.
    pub const fn seller(&self) -> PublicKey {
        self.seller
    }
    /// Returns the claimed exact author time, without observing a clock.
    pub const fn created_at(&self) -> UnixSeconds {
        self.created_at
    }
    /// Returns the positive declared author sequence, not verified continuity.
    pub const fn author_sequence(&self) -> Sequence {
        self.author_sequence
    }
    /// Returns the explicit previous-record claim; sequence1 requires None.
    pub const fn author_previous(&self) -> Option<RecordId> {
        self.author_previous
    }
    /// Borrows sorted unique full digests. Generic source identity does not
    /// convert a chat rumor, structured record or signed carrier into another.
    pub fn parents(&self) -> &[SourceId] {
        &self.parents
    }
    fn require(&self, parent: SourceId) -> Result<(), CommerceError> {
        if self.parents.contains(&parent) {
            Ok(())
        } else {
            Err(CommerceError::MissingParent)
        }
    }
    fn require_record(&self, parent: RecordId) -> Result<(), CommerceError> {
        self.require(source(parent))
    }
    fn require_optional(&self, parent: Option<RecordId>) -> Result<(), CommerceError> {
        match parent {
            Some(parent) => self.require_record(parent),
            None => Ok(()),
        }
    }
    fn terms(&self, terms: &Terms) -> Result<(), CommerceError> {
        if terms.buyer() != self.buyer || terms.seller() != self.seller {
            return Err(CommerceError::TermsPartiesMismatch);
        }
        for line in terms.lines() {
            self.require(SourceId::from_bytes(line.source().event_id().into_bytes()))?;
        }
        Ok(())
    }
}

/// Closed logical record-family vocabulary; this is not a Nostr numeric kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CommerceType {
    /// Complete seller quote.
    Quote,
    /// Complete buyer request.
    Request,
    /// Seller refusal of a request.
    Decline,
    /// Complete initial/replacement terms proposal.
    TermsProposed,
    /// Counterparty terms authorization claim.
    TermsAuthorized,
    /// Counterparty proposal rejection claim.
    TermsRejected,
    /// Seller agreement commitment claim.
    AgreementCommitted,
    /// Buyer request to withdraw an authorization.
    WithdrawalRequested,
    /// Seller withdrawal-decision acknowledgement.
    WithdrawalAcknowledged,
    /// Bilateral residual-cancellation proposal.
    CancellationProposed,
    /// Counterparty cancellation authorization claim.
    CancellationAuthorized,
    /// Seller residual-release claim.
    ResidualReleased,
    /// Seller readiness assertion.
    Ready,
    /// Seller handover assertion.
    Handover,
    /// Independent buyer observation.
    Receipt,
    /// Participant issue report.
    Issue,
    /// Complete issue-resolution proposal.
    ResolutionProposed,
    /// Counterparty resolution authorization claim.
    ResolutionAuthorized,
    /// Seller resolution-commitment claim.
    ResolutionCommitted,
}
impl CommerceType {
    /// Returns the exact closed content token, never a placeholder rumor kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Quote => "quote",
            Self::Request => "request",
            Self::Decline => "decline",
            Self::TermsProposed => "terms_proposed",
            Self::TermsAuthorized => "terms_authorized",
            Self::TermsRejected => "terms_rejected",
            Self::AgreementCommitted => "agreement_committed",
            Self::WithdrawalRequested => "withdrawal_requested",
            Self::WithdrawalAcknowledged => "withdrawal_acknowledged",
            Self::CancellationProposed => "cancellation_proposed",
            Self::CancellationAuthorized => "cancellation_authorized",
            Self::ResidualReleased => "residual_released",
            Self::Ready => "ready",
            Self::Handover => "handover",
            Self::Receipt => "receipt",
            Self::Issue => "issue",
            Self::ResolutionProposed => "resolution_proposed",
            Self::ResolutionAuthorized => "resolution_authorized",
            Self::ResolutionCommitted => "resolution_committed",
        }
    }
    fn author(self, header: &CommerceHeader) -> Result<(), CommerceError> {
        let valid = match self {
            Self::Quote
            | Self::Decline
            | Self::AgreementCommitted
            | Self::WithdrawalAcknowledged
            | Self::ResidualReleased
            | Self::Ready
            | Self::Handover
            | Self::ResolutionCommitted => header.author == header.seller,
            Self::Request | Self::WithdrawalRequested | Self::Receipt => {
                header.author == header.buyer
            }
            // The exact consenting/rejecting counterparty is determined from
            // authenticated parent proposals, never just these declarations.
            Self::TermsProposed
            | Self::TermsAuthorized
            | Self::TermsRejected
            | Self::CancellationProposed
            | Self::CancellationAuthorized
            | Self::Issue
            | Self::ResolutionProposed
            | Self::ResolutionAuthorized => true,
        };
        if valid {
            Ok(())
        } else {
            Err(CommerceError::InvalidAuthor)
        }
    }
}

/// Complete typed private payload alternatives. Each nested value is bounded;
/// its asserted sources, authority and physical facts still need admission.
#[derive(Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum CommercePayload {
    /// Complete seller quote with an optional generic inquiry/source reference.
    Quote(TermsQuote),
    /// Complete buyer request and optional exact structured quote.
    Request(TermsRequest),
    /// Request target and private refusal reason.
    Decline(RecordReason),
    /// Complete proposed terms and explicit optional base agreement.
    TermsProposed(TermsProposal),
    /// Exact proposal and complete-terms commitment claim.
    TermsAuthorized(TermsAuthorization),
    /// Exact proposal and private rejection reason.
    TermsRejected(RecordReason),
    /// Complete seller commitment metadata.
    AgreementCommitted(AgreementCommitment),
    /// Exact authorization and private withdrawal reason.
    WithdrawalRequested(RecordReason),
    /// Seller's explicit withdrawal-decision metadata.
    WithdrawalAcknowledged(WithdrawalAcknowledgement),
    /// Complete proposed residual release.
    CancellationProposed(CancellationProposal),
    /// Exact cancellation-proposal authorization claim.
    CancellationAuthorized(ProposalAuthorization),
    /// Complete seller residual-release metadata.
    ResidualReleased(ResidualRelease),
    /// Complete seller readiness assertion.
    Ready(ReadinessClaim),
    /// Complete seller handover/measurement assertion.
    Handover(HandoverClaim),
    /// Independent buyer receipt assertion.
    Receipt(ReceiptObservation),
    /// Complete bounded participant issue report.
    Issue(IssueReport),
    /// Complete proposed issue resolution.
    ResolutionProposed(ResolutionProposal),
    /// Exact resolution-proposal authorization claim.
    ResolutionAuthorized(ProposalAuthorization),
    /// Seller's complete resolution-commitment metadata.
    ResolutionCommitted(ResolutionCommitment),
}
impl CommercePayload {
    /// Returns the logical content family, without assigning a Nostr carrier kind.
    pub const fn record_type(&self) -> CommerceType {
        match self {
            Self::Quote(_) => CommerceType::Quote,
            Self::Request(_) => CommerceType::Request,
            Self::Decline(_) => CommerceType::Decline,
            Self::TermsProposed(_) => CommerceType::TermsProposed,
            Self::TermsAuthorized(_) => CommerceType::TermsAuthorized,
            Self::TermsRejected(_) => CommerceType::TermsRejected,
            Self::AgreementCommitted(_) => CommerceType::AgreementCommitted,
            Self::WithdrawalRequested(_) => CommerceType::WithdrawalRequested,
            Self::WithdrawalAcknowledged(_) => CommerceType::WithdrawalAcknowledged,
            Self::CancellationProposed(_) => CommerceType::CancellationProposed,
            Self::CancellationAuthorized(_) => CommerceType::CancellationAuthorized,
            Self::ResidualReleased(_) => CommerceType::ResidualReleased,
            Self::Ready(_) => CommerceType::Ready,
            Self::Handover(_) => CommerceType::Handover,
            Self::Receipt(_) => CommerceType::Receipt,
            Self::Issue(_) => CommerceType::Issue,
            Self::ResolutionProposed(_) => CommerceType::ResolutionProposed,
            Self::ResolutionAuthorized(_) => CommerceType::ResolutionAuthorized,
            Self::ResolutionCommitted(_) => CommerceType::ResolutionCommitted,
        }
    }
    fn references(&self, header: &CommerceHeader) -> Result<(), CommerceError> {
        match self {
            Self::Quote(value) => {
                header.terms(value.terms())?;
                if let Some(parent) = value.in_reply_to() {
                    header.require(parent)?;
                }
            }
            Self::Request(value) => {
                header.terms(value.terms())?;
                header.require_optional(value.quote())?;
            }
            Self::Decline(value)
            | Self::TermsRejected(value)
            | Self::WithdrawalRequested(value) => header.require_record(value.target())?,
            Self::TermsProposed(value) => {
                header.terms(value.terms())?;
                header.require_optional(value.base_agreement())?;
            }
            Self::TermsAuthorized(value) => header.require_record(value.proposal())?,
            Self::AgreementCommitted(value) => {
                header.require_optional(value.base_agreement())?;
                header.require_record(value.authorization())?;
            }
            Self::WithdrawalAcknowledged(value) => header.require_record(value.withdrawal())?,
            Self::CancellationProposed(value) => header.require_record(value.agreement())?,
            Self::CancellationAuthorized(value) | Self::ResolutionAuthorized(value) => {
                header.require_record(value.proposal())?
            }
            Self::ResidualReleased(value) => {
                header.require_record(value.proposal())?;
                header.require_record(value.authorization())?;
                header.require_record(value.agreement())?;
            }
            Self::Ready(value) => header.require_record(value.agreement())?,
            Self::Handover(value) => header.require_record(value.agreement())?,
            Self::Receipt(value) => {
                header.require_record(value.handover())?;
                header.require_optional(value.supersedes())?;
            }
            Self::Issue(value) => {
                for parent in value.source_records() {
                    header.require_record(*parent)?;
                }
            }
            Self::ResolutionProposed(value) => {
                header.require_record(value.agreement())?;
                for parent in value
                    .resolution()
                    .issues()
                    .iter()
                    .chain(value.resolution().accepted_receipts())
                {
                    header.require_record(*parent)?;
                }
            }
            Self::ResolutionCommitted(value) => {
                header.require_record(value.proposal())?;
                header.require_record(value.authorization())?;
            }
        }
        Ok(())
    }
}

/// Complete version1 logical content before protocol-owned canonical record-ID
/// computation. No automatic serialization, authentication or effect is exposed.
#[derive(Clone, Eq, PartialEq)]
pub struct CommerceContent {
    header: CommerceHeader,
    payload: CommercePayload,
}
impl CommerceContent {
    /// Checks self-contained declared role/order/terms/reference consistency.
    /// Parent authentication, exact cross-record scope/roles/coverage, deadline
    /// actionability, terms digests and agreement-relative economic/state checks
    /// remain mandatory before any consent, reduction, signing or stock effects.
    pub fn new(header: CommerceHeader, payload: CommercePayload) -> Result<Self, CommerceError> {
        let record_type = payload.record_type();
        if record_type != CommerceType::Quote && header.order_id.is_none() {
            return Err(CommerceError::InvalidOrder);
        }
        record_type.author(&header)?;
        payload.references(&header)?;
        Ok(Self { header, payload })
    }
    /// Returns the exact fixed supported content profile identity.
    pub const fn profile(&self) -> &'static str {
        "radroots.commerce"
    }
    /// Returns the exact fixed supported content version.
    pub const fn version(&self) -> u32 {
        1
    }
    /// Explicitly borrows the original declared causal/scope metadata.
    pub const fn header(&self) -> &CommerceHeader {
        &self.header
    }
    /// Explicitly borrows complete private payload data.
    pub const fn payload(&self) -> &CommercePayload {
        &self.payload
    }
    /// Returns the closed logical family; no numeric rumor kind is assigned.
    pub const fn record_type(&self) -> CommerceType {
        self.payload.record_type()
    }
}
fn source(record: RecordId) -> SourceId {
    SourceId::from_bytes(record.into_bytes())
}

/// Input-free record-value validation failure; no private text is retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CommerceError {
    /// The declared buyer and seller are the same participant.
    PartiesCoincide,
    /// The declared author is not a participant or has the wrong fixed role.
    InvalidAuthor,
    /// A stateful order record lacks an order identity.
    InvalidOrder,
    /// A declared author/seller sequence is zero.
    InvalidSequence,
    /// First/later author sequence has the wrong explicit previous-link shape.
    InvalidPrevious,
    /// Parent references exceed8 or are not strictly sorted and unique.
    InvalidParents,
    /// A declared source required by this value is absent from the parent set.
    MissingParent,
    /// Complete terms name different participants from their common header.
    TermsPartiesMismatch,
    /// Bounded private text exceeds its declared character/UTF-8 limits.
    InvalidText,
    /// A line set has invalid count or duplicate line identities.
    InvalidLines,
    /// Measurements exceed bounds, repeat a line or name a line outside the claim.
    InvalidMeasurements,
    /// A source/reference set has invalid count or duplicate identities.
    InvalidReferences,
}
impl fmt::Display for CommerceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PartiesCoincide => "commerce participants must be distinct",
            Self::InvalidAuthor => "invalid declared commerce author",
            Self::InvalidOrder => "commerce order identity required",
            Self::InvalidSequence => "commerce sequence must be positive",
            Self::InvalidPrevious => "invalid previous commerce record",
            Self::InvalidParents => "invalid commerce parent set",
            Self::MissingParent => "required commerce parent not declared",
            Self::TermsPartiesMismatch => "commerce terms participants differ",
            Self::InvalidText => "commerce text exceeds bound",
            Self::InvalidLines => "invalid commerce line set",
            Self::InvalidMeasurements => "invalid commerce measurements",
            Self::InvalidReferences => "invalid commerce references",
        })
    }
}
impl core::error::Error for CommerceError {}
macro_rules! redacted {
    ($($name:ident),+)=> { $(impl fmt::Debug for $name {
        fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result { f.write_str(concat!(stringify!($name),"(<redacted>)")) }
    })+ };
}
redacted!(CommerceHeader, CommercePayload, CommerceContent);
