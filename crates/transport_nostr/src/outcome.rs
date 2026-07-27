#![forbid(unsafe_code)]

use radroots_transport::{
    RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES, RadrootsTransportError, RadrootsTransportOutcome,
    RadrootsTransportOutcomeKind,
};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::fmt;

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

    pub fn transport_outcome_kind(self) -> RadrootsTransportOutcomeKind {
        match self {
            Self::Accepted => RadrootsTransportOutcomeKind::Accepted,
            Self::DuplicateAccepted | Self::SkippedAlreadyAccepted => {
                RadrootsTransportOutcomeKind::DuplicateAccepted
            }
            Self::Blocked | Self::Invalid | Self::Restricted | Self::Muted | Self::Unsupported => {
                RadrootsTransportOutcomeKind::Rejected
            }
            Self::RelayUrlRejected => RadrootsTransportOutcomeKind::RouteUnavailable,
            Self::PaymentRequired | Self::PowRequired | Self::AuthRequired => {
                RadrootsTransportOutcomeKind::PolicyDenied
            }
            Self::RateLimited | Self::Error | Self::Unknown => {
                RadrootsTransportOutcomeKind::TransportUnavailable
            }
            Self::Timeout => RadrootsTransportOutcomeKind::Timeout,
            Self::ConnectionFailed => RadrootsTransportOutcomeKind::ConnectionFailed,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RadrootsRelayOutcome {
    kind: RadrootsRelayOutcomeKind,
    message: Option<String>,
}

impl RadrootsRelayOutcome {
    pub fn accepted() -> Self {
        Self {
            kind: RadrootsRelayOutcomeKind::Accepted,
            message: None,
        }
    }

    pub fn accepted_with_message(
        message: impl Into<String>,
    ) -> Result<Self, crate::RadrootsRelayTransportError> {
        Self::try_new(RadrootsRelayOutcomeKind::Accepted, Some(message.into()))
    }

    pub fn duplicate_accepted(
        message: impl Into<String>,
    ) -> Result<Self, crate::RadrootsRelayTransportError> {
        Self::try_new(
            RadrootsRelayOutcomeKind::DuplicateAccepted,
            Some(message.into()),
        )
    }

    pub fn connection_failed(
        message: impl Into<String>,
    ) -> Result<Self, crate::RadrootsRelayTransportError> {
        Self::try_new(
            RadrootsRelayOutcomeKind::ConnectionFailed,
            Some(message.into()),
        )
    }

    pub fn unknown(message: impl Into<String>) -> Result<Self, crate::RadrootsRelayTransportError> {
        Self::try_new(RadrootsRelayOutcomeKind::Unknown, Some(message.into()))
    }

    pub fn timeout(message: impl Into<String>) -> Result<Self, crate::RadrootsRelayTransportError> {
        Self::try_new(RadrootsRelayOutcomeKind::Timeout, Some(message.into()))
    }

    pub fn relay_url_rejected(
        message: impl Into<String>,
    ) -> Result<Self, crate::RadrootsRelayTransportError> {
        Self::try_new(
            RadrootsRelayOutcomeKind::RelayUrlRejected,
            Some(message.into()),
        )
    }

    pub fn skipped_already_accepted(
        message: impl Into<String>,
    ) -> Result<Self, crate::RadrootsRelayTransportError> {
        Self::try_new(
            RadrootsRelayOutcomeKind::SkippedAlreadyAccepted,
            Some(message.into()),
        )
    }

    pub fn try_new(
        kind: RadrootsRelayOutcomeKind,
        message: Option<String>,
    ) -> Result<Self, crate::RadrootsRelayTransportError> {
        if let Some(message) = message.as_deref() {
            ensure_relay_outcome_message(message)?;
        }
        Ok(Self { kind, message })
    }

    pub fn classify(message: impl AsRef<str>) -> Result<Self, crate::RadrootsRelayTransportError> {
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
        Self::try_new(kind, Some(message.to_owned()))
    }

    pub fn kind(&self) -> RadrootsRelayOutcomeKind {
        self.kind
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
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

    pub fn to_transport_outcome(&self) -> Result<RadrootsTransportOutcome, RadrootsTransportError> {
        let mut outcome = RadrootsTransportOutcome::new(self.kind.transport_outcome_kind())
            .try_with_code(self.kind.as_str())?;
        if let Some(message) = &self.message {
            outcome = outcome.try_with_message(message.clone())?;
        }
        Ok(outcome)
    }
}

fn ensure_relay_outcome_message(message: &str) -> Result<(), crate::RadrootsRelayTransportError> {
    if message.len() > RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES {
        return Err(
            crate::RadrootsRelayTransportError::DiagnosticLimitExceeded {
                field: "relay_outcome_message",
                max: RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES,
                actual: message.len(),
            },
        );
    }
    Ok(())
}

struct BoundedRelayOutcomeMessage;

impl<'de> de::Visitor<'de> for BoundedRelayOutcomeMessage {
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a relay outcome message of at most {RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES} UTF-8 bytes"
        )
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(value)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        ensure_relay_outcome_message(value)
            .map_err(E::custom)
            .map(|()| value.to_owned())
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        ensure_relay_outcome_message(value.as_str())
            .map_err(E::custom)
            .map(|()| value)
    }
}

fn deserialize_relay_outcome_message<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct OptionalBoundedRelayOutcomeMessage;

    impl<'de> de::Visitor<'de> for OptionalBoundedRelayOutcomeMessage {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded relay outcome message or null")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer
                .deserialize_string(BoundedRelayOutcomeMessage)
                .map(Some)
        }
    }

    deserializer.deserialize_option(OptionalBoundedRelayOutcomeMessage)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RadrootsRelayOutcomeWire {
    kind: RadrootsRelayOutcomeKind,
    #[serde(deserialize_with = "deserialize_relay_outcome_message")]
    message: Option<String>,
}

impl<'de> Deserialize<'de> for RadrootsRelayOutcome {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RadrootsRelayOutcomeWire::deserialize(deserializer)?;
        Self::try_new(wire.kind, wire.message).map_err(de::Error::custom)
    }
}
