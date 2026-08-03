use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

use futures_executor::block_on;
use radroots_event::{SignedEvent, wire::Nip01EventWire};
use radroots_protocol::runtime::v1::OperationId;
use radroots_storage::{
    Journal, Outbox,
    event::SourceGeneration,
    journal::{IdempotencyDigest, IdempotencyKey, OperationInstanceId, PrepareOperation},
    memory::MemoryStorage,
    outbox::{DeliveryPlanDigest, EnqueueOutboxItem, LeaseOwner, OutboxItemId, OutboxStage},
};
use radroots_storage_sqlite::{OpenMode, OpenOptions, Paths, SqliteStorage};
use radroots_sync::{
    Engine,
    policy::{Clock, DeadlinePolicy, Error, IdSource, OperationKind, SyncId, SyncStorage},
    push::DeliveryRunRequest,
};
use radroots_transport::{
    DeliveryReceipt, DeliveryRequest, Error as TransportError, EventSink, SinkStatus, Target,
    TargetSet, TransportId,
    capability::{Availability, Maturity, SinkCapabilities},
    outcome::DeliveryOutcome,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::{DeliveryPayload, DeliveryTargetReceipt},
};

struct SequenceClock(AtomicU64);

impl Clock for SequenceClock {
    fn now_unix_ms(&self) -> Result<u64, Error> {
        Ok(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

struct SequenceIds(AtomicU64);

impl IdSource for SequenceIds {
    fn next_id(&self, _operation: OperationKind) -> Result<SyncId, Error> {
        let byte = u8::try_from(self.0.fetch_add(1, Ordering::Relaxed))
            .map_err(|_| Error::InvalidSyncId)?;
        SyncId::new([byte; 16])
    }
}

struct RecoverySink(AtomicUsize);

impl EventSink for RecoverySink {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SinkStatus, TransportError>> {
        Box::pin(async {
            Ok(SinkStatus::new(
                TransportId::NOSTR,
                true,
                Maturity::Stable,
                Availability::Available,
                SinkCapabilities::DELIVER,
                "ready",
            ))
        })
    }

    fn deliver(
        &self,
        request: DeliveryRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<DeliveryReceipt, TransportError>> {
        let call = self.0.fetch_add(1, Ordering::Relaxed);
        Box::pin(async move {
            if call == 0 {
                return Err(TransportError::UnsupportedOperation);
            }
            DeliveryReceipt::for_request(
                &request,
                request
                    .target_set()
                    .targets()
                    .iter()
                    .cloned()
                    .map(|target| {
                        DeliveryTargetReceipt::attempted(target, DeliveryOutcome::accepted())
                    })
                    .collect(),
            )
        })
    }
}

fn signed_event() -> SignedEvent {
    let mut wire = Nip01EventWire {
        id: "0".repeat(64),
        pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".to_owned(),
        created_at: 1_800_000_100,
        kind: 0,
        tags: vec![],
        content: "reliability-scenario".to_owned(),
        sig: "42".repeat(64),
        extra: Default::default(),
    };
    wire.id = wire.computed_event_id().expect("event id").to_hex();
    let raw = serde_json::json!({
        "id": &wire.id,
        "pubkey": &wire.pubkey,
        "created_at": wire.created_at,
        "kind": wire.kind,
        "tags": &wire.tags,
        "content": &wire.content,
        "sig": &wire.sig,
    })
    .to_string();
    SignedEvent::from_wire_verified_id(wire, raw).expect("signed event")
}

fn enqueue_request() -> EnqueueOutboxItem {
    let target = Target::new(TransportId::NOSTR, "wss://reliability.example").expect("target");
    let request = DeliveryRequest::new(
        "sync-reliability",
        DeliveryPayload::new(signed_event()),
        TargetSet::new(vec![target]).expect("targets"),
        SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        10_000,
    )
    .expect("request");
    EnqueueOutboxItem::new(
        OutboxItemId::new([5; 16]).expect("item id"),
        OperationInstanceId::new([6; 16]).expect("operation id"),
        DeliveryPlanDigest::new([7; 32]),
        request,
        10,
    )
    .expect("enqueue")
}

fn engine(
    storage: Arc<dyn SyncStorage>,
    sink: Arc<RecoverySink>,
    clock_start: u64,
    id_start: u64,
) -> Engine {
    Engine::builder(
        storage,
        Arc::new(SequenceClock(AtomicU64::new(clock_start))),
        Arc::new(SequenceIds(AtomicU64::new(id_start))),
        DeadlinePolicy::new(1_000, 1_000, 1_000).expect("deadlines"),
    )
    .sink(sink)
    .build()
    .expect("engine")
}

fn run(seed: u8) -> DeliveryRunRequest {
    DeliveryRunRequest::new(
        LeaseOwner::parse("sync-reliability").expect("owner"),
        SyncId::new([seed; 16]).expect("seed"),
        100,
        1,
    )
    .expect("run")
}

async fn retry_scenario(storage: Arc<dyn SyncStorage>) {
    Journal::prepare(
        storage.as_ref(),
        PrepareOperation::new(
            OperationInstanceId::new([6; 16]).expect("operation id"),
            OperationId::SyncPush,
            IdempotencyKey::parse("sync-reliability").expect("idempotency key"),
            IdempotencyDigest::new([8; 32]),
            9,
        )
        .expect("prepare operation"),
    )
    .await
    .expect("prepare");
    Outbox::enqueue(storage.as_ref(), enqueue_request())
        .await
        .expect("enqueue");
    let sync = engine(
        storage,
        Arc::new(RecoverySink(AtomicUsize::new(0))),
        100,
        20,
    );
    let first = sync.deliver_pending(run(30)).await.expect("first attempt");
    assert_eq!(
        first.outcomes()[0].as_ref().expect("durable retry").stage(),
        OutboxStage::Retryable
    );
    let second = sync.deliver_pending(run(31)).await.expect("retry");
    assert_eq!(
        second.outcomes()[0]
            .as_ref()
            .expect("durable success")
            .stage(),
        OutboxStage::Satisfied
    );
    let status = sync.status(&[]).await.expect("status").to_protocol();
    assert_eq!(status.outbox.satisfied, 1);
}

#[test]
fn memory_runs_the_durable_retry_scenario() {
    let storage: Arc<dyn SyncStorage> = Arc::new(MemoryStorage::new(
        SourceGeneration::new([9; 32]).expect("generation"),
    ));
    block_on(retry_scenario(storage));
}

#[tokio::test]
async fn sqlite_recovers_retryable_work_after_reopen() {
    let directory = tempfile::tempdir().expect("directory");
    let paths = Paths::from_directory(directory.path()).expect("paths");
    {
        let store = Arc::new(
            SqliteStorage::open(
                OpenOptions::new(paths.clone(), OpenMode::Create)
                    .with_source_generation(SourceGeneration::new([10; 32]).expect("generation"), 1)
                    .expect("source generation"),
            )
            .await
            .expect("open"),
        );
        Journal::prepare(
            store.as_ref(),
            PrepareOperation::new(
                OperationInstanceId::new([6; 16]).expect("operation id"),
                OperationId::SyncPush,
                IdempotencyKey::parse("sync-reliability").expect("idempotency key"),
                IdempotencyDigest::new([8; 32]),
                9,
            )
            .expect("prepare operation"),
        )
        .await
        .expect("prepare");
        Outbox::enqueue(store.as_ref(), enqueue_request())
            .await
            .expect("enqueue");
        let storage: Arc<dyn SyncStorage> = store;
        let sync = engine(
            storage,
            Arc::new(RecoverySink(AtomicUsize::new(0))),
            100,
            20,
        );
        let first = sync.deliver_pending(run(40)).await.expect("first attempt");
        assert_eq!(
            first.outcomes()[0].as_ref().expect("retryable").stage(),
            OutboxStage::Retryable
        );
    }
    let store = Arc::new(
        SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadWriteExisting))
            .await
            .expect("reopen"),
    );
    let storage: Arc<dyn SyncStorage> = store;
    let sync = engine(
        storage,
        Arc::new(RecoverySink(AtomicUsize::new(1))),
        200,
        40,
    );
    let recovered = sync
        .deliver_pending(run(41))
        .await
        .expect("recovered retry");
    assert_eq!(
        recovered.outcomes()[0].as_ref().expect("satisfied").stage(),
        OutboxStage::Satisfied
    );
}
