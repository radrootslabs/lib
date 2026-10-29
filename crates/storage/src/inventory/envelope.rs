//! Original stock-envelope selectors and non-authoritative retained reports.
use super::*;
use core::{fmt, num::NonZeroU64};
use radroots_domain::{EnvelopeEventId, PublicKey};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Original recipient selectors and nullable observation; never stock authority.
pub struct StockEnvelopeRequest {
    binding: WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    operation: OperationId,
    digest: CommandDigest,
    recipient: PublicKey,
    observed: Option<NonZeroU64>,
    envelope: Option<EnvelopeEventId>,
}
impl StockEnvelopeRequest {
    pub fn new(
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
        recipient: PublicKey,
        observed: Option<NonZeroU64>,
    ) -> Result<Self, OperationError> {
        if binding.scope() != WorkspaceScope::SellerStockOwner {
            return Err(OperationError::Unauthorized);
        }
        if minimum.authority() != binding.input().authority() {
            return Err(crate::workspace::WorkspaceError::StaleAuthority.into());
        }
        Ok(Self {
            binding,
            minimum,
            operation,
            digest,
            recipient,
            observed,
            envelope: None,
        })
    }
    /// Selects an exact retained outer. This metadata grants neither wrapping
    /// nor transport permission. Omission keeps original-envelope semantics.
    pub const fn with_envelope(mut self, event: EnvelopeEventId) -> Self {
        self.envelope = Some(event);
        self
    }
    pub const fn envelope_event_id(&self) -> Option<EnvelopeEventId> {
        self.envelope
    }
    pub const fn binding(&self) -> &WorkspaceBinding {
        &self.binding
    }
    pub const fn minimum_checkpoint(&self) -> InventoryCheckpointRef {
        self.minimum
    }
    pub const fn operation_id(&self) -> OperationId {
        self.operation
    }
    pub const fn command_digest(&self) -> CommandDigest {
        self.digest
    }
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
    }
    pub const fn observed_at_unix_ms(&self) -> Option<NonZeroU64> {
        self.observed
    }
    pub fn attempt(&self) -> CommitAttempt {
        CommitAttempt::new(
            self.binding.input().workspace_id(),
            self.operation,
            self.digest,
            None,
        )
    }
}
impl fmt::Debug for StockEnvelopeRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockEnvelopeRequest(<redacted>)")
    }
}

/// Actual capture metadata, independent of grant, dispatch or relay acceptance.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct StockEnvelopeReceipt {
    attempt: CommitAttempt,
    evidence: StockEvidenceReceipt,
    installation: TransactionId,
    event: EnvelopeEventId,
    digest: [u8; 32],
    checkpoint: InventoryCheckpointRef,
    observed: Option<NonZeroU64>,
    rewrap_admission: Option<TransactionId>,
}
impl StockEnvelopeReceipt {
    /// Checks scoped report shape only. The concrete host must authenticate
    /// actual original S/T2, containment and confirmed capture atomically.
    pub fn from_committed_store(
        request: &StockEnvelopeRequest,
        attempt: CommitAttempt,
        evidence: StockEvidenceReceipt,
        installation: TransactionId,
        event: EnvelopeEventId,
        digest: [u8; 32],
        checkpoint: InventoryCheckpointRef,
    ) -> Result<Self, OperationError> {
        let source = evidence.attempt();
        if attempt.transaction_id().is_none()
            || attempt.workspace_id() != request.binding().input().workspace_id()
            || attempt.operation_id() != request.operation_id()
            || attempt.command_digest() != request.command_digest()
            || source.workspace_id() != attempt.workspace_id()
            || source.operation_id() != attempt.operation_id()
            || source.command_digest() != attempt.command_digest()
            || evidence.recipient() != request.recipient()
            || checkpoint.authority() != request.binding().input().authority()
            || checkpoint.revision().get() == 0
            || request
                .envelope_event_id()
                .is_some_and(|selected| selected != event)
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            attempt,
            evidence,
            installation,
            event,
            digest,
            checkpoint,
            observed: request.observed_at_unix_ms(),
            rewrap_admission: None,
        })
    }
    pub const fn attempt(&self) -> CommitAttempt {
        self.attempt
    }
    pub const fn evidence(&self) -> StockEvidenceReceipt {
        self.evidence
    }
    pub const fn recipient(&self) -> PublicKey {
        self.evidence.recipient()
    }
    pub const fn installation_transaction_id(&self) -> TransactionId {
        self.installation
    }
    pub const fn event_id(&self) -> EnvelopeEventId {
        self.event
    }
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
    pub const fn observed_at_unix_ms(&self) -> Option<NonZeroU64> {
        self.observed
    }
    pub const fn rewrap_admission_transaction_id(&self) -> Option<TransactionId> {
        self.rewrap_admission
    }
    /// Links a reported additional outer to its original wrapping admission.
    /// Only shape is checked; the qualified host authenticates actual F13/F14.
    pub fn with_rewrap_admission(
        mut self,
        admission: &super::StockRewrapAdmission,
    ) -> Result<Self, OperationError> {
        let original = admission.original();
        if self.evidence != original.evidence()
            || self.installation != original.installation_transaction_id()
            || self.event == original.event_id()
            || self.checkpoint.authority() != admission.checkpoint().authority()
            || self.checkpoint.revision() <= admission.checkpoint().revision()
            || self.attempt.transaction_id() == Some(admission.transaction_id())
            || self
                .rewrap_admission
                .is_some_and(|transaction| transaction != admission.transaction_id())
        {
            return Err(OperationError::RecoveryRequired);
        }
        self.rewrap_admission = Some(admission.transaction_id());
        Ok(self)
    }
}
impl fmt::Debug for StockEnvelopeReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockEnvelopeReceipt(<redacted>)")
    }
}

/// Exact retained outer. Construction verifies its carrier and reported digest,
/// but grants no backend, inventory, dispatch or relay-acceptance authority.
pub struct StockEnvelopeSource {
    receipt: StockEnvelopeReceipt,
    raw: Zeroizing<String>,
}
impl StockEnvelopeSource {
    pub fn from_retained_wire(
        receipt: StockEnvelopeReceipt,
        raw: &str,
    ) -> Result<Self, OperationError> {
        let outer = crate::command::verified_private_outer(raw, receipt.recipient())?;
        if outer.envelope().id().as_bytes() != receipt.event_id().as_bytes()
            || <[u8; 32]>::from(Sha256::digest(raw.as_bytes())) != *receipt.digest()
        {
            return Err(OperationError::InvalidEvidence);
        }
        Ok(Self {
            receipt,
            raw: Zeroizing::new(raw.to_owned()),
        })
    }
    pub const fn receipt(&self) -> StockEnvelopeReceipt {
        self.receipt
    }
    pub fn with_json<T>(&self, use_json: impl FnOnce(&str) -> T) -> T {
        use_json(&self.raw)
    }
}
impl fmt::Debug for StockEnvelopeSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockEnvelopeSource(<redacted>)")
    }
}

/// Confirmed original capture or its actual unresolved transaction identity.
#[derive(Debug)]
pub enum StockEnvelopeOutcome {
    Committed(Box<StockEnvelopeReceipt>),
    Unknown(CommitAttempt),
}
