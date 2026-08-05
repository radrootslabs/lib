//! Curated client construction and operation entry points.

#[cfg(any(feature = "nostr", feature = "full"))]
use std::sync::Arc;

#[cfg(any(feature = "nostr", feature = "full"))]
use radroots_transport::{EventSink, EventSource};

#[cfg(any(
    feature = "client",
    feature = "native",
    feature = "nostr",
    feature = "nip46",
    feature = "full"
))]
use crate::ClientBuilder;
use crate::transport::Profile;

/// Creates the safe ordinary client builder with deterministic in-process
/// storage and no transport, signer, runtime, worker, or file authority.
#[cfg(feature = "client")]
#[must_use]
pub fn memory() -> ClientBuilder {
    ClientBuilder::memory_default()
}

/// Returns the explicit no-transport profile used by ordinary local work.
#[must_use]
pub const fn local_only() -> Profile {
    Profile::local_only()
}

/// Adds caller-owned source and sink capabilities without connecting them or
/// selecting a fallback transport.
#[cfg(any(feature = "nostr", feature = "full"))]
#[must_use]
pub fn with_transport(
    builder: ClientBuilder,
    source: Arc<dyn EventSource>,
    sink: Arc<dyn EventSink>,
) -> ClientBuilder {
    builder.source(source).sink(sink)
}

/// Adds an explicitly constructed NIP-46 signer provider.
#[cfg(any(feature = "nip46", feature = "full"))]
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn with_nip46_signer(
    builder: ClientBuilder,
    provider: crate::signing::Provider,
) -> ClientBuilder {
    builder.signing(provider)
}

/// Explicitly opens validated native SQLite storage.
#[cfg(any(feature = "native", feature = "full"))]
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn native(options: crate::storage::SqliteOptions) -> crate::Result<ClientBuilder> {
    ClientBuilder::sqlite(options).await
}

/// Reports whether the concrete GeoNames capability was selected at compile
/// time. Asset inspection, acquisition, and database opening remain explicit.
#[cfg(any(feature = "geonames", feature = "full"))]
#[must_use]
pub const fn geonames_enabled() -> bool {
    true
}

/// Reports whether canonical knowledge contracts were selected at compile
/// time. Merely selecting the feature performs no event or storage operation.
#[cfg(any(feature = "knowledge", feature = "full"))]
#[must_use]
pub const fn knowledge_enabled() -> bool {
    true
}

#[cfg(all(test, feature = "full"))]
mod tests {
    use super::{
        Arc, EventSink, EventSource, Profile, geonames_enabled, knowledge_enabled, local_only,
        memory, with_transport,
    };
    use radroots_transport::{
        DeliveryReceipt, DeliveryRequest, Error as TransportError, FetchPage, FetchRequest,
        SinkFailure, SinkStatus, SourceStatus, outcome::Retryability,
        source::BoxFuture as TransportFuture,
    };

    struct TestSource;
    struct TestSink;

    impl EventSource for TestSource {
        fn status(&self) -> TransportFuture<'_, Result<SourceStatus, TransportError>> {
            Box::pin(async { Err(TransportError::UnsupportedOperation) })
        }

        fn fetch(
            &self,
            _request: FetchRequest,
        ) -> TransportFuture<'_, Result<FetchPage, TransportError>> {
            Box::pin(async { Err(TransportError::UnsupportedOperation) })
        }
    }

    impl EventSink for TestSink {
        fn status(&self) -> TransportFuture<'_, Result<SinkStatus, TransportError>> {
            Box::pin(async { Err(TransportError::UnsupportedOperation) })
        }

        fn deliver(
            &self,
            request: DeliveryRequest,
        ) -> TransportFuture<'_, Result<DeliveryReceipt, SinkFailure>> {
            Box::pin(async move {
                Err(SinkFailure::for_request(
                    &request,
                    "test_sink_unavailable",
                    Retryability::Terminal,
                    None,
                    None,
                    Vec::new(),
                )
                .expect("test sink failure"))
            })
        }
    }

    #[test]
    fn curated_builders_cover_every_directly_testable_entry_point() {
        let local = memory().build().expect("memory client");
        assert_eq!(local_only(), Profile::local_only());
        assert!(local.source().expect("source").is_none());

        let composed = with_transport(memory(), Arc::new(TestSource), Arc::new(TestSink))
            .build()
            .expect("transport client");
        assert!(composed.source().expect("source").is_some());
        assert!(composed.sink().expect("sink").is_some());

        assert!(geonames_enabled());
        assert!(knowledge_enabled());
    }
}
