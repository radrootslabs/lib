#![forbid(unsafe_code)]

use radroots_transport::{RadrootsTransportDeliveryTargetStatus, RadrootsTransportOutcome};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadrootsRelayOutcomeKind {
    Accepted,
    DuplicateAccepted,
    Blocked,
    RateLimited,
    Invalid,
    PowRequired,
    Restricted,
    AuthRequired,
    Muted,
    Unsupported,
    PaymentRequired,
    Error,
    Timeout,
    ConnectionFailed,
    RelayUrlRejected,
    SkippedAlreadyAccepted,
    Unknown,
}

impl RadrootsRelayOutcomeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::DuplicateAccepted => "duplicate_accepted",
            Self::Blocked => "blocked",
            Self::RateLimited => "rate_limited",
            Self::Invalid => "invalid",
            Self::PowRequired => "pow_required",
            Self::Restricted => "restricted",
            Self::AuthRequired => "auth_required",
            Self::Muted => "muted",
            Self::Unsupported => "unsupported",
            Self::PaymentRequired => "payment_required",
            Self::Error => "error",
            Self::Timeout => "timeout",
            Self::ConnectionFailed => "connection_failed",
            Self::RelayUrlRejected => "relay_url_rejected",
            Self::SkippedAlreadyAccepted => "skipped_already_accepted",
            Self::Unknown => "unknown",
        }
    }

    pub fn counts_toward_quorum(self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::DuplicateAccepted | Self::SkippedAlreadyAccepted
        )
    }

    pub fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::RateLimited
                | Self::PowRequired
                | Self::AuthRequired
                | Self::Error
                | Self::Timeout
                | Self::ConnectionFailed
                | Self::Unknown
        )
    }

    pub fn is_terminal_failure(self) -> bool {
        matches!(
            self,
            Self::Blocked
                | Self::Invalid
                | Self::Restricted
                | Self::Muted
                | Self::Unsupported
                | Self::PaymentRequired
                | Self::RelayUrlRejected
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsRelayOutcome {
    pub kind: RadrootsRelayOutcomeKind,
    pub message: Option<String>,
}

impl RadrootsRelayOutcome {
    pub fn accepted() -> Self {
        Self {
            kind: RadrootsRelayOutcomeKind::Accepted,
            message: None,
        }
    }

    pub fn duplicate_accepted(message: impl Into<String>) -> Self {
        Self {
            kind: RadrootsRelayOutcomeKind::DuplicateAccepted,
            message: Some(message.into()),
        }
    }

    pub fn connection_failed(message: impl Into<String>) -> Self {
        Self {
            kind: RadrootsRelayOutcomeKind::ConnectionFailed,
            message: Some(message.into()),
        }
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self {
            kind: RadrootsRelayOutcomeKind::Timeout,
            message: Some(message.into()),
        }
    }

    pub fn relay_url_rejected(message: impl Into<String>) -> Self {
        Self {
            kind: RadrootsRelayOutcomeKind::RelayUrlRejected,
            message: Some(message.into()),
        }
    }

    pub fn skipped_already_accepted(message: impl Into<String>) -> Self {
        Self {
            kind: RadrootsRelayOutcomeKind::SkippedAlreadyAccepted,
            message: Some(message.into()),
        }
    }

    pub fn classify(message: impl AsRef<str>) -> Self {
        let message = message.as_ref().trim();
        let lower = message.to_ascii_lowercase();
        let kind = if lower.starts_with("duplicate:") {
            RadrootsRelayOutcomeKind::DuplicateAccepted
        } else if lower.starts_with("blocked:") {
            RadrootsRelayOutcomeKind::Blocked
        } else if lower.starts_with("rate-limited:") {
            RadrootsRelayOutcomeKind::RateLimited
        } else if lower.starts_with("invalid:") {
            RadrootsRelayOutcomeKind::Invalid
        } else if lower.starts_with("pow:") {
            RadrootsRelayOutcomeKind::PowRequired
        } else if lower.starts_with("restricted:") {
            RadrootsRelayOutcomeKind::Restricted
        } else if lower.starts_with("auth-required:") {
            RadrootsRelayOutcomeKind::AuthRequired
        } else if lower.starts_with("mute:") {
            RadrootsRelayOutcomeKind::Muted
        } else if lower.starts_with("unsupported:") {
            RadrootsRelayOutcomeKind::Unsupported
        } else if lower.starts_with("payment-required:") {
            RadrootsRelayOutcomeKind::PaymentRequired
        } else if lower.starts_with("error:") {
            RadrootsRelayOutcomeKind::Error
        } else if lower.starts_with("timeout:") {
            RadrootsRelayOutcomeKind::Timeout
        } else {
            RadrootsRelayOutcomeKind::Unknown
        };
        Self {
            kind,
            message: Some(message.to_owned()),
        }
    }

    pub fn counts_toward_quorum(&self) -> bool {
        self.kind.counts_toward_quorum()
    }

    pub fn is_retryable(&self) -> bool {
        self.kind.is_retryable()
    }

    pub fn is_terminal_failure(&self) -> bool {
        self.kind.is_terminal_failure()
    }

    pub fn to_transport_outcome(&self) -> RadrootsTransportOutcome {
        let status = if self.counts_toward_quorum() {
            RadrootsTransportDeliveryTargetStatus::Accepted
        } else if self.is_retryable() {
            RadrootsTransportDeliveryTargetStatus::FailedRetryable
        } else {
            RadrootsTransportDeliveryTargetStatus::FailedTerminal
        };
        let mut outcome = RadrootsTransportOutcome::new(status);
        outcome.code = Some(self.kind.as_str().to_owned());
        outcome.message = self.message.clone();
        outcome
    }
}
