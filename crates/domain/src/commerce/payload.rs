use super::{
    CommerceError,
    value::{check_lines, check_text},
};
use super::{FinalMeasurement, LineQuantity, Resolution};
use crate::{
    AuthorityRef, FulfillmentGroupId, RecordId, Sequence, SourceId, Terms, TermsDigest,
    UnixSeconds, Window,
};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::fmt;
use zeroize::Zeroizing;

macro_rules! copy_getters {
    ($($field:ident:$ty:ty),+ $(,)?)=> { $(
        #[doc=concat!("Returns the explicit `",stringify!($field),"` claim without authenticating it.")]
        pub const fn $field(&self)->$ty { self.$field }
    )+ };
}
macro_rules! simple {
    ($name:ident,$doc:literal,{$($field:ident:$ty:ty),+ $(,)?})=> {
        #[doc=$doc]
        #[derive(Clone,Copy,Eq,PartialEq)]
        pub struct $name { $($field:$ty),+ }
        impl $name {
            /// Captures already validated representations; references remain claims.
            pub const fn new($($field:$ty),+)->Self { Self { $($field),+ } }
            copy_getters!($($field:$ty),+);
        }
    };
}

/// Complete seller quote data, without a hold, signature or buyer consent.
#[derive(Clone, Eq, PartialEq)]
pub struct TermsQuote {
    terms: Box<Terms>,
    expires_at: UnixSeconds,
    in_reply_to: Option<SourceId>,
}
impl TermsQuote {
    /// Captures complete values. Expiry actionability and any referenced chat or
    /// record require independent source/time admission; no clock is observed.
    pub fn new(terms: Terms, expires_at: UnixSeconds, in_reply_to: Option<SourceId>) -> Self {
        Self {
            terms: Box::new(terms),
            expires_at,
            in_reply_to,
        }
    }
    /// Explicitly borrows the complete private terms claim.
    pub const fn terms(&self) -> &Terms {
        &self.terms
    }
    copy_getters!(expires_at:UnixSeconds,in_reply_to:Option<SourceId>);
}
/// Complete buyer request data, without authentication or stock effects.
#[derive(Clone, Eq, PartialEq)]
pub struct TermsRequest {
    terms: Box<Terms>,
    expires_at: UnixSeconds,
    quote: Option<RecordId>,
}
impl TermsRequest {
    /// Captures complete terms and an explicit optional quote reference.
    pub fn new(terms: Terms, expires_at: UnixSeconds, quote: Option<RecordId>) -> Self {
        Self {
            terms: Box::new(terms),
            expires_at,
            quote,
        }
    }
    /// Explicitly borrows the complete private terms claim.
    pub const fn terms(&self) -> &Terms {
        &self.terms
    }
    copy_getters!(expires_at:UnixSeconds,quote:Option<RecordId>);
}
/// A typed record target and bounded private reason. Its enclosing variant
/// identifies whether the target is a request, proposal or authorization.
#[derive(Clone, Eq, PartialEq)]
pub struct RecordReason {
    target: RecordId,
    reason: Zeroizing<String>,
}
impl RecordReason {
    /// Bounds exact reason text before copying; target family needs admission.
    pub fn new(target: RecordId, reason: &str) -> Result<Self, CommerceError> {
        check_text(reason, 1024)?;
        Ok(Self {
            target,
            reason: Zeroizing::new(reason.into()),
        })
    }
    copy_getters!(target:RecordId);
    /// Explicitly borrows the private reason, without terminal/display escaping.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}
/// Complete proposed replacement terms, preserving the declared base reference.
#[derive(Clone, Eq, PartialEq)]
pub struct TermsProposal {
    base_agreement: Option<RecordId>,
    terms: Box<Terms>,
    expires_at: UnixSeconds,
    reason: Zeroizing<String>,
}
impl TermsProposal {
    /// Bounds private text before capture. Consent and base-head admission are separate.
    pub fn new(
        base_agreement: Option<RecordId>,
        terms: Terms,
        expires_at: UnixSeconds,
        reason: &str,
    ) -> Result<Self, CommerceError> {
        check_text(reason, 1024)?;
        Ok(Self {
            base_agreement,
            terms: Box::new(terms),
            expires_at,
            reason: Zeroizing::new(reason.into()),
        })
    }
    copy_getters!(base_agreement:Option<RecordId>,expires_at:UnixSeconds);
    /// Explicitly borrows complete private replacement terms.
    pub const fn terms(&self) -> &Terms {
        &self.terms
    }
    /// Explicitly borrows private proposal text.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}
simple!(TermsAuthorization,"A proposal/terms commitment claim, not participant consent.",{
    proposal:RecordId,terms_digest:TermsDigest,expires_at:UnixSeconds
});
simple!(ProposalAuthorization,"A proposal/expiry claim whose counterparty must be independently verified.",{
    proposal:RecordId,expires_at:UnixSeconds
});

/// Seller commitment data, without authenticated authority or installation.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct AgreementCommitment {
    base_agreement: Option<RecordId>,
    authorization: RecordId,
    terms_digest: TermsDigest,
    authority: AuthorityRef,
    seller_sequence: Sequence,
}
impl AgreementCommitment {
    /// Requires a positive seller sequence; all source/authority evidence is separate.
    pub fn new(
        base_agreement: Option<RecordId>,
        authorization: RecordId,
        terms_digest: TermsDigest,
        authority: AuthorityRef,
        seller_sequence: Sequence,
    ) -> Result<Self, CommerceError> {
        positive(seller_sequence)?;
        Ok(Self {
            base_agreement,
            authorization,
            terms_digest,
            authority,
            seller_sequence,
        })
    }
    copy_getters!(base_agreement:Option<RecordId>,authorization:RecordId,terms_digest:TermsDigest,
        authority:AuthorityRef,seller_sequence:Sequence);
}
/// Serialized seller withdrawal-decision claim, without final race resolution.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct WithdrawalAcknowledgement {
    withdrawal: RecordId,
    seller_sequence: Sequence,
}
impl WithdrawalAcknowledgement {
    /// Requires a positive explicit seller decision sequence.
    pub fn new(withdrawal: RecordId, seller_sequence: Sequence) -> Result<Self, CommerceError> {
        positive(seller_sequence)?;
        Ok(Self {
            withdrawal,
            seller_sequence,
        })
    }
    copy_getters!(withdrawal:RecordId,seller_sequence:Sequence);
}
/// Bounded residual-cancellation proposal, without releasing any stock.
#[derive(Clone, Eq, PartialEq)]
pub struct CancellationProposal {
    agreement: RecordId,
    residual: Box<[LineQuantity]>,
    reason: Zeroizing<String>,
    expires_at: UnixSeconds,
}
impl CancellationProposal {
    /// Requires one through32 distinct line assertions and bounded private text.
    pub fn new(
        agreement: RecordId,
        residual: Vec<LineQuantity>,
        reason: &str,
        expires_at: UnixSeconds,
    ) -> Result<Self, CommerceError> {
        check_lines(&residual, 1)?;
        check_text(reason, 1024)?;
        Ok(Self {
            agreement,
            residual: residual.into_boxed_slice(),
            reason: Zeroizing::new(reason.into()),
            expires_at,
        })
    }
    copy_getters!(agreement:RecordId,expires_at:UnixSeconds);
    /// Borrows exact proposed residual assertions, without assuming current residual.
    pub fn residual(&self) -> &[LineQuantity] {
        &self.residual
    }
    /// Explicitly borrows the private cancellation reason.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}
/// Complete seller residual-release claim; construction does not move stock.
#[derive(Clone, Eq, PartialEq)]
pub struct ResidualRelease {
    proposal: RecordId,
    authorization: RecordId,
    agreement: RecordId,
    residual: Box<[LineQuantity]>,
    authority: AuthorityRef,
    seller_sequence: Sequence,
}
impl ResidualRelease {
    /// Bounds line data and requires a positive seller decision sequence.
    pub fn new(
        proposal: RecordId,
        authorization: RecordId,
        agreement: RecordId,
        residual: Vec<LineQuantity>,
        authority: AuthorityRef,
        seller_sequence: Sequence,
    ) -> Result<Self, CommerceError> {
        check_lines(&residual, 1)?;
        positive(seller_sequence)?;
        Ok(Self {
            proposal,
            authorization,
            agreement,
            residual: residual.into_boxed_slice(),
            authority,
            seller_sequence,
        })
    }
    copy_getters!(proposal:RecordId,authorization:RecordId,agreement:RecordId,authority:AuthorityRef,seller_sequence:Sequence);
    /// Borrows the complete declared release quantities, pending state/consent checks.
    pub fn residual(&self) -> &[LineQuantity] {
        &self.residual
    }
}
/// A seller readiness assertion, without physical handover or buyer receipt.
#[derive(Clone, Eq, PartialEq)]
pub struct ReadinessClaim {
    agreement: RecordId,
    fulfillment_group: FulfillmentGroupId,
    lines: Box<[LineQuantity]>,
    window: Window,
}
impl ReadinessClaim {
    /// Bounds exact declared lines; agreement/group membership needs admission.
    pub fn new(
        agreement: RecordId,
        fulfillment_group: FulfillmentGroupId,
        lines: Vec<LineQuantity>,
        window: Window,
    ) -> Result<Self, CommerceError> {
        check_lines(&lines, 1)?;
        Ok(Self {
            agreement,
            fulfillment_group,
            lines: lines.into_boxed_slice(),
            window,
        })
    }
    copy_getters!(agreement:RecordId,fulfillment_group:FulfillmentGroupId,window:Window);
    /// Borrows complete asserted ready quantities.
    pub fn lines(&self) -> &[LineQuantity] {
        &self.lines
    }
}
/// A seller handover/measurement assertion; it is not physical truth or consumption.
#[derive(Clone, Eq, PartialEq)]
pub struct HandoverClaim {
    agreement: RecordId,
    fulfillment_group: FulfillmentGroupId,
    lines: Box<[LineQuantity]>,
    measurements: Box<[FinalMeasurement]>,
    occurred_at: UnixSeconds,
    authority: AuthorityRef,
    seller_sequence: Sequence,
}
impl HandoverClaim {
    /// Bounds line/measurement data, rejects duplicate or unrelated measurement
    /// line IDs and requires a positive seller decision sequence. Agreement
    /// unit/range/economic/cumulative admission remains mandatory and separate.
    pub fn new(
        agreement: RecordId,
        fulfillment_group: FulfillmentGroupId,
        lines: Vec<LineQuantity>,
        measurements: Vec<FinalMeasurement>,
        occurred_at: UnixSeconds,
        authority: AuthorityRef,
        seller_sequence: Sequence,
    ) -> Result<Self, CommerceError> {
        check_lines(&lines, 1)?;
        positive(seller_sequence)?;
        if measurements.len() > 32 {
            return Err(CommerceError::InvalidMeasurements);
        }
        for (index, measurement) in measurements.iter().enumerate() {
            if !lines
                .iter()
                .any(|line| line.line_id() == measurement.line_id())
                || measurements[..index]
                    .iter()
                    .any(|prior| prior.line_id() == measurement.line_id())
            {
                return Err(CommerceError::InvalidMeasurements);
            }
        }
        Ok(Self {
            agreement,
            fulfillment_group,
            lines: lines.into_boxed_slice(),
            measurements: measurements.into_boxed_slice(),
            occurred_at,
            authority,
            seller_sequence,
        })
    }
    copy_getters!(agreement:RecordId,fulfillment_group:FulfillmentGroupId,occurred_at:UnixSeconds,
        authority:AuthorityRef,seller_sequence:Sequence);
    /// Borrows complete seller quantity assertions.
    pub fn lines(&self) -> &[LineQuantity] {
        &self.lines
    }
    /// Borrows bounded final measurements, still requiring independent price/range checks.
    pub fn measurements(&self) -> &[FinalMeasurement] {
        &self.measurements
    }
}
/// A buyer's independent observation, without seller stock consumption.
#[derive(Clone, Eq, PartialEq)]
pub struct ReceiptObservation {
    handover: RecordId,
    lines: Box<[LineQuantity]>,
    supersedes: Option<RecordId>,
    occurred_at: UnixSeconds,
    note: Zeroizing<String>,
}
impl ReceiptObservation {
    /// Bounds exact observations and private text. Zero observation remains
    /// representable; it never becomes a positive movement or inferred release.
    pub fn new(
        handover: RecordId,
        lines: Vec<LineQuantity>,
        supersedes: Option<RecordId>,
        occurred_at: UnixSeconds,
        note: &str,
    ) -> Result<Self, CommerceError> {
        check_lines(&lines, 1)?;
        check_text(note, 1024)?;
        Ok(Self {
            handover,
            lines: lines.into_boxed_slice(),
            supersedes,
            occurred_at,
            note: Zeroizing::new(note.into()),
        })
    }
    copy_getters!(handover:RecordId,supersedes:Option<RecordId>,occurred_at:UnixSeconds);
    /// Borrows complete asserted observations; supersession is not accumulation.
    pub fn lines(&self) -> &[LineQuantity] {
        &self.lines
    }
    /// Explicitly borrows private receipt text.
    pub fn note(&self) -> &str {
        &self.note
    }
}
/// A complete proposed issue resolution, not mutual authorization or installation.
#[derive(Clone, Eq, PartialEq)]
pub struct ResolutionProposal {
    agreement: RecordId,
    resolution: Resolution,
    expires_at: UnixSeconds,
}
impl ResolutionProposal {
    /// Captures bounded resolution data and explicit source/time claims.
    pub const fn new(agreement: RecordId, resolution: Resolution, expires_at: UnixSeconds) -> Self {
        Self {
            agreement,
            resolution,
            expires_at,
        }
    }
    copy_getters!(agreement:RecordId,expires_at:UnixSeconds);
    /// Explicitly borrows complete private resolution data.
    pub const fn resolution(&self) -> &Resolution {
        &self.resolution
    }
}
/// A seller resolution-commitment claim, without installed effects.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ResolutionCommitment {
    proposal: RecordId,
    authorization: RecordId,
    authority: AuthorityRef,
    seller_sequence: Sequence,
}
impl ResolutionCommitment {
    /// Requires a positive seller decision sequence; consent/authority need admission.
    pub fn new(
        proposal: RecordId,
        authorization: RecordId,
        authority: AuthorityRef,
        seller_sequence: Sequence,
    ) -> Result<Self, CommerceError> {
        positive(seller_sequence)?;
        Ok(Self {
            proposal,
            authorization,
            authority,
            seller_sequence,
        })
    }
    copy_getters!(proposal:RecordId,authorization:RecordId,authority:AuthorityRef,seller_sequence:Sequence);
}
fn positive(sequence: Sequence) -> Result<(), CommerceError> {
    if sequence.get() == 0 {
        return Err(CommerceError::InvalidSequence);
    }
    Ok(())
}
macro_rules! redacted {
    ($($name:ident),+)=> { $(impl fmt::Debug for $name {
        fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result { f.write_str(concat!(stringify!($name),"(<redacted>)")) }
    })+ };
}
redacted!(
    TermsQuote,
    TermsRequest,
    RecordReason,
    TermsProposal,
    TermsAuthorization,
    ProposalAuthorization,
    AgreementCommitment,
    WithdrawalAcknowledgement,
    CancellationProposal,
    ResidualRelease,
    ReadinessClaim,
    HandoverClaim,
    ReceiptObservation,
    ResolutionProposal,
    ResolutionCommitment
);
