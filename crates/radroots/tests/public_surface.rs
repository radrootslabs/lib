#[test]
fn approved_module_skeleton_is_public() {
    #[allow(unused_imports)]
    use radroots::{
        client, event, farm, identity, knowledge, listing, signing, storage, sync, trade, transport,
    };
}

#[test]
fn root_exports_only_the_client_boundary() {
    fn assert_client<T: Clone>() {}
    #[cfg(not(target_arch = "wasm32"))]
    fn assert_native_client<T: Send + Sync>() {}
    fn assert_error<T: std::error::Error + Send + Sync + 'static>() {}

    assert_client::<radroots::Client>();
    #[cfg(not(target_arch = "wasm32"))]
    assert_native_client::<radroots::Client>();
    assert_error::<radroots::Error>();
    let result: radroots::Result<()> = Ok(());
    assert!(result.is_ok());
}

#[test]
fn canonical_domain_paths_compile() {
    #[allow(unused_imports)]
    use radroots::{
        event::{Event, EventId, GenericEventDraft},
        farm::{Farm, FarmPublicLocation, Plan as FarmPlan, PrepareRequest as FarmRequest},
        identity::{AccountId, PublicKey, Username},
        listing::{EditV1, OperationalListing, Plan as ListingPlan},
        signing::{Mode, Provider},
        storage::{IntegrityStatus, Operations as StorageOperations, Status},
        trade::{Money, MutationV1, Plan as TradePlan, Projection},
        transport::{Profile, SatisfactionPolicy, Target, TargetSet, TransportId},
    };
}

#[cfg(feature = "client")]
#[test]
fn ordinary_memory_and_local_only_construction_is_inert() {
    let client = radroots::Client::preview();
    assert_eq!(
        client.state(),
        radroots_execution::session::SessionState::Ready
    );
    assert!(radroots::client::local_only().is_local_only());
}

#[cfg(any(feature = "nostr", feature = "full"))]
#[test]
fn retained_advanced_transport_composition_is_inert() {
    use std::sync::Arc;

    use radroots::transport::{EventSink, EventSource};
    use radroots_transport::{
        DeliveryReceipt, DeliveryRequest, Error, FetchPage, FetchRequest, SinkFailure, SinkStatus,
        SourceStatus, outcome::Retryability, source::BoxFuture,
    };

    struct Source;
    struct Sink;

    impl EventSource for Source {
        fn status(&self) -> BoxFuture<'_, Result<SourceStatus, Error>> {
            Box::pin(async { Err(Error::UnsupportedOperation) })
        }

        fn fetch(&self, _request: FetchRequest) -> BoxFuture<'_, Result<FetchPage, Error>> {
            Box::pin(async { Err(Error::UnsupportedOperation) })
        }
    }

    impl EventSink for Sink {
        fn status(&self) -> BoxFuture<'_, Result<SinkStatus, Error>> {
            Box::pin(async { Err(Error::UnsupportedOperation) })
        }

        fn deliver(
            &self,
            request: DeliveryRequest,
        ) -> BoxFuture<'_, Result<DeliveryReceipt, SinkFailure>> {
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

    // The retained advanced SDK owns this source/sink composition until its
    // concrete ordinary network host is qualified. This does not claim the
    // new preview/native Client implements a network worker.
    let client = radroots_sdk::ClientBuilder::memory_default()
        .source(Arc::new(Source))
        .sink(Arc::new(Sink))
        .build()
        .expect("explicit advanced transport client");
    assert!(client.source().expect("source").is_some());
    assert!(client.sink().expect("sink").is_some());
}

#[cfg(any(feature = "geonames", feature = "full"))]
#[test]
fn geonames_selection_is_explicit() {
    assert!(radroots::client::geonames_enabled());
}

#[cfg(any(feature = "knowledge", feature = "full"))]
#[test]
fn knowledge_selection_is_explicit() {
    assert!(radroots::client::knowledge_enabled());
}

#[cfg(any(feature = "knowledge", feature = "full"))]
#[test]
fn canonical_knowledge_paths_compile() {
    #[allow(unused_imports)]
    use radroots::knowledge::{KnowledgeClaim, WikiArticle, normalize_wiki_d_tag};
}

#[cfg(feature = "client")]
#[test]
fn preview_handles_share_explicit_start_and_actual_memory_close() {
    use radroots::{Client, StartMode};
    use radroots_execution::session::SessionState;
    use radroots_storage::status::{ShutdownState, StorageBackend};
    let client = Client::preview();
    let clone = client.clone();
    let status = futures::executor::block_on(client.storage_status()).unwrap();
    assert_eq!(status.backend(), StorageBackend::Memory);
    assert_eq!(status.shutdown(), ShutdownState::Open);
    assert!(client.signer_session().is_none());
    let started = futures::executor::block_on(client.start(StartMode::LocalOnly)).unwrap();
    assert_eq!(started.state(), SessionState::Running);
    assert_eq!(clone.state(), SessionState::Running);
    let receipt = futures::executor::block_on(clone.close()).unwrap();
    assert_eq!(receipt.storage().shutdown(), ShutdownState::Closed);
    assert_eq!(client.state(), SessionState::Closed);
}
