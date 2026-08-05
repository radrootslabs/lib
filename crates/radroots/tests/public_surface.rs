#[test]
fn approved_module_skeleton_is_public() {
    #[allow(unused_imports)]
    use radroots::{
        client, event, farm, identity, knowledge, listing, signing, storage, sync, trade, transport,
    };
}

#[test]
fn root_exports_only_the_client_boundary() {
    fn assert_client<T: Clone + Send + Sync>() {}
    fn assert_builder<T: Send + Sync>() {}
    fn assert_error<T: std::error::Error + Send + Sync + 'static>() {}

    assert_client::<radroots::Client>();
    assert_builder::<radroots::ClientBuilder>();
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
    let client = radroots::client::memory().build().expect("memory client");
    assert!(!client.is_closed());
    assert!(radroots::client::local_only().is_local_only());
}

#[cfg(any(feature = "nostr", feature = "full"))]
#[test]
fn explicit_transport_composition_is_inert() {
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

    let client = radroots::client::with_transport(
        radroots::client::memory(),
        Arc::new(Source),
        Arc::new(Sink),
    )
    .build()
    .expect("explicit transport client");
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
