//! Stable Nostr transport failures.

use core::fmt;

/// Error returned by the Nostr transport adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// The adapter has not yet been configured for an operation.
    NotConfigured,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConfigured => formatter.write_str("Nostr transport is not configured"),
        }
    }
}

impl std::error::Error for Error {}
