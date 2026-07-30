//! Normalized transport operation outcomes.

use crate::target::TargetFingerprint;
use alloc::string::String;

/// Maximum encoded normalized outcome code length.
pub const DELIVERY_OUTCOME_CODE_MAX_BYTES: usize = 64;
/// Maximum encoded normalized outcome message length.
pub const DELIVERY_OUTCOME_MESSAGE_MAX_BYTES: usize = 1_024;

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

    /// Returns bounded adapter-normalized diagnostic detail.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

/// Normalized result class for one delivery target.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryOutcomeKind {
    /// The target accepted responsibility for the event.
    Accepted,
    /// The target confirmed final delivery.
    Delivered,
    /// The target rejected the event permanently.
    Rejected,
    /// The target was temporarily unavailable.
    Unavailable,
    /// The adapter reported another normalized failure.
    Failed,
}

/// Whether a failed outcome can be retried without changing the request.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Retryability {
    /// Outcome is successful and retry classification does not apply.
    NotApplicable,
    /// A caller may decide to retry the same target.
    Retryable,
    /// Retrying the same target and payload is not useful.
    Terminal,
}

/// Validated normalized outcome for one delivery target.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryOutcome {
    kind: DeliveryOutcomeKind,
    retryability: Retryability,
    code: Option<String>,
    message: Option<String>,
}

impl DeliveryOutcome {
    /// The target accepted responsibility for the event.
    pub const fn accepted() -> Self {
        Self::new_success(DeliveryOutcomeKind::Accepted)
    }

    /// The target confirmed final delivery.
    pub const fn delivered() -> Self {
        Self::new_success(DeliveryOutcomeKind::Delivered)
    }

    /// The target rejected the event permanently.
    pub const fn rejected() -> Self {
        Self::new_failure(DeliveryOutcomeKind::Rejected, Retryability::Terminal)
    }

    /// The target was temporarily unavailable.
    pub const fn unavailable() -> Self {
        Self::new_failure(DeliveryOutcomeKind::Unavailable, Retryability::Retryable)
    }

    /// Creates another normalized failure with explicit retry classification.
    pub const fn failed(retryability: Retryability) -> Result<Self, crate::Error> {
        if matches!(retryability, Retryability::NotApplicable) {
            return Err(crate::Error::InvalidDeliveryOutcome);
        }
        Ok(Self::new_failure(DeliveryOutcomeKind::Failed, retryability))
    }

    const fn new_success(kind: DeliveryOutcomeKind) -> Self {
        Self {
            kind,
            retryability: Retryability::NotApplicable,
            code: None,
            message: None,
        }
    }

    const fn new_failure(kind: DeliveryOutcomeKind, retryability: Retryability) -> Self {
        Self {
            kind,
            retryability,
            code: None,
            message: None,
        }
    }

    /// Attaches adapter-normalized diagnostic fields.
    pub fn with_detail(
        mut self,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, crate::Error> {
        let code = code.into();
        let message = message.into();
        validate_delivery_detail(code.as_str(), message.as_str())?;
        self.code = Some(code);
        self.message = Some(message);
        Ok(self)
    }

    /// Returns the normalized result kind.
    pub const fn kind(&self) -> DeliveryOutcomeKind {
        self.kind
    }

    /// Returns the explicit retry classification.
    pub const fn retryability(&self) -> Retryability {
        self.retryability
    }

    /// Whether this outcome satisfies the requested success class.
    pub const fn satisfies(&self, class: crate::policy::SatisfactionClass) -> bool {
        match class {
            crate::policy::SatisfactionClass::Accepted => matches!(
                self.kind,
                DeliveryOutcomeKind::Accepted | DeliveryOutcomeKind::Delivered
            ),
            crate::policy::SatisfactionClass::Delivered => {
                matches!(self.kind, DeliveryOutcomeKind::Delivered)
            }
        }
    }

    /// Whether the same target and payload may be retried.
    pub const fn is_retryable(&self) -> bool {
        matches!(self.retryability, Retryability::Retryable)
    }

    /// Whether the failure is terminal for the same target and payload.
    pub const fn is_terminal(&self) -> bool {
        matches!(self.retryability, Retryability::Terminal)
    }

    /// Returns the adapter-normalized code.
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Returns caller-safe diagnostic detail.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub(crate) fn validate(&self) -> Result<(), crate::Error> {
        let valid = match self.kind {
            DeliveryOutcomeKind::Accepted | DeliveryOutcomeKind::Delivered => {
                matches!(self.retryability, Retryability::NotApplicable)
            }
            DeliveryOutcomeKind::Rejected => matches!(self.retryability, Retryability::Terminal),
            DeliveryOutcomeKind::Unavailable => {
                matches!(self.retryability, Retryability::Retryable)
            }
            DeliveryOutcomeKind::Failed => {
                !matches!(self.retryability, Retryability::NotApplicable)
            }
        };
        if !valid {
            return Err(crate::Error::InvalidDeliveryOutcome);
        }
        match (&self.code, &self.message) {
            (None, None) => Ok(()),
            (Some(code), Some(message)) => validate_delivery_detail(code, message),
            (None, Some(_)) | (Some(_), None) => Err(crate::Error::InvalidDeliveryOutcome),
        }
    }
}

fn validate_delivery_detail(code: &str, message: &str) -> Result<(), crate::Error> {
    let valid_code = !code.is_empty()
        && code.len() <= DELIVERY_OUTCOME_CODE_MAX_BYTES
        && code.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        });
    let valid_message = !message.is_empty()
        && message.len() <= DELIVERY_OUTCOME_MESSAGE_MAX_BYTES
        && message == message.trim()
        && !message.chars().any(char::is_control);
    if valid_code && valid_message {
        Ok(())
    } else {
        Err(crate::Error::InvalidDeliveryOutcome)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for DeliveryOutcome {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            kind: DeliveryOutcomeKind,
            retryability: Retryability,
            code: Option<String>,
            message: Option<String>,
        }

        let wire = Wire::deserialize(deserializer)?;
        let outcome = Self {
            kind: wire.kind,
            retryability: wire.retryability,
            code: wire.code,
            message: wire.message,
        };
        outcome.validate().map_err(serde::de::Error::custom)?;
        Ok(outcome)
    }
}
