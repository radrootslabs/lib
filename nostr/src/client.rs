#![forbid(unsafe_code)]

use core::ops::Deref;
use core::time::Duration;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;

use nostr_sdk::{Client, ClientBuilder, ClientOptions};
use radroots_identity::RadrootsIdentity;

use crate::error::RadrootsNostrError;
use crate::types::{
    RadrootsNostrEvent,
    RadrootsNostrEventBuilder,
    RadrootsNostrEventId,
    RadrootsNostrFilter,
    RadrootsNostrKeys,
    RadrootsNostrMonitor,
    RadrootsNostrOutput,
    RadrootsNostrRelay,
    RadrootsNostrRelayUrl,
    RadrootsNostrSubscribeAutoCloseOptions,
    RadrootsNostrSubscriptionId,
};
use crate::types::RadrootsNostrMetadata;

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
        let parsed: SocketAddr = addr
            .parse()
            .map_err(|err: std::net::AddrParseError| {
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

    pub fn into_inner(self) -> Client {
        self.inner
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

    pub async fn subscribe(
        &self,
        filter: RadrootsNostrFilter,
        opts: Option<RadrootsNostrSubscribeAutoCloseOptions>,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrSubscriptionId>, RadrootsNostrError> {
        Ok(self.inner.subscribe(filter, opts).await?)
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

    pub async fn set_metadata(
        &self,
        md: &RadrootsNostrMetadata,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
        Ok(self.inner.set_metadata(md).await?)
    }
}

impl Deref for RadrootsNostrClient {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub async fn radroots_nostr_send_event(
    client: &RadrootsNostrClient,
    event: RadrootsNostrEventBuilder,
) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
    Ok(client.send_event_builder(event).await?)
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
