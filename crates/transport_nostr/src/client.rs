#![forbid(unsafe_code)]

use core::time::Duration;
use core::{
    fmt::Debug,
    pin::Pin,
    task::{Context, Poll},
};
use std::collections::{HashMap, HashSet};
#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;

use futures::Stream;
use nostr_sdk::{Client, ClientBuilder, ClientOptions};

use crate::{RadrootsRelayTransportError, RelayUrl};
use nostr::{Keys, SecretKey, SubscriptionId};
use radroots_nostr::event::Event as RadrootsNostrEvent;
use radroots_nostr::event::EventId as RadrootsNostrEventId;
use radroots_nostr::filter::Filter as RadrootsNostrFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsNostrRelayStatus {
    Initialized,
    Pending,
    Connecting,
    Connected,
    Disconnected,
    Terminated,
    Banned,
    Sleeping,
}

fn normalize_relay_status(value: nostr_sdk::RelayStatus) -> RadrootsNostrRelayStatus {
    match value {
        nostr_sdk::RelayStatus::Initialized => RadrootsNostrRelayStatus::Initialized,
        nostr_sdk::RelayStatus::Pending => RadrootsNostrRelayStatus::Pending,
        nostr_sdk::RelayStatus::Connecting => RadrootsNostrRelayStatus::Connecting,
        nostr_sdk::RelayStatus::Connected => RadrootsNostrRelayStatus::Connected,
        nostr_sdk::RelayStatus::Disconnected => RadrootsNostrRelayStatus::Disconnected,
        nostr_sdk::RelayStatus::Terminated => RadrootsNostrRelayStatus::Terminated,
        nostr_sdk::RelayStatus::Banned => RadrootsNostrRelayStatus::Banned,
        nostr_sdk::RelayStatus::Sleeping => RadrootsNostrRelayStatus::Sleeping,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsNostrMonitorNotification {
    StatusChanged {
        relay_url: RelayUrl,
        status: RadrootsNostrRelayStatus,
    },
}

#[derive(Debug, Clone)]
pub struct RadrootsNostrMonitor {
    inner: nostr_sdk::prelude::Monitor,
}

impl RadrootsNostrMonitor {
    pub fn new(channel_size: usize) -> Self {
        Self {
            inner: nostr_sdk::prelude::Monitor::new(channel_size),
        }
    }

    pub fn subscribe(&self) -> RadrootsNostrMonitorReceiver {
        RadrootsNostrMonitorReceiver {
            inner: self.inner.subscribe(),
        }
    }
}

pub struct RadrootsNostrMonitorReceiver {
    inner: tokio::sync::broadcast::Receiver<nostr_sdk::prelude::MonitorNotification>,
}

impl RadrootsNostrMonitorReceiver {
    pub async fn recv(
        &mut self,
    ) -> Result<RadrootsNostrMonitorNotification, RadrootsNostrMonitorReceiveError> {
        self.inner
            .recv()
            .await
            .map(normalize_monitor_notification)
            .map_err(|error| match error {
                tokio::sync::broadcast::error::RecvError::Closed => {
                    RadrootsNostrMonitorReceiveError::Closed
                }
                tokio::sync::broadcast::error::RecvError::Lagged(skipped) => {
                    RadrootsNostrMonitorReceiveError::Lagged { skipped }
                }
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsNostrMonitorReceiveError {
    Closed,
    Lagged { skipped: u64 },
}

fn normalize_monitor_notification(
    notification: nostr_sdk::prelude::MonitorNotification,
) -> RadrootsNostrMonitorNotification {
    match notification {
        nostr_sdk::prelude::MonitorNotification::StatusChanged { relay_url, status } => {
            RadrootsNostrMonitorNotification::StatusChanged {
                relay_url: RelayUrl::from_normalized_transport(relay_url.to_string()),
                status: normalize_relay_status(status),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RadrootsNostrOutput<T> {
    pub val: T,
    pub success: HashSet<RelayUrl>,
    pub failed: HashMap<RelayUrl, String>,
}

fn normalize_output<T>(output: nostr_sdk::prelude::Output<T>) -> RadrootsNostrOutput<T>
where
    T: Debug,
{
    RadrootsNostrOutput {
        val: output.val,
        success: output
            .success
            .into_iter()
            .map(|url| RelayUrl::from_normalized_transport(url.to_string()))
            .collect(),
        failed: output
            .failed
            .into_iter()
            .map(|(url, error)| (RelayUrl::from_normalized_transport(url.to_string()), error))
            .collect(),
    }
}

fn normalize_subscription_output(
    output: nostr_sdk::prelude::Output<SubscriptionId>,
) -> RadrootsNostrOutput<RadrootsNostrSubscriptionId> {
    let output = normalize_output(output);
    RadrootsNostrOutput {
        val: RadrootsNostrSubscriptionId(output.val.to_string()),
        success: output.success,
        failed: output.failed,
    }
}

/// An opaque local credential for the compatibility Nostr transport client.
///
/// Secret material cannot be cloned, formatted, or serialized through this
/// boundary. Hosts should retain their authoritative credential and create a
/// short-lived transport credential only when constructing a client.
///
/// ```compile_fail
/// use radroots_transport_nostr::RadrootsNostrClientKey;
///
/// let key = RadrootsNostrClientKey::generate();
/// let duplicated = key.clone();
/// ```
pub struct RadrootsNostrClientKey {
    inner: Keys,
}

impl RadrootsNostrClientKey {
    pub fn generate() -> Self {
        Self {
            inner: Keys::generate(),
        }
    }

    pub fn from_secret_key_bytes(
        secret_key: [u8; 32],
    ) -> Result<Self, RadrootsRelayTransportError> {
        let secret_key = SecretKey::from_slice(&secret_key)
            .map_err(|error| RadrootsRelayTransportError::ClientConfig(error.to_string()))?;
        Ok(Self {
            inner: Keys::new(secret_key),
        })
    }

    pub fn public_key_hex(&self) -> String {
        self.inner.public_key().to_hex()
    }

    fn into_inner(self) -> Keys {
        self.inner
    }
}

impl Debug for RadrootsNostrClientKey {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("RadrootsNostrClientKey([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsNostrSubscriptionId(String);

impl RadrootsNostrSubscriptionId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl core::fmt::Display for RadrootsNostrSubscriptionId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

pub struct RadrootsNostrEventStream {
    inner: nostr_sdk::pool::stream::BoxedStream<RadrootsNostrEvent>,
}

impl Stream for RadrootsNostrEventStream {
    type Item = RadrootsNostrEvent;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().inner.as_mut().poll_next(context)
    }
}

#[derive(Clone)]
pub struct RadrootsNostrRelay {
    inner: nostr_sdk::Relay,
    url: RelayUrl,
}

impl RadrootsNostrRelay {
    pub fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    pub fn url(&self) -> &RelayUrl {
        &self.url
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RadrootsNostrSubscribeAutoCloseOptions {
    timeout: Option<Duration>,
    idle_timeout: Option<Duration>,
}

impl RadrootsNostrSubscribeAutoCloseOptions {
    pub fn timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn idle_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.idle_timeout = timeout;
        self
    }

    fn into_sdk(self) -> nostr_sdk::SubscribeAutoCloseOptions {
        nostr_sdk::SubscribeAutoCloseOptions::default()
            .timeout(self.timeout)
            .idle_timeout(self.idle_timeout)
    }
}

#[derive(Clone)]
pub struct RadrootsNostrClient {
    inner: Client,
    monitor: Option<RadrootsNostrMonitor>,
}

#[derive(Debug, Clone, Default)]
pub struct RadrootsNostrClientOptions {
    automatic_authentication: Option<bool>,
    max_avg_latency_ms: Option<u64>,
    verify_subscriptions: Option<bool>,
    ban_relay_on_mismatch: Option<bool>,
    #[cfg(not(target_arch = "wasm32"))]
    proxy: Option<SocketAddr>,
}

impl RadrootsNostrClientOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn automatic_authentication(mut self, enabled: bool) -> Self {
        self.automatic_authentication = Some(enabled);
        self
    }

    pub fn max_avg_latency_ms(mut self, max_ms: u64) -> Self {
        self.max_avg_latency_ms = Some(max_ms);
        self
    }

    pub fn verify_subscriptions(mut self, enabled: bool) -> Self {
        self.verify_subscriptions = Some(enabled);
        self
    }

    pub fn ban_relay_on_mismatch(mut self, enabled: bool) -> Self {
        self.ban_relay_on_mismatch = Some(enabled);
        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn proxy_addr(mut self, addr: SocketAddr) -> Self {
        self.proxy = Some(addr);
        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn proxy_str(mut self, addr: &str) -> Result<Self, RadrootsRelayTransportError> {
        let parsed: SocketAddr = addr.parse().map_err(|err: std::net::AddrParseError| {
            RadrootsRelayTransportError::ClientConfig(err.to_string())
        })?;
        self.proxy = Some(parsed);
        Ok(self)
    }

    fn to_client_options(&self) -> ClientOptions {
        let mut options = ClientOptions::new();
        if let Some(enabled) = self.automatic_authentication {
            options = options.automatic_authentication(enabled);
        }
        if let Some(max_ms) = self.max_avg_latency_ms {
            options = options.max_avg_latency(Duration::from_millis(max_ms));
        }
        if let Some(enabled) = self.verify_subscriptions {
            options = options.verify_subscriptions(enabled);
        }
        if let Some(enabled) = self.ban_relay_on_mismatch {
            options = options.ban_relay_on_mismatch(enabled);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(proxy) = self.proxy {
            let connection = nostr_sdk::client::options::Connection::new().proxy(proxy);
            options = options.connection(connection);
        }
        options
    }
}

impl RadrootsNostrClient {
    pub fn new_signerless() -> Self {
        Self {
            inner: Client::default(),
            monitor: None,
        }
    }

    pub fn new_signerless_with_options(options: RadrootsNostrClientOptions) -> Self {
        let inner = ClientBuilder::new()
            .opts(options.to_client_options())
            .build();
        Self {
            inner,
            monitor: None,
        }
    }

    pub fn new(keys: RadrootsNostrClientKey) -> Self {
        Self {
            inner: Client::new(keys.into_inner()),
            monitor: None,
        }
    }

    pub fn from_keys_with_options(
        keys: RadrootsNostrClientKey,
        options: RadrootsNostrClientOptions,
    ) -> Self {
        let inner = ClientBuilder::new()
            .signer(keys.into_inner())
            .opts(options.to_client_options())
            .build();
        Self {
            inner,
            monitor: None,
        }
    }

    pub fn new_with_monitor(keys: RadrootsNostrClientKey, monitor: RadrootsNostrMonitor) -> Self {
        let inner = Client::builder()
            .signer(keys.into_inner())
            .monitor(monitor.inner.clone())
            .build();
        Self {
            inner,
            monitor: Some(monitor),
        }
    }

    pub async fn has_signer(&self) -> bool {
        self.inner.has_signer().await
    }

    pub async fn public_key_hex(&self) -> Result<String, RadrootsRelayTransportError> {
        self.inner
            .public_key()
            .await
            .map(|public_key| public_key.to_hex())
            .map_err(|error| RadrootsRelayTransportError::Client(error.to_string()))
    }

    pub fn monitor(&self) -> Option<&RadrootsNostrMonitor> {
        self.monitor.as_ref()
    }

    pub async fn connect(&self) {
        self.inner.connect().await;
    }

    pub async fn wait_for_connection(&self, timeout: Duration) {
        self.inner.wait_for_connection(timeout).await;
    }

    pub async fn try_connect(&self, timeout: Duration) -> RadrootsNostrOutput<()> {
        normalize_output(self.inner.try_connect(timeout).await)
    }

    pub async fn add_relay(&self, url: &str) -> Result<bool, RadrootsRelayTransportError> {
        self.inner
            .add_relay(url)
            .await
            .map_err(|error| RadrootsRelayTransportError::Client(error.to_string()))
    }

    pub async fn add_write_relay(&self, url: &str) -> Result<bool, RadrootsRelayTransportError> {
        self.inner
            .add_write_relay(url)
            .await
            .map_err(|error| RadrootsRelayTransportError::Client(error.to_string()))
    }

    pub async fn add_read_relay(&self, url: &str) -> Result<bool, RadrootsRelayTransportError> {
        self.inner
            .add_read_relay(url)
            .await
            .map_err(|error| RadrootsRelayTransportError::Client(error.to_string()))
    }

    pub async fn remove_relay(&self, url: &str) -> Result<(), RadrootsRelayTransportError> {
        self.inner
            .force_remove_relay(url)
            .await
            .map_err(|error| RadrootsRelayTransportError::Client(error.to_string()))
    }

    pub async fn relays(&self) -> HashMap<RelayUrl, RadrootsNostrRelay> {
        self.inner
            .relays()
            .await
            .into_iter()
            .map(|(url, inner)| {
                let url = RelayUrl::from_normalized_transport(url.to_string());
                (url.clone(), RadrootsNostrRelay { inner, url })
            })
            .collect()
    }

    pub async fn fetch_events(
        &self,
        filter: RadrootsNostrFilter,
        timeout: Duration,
    ) -> Result<Vec<RadrootsNostrEvent>, RadrootsRelayTransportError> {
        self.inner
            .fetch_events(filter, timeout)
            .await
            .map(|events| events.to_vec())
            .map_err(|error| RadrootsRelayTransportError::Client(error.to_string()))
    }

    pub async fn query_database(
        &self,
        filter: RadrootsNostrFilter,
    ) -> Result<Vec<RadrootsNostrEvent>, RadrootsRelayTransportError> {
        self.inner
            .database()
            .query(filter)
            .await
            .map(|events| events.to_vec())
            .map_err(|error| RadrootsRelayTransportError::ClientDatabase(error.to_string()))
    }

    pub async fn stream_events(
        &self,
        filter: RadrootsNostrFilter,
        timeout: Duration,
    ) -> Result<RadrootsNostrEventStream, RadrootsRelayTransportError> {
        self.inner
            .stream_events(filter, timeout)
            .await
            .map(|inner| RadrootsNostrEventStream { inner })
            .map_err(|error| RadrootsRelayTransportError::Client(error.to_string()))
    }

    pub async fn subscribe(
        &self,
        filter: RadrootsNostrFilter,
        options: Option<RadrootsNostrSubscribeAutoCloseOptions>,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrSubscriptionId>, RadrootsRelayTransportError> {
        self.inner
            .subscribe(
                filter,
                options.map(RadrootsNostrSubscribeAutoCloseOptions::into_sdk),
            )
            .await
            .map(normalize_subscription_output)
            .map_err(|error| RadrootsRelayTransportError::Client(error.to_string()))
    }

    pub async fn subscribe_to_relays(
        &self,
        relays: &[RelayUrl],
        filter: RadrootsNostrFilter,
        options: Option<RadrootsNostrSubscribeAutoCloseOptions>,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrSubscriptionId>, RadrootsRelayTransportError> {
        self.inner
            .subscribe_to(
                relays.iter().map(RelayUrl::as_str),
                filter,
                options.map(RadrootsNostrSubscribeAutoCloseOptions::into_sdk),
            )
            .await
            .map(normalize_subscription_output)
            .map_err(|error| RadrootsRelayTransportError::Client(error.to_string()))
    }

    pub async fn unsubscribe(&self, subscription_id: &RadrootsNostrSubscriptionId) {
        self.inner
            .unsubscribe(&SubscriptionId::new(subscription_id.as_str()))
            .await;
    }

    /// Relays a caller-supplied signed event.
    ///
    /// This is a transport boundary, not an authored-builder boundary. The
    /// caller is responsible for the event's authoring policy and signature.
    pub async fn send_event(
        &self,
        event: &RadrootsNostrEvent,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsRelayTransportError> {
        self.inner
            .send_event(event)
            .await
            .map(normalize_output)
            .map_err(|error| RadrootsRelayTransportError::Client(error.to_string()))
    }

    pub async fn send_event_to_relays(
        &self,
        relays: &[RelayUrl],
        event: &RadrootsNostrEvent,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsRelayTransportError> {
        self.inner
            .send_event_to(relays.iter().map(RelayUrl::as_str), event)
            .await
            .map(normalize_output)
            .map_err(|error| RadrootsRelayTransportError::Client(error.to_string()))
    }

    pub async fn send_event_to(
        &self,
        relays: Vec<String>,
        event: &RadrootsNostrEvent,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsRelayTransportError> {
        self.inner
            .send_event_to(relays, event)
            .await
            .map(normalize_output)
            .map_err(|error| RadrootsRelayTransportError::Client(error.to_string()))
    }
}

pub async fn radroots_nostr_fetch_event_by_id(
    client: &RadrootsNostrClient,
    id: &str,
) -> Result<RadrootsNostrEvent, RadrootsRelayTransportError> {
    let event_id = RadrootsNostrEventId::parse(id)
        .map_err(|error| RadrootsRelayTransportError::NostrEvent(error.to_string()))?;
    let filter = RadrootsNostrFilter::new().id(event_id);
    let events = client.fetch_events(filter, Duration::from_secs(10)).await?;
    let event = events
        .first()
        .ok_or_else(|| RadrootsRelayTransportError::EventNotFound(event_id.to_hex()))?;
    Ok(event.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_key_debug_output_is_redacted() {
        let key = RadrootsNostrClientKey::generate();

        assert_eq!(format!("{key:?}"), "RadrootsNostrClientKey([REDACTED])");
        assert!(!format!("{key:?}").contains(&key.public_key_hex()));
    }

    #[test]
    fn client_key_rejects_invalid_secret_scalar() {
        assert!(RadrootsNostrClientKey::from_secret_key_bytes([0; 32]).is_err());
    }
}
