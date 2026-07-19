#![forbid(unsafe_code)]

use core::time::Duration;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;

use nostr_sdk::{Client, ClientBuilder, ClientOptions};
use radroots_identity::RadrootsIdentity;

use crate::error::RadrootsNostrError;
#[cfg(feature = "events")]
use crate::events::comment::RadrootsNostrNip22CommentEventBuilder;
#[cfg(feature = "events")]
use crate::events::food_availability::RadrootsNostrFoodAvailabilityEventBuilder;
#[cfg(feature = "events")]
use crate::events::post::RadrootsNostrPostEventBuilder;
#[cfg(feature = "events")]
use crate::events::reply::RadrootsNostrNip10ReplyEventBuilder;
use crate::types::{
    RadrootsNostrEvent, RadrootsNostrEventId, RadrootsNostrEventStream, RadrootsNostrFilter,
    RadrootsNostrGenericEventBuilder, RadrootsNostrKeys, RadrootsNostrMonitor, RadrootsNostrOutput,
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

    /// Publishes a generic event builder.
    ///
    /// Kind 0 profiles, all kind 1 events, kind 1111 Comments, and focused or
    /// mixed kind 30402 FoodAvailability marker partitions are rejected
    /// because their product shape must come from typed authoring. Marker-free
    /// NIP-99 and operational-only kind 30402 builders remain available for
    /// compatibility.
    pub async fn send_event_builder(
        &self,
        event: RadrootsNostrGenericEventBuilder,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
        let event = event.into_checked_event_builder()?;
        Ok(self.inner.send_event_builder(event).await?)
    }

    /// Publishes a validated root post through the sealed typed boundary.
    #[cfg(feature = "events")]
    pub async fn send_post_event_builder(
        &self,
        event: RadrootsNostrPostEventBuilder,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
        Ok(self
            .inner
            .send_event_builder(event.into_event_builder())
            .await?)
    }

    /// Publishes a validated strict marked NIP-10 Reply.
    #[cfg(feature = "events")]
    pub async fn send_nip10_reply_event_builder(
        &self,
        event: RadrootsNostrNip10ReplyEventBuilder,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
        Ok(self
            .inner
            .send_event_builder(event.into_event_builder())
            .await?)
    }

    /// Publishes a validated strict NIP-22 Comment.
    #[cfg(feature = "events")]
    pub async fn send_nip22_comment_event_builder(
        &self,
        event: RadrootsNostrNip22CommentEventBuilder,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
        Ok(self
            .inner
            .send_event_builder(event.into_event_builder())
            .await?)
    }

    /// Publishes a validated focused FoodAvailability event.
    ///
    /// Media-bearing callers must prove successful BUD-02 upload first.
    #[cfg(feature = "events")]
    pub async fn send_food_availability_event_builder(
        &self,
        event: RadrootsNostrFoodAvailabilityEventBuilder,
    ) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
        Ok(self
            .inner
            .send_event_builder(event.into_event_builder())
            .await?)
    }

    /// Relays a caller-supplied signed event.
    ///
    /// This is a transport boundary, not an authored-builder boundary. The
    /// caller is responsible for the event's authoring policy and signature.
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

/// Publishes a generic builder subject to typed-authoring reservations.
pub async fn radroots_nostr_send_event(
    client: &RadrootsNostrClient,
    event: RadrootsNostrGenericEventBuilder,
) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
    client.send_event_builder(event).await
}

/// Publishes a validated root post through the sealed typed boundary.
#[cfg(feature = "events")]
pub async fn radroots_nostr_send_post_event(
    client: &RadrootsNostrClient,
    event: RadrootsNostrPostEventBuilder,
) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
    client.send_post_event_builder(event).await
}

/// Publishes a validated strict marked NIP-10 Reply.
#[cfg(feature = "events")]
pub async fn radroots_nostr_send_nip10_reply_event(
    client: &RadrootsNostrClient,
    event: RadrootsNostrNip10ReplyEventBuilder,
) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
    client.send_nip10_reply_event_builder(event).await
}

/// Publishes a validated strict NIP-22 Comment.
#[cfg(feature = "events")]
pub async fn radroots_nostr_send_nip22_comment_event(
    client: &RadrootsNostrClient,
    event: RadrootsNostrNip22CommentEventBuilder,
) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
    client.send_nip22_comment_event_builder(event).await
}

/// Publishes a validated focused FoodAvailability event.
///
/// Media-bearing callers must prove successful BUD-02 upload first.
#[cfg(feature = "events")]
pub async fn radroots_nostr_send_food_availability_event(
    client: &RadrootsNostrClient,
    event: RadrootsNostrFoodAvailabilityEventBuilder,
) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
    client.send_food_availability_event_builder(event).await
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
    use crate::error::RadrootsNostrError;
    use crate::types::{
        RadrootsNostrFilter, RadrootsNostrGenericEventBuilder, RadrootsNostrKeys,
        RadrootsNostrKind, RadrootsNostrSecretKey, RadrootsNostrSubscriptionId, RadrootsNostrTag,
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

        let event = nostr::EventBuilder::new(RadrootsNostrKind::TextNote, "test")
            .sign_with_keys(&keys)
            .expect("test event");
        let published = client.send_event_to_relays(&[], &event).await;
        assert!(published.is_err());

        client
            .unsubscribe(&RadrootsNostrSubscriptionId::new("missing"))
            .await;
    }

    #[tokio::test]
    async fn generic_builder_rejects_all_typed_authoring_reservations_before_signer_access() {
        let client = RadrootsNostrClient::new_signerless();
        let raw_kind_one = RadrootsNostrKind::Custom(RadrootsNostrKind::TextNote.as_u16());
        let comment = RadrootsNostrKind::Custom(radroots_event::kinds::KIND_COMMENT as u16);
        let classified_listing = RadrootsNostrKind::Custom(30_402);
        let builders = vec![
            RadrootsNostrGenericEventBuilder::new(RadrootsNostrKind::Metadata, "{}"),
            RadrootsNostrGenericEventBuilder::new(raw_kind_one, "Unmarked root"),
            RadrootsNostrGenericEventBuilder::new(raw_kind_one, "Is it ripe?").tags([
                RadrootsNostrTag::parse(["t", "radroots-ask"]).expect("ask marker"),
                RadrootsNostrTag::parse([
                    "imeta",
                    "url https://media.example/ask.webp",
                    "m image/webp",
                ])
                .expect("image metadata"),
            ]),
            RadrootsNostrGenericEventBuilder::new(comment, "Raw Comment"),
            RadrootsNostrGenericEventBuilder::new(classified_listing, "Focused").tag(
                RadrootsNostrTag::parse(["radroots:price_unit", "lb"]).expect("focused marker"),
            ),
            RadrootsNostrGenericEventBuilder::new(classified_listing, "Ambiguous").tags([
                RadrootsNostrTag::parse(["radroots:price_unit", "lb"]).expect("focused marker"),
                RadrootsNostrTag::parse(["radroots:primary_bin", "bin-1"])
                    .expect("operational marker"),
            ]),
        ];

        for builder in builders {
            let error = client
                .send_event_builder(builder)
                .await
                .expect_err("raw root kind-1 builder must be rejected before signer access");

            assert!(matches!(
                error,
                RadrootsNostrError::TypedAuthoringRequired { .. }
            ));
        }
    }

    #[tokio::test]
    async fn generic_builder_rejects_e_tagged_thread_before_signer_access() {
        let keys = RadrootsNostrKeys::new(
            RadrootsNostrSecretKey::from_slice(&[3_u8; 32]).expect("test secret key"),
        );
        let client = RadrootsNostrClient::new(keys);
        let builder = RadrootsNostrGenericEventBuilder::text_note("Reply").tag(
            crate::types::RadrootsNostrTag::event(crate::types::RadrootsNostrEventId::all_zeros()),
        );

        let error = client
            .send_event_builder(builder)
            .await
            .expect_err("generic reply must fail before relay access");

        assert!(matches!(
            error,
            RadrootsNostrError::TypedAuthoringRequired { .. }
        ));
    }

    #[cfg(feature = "events")]
    #[tokio::test]
    async fn sealed_post_builder_reaches_typed_client_publication() {
        let keys = RadrootsNostrKeys::new(
            RadrootsNostrSecretKey::from_slice(&[4_u8; 32]).expect("test secret key"),
        );
        let client = RadrootsNostrClient::new(keys);
        let update = radroots_event::post::RadrootsAuthoredUpdate::new("Farm update")
            .expect("authored update");
        let builder = crate::events::post::radroots_nostr_build_update_event(&update)
            .expect("sealed post builder");

        let error = client
            .send_post_event_builder(builder)
            .await
            .expect_err("no relay is configured");

        assert!(matches!(error, RadrootsNostrError::ClientError(_)));
    }

    #[cfg(feature = "events")]
    #[tokio::test]
    async fn sealed_nip10_reply_builder_reaches_typed_client_publication() {
        let keys = RadrootsNostrKeys::new(
            RadrootsNostrSecretKey::from_slice(&[5_u8; 32]).expect("test secret key"),
        );
        let client = RadrootsNostrClient::new(keys);
        let reference = radroots_event::reply::RadrootsNip10ReplyReference::parse(
            "a".repeat(64),
            "b".repeat(64),
            None,
        )
        .expect("reference");
        let reply = radroots_event::reply::RadrootsAuthoredNip10Reply::direct("Reply", reference)
            .expect("reply");
        let builder =
            crate::events::reply::radroots_nostr_build_nip10_reply_event(&reply).expect("builder");

        let error = client
            .send_nip10_reply_event_builder(builder)
            .await
            .expect_err("no relay is configured");

        assert!(matches!(error, RadrootsNostrError::ClientError(_)));
    }
}
