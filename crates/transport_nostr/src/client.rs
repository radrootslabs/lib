//! Concrete Nostr transport composition.

/// Configuration for a concrete Nostr transport.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Config;

/// Concrete Nostr implementation of the transport source and sink SPIs.
#[derive(Clone, Debug, Default)]
pub struct NostrTransport {
    config: Config,
}

impl NostrTransport {
    /// Creates an inert transport from explicit configuration.
    pub const fn new(config: Config) -> Self {
        Self { config }
    }

    /// Returns the transport configuration.
    pub const fn config(&self) -> &Config {
        &self.config
    }
}
