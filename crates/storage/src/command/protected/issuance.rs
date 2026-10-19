//! Opaque original claim and obtained-evidence metadata for protecting hosts.
//! Decoding supplies no current access, actual commit or dispatch permission.
use super::plan::PART_BYTES;
use crate::command::{BoundSigningPlan, OperationError, SealEvidence, SigningAttempt};
use core::num::NonZeroU64;
use radroots_domain::{
    CommandDigest, EnvelopeEventId, GrantId, OperationId, PublicKey, SigningAttemptId,
    TransactionId, UnixSeconds, WorkspaceId,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct ClaimWire {
    version: u8,
    workspace: WorkspaceId,
    operation_id: OperationId,
    command_digest: CommandDigest,
    plan_digest: [u8; 32],
    recipient: PublicKey,
    attempt_id: SigningAttemptId,
    transaction_id: TransactionId,
    claimed_at: UnixSeconds,
    intent_digest: [u8; 32],
    deadline_unix_ms: u64,
    preparation_transaction: TransactionId,
    plan_transaction: TransactionId,
    grant_id: GrantId,
    expected_event: EnvelopeEventId,
}
impl ClaimWire {
    fn new(attempt: &SigningAttempt) -> Self {
        let plan = attempt.bound_plan();
        let prepared = plan.prepared();
        let identity = prepared.identity();
        Self {
            version: 1,
            workspace: identity.workspace_id(),
            operation_id: identity.operation_id(),
            command_digest: identity.command_digest(),
            plan_digest: *plan.plan().digest(),
            recipient: attempt.seal().recipient(),
            attempt_id: attempt.attempt_id(),
            transaction_id: attempt.transaction_id(),
            claimed_at: attempt.claimed_at(),
            intent_digest: *attempt.binding().digest(),
            deadline_unix_ms: attempt.deadline_unix_ms().get(),
            preparation_transaction: prepared.transaction_id(),
            plan_transaction: plan.transaction_id(),
            grant_id: prepared.request().binding().input().grant_id(),
            expected_event: EnvelopeEventId::from_bytes(attempt.seal().event_id()),
        }
    }
}
/// Closed v1 claim codec. The concrete host still verifies its retained indexes
/// and acknowledges actual completion before reporting a newly claimed attempt.
pub struct ClaimRecord(ClaimWire);
impl ClaimRecord {
    pub fn capture(attempt: &SigningAttempt) -> Self {
        Self(ClaimWire::new(attempt))
    }
    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>, OperationError> {
        encode(&self.0)
    }
    pub fn decode(
        raw: &[u8],
        bound: BoundSigningPlan,
        recipient: PublicKey,
    ) -> Result<SigningAttempt, OperationError> {
        if raw.len() > PART_BYTES {
            return Err(OperationError::RecoveryRequired);
        }
        let wire: ClaimWire =
            serde_json::from_slice(raw).map_err(|_| OperationError::RecoveryRequired)?;
        let attempt = SigningAttempt::from_committed_store(
            bound,
            recipient,
            wire.attempt_id,
            wire.transaction_id,
            wire.claimed_at,
        )?;
        if wire != ClaimWire::new(&attempt) {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(attempt)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceWire {
    version: u8,
    workspace: WorkspaceId,
    operation_id: OperationId,
    command_digest: CommandDigest,
    plan_digest: [u8; 32],
    recipient: PublicKey,
    attempt_id: SigningAttemptId,
    claim_transaction: TransactionId,
    intent_digest: [u8; 32],
    event_id: EnvelopeEventId,
    evidence_digest: [u8; 32],
    transaction_id: TransactionId,
    observed_at: Option<u64>,
    byte_count: usize,
    part_count: usize,
}
/// Complete authenticated obtained-seal metadata, preserving the original
/// optional observation. Each clear part remains independently bounded.
pub struct EvidenceManifest(EvidenceWire);
impl EvidenceManifest {
    pub fn capture(
        attempt: &SigningAttempt,
        evidence: &SealEvidence,
        transaction: TransactionId,
    ) -> Result<Self, OperationError> {
        if !evidence.matches_attempt(attempt) {
            return Err(OperationError::InvalidEvidence);
        }
        let result = Self(EvidenceWire {
            version: 1,
            workspace: evidence.workspace_id(),
            operation_id: evidence.operation_id(),
            command_digest: evidence.command_digest(),
            plan_digest: *evidence.plan_digest(),
            recipient: evidence.recipient(),
            attempt_id: evidence.attempt_id(),
            claim_transaction: attempt.transaction_id(),
            intent_digest: *evidence.intent().digest(),
            event_id: evidence.event_id(),
            evidence_digest: *evidence.digest(),
            transaction_id: transaction,
            observed_at: evidence.observed_at_unix_ms().map(NonZeroU64::get),
            byte_count: evidence.raw_json().len(),
            part_count: evidence.raw_json().len().div_ceil(PART_BYTES),
        });
        result.validate(attempt)?;
        Ok(result)
    }
    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>, OperationError> {
        encode(&self.0)
    }
    pub fn decode(raw: &[u8], attempt: &SigningAttempt) -> Result<Self, OperationError> {
        if raw.len() > PART_BYTES {
            return Err(OperationError::RecoveryRequired);
        }
        let result =
            Self(serde_json::from_slice(raw).map_err(|_| OperationError::RecoveryRequired)?);
        result.validate(attempt)?;
        Ok(result)
    }
    fn validate(&self, attempt: &SigningAttempt) -> Result<(), OperationError> {
        let w = &self.0;
        let bound = attempt.bound_plan();
        let identity = bound.prepared().identity();
        if w.version != 1
            || w.workspace != identity.workspace_id()
            || w.operation_id != identity.operation_id()
            || w.command_digest != identity.command_digest()
            || &w.plan_digest != bound.plan().digest()
            || w.recipient != attempt.seal().recipient()
            || w.attempt_id != attempt.attempt_id()
            || w.claim_transaction != attempt.transaction_id()
            || &w.intent_digest != attempt.binding().digest()
            || w.event_id.as_bytes() != &attempt.seal().event_id()
            || !(1..=65536).contains(&w.byte_count)
            || !(1..=4).contains(&w.part_count)
            || w.byte_count.div_ceil(PART_BYTES) != w.part_count
            || w.observed_at
                .is_some_and(|t| t == 0 || t > 253_402_300_799_999)
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.0.transaction_id
    }
    pub const fn attempt_id(&self) -> SigningAttemptId {
        self.0.attempt_id
    }
    pub const fn event_id(&self) -> EnvelopeEventId {
        self.0.event_id
    }
    pub const fn evidence_digest(&self) -> &[u8; 32] {
        &self.0.evidence_digest
    }
    pub const fn byte_count(&self) -> usize {
        self.0.byte_count
    }
    pub const fn part_count(&self) -> usize {
        self.0.part_count
    }
    pub fn verify(
        &self,
        attempt: &SigningAttempt,
        raw: &str,
    ) -> Result<SealEvidence, OperationError> {
        self.validate(attempt)?;
        if raw.len() != self.byte_count() {
            return Err(OperationError::RecoveryRequired);
        }
        let evidence =
            SealEvidence::verify(attempt, raw, self.0.observed_at.and_then(NonZeroU64::new))
                .map_err(|_| OperationError::RecoveryRequired)?;
        if evidence.digest() != self.evidence_digest() {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(evidence)
    }
}
fn encode(value: &impl Serialize) -> Result<Zeroizing<Vec<u8>>, OperationError> {
    let raw =
        Zeroizing::new(serde_json::to_vec(value).map_err(|_| OperationError::RecoveryRequired)?);
    if raw.len() > PART_BYTES {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(raw)
}
