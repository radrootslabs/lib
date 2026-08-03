//! Client construction and lifecycle.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

use radroots_signing::Signer;
use radroots_storage::Storage;
#[cfg(feature = "memory")]
use radroots_storage::{event::SourceGeneration, memory::MemoryStorage};
use radroots_transport::{EventSink, EventSource};

use crate::{
    Error, Result,
    capability::{Availability, CapabilityId, CapabilityReport},
};

/// Cloneable handle to a composed Radroots client.
#[derive(Clone)]
pub struct Client {
    inner: Arc<ClientInner>,
}

/// Explicit composition boundary for a [`Client`].
#[derive(Default)]
pub struct ClientBuilder {
    storage: Option<Arc<dyn Storage>>,
    #[cfg(feature = "sync")]
    sync_storage: Option<Arc<dyn radroots_sync::policy::SyncStorage>>,
    signer: Option<Arc<dyn Signer>>,
    source: Option<Arc<dyn EventSource>>,
    sink: Option<Arc<dyn EventSink>>,
    #[cfg(feature = "sync")]
    sync: Option<radroots_sync::Engine>,
    #[cfg(feature = "sync")]
    host_sync: Option<crate::sync::HostPolicy>,
    #[cfg(feature = "local-signing")]
    signing_slot: Option<crate::signing::Slot>,
    #[cfg(feature = "nostr")]
    nostr_slot: Option<crate::transport::NostrSlot>,
    capability_availability: BTreeMap<CapabilityId, Availability>,
    explicitly_configured_capabilities: BTreeSet<CapabilityId>,
}

struct ClientInner {
    storage: Arc<dyn Storage>,
    signer: Option<Arc<dyn Signer>>,
    source: Option<Arc<dyn EventSource>>,
    sink: Option<Arc<dyn EventSink>>,
    #[cfg(feature = "sync")]
    sync: Option<radroots_sync::Engine>,
    #[cfg(feature = "local-signing")]
    signing_slot: Option<crate::signing::Slot>,
    #[cfg(feature = "nostr")]
    nostr_slot: Option<crate::transport::NostrSlot>,
    capability_availability: BTreeMap<CapabilityId, Availability>,
    explicitly_configured_capabilities: BTreeSet<CapabilityId>,
    lifecycle: AtomicU8,
}

const OPEN: u8 = 0;
const CLOSING: u8 = 1;
const CLOSE_RETRY_REQUIRED: u8 = 2;
const CLOSED: u8 = 3;

/// Host-authored, media-free profile replacement.
#[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileDraft {
    name: String,
    display_name: Option<String>,
    about: Option<String>,
    nip05: Option<String>,
    bot: Option<bool>,
}

#[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
impl ProfileDraft {
    /// Creates a complete replacement with the required canonical name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            display_name: None,
            about: None,
            nip05: None,
            bot: None,
        }
    }

    /// Sets the optional display name.
    #[must_use]
    pub fn with_display_name(mut self, value: impl Into<String>) -> Self {
        self.display_name = Some(value.into());
        self
    }

    /// Sets the optional profile description.
    #[must_use]
    pub fn with_about(mut self, value: impl Into<String>) -> Self {
        self.about = Some(value.into());
        self
    }

    /// Sets a syntax-checked NIP-05 identifier at publish time.
    #[must_use]
    pub fn with_nip05(mut self, value: impl Into<String>) -> Self {
        self.nip05 = Some(value.into());
        self
    }

    /// Sets the optional NIP-05 bot marker.
    #[must_use]
    pub const fn with_bot(mut self, value: bool) -> Self {
        self.bot = Some(value);
        self
    }
}

/// One verified, durably ingested profile observation.
#[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileEvent {
    event_id: String,
    author: String,
    created_at: u64,
    name: Option<String>,
    display_name: Option<String>,
    about: Option<String>,
    picture: Option<String>,
    banner: Option<String>,
    nip05: Option<String>,
    bot: Option<bool>,
}

#[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
impl ProfileEvent {
    /// Returns the canonical event identifier.
    pub fn event_id(&self) -> &str {
        self.event_id.as_str()
    }
    /// Returns the canonical author public key.
    pub fn author(&self) -> &str {
        self.author.as_str()
    }
    /// Returns the event timestamp in Unix seconds.
    pub const fn created_at(&self) -> u64 {
        self.created_at
    }
    /// Returns the projected profile name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    /// Returns the projected display name.
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }
    /// Returns the projected description.
    pub fn about(&self) -> Option<&str> {
        self.about.as_deref()
    }
    /// Returns the unverified inbound picture reference.
    pub fn picture(&self) -> Option<&str> {
        self.picture.as_deref()
    }
    /// Returns the unverified inbound banner reference.
    pub fn banner(&self) -> Option<&str> {
        self.banner.as_deref()
    }
    /// Returns the syntax-checked, unresolved NIP-05 identifier.
    pub fn nip05(&self) -> Option<&str> {
        self.nip05.as_deref()
    }
    /// Returns the optional bot marker.
    pub const fn bot(&self) -> Option<bool> {
        self.bot
    }
}

/// One verified, durably ingested kind-1 social event.
#[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostEvent {
    event_id: String,
    author: String,
    created_at: u64,
    content: String,
}

#[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
impl PostEvent {
    /// Returns the canonical event identifier.
    pub fn event_id(&self) -> &str {
        self.event_id.as_str()
    }
    /// Returns the canonical author public key.
    pub fn author(&self) -> &str {
        self.author.as_str()
    }
    /// Returns the event timestamp in Unix seconds.
    pub const fn created_at(&self) -> u64 {
        self.created_at
    }
    /// Returns the canonical event content.
    pub fn content(&self) -> &str {
        self.content.as_str()
    }
}

/// Result of one explicit local commit followed by one delivery pass.
#[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishReceipt {
    event_id: String,
    replay: bool,
    delivered: bool,
}

#[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
impl PublishReceipt {
    /// Returns the canonical signed event identifier committed locally.
    pub fn event_id(&self) -> &str {
        self.event_id.as_str()
    }
    /// Returns whether the durable enqueue replayed an identical operation.
    pub const fn is_replay(&self) -> bool {
        self.replay
    }
    /// Returns whether this explicit pass recorded at least one success.
    pub const fn is_delivered(&self) -> bool {
        self.delivered
    }
    /// Returns whether durable local intent remains pending delivery.
    pub const fn is_delivery_pending(&self) -> bool {
        !self.delivered
    }
}

/// Passive status of the configured shared Nostr source and sink.
#[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportHealth {
    configured: bool,
    source_available: bool,
    sink_available: bool,
}

#[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
impl TransportHealth {
    /// Returns whether a validated relay set is installed.
    pub const fn is_configured(&self) -> bool {
        self.configured
    }
    /// Returns whether passive source status is fully available.
    pub const fn is_source_available(&self) -> bool {
        self.source_available
    }
    /// Returns whether passive sink status is fully available.
    pub const fn is_sink_available(&self) -> bool {
        self.sink_available
    }
}

/// Borrowed high-level social operations over one shared SDK engine.
#[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
#[derive(Clone, Copy)]
pub struct SocialOperations<'a> {
    client: &'a Client,
}

impl ClientBuilder {
    /// Creates an empty builder with no hidden storage, network, signing, or
    /// runtime side effects.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a builder backed by deterministic in-process memory storage.
    #[cfg(feature = "memory")]
    #[must_use]
    pub fn memory(generation: SourceGeneration) -> Self {
        let storage = Arc::new(MemoryStorage::new(generation));
        let mut builder = Self::new().storage(storage.clone());
        #[cfg(feature = "sync")]
        {
            builder.sync_storage = Some(storage);
        }
        builder
    }

    /// Creates the ordinary deterministic in-process memory configuration.
    ///
    /// This performs no I/O and is intended for ephemeral local clients whose
    /// source-generation identity does not need to survive the process. Hosts
    /// that persist cursors should call [`Self::memory`] with their own
    /// generation instead.
    #[cfg(feature = "memory")]
    #[must_use]
    pub fn memory_default() -> Self {
        let storage = Arc::new(MemoryStorage::default());
        let mut builder = Self::new().storage(storage.clone());
        #[cfg(feature = "sync")]
        {
            builder.sync_storage = Some(storage);
        }
        builder
    }

    /// Explicitly opens canonical SQLite storage from validated host-owned
    /// configuration and returns a builder containing only the storage SPI.
    #[cfg(feature = "sqlite")]
    pub async fn sqlite(options: crate::storage::SqliteOptions) -> Result<Self> {
        let storage = radroots_storage_sqlite::SqliteStorage::open(options)
            .await
            .map_err(Error::storage_open_failed)?;
        let storage = Arc::new(storage);
        let mut builder = Self::new()
            .storage(storage.clone())
            .capability_availability(CapabilityId::PERSISTENT_STORAGE, Availability::Available);
        #[cfg(feature = "sync")]
        {
            builder.sync_storage = Some(storage);
        }
        Ok(builder)
    }

    /// Injects the canonical storage capability.
    #[must_use]
    pub fn storage(mut self, storage: Arc<dyn Storage>) -> Self {
        self.storage = Some(storage);
        self
    }

    /// Injects an optional canonical signer capability.
    #[must_use]
    pub fn signer(mut self, signer: Arc<dyn Signer>) -> Self {
        self.signer = Some(signer);
        self
    }

    /// Installs a module-scoped signer provider over the canonical SPI and
    /// records its presentation-independent runtime capability.
    #[must_use]
    pub fn signing(mut self, provider: crate::signing::Provider) -> Self {
        let capability = match provider.mode() {
            crate::signing::Mode::Local => Some(CapabilityId::LOCAL_SIGNING),
            crate::signing::Mode::Nip46 => Some(CapabilityId::NIP46_SIGNING),
            crate::signing::Mode::Host => None,
        };
        #[cfg(feature = "local-signing")]
        {
            let (signer, slot) = provider.into_parts();
            self.signer = Some(signer);
            self.signing_slot = slot;
        }
        #[cfg(not(feature = "local-signing"))]
        {
            self.signer = Some(provider.into_signer());
        }
        if let Some(capability) = capability {
            self.explicitly_configured_capabilities.insert(capability);
            self.capability_availability
                .insert(capability, Availability::Available);
        }
        self
    }

    /// Injects an optional inbound event source.
    #[must_use]
    pub fn source(mut self, source: Arc<dyn EventSource>) -> Self {
        self.source = Some(source);
        self
    }

    /// Injects an optional outbound event sink.
    #[must_use]
    pub fn sink(mut self, sink: Arc<dyn EventSink>) -> Self {
        self.sink = Some(sink);
        self
    }

    /// Installs one host-reconfigurable Nostr source and sink.
    #[cfg(feature = "nostr")]
    #[must_use]
    pub fn nostr(mut self, slot: crate::transport::NostrSlot) -> Self {
        self.source = Some(Arc::new(slot.clone()));
        self.sink = Some(Arc::new(slot.clone()));
        self.nostr_slot = Some(slot);
        self
    }

    /// Injects an explicitly composed synchronization engine.
    #[cfg(feature = "sync")]
    #[must_use]
    pub fn sync_engine(mut self, sync: radroots_sync::Engine) -> Self {
        self.sync = Some(sync);
        self
    }

    /// Requests one SDK-composed engine using explicit native host policy.
    ///
    /// This is available only for SDK-created memory or SQLite storage, whose
    /// complete synchronization capability is known without downcasting.
    #[cfg(feature = "sync")]
    #[must_use]
    pub fn host_sync(mut self, policy: crate::sync::HostPolicy) -> Self {
        self.host_sync = Some(policy);
        self
    }

    /// Marks a specialized capability as configured and records its
    /// host-observed initial availability without probing resources.
    ///
    /// Reports ignore this observation for capabilities that are not compiled
    /// or configured. Runtime IDs are independent from Cargo feature names.
    #[must_use]
    pub fn capability_availability(mut self, id: CapabilityId, availability: Availability) -> Self {
        self.explicitly_configured_capabilities.insert(id);
        self.capability_availability.insert(id, availability);
        self
    }

    /// Validates the selected capabilities and creates a client handle.
    #[allow(unused_mut)]
    pub fn build(mut self) -> Result<Client> {
        let storage = self.storage.ok_or_else(Error::missing_storage)?;
        if self.signer.is_some() && self.sink.is_none() {
            return Err(Error::signer_without_sink());
        }
        #[cfg(feature = "sync")]
        if let Some(policy) = self.host_sync {
            let sync_storage = self
                .sync_storage
                .take()
                .ok_or_else(Error::shared_operation_unavailable)?;
            let (clock, ids, deadlines) = policy.composition();
            let mut builder = radroots_sync::Engine::builder(sync_storage, clock, ids, deadlines);
            if let Some(source) = self.source.as_ref() {
                builder = builder.source(Arc::clone(source));
            }
            if let Some(sink) = self.sink.as_ref() {
                builder = builder.sink(Arc::clone(sink));
            }
            if let Some(signer) = self.signer.as_ref() {
                builder = builder.signer(Arc::clone(signer));
            }
            self.sync = Some(builder.build().map_err(Error::invalid_host_configuration)?);
        }
        Ok(Client {
            inner: Arc::new(ClientInner {
                storage,
                signer: self.signer,
                source: self.source,
                sink: self.sink,
                #[cfg(feature = "sync")]
                sync: self.sync,
                #[cfg(feature = "local-signing")]
                signing_slot: self.signing_slot,
                #[cfg(feature = "nostr")]
                nostr_slot: self.nostr_slot,
                capability_availability: self.capability_availability,
                explicitly_configured_capabilities: self.explicitly_configured_capabilities,
                lifecycle: AtomicU8::new(OPEN),
            }),
        })
    }
}

impl Client {
    /// Returns a deterministic capability report without probing resources or
    /// performing filesystem, network, signing, or storage operations.
    #[must_use]
    pub fn capabilities(&self) -> CapabilityReport {
        let lifecycle_availability = match self.inner.lifecycle.load(Ordering::Acquire) {
            OPEN => Availability::Available,
            CLOSING | CLOSE_RETRY_REQUIRED => Availability::Degraded,
            _ => Availability::Unavailable,
        };
        crate::capability::report(crate::capability::Context {
            storage: true,
            signer: self.inner.signer.is_some(),
            source: self.inner.source.is_some(),
            sink: self.inner.sink.is_some(),
            sync: self.sync_is_configured(),
            lifecycle_availability,
            explicitly_configured: &self.inner.explicitly_configured_capabilities,
            overrides: &self.inner.capability_availability,
        })
    }

    /// Returns canonical backend status without exposing a backend handle.
    pub async fn storage_status(&self) -> Result<crate::storage::Status> {
        let storage = self.storage()?;
        radroots_storage::BackupSource::status(storage)
            .await
            .map_err(Error::storage_inspection_failed)
    }

    /// Runs canonical integrity inspection without exposing backend internals.
    pub async fn storage_integrity(&self) -> Result<crate::storage::IntegrityStatus> {
        let storage = self.storage()?;
        radroots_storage::BackupSource::integrity(storage)
            .await
            .map_err(Error::storage_inspection_failed)
    }

    /// Returns the injected canonical storage capability.
    pub fn storage(&self) -> Result<&dyn Storage> {
        self.require_open()?;
        Ok(self.inner.storage.as_ref())
    }

    /// Returns backend-neutral backup, restore, status, and integrity operations.
    pub fn storage_operations(&self) -> Result<crate::storage::Operations<'_>> {
        Ok(crate::storage::Operations::new(self.storage()?))
    }

    /// Returns the injected signer, when outbound authoring is enabled.
    pub fn signer(&self) -> Result<Option<&dyn Signer>> {
        self.require_open()?;
        Ok(self.inner.signer.as_deref())
    }

    /// Returns the injected inbound source, when pull is enabled.
    pub fn source(&self) -> Result<Option<&dyn EventSource>> {
        self.require_open()?;
        Ok(self.inner.source.as_deref())
    }

    /// Returns the injected outbound sink, when delivery is enabled.
    pub fn sink(&self) -> Result<Option<&dyn EventSink>> {
        self.require_open()?;
        Ok(self.inner.sink.as_deref())
    }

    /// Returns client-scoped canonical synchronization operations, when configured.
    #[cfg(feature = "sync")]
    pub fn sync(&self) -> Result<Option<crate::sync::Operations<'_>>> {
        self.require_open()?;
        Ok(self.inner.sync.as_ref().map(crate::sync::Operations::new))
    }

    /// Returns farm commit operations when canonical synchronization is configured.
    #[cfg(feature = "sync")]
    pub fn farm(&self) -> Result<Option<crate::farm::Operations<'_>>> {
        Ok(self.sync()?.map(crate::farm::Operations::new))
    }

    /// Returns listing commit operations when canonical synchronization is configured.
    #[cfg(feature = "sync")]
    pub fn listing(&self) -> Result<Option<crate::listing::Operations<'_>>> {
        Ok(self.sync()?.map(crate::listing::Operations::new))
    }

    /// Returns trade operations when canonical synchronization is configured.
    #[cfg(feature = "sync")]
    pub fn trade(&self) -> Result<Option<crate::trade::Operations<'_>>> {
        let storage = self.storage()?;
        Ok(self
            .sync()?
            .map(|sync| crate::trade::Operations::new(storage, sync)))
    }

    /// Returns high-level shared social operations when the required explicit
    /// signer, transport, and synchronization composition is present.
    #[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
    pub fn social(&self) -> Result<SocialOperations<'_>> {
        self.require_open()?;
        if self.inner.sync.is_none()
            || self.inner.signing_slot.is_none()
            || self.inner.nostr_slot.is_none()
        {
            return Err(Error::shared_operation_unavailable());
        }
        Ok(SocialOperations { client: self })
    }

    /// Returns whether explicit close completed successfully or reached the
    /// lower storage commit point.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.inner.lifecycle.load(Ordering::Acquire) == CLOSED
    }

    /// Explicitly closes active storage resources across every client clone.
    ///
    /// Dropping this future before its first poll has no effect. Cancellation
    /// after close begins leaves the client unavailable and permits an
    /// explicit retry; it never reports rollback. Repeated completed close is
    /// idempotent. No worker, executor, or blocking `Drop` path is installed.
    pub async fn close(&self) -> Result<()> {
        loop {
            match self.inner.lifecycle.load(Ordering::Acquire) {
                CLOSED => return Ok(()),
                CLOSING => return Err(Error::close_in_progress()),
                state @ (OPEN | CLOSE_RETRY_REQUIRED) => {
                    if self
                        .inner
                        .lifecycle
                        .compare_exchange(state, CLOSING, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        break;
                    }
                }
                _ => return Err(Error::client_closed()),
            }
        }

        let attempt = CloseAttempt::new(Arc::clone(&self.inner));
        let close_result = radroots_storage::BackupSource::close(self.inner.storage.as_ref()).await;
        attempt.complete();
        close_result
            .map(|_| ())
            .map_err(Error::storage_close_failed)
    }

    fn require_open(&self) -> Result<()> {
        match self.inner.lifecycle.load(Ordering::Acquire) {
            OPEN => Ok(()),
            CLOSING | CLOSE_RETRY_REQUIRED => Err(Error::client_closing()),
            _ => Err(Error::client_closed()),
        }
    }

    #[cfg(feature = "sync")]
    fn sync_is_configured(&self) -> bool {
        self.inner.sync.is_some()
    }

    #[cfg(not(feature = "sync"))]
    fn sync_is_configured(&self) -> bool {
        false
    }
}

#[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
impl SocialOperations<'_> {
    /// Observes both transport directions without initiating relay work.
    pub async fn transport_health(&self) -> Result<TransportHealth> {
        use radroots_transport::capability::Availability as TransportAvailability;
        let slot = self.nostr()?;
        let configured = slot.targets().is_some();
        let source = radroots_transport::EventSource::status(slot)
            .await
            .map_err(|_| Error::shared_operation_failed_without_source())?;
        let sink = radroots_transport::EventSink::status(slot)
            .await
            .map_err(|_| Error::shared_operation_failed_without_source())?;
        Ok(TransportHealth {
            configured,
            source_available: source.availability() == TransportAvailability::Available,
            sink_available: sink.availability() == TransportAvailability::Available,
        })
    }

    /// Fetches, verifies, and durably ingests the latest profile for the active signer.
    pub async fn fetch_profile_for_signer(&self) -> Result<Option<ProfileEvent>> {
        let identity = self.identity()?;
        let selector = radroots_transport::source::FetchSelector::all()
            .with_kinds(vec![radroots_event::envelope::kind::KIND_PROFILE])
            .and_then(|selector| selector.with_authors(vec![identity.public_key()]))
            .map_err(|_| Error::invalid_host_configuration_without_source())?;
        let events = self.fetch(32, selector).await?;
        let mut profiles = events
            .into_iter()
            .filter_map(|event| profile_event(&event).ok())
            .collect::<Vec<_>>();
        profiles.sort_by_key(|event| std::cmp::Reverse(event.created_at));
        Ok(profiles.into_iter().next())
    }

    /// Fetches, verifies, and durably ingests a bounded kind-1 page.
    pub async fn fetch_posts(
        &self,
        limit: u16,
        since_unix_seconds: Option<u64>,
    ) -> Result<Vec<PostEvent>> {
        let mut selector = radroots_transport::source::FetchSelector::all()
            .with_kinds(vec![radroots_event::envelope::kind::KIND_POST])
            .map_err(|_| Error::invalid_host_configuration_without_source())?;
        if let Some(since) = since_unix_seconds {
            selector = selector
                .with_since_unix_seconds(since)
                .map_err(|_| Error::invalid_host_configuration_without_source())?;
        }
        let mut posts = self
            .fetch(limit, selector)
            .await?
            .into_iter()
            .map(|event| PostEvent {
                event_id: event.id_hex(),
                author: event.pubkey().to_hex(),
                created_at: event.created_at(),
                content: event.content().to_owned(),
            })
            .collect::<Vec<_>>();
        posts.sort_by_key(|event| std::cmp::Reverse(event.created_at));
        Ok(posts)
    }

    /// Publishes a complete media-free profile replacement.
    pub async fn publish_profile(&self, draft: ProfileDraft) -> Result<PublishReceipt> {
        let mut profile = radroots_event::profile::AuthoredProfile::new(draft.name)
            .map_err(Error::invalid_host_configuration)?;
        if let Some(value) = draft.display_name {
            profile = profile.with_display_name(value);
        }
        if let Some(value) = draft.about {
            profile = profile.with_about(value);
        }
        if let Some(value) = draft.nip05 {
            profile = profile.with_nip05(
                radroots_event::profile::Nip05Identifier::parse(value.as_str())
                    .map_err(Error::invalid_host_configuration)?,
            );
        }
        if let Some(value) = draft.bot {
            profile = profile.with_bot(value);
        }
        let parts =
            radroots_event_codec::profile::authored::authored_profile_to_wire_parts(&profile)
                .map_err(Error::invalid_host_configuration)?;
        self.publish("radroots.profile.metadata.v1", parts).await
    }

    /// Publishes one strict root kind-1 update.
    pub async fn publish_text(&self, content: impl Into<String>) -> Result<PublishReceipt> {
        let update = radroots_event::post::AuthoredUpdate::new(content)
            .map_err(Error::invalid_host_configuration)?;
        let parts = radroots_event_codec::post::authored::authored_update_to_wire_parts(&update);
        self.publish("radroots.social.update.v1", parts).await
    }

    /// Publishes one strict direct NIP-10 reply.
    pub async fn publish_reply(
        &self,
        content: impl Into<String>,
        root_event_id: &str,
        root_author: &str,
        relay_hint: Option<&str>,
    ) -> Result<PublishReceipt> {
        let reference = radroots_event::post::reply::Nip10ReplyReference::parse(
            root_event_id,
            root_author,
            relay_hint,
        )
        .map_err(Error::invalid_host_configuration)?;
        let reply = radroots_event::post::reply::AuthoredNip10Reply::direct(content, reference)
            .map_err(Error::invalid_host_configuration)?;
        let parts =
            radroots_event_codec::reply::authored::authored_nip10_reply_to_wire_parts(&reply);
        self.publish("radroots.social.reply.v1", parts).await
    }

    async fn fetch(
        &self,
        limit: u16,
        selector: radroots_transport::source::FetchSelector,
    ) -> Result<Vec<radroots_event::SignedEvent>> {
        let slot = self.nostr()?;
        let targets = slot
            .targets()
            .ok_or_else(Error::shared_operation_unavailable)?;
        let request_id = format!("sdk-fetch-{}", uuid::Uuid::new_v4());
        let deadline = now_unix_ms()?.saturating_add(30_000);
        let request = radroots_transport::FetchRequest::new(
            request_id,
            targets,
            radroots_transport::source::FetchBounds::new(limit, deadline)
                .map_err(|_| Error::invalid_host_configuration_without_source())?,
        )
        .map_err(|_| Error::invalid_host_configuration_without_source())?
        .with_selector(selector);
        let page = radroots_transport::EventSource::fetch(slot, request)
            .await
            .map_err(|_| Error::shared_operation_failed_without_source())?;
        let observed = page.events().to_vec();
        let receipt = self
            .client
            .sync()?
            .ok_or_else(Error::shared_operation_unavailable)?
            .ingest_batch(
                observed.clone(),
                &radroots_sync::ingest::RegistryPolicy::verified(),
            )
            .await;
        Ok(observed
            .into_iter()
            .zip(receipt.outcomes())
            .filter_map(|(observed, outcome)| {
                outcome.as_ref().ok().map(|_| observed.event().clone())
            })
            .collect())
    }

    async fn publish(
        &self,
        contract_id: &'static str,
        parts: radroots_event::wire::Nip01EventWireParts,
    ) -> Result<PublishReceipt> {
        use radroots_event::contract::AuthorRole;
        use radroots_signing::{Actor, actor::ActorSource, request::CancellationPolicy};
        use radroots_storage::{journal::IdempotencyKey, outbox::LeaseOwner};
        use radroots_sync::{PushRequest, policy::SyncId, push::DeliveryRunRequest};
        use radroots_transport::policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy};

        let identity = self.identity()?;
        let targets = self
            .nostr()?
            .targets()
            .ok_or_else(Error::shared_operation_unavailable)?;
        let operation_uuid = uuid::Uuid::new_v4();
        let operation_id =
            SyncId::new(*operation_uuid.as_bytes()).map_err(Error::invalid_host_configuration)?;
        let draft = radroots_event::EventDraft::new(
            contract_id,
            parts.kind,
            now_unix_ms()? / 1_000,
            parts.tags,
            parts.content,
            identity.public_key_hex(),
        )
        .map_err(Error::invalid_host_configuration)?;
        let actor = Actor::new(
            identity.public_key(),
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Any],
        )
        .map_err(Error::invalid_host_configuration)?;
        let request = PushRequest::new(
            operation_id,
            IdempotencyKey::parse(format!("sdk-{operation_uuid}"))
                .map_err(Error::invalid_host_configuration)?,
            actor,
            draft,
            targets,
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::any()),
            CancellationPolicy::PreservePublishedRequest,
        )
        .map_err(Error::invalid_host_configuration)?;
        let sync = self
            .client
            .sync()?
            .ok_or_else(Error::shared_operation_unavailable)?;
        let push = sync
            .sign_and_enqueue(request)
            .await
            .map_err(Error::shared_operation_failed)?;
        let event_id = push.outbox().request().payload().event().id_hex();
        let delivery = sync
            .deliver_pending(
                DeliveryRunRequest::new(
                    LeaseOwner::parse("radroots-sdk-host")
                        .map_err(Error::invalid_host_configuration)?,
                    SyncId::new(*uuid::Uuid::new_v4().as_bytes())
                        .map_err(Error::invalid_host_configuration)?,
                    30_000,
                    radroots_storage::outbox::OUTBOX_CLAIM_LIMIT_MAX,
                )
                .map_err(Error::invalid_host_configuration)?,
            )
            .await
            .map_err(Error::shared_operation_failed)?;
        Ok(PublishReceipt {
            event_id,
            replay: push.is_replay(),
            delivered: delivery.outcomes().iter().any(|outcome| {
                outcome.as_ref().is_ok_and(|record| {
                    record.item_id() == push.outbox().item_id()
                        && record.satisfaction()
                            != radroots_storage::outbox::SatisfactionResult::Pending
                })
            }),
        })
    }

    fn identity(&self) -> Result<crate::signing::LocalIdentity> {
        self.client
            .inner
            .signing_slot
            .as_ref()
            .and_then(crate::signing::Slot::identity)
            .ok_or_else(Error::shared_operation_unavailable)
    }

    fn nostr(&self) -> Result<&crate::transport::NostrSlot> {
        self.client
            .inner
            .nostr_slot
            .as_ref()
            .ok_or_else(Error::shared_operation_unavailable)
    }
}

#[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
fn now_unix_ms() -> Result<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .filter(|value| *value != 0)
        .ok_or_else(Error::shared_operation_unavailable)
}

#[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
fn profile_event(
    event: &radroots_event::SignedEvent,
) -> std::result::Result<
    ProfileEvent,
    radroots_event_codec::profile::inbound::RadrootsProfileMetadataParseError,
> {
    let profile =
        radroots_event_codec::profile::inbound::parse_inbound_profile_metadata(event.content())?;
    Ok(ProfileEvent {
        event_id: event.id_hex(),
        author: event.pubkey().to_hex(),
        created_at: event.created_at(),
        name: profile.name().map(str::to_owned),
        display_name: profile.display_name().map(str::to_owned),
        about: profile.about().map(str::to_owned),
        picture: profile.picture().map(|value| value.as_str().to_owned()),
        banner: profile.banner().map(|value| value.as_str().to_owned()),
        nip05: profile.nip05().map(|value| value.as_str().to_owned()),
        bot: profile.bot(),
    })
}

struct CloseAttempt {
    inner: Arc<ClientInner>,
    completed: bool,
}

impl CloseAttempt {
    fn new(inner: Arc<ClientInner>) -> Self {
        Self {
            inner,
            completed: false,
        }
    }

    fn complete(mut self) {
        self.inner.lifecycle.store(CLOSED, Ordering::Release);
        self.completed = true;
    }
}

impl Drop for CloseAttempt {
    fn drop(&mut self) {
        if !self.completed {
            self.inner
                .lifecycle
                .store(CLOSE_RETRY_REQUIRED, Ordering::Release);
        }
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Client")
            .field("signer", &self.inner.signer.is_some())
            .field("source", &self.inner.source.is_some())
            .field("sink", &self.inner.sink.is_some())
            .field("closed", &self.is_closed())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for ClientBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClientBuilder")
            .field("storage", &self.storage.is_some())
            .field("signer", &self.signer.is_some())
            .field("source", &self.source.is_some())
            .field("sink", &self.sink.is_some())
            .finish_non_exhaustive()
    }
}

#[cfg(all(test, feature = "memory"))]
mod tests {
    use super::*;
    use radroots_signing::{
        Error as SigningError, SignReceipt, SignRequest, SignerStatus, error::Kind,
        signer::BoxFuture as SigningFuture,
    };
    use radroots_transport::{
        DeliveryReceipt, DeliveryRequest, Error as TransportError, FetchPage, FetchRequest,
        SinkStatus, SourceStatus, source::BoxFuture as TransportFuture,
    };
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
    };

    struct TestSource;
    struct TestSink;
    struct TestSigner;

    impl EventSource for TestSource {
        fn status(&self) -> TransportFuture<'_, std::result::Result<SourceStatus, TransportError>> {
            Box::pin(async { Err(TransportError::UnsupportedOperation) })
        }

        fn fetch(
            &self,
            _request: FetchRequest,
        ) -> TransportFuture<'_, std::result::Result<FetchPage, TransportError>> {
            Box::pin(async { Err(TransportError::UnsupportedOperation) })
        }
    }

    impl EventSink for TestSink {
        fn status(&self) -> TransportFuture<'_, std::result::Result<SinkStatus, TransportError>> {
            Box::pin(async { Err(TransportError::UnsupportedOperation) })
        }

        fn deliver(
            &self,
            _request: DeliveryRequest,
        ) -> TransportFuture<'_, std::result::Result<DeliveryReceipt, TransportError>> {
            Box::pin(async { Err(TransportError::UnsupportedOperation) })
        }
    }

    impl Signer for TestSigner {
        fn status(&self) -> SigningFuture<'_, std::result::Result<SignerStatus, SigningError>> {
            Box::pin(async { Err(SigningError::new(Kind::InternalError)) })
        }

        fn sign(
            &self,
            _request: SignRequest,
        ) -> SigningFuture<'_, std::result::Result<SignReceipt, SigningError>> {
            Box::pin(async { Err(SigningError::new(Kind::InternalError)) })
        }
    }

    fn generation() -> SourceGeneration {
        SourceGeneration::new([1; 32]).expect("non-zero generation")
    }

    #[test]
    fn missing_storage_and_signer_without_sink_fail_closed() {
        assert!(matches!(
            ClientBuilder::new().build(),
            Err(error) if error.kind() == crate::error::ErrorKind::MissingStorage
        ));
        assert!(matches!(
            ClientBuilder::memory(generation())
                .signer(Arc::new(TestSigner))
                .build(),
            Err(error) if error.kind() == crate::error::ErrorKind::SignerWithoutSink
        ));
    }

    #[test]
    fn memory_local_source_only_and_sink_only_compositions_are_explicit() {
        let local = ClientBuilder::memory(generation()).build().expect("local");
        assert!(local.source().expect("source capability").is_none());
        assert!(local.sink().expect("sink capability").is_none());
        assert!(local.signer().expect("signer capability").is_none());

        let source = ClientBuilder::memory(generation())
            .source(Arc::new(TestSource))
            .build()
            .expect("source-only");
        assert!(source.source().expect("source capability").is_some());
        assert!(source.sink().expect("sink capability").is_none());

        let sink = ClientBuilder::memory(generation())
            .sink(Arc::new(TestSink))
            .build()
            .expect("sink-only");
        assert!(sink.source().expect("source capability").is_none());
        assert!(sink.sink().expect("sink capability").is_some());
    }

    #[test]
    fn signer_with_sink_is_valid_and_diagnostics_are_capability_only() {
        let client = ClientBuilder::memory(generation())
            .sink(Arc::new(TestSink))
            .signer(Arc::new(TestSigner))
            .build()
            .expect("outbound client");
        assert!(client.signer().expect("signer capability").is_some());
        assert_eq!(
            format!("{client:?}"),
            "Client { signer: true, source: false, sink: true, closed: false, .. }"
        );
    }

    #[cfg(feature = "local-signing")]
    #[test]
    fn module_scoped_signing_provider_configures_the_matching_capability() {
        let signer = radroots_nostr::signing::LocalSigner::generate().expect("local signer");
        let client = ClientBuilder::memory(generation())
            .sink(Arc::new(TestSink))
            .signing(crate::signing::Provider::local(signer))
            .build()
            .expect("client");
        let status = client
            .capabilities()
            .get(CapabilityId::LOCAL_SIGNING)
            .expect("local signing");
        assert!(status.is_compiled());
        assert!(status.is_configured());
        assert_eq!(status.availability(), Availability::Available);
    }

    #[cfg(all(feature = "sync", feature = "nostr", feature = "local-signing"))]
    #[tokio::test]
    async fn shared_mobile_composition_is_single_client_explicit_and_fail_closed() {
        let signing = crate::signing::Slot::new();
        let nostr = crate::transport::NostrSlot::new(crate::transport::RelayUrlPolicy::Local);
        let client = ClientBuilder::memory(generation())
            .signing(crate::signing::Provider::slot(signing.clone()))
            .nostr(nostr.clone())
            .host_sync(crate::sync::HostPolicy::standard())
            .build()
            .expect("shared client");

        let health = client
            .social()
            .expect("social composition")
            .transport_health()
            .await
            .expect("passive health");
        assert!(!health.is_configured());
        assert!(!health.is_source_available());
        assert!(!health.is_sink_available());
        assert!(matches!(
            client
                .social()
                .expect("social composition")
                .fetch_posts(1, None)
                .await,
            Err(error) if error.kind() == crate::error::ErrorKind::SharedOperationUnavailable
        ));

        let (_secret, identity) = signing.generate().expect("host key handoff");
        assert_eq!(signing.identity(), Some(identity));
        nostr
            .configure(["ws://127.0.0.1:7447"])
            .expect("relay selection");
        assert!(nostr.targets().is_some());
    }

    #[test]
    fn close_is_clone_shared_idempotent_and_rejects_later_capability_access() {
        let client = ClientBuilder::memory(generation()).build().expect("client");
        let clone = client.clone();
        assert!(!client.is_closed());
        block_on(client.close()).expect("first close");
        assert!(clone.is_closed());
        block_on(clone.close()).expect("repeated close");
        assert!(matches!(
            clone.storage(),
            Err(error) if error.kind() == crate::error::ErrorKind::ClientClosed
        ));
        assert!(matches!(
            client.source(),
            Err(error) if error.kind() == crate::error::ErrorKind::ClientClosed
        ));
    }

    #[test]
    fn close_cancellation_boundaries_are_explicit_and_retryable() {
        let client = ClientBuilder::memory(generation()).build().expect("client");
        let unpolled = client.close();
        drop(unpolled);
        assert!(client.storage().is_ok());

        client.inner.lifecycle.store(CLOSING, Ordering::Release);
        let attempt = CloseAttempt::new(Arc::clone(&client.inner));
        drop(attempt);
        assert!(matches!(
            client.storage(),
            Err(error) if error.kind() == crate::error::ErrorKind::ClientClosing
        ));
        block_on(client.close()).expect("retry close");
        assert!(client.is_closed());
    }

    #[test]
    fn concurrent_clones_converge_on_one_closed_state() {
        let client = ClientBuilder::memory(generation()).build().expect("client");
        let first = client.clone();
        let second = client.clone();
        let outcomes = std::thread::scope(|scope| {
            let first_close = scope.spawn(move || block_on(first.close()));
            let second_close = scope.spawn(move || block_on(second.close()));
            [
                first_close.join().expect("first thread"),
                second_close.join().expect("second thread"),
            ]
        });
        assert!(outcomes.iter().all(|outcome| {
            outcome.is_ok()
                || matches!(
                    outcome,
                    Err(error)
                        if error.kind() == crate::error::ErrorKind::CloseInProgress
                )
        }));
        if !client.is_closed() {
            block_on(client.close()).expect("finish close");
        }
        assert!(client.is_closed());
    }

    #[test]
    fn capability_reports_separate_configuration_degradation_and_lifecycle() {
        let local = ClientBuilder::memory(generation())
            .capability_availability(CapabilityId::CANONICAL_STORAGE, Availability::Degraded)
            .build()
            .expect("client");
        let report = local.capabilities();
        let storage = report
            .get(CapabilityId::CANONICAL_STORAGE)
            .expect("storage");
        assert!(storage.is_compiled());
        assert!(storage.is_configured());
        assert_eq!(storage.availability(), Availability::Degraded);

        let signing = report.get(CapabilityId::LOCAL_SIGNING).expect("signing");
        assert!(!signing.is_configured());
        assert!(matches!(
            signing.availability(),
            Availability::Unavailable | Availability::Unsupported
        ));

        block_on(local.close()).expect("close");
        assert_eq!(
            local
                .capabilities()
                .get(CapabilityId::BACKUP_RESTORE)
                .expect("backup")
                .availability(),
            Availability::Unavailable
        );
    }

    #[test]
    fn memory_storage_status_integrity_and_lifecycle_use_native_contracts() {
        use radroots_storage::status::{
            IntegrityHealth, ShutdownState, StorageBackend, StorageOpenMode, WriterPolicy,
        };

        let client = ClientBuilder::memory(generation()).build().expect("client");
        let status = block_on(client.storage_status()).expect("status");
        assert_eq!(status.backend(), StorageBackend::Memory);
        assert_eq!(status.open_mode(), StorageOpenMode::Create);
        assert_eq!(status.writer_policy(), WriterPolicy::NoWriter);
        assert_eq!(status.shutdown(), ShutdownState::Open);
        assert_eq!(
            block_on(client.storage_integrity())
                .expect("integrity")
                .health(),
            IntegrityHealth::Healthy
        );
        block_on(client.close()).expect("close");
        assert_eq!(
            block_on(client.storage_status())
                .expect_err("closed")
                .kind(),
            crate::error::ErrorKind::ClientClosed
        );
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn sqlite_builder_exposes_native_status_integrity_and_lifecycle() {
        use radroots_storage::status::{
            IntegrityHealth, ShutdownState, StorageBackend, StorageOpenMode, WriterPolicy,
        };

        let directory = tempfile::tempdir().expect("temporary directory");
        let paths = crate::storage::SqlitePaths::from_directory(directory.path()).expect("paths");
        let options =
            crate::storage::SqliteOptions::new(paths, crate::storage::SqliteOpenMode::Create)
                .with_source_generation(generation(), 1)
                .expect("source generation");
        let client = ClientBuilder::sqlite(options)
            .await
            .expect("open builder")
            .build()
            .expect("client");
        let status = client.storage_status().await.expect("status");
        assert_eq!(status.backend(), StorageBackend::Sqlite);
        assert_eq!(status.open_mode(), StorageOpenMode::Create);
        assert_eq!(status.writer_policy(), WriterPolicy::AdvisoryProcessLock);
        assert_eq!(status.shutdown(), ShutdownState::Open);
        assert!(status.wal_enabled());
        assert_ne!(status.busy_timeout_ms(), 0);
        assert_eq!(
            client
                .storage_integrity()
                .await
                .expect("integrity")
                .health(),
            IntegrityHealth::Unknown
        );
        client.close().await.expect("close");
        assert!(client.is_closed());
    }

    struct ThreadWaker;

    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            std::thread::current().unpark();
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = Waker::from(Arc::new(ThreadWaker));
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::park(),
            }
        }
    }
}
