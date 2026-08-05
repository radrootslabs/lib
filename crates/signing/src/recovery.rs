//! Explicit signer replay and uncertain remote-effect recovery contracts.

/// What exact replay behavior a signer guarantees.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayCapability {
    ExactReplayByRequestId,
    LocalReplaySafe,
    NonReplayable,
}

/// Whether a failed call may already have created a durable remote effect.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RemoteEffect {
    #[default]
    None,
    MayHaveOccurred,
}

/// Required durable recovery treatment after a signing failure.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryDisposition {
    RetryExactRequest,
    RetryLocal,
    Indeterminate,
    Failed,
}

#[must_use]
pub const fn recovery_disposition(
    replay: ReplayCapability,
    remote_effect: RemoteEffect,
    retryable: bool,
) -> RecoveryDisposition {
    if !retryable {
        return RecoveryDisposition::Failed;
    }
    match (replay, remote_effect) {
        (ReplayCapability::ExactReplayByRequestId, _) => RecoveryDisposition::RetryExactRequest,
        (ReplayCapability::LocalReplaySafe, RemoteEffect::None) => RecoveryDisposition::RetryLocal,
        (ReplayCapability::NonReplayable, RemoteEffect::MayHaveOccurred)
        | (ReplayCapability::LocalReplaySafe, RemoteEffect::MayHaveOccurred) => {
            RecoveryDisposition::Indeterminate
        }
        (ReplayCapability::NonReplayable, RemoteEffect::None) => RecoveryDisposition::Failed,
    }
}
