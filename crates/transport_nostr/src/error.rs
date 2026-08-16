//! Stable Nostr transport failures.

use core::fmt;

/// Error returned by the Nostr transport adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// No relay was configured.
    EmptyRelaySet,
    /// The configured relay count exceeds the adapter bound.
    TooManyRelays { max: usize, actual: usize },
    /// A canonical relay URL occurs more than once.
    DuplicateRelayUrl { url: String },
    /// The URL is not a valid canonical Nostr relay target.
    InvalidRelayUrl { url: String, reason: String },
    /// The URL scheme is not permitted by the selected policy.
    RelaySchemeDenied { url: String },
    /// The URL destination is not permitted by the selected policy.
    RelayDestinationDenied { url: String, reason: &'static str },
    /// DNS resolution produced no addresses.
    EmptyResolution { url: String },
    /// A resolved address violates the selected policy.
    ResolvedAddressDenied { url: String, address: String },
    /// An endpoint policy is incompatible with its host profile kind.
    RelayProfilePolicyMismatch,
    /// A connection or request timeout is outside its governed bounds.
    InvalidTimeout { field: &'static str, value_ms: u64 },
    /// The per-operation connection limit is outside its governed bounds.
    InvalidConnectionLimit { value: usize },
    /// The reconnect delay policy is empty, inverted, or exceeds its bound.
    InvalidReconnectBackoff {
        initial_delay_ms: u64,
        max_delay_ms: u64,
    },
    /// A transport-neutral target is not a Nostr target.
    UnexpectedTransport { actual: String },
    /// A relay cursor contains a noncanonical event position.
    InvalidRelayCursor,
    /// The generic transport target rejected the relay URL.
    Target(String),
    /// The relay challenge is empty, malformed, or outside its time bounds.
    InvalidAuthChallenge,
    /// A different live challenge already exists for this relay.
    AuthChallengeConflict,
    /// No matching live challenge exists for this relay.
    AuthChallengeMissing,
    /// The matching challenge expired before response submission.
    AuthChallengeExpired,
    /// The host supplied no signed NIP-42 response.
    AuthSignerUnavailable,
    /// The signed response does not match the pending relay and challenge.
    AuthResponseMismatch,
    /// The signed response is malformed or cryptographically invalid.
    AuthResponseInvalid,
    /// The host explicitly rejected the pending challenge.
    AuthRejected,
    /// Internal authentication state could not be accessed safely.
    AuthStateUnavailable,
    /// The relay rejected or failed the explicit AUTH message.
    AuthTransport,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRelaySet => formatter.write_str("relay set must not be empty"),
            Self::TooManyRelays { max, actual } => {
                write!(formatter, "relay count {actual} exceeds maximum {max}")
            }
            Self::DuplicateRelayUrl { url } => write!(formatter, "duplicate relay URL `{url}`"),
            Self::InvalidRelayUrl { url, reason } => {
                write!(formatter, "invalid relay URL `{url}`: {reason}")
            }
            Self::RelaySchemeDenied { url } => {
                write!(formatter, "relay URL scheme is denied by policy: `{url}`")
            }
            Self::RelayDestinationDenied { url, reason } => {
                write!(
                    formatter,
                    "relay URL destination is denied: `{url}` ({reason})"
                )
            }
            Self::EmptyResolution { url } => {
                write!(formatter, "relay URL resolved to no addresses: `{url}`")
            }
            Self::ResolvedAddressDenied { url, address } => write!(
                formatter,
                "relay URL `{url}` resolved to denied address `{address}`"
            ),
            Self::RelayProfilePolicyMismatch => {
                formatter.write_str("relay endpoint policy does not match its profile kind")
            }
            Self::InvalidTimeout { field, value_ms } => {
                write!(formatter, "invalid {field} timeout: {value_ms}ms")
            }
            Self::InvalidConnectionLimit { value } => {
                write!(formatter, "invalid connection limit: {value}")
            }
            Self::InvalidReconnectBackoff {
                initial_delay_ms,
                max_delay_ms,
            } => write!(
                formatter,
                "invalid reconnect backoff: initial={initial_delay_ms}ms max={max_delay_ms}ms"
            ),
            Self::UnexpectedTransport { actual } => {
                write!(
                    formatter,
                    "expected Nostr transport target, received `{actual}`"
                )
            }
            Self::InvalidRelayCursor => formatter.write_str("invalid relay cursor"),
            Self::Target(reason) => write!(formatter, "transport target error: {reason}"),
            Self::InvalidAuthChallenge => formatter.write_str("invalid NIP-42 challenge"),
            Self::AuthChallengeConflict => {
                formatter.write_str("a different NIP-42 challenge is already pending")
            }
            Self::AuthChallengeMissing => {
                formatter.write_str("matching NIP-42 challenge is not pending")
            }
            Self::AuthChallengeExpired => formatter.write_str("NIP-42 challenge expired"),
            Self::AuthSignerUnavailable => {
                formatter.write_str("NIP-42 response signer is unavailable")
            }
            Self::AuthResponseMismatch => {
                formatter.write_str("NIP-42 response does not match its challenge")
            }
            Self::AuthResponseInvalid => formatter.write_str("NIP-42 response is invalid"),
            Self::AuthRejected => formatter.write_str("NIP-42 challenge was rejected"),
            Self::AuthStateUnavailable => {
                formatter.write_str("NIP-42 authentication state is unavailable")
            }
            Self::AuthTransport => formatter.write_str("NIP-42 relay submission failed"),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_has_a_stable_nonempty_message() {
        let errors = [
            Error::EmptyRelaySet,
            Error::TooManyRelays { max: 1, actual: 2 },
            Error::DuplicateRelayUrl {
                url: "wss://relay.example".into(),
            },
            Error::InvalidRelayUrl {
                url: "bad".into(),
                reason: "invalid".into(),
            },
            Error::RelaySchemeDenied {
                url: "ws://relay.example".into(),
            },
            Error::RelayDestinationDenied {
                url: "wss://localhost".into(),
                reason: "denied",
            },
            Error::EmptyResolution {
                url: "wss://relay.example".into(),
            },
            Error::ResolvedAddressDenied {
                url: "wss://relay.example".into(),
                address: "127.0.0.1".into(),
            },
            Error::RelayProfilePolicyMismatch,
            Error::InvalidTimeout {
                field: "request",
                value_ms: 0,
            },
            Error::InvalidConnectionLimit { value: 0 },
            Error::InvalidReconnectBackoff {
                initial_delay_ms: 0,
                max_delay_ms: 1,
            },
            Error::UnexpectedTransport {
                actual: "local".into(),
            },
            Error::InvalidRelayCursor,
            Error::Target("invalid".into()),
            Error::InvalidAuthChallenge,
            Error::AuthChallengeConflict,
            Error::AuthChallengeMissing,
            Error::AuthChallengeExpired,
            Error::AuthSignerUnavailable,
            Error::AuthResponseMismatch,
            Error::AuthResponseInvalid,
            Error::AuthRejected,
            Error::AuthStateUnavailable,
            Error::AuthTransport,
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
