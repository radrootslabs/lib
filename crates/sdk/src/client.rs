//! Client construction and lifecycle.

use std::sync::Arc;

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
            }),
        })
    }
}

impl Client {
    /// Returns the injected canonical storage capability.
    #[must_use]
    pub fn storage(&self) -> &dyn Storage {
        self.inner.storage.as_ref()
    }

    /// Returns the injected signer, when outbound authoring is enabled.
    #[must_use]
    pub fn signer(&self) -> Option<&dyn Signer> {
        self.inner.signer.as_deref()
    }

    /// Returns the injected inbound source, when pull is enabled.
    #[must_use]
    pub fn source(&self) -> Option<&dyn EventSource> {
        self.inner.source.as_deref()
    }

    /// Returns the injected outbound sink, when delivery is enabled.
    #[must_use]
    pub fn sink(&self) -> Option<&dyn EventSink> {
        self.inner.sink.as_deref()
    }

    /// Returns the explicit synchronization engine, when configured.
    #[cfg(feature = "sync")]
    #[must_use]
    pub fn sync_engine(&self) -> Option<&radroots_sync::Engine> {
        self.inner.sync.as_ref()
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Client")
            .field("signer", &self.inner.signer.is_some())
            .field("source", &self.inner.source.is_some())
            .field("sink", &self.inner.sink.is_some())
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
        assert!(local.source().is_none());
        assert!(local.sink().is_none());
        assert!(local.signer().is_none());

        let source = ClientBuilder::memory(generation())
            .source(Arc::new(TestSource))
            .build()
            .expect("source-only");
        assert!(source.source().is_some());
        assert!(source.sink().is_none());

        let sink = ClientBuilder::memory(generation())
            .sink(Arc::new(TestSink))
            .build()
            .expect("sink-only");
        assert!(sink.source().is_none());
        assert!(sink.sink().is_some());
    }

    #[test]
    fn signer_with_sink_is_valid_and_diagnostics_are_capability_only() {
        let client = ClientBuilder::memory(generation())
            .sink(Arc::new(TestSink))
            .signer(Arc::new(TestSigner))
            .build()
            .expect("outbound client");
        assert!(client.signer().is_some());
        assert_eq!(
            format!("{client:?}"),
            "Client { signer: true, source: false, sink: true, .. }"
        );
    }
}
