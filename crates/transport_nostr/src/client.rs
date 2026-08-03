//! Concrete Nostr transport composition.

use crate::{Error, RelayUrl, RelayUrlPolicy};
use core::fmt;
use std::collections::BTreeSet;
use std::sync::Arc;

/// Maximum relay targets accepted by one transport instance.
pub(crate) const MAX_RELAYS: usize = 64;
const MAX_TIMEOUT_MS: u64 = 120_000;
const MAX_CONNECTIONS: usize = 64;

/// Validated configuration for a concrete Nostr transport.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    relays: Vec<RelayUrl>,
    relay_url_policy: RelayUrlPolicy,
    connect_timeout_ms: u64,
    request_timeout_ms: u64,
    status_timeout_ms: u64,
    max_connections: usize,
}

impl Config {
    /// Builds configuration from explicit relay and network policy inputs.
    pub fn new<I, S>(relay_url_policy: RelayUrlPolicy, relays: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut canonical = Vec::new();
        let mut seen = BTreeSet::new();
        for relay in relays {
            let relay = RelayUrl::parse(relay, relay_url_policy)?;
            if !seen.insert(relay.clone()) {
                return Err(Error::DuplicateRelayUrl {
                    url: relay.to_string(),
                });
            }
            canonical.push(relay);
            if canonical.len() > MAX_RELAYS {
                return Err(Error::TooManyRelays {
                    max: MAX_RELAYS,
                    actual: canonical.len(),
                });
            }
        }
        if canonical.is_empty() {
            return Err(Error::EmptyRelaySet);
        }
        Ok(Self {
            relays: canonical,
            relay_url_policy,
            connect_timeout_ms: 10_000,
            request_timeout_ms: 30_000,
            status_timeout_ms: 5_000,
            max_connections: 8,
        })
    }

    /// Sets explicit bounded connection, request, and status timeouts.
    pub fn with_timeouts(
        mut self,
        connect_timeout_ms: u64,
        request_timeout_ms: u64,
        status_timeout_ms: u64,
    ) -> Result<Self, Error> {
        validate_timeout("connect", connect_timeout_ms)?;
        validate_timeout("request", request_timeout_ms)?;
        validate_timeout("status", status_timeout_ms)?;
        self.connect_timeout_ms = connect_timeout_ms;
        self.request_timeout_ms = request_timeout_ms;
        self.status_timeout_ms = status_timeout_ms;
        Ok(self)
    }

    /// Sets the maximum simultaneous relay connections for one operation.
    pub fn with_max_connections(mut self, value: usize) -> Result<Self, Error> {
        if value == 0 || value > MAX_CONNECTIONS || value > self.relays.len() {
            return Err(Error::InvalidConnectionLimit { value });
        }
        self.max_connections = value;
        Ok(self)
    }

    /// Returns relays in caller-specified order.
    pub fn relays(&self) -> &[RelayUrl] {
        self.relays.as_slice()
    }

    /// Returns the network policy that must also be applied after DNS resolution.
    pub const fn relay_url_policy(&self) -> RelayUrlPolicy {
        self.relay_url_policy
    }

    /// Returns the connection establishment deadline in milliseconds.
    pub const fn connect_timeout_ms(&self) -> u64 {
        self.connect_timeout_ms
    }

    /// Returns the bounded request deadline in milliseconds.
    pub const fn request_timeout_ms(&self) -> u64 {
        self.request_timeout_ms
    }

    /// Returns the passive status observation deadline in milliseconds.
    pub const fn status_timeout_ms(&self) -> u64 {
        self.status_timeout_ms
    }

    /// Returns the maximum simultaneous relay connections.
    pub const fn max_connections(&self) -> usize {
        self.max_connections
    }
}

fn validate_timeout(field: &'static str, value_ms: u64) -> Result<(), Error> {
    if value_ms == 0 || value_ms > MAX_TIMEOUT_MS {
        return Err(Error::InvalidTimeout { field, value_ms });
    }
    Ok(())
}

/// Concrete Nostr implementation of the transport source and sink SPIs.
#[derive(Clone)]
pub struct NostrTransport {
    config: Config,
    pub(crate) client: Arc<dyn crate::sink::RelayClient>,
}

impl NostrTransport {
    /// Creates an inert transport from validated explicit configuration.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            client: Arc::new(crate::sink::LiveRelayClient),
        }
    }

    /// Returns the transport configuration.
    pub const fn config(&self) -> &Config {
        &self.config
    }

    #[cfg(test)]
    pub(crate) fn with_client(config: Config, client: Arc<dyn crate::sink::RelayClient>) -> Self {
        Self { config, client }
    }
}

impl fmt::Debug for NostrTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NostrTransport")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_rejects_empty_duplicate_and_excessive_relay_sets() {
        assert!(Config::new(RelayUrlPolicy::Public, Vec::<String>::new()).is_err());
        assert!(
            Config::new(
                RelayUrlPolicy::Public,
                ["wss://relay.example.com", "wss://RELAY.EXAMPLE.COM:443/"],
            )
            .is_err()
        );
        let relays = (0..=MAX_RELAYS).map(|index| format!("wss://r{index}.example.com"));
        assert!(Config::new(RelayUrlPolicy::Public, relays).is_err());
    }

    #[test]
    fn config_rejects_unbounded_limits() {
        let config =
            Config::new(RelayUrlPolicy::Public, ["wss://relay.example.com"]).expect("config");
        assert!(config.clone().with_timeouts(0, 1, 1).is_err());
        assert!(config.clone().with_timeouts(1, 120_001, 1).is_err());
        assert!(config.with_max_connections(2).is_err());
    }
}
