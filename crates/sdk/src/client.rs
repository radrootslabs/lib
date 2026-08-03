//! Client construction and lifecycle.

use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

use radroots_signing::Signer;
use radroots_storage::Storage;
#[cfg(feature = "memory")]
use radroots_storage::{event::SourceGeneration, memory::MemoryStorage};
use radroots_transport::{EventSink, EventSource};

use crate::{Error, Result};

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
}

struct ClientInner {
    storage: Arc<dyn Storage>,
    signer: Option<Arc<dyn Signer>>,
    source: Option<Arc<dyn EventSource>>,
    sink: Option<Arc<dyn EventSink>>,
    #[cfg(feature = "sync")]
    sync: Option<radroots_sync::Engine>,
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

    /// Validates the selected capabilities and creates a client handle.
    pub fn build(self) -> Result<Client> {
        let storage = self.storage.ok_or(Error::MissingStorage)?;
        if self.signer.is_some() && self.sink.is_none() {
            return Err(Error::SignerWithoutSink);
        }
        Ok(Client {
            inner: Arc::new(ClientInner {
                storage,
                signer: self.signer,
                source: self.source,
                sink: self.sink,
                #[cfg(feature = "sync")]
                sync: self.sync,
                lifecycle: AtomicU8::new(OPEN),
            }),
        })
    }
}

impl Client {
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
                CLOSING => return Err(Error::CloseInProgress),
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
                _ => return Err(Error::ClientClosed),
            }
        }

        let attempt = CloseAttempt::new(Arc::clone(&self.inner));
        let close_result = radroots_storage::BackupSource::close(self.inner.storage.as_ref()).await;
        attempt.complete();
        close_result
            .map(|_| ())
            .map_err(|_| Error::StorageCloseFailed)
    }

    fn require_open(&self) -> Result<()> {
        match self.inner.lifecycle.load(Ordering::Acquire) {
            OPEN => Ok(()),
            CLOSING | CLOSE_RETRY_REQUIRED => Err(Error::ClientClosing),
            _ => Err(Error::ClientClosed),
        }
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
            Err(Error::MissingStorage)
        ));
        assert!(matches!(
            ClientBuilder::memory(generation())
                .signer(Arc::new(TestSigner))
                .build(),
            Err(Error::SignerWithoutSink)
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
        assert!(matches!(clone.storage(), Err(Error::ClientClosed)));
        assert!(matches!(client.source(), Err(Error::ClientClosed)));
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
        assert!(matches!(client.storage(), Err(Error::ClientClosing)));
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
        assert!(
            outcomes.iter().all(|outcome| {
                outcome.is_ok() || matches!(outcome, Err(Error::CloseInProgress))
            })
        );
        if !client.is_closed() {
            block_on(client.close()).expect("finish close");
        }
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
