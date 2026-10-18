//! Bounded observations of an already retained exact transport attempt.

use super::{DeliveryAttempt, OperationError, PublicationWork};
use core::{fmt, num::NonZeroU64};
use radroots_domain::EnvelopeEventId;
use radroots_transport::{
    DeliveryReceipt, SinkFailure,
    outcome::DeliveryOutcomeKind,
    sink::{DeliveryRequestId, DeliveryTargetReceipt},
    target::TargetFingerprint,
};

/// Independent append-only outcome classes. Each actual attempt/target has at
/// most one original observation of each class; a later class never erases it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryObservationKind {
    /// The relay accepted this exact outer event, not recipient delivery.
    Accepted,
    /// The relay refused the event.
    Rejected,
    /// The bounded attempt ended without an acknowledgement before its deadline.
    TimedOut,
    /// The connection failed without establishing acceptance.
    Disconnected,
    /// The target result is uncertain or another normalized failure occurred.
    Unknown,
    /// The adapter explicitly reports that it did not attempt this target.
    NotAttempted,
}
impl DeliveryObservationKind {
    /// Fixed protected-record discriminator, never an authorization token.
    pub const fn code(self) -> u8 {
        match self {
            Self::Accepted => 1,
            Self::Rejected => 2,
            Self::TimedOut => 3,
            Self::Disconnected => 4,
            Self::Unknown => 5,
            Self::NotAttempted => 6,
        }
    }
}

/// Exact attempt-bound normalized data. Construction authenticates no adapter
/// or storage. The native owner independently checks actual protected linkage.
#[derive(Clone, Eq, PartialEq)]
pub struct DeliveryObservation {
    work: PublicationWork,
    request: DeliveryRequestId,
    event: EnvelopeEventId,
    envelope_digest: [u8; 32],
    target: DeliveryTargetReceipt,
    kind: DeliveryObservationKind,
    ordinal: u8,
    observed_at: Option<NonZeroU64>,
}
impl DeliveryObservation {
    /// Selects one exact target only after checking the complete receipt's
    /// original request, target set and exact payload binding.
    pub fn from_receipt(
        attempt: &DeliveryAttempt,
        receipt: &DeliveryReceipt,
        target: &TargetFingerprint,
        observed_at_unix_ms: Option<NonZeroU64>,
    ) -> Result<Self, OperationError> {
        receipt
            .validate_for_request(&attempt.transport_request()?)
            .map_err(|_| OperationError::InvalidEvidence)?;
        let report = receipt
            .target_receipts()
            .iter()
            .find(|report| report.target().fingerprint() == target)
            .ok_or(OperationError::InvalidEvidence)?;
        Self::from_store_report(attempt, report, observed_at_unix_ms)
    }

    /// Selects actual partial evidence from a request-bound sink failure.
    /// Missing target evidence remains absent; it is never invented as failure.
    pub fn from_failure(
        attempt: &DeliveryAttempt,
        failure: &SinkFailure,
        target: &TargetFingerprint,
        observed_at_unix_ms: Option<NonZeroU64>,
    ) -> Result<Self, OperationError> {
        failure
            .validate_for_request(&attempt.transport_request()?)
            .map_err(|_| OperationError::InvalidEvidence)?;
        let report = failure
            .partial_evidence()
            .iter()
            .find(|report| report.target().fingerprint() == target)
            .ok_or(OperationError::InvalidEvidence)?;
        Self::from_store_report(attempt, report, observed_at_unix_ms)
    }

    /// Reconstructs bounded data against an original attempt. This is not proof
    /// of execution, remote acknowledgement, current access or a durable row.
    pub fn from_store_report(
        attempt: &DeliveryAttempt,
        report: &DeliveryTargetReceipt,
        observed_at_unix_ms: Option<NonZeroU64>,
    ) -> Result<Self, OperationError> {
        if observed_at_unix_ms.is_some_and(|time| time.get() > 253_402_300_799_999)
            || !attempt
                .intent()
                .targets()
                .targets()
                .contains(report.target())
            || report.outcome().kind() == DeliveryOutcomeKind::Delivered
        {
            return Err(OperationError::InvalidEvidence);
        }
        let kind = if !report.was_attempted() {
            DeliveryObservationKind::NotAttempted
        } else {
            match report.outcome().kind() {
                DeliveryOutcomeKind::Accepted => DeliveryObservationKind::Accepted,
                DeliveryOutcomeKind::Rejected => DeliveryObservationKind::Rejected,
                DeliveryOutcomeKind::Failed | DeliveryOutcomeKind::Unavailable => {
                    match report.outcome().code() {
                        Some("timeout" | "deadline_exceeded") => DeliveryObservationKind::TimedOut,
                        Some("connection_failed" | "disconnected") => {
                            DeliveryObservationKind::Disconnected
                        }
                        _ => DeliveryObservationKind::Unknown,
                    }
                }
                DeliveryOutcomeKind::Delivered => return Err(OperationError::InvalidEvidence),
            }
        };
        let index = attempt
            .intent()
            .selection()
            .iter()
            .position(|target| target == report.target().fingerprint())
            .ok_or(OperationError::InvalidEvidence)?;
        let ordinal = *attempt
            .target_ordinals()
            .get(index)
            .ok_or(OperationError::InvalidEvidence)?;
        Ok(Self {
            work: attempt.work(),
            request: attempt.intent().request_id().clone(),
            event: attempt.intent().event_id(),
            envelope_digest: *attempt.envelope_digest(),
            target: report.clone(),
            kind,
            ordinal,
            observed_at: observed_at_unix_ms,
        })
    }
    /// Original publication scope, independently checked by the native owner.
    pub const fn work(&self) -> PublicationWork {
        self.work
    }
    /// Full original potential invocation identity.
    pub const fn request_id(&self) -> &DeliveryRequestId {
        &self.request
    }
    /// Actual outer identity, separate from logical message and recipient seal.
    pub const fn event_id(&self) -> EnvelopeEventId {
        self.event
    }
    /// Digest of the exact original retained signed representation.
    pub const fn envelope_digest(&self) -> &[u8; 32] {
        &self.envelope_digest
    }
    /// Original normalized target evidence, for protected handling only.
    pub const fn target_receipt(&self) -> &DeliveryTargetReceipt {
        &self.target
    }
    /// Independent observation class; it changes no command installation fact.
    pub const fn kind(&self) -> DeliveryObservationKind {
        self.kind
    }
    /// Original potential invocation number for this exact selected endpoint.
    pub const fn target_ordinal(&self) -> u8 {
        self.ordinal
    }
    /// First original optional observation time. No ambient clock is consulted.
    pub const fn observed_at_unix_ms(&self) -> Option<NonZeroU64> {
        self.observed_at
    }
}
impl fmt::Debug for DeliveryObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeliveryObservation(<redacted>)")
    }
}

/// Actual owner outcome, separate from the observation's remote meaning.
pub enum DeliveryObservationOutcome {
    /// Confirmed append or matching original first observation.
    Committed(DeliveryObservation),
    /// Reconcile this original observation before inferring durable capture.
    Unknown(DeliveryObservation),
}
impl fmt::Debug for DeliveryObservationOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeliveryObservationOutcome(<redacted>)")
    }
}
