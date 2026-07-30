//! Normalized transport operation outcomes.

use crate::target::TargetFingerprint;
use alloc::string::String;

/// Target-local result of one bounded fetch attempt.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FetchTargetState {
    /// The target reached its current end without error.
    Complete,
    /// The target produced some results but did not reach its current end.
    Partial,
    /// The target was not available for this operation.
    Unavailable,
    /// The attempt failed and a caller may choose to retry.
    FailedRetryable,
    /// The attempt failed and retrying the same request is not useful.
    FailedTerminal,
    /// Work for this target stopped because the operation was cancelled.
    Cancelled,
}

impl FetchTargetState {
    /// Whether a caller may choose to retry this target.
    pub const fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::Partial | Self::Unavailable | Self::FailedRetryable
        )
    }

    /// Whether this target reached a terminal state for the current request.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Complete | Self::FailedTerminal)
    }
}

/// Explicit result for one requested source target.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchTargetOutcome {
    target: TargetFingerprint,
    state: FetchTargetState,
    message: Option<String>,
}

impl FetchTargetOutcome {
    /// Creates a target-specific normalized outcome.
    pub const fn new(target: TargetFingerprint, state: FetchTargetState) -> Self {
        Self {
            target,
            state,
            message: None,
        }
    }

    /// Attaches caller-safe diagnostic detail.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Returns the exact requested target fingerprint.
    pub const fn target(&self) -> &TargetFingerprint {
        &self.target
    }

    /// Returns normalized state.
    pub const fn state(&self) -> FetchTargetState {
        self.state
    }

    /// Returns caller-safe diagnostic detail.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}
