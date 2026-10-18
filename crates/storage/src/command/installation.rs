//! Structural T2 reports. Only an actual qualified transaction establishes
//! installation; constructing these values never grants authority or delivery.
use super::{BoundSigningPlan, CommitAttempt, EvidenceReceipt, OperationError};
use crate::workspace::WorkspaceBinding;
use core::fmt;
use radroots_domain::{
    CommandDigest, EnvelopeEventId, MessageId, OperationId, TransactionId, UnixSeconds, WorkspaceId,
};
use std::boxed::Box;

/// Original-operation selectors for native installation. The concrete owner
/// independently loads and authenticates the complete retained graph.
pub struct InstallationRequest {
    binding: WorkspaceBinding,
    operation: OperationId,
    command: CommandDigest,
    plan: [u8; 32],
}
impl InstallationRequest {
    /// Captures exact selectors, without authorizing installation.
    pub const fn new(
        binding: WorkspaceBinding,
        operation: OperationId,
        command: CommandDigest,
        plan: [u8; 32],
    ) -> Self {
        Self {
            binding,
            operation,
            command,
            plan,
        }
    }
    /// Original selected workspace scope.
    pub const fn binding(&self) -> &WorkspaceBinding {
        &self.binding
    }
    /// Original full operation identity.
    pub const fn operation_id(&self) -> OperationId {
        self.operation
    }
    /// Original semantic command commitment.
    pub const fn command_digest(&self) -> CommandDigest {
        self.command
    }
    /// Exact retained complete signing plan commitment.
    pub const fn plan_digest(&self) -> &[u8; 32] {
        &self.plan
    }
    /// Stable original anchor for an unknown transaction result.
    pub fn identity(&self) -> CommitAttempt {
        CommitAttempt::new(
            self.binding.input().workspace_id(),
            self.operation,
            self.command,
            None,
        )
    }
}
impl fmt::Debug for InstallationRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InstallationRequest(<redacted>)")
    }
}

/// Original confirmed logical installation and its actual obtained basis.
/// The body remains in the protected original command; no ciphertext or text is
/// copied into this fixed reference. This data is not backend qualification.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ChatInstallation {
    plan: [u8; 32],
    message: MessageId,
    preparation_transaction: TransactionId,
    plan_transaction: TransactionId,
    transaction: TransactionId,
    installed_at: UnixSeconds,
    basis: EvidenceReceipt,
}
impl ChatInstallation {
    /// Checks a structural store report against the original complete plan.
    /// The backend must also authenticate the referenced actual obtained bytes
    /// and compare every retained fact in the installation transaction.
    pub fn from_committed_store(
        bound: &BoundSigningPlan,
        basis: EvidenceReceipt,
        transaction: TransactionId,
        installed_at: UnixSeconds,
    ) -> Result<Self, OperationError> {
        check_evidence(bound, basis)?;
        if installed_at < bound.bound_at() {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            plan: *bound.plan().digest(),
            message: bound.plan().message_id(),
            preparation_transaction: bound.prepared().transaction_id(),
            plan_transaction: bound.transaction_id(),
            transaction,
            installed_at,
            basis,
        })
    }
    /// Original workspace.
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.basis.workspace_id()
    }
    /// Original operation.
    pub const fn operation_id(&self) -> OperationId {
        self.basis.operation_id()
    }
    /// Original semantic command commitment.
    pub const fn command_digest(&self) -> CommandDigest {
        self.basis.command_digest()
    }
    /// Exact retained plan commitment.
    pub const fn plan_digest(&self) -> &[u8; 32] {
        &self.plan
    }
    /// Original logical message, distinct from seal/envelope IDs.
    pub const fn message_id(&self) -> MessageId {
        self.message
    }
    /// Original T0 transaction.
    pub const fn preparation_transaction_id(&self) -> TransactionId {
        self.preparation_transaction
    }
    /// Original T1 transaction.
    pub const fn plan_transaction_id(&self) -> TransactionId {
        self.plan_transaction
    }
    /// Original exactly-once logical installation transaction.
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    /// Actual installation clock observation, not the original message time.
    pub const fn installed_at(&self) -> UnixSeconds {
        self.installed_at
    }
    /// Exact original capture that established the installation basis.
    pub const fn basis(&self) -> EvidenceReceipt {
        self.basis
    }
}
impl fmt::Debug for ChatInstallation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChatInstallation(<redacted>)")
    }
}

/// Per-recipient pending wrapping/publication work, referencing an actual
/// retained exact seal and the original plan's selected routes. This is neither
/// a signed outer envelope nor a transport acknowledgement.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PublicationWork {
    installation: TransactionId,
    plan: [u8; 32],
    evidence: EvidenceReceipt,
    transaction: TransactionId,
    queued_at: UnixSeconds,
}
impl PublicationWork {
    /// Checks scoped data only. An actual adapter must authenticate this work
    /// and its evidence and atomically link it to the retained installation.
    pub fn from_committed_store(
        bound: &BoundSigningPlan,
        installation: ChatInstallation,
        evidence: EvidenceReceipt,
        transaction: TransactionId,
        queued_at: UnixSeconds,
    ) -> Result<Self, OperationError> {
        check_evidence(bound, evidence)?;
        if installation.workspace_id() != evidence.workspace_id()
            || installation.operation_id() != evidence.operation_id()
            || installation.command_digest() != evidence.command_digest()
            || installation.plan_digest() != bound.plan().digest()
            || installation.message_id() != bound.plan().message_id()
            || installation.preparation_transaction_id() != bound.prepared().transaction_id()
            || installation.plan_transaction_id() != bound.transaction_id()
            || queued_at < installation.installed_at()
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            installation: installation.transaction_id(),
            plan: *bound.plan().digest(),
            evidence,
            transaction,
            queued_at,
        })
    }
    /// Original installation this work extends without replacing it.
    pub const fn installation_transaction_id(&self) -> TransactionId {
        self.installation
    }
    /// Original immutable plan holding the exact selected routes.
    pub const fn plan_digest(&self) -> &[u8; 32] {
        &self.plan
    }
    /// Exact recipient, seal and protected capture reference.
    pub const fn evidence(&self) -> EvidenceReceipt {
        self.evidence
    }
    /// Confirmed transaction that first retained this recipient's work.
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    /// Actual original work-retention observation.
    pub const fn queued_at(&self) -> UnixSeconds {
        self.queued_at
    }
}
impl fmt::Debug for PublicationWork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PublicationWork(<redacted>)")
    }
}

/// Actual T2 completion or an original retained uncertainty anchor. Applications
/// must reconcile unknown completion before any further effects.
pub enum InstallationOutcome {
    /// The logical installation and eligible publication links are confirmed.
    Committed(Box<ChatInstallation>),
    /// Completion is unknown; retained claims/evidence are never discarded.
    Unknown(CommitAttempt),
}
impl fmt::Debug for InstallationOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Committed(_) => "InstallationOutcome::Committed(<redacted>)",
            Self::Unknown(_) => "InstallationOutcome::Unknown(<retained>)",
        })
    }
}

fn check_evidence(
    bound: &BoundSigningPlan,
    evidence: EvidenceReceipt,
) -> Result<(), OperationError> {
    let identity = bound.prepared().identity();
    let seal = bound
        .plan()
        .seals()
        .iter()
        .find(|seal| seal.recipient() == evidence.recipient())
        .ok_or(OperationError::RecoveryRequired)?;
    if evidence.workspace_id() != identity.workspace_id()
        || evidence.operation_id() != identity.operation_id()
        || evidence.command_digest() != identity.command_digest()
        || evidence.event_id() != EnvelopeEventId::from_bytes(seal.event_id())
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
