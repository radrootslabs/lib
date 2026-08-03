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
    signer: Option<Arc<dyn Signer>>,
    source: Option<Arc<dyn EventSource>>,
    sink: Option<Arc<dyn EventSink>>,
    #[cfg(feature = "sync")]
    sync: Option<radroots_sync::Engine>,
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
    capability_availability: BTreeMap<CapabilityId, Availability>,
    explicitly_configured_capabilities: BTreeSet<CapabilityId>,
    lifecycle: AtomicU8,
}

const OPEN: u8 = 0;
const CLOSING: u8 = 1;
const CLOSE_RETRY_REQUIRED: u8 = 2;
const CLOSED: u8 = 3;

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
        Self::new().storage(Arc::new(MemoryStorage::new(generation)))
    }

    /// Explicitly opens canonical SQLite storage from validated host-owned
    /// configuration and returns a builder containing only the storage SPI.
    #[cfg(feature = "sqlite")]
    pub async fn sqlite(options: crate::storage::SqliteOptions) -> Result<Self> {
        let storage = radroots_storage_sqlite::SqliteStorage::open(options)
            .await
            .map_err(Error::storage_open_failed)?;
        Ok(Self::new()
            .storage(Arc::new(storage))
            .capability_availability(CapabilityId::PERSISTENT_STORAGE, Availability::Available))
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

    /// Injects an explicitly composed synchronization engine.
    #[cfg(feature = "sync")]
    #[must_use]
    pub fn sync_engine(mut self, sync: radroots_sync::Engine) -> Self {
        self.sync = Some(sync);
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
    pub fn build(self) -> Result<Client> {
        let storage = self.storage.ok_or_else(Error::missing_storage)?;
        if self.signer.is_some() && self.sink.is_none() {
            return Err(Error::signer_without_sink());
        }
        Ok(Client {
            inner: Arc::new(ClientInner {
                storage,
                signer: self.signer,
                source: self.source,
                sink: self.sink,
                #[cfg(feature = "sync")]
                sync: self.sync,
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

    /// Returns the explicit synchronization engine, when configured.
    #[cfg(feature = "sync")]
    pub fn sync_engine(&self) -> Result<Option<&radroots_sync::Engine>> {
        self.require_open()?;
        Ok(self.inner.sync.as_ref())
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
