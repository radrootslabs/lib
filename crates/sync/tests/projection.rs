use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use futures_executor::block_on;
use radroots_event::{
    SignedEvent,
    admission::{AdmissionPolicy, RawEvent, SignatureVerifier, VisibilityPolicy},
    draft::SignedEventParts,
    envelope::EventEnvelope,
    wire::compute_canonical_nip01_event_id,
};
use radroots_storage::{
    EventStore, ProjectionStore,
    event::{EventAdmission, SourceGeneration, StoredVisibleEvent},
    memory::MemoryStorage,
    projection::{ProjectionGeneration, ProjectionHealth, ProjectionId},
};
use radroots_sync::{
    Engine,
    policy::{Clock, DeadlinePolicy, Error, IdSource, OperationKind, SyncId, SyncStorage},
    projection::{Reducer, ReducerError, RefreshKind, RefreshRequest, RefreshState},
};
use radroots_transport::{
    Error as TransportError, EventSource, FetchPage, FetchRequest, SourceStatus, Target,
    TransportId,
    source::{EventProvenance, ObservedEvent},
};

const PUBKEY: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const CONTENT: &str = "{\"display_name\":\"Moss Street Farm\",\"bot\":false}";

struct MockSource;

impl EventSource for MockSource {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SourceStatus, TransportError>> {
        Box::pin(async { unreachable!("projection refresh does not inspect source") })
    }

    fn fetch(
        &self,
        _request: FetchRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<FetchPage, TransportError>> {
        Box::pin(async { unreachable!("projection refresh does not fetch") })
    }
}

struct TestClock(AtomicU64);

impl Clock for TestClock {
    fn now_unix_ms(&self) -> Result<u64, Error> {
        Ok(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

struct TestIds(Mutex<u8>);

impl IdSource for TestIds {
    fn next_id(&self, operation: OperationKind) -> Result<SyncId, Error> {
        assert_eq!(operation, OperationKind::Projection);
        let mut value = self.0.lock().expect("ids");
        let current = *value;
        *value += 1;
        SyncId::new([current; 16])
    }
}

struct Allow;

impl SignatureVerifier for Allow {
    fn verify_signature(&self, _event: &EventEnvelope) -> Result<(), radroots_event::Error> {
        Ok(())
    }
}

impl AdmissionPolicy for Allow {
    type Error = core::convert::Infallible;
    fn policy_id(&self) -> &'static str {
        "test.projection.admission.v1"
    }
    fn admit(
        &self,
        _event: &radroots_event::admission::ContractValidatedEvent,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl VisibilityPolicy for Allow {
    type Error = core::convert::Infallible;
    fn policy_id(&self) -> &'static str {
        "test.projection.visibility.v1"
    }
    fn make_visible(
        &self,
        _event: &radroots_event::admission::AdmittedEvent,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct CountingReducer {
    projection_id: ProjectionId,
    generation: ProjectionGeneration,
    fail: bool,
}

impl Reducer for CountingReducer {
    fn projection_id(&self) -> &ProjectionId {
        &self.projection_id
    }
    fn generation(&self) -> ProjectionGeneration {
        self.generation
    }
    fn reduce(
        &self,
        events: &[StoredVisibleEvent],
        prior_projected_rows: u64,
    ) -> Result<u64, ReducerError> {
        if self.fail {
            return Err(ReducerError);
        }
        prior_projected_rows
            .checked_add(u64::try_from(events.len()).expect("event count"))
            .ok_or(ReducerError)
    }
}

fn setup() -> (Engine, Arc<MemoryStorage>, ProjectionId) {
    let storage = Arc::new(MemoryStorage::new(
        SourceGeneration::new([9; 32]).expect("generation"),
    ));
    let storage_capability: Arc<dyn SyncStorage> = storage.clone();
    let engine = Engine::builder(
        storage_capability,
        Arc::new(TestClock(AtomicU64::new(1_000))),
        Arc::new(TestIds(Mutex::new(1))),
        DeadlinePolicy::new(100, 100, 100).expect("deadlines"),
    )
    .source(Arc::new(MockSource))
    .build()
    .expect("engine");
    (
        engine,
        storage,
        ProjectionId::parse("test.projection").expect("projection id"),
    )
}

fn signed_event(created_at: u64) -> SignedEvent {
    let tags: Vec<Vec<String>> = vec![];
    let id = compute_canonical_nip01_event_id(PUBKEY, created_at, 0, &tags, CONTENT)
        .expect("event id")
        .to_hex();
    let signature = "42".repeat(64);
    let raw_json = format!(
        "{{\"id\":\"{id}\",\"pubkey\":\"{PUBKEY}\",\"created_at\":{created_at},\"kind\":0,\"tags\":[],\"content\":{content:?},\"sig\":\"{signature}\"}}",
        content = CONTENT,
    );
    SignedEvent::new(SignedEventParts {
        id,
        pubkey: PUBKEY.to_owned(),
        created_at,
        kind: 0,
        tags,
        content: CONTENT.to_owned(),
        sig: signature,
        raw_json,
    })
    .expect("signed event")
}

fn seed(storage: &MemoryStorage, count: u64) {
    for offset in 0..count {
        let event = signed_event(1_800_000_100 + offset);
        let visible = RawEvent::new(event.envelope().clone())
            .verify_id()
            .expect("id")
            .verify_signature(&Allow)
            .expect("signature")
            .validate_contract()
            .expect("contract")
            .admit_with(&Allow)
            .expect("admission")
            .make_visible_with(&Allow)
            .expect("visibility");
        let target = Target::new(TransportId::NOSTR, "wss://relay.example").expect("target");
        let provenance = EventProvenance::new(
            TransportId::NOSTR,
            target.fingerprint().clone(),
            1_900_000_000_000 + offset,
        )
        .expect("provenance");
        block_on(
            storage.admit(
                EventAdmission::visible(ObservedEvent::new(event, provenance), visible)
                    .expect("visible admission"),
            ),
        )
        .expect("seed event");
    }
}

fn reducer(id: &ProjectionId, generation: u8, fail: bool) -> CountingReducer {
    CountingReducer {
        projection_id: id.clone(),
        generation: ProjectionGeneration::new([generation; 32]).expect("generation"),
        fail,
    }
}

#[test]
fn incremental_refresh_checkpoints_visible_events() {
    let (engine, storage, id) = setup();
    seed(&storage, 1);
    let reducer = reducer(&id, 1, false);
    let receipt = block_on(engine.refresh_projection(
        RefreshRequest::new(id.clone(), reducer.generation(), 10, 1).expect("request"),
        &reducer,
    ))
    .expect("refresh");
    assert_eq!(receipt.kind(), RefreshKind::Incremental);
    assert_eq!(receipt.state(), RefreshState::Complete);
    assert_eq!(receipt.events_reduced(), 1);
    assert_eq!(
        receipt.checkpoint().expect("checkpoint").projected_rows(),
        1
    );
    assert_eq!(
        block_on(ProjectionStore::status(&*storage, id))
            .expect("status")
            .expect("projection")
            .health(),
        ProjectionHealth::Ready
    );
}

#[test]
fn generation_change_rebuilds_and_reducer_failure_is_durable() {
    let (engine, storage, id) = setup();
    seed(&storage, 1);
    let first = reducer(&id, 1, false);
    block_on(engine.refresh_projection(
        RefreshRequest::new(id.clone(), first.generation(), 10, 1).expect("request"),
        &first,
    ))
    .expect("initial refresh");

    let replacement = reducer(&id, 2, false);
    let rebuilt = block_on(engine.refresh_projection(
        RefreshRequest::new(id.clone(), replacement.generation(), 10, 1).expect("request"),
        &replacement,
    ))
    .expect("rebuild");
    assert_eq!(rebuilt.kind(), RefreshKind::Rebuild);
    assert_eq!(rebuilt.state(), RefreshState::Complete);

    let failing = reducer(&id, 3, true);
    let failed = block_on(engine.refresh_projection(
        RefreshRequest::new(id.clone(), failing.generation(), 10, 1).expect("request"),
        &failing,
    ))
    .expect("normalized failure");
    assert_eq!(failed.state(), RefreshState::Failed);
    assert_eq!(
        block_on(ProjectionStore::status(&*storage, id))
            .expect("status")
            .expect("projection")
            .health(),
        ProjectionHealth::Failed
    );
}

#[test]
fn partial_rebuild_resumes_and_rejects_concurrent_generation() {
    let (engine, storage, id) = setup();
    seed(&storage, 2);
    let first = reducer(&id, 1, false);
    block_on(engine.refresh_projection(
        RefreshRequest::new(id.clone(), first.generation(), 10, 1).expect("request"),
        &first,
    ))
    .expect("initial refresh");

    let replacement = reducer(&id, 2, false);
    let partial = block_on(engine.refresh_projection(
        RefreshRequest::new(id.clone(), replacement.generation(), 1, 1).expect("request"),
        &replacement,
    ))
    .expect("partial rebuild");
    assert_eq!(partial.state(), RefreshState::Partial);
    assert!(partial.rebuild_ticket().is_some());

    let concurrent = reducer(&id, 3, false);
    assert_eq!(
        block_on(engine.refresh_projection(
            RefreshRequest::new(id.clone(), concurrent.generation(), 1, 1).expect("request"),
            &concurrent,
        )),
        Err(Error::StorageConflict)
    );

    let second = block_on(engine.refresh_projection(
        RefreshRequest::new(id.clone(), replacement.generation(), 1, 1).expect("request"),
        &replacement,
    ))
    .expect("second batch");
    assert_eq!(second.state(), RefreshState::Partial);
    let complete = block_on(engine.refresh_projection(
        RefreshRequest::new(id, replacement.generation(), 1, 1).expect("request"),
        &replacement,
    ))
    .expect("complete rebuild");
    assert_eq!(complete.state(), RefreshState::Complete);
}
