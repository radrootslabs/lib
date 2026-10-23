use super::{EncodedRecord, RECORD_MAX_BYTES, RecordWireError};
use crate::{
    command::{self, canonical_key, required_nullable},
    profile,
    terms::{self, decode::RawArray},
};
use alloc::{string::String, vec::Vec};
use core::fmt;
use radroots_domain::listing::MeasurementUnit;
use radroots_domain::*;
use serde::Deserialize;
use serde_json::value::RawValue;

/// Complete self-consistent record claims and their checked canonical identity.
/// This value supplies no signature, consent, current authority or stock effect.
pub struct DecodedRecord {
    content: CommerceContent,
    identity: RecordId,
}
impl DecodedRecord {
    /// Explicitly borrows private complete claims without authenticated evidence.
    pub const fn content(&self) -> &CommerceContent {
        &self.content
    }
    /// Returns the verified content hash, distinct from a NIP-01 rumor ID.
    pub const fn record_id(&self) -> RecordId {
        self.identity
    }
    /// Matches final carrier claims only; independent authenticated opening and
    /// recipient binding remain mandatory before this can inform admission.
    pub fn check_rumor_fields(
        &self,
        author: PublicKey,
        created_at: u64,
        kind: u16,
    ) -> Result<(), RecordWireError> {
        if author != self.content.header().author()
            || created_at != self.content.header().created_at().get()
            || kind != profile::COMMERCE_KIND
        {
            return Err(RecordWireError::CarrierMismatch);
        }
        Ok(())
    }
}
impl fmt::Debug for DecodedRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DecodedRecord(<redacted>)")
    }
}
/// Decodes a closed bounded complete profile and checks its record-ID domain.
/// Callers own original private bytes and must retain unsupported raw evidence
/// at their protected boundary; this pure function never stores or discards it.
pub fn decode(raw: &[u8]) -> Result<DecodedRecord, RecordWireError> {
    if raw.len() > RECORD_MAX_BYTES {
        return Err(RecordWireError::LimitExceeded);
    }
    command::check_frame(raw).map_err(command_error)?;
    let probe: ProfileProbe = command::parse_object(raw).map_err(command_error)?;
    if probe.profile != profile::COMMERCE_PROFILE
        || probe.version != profile::PROFILE_VERSION
        || !known_type(&probe.kind)
    {
        return Err(RecordWireError::UnsupportedProfile);
    }
    let value: Record<'_> = command::parse_object(raw).map_err(command_error)?;
    if value.profile != profile::COMMERCE_PROFILE || value.version != profile::PROFILE_VERSION {
        return Err(RecordWireError::UnsupportedProfile);
    }
    let header = invalid(CommerceHeader::new(
        value.thread_id,
        value.order_id,
        value.author,
        value.buyer,
        value.seller,
        value.created_at,
        value.author_sequence,
        value.author_previous,
        identifiers(value.parents)?,
    ))?;
    let content = invalid(CommerceContent::new(
        header,
        payload(&value.kind, value.payload)?,
    ))?;
    let encoded = EncodedRecord::capture(&content)?;
    if encoded.record_id() != value.record_id {
        return Err(RecordWireError::IdentityMismatch);
    }
    Ok(DecodedRecord {
        content,
        identity: encoded.record_id(),
    })
}
#[derive(Deserialize)]
struct ProfileProbe {
    profile: String,
    version: u32,
    #[serde(rename = "type")]
    kind: String,
}
fn known_type(value: &str) -> bool {
    const TYPES: &[CommerceType] = &[
        CommerceType::Quote,
        CommerceType::Request,
        CommerceType::Decline,
        CommerceType::TermsProposed,
        CommerceType::TermsAuthorized,
        CommerceType::TermsRejected,
        CommerceType::AgreementCommitted,
        CommerceType::WithdrawalRequested,
        CommerceType::WithdrawalAcknowledged,
        CommerceType::CancellationProposed,
        CommerceType::CancellationAuthorized,
        CommerceType::ResidualReleased,
        CommerceType::Ready,
        CommerceType::Handover,
        CommerceType::Receipt,
        CommerceType::Issue,
        CommerceType::ResolutionProposed,
        CommerceType::ResolutionAuthorized,
        CommerceType::ResolutionCommitted,
    ];
    TYPES.iter().any(|kind| kind.as_str() == value)
}
fn invalid<T, E>(value: Result<T, E>) -> Result<T, RecordWireError> {
    value.map_err(|_| RecordWireError::InvalidInput)
}
fn command_error(value: command::CommandError) -> RecordWireError {
    match value {
        command::CommandError::LimitExceeded => RecordWireError::LimitExceeded,
        command::CommandError::UnsupportedProfile => RecordWireError::UnsupportedProfile,
        _ => RecordWireError::InvalidInput,
    }
}
fn parse<'a, T: Deserialize<'a>>(raw: &'a RawValue) -> Result<T, RecordWireError> {
    command::parse_object(raw.get().as_bytes()).map_err(command_error)
}
fn identifiers<'a, T: Deserialize<'a>, const N: usize>(
    values: RawArray<'a, N>,
) -> Result<Vec<T>, RecordWireError> {
    values
        .0
        .into_iter()
        .map(|raw| invalid(serde_json::from_str(raw.get())))
        .collect()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record<'a> {
    profile: String,
    version: u32,
    #[serde(rename = "type")]
    kind: String,
    record_id: RecordId,
    thread_id: ThreadId,
    #[serde(deserialize_with = "required_nullable")]
    order_id: Option<OrderId>,
    #[serde(deserialize_with = "canonical_key")]
    author: PublicKey,
    #[serde(deserialize_with = "canonical_key")]
    buyer: PublicKey,
    #[serde(deserialize_with = "canonical_key")]
    seller: PublicKey,
    created_at: UnixSeconds,
    author_sequence: Sequence,
    #[serde(deserialize_with = "required_nullable")]
    author_previous: Option<RecordId>,
    #[serde(borrow)]
    parents: RawArray<'a, 8>,
    #[serde(borrow)]
    payload: &'a RawValue,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Quote<'a> {
    #[serde(borrow)]
    terms: &'a RawValue,
    expires_at: UnixSeconds,
    #[serde(deserialize_with = "required_nullable")]
    in_reply_to: Option<SourceId>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request<'a> {
    #[serde(borrow)]
    terms: &'a RawValue,
    expires_at: UnixSeconds,
    #[serde(deserialize_with = "required_nullable")]
    quote: Option<RecordId>,
}
macro_rules! reason {
    ($name:ident,$field:ident) => {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct $name<'a> {
            $field: RecordId,
            #[serde(borrow)]
            reason: &'a RawValue,
        }
    };
}
reason!(Decline, request);
reason!(Rejection, proposal);
reason!(Withdrawal, authorization);
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Proposal<'a> {
    #[serde(deserialize_with = "required_nullable")]
    base_agreement: Option<RecordId>,
    #[serde(borrow)]
    terms: &'a RawValue,
    expires_at: UnixSeconds,
    #[serde(borrow)]
    reason: &'a RawValue,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TermsApproval {
    proposal: RecordId,
    terms_digest: TermsDigest,
    expires_at: UnixSeconds,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Agreement {
    #[serde(deserialize_with = "required_nullable")]
    base_agreement: Option<RecordId>,
    authorization: RecordId,
    terms_digest: TermsDigest,
    authority: AuthorityRef,
    seller_sequence: Sequence,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WithdrawalAck {
    withdrawal: RecordId,
    seller_sequence: Sequence,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cancellation<'a> {
    agreement: RecordId,
    #[serde(borrow)]
    residual: RawArray<'a, 32>,
    #[serde(borrow)]
    reason: &'a RawValue,
    expires_at: UnixSeconds,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Approval {
    proposal: RecordId,
    expires_at: UnixSeconds,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Release<'a> {
    proposal: RecordId,
    authorization: RecordId,
    agreement: RecordId,
    #[serde(borrow)]
    residual: RawArray<'a, 32>,
    authority: AuthorityRef,
    seller_sequence: Sequence,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready<'a> {
    agreement: RecordId,
    fulfillment_group: FulfillmentGroupId,
    #[serde(borrow)]
    lines: RawArray<'a, 32>,
    #[serde(borrow)]
    window: &'a RawValue,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Handover<'a> {
    agreement: RecordId,
    fulfillment_group: FulfillmentGroupId,
    #[serde(borrow)]
    lines: RawArray<'a, 32>,
    #[serde(borrow)]
    measurements: RawArray<'a, 32>,
    occurred_at: UnixSeconds,
    authority: AuthorityRef,
    seller_sequence: Sequence,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt<'a> {
    handover: RecordId,
    #[serde(borrow)]
    lines: RawArray<'a, 32>,
    #[serde(deserialize_with = "required_nullable")]
    supersedes: Option<RecordId>,
    occurred_at: UnixSeconds,
    #[serde(borrow)]
    note: &'a RawValue,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Issue<'a> {
    code: String,
    #[serde(borrow)]
    description: &'a RawValue,
    #[serde(borrow)]
    lines: RawArray<'a, 32>,
    #[serde(borrow)]
    source_records: RawArray<'a, 32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Resolve<'a> {
    agreement: RecordId,
    #[serde(borrow)]
    resolution: &'a RawValue,
    expires_at: UnixSeconds,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Commitment {
    proposal: RecordId,
    authorization: RecordId,
    authority: AuthorityRef,
    seller_sequence: Sequence,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Line {
    line_id: AgreementLineId,
    amount: Decimal,
}
fn lines(values: RawArray<'_, 32>) -> Result<Vec<LineQuantity>, RecordWireError> {
    values
        .0
        .into_iter()
        .map(|raw| {
            let value: Line = parse(raw)?;
            Ok(LineQuantity::new(value.line_id, value.amount))
        })
        .collect()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Measurement<'a> {
    line_id: AgreementLineId,
    #[serde(borrow)]
    quantity: &'a RawValue,
    #[serde(borrow)]
    line_total: &'a RawValue,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasuredQuantity {
    amount: Decimal,
    unit: String,
}
fn quantity(raw: &RawValue) -> Result<Quantity, RecordWireError> {
    let value: MeasuredQuantity = parse(raw)?;
    let unit = match value.unit.as_str() {
        "g" => MeasurementUnit::Gram,
        "kg" => MeasurementUnit::Kilogram,
        "mL" => MeasurementUnit::Millilitre,
        "L" => MeasurementUnit::Litre,
        "each" => MeasurementUnit::Each,
        _ => return Err(RecordWireError::UnsupportedProfile),
    };
    invalid(Quantity::new(value.amount, unit))
}
fn measurements(values: RawArray<'_, 32>) -> Result<Vec<FinalMeasurement>, RecordWireError> {
    values
        .0
        .into_iter()
        .map(|raw| {
            let value: Measurement<'_> = parse(raw)?;
            Ok(FinalMeasurement::new(
                value.line_id,
                quantity(value.quantity)?,
                terms::decode::money(value.line_total)?,
            ))
        })
        .collect()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolutionInput<'a> {
    #[serde(borrow)]
    issues: RawArray<'a, 32>,
    #[serde(borrow)]
    accepted_receipts: RawArray<'a, 32>,
    #[serde(borrow)]
    release_residual: RawArray<'a, 32>,
    #[serde(borrow)]
    summary: &'a RawValue,
}
fn resolution(raw: &RawValue) -> Result<Resolution, RecordWireError> {
    let value: ResolutionInput<'_> = parse(raw)?;
    let summary = terms::text::decode(value.summary.get(), 2048)?;
    invalid(Resolution::new(
        identifiers(value.issues)?,
        identifiers(value.accepted_receipts)?,
        lines(value.release_residual)?,
        &summary,
    ))
}
fn payload(kind: &str, raw: &RawValue) -> Result<CommercePayload, RecordWireError> {
    Ok(match kind {
        "quote" => {
            let value: Quote<'_> = parse(raw)?;
            CommercePayload::Quote(TermsQuote::new(
                terms::decode(value.terms.get().as_bytes())?,
                value.expires_at,
                value.in_reply_to,
            ))
        }
        "request" => {
            let value: Request<'_> = parse(raw)?;
            CommercePayload::Request(TermsRequest::new(
                terms::decode(value.terms.get().as_bytes())?,
                value.expires_at,
                value.quote,
            ))
        }
        "decline" => {
            let value: Decline<'_> = parse(raw)?;
            let reason = terms::text::decode(value.reason.get(), 1024)?;
            CommercePayload::Decline(invalid(RecordReason::new(value.request, &reason))?)
        }
        "terms_proposed" => {
            let value: Proposal<'_> = parse(raw)?;
            let reason = terms::text::decode(value.reason.get(), 1024)?;
            CommercePayload::TermsProposed(invalid(TermsProposal::new(
                value.base_agreement,
                terms::decode(value.terms.get().as_bytes())?,
                value.expires_at,
                &reason,
            ))?)
        }
        "terms_authorized" => {
            let value: TermsApproval = parse(raw)?;
            CommercePayload::TermsAuthorized(TermsAuthorization::new(
                value.proposal,
                value.terms_digest,
                value.expires_at,
            ))
        }
        "terms_rejected" => {
            let value: Rejection<'_> = parse(raw)?;
            let reason = terms::text::decode(value.reason.get(), 1024)?;
            CommercePayload::TermsRejected(invalid(RecordReason::new(value.proposal, &reason))?)
        }
        "agreement_committed" => {
            let value: Agreement = parse(raw)?;
            CommercePayload::AgreementCommitted(invalid(AgreementCommitment::new(
                value.base_agreement,
                value.authorization,
                value.terms_digest,
                value.authority,
                value.seller_sequence,
            ))?)
        }
        "withdrawal_requested" => {
            let value: Withdrawal<'_> = parse(raw)?;
            let reason = terms::text::decode(value.reason.get(), 1024)?;
            CommercePayload::WithdrawalRequested(invalid(RecordReason::new(
                value.authorization,
                &reason,
            ))?)
        }
        "withdrawal_acknowledged" => {
            let value: WithdrawalAck = parse(raw)?;
            CommercePayload::WithdrawalAcknowledged(invalid(WithdrawalAcknowledgement::new(
                value.withdrawal,
                value.seller_sequence,
            ))?)
        }
        "cancellation_proposed" => {
            let value: Cancellation<'_> = parse(raw)?;
            let reason = terms::text::decode(value.reason.get(), 1024)?;
            CommercePayload::CancellationProposed(invalid(CancellationProposal::new(
                value.agreement,
                lines(value.residual)?,
                &reason,
                value.expires_at,
            ))?)
        }
        "cancellation_authorized" => {
            let value: Approval = parse(raw)?;
            CommercePayload::CancellationAuthorized(ProposalAuthorization::new(
                value.proposal,
                value.expires_at,
            ))
        }
        "residual_released" => {
            let value: Release<'_> = parse(raw)?;
            CommercePayload::ResidualReleased(invalid(ResidualRelease::new(
                value.proposal,
                value.authorization,
                value.agreement,
                lines(value.residual)?,
                value.authority,
                value.seller_sequence,
            ))?)
        }
        "ready" => {
            let value: Ready<'_> = parse(raw)?;
            CommercePayload::Ready(invalid(ReadinessClaim::new(
                value.agreement,
                value.fulfillment_group,
                lines(value.lines)?,
                terms::decode::window(value.window)?,
            ))?)
        }
        "handover" => {
            let value: Handover<'_> = parse(raw)?;
            CommercePayload::Handover(invalid(HandoverClaim::new(
                value.agreement,
                value.fulfillment_group,
                lines(value.lines)?,
                measurements(value.measurements)?,
                value.occurred_at,
                value.authority,
                value.seller_sequence,
            ))?)
        }
        "receipt" => {
            let value: Receipt<'_> = parse(raw)?;
            let note = terms::text::decode(value.note.get(), 1024)?;
            CommercePayload::Receipt(invalid(ReceiptObservation::new(
                value.handover,
                lines(value.lines)?,
                value.supersedes,
                value.occurred_at,
                &note,
            ))?)
        }
        "issue" => {
            let value: Issue<'_> = parse(raw)?;
            let description = terms::text::decode(value.description.get(), 2048)?;
            let code = match value.code.as_str() {
                "shortage" => IssueCode::Shortage,
                "quality" => IssueCode::Quality,
                "substitution" => IssueCode::Substitution,
                "collection" => IssueCode::Collection,
                "excess" => IssueCode::Excess,
                "other" => IssueCode::Other,
                _ => return Err(RecordWireError::UnsupportedProfile),
            };
            CommercePayload::Issue(invalid(IssueReport::new(
                code,
                &description,
                lines(value.lines)?,
                identifiers(value.source_records)?,
            ))?)
        }
        "resolution_proposed" => {
            let value: Resolve<'_> = parse(raw)?;
            CommercePayload::ResolutionProposed(ResolutionProposal::new(
                value.agreement,
                resolution(value.resolution)?,
                value.expires_at,
            ))
        }
        "resolution_authorized" => {
            let value: Approval = parse(raw)?;
            CommercePayload::ResolutionAuthorized(ProposalAuthorization::new(
                value.proposal,
                value.expires_at,
            ))
        }
        "resolution_committed" => {
            let value: Commitment = parse(raw)?;
            CommercePayload::ResolutionCommitted(invalid(ResolutionCommitment::new(
                value.proposal,
                value.authorization,
                value.authority,
                value.seller_sequence,
            ))?)
        }
        _ => return Err(RecordWireError::UnsupportedProfile),
    })
}
