//! Owned original dispatch and retained refusal IO, without release authority.
use super::*;
use radroots_signing::{
    issuance::{ExactIssuer, RejectedArtifact, SignedArtifact},
    provider::ProviderError,
};

/// The host's newly confirmed single-use claim owns its original issuer call.
/// Existing claim reports and recovery refusals cannot construct this port.
pub trait AcceptanceDispatch: HostBound {
    /// Original claim, checked by Core before any invocation.
    fn claim(&self) -> AcceptanceClaim;
    /// Consumes this permit and invokes the selected issuer once.
    fn dispatch<'a>(
        self: Box<Self>,
        issuer: &'a dyn ExactIssuer,
        cancellation: CancellationSignal,
    ) -> HostFuture<'a, Result<AcceptanceDispatchOutcome, OperationError>>;
}

/// Original invocation facts; none claims durable capture or installation.
pub enum AcceptanceDispatchOutcome {
    /// Exact signed material, which must remain owned through persistence.
    Obtained(AcceptanceClaim, SignedArtifact),
    /// Owned original refusal; capture has not yet been confirmed.
    Unissued(Box<dyn AcceptanceUnissued>),
    /// Original invocation remains uncertain; keep its stock claim.
    Unknown(AcceptanceClaim, ProviderError),
    /// Bounded diagnostic from a rejected callback, never nonissuance proof.
    Rejected(AcceptanceClaim, RejectedArtifact),
}

/// Authenticated original-invocation fact reported by the concrete store.
/// Public report data grants no dispatch, abort, release or capture authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptanceUnissuedReceipt {
    /// Actual confirmed capture transaction and original operation identity.
    pub attempt: CommitAttempt,
    /// Original protected signing claim.
    pub claim: radroots_domain::SigningAttemptId,
    /// Original exact recipient.
    pub recipient: radroots_domain::PublicKey,
    /// Original invocation's reported refusal.
    pub reason: ProviderError,
    /// Authenticated capture checkpoint, without allocation authority.
    pub checkpoint: radroots_storage::inventory::InventoryCheckpointRef,
}

/// Actual durable completion, kept distinct from original signer observation.
pub enum AcceptanceUnissuedOutcome {
    /// Original fact was durably retained with a confirmed transaction.
    Committed(Box<AcceptanceUnissuedReceipt>),
    /// Actual capture transaction has uncertain completion.
    Unknown(CommitAttempt),
}

/// Owns the target's original invocation observation and retained capture proof.
/// Implementations must originate in that host's consumed fresh dispatch and
/// preserve the original material across failed writes and same-owner recovery.
/// Implementing this port does not qualify an arbitrary issuer's honesty.
pub trait AcceptanceUnissued: HostBound {
    /// Original claim, never permission to invoke again.
    fn claim(&self) -> AcceptanceClaim;
    /// Original invocation's independently retained refusal reason.
    fn reason(&self) -> ProviderError;
    /// Captures borrowed original proof without fresh signing or permission.
    fn capture(&self) -> HostFuture<'_, Result<AcceptanceUnissuedOutcome, OperationError>>;
    /// Reconciles original material on its retained target owner.
    fn reconcile(&self) -> HostFuture<'_, Result<AcceptanceUnissuedOutcome, OperationError>>;
}
