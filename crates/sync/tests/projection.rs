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
    projection::{
        ProjectionGeneration, ProjectionHealth, ProjectionId, RawSourceDigest, RebuildFailure,
        RebuildTicketId,
    },
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
    regress: bool,
}

impl Reducer for CountingReducer {
    fn projection_id(&self) -> &ProjectionId {
        &self.projection_id
    }
    fn generation(&self) -> ProjectionGeneration {
        self.generation
    }
    fn begin_rebuild(
        &self,
        _ticket_id: RebuildTicketId,
        _source_generation: SourceGeneration,
        _source_digest: RawSourceDigest,
    ) -> Result<(), ReducerError> {
        if self.fail { Err(ReducerError) } else { Ok(()) }
    }
    fn reduce(
        &self,
        events: &[StoredVisibleEvent],
        prior_projected_rows: u64,
        _rebuild_ticket: Option<RebuildTicketId>,
    ) -> Result<u64, ReducerError> {
        if self.fail {
            return Err(ReducerError);
        }
        if self.regress {
            return Ok(prior_projected_rows.saturating_sub(1));
        }
        prior_projected_rows
            .checked_add(u64::try_from(events.len()).expect("event count"))
            .ok_or(ReducerError)
    }
    fn abort_rebuild(
        &self,
        _ticket_id: RebuildTicketId,
        _failure: RebuildFailure,
    ) -> Result<(), ReducerError> {
        Ok(())
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
        regress: false,
    }
}

#[test]
fn incremental_refresh_checkpoints_visible_events() {
    let (engine, storage, id) = setup();
    seed(&storage, 1);
    let reducer = reducer(&id, 1, false);
    let request = RefreshRequest::new(id.clone(), reducer.generation(), 10, 1).expect("request");
    assert_eq!(request.projection_id(), &id);
    assert_eq!(request.generation(), reducer.generation());
    assert_eq!(request.batch_limit(), 10);
    assert_eq!(request.max_batches(), 1);
    let receipt = block_on(engine.refresh_projection(request, &reducer)).expect("refresh");
    assert_eq!(receipt.kind(), RefreshKind::Incremental);
    assert_eq!(receipt.state(), RefreshState::Complete);
    assert_eq!(receipt.events_reduced(), 1);
    assert_eq!(receipt.batches(), 1);
    assert!(receipt.rebuild_ticket().is_none());
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
        ProjectionHealth::Ready
    );
    let retried_failure = block_on(
        engine.refresh_projection(
            RefreshRequest::new(failing.projection_id().clone(), failing.generation(), 10, 1)
                .expect("failed generation request"),
            &failing,
        ),
    )
    .expect("retry failed generation");
    assert_eq!(retried_failure.state(), RefreshState::Failed);
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
    let visible_status = block_on(ProjectionStore::status(&*storage, id.clone()))
        .expect("status")
        .expect("projection");
    assert_eq!(visible_status.generation(), first.generation());
    assert_eq!(visible_status.health(), ProjectionHealth::Rebuilding);

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
        RefreshRequest::new(id.clone(), replacement.generation(), 1, 1).expect("request"),
        &replacement,
    ))
    .expect("complete rebuild");
    assert_eq!(complete.state(), RefreshState::Complete);
    for invalid in [
        RefreshRequest::new(id.clone(), replacement.generation(), 0, 1),
        RefreshRequest::new(
            id.clone(),
            replacement.generation(),
            radroots_storage::event::EVENT_QUERY_LIMIT_MAX + 1,
            1,
        ),
        RefreshRequest::new(id.clone(), replacement.generation(), 1, 0),
        RefreshRequest::new(
            id,
            replacement.generation(),
            1,
            radroots_sync::projection::PROJECTION_REFRESH_MAX_BATCHES + 1,
        ),
    ] {
        assert_eq!(invalid, Err(Error::InvalidProjectionRequest));
    }
}

#[test]
fn source_change_fails_rebuild_and_preserves_prior_generation() {
    let (engine, storage, id) = setup();
    seed(&storage, 2);
    let active = reducer(&id, 1, false);
    block_on(engine.refresh_projection(
        RefreshRequest::new(id.clone(), active.generation(), 10, 1).expect("request"),
        &active,
    ))
    .expect("initial refresh");

    let replacement = reducer(&id, 2, false);
    let partial = block_on(engine.refresh_projection(
        RefreshRequest::new(id.clone(), replacement.generation(), 1, 1).expect("request"),
        &replacement,
    ))
    .expect("partial rebuild");
    let ticket_id = partial.rebuild_ticket().expect("ticket");
    seed(&storage, 3);

    let failed = block_on(engine.refresh_projection(
        RefreshRequest::new(id.clone(), replacement.generation(), 1, 1).expect("request"),
        &replacement,
    ))
    .expect("source change is normalized");
    assert_eq!(failed.state(), RefreshState::Failed);
    let status = block_on(ProjectionStore::status(&*storage, id))
        .expect("status")
        .expect("projection");
    assert_eq!(status.generation(), active.generation());
    assert_eq!(status.health(), ProjectionHealth::Ready);
    let ticket = block_on(storage.rebuild(ticket_id))
        .expect("ticket lookup")
        .expect("durable ticket");
    assert_eq!(ticket.failure(), Some(RebuildFailure::SourceChanged));
}

#[test]
fn reducer_identity_progress_and_multi_batch_boundaries_fail_closed() {
    let (engine, storage, id) = setup();
    seed(&storage, 3);
    let active_reducer = reducer(&id, 1, false);
    let request =
        RefreshRequest::new(id.clone(), active_reducer.generation(), 1, 2).expect("request");
    let wrong_id = reducer(
        &ProjectionId::parse("different-projection").expect("projection id"),
        1,
        false,
    );
    assert_eq!(
        block_on(engine.refresh_projection(request.clone(), &wrong_id)),
        Err(Error::InvalidProjectionRequest)
    );
    let wrong_generation = reducer(&id, 2, false);
    assert_eq!(
        block_on(engine.refresh_projection(request.clone(), &wrong_generation)),
        Err(Error::InvalidProjectionRequest)
    );
    let partial =
        block_on(engine.refresh_projection(request, &active_reducer)).expect("two batches");
    assert_eq!(partial.state(), RefreshState::Partial);
    assert_eq!(partial.batches(), 2);

    let (engine, storage, id) = setup();
    seed(&storage, 1);
    let failing = reducer(&id, 1, true);
    let failed = block_on(engine.refresh_projection(
        RefreshRequest::new(id.clone(), failing.generation(), 1, 1).expect("request"),
        &failing,
    ))
    .expect("normalized incremental failure");
    assert_eq!(failed.state(), RefreshState::Failed);
    assert!(failed.rebuild_ticket().is_none());

    let (engine, storage, id) = setup();
    seed(&storage, 1);
    let initial = reducer(&id, 1, false);
    block_on(engine.refresh_projection(
        RefreshRequest::new(id.clone(), initial.generation(), 1, 1).expect("request"),
        &initial,
    ))
    .expect("initial projection");
    seed(&storage, 2);
    let regressing = CountingReducer {
        projection_id: id.clone(),
        generation: initial.generation(),
        fail: false,
        regress: true,
    };
    assert_eq!(
        block_on(engine.refresh_projection(
            RefreshRequest::new(id, regressing.generation(), 1, 1).expect("request"),
            &regressing,
        )),
        Err(Error::InvalidReducerOutput)
    );
}
