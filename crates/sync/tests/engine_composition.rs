use std::sync::Arc;

use futures_executor::block_on;
use radroots_protocol::runtime::v1::{OPERATION_SCHEMA_VERSION, SyncCapabilityState, SyncHealth};
use radroots_signing::{Error as SigningError, SignReceipt, SignRequest, Signer, SignerStatus};
use radroots_storage::{event::SourceGeneration, memory::MemoryStorage, projection::ProjectionId};
use radroots_sync::{
    Engine,
    policy::{Clock, DeadlinePolicy, Error, IdSource, OperationKind, SyncId, SyncStorage},
};
use radroots_transport::{
    DeliveryReceipt, DeliveryRequest, Error as TransportError, EventSink, EventSource, FetchPage,
    FetchRequest, SinkStatus, SourceStatus,
    capability::{Availability, Maturity, SinkCapabilities, SourceCapabilities},
};

struct MockSource;
struct MockSink;
struct MockSigner;
struct UnconfiguredSource;
struct FixedClock;
struct FixedIds;

type TestDependencies = (
    Arc<dyn SyncStorage>,
    Arc<dyn Clock>,
    Arc<dyn IdSource>,
    DeadlinePolicy,
);

impl EventSource for MockSource {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SourceStatus, TransportError>> {
        Box::pin(async {
            Ok(SourceStatus::new(
                radroots_transport::TransportId::NOSTR,
                true,
                Maturity::Stable,
                Availability::Available,
                SourceCapabilities::FETCH,
                "ready",
            ))
        })
    }

    fn fetch(
        &self,
        _request: FetchRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<FetchPage, TransportError>> {
        Box::pin(async { unreachable!("composition does not fetch") })
    }
}

impl EventSource for UnconfiguredSource {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SourceStatus, TransportError>> {
        Box::pin(async {
            Ok(SourceStatus::new(
                radroots_transport::TransportId::NOSTR,
                false,
                Maturity::Preview,
                Availability::Available,
                SourceCapabilities::FETCH,
                "not configured",
            ))
        })
    }

    fn fetch(
        &self,
        _request: FetchRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<FetchPage, TransportError>> {
        Box::pin(async { unreachable!("status does not fetch") })
    }
}

impl EventSink for MockSink {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SinkStatus, TransportError>> {
        Box::pin(async {
            Ok(SinkStatus::new(
                radroots_transport::TransportId::NOSTR,
                true,
                Maturity::Preview,
                Availability::Degraded,
                SinkCapabilities::DELIVER,
                "degraded",
            ))
        })
    }

    fn deliver(
        &self,
        _request: DeliveryRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<DeliveryReceipt, TransportError>> {
        Box::pin(async { unreachable!("composition does not deliver") })
    }
}

impl Signer for MockSigner {
    fn status(
        &self,
    ) -> radroots_signing::signer::BoxFuture<'_, Result<SignerStatus, SigningError>> {
        Box::pin(async { Ok(SignerStatus::unavailable()) })
    }

    fn sign(
        &self,
        _request: SignRequest,
    ) -> radroots_signing::signer::BoxFuture<'_, Result<SignReceipt, SigningError>> {
        Box::pin(async { unreachable!("composition does not sign") })
    }
}

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> Result<u64, Error> {
        Ok(1_700_000_000_000)
    }
}

impl IdSource for FixedIds {
    fn next_id(&self, operation: OperationKind) -> Result<SyncId, Error> {
        let byte = match operation {
            OperationKind::Pull => 1,
            OperationKind::Sign => 2,
            OperationKind::Deliver => 3,
            _ => 4,
        };
        SyncId::new([byte; 16])
    }
}

fn dependencies() -> TestDependencies {
    let generation = SourceGeneration::new([7; 32]).expect("generation");
    (
        Arc::new(MemoryStorage::new(generation)),
        Arc::new(FixedClock),
        Arc::new(FixedIds),
        DeadlinePolicy::new(10_000, 20_000, 30_000).expect("deadlines"),
    )
}

#[test]
fn source_only_sink_only_and_full_compositions_are_explicit() {
    let (storage, clock, ids, deadlines) = dependencies();
    let source_only = Engine::builder(storage, clock, ids, deadlines)
        .source(Arc::new(MockSource))
        .build()
        .expect("source engine");
    assert!(source_only.source().is_some());
    assert!(source_only.sink().is_none());
    assert!(source_only.signer().is_none());

    let (storage, clock, ids, deadlines) = dependencies();
    let sink_only = Engine::builder(storage, clock, ids, deadlines)
        .sink(Arc::new(MockSink))
        .build()
        .expect("sink engine");
    assert!(sink_only.source().is_none());
    assert!(sink_only.sink().is_some());
    assert!(sink_only.signer().is_none());

    let (storage, clock, ids, deadlines) = dependencies();
    let full = Engine::builder(storage, clock, ids, deadlines)
        .source(Arc::new(MockSource))
        .sink(Arc::new(MockSink))
        .signer(Arc::new(MockSigner))
        .build()
        .expect("full engine");
    assert!(full.source().is_some());
    assert!(full.sink().is_some());
    assert!(full.signer().is_some());
    assert_eq!(
        full.clock().now_unix_ms().expect("clock"),
        1_700_000_000_000
    );
    assert_eq!(
        full.deadlines()
            .deadline_unix_ms(OperationKind::Deliver, 1_000)
            .expect("deadline"),
        31_000
    );
}

#[test]
fn invalid_compositions_and_ambient_policy_inputs_fail_closed() {
    let (storage, clock, ids, deadlines) = dependencies();
    assert_eq!(
        Engine::builder(storage, clock, ids, deadlines)
            .build()
            .expect_err("missing transport"),
        Error::MissingTransportCapability
    );

    let (storage, clock, ids, deadlines) = dependencies();
    assert_eq!(
        Engine::builder(storage, clock, ids, deadlines)
            .signer(Arc::new(MockSigner))
            .build()
            .expect_err("signer without sink"),
        Error::SignerWithoutSink
    );
    assert_eq!(
        DeadlinePolicy::new(0, 1, 1),
        Err(Error::InvalidDeadlinePolicy)
    );
    assert_eq!(SyncId::new([0; 16]), Err(Error::InvalidSyncId));
}

#[test]
fn status_aggregates_typed_capability_and_protocol_reports() {
    let (storage, clock, ids, deadlines) = dependencies();
    let full = Engine::builder(storage, clock, ids, deadlines)
        .source(Arc::new(MockSource))
        .sink(Arc::new(MockSink))
        .signer(Arc::new(MockSigner))
        .build()
        .expect("full engine");
    let projection = ProjectionId::parse("market-listings").expect("projection id");
    let status = block_on(full.status(std::slice::from_ref(&projection))).expect("sync status");
    assert_eq!(status.health(), SyncHealth::Degraded);
    assert_eq!(status.source().state(), SyncCapabilityState::Available);
    assert_eq!(status.sink().state(), SyncCapabilityState::Degraded);
    assert_eq!(status.signer().state(), SyncCapabilityState::Configured);
    assert_eq!(status.projections()[0].projection_id(), &projection);
    assert!(status.projections()[0].status().is_none());
    let protocol = status.to_protocol();
    assert_eq!(protocol.schema_version, OPERATION_SCHEMA_VERSION);
    assert_eq!(protocol.health, SyncHealth::Degraded);
    assert_eq!(protocol.source, SyncCapabilityState::Available);
    assert_eq!(protocol.sink, SyncCapabilityState::Degraded);
    assert_eq!(protocol.signer, SyncCapabilityState::Configured);
    assert_eq!(protocol.projections.untracked, 1);

    let (storage, clock, ids, deadlines) = dependencies();
    let sink_only = Engine::builder(storage, clock, ids, deadlines)
        .sink(Arc::new(MockSink))
        .build()
        .expect("sink engine");
    let status = block_on(sink_only.status(&[])).expect("sink-only status");
    assert_eq!(status.source().state(), SyncCapabilityState::Unsupported);
    assert_eq!(status.signer().state(), SyncCapabilityState::Unsupported);

    let (storage, clock, ids, deadlines) = dependencies();
    let compiled = Engine::builder(storage, clock, ids, deadlines)
        .source(Arc::new(UnconfiguredSource))
        .build()
        .expect("unconfigured source engine");
    let status = block_on(compiled.status(&[])).expect("compiled status");
    assert_eq!(status.source().state(), SyncCapabilityState::Compiled);
    assert!(status.source().status().is_some());
    assert_eq!(status.sink().state(), SyncCapabilityState::Unsupported);

    assert_eq!(
        block_on(full.status(&[projection.clone(), projection])),
        Err(Error::InvalidStatusRequest)
    );
}
