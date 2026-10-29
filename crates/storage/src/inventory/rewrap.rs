//! Explicit rewrap invocation identity and non-authoritative recovery reports.
use super::*;
use core::fmt;
use radroots_domain::{Sequence, UnixSeconds};

pub struct StockRewrapRequest {
    source: StockEnvelopeRequest,
    id: OperationId,
}
impl StockRewrapRequest {
    pub const fn new(source: StockEnvelopeRequest, id: OperationId) -> Self {
        Self { source, id }
    }
    pub const fn source(&self) -> &StockEnvelopeRequest {
        &self.source
    }
    pub const fn rewrap_id(&self) -> OperationId {
        self.id
    }
    pub fn with_envelope(mut self, event: radroots_domain::EnvelopeEventId) -> Self {
        self.source = self.source.with_envelope(event);
        self
    }
    pub fn attempt(&self) -> CommitAttempt {
        self.source.attempt()
    }
}
impl fmt::Debug for StockRewrapRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockRewrapRequest(<redacted>)")
    }
}

/// Original confirmed wrapping admission. Only a qualified concrete owner's
/// newly confirmed Claimed outcome can qualify an invocation. This report,
/// an existing admission or an uncertain commit cannot do so.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct StockRewrapAdmission {
    id: OperationId,
    original: StockEnvelopeReceipt,
    transaction: TransactionId,
    at: UnixSeconds,
    expires: UnixSeconds,
    checkpoint: InventoryCheckpointRef,
}
impl StockRewrapAdmission {
    pub fn from_store_report(
        request: &StockRewrapRequest,
        original: StockEnvelopeReceipt,
        transaction: TransactionId,
        at: UnixSeconds,
        expires: UnixSeconds,
        checkpoint: InventoryCheckpointRef,
    ) -> Result<Self, OperationError> {
        let source = request.source();
        let attempt = original.attempt();
        if source.envelope_event_id().is_some()
            || original.rewrap_admission_transaction_id().is_some()
            || attempt.workspace_id() != source.binding().input().workspace_id()
            || attempt.operation_id() != source.operation_id()
            || attempt.command_digest() != source.command_digest()
            || original.recipient() != source.recipient()
            || at.get() == 0
            || expires <= at
            || expires
                .get()
                .checked_sub(at.get())
                .is_none_or(|seconds| seconds > 30)
            || checkpoint.authority() != source.binding().input().authority()
            || original.checkpoint().authority() != checkpoint.authority()
            || checkpoint.revision() <= original.checkpoint().revision()
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            id: request.id,
            original,
            transaction,
            at,
            expires,
            checkpoint,
        })
    }
    pub const fn rewrap_id(&self) -> OperationId {
        self.id
    }
    pub const fn original(&self) -> StockEnvelopeReceipt {
        self.original
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    pub const fn prepared_at(&self) -> UnixSeconds {
        self.at
    }
    pub const fn expires_at(&self) -> UnixSeconds {
        self.expires
    }
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
}
impl fmt::Debug for StockRewrapAdmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockRewrapAdmission(<redacted>)")
    }
}
#[derive(Debug)]
pub enum StockRewrapOutcome {
    Claimed(Box<StockRewrapAdmission>),
    Retained(Box<StockRewrapAdmission>),
    Unknown(CommitAttempt),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockRewrapState {
    admission: StockRewrapAdmission,
    envelope: Option<StockEnvelopeReceipt>,
}
impl StockRewrapState {
    /// Validates shape only. The host must authenticate the actual F13/F14
    /// link. Missing bytes convey no proof of nonissuance or new wrapping right.
    pub fn from_store_report(
        admission: StockRewrapAdmission,
        envelope: Option<StockEnvelopeReceipt>,
    ) -> Result<Self, OperationError> {
        if let Some(envelope) = envelope
            && (envelope.rewrap_admission_transaction_id() != Some(admission.transaction_id())
                || envelope.evidence() != admission.original().evidence()
                || envelope.installation_transaction_id()
                    != admission.original().installation_transaction_id()
                || envelope.checkpoint().authority() != admission.checkpoint().authority()
                || envelope.checkpoint().revision() <= admission.checkpoint().revision())
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            admission,
            envelope,
        })
    }
    pub const fn admission(&self) -> StockRewrapAdmission {
        self.admission
    }
    pub const fn envelope(&self) -> Option<StockEnvelopeReceipt> {
        self.envelope
    }
}

#[derive(Debug)]
pub struct StockRewrapPage {
    entries: Box<[StockRewrapState]>,
    next: Option<Sequence>,
    checkpoint: InventoryCheckpointRef,
}
impl StockRewrapPage {
    pub fn from_store_report(
        request: &StockEnvelopeRequest,
        entries: Vec<StockRewrapState>,
        more: bool,
        checkpoint: InventoryCheckpointRef,
    ) -> Result<Self, OperationError> {
        if entries.len() > 16
            || (more && entries.len() != 16)
            || request.envelope_event_id().is_some()
            || checkpoint.authority() != request.binding().input().authority()
            || checkpoint.revision() < request.minimum_checkpoint().revision()
            || entries.windows(2).any(|pair| {
                pair[0].admission.checkpoint().revision()
                    >= pair[1].admission.checkpoint().revision()
            })
        {
            return Err(OperationError::RecoveryRequired);
        }
        for (index, state) in entries.iter().enumerate() {
            let original = state.admission.original();
            if original.attempt().workspace_id() != request.binding().input().workspace_id()
                || original.attempt().operation_id() != request.operation_id()
                || original.attempt().command_digest() != request.command_digest()
                || original.recipient() != request.recipient()
                || state.admission.checkpoint().authority() != checkpoint.authority()
                || state.admission.checkpoint().revision() > checkpoint.revision()
                || state.envelope.is_some_and(|envelope| {
                    envelope.checkpoint().revision() > checkpoint.revision()
                })
                || entries[..index].iter().any(|prior| {
                    prior.admission.rewrap_id() == state.admission.rewrap_id()
                        || prior.admission.transaction_id() == state.admission.transaction_id()
                })
            {
                return Err(OperationError::RecoveryRequired);
            }
        }
        let next = if more {
            entries
                .last()
                .map(|state| state.admission.checkpoint().revision())
        } else {
            None
        };
        Ok(Self {
            entries: entries.into_boxed_slice(),
            next,
            checkpoint,
        })
    }
    pub fn entries(&self) -> &[StockRewrapState] {
        &self.entries
    }
    pub const fn next_after(&self) -> Option<Sequence> {
        self.next
    }
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
}
