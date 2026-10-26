//! Bounded original evidence input and confirmed append metadata.
use super::*;
use core::num::NonZeroU64;
use radroots_domain::{PublicKey, SigningAttemptId};
use zeroize::Zeroizing;

/// Obtained original wire bytes, never proof of a protected claim or authority.
/// The concrete store must authenticate the actual retained claim and verify
/// these bytes against it before append. Construction performs no cryptography.
pub struct StockEvidenceRequest {
    binding: WorkspaceBinding,
    operation: OperationId,
    digest: CommandDigest,
    claim: SigningAttemptId,
    recipient: PublicKey,
    intent: [u8; 32],
    raw: Zeroizing<String>,
    observed: Option<NonZeroU64>,
}
impl StockEvidenceRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn from_obtained_wire(
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: CommandDigest,
        claim: SigningAttemptId,
        recipient: PublicKey,
        intent: [u8; 32],
        raw: &str,
        observed: Option<NonZeroU64>,
    ) -> Result<Self, OperationError> {
        if binding.scope() != WorkspaceScope::SellerStockOwner {
            return Err(OperationError::Unauthorized);
        }
        if raw.is_empty() || raw.len() > 64 * 1024 {
            return Err(OperationError::InvalidEvidence);
        }
        Ok(Self {
            binding,
            operation,
            digest,
            claim,
            recipient,
            intent,
            raw: Zeroizing::new(raw.to_string()),
            observed,
        })
    }
    pub const fn binding(&self) -> &WorkspaceBinding {
        &self.binding
    }
    pub const fn operation_id(&self) -> OperationId {
        self.operation
    }
    pub const fn command_digest(&self) -> CommandDigest {
        self.digest
    }
    pub const fn signing_attempt_id(&self) -> SigningAttemptId {
        self.claim
    }
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
    }
    pub const fn intent_digest(&self) -> &[u8; 32] {
        &self.intent
    }
    /// Digest of these exact obtained bytes, not a signature or commit proof.
    pub fn evidence_digest(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        self.with_json(|raw| Sha256::digest(raw.as_bytes()).into())
    }
    pub const fn observed_at_unix_ms(&self) -> Option<NonZeroU64> {
        self.observed
    }
    pub fn with_json<T>(&self, use_json: impl FnOnce(&str) -> T) -> T {
        use_json(&self.raw)
    }
    /// Bounded protected copy for the host's existing owned worker. The caller
    /// retains its original bytes across a failed or uncertain append.
    pub fn retained_copy(&self) -> Self {
        Self {
            binding: self.binding.clone(),
            operation: self.operation,
            digest: self.digest,
            claim: self.claim,
            recipient: self.recipient,
            intent: self.intent,
            raw: Zeroizing::new(self.raw.to_string()),
            observed: self.observed,
        }
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
impl core::fmt::Debug for StockEvidenceRequest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StockEvidenceRequest(<redacted>)")
    }
}

/// Confirmed append identity, independent of installation or delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockEvidenceReceipt {
    attempt: CommitAttempt,
    claim: SigningAttemptId,
    recipient: PublicKey,
    digest: [u8; 32],
    observed: Option<NonZeroU64>,
}
impl StockEvidenceReceipt {
    /// Validates report shape only. The concrete host confirms the actual
    /// authenticated append and original observation; metadata grants nothing.
    pub fn from_committed_store(
        attempt: CommitAttempt,
        claim: SigningAttemptId,
        recipient: PublicKey,
        digest: [u8; 32],
        observed: Option<NonZeroU64>,
    ) -> Result<Self, OperationError> {
        if attempt.transaction_id().is_none() {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            attempt,
            claim,
            recipient,
            digest,
            observed,
        })
    }
    pub const fn attempt(&self) -> CommitAttempt {
        self.attempt
    }
    pub const fn signing_attempt_id(&self) -> SigningAttemptId {
        self.claim
    }
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
    }
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub const fn observed_at_unix_ms(&self) -> Option<NonZeroU64> {
        self.observed
    }
}
#[derive(Debug)]
pub enum StockEvidenceOutcome {
    Committed(StockEvidenceReceipt),
    Unknown(CommitAttempt),
}
