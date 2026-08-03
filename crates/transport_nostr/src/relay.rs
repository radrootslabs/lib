//! Nostr relay identifiers and network policy.

/// Validated Nostr relay URL.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelayUrl(String);

impl RelayUrl {
    /// Returns the validated URL representation.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Network destinations accepted for a relay URL.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RelayUrlPolicy {
    /// Public TLS relay endpoints only.
    Public,
    /// Exact loopback relay endpoints only.
    Local,
}
