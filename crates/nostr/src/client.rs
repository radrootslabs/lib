#![forbid(unsafe_code)]

use core::time::Duration;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;

use nostr_sdk::{Client, ClientBuilder, ClientOptions};
use radroots_identity::RadrootsIdentity;

use crate::error::RadrootsNostrError;
use crate::types::{
    RadrootsNostrEvent, RadrootsNostrEventBuilder, RadrootsNostrEventId, RadrootsNostrEventStream,
    RadrootsNostrFilter, RadrootsNostrKeys, RadrootsNostrMonitor, RadrootsNostrOutput,
    RadrootsNostrPublicKey, RadrootsNostrRelay, RadrootsNostrRelayUrl,
    RadrootsNostrSubscribeAutoCloseOptions, RadrootsNostrSubscriptionId,
};

#[derive(Clone)]
pub struct RadrootsNostrClient {
    inner: Client,
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
    pub fn proxy_str(mut self, addr: &str) -> Result<Self, RadrootsNostrError> {
        let parsed: SocketAddr = addr.parse().map_err(|err: std::net::AddrParseError| {
            RadrootsNostrError::ClientConfigError(err.to_string())
        })?;
        self.proxy = Some(parsed);
        Ok(self)
    }

    fn to_client_options(&self) -> Result<ClientOptions, RadrootsNostrError> {
        let mut opts = ClientOptions::new();
        if let Some(enabled) = self.automatic_authentication {
            opts = opts.automatic_authentication(enabled);
        }
        if let Some(max_ms) = self.max_avg_latency_ms {
            opts = opts.max_avg_latency(Duration::from_millis(max_ms));
        }
        if let Some(enabled) = self.verify_subscriptions {
            opts = opts.verify_subscriptions(enabled);
        }
        if let Some(enabled) = self.ban_relay_on_mismatch {
            opts = opts.ban_relay_on_mismatch(enabled);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(proxy) = self.proxy {
            let connection = nostr_sdk::client::options::Connection::new().proxy(proxy);
            opts = opts.connection(connection);
        }
        Ok(opts)
    }
}

impl RadrootsNostrClient {
    pub fn new_signerless() -> Self {
        Self {
            inner: Client::default(),
        }
    }

    pub fn new_signerless_with_options(
        options: RadrootsNostrClientOptions,
    ) -> Result<Self, RadrootsNostrError> {
        let opts = options.to_client_options()?;
        let inner = ClientBuilder::new().opts(opts).build();
        Ok(Self { inner })
    }

    pub fn new(keys: RadrootsNostrKeys) -> Self {
        Self {
            inner: Client::new(keys),
        }
    }

    pub fn from_keys_with_options(
        keys: RadrootsNostrKeys,
        options: RadrootsNostrClientOptions,
    ) -> Result<Self, RadrootsNostrError> {
        let opts = options.to_client_options()?;
        let inner = ClientBuilder::new().signer(keys).opts(opts).build();
        Ok(Self { inner })
    }

    pub fn new_with_monitor(keys: RadrootsNostrKeys, monitor: RadrootsNostrMonitor) -> Self {
        let inner = Client::builder().signer(keys).monitor(monitor).build();
        Self { inner }
    }

    pub fn from_identity(identity: &RadrootsIdentity) -> Self {
        Self::new(identity.keys().clone())
    }

    pub fn from_identity_owned(identity: RadrootsIdentity) -> Self {
        Self::new(identity.into_keys())
    }

    pub fn from_inner(inner: Client) -> Self {
        Self { inner }
    }

    /// Returns the underlying SDK client for explicit low-level interoperability.
    pub fn into_inner(self) -> Client {
        self.inner
    }

    pub async fn has_signer(&self) -> bool {
        self.inner.has_signer().await
    }

    pub async fn public_key(&self) -> Result<RadrootsNostrPublicKey, RadrootsNostrError> {
        Ok(self.inner.public_key().await?)
    }

    pub fn monitor(&self) -> Option<&RadrootsNostrMonitor> {
        self.inner.monitor()
    }

    pub async fn connect(&self) {
        self.inner.connect().await;
    }

    pub async fn wait_for_connection(&self, timeout: Duration) {
        self.inner.wait_for_connection(timeout).await;
    }

    pub async fn try_connect(&self, timeout: Duration) -> RadrootsNostrOutput<()> {
        self.inner.try_connect(timeout).await
    }

    pub async fn add_relay(&self, url: &str) -> Result<bool, RadrootsNostrError> {
        Ok(self.inner.add_relay(url).await?)
    }

    pub async fn add_write_relay(&self, url: &str) -> Result<bool, RadrootsNostrError> {
        Ok(self.inner.add_write_relay(url).await?)
    }

    pub async fn add_read_relay(&self, url: &str) -> Result<bool, RadrootsNostrError> {
        Ok(self.inner.add_read_relay(url).await?)
    }

    pub async fn remove_relay(&self, url: &str) -> Result<(), RadrootsNostrError> {
        self.inner.force_remove_relay(url).await?;
        Ok(())
    }

    pub async fn relays(&self) -> HashMap<RadrootsNostrRelayUrl, RadrootsNostrRelay> {
        self.inner.relays().await
    }

    pub async fn fetch_events(
        &self,
        filter: RadrootsNostrFilter,
        timeout: Duration,
    ) -> Result<Vec<RadrootsNostrEvent>, RadrootsNostrError> {
        let events = self.inner.fetch_events(filter, timeout).await?;
        Ok(events.to_vec())
    }

    pub async fn query_database(
        &self,
        filter: RadrootsNostrFilter,
    ) -> Result<Vec<RadrootsNostrEvent>, RadrootsNostrError> {
        Ok(self.inner.database().query(filter).await?.to_vec())
    }

    pub async fn stream_events(
        &self,
        filter: RadrootsNostrFilter,
        timeout: Duration,
    ) -> Result<RadrootsNostrEventStream, RadrootsNostrError> {
        Ok(self.inner.stream_events(filter, timeout).await?)
    }

    pub async fn subscribe(
        &self,
        filter: RadrootsNostrFilter,
        opts: Option<RadrootsNostrSubscribeAutoCloseOptions>,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrSubscriptionId>, RadrootsNostrError> {
        Ok(self.inner.subscribe(filter, opts).await?)
    }

    pub async fn subscribe_to_relays(
        &self,
        relays: &[RadrootsNostrRelayUrl],
        filter: RadrootsNostrFilter,
        opts: Option<RadrootsNostrSubscribeAutoCloseOptions>,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrSubscriptionId>, RadrootsNostrError> {
        Ok(self
            .inner
            .subscribe_to(relays.iter().cloned(), filter, opts)
            .await?)
    }

    pub async fn unsubscribe(&self, subscription_id: &RadrootsNostrSubscriptionId) {
        self.inner.unsubscribe(subscription_id).await;
    }

    pub async fn send_event_builder(
        &self,
        event: RadrootsNostrEventBuilder,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
        Ok(self.inner.send_event_builder(event).await?)
    }

    pub async fn send_event(
        &self,
        event: &RadrootsNostrEvent,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
        Ok(self.inner.send_event(event).await?)
    }

    pub async fn send_event_to_relays(
        &self,
        relays: &[RadrootsNostrRelayUrl],
        event: &RadrootsNostrEvent,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
        Ok(self
            .inner
            .send_event_to(relays.iter().cloned(), event)
            .await?)
    }

    pub async fn send_event_to(
        &self,
        relays: Vec<String>,
        event: &RadrootsNostrEvent,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
        Ok(self.inner.send_event_to(relays, event).await?)
    }
}

pub async fn radroots_nostr_send_event(
    client: &RadrootsNostrClient,
    event: RadrootsNostrEventBuilder,
) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
    client.send_event_builder(event).await
}

pub async fn radroots_nostr_fetch_event_by_id(
    client: &RadrootsNostrClient,
    id: &str,
) -> Result<RadrootsNostrEvent, RadrootsNostrError> {
    let event_id = RadrootsNostrEventId::parse(id)?;
    let filter = RadrootsNostrFilter::new().id(event_id);
    let events = client.fetch_events(filter, Duration::from_secs(10)).await?;
    let event = events
        .first()
        .ok_or_else(|| RadrootsNostrError::EventNotFound(event_id.to_hex()))?;
    Ok(event.clone())
}

#[cfg(test)]
mod tests {
    use super::{RadrootsNostrClient, RadrootsNostrClientOptions};
    use crate::types::{
        RadrootsNostrEventBuilder, RadrootsNostrFilter, RadrootsNostrKeys, RadrootsNostrKind,
        RadrootsNostrSecretKey, RadrootsNostrSubscriptionId,
    };

    #[tokio::test]
    async fn signerless_client_has_no_signer() {
        let client = RadrootsNostrClient::new_signerless();

        assert!(!client.has_signer().await);
    }

    #[tokio::test]
    async fn signerless_client_with_options_has_no_signer() {
        let client = RadrootsNostrClient::new_signerless_with_options(
            RadrootsNostrClientOptions::new()
                .automatic_authentication(true)
                .verify_subscriptions(true),
        )
        .expect("signerless client");

        assert!(!client.has_signer().await);
    }

    #[tokio::test]
    async fn targeted_operations_require_relays_and_cleanup_is_explicit() {
        let keys = RadrootsNostrKeys::new(
            RadrootsNostrSecretKey::from_slice(&[1_u8; 32]).expect("test secret key"),
        );
        let client = RadrootsNostrClient::new(keys.clone());
        let subscription = client
            .subscribe_to_relays(&[], RadrootsNostrFilter::new(), None)
            .await;
        assert!(subscription.is_err());

        let event = RadrootsNostrEventBuilder::new(RadrootsNostrKind::TextNote, "test")
            .sign_with_keys(&keys)
            .expect("test event");
        let published = client.send_event_to_relays(&[], &event).await;
        assert!(published.is_err());

        client
            .unsubscribe(&RadrootsNostrSubscriptionId::new("missing"))
            .await;
    }
}
