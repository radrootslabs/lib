//! Stock-owned original delivery reports. Data construction grants no dispatch.
use super::*;
use crate::command::{DeliveryIntent, DeliveryObservationKind};
use core::{fmt, num::NonZeroU64};
use radroots_domain::{EnvelopeEventId, PublicKey, UnixSeconds};
use radroots_protocol::intent::ArtifactRole;
use radroots_transport::{
    DeliveryReceipt, SinkFailure,
    sink::{DeliveryPayload, DeliveryRequest, DeliveryRequestId, DeliveryTargetReceipt},
    target::TargetFingerprint,
};
use std::sync::Arc;

/// Exact original operation/recipient selectors and one requested transport intent.
pub struct StockDeliveryRequest {
    source: StockEnvelopeRequest,
    intent: DeliveryIntent,
}
impl StockDeliveryRequest {
    pub fn new(
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
        recipient: PublicKey,
        intent: DeliveryIntent,
    ) -> Result<Self, OperationError> {
        if intent.role() != ArtifactRole::GiftWrap {
            return Err(OperationError::InvalidCommand);
        }
        Ok(Self {
            source: StockEnvelopeRequest::new(
                binding, minimum, operation, digest, recipient, None,
            )?,
            intent,
        })
    }
    pub const fn source(&self) -> &StockEnvelopeRequest {
        &self.source
    }
    pub const fn intent(&self) -> &DeliveryIntent {
        &self.intent
    }
    pub fn attempt(&self) -> CommitAttempt {
        self.source.attempt()
    }
}
impl fmt::Debug for StockDeliveryRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockDeliveryRequest(<redacted>)")
    }
}

/// Original retained intent and exact outer. Queries of this report cannot
/// authorize transport; only a qualified owner's fresh confirmed claim can.
pub struct StockDeliveryAttempt {
    source: Arc<StockEnvelopeSource>,
    intent: DeliveryIntent,
    transaction: TransactionId,
    retained_at: UnixSeconds,
    checkpoint: InventoryCheckpointRef,
    ordinals: Box<[u8]>,
}
impl StockDeliveryAttempt {
    /// Checks report shape and the actual carrier. The concrete owner must
    /// authenticate the original stock graph and observe transaction completion.
    pub fn from_store_report(
        source: Arc<StockEnvelopeSource>,
        intent: DeliveryIntent,
        transaction: TransactionId,
        retained_at: UnixSeconds,
        checkpoint: InventoryCheckpointRef,
        ordinals: Vec<u8>,
    ) -> Result<Self, OperationError> {
        if intent.role() != ArtifactRole::GiftWrap
            || intent.event_id() != source.receipt().event_id()
            || checkpoint.authority() != source.receipt().checkpoint().authority()
            || checkpoint.revision() <= source.receipt().checkpoint().revision()
            || ordinals.len() != intent.selection().len()
            || ordinals.iter().any(|n| !(1..=12).contains(n))
            || retained_at
                .get()
                .checked_mul(1000)
                .is_none_or(|at| at >= intent.deadline_unix_ms().get())
        {
            return Err(OperationError::RecoveryRequired);
        }
        let outer = source.with_json(|raw| {
            crate::command::verified_private_outer(raw, source.receipt().recipient())
        })?;
        if outer.envelope().created_at_u64() > retained_at.get() {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            source,
            intent,
            transaction,
            retained_at,
            checkpoint,
            ordinals: ordinals.into_boxed_slice(),
        })
    }
    pub fn source(&self) -> &StockEnvelopeSource {
        &self.source
    }
    pub const fn intent(&self) -> &DeliveryIntent {
        &self.intent
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    pub const fn retained_at(&self) -> UnixSeconds {
        self.retained_at
    }
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
    pub fn target_ordinals(&self) -> &[u8] {
        &self.ordinals
    }
    /// Reconstructs the original request without supplying a dispatch permit.
    pub fn transport_request(&self) -> Result<DeliveryRequest, OperationError> {
        let outer = self.source.with_json(|raw| {
            crate::command::verified_private_outer(raw, self.source.receipt().recipient())
        })?;
        DeliveryRequest::new(
            self.intent.request_id().as_str(),
            DeliveryPayload::new(outer),
            self.intent.targets().clone(),
            self.intent.policy().clone(),
            self.intent.deadline_unix_ms().get(),
        )
        .map_err(|_| OperationError::RecoveryRequired)
    }
}
impl fmt::Debug for StockDeliveryAttempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockDeliveryAttempt(<redacted>)")
    }
}

/// Actual store outcome; public construction does not qualify an arbitrary host.
pub enum StockDeliveryAttemptOutcome {
    /// Newly confirmed potential invocation; its qualified owner may proceed once.
    Claimed(Box<StockDeliveryAttempt>),
    /// Original existing request, without another dispatch right.
    Retained(Box<StockDeliveryAttempt>),
    /// Uncertain transaction, without any dispatch right.
    Unknown {
        operation: CommitAttempt,
        request: DeliveryRequestId,
    },
}
impl fmt::Debug for StockDeliveryAttemptOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockDeliveryAttemptOutcome(<redacted>)")
    }
}

/// Independent exact attempt/target outcome, never buyer receipt or stock effect.
#[derive(Clone, Eq, PartialEq)]
pub struct StockDeliveryObservation {
    source: StockEnvelopeReceipt,
    request: DeliveryRequestId,
    attempt: TransactionId,
    checkpoint: InventoryCheckpointRef,
    target: DeliveryTargetReceipt,
    kind: DeliveryObservationKind,
    ordinal: u8,
    observed: Option<NonZeroU64>,
}
impl StockDeliveryObservation {
    pub fn from_receipt(
        attempt: &StockDeliveryAttempt,
        receipt: &DeliveryReceipt,
        target: &TargetFingerprint,
        observed: Option<NonZeroU64>,
    ) -> Result<Self, OperationError> {
        receipt
            .validate_for_request(&attempt.transport_request()?)
            .map_err(|_| OperationError::InvalidEvidence)?;
        let report = receipt
            .target_receipts()
            .iter()
            .find(|report| report.target().fingerprint() == target)
            .ok_or(OperationError::InvalidEvidence)?;
        Self::from_store_report(attempt, report, observed)
    }
    pub fn from_failure(
        attempt: &StockDeliveryAttempt,
        failure: &SinkFailure,
        target: &TargetFingerprint,
        observed: Option<NonZeroU64>,
    ) -> Result<Self, OperationError> {
        failure
            .validate_for_request(&attempt.transport_request()?)
            .map_err(|_| OperationError::InvalidEvidence)?;
        let report = failure
            .partial_evidence()
            .iter()
            .find(|report| report.target().fingerprint() == target)
            .ok_or(OperationError::InvalidEvidence)?;
        Self::from_store_report(attempt, report, observed)
    }
    /// Normalizes bounded data only. The native owner authenticates the actual
    /// original attempt and preserves the first observation of each class.
    pub fn from_store_report(
        attempt: &StockDeliveryAttempt,
        report: &DeliveryTargetReceipt,
        observed: Option<NonZeroU64>,
    ) -> Result<Self, OperationError> {
        let (kind, ordinal) = crate::command::normalize_delivery_observation(
            attempt.intent(),
            attempt.target_ordinals(),
            report,
            observed,
        )?;
        Ok(Self {
            source: attempt.source().receipt(),
            request: attempt.intent().request_id().clone(),
            attempt: attempt.transaction_id(),
            checkpoint: attempt.checkpoint(),
            target: report.clone(),
            kind,
            ordinal,
            observed,
        })
    }
    pub const fn source(&self) -> StockEnvelopeReceipt {
        self.source
    }
    pub const fn request_id(&self) -> &DeliveryRequestId {
        &self.request
    }
    pub const fn event_id(&self) -> EnvelopeEventId {
        self.source.event_id()
    }
    pub const fn attempt_transaction_id(&self) -> TransactionId {
        self.attempt
    }
    pub const fn attempt_checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
    pub const fn target_receipt(&self) -> &DeliveryTargetReceipt {
        &self.target
    }
    pub const fn kind(&self) -> DeliveryObservationKind {
        self.kind
    }
    pub const fn target_ordinal(&self) -> u8 {
        self.ordinal
    }
    pub const fn observed_at_unix_ms(&self) -> Option<NonZeroU64> {
        self.observed
    }
}
impl fmt::Debug for StockDeliveryObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockDeliveryObservation(<redacted>)")
    }
}

/// Actual original observation append metadata, with no remote delivery claim.
#[derive(Clone, Eq, PartialEq)]
pub struct StockDeliveryObservationReceipt {
    observation: StockDeliveryObservation,
    transaction: TransactionId,
    checkpoint: InventoryCheckpointRef,
}
impl StockDeliveryObservationReceipt {
    pub fn from_store_report(
        observation: StockDeliveryObservation,
        transaction: TransactionId,
        checkpoint: InventoryCheckpointRef,
    ) -> Result<Self, OperationError> {
        if checkpoint.authority() != observation.attempt_checkpoint().authority()
            || checkpoint.revision() <= observation.attempt_checkpoint().revision()
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            observation,
            transaction,
            checkpoint,
        })
    }
    pub const fn observation(&self) -> &StockDeliveryObservation {
        &self.observation
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
}
impl fmt::Debug for StockDeliveryObservationReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockDeliveryObservationReceipt(<redacted>)")
    }
}

/// Confirmed original append or unresolved capture of the borrowed original.
pub enum StockDeliveryObservationOutcome {
    Committed(Box<StockDeliveryObservationReceipt>),
    Unknown(CommitAttempt),
}
impl fmt::Debug for StockDeliveryObservationOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockDeliveryObservationOutcome(<redacted>)")
    }
}
