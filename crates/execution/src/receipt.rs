//! Owned operation observations. Receipt values grant no authority, prove no
//! custom adapter qualification and never restart a retained signing operation.
use crate::{Admission, Error};
use alloc::boxed::Box;
use core::{fmt, num::NonZeroU64};
use radroots_domain::{
    CheckpointDigest, CheckpointRef, CommandContextData, CommandDigest, EnvelopeEventId,
    IssuanceWindow, MessageId, PublicKey, RecordId, SigningAttemptId, SourceId, TransactionId,
    UnixSeconds,
};
use radroots_protocol::intent::ArtifactRole;

/// Installation is independent of signature and relay observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BusinessInstallation {
    /// This effect class has no business installation.
    NotApplicable,
    /// No installation is confirmed.
    NotInstalled,
    /// Actual installation was confirmed.
    Installed,
    /// Installation completion requires reconciliation.
    Uncertain,
}
/// Issuance classification, without conflating recipients or delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Issuance {
    /// No retained issuance work has been bound.
    None,
    /// Retained work remains before complete obtained evidence.
    Pending,
    /// Every expected artifact has obtained evidence.
    Obtained,
    /// At least one retained invocation lacks a known obtained result.
    Unknown,
    /// Qualified evidence explicitly proves no issuance; absence alone does not.
    ProvablyUnissued,
}
/// Delivery observation progress; observed does not mean every relay accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Delivery {
    /// No exact publication work has started.
    NotStarted,
    /// Retained publication work awaits observations.
    Pending,
    /// The requested observation scope has completed; inspect each target.
    Observed,
    /// Publication completion remains unknown.
    Unknown,
}
/// Why a caller's observation ended, independently of retained operation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WaitTermination {
    /// The caller received this observation, possibly of a pending operation.
    Returned,
    /// The caller stopped waiting.
    Cancelled,
    /// The caller's wait deadline elapsed.
    DeadlineExceeded,
    /// The client closed its observation scope.
    ClientClosed,
}
/// Required continuation is explicit and never permission for blind reissue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RecoveryRequirement {
    /// Continue using the original captured command and retained identity.
    ContinueOriginalOperation,
    /// Reconcile the original potential issuance using its qualified contract.
    ReconcileIssuance,
    /// Install matching already obtained evidence without releasing its claims.
    InstallObtainedEvidence,
    /// Continue retained exact publication work.
    ResumeDelivery,
    /// An operator must reconcile the original state.
    ManualReconciliation,
}
/// One exact event/target observation, separate from business success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TargetObservation {
    /// The target explicitly accepted this exact event.
    Accepted,
    /// The target explicitly rejected this exact event.
    Rejected,
    /// No matching observation arrived before the bounded attempt ended.
    TimedOut,
    /// The transport disconnected without a matching acknowledgement.
    Disconnected,
    /// Completion requires reconciliation.
    Unknown,
}
macro_rules! copied {
    ($name:ident, $ty:ty, $field:ident, $doc:literal) => {
        #[doc=$doc]
        pub const fn $name(&self) -> $ty {
            self.$field
        }
    };
}

/// A protected artifact reference and its independent original issuance facts.
/// No raw signed bytes, ciphertext or private command text is retained here.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ArtifactObservation {
    role: ArtifactRole,
    recipient: Option<PublicKey>,
    expected_event: Option<EnvelopeEventId>,
    issuance: Issuance,
    attempt: Option<SigningAttemptId>,
    claim_transaction: Option<TransactionId>,
    intent_digest: Option<[u8; 32]>,
    capture_transaction: Option<TransactionId>,
    obtained_digest: Option<[u8; 32]>,
    observed_at: Option<NonZeroU64>,
}
impl ArtifactObservation {
    copied!(
        role,
        ArtifactRole,
        role,
        "Exact artifact purpose, not command permission."
    );
    copied!(
        recipient,
        Option<PublicKey>,
        recipient,
        "Exact intended recipient, when private."
    );
    copied!(
        expected_event,
        Option<EnvelopeEventId>,
        expected_event,
        "Exact seal/event identity, never a logical record ID."
    );
    copied!(
        issuance,
        Issuance,
        issuance,
        "This artifact's independent issuance state."
    );
    copied!(
        attempt_id,
        Option<SigningAttemptId>,
        attempt,
        "Original full-width attempt identity."
    );
    copied!(
        claim_transaction_id,
        Option<TransactionId>,
        claim_transaction,
        "Original confirmed claim transaction."
    );
    copied!(
        intent_digest,
        Option<[u8; 32]>,
        intent_digest,
        "Exact original intent commitment."
    );
    copied!(
        capture_transaction_id,
        Option<TransactionId>,
        capture_transaction,
        "Original protected capture transaction."
    );
    copied!(
        obtained_digest,
        Option<[u8; 32]>,
        obtained_digest,
        "Digest of the exact original obtained representation."
    );
    copied!(
        observed_at_unix_ms,
        Option<NonZeroU64>,
        observed_at,
        "Original optional observation; missing remains explicit."
    );
}
impl fmt::Debug for ArtifactObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ArtifactObservation(<redacted>)")
    }
}
/// Actual retained event/target observation. An empty collection reports no
/// delivery observations; selecting a route is not an acknowledgement.
pub struct DeliveryObservation {
    event: EnvelopeEventId,
    recipient: Option<PublicKey>,
    target: Box<str>,
    outcome: TargetObservation,
    observed_at: Option<NonZeroU64>,
}
impl DeliveryObservation {
    copied!(
        event_id,
        EnvelopeEventId,
        event,
        "Exact event whose target observation was checked."
    );
    copied!(
        recipient,
        Option<PublicKey>,
        recipient,
        "Retained recipient scope, when private."
    );
    copied!(
        outcome,
        TargetObservation,
        outcome,
        "Individual target observation; no business effect is implied."
    );
    copied!(
        observed_at_unix_ms,
        Option<NonZeroU64>,
        observed_at,
        "Original optional target observation time."
    );
    /// Borrows the exact retained target for authorized presentation.
    pub fn target(&self) -> &str {
        &self.target
    }
}
impl fmt::Debug for DeliveryObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeliveryObservation(<redacted>)")
    }
}

/// Owned observation of one original operation. It does not authenticate a
/// principal or qualify a custom host. Business, issuance, delivery and caller
/// wait dimensions remain independent; private payloads never enter diagnostics.
pub struct OperationReceipt<T> {
    context: CommandContextData,
    command_digest: CommandDigest,
    admission: Admission,
    admission_transaction: TransactionId,
    plan_transaction: Option<TransactionId>,
    issuance_window: IssuanceWindow,
    authority_checkpoint: CheckpointRef,
    operation_checkpoint: CheckpointDigest,
    conflicting_records: Box<[RecordId]>,
    conflicting_sources: Box<[SourceId]>,
    observed_at: UnixSeconds,
    business_installation: BusinessInstallation,
    business_result: Option<T>,
    message_id: Option<MessageId>,
    record_ids: Box<[RecordId]>,
    artifacts: Box<[ArtifactObservation]>,
    issuance: Issuance,
    delivery: Delivery,
    delivery_observations: Box<[DeliveryObservation]>,
    wait_termination: WaitTermination,
    recovery: Box<[RecoveryRequirement]>,
    error: Option<Error>,
}
impl<T> OperationReceipt<T> {
    /// Original captured data; this is not an authenticated executable context.
    pub const fn context(&self) -> &CommandContextData {
        &self.context
    }
    copied!(
        command_digest,
        CommandDigest,
        command_digest,
        "Original semantic command commitment."
    );
    copied!(
        admission,
        Admission,
        admission,
        "Actual reported initial-admission disposition."
    );
    /// Reports confirmed preparation only, independently of later effects.
    pub const fn durable(&self) -> bool {
        matches!(self.admission, Admission::Confirmed)
    }
    copied!(
        admission_transaction_id,
        TransactionId,
        admission_transaction,
        "Original confirmed preparation transaction."
    );
    copied!(
        plan_transaction_id,
        Option<TransactionId>,
        plan_transaction,
        "Original complete-plan transaction, when present."
    );
    copied!(
        issuance_window,
        IssuanceWindow,
        issuance_window,
        "Original first-issue bounds; never restarted by a receipt."
    );
    /// Authority/evidence checkpoint originally captured by the operation.
    pub const fn authority_checkpoint(&self) -> &CheckpointRef {
        &self.authority_checkpoint
    }
    copied!(
        operation_checkpoint,
        CheckpointDigest,
        operation_checkpoint,
        "Stable identity of this coherent retained progress observation."
    );
    /// Missing sources at the original authority checkpoint; partial coverage
    /// never becomes global completeness because a local query succeeded.
    pub fn missing_sources(&self) -> &[SourceId] {
        self.authority_checkpoint.missing()
    }
    /// Structured-record conflicts in this scoped observation. Empty does not
    /// establish complete remote history.
    pub fn conflicting_records(&self) -> &[RecordId] {
        &self.conflicting_records
    }
    /// Source conflicts in this scoped observation, separate from record IDs.
    pub fn conflicting_sources(&self) -> &[SourceId] {
        &self.conflicting_sources
    }
    copied!(
        observed_at,
        UnixSeconds,
        observed_at,
        "Actual query observation; no global freshness guarantee."
    );
    copied!(
        business_installation,
        BusinessInstallation,
        business_installation,
        "Installation outcome independently of signing and delivery."
    );
    /// Borrows the owned installed business snapshot when actually available.
    pub fn business_result(&self) -> Option<&T> {
        self.business_result.as_ref()
    }
    copied!(
        message_id,
        Option<MessageId>,
        message_id,
        "Original logical message identity, separate from signed event IDs."
    );
    /// Borrows actual structured record IDs; chat message IDs are not cast here.
    pub fn record_ids(&self) -> &[RecordId] {
        &self.record_ids
    }
    /// Borrows independent exact artifact references and issuance observations.
    pub fn artifacts(&self) -> &[ArtifactObservation] {
        &self.artifacts
    }
    copied!(
        issuance,
        Issuance,
        issuance,
        "Aggregate issuance; any unresolved invocation stays unknown."
    );
    copied!(
        delivery,
        Delivery,
        delivery,
        "Delivery progress, never a business rollback signal."
    );
    /// Borrows actual exact event/target observations.
    pub fn delivery_observations(&self) -> &[DeliveryObservation] {
        &self.delivery_observations
    }
    copied!(
        wait_termination,
        WaitTermination,
        wait_termination,
        "Caller waiting is independent of retained effects."
    );
    /// Purely changes this caller observation's wait termination. It does not
    /// cancel, revoke, abort, install or deliver any operation.
    pub fn with_wait_termination(mut self, termination: WaitTermination) -> Self {
        self.wait_termination = termination;
        self
    }
    /// Borrows explicit original-operation recovery requirements.
    pub fn recovery(&self) -> &[RecoveryRequirement] {
        &self.recovery
    }
    copied!(
        error,
        Option<Error>,
        error,
        "Fixed redacted execution cause, when known."
    );
}
impl<T> fmt::Debug for OperationReceipt<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OperationReceipt(<redacted>)")
    }
}

#[cfg(feature = "receipts")]
impl OperationReceipt<()> {
    /// Projects a structurally validated T0/T1/S report. Only the concrete
    /// adapter's actual authenticated coherent query qualifies the facts. This
    /// conversion performs no I/O, issuance, installation or publication.
    pub fn from_snapshot(snapshot: &radroots_storage::command::OperationSnapshot) -> Self {
        use alloc::{vec, vec::Vec};
        let prepared = snapshot.prepared();
        let mut artifacts = Vec::with_capacity(snapshot.recipients().len());
        for recipient in snapshot.recipients() {
            let claim = recipient.claim();
            let obtained = recipient.obtained();
            let issuance = if obtained.is_some() {
                Issuance::Obtained
            } else if claim.is_some() {
                Issuance::Unknown
            } else if snapshot.bound_plan().is_some() {
                Issuance::Pending
            } else {
                Issuance::None
            };
            artifacts.push(ArtifactObservation {
                role: ArtifactRole::BusinessSeal,
                recipient: Some(recipient.recipient()),
                expected_event: recipient.expected_event(),
                issuance,
                attempt: claim.map(|value| value.attempt_id()),
                claim_transaction: claim.map(|value| value.transaction_id()),
                intent_digest: claim.map(|value| *value.intent().digest()),
                capture_transaction: obtained.map(|value| value.transaction_id()),
                obtained_digest: obtained.map(|value| *value.digest()),
                observed_at: obtained.and_then(|value| value.observed_at_unix_ms()),
            });
        }
        let unknown = artifacts
            .iter()
            .any(|value| value.issuance == Issuance::Unknown);
        let obtained = artifacts
            .iter()
            .any(|value| value.issuance == Issuance::Obtained);
        let issuance = if unknown {
            Issuance::Unknown
        } else if artifacts
            .iter()
            .all(|value| value.issuance == Issuance::Obtained)
        {
            Issuance::Obtained
        } else if snapshot.bound_plan().is_some() {
            Issuance::Pending
        } else {
            Issuance::None
        };
        let expired = !unknown
            && issuance != Issuance::Obtained
            && snapshot.observed_at() >= prepared.issuance_window().first_issue_not_after();
        let mut recovery = Vec::new();
        if unknown {
            recovery.push(RecoveryRequirement::ReconcileIssuance);
        }
        if obtained {
            recovery.push(RecoveryRequirement::InstallObtainedEvidence);
        }
        if expired {
            recovery.push(RecoveryRequirement::ManualReconciliation);
        }
        if recovery.is_empty() {
            recovery.push(RecoveryRequirement::ContinueOriginalOperation);
        }
        Self {
            context: prepared.request().command().context().clone(),
            command_digest: prepared.identity().command_digest(),
            admission: Admission::Confirmed,
            admission_transaction: prepared.transaction_id(),
            plan_transaction: snapshot.bound_plan().map(|value| value.transaction_id()),
            issuance_window: prepared.issuance_window(),
            authority_checkpoint: prepared.request().binding().input().checkpoint().clone(),
            operation_checkpoint: snapshot.checkpoint_digest(),
            conflicting_records: vec![].into_boxed_slice(),
            conflicting_sources: vec![].into_boxed_slice(),
            observed_at: snapshot.observed_at(),
            business_installation: BusinessInstallation::NotInstalled,
            business_result: None,
            message_id: snapshot.message_id(),
            record_ids: vec![].into_boxed_slice(),
            artifacts: artifacts.into_boxed_slice(),
            issuance,
            delivery: Delivery::NotStarted,
            delivery_observations: vec![].into_boxed_slice(),
            wait_termination: WaitTermination::Returned,
            recovery: recovery.into_boxed_slice(),
            error: if unknown {
                Some(Error::retained_signing_unknown(
                    prepared.identity().operation_id(),
                ))
            } else if expired {
                Some(Error::retained_deadline_exceeded(
                    prepared.identity().operation_id(),
                ))
            } else {
                None
            },
        }
    }
}
