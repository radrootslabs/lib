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
    /// A connection or request timeout is outside its governed bounds.
    InvalidTimeout { field: &'static str, value_ms: u64 },
    /// The per-operation connection limit is outside its governed bounds.
    InvalidConnectionLimit { value: usize },
    /// A transport-neutral target is not a Nostr target.
    UnexpectedTransport { actual: String },
    /// The generic transport target rejected the relay URL.
    Target(String),
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
            Self::InvalidTimeout { field, value_ms } => {
                write!(formatter, "invalid {field} timeout: {value_ms}ms")
            }
            Self::InvalidConnectionLimit { value } => {
                write!(formatter, "invalid connection limit: {value}")
            }
            Self::UnexpectedTransport { actual } => {
                write!(
                    formatter,
                    "expected Nostr transport target, received `{actual}`"
                )
            }
            Self::Target(reason) => write!(formatter, "transport target error: {reason}"),
        }
    }
}

impl std::error::Error for Error {}
