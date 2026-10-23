//! Concrete Nostr transport composition.

use crate::{Error, RelayEndpoint, RelayProfile, RelayProfileKind, RelayStatusReport, RelayUrl};
use core::fmt;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Maximum relay targets accepted by one transport instance.
pub(crate) const MAX_RELAYS: usize = 64;
const MAX_TIMEOUT_MS: u64 = 120_000;
const MAX_CONNECTIONS: usize = 64;
const MAX_RECONNECT_DELAY_MS: u64 = 15 * 60 * 1_000;

/// Deterministic exponential reconnect policy applied independently per relay
/// and capability direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconnectBackoff {
    initial_delay_ms: u64,
    max_delay_ms: u64,
}

impl ReconnectBackoff {
    /// Creates a bounded reconnect policy.
    pub fn new(initial_delay_ms: u64, max_delay_ms: u64) -> Result<Self, Error> {
        if initial_delay_ms == 0
            || max_delay_ms < initial_delay_ms
            || max_delay_ms > MAX_RECONNECT_DELAY_MS
        {
            return Err(Error::InvalidReconnectBackoff {
                initial_delay_ms,
                max_delay_ms,
            });
        }
        Ok(Self {
            initial_delay_ms,
            max_delay_ms,
        })
    }

    /// Returns the first delay after a retryable failure.
    #[must_use]
    pub const fn initial_delay_ms(self) -> u64 {
        self.initial_delay_ms
    }

    /// Returns the upper bound for any computed reconnect delay.
    #[must_use]
    pub const fn max_delay_ms(self) -> u64 {
        self.max_delay_ms
    }

    pub(crate) fn delay_ms(self, consecutive_failures: u32) -> u64 {
        let exponent = consecutive_failures.saturating_sub(1).min(63);
        self.initial_delay_ms
            .saturating_mul(1_u64 << exponent)
            .min(self.max_delay_ms)
    }
}

impl Default for ReconnectBackoff {
    fn default() -> Self {
        Self {
            initial_delay_ms: 1_000,
            max_delay_ms: 60_000,
        }
    }
}

/// Validated configuration for a concrete Nostr transport.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    profile_kind: RelayProfileKind,
    endpoints: Vec<RelayEndpoint>,
    relays: Vec<RelayUrl>,
    connect_timeout_ms: u64,
    request_timeout_ms: u64,
    status_timeout_ms: u64,
    max_connections: usize,
    reconnect_backoff: ReconnectBackoff,
}

impl Config {
    /// Builds inert transport configuration from one validated host profile.
    #[must_use]
    pub fn from_profile(profile: RelayProfile) -> Self {
        let relays: Vec<_> = profile
            .endpoints()
            .iter()
            .map(|endpoint| endpoint.url().clone())
            .collect();
        let max_connections = 8.min(relays.len());
        Self {
            profile_kind: profile.kind(),
            endpoints: profile.endpoints().to_vec(),
            relays,
            connect_timeout_ms: 10_000,
            request_timeout_ms: 30_000,
            status_timeout_ms: 5_000,
            max_connections,
            reconnect_backoff: ReconnectBackoff::default(),
        }
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

    /// Sets the deterministic per-relay reconnect policy.
    #[must_use]
    pub const fn with_reconnect_backoff(mut self, value: ReconnectBackoff) -> Self {
        self.reconnect_backoff = value;
        self
    }

    /// Returns the selected host profile kind.
    #[must_use]
    pub const fn profile_kind(&self) -> RelayProfileKind {
        self.profile_kind
    }

    /// Returns configured endpoints with directional access and network policy.
    #[must_use]
    pub fn endpoints(&self) -> &[RelayEndpoint] {
        self.endpoints.as_slice()
    }

    /// Returns relays in caller-specified order.
    pub fn relays(&self) -> &[RelayUrl] {
        self.relays.as_slice()
    }

    /// Returns relays authorized for reads in deterministic profile order.
    pub fn read_relays(&self) -> impl Iterator<Item = &RelayUrl> {
        self.endpoints
            .iter()
            .filter_map(|endpoint| endpoint.access().can_read().then_some(endpoint.url()))
    }

    /// Returns relays authorized for publication in deterministic profile order.
    pub fn write_relays(&self) -> impl Iterator<Item = &RelayUrl> {
        self.endpoints
            .iter()
            .filter_map(|endpoint| endpoint.access().can_write().then_some(endpoint.url()))
    }

    pub(crate) fn endpoint_for_target(
        &self,
        target: &radroots_transport::Target,
    ) -> Option<&RelayEndpoint> {
        (*target.kind() == radroots_transport::TransportId::NOSTR)
            .then(|| target.uri().as_str())
            .and_then(|url| {
                self.endpoints
                    .iter()
                    .find(|endpoint| endpoint.url().as_str() == url)
            })
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

    /// Returns the per-relay reconnect policy.
    #[must_use]
    pub const fn reconnect_backoff(&self) -> ReconnectBackoff {
        self.reconnect_backoff
    }
}

fn validate_timeout(field: &'static str, value_ms: u64) -> Result<(), Error> {
    if value_ms == 0 || value_ms > MAX_TIMEOUT_MS {
        return Err(Error::InvalidTimeout { field, value_ms });
    }
    Ok(())
}

pub(crate) async fn connect_relay(
    client: &nostr_sdk::client::Client,
    url: &str,
    timeout: std::time::Duration,
) -> Result<nostr_sdk::relay::Relay, nostr_sdk::error::Error> {
    let relay = client.relay(url).await?.ok_or_else(|| {
        nostr_sdk::error::Error::with_static_message(
            nostr_sdk::error::ErrorKind::NotFound,
            "configured relay is unavailable",
        )
    })?;
    relay.try_connect().timeout(timeout).await?;
    Ok(relay)
}

pub(crate) async fn connect_tracked(
    client: &nostr_sdk::client::Client,
    connections: &crate::connections::Connections,
    url: &str,
    timeout: std::time::Duration,
) -> Result<nostr_sdk::relay::Relay, nostr_sdk::error::Error> {
    let relay = client.relay(url).await?.ok_or_else(|| {
        nostr_sdk::error::Error::with_static_message(
            nostr_sdk::error::ErrorKind::NotFound,
            "configured relay is unavailable",
        )
    })?;
    connections.register(relay)?;
    connect_relay(client, url, timeout).await
}

#[derive(Debug)]
struct VerifiedEventAdmission;

impl nostr_sdk::policy::AdmitPolicy for VerifiedEventAdmission {
    fn admit_event<'a>(
        &'a self,
        _relay_url: &'a nostr_sdk::prelude::RelayUrl,
        _subscription_id: &'a nostr_sdk::prelude::SubscriptionId,
        event: &'a nostr_sdk::prelude::Event,
    ) -> futures::future::BoxFuture<
        'a,
        Result<nostr_sdk::policy::AdmitStatus, nostr_sdk::error::Error>,
    > {
        Box::pin(async move {
            // The SDK verification cache keys only the ID, not the received body or signature.
            Ok(if event.verify().is_ok() {
                nostr_sdk::policy::AdmitStatus::Success
            } else {
                nostr_sdk::policy::AdmitStatus::Rejected { reason: None }
            })
        })
    }
}

/// Concrete Nostr implementation of the transport source and sink SPIs.
#[derive(Clone)]
pub struct NostrTransport {
    config: Config,
    session_client: nostr_sdk::client::Client,
    pub(crate) connections: Arc<crate::connections::Connections>,
    pub(crate) client: Arc<dyn crate::sink::RelayClient>,
    pub(crate) source_client: Arc<dyn crate::source::RelaySourceClient>,
    pub(crate) subscription_client: Arc<dyn crate::subscription::RelaySubscriptionClient>,
    pub(crate) auth: Arc<crate::auth::AuthFlow>,
    pub(crate) status: Arc<crate::status::StatusTracker>,
    subscription_sequence: Arc<AtomicU64>,
}

impl NostrTransport {
    /// Creates an inert transport from validated explicit configuration.
    pub fn new(
        config: Config,
        host: Arc<dyn radroots_transport::host::websocket::Connector>,
    ) -> Self {
        let connector = crate::session_socket::SessionWebsocketTransport::new(
            config.endpoints(),
            std::time::Duration::from_millis(config.connect_timeout_ms()),
            host,
        );
        let writers = connector.writers.clone();
        let ingress = connector.ingress.clone();
        let client = nostr_sdk::client::Client::builder()
            .websocket_transport(connector)
            .admit_policy(VerifiedEventAdmission)
            .max_relays(std::num::NonZeroUsize::new(config.endpoints().len()))
            .connect_timeout(std::time::Duration::from_millis(
                config.connect_timeout_ms(),
            ))
            .build();
        let status = Arc::new(crate::status::StatusTracker::new(&config));
        let connections = Arc::new(crate::connections::Connections::new(
            config.endpoints().len(),
        ));
        Self {
            config,
            session_client: client.clone(),
            connections: Arc::clone(&connections),
            client: Arc::new(crate::sink::LiveRelayClient::new(
                client.clone(),
                writers,
                Arc::clone(&connections),
            )),
            source_client: Arc::new(crate::source::LiveRelaySourceClient::new(
                client.clone(),
                ingress.clone(),
                Arc::clone(&connections),
            )),
            subscription_client: Arc::new(crate::subscription::LiveRelaySubscriptionClient::new(
                client.clone(),
                connections,
                ingress,
            )),
            auth: Arc::new(crate::auth::AuthFlow::new(Arc::new(
                crate::auth::LiveAuthClient::new(client),
            ))),
            status,
            subscription_sequence: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Returns the transport configuration.
    pub const fn config(&self) -> &Config {
        &self.config
    }

    /// Returns passive per-relay and aggregate evidence without network I/O.
    #[must_use]
    pub fn relay_status(&self) -> RelayStatusReport {
        self.status.report()
    }

    /// Closes admission, disposes pending AUTH challenges and shuts down the owned relay pool.
    ///
    /// An unpolled future has no effect. Re-await after cancellation to finish
    /// cleanup. This is idempotent across clones; already-started operations
    /// must still be awaited and their obtained results retained by their owner.
    /// Upstream pool shutdown signals its relay tasks; this method does not
    /// claim to join every upstream internal task.
    pub async fn shutdown(&self) {
        self.close_admission();
        self.session_client.shutdown().await;
    }

    /// Refuses new calls while already-started calls retain their exact observations.
    ///
    /// When the last active call completes or is cancelled, its registered
    /// relays disconnect synchronously. The owner must still await pool
    /// shutdown; this method claims neither task joining nor durable completion.
    pub fn close_admission(&self) {
        self.connections.close();
        self.auth.close();
    }

    /// Reports active transport calls, excluding passive status and live subscription handles.
    pub fn has_in_flight_operations(&self) -> bool {
        self.connections.has_active_calls()
    }

    /// Reports closed admission, including interrupted shutdown, without claiming task joining.
    pub fn admission_is_closed(&self) -> bool {
        self.connections.is_closed()
    }

    pub(crate) fn require_open(&self) -> Result<(), radroots_transport::Error> {
        if self.admission_is_closed() {
            Err(radroots_transport::Error::TransportClosed)
        } else {
            Ok(())
        }
    }

    #[cfg(test)]
    pub(crate) fn with_client(config: Config, client: Arc<dyn crate::sink::RelayClient>) -> Self {
        let status = Arc::new(crate::status::StatusTracker::new(&config));
        Self {
            config,
            session_client: nostr_sdk::client::Client::default(),
            connections: Arc::new(crate::connections::Connections::new(MAX_RELAYS)),
            client,
            source_client: Arc::new(crate::source::LiveRelaySourceClient::isolated()),
            subscription_client: Arc::new(
                crate::subscription::LiveRelaySubscriptionClient::isolated(),
            ),
            auth: Arc::new(crate::auth::AuthFlow::isolated()),
            status,
            subscription_sequence: Arc::new(AtomicU64::new(0)),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_source_client(
        config: Config,
        source_client: Arc<dyn crate::source::RelaySourceClient>,
    ) -> Self {
        let status = Arc::new(crate::status::StatusTracker::new(&config));
        Self {
            config,
            session_client: nostr_sdk::client::Client::default(),
            connections: Arc::new(crate::connections::Connections::new(MAX_RELAYS)),
            client: Arc::new(crate::sink::LiveRelayClient::isolated()),
            source_client,
            subscription_client: Arc::new(
                crate::subscription::LiveRelaySubscriptionClient::isolated(),
            ),
            auth: Arc::new(crate::auth::AuthFlow::isolated()),
            status,
            subscription_sequence: Arc::new(AtomicU64::new(0)),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_subscription_client(
        config: Config,
        subscription_client: Arc<dyn crate::subscription::RelaySubscriptionClient>,
    ) -> Self {
        let status = Arc::new(crate::status::StatusTracker::new(&config));
        Self {
            config,
            session_client: nostr_sdk::client::Client::default(),
            connections: Arc::new(crate::connections::Connections::new(MAX_RELAYS)),
            client: Arc::new(crate::sink::LiveRelayClient::isolated()),
            source_client: Arc::new(crate::source::LiveRelaySourceClient::isolated()),
            subscription_client,
            auth: Arc::new(crate::auth::AuthFlow::isolated()),
            status,
            subscription_sequence: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(crate) fn next_subscription_sequence(&self) -> Result<u64, radroots_transport::Error> {
        self.subscription_sequence
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                current.checked_add(1)
            })
            .map(|previous| previous + 1)
            .map_err(|_| radroots_transport::Error::SubscriptionUnavailable)
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
#[path = "native_backend_tests.rs"]
mod native_backend_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_rejects_empty_duplicate_and_excessive_relay_sets() {
        assert!(
            crate::profile::test_profile(
                RelayProfileKind::Simulator,
                crate::RelayUrlPolicy::Local,
                Vec::<String>::new(),
            )
            .is_err()
        );
        assert!(
            crate::profile::test_profile(
                RelayProfileKind::Public,
                crate::RelayUrlPolicy::Public,
                ["wss://relay.example.com", "wss://RELAY.EXAMPLE.COM:443/"],
            )
            .is_err()
        );
        let relays = (0..=MAX_RELAYS).map(|index| format!("wss://r{index}.example.com"));
        assert!(
            crate::profile::test_profile(
                RelayProfileKind::Public,
                crate::RelayUrlPolicy::Public,
                relays,
            )
            .is_err()
        );
    }

    #[test]
    fn config_rejects_unbounded_limits() {
        let config = Config::from_profile(
            crate::profile::test_profile(
                RelayProfileKind::Public,
                crate::RelayUrlPolicy::Public,
                ["wss://relay.example.com"],
            )
            .expect("profile"),
        );
        assert!(config.clone().with_timeouts(0, 1, 1).is_err());
        assert!(config.clone().with_timeouts(1, 120_001, 1).is_err());
        assert!(config.with_max_connections(3).is_err());
    }

    #[test]
    fn valid_configuration_accessors_and_transport_debug_are_complete() {
        let config = Config::from_profile(
            crate::profile::test_profile(
                RelayProfileKind::Public,
                crate::RelayUrlPolicy::Public,
                ["wss://one.example", "wss://two.example"],
            )
            .expect("profile"),
        )
        .with_timeouts(1, 2, 3)
        .expect("timeouts")
        .with_max_connections(2)
        .expect("connections");
        assert_eq!(config.relays().len(), 2);
        assert_eq!(config.read_relays().count(), 2);
        assert_eq!(config.write_relays().count(), 2);
        assert_eq!(config.profile_kind(), RelayProfileKind::Public);
        assert_eq!(config.connect_timeout_ms(), 1);
        assert_eq!(config.request_timeout_ms(), 2);
        assert_eq!(config.status_timeout_ms(), 3);
        assert_eq!(config.max_connections(), 2);
        assert_eq!(config.reconnect_backoff(), ReconnectBackoff::default());
        assert!(ReconnectBackoff::new(0, 1).is_err());
        assert!(ReconnectBackoff::new(2, 1).is_err());
        assert!(ReconnectBackoff::new(1, MAX_RECONNECT_DELAY_MS + 1).is_err());
        let backoff = ReconnectBackoff::new(2, 5).expect("backoff");
        assert_eq!(backoff.delay_ms(0), 2);
        assert_eq!(backoff.delay_ms(1), 2);
        assert_eq!(backoff.delay_ms(2), 4);
        assert_eq!(backoff.delay_ms(3), 5);
        assert!(config.clone().with_timeouts(120_001, 1, 1).is_err());
        assert!(config.clone().with_timeouts(1, 1, 0).is_err());
        assert!(config.clone().with_max_connections(0).is_err());
        assert!(
            config
                .clone()
                .with_max_connections(MAX_CONNECTIONS + 1)
                .is_err()
        );

        let transport = crate::test_host::transport(config.clone());
        assert_eq!(transport.config(), &config);
        let debug = format!("{transport:?}");
        assert!(debug.contains("NostrTransport"));
        assert!(!debug.contains("client"));
    }

    #[test]
    fn subscription_sequence_is_monotonic_and_fails_closed_at_overflow() {
        let config = Config::from_profile(
            crate::profile::test_profile(
                RelayProfileKind::Public,
                crate::RelayUrlPolicy::Public,
                ["wss://relay.example.com"],
            )
            .expect("profile"),
        );
        let transport = crate::test_host::transport(config);
        assert_eq!(transport.next_subscription_sequence(), Ok(1));
        assert_eq!(transport.next_subscription_sequence(), Ok(2));
        transport
            .subscription_sequence
            .store(u64::MAX, Ordering::SeqCst);
        assert_eq!(
            transport.next_subscription_sequence(),
            Err(radroots_transport::Error::SubscriptionUnavailable)
        );
    }
}
