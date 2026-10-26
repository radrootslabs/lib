//! Exact-intent recovery data. Refusal of a recovery call proves nothing about
//! the original invocation and can never authorize a stock release.
use super::{
    IntentBinding, OutcomeKind, RejectedArtifact, SignedArtifact, SigningIntent, SigningOutcome,
};
use crate::provider::{ProviderCall, ProviderError};

/// Support actually implemented by a provider, requiring independent adapter
/// qualification. A declaration alone is never permission to request an intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExactRecoveryCapability {
    /// No qualified original-intent recovery method is available.
    Unavailable,
    /// A local custody owner can recompute only the complete original preimage
    /// while its original first-issuance window is still open.
    LocalExactReplay,
}

/// Original immutable intent and a distinct bounded recovery-call budget.
/// Construction renews no original first-issuance deadline or business consent.
pub struct ExactRecoveryRequest {
    original: SigningIntent,
    call: ProviderCall,
}
impl ExactRecoveryRequest {
    /// Checks identity and representation only. The executor must authenticate
    /// current original-operation recovery access and the adapter's support.
    pub fn new(original: SigningIntent, call: ProviderCall) -> Result<Self, ProviderError> {
        if original.author() != call.identity() {
            return Err(ProviderError::IdentityChanged);
        }
        if call.deadline_unix_ms().get() > super::MAX_OBSERVATION_UNIX_MS {
            return Err(ProviderError::InvalidInput);
        }
        Ok(Self { original, call })
    }
    /// Borrows the exact original fields, event ID, deadline and intent binding.
    pub const fn original(&self) -> &SigningIntent {
        &self.original
    }
    /// Borrows only this recovery invocation's identity/deadline/cancellation.
    pub const fn call(&self) -> &ProviderCall {
        &self.call
    }
    /// Returns the immutable original intent binding, never a new request ID.
    pub const fn binding(&self) -> IntentBinding {
        self.original.binding()
    }
}
impl core::fmt::Debug for ExactRecoveryRequest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("ExactRecoveryRequest(<redacted>)")
    }
}

/// Recovery observations do not include a proven-unissued variant.
#[derive(Debug)]
#[non_exhaustive]
pub enum ExactRecoveryKind {
    /// Exact authenticated original-intent bytes are now obtained.
    Obtained(SignedArtifact),
    /// The original issuance remains unresolved, including unsupported methods,
    /// current refusal, cancellation and failure of this recovery invocation.
    Indeterminate(ProviderError),
    /// Untrusted output is neither success nor proof of original nonissuance.
    Rejected(RejectedArtifact),
}

/// A bound recovery observation, distinct from durable capture or business state.
#[derive(Debug)]
pub struct ExactRecoveryOutcome {
    binding: IntentBinding,
    kind: ExactRecoveryKind,
}
impl ExactRecoveryOutcome {
    /// Retains uncertainty about the original intent without dispatching work.
    pub const fn indeterminate(binding: IntentBinding, reason: ProviderError) -> Self {
        Self {
            binding,
            kind: ExactRecoveryKind::Indeterminate(reason),
        }
    }
    /// Converts actual exact-field verification output. A pre-dispatch refusal
    /// for a recovery invocation cannot prove the original was unissued.
    pub fn from_recovery_verification(outcome: SigningOutcome) -> Self {
        let (binding, kind) = outcome.into_parts();
        let kind = match kind {
            OutcomeKind::Obtained(value) => ExactRecoveryKind::Obtained(value),
            OutcomeKind::Rejected(value) => ExactRecoveryKind::Rejected(value),
            OutcomeKind::Unknown(reason) | OutcomeKind::NotDispatched(reason) => {
                ExactRecoveryKind::Indeterminate(reason)
            }
        };
        Self { binding, kind }
    }
    /// Enforces the original callback identity using the issuance verifier's
    /// bounded forensic policy. A foreign result grants no success or release.
    pub fn enforce_binding(self, expected: IntentBinding) -> Self {
        let kind = match self.kind {
            ExactRecoveryKind::Obtained(value) => OutcomeKind::Obtained(value),
            ExactRecoveryKind::Rejected(value) => OutcomeKind::Rejected(value),
            ExactRecoveryKind::Indeterminate(reason) => OutcomeKind::Unknown(reason),
        };
        Self::from_recovery_verification(
            SigningOutcome {
                binding: self.binding,
                kind,
            }
            .enforce_binding(expected),
        )
    }
    /// Returns the original operation/intent binding.
    pub const fn binding(&self) -> IntentBinding {
        self.binding
    }
    /// Borrows the actual observation, never permission to release or reissue.
    pub const fn kind(&self) -> &ExactRecoveryKind {
        &self.kind
    }
    /// Transfers obtained original bytes to their durable evidence owner.
    pub fn into_parts(self) -> (IntentBinding, ExactRecoveryKind) {
        (self.binding, self.kind)
    }
}
