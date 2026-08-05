use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

use futures::{FutureExt, task::noop_waker_ref};
use futures_executor::block_on;
use radroots_event::{
    GenericEventDraft, SignedEvent, contract::AuthorRole, draft::SignedEventParts,
};
use radroots_event_codec::authoring::AuthoredEventPlan;
use radroots_protocol::runtime::v1::SyncRetryDecision;
use radroots_signing::{
    Actor, Error as SigningError, SignReceipt, SignRequest, Signer, SignerStatus,
    actor::ActorSource, error::Kind as SigningErrorKind, request::CancellationPolicy,
};
use radroots_storage::{
    EventStore, Journal, Outbox,
    event::{EventQuery, EventQueryBounds, SourceGeneration},
    journal::{IdempotencyKey, JournalStage, OperationInstanceId},
    memory::MemoryStorage,
    outbox::{ClaimOutboxItems, LeaseId, LeaseOwner, OutboxStage, SatisfactionResult},
};
use radroots_storage_sqlite::{OpenMode, OpenOptions, Paths, SqliteStorage};
use radroots_sync::{
    Engine, PushRequest,
    policy::{Clock, DeadlinePolicy, Error, IdSource, OperationKind, SyncId, SyncStorage},
    push::DeliveryRunRequest,
};
use radroots_transport::{
    DeliveryReceipt, DeliveryRequest, Error as TransportError, EventSink, SinkFailure, SinkStatus,
    Target, TargetSet, TransportId,
    outcome::{DeliveryOutcome, Retryability},
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::DeliveryTargetReceipt,
};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey};

const CONTENT: &str = "frozen-content";

struct MockSink;

impl EventSink for MockSink {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SinkStatus, TransportError>> {
        Box::pin(async { unreachable!("enqueue does not inspect sink") })
    }
    fn deliver(
        &self,
        _request: DeliveryRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<DeliveryReceipt, SinkFailure>> {
        Box::pin(async { unreachable!("enqueue does not deliver") })
    }
}

enum DeliveryBehavior {
    Outcomes(Vec<DeliveryOutcome>),
    AdapterError,
    MismatchedRequest,
}

struct ScriptedSink {
    behaviors: Mutex<VecDeque<DeliveryBehavior>>,
    requests: Mutex<Vec<DeliveryRequest>>,
}

impl ScriptedSink {
    fn new(behaviors: impl IntoIterator<Item = DeliveryBehavior>) -> Self {
        Self {
            behaviors: Mutex::new(behaviors.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
        }
    }
}

impl EventSink for ScriptedSink {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SinkStatus, TransportError>> {
        Box::pin(async { unreachable!("delivery does not inspect sink status") })
    }

    fn deliver(
        &self,
        request: DeliveryRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<DeliveryReceipt, SinkFailure>> {
        self.requests
            .lock()
            .expect("scripted request lock")
            .push(request.clone());
        let behavior = self
            .behaviors
            .lock()
            .expect("scripted behavior lock")
            .pop_front()
            .expect("scripted delivery behavior");
        Box::pin(async move {
            match behavior {
                DeliveryBehavior::Outcomes(outcomes) => {
                    Ok(receipt(&request, outcomes).expect("valid scripted receipt"))
                }
                DeliveryBehavior::AdapterError => Err(SinkFailure::for_request(
                    &request,
                    "adapter_unavailable",
                    Retryability::Retryable,
                    None,
                    None,
                    Vec::new(),
                )
                .expect("valid scripted failure")),
                DeliveryBehavior::MismatchedRequest => {
                    let mismatched = DeliveryRequest::new(
                        "mismatched-request",
                        request.payload().clone(),
                        request.target_set().clone(),
                        request.satisfaction().clone(),
                        request.deadline_unix_ms(),
                    )
                    .expect("mismatched request");
                    Ok(receipt(
                        &mismatched,
                        vec![DeliveryOutcome::accepted(); mismatched.target_set().len()],
                    )
                    .expect("mismatched receipt"))
                }
            }
        })
    }
}

struct TestClock(AtomicU64);

impl Clock for TestClock {
    fn now_unix_ms(&self) -> Result<u64, Error> {
        Ok(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

struct TestIds(AtomicU64);

impl IdSource for TestIds {
    fn next_id(&self, _operation: OperationKind) -> Result<SyncId, Error> {
        let value = self.0.fetch_add(1, Ordering::Relaxed);
        let byte = u8::try_from(value).map_err(|_| Error::InvalidSyncId)?;
        SyncId::new([byte; 16])
    }
}

#[derive(Clone, Copy)]
enum SignBehavior {
    Success { completed_at_unix_ms: u64 },
    Error(SigningErrorKind),
    Pending,
}

struct MockSigner {
    behavior: SignBehavior,
    calls: AtomicUsize,
}

impl MockSigner {
    fn new(behavior: SignBehavior) -> Self {
        Self {
            behavior,
            calls: AtomicUsize::new(0),
        }
    }
}

impl Signer for MockSigner {
    fn status(
        &self,
    ) -> radroots_signing::signer::BoxFuture<'_, Result<SignerStatus, SigningError>> {
        Box::pin(async { unreachable!("enqueue does not inspect signer status") })
    }

    fn sign(
        &self,
        request: SignRequest,
    ) -> radroots_signing::signer::BoxFuture<'_, Result<SignReceipt, SigningError>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Box::pin(async move {
            match self.behavior {
                SignBehavior::Success {
                    completed_at_unix_ms,
                } => SignReceipt::from_signed_event(
                    &request,
                    signed_event(&request),
                    completed_at_unix_ms,
                ),
                SignBehavior::Error(kind) => Err(SigningError::new(kind)),
                SignBehavior::Pending => std::future::pending().await,
            }
        })
    }
}

fn signing_keypair() -> Keypair {
    let secret = SecretKey::from_slice(&[1; 32]).expect("secret key");
    Keypair::from_secret_key(&Secp256k1::new(), &secret)
}

fn public_key_hex() -> String {
    signing_keypair().x_only_public_key().0.to_string()
}

fn signed_event(request: &SignRequest) -> SignedEvent {
    let plan = request.plan();
    let id = plan.expected_event_id().to_hex();
    let pubkey = public_key_hex();
    let signature = Secp256k1::new()
        .sign_schnorr_no_aux_rand(
            &Message::from_digest(*plan.expected_event_id().as_bytes()),
            &signing_keypair(),
        )
        .to_string();
    let raw_json = format!(
        "{{\"id\":\"{id}\",\"pubkey\":\"{pubkey}\",\"created_at\":{},\"kind\":{},\"tags\":{:?},\"content\":{content:?},\"sig\":\"{signature}\"}}",
        plan.created_at(),
        plan.body().kind(),
        plan.body().tags(),
        content = plan.body().content(),
    );
    SignedEvent::new(SignedEventParts {
        id,
        pubkey,
        created_at: plan.created_at(),
        kind: plan.body().kind(),
        tags: plan.body().tags().to_vec(),
        content: plan.body().content().to_owned(),
        sig: signature,
        raw_json,
    })
    .expect("signed event")
}

fn request(operation_byte: u8, relay: &str) -> PushRequest {
    request_with_policy(
        operation_byte,
        &[relay],
        SatisfactionClass::Accepted,
        TargetPolicy::any(),
    )
}

fn request_with_policy(
    operation_byte: u8,
    relays: &[&str],
    class: SatisfactionClass,
    target_policy: TargetPolicy,
) -> PushRequest {
    let pubkey = public_key_hex();
    let plan = AuthoredEventPlan::from_generic(
        GenericEventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            1_800_000_100,
            vec![],
            CONTENT,
            pubkey,
        )
        .expect("draft"),
    )
    .expect("authored plan");
    let actor = Actor::new(
        *plan.author(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Any],
    )
    .expect("actor");
    PushRequest::new(
        SyncId::new([operation_byte; 16]).expect("operation id"),
        IdempotencyKey::parse(format!("push-{operation_byte}")).expect("idempotency key"),
        actor,
        plan,
        TargetSet::new(
            relays
                .iter()
                .map(|relay| Target::new(TransportId::NOSTR, *relay).expect("target"))
                .collect(),
        )
        .expect("targets"),
        SatisfactionPolicy::new(class, target_policy),
        1_800_000_300_000,
        CancellationPolicy::PreservePublishedRequest,
    )
    .expect("push request")
}

fn receipt(
    request: &DeliveryRequest,
    outcomes: Vec<DeliveryOutcome>,
) -> Result<DeliveryReceipt, TransportError> {
    let targets = request
        .target_set()
        .targets()
        .iter()
        .cloned()
        .zip(outcomes)
        .map(|(target, outcome)| DeliveryTargetReceipt::attempted(target, outcome))
        .collect();
    DeliveryReceipt::for_request(request, targets)
}

fn setup_engine(signer: Arc<MockSigner>) -> (Engine, Arc<MemoryStorage>) {
    setup_engine_with_sink(signer, Arc::new(MockSink)).0
}

fn setup_engine_with_sink(
    signer: Arc<MockSigner>,
    sink: Arc<dyn EventSink>,
) -> ((Engine, Arc<MemoryStorage>), Arc<TestClock>) {
    let storage = Arc::new(MemoryStorage::new(
        SourceGeneration::new([6; 32]).expect("generation"),
    ));
    let capability: Arc<dyn SyncStorage> = storage.clone();
    let clock = Arc::new(TestClock(AtomicU64::new(1_800_000_200_000)));
    let engine = Engine::builder(
        capability,
        clock.clone(),
        Arc::new(TestIds(AtomicU64::new(10))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
    )
    .sink(sink)
    .signer(signer)
    .build()
    .expect("engine");
    ((engine, storage), clock)
}

fn delivery_run(seed: u8, limit: u16) -> DeliveryRunRequest {
    DeliveryRunRequest::new(
        LeaseOwner::parse("sync-delivery-test").expect("lease owner"),
        SyncId::new([seed; 16]).expect("lease seed"),
        1_000,
        limit,
    )
    .expect("delivery run")
}

#[test]
fn authorized_signing_atomically_enqueues_and_replays_without_resigning() {
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let (engine, storage) = setup_engine(signer.clone());
    let request = request(1, "wss://relay.example");
    assert_eq!(request.operation_id().as_bytes(), &[1; 16]);
    assert_eq!(request.idempotency_key().as_str(), "push-1");
    assert_ne!(request.actor().public_key().as_bytes(), &[0; 32]);
    assert!(!request.plan().body().content().is_empty());
    assert_eq!(request.targets().len(), 1);
    assert_eq!(request.satisfaction().class(), SatisfactionClass::Accepted);
    assert_eq!(
        request.cancellation(),
        CancellationPolicy::PreservePublishedRequest
    );
    assert!(format!("{request:?}").contains("redacted exact authored plan"));
    let receipt = block_on(engine.sign_and_enqueue(request.clone())).expect("enqueue");
    assert_eq!(receipt.operation_id().as_bytes(), &[1; 16]);
    assert!(!receipt.is_replay());
    assert_eq!(receipt.outbox().stage(), OutboxStage::Pending);
    assert_eq!(signer.calls.load(Ordering::Relaxed), 1);
    let replay = block_on(engine.sign_and_enqueue(request)).expect("replay");
    assert!(replay.is_replay());
    assert_eq!(replay.outbox(), receipt.outbox());
    assert_eq!(signer.calls.load(Ordering::Relaxed), 1);

    let visible = block_on(storage.query_visible(EventQuery::all(
        EventQueryBounds::first(10).expect("bounds"),
    )))
    .expect("visible events");
    assert_eq!(visible.items().len(), 1);
    assert!(
        block_on(Outbox::item(
            &*storage,
            radroots_storage::outbox::OutboxItemId::new([1; 16]).expect("item id"),
        ))
        .expect("outbox item")
        .is_some()
    );
}

#[test]
fn preparation_is_atomic_status_visible_and_replays_without_external_effects() {
    let signer = Arc::new(MockSigner::new(SignBehavior::Pending));
    let (engine, storage) = setup_engine(signer.clone());
    let push = request(6, "wss://relay.example");

    let never_polled = engine.prepare_push(push.clone());
    drop(never_polled);
    assert!(
        block_on(engine.push_status(push.operation_id()))
            .expect("status before prepare")
            .is_none()
    );

    let prepared = block_on(engine.prepare_push(push.clone())).expect("prepare");
    assert!(!prepared.is_replay());
    assert_eq!(
        prepared.operation().artifact_ids(),
        &[prepared.artifact().artifact_id()]
    );
    assert_eq!(
        prepared.delivery_plan().artifact_id(),
        prepared.artifact().artifact_id()
    );
    assert!(prepared.delivery_plan().request().is_none());
    assert!(
        block_on(Journal::operation(
            &*storage,
            OperationInstanceId::new(*push.operation_id().as_bytes()).expect("operation"),
        ))
        .expect("legacy journal lookup")
        .is_none()
    );

    let status = block_on(engine.push_status(push.operation_id()))
        .expect("status after prepare")
        .expect("prepared status");
    assert_eq!(status.operation(), prepared.operation());
    assert_eq!(status.artifact(), prepared.artifact());
    assert_eq!(status.delivery_plan(), prepared.delivery_plan());

    let replay = block_on(engine.prepare_push(push)).expect("exact replay");
    assert!(replay.is_replay());
    assert_eq!(replay.operation(), prepared.operation());
    assert_eq!(replay.artifact(), prepared.artifact());
    assert_eq!(replay.delivery_plan(), prepared.delivery_plan());
    assert_eq!(signer.calls.load(Ordering::Relaxed), 0);

    assert_eq!(
        block_on(engine.prepare_push(request(6, "wss://other.example"))),
        Err(Error::StorageConflict)
    );
    assert_eq!(signer.calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn sqlite_preparation_reopens_and_replays_the_original_complete_intent() {
    let directory = tempfile::tempdir().expect("database directory");
    let paths = Paths::from_directory(directory.path()).expect("paths");
    let push = request(7, "wss://relay.example");
    let original = {
        let store = Arc::new(
            SqliteStorage::open(
                OpenOptions::new(paths.clone(), OpenMode::Create)
                    .with_source_generation(SourceGeneration::new([7; 32]).expect("generation"), 1)
                    .expect("source generation"),
            )
            .await
            .expect("open SQLite"),
        );
        let capability: Arc<dyn SyncStorage> = store;
        let signer = Arc::new(MockSigner::new(SignBehavior::Pending));
        let engine = Engine::builder(
            capability,
            Arc::new(TestClock(AtomicU64::new(1_800_000_200_000))),
            Arc::new(TestIds(AtomicU64::new(80))),
            DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
        )
        .sink(Arc::new(MockSink))
        .signer(signer.clone())
        .build()
        .expect("engine");
        let prepared = engine.prepare_push(push.clone()).await.expect("prepare");
        assert!(!prepared.is_replay());
        assert_eq!(signer.calls.load(Ordering::Relaxed), 0);
        prepared
    };

    let store = Arc::new(
        SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadWriteExisting))
            .await
            .expect("reopen SQLite"),
    );
    let capability: Arc<dyn SyncStorage> = store;
    let signer = Arc::new(MockSigner::new(SignBehavior::Pending));
    let engine = Engine::builder(
        capability,
        Arc::new(TestClock(AtomicU64::new(1_800_000_210_000))),
        Arc::new(TestIds(AtomicU64::new(90))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
    )
    .sink(Arc::new(MockSink))
    .signer(signer.clone())
    .build()
    .expect("engine");
    let status = engine
        .push_status(push.operation_id())
        .await
        .expect("status")
        .expect("durable preparation");
    assert_eq!(status.operation(), original.operation());
    assert_eq!(status.artifact(), original.artifact());
    assert_eq!(status.delivery_plan(), original.delivery_plan());
    let replay = engine.prepare_push(push).await.expect("replay");
    assert!(replay.is_replay());
    assert_eq!(replay.operation(), original.operation());
    assert_eq!(replay.artifact(), original.artifact());
    assert_eq!(replay.delivery_plan(), original.delivery_plan());
    assert_eq!(signer.calls.load(Ordering::Relaxed), 0);
}

#[test]
fn signer_rejection_challenge_and_timeout_fail_before_enqueue() {
    for kind in [
        SigningErrorKind::SignerRejected,
        SigningErrorKind::SignerCapabilityMissing,
        SigningErrorKind::SignerTimeout,
        SigningErrorKind::SignerOutputInvalid,
    ] {
        let signer = Arc::new(MockSigner::new(SignBehavior::Error(kind)));
        let (engine, storage) = setup_engine(signer);
        assert_eq!(
            block_on(engine.sign_and_enqueue(request(2, "wss://relay.example"))),
            Err(Error::SignerFailed)
        );
        assert!(
            block_on(Outbox::item(
                &*storage,
                radroots_storage::outbox::OutboxItemId::new([2; 16]).expect("item id"),
            ))
            .expect("outbox lookup")
            .is_none()
        );
    }

    let late = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_212_000,
    }));
    let (engine, _) = setup_engine(late);
    assert_eq!(
        block_on(engine.sign_and_enqueue(request(3, "wss://relay.example"))),
        Err(Error::SignerDeadlineExceeded)
    );
}

#[test]
fn idempotency_conflict_and_cancellation_before_commit_fail_closed() {
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let (engine, _) = setup_engine(signer.clone());
    block_on(engine.sign_and_enqueue(request(4, "wss://relay.example"))).expect("enqueue");
    assert_eq!(
        block_on(engine.sign_and_enqueue(request(4, "wss://other.example"))),
        Err(Error::StorageConflict)
    );
    assert_eq!(signer.calls.load(Ordering::Relaxed), 1);

    let pending = Arc::new(MockSigner::new(SignBehavior::Pending));
    let (engine, storage) = setup_engine(pending);
    let mut future = Box::pin(engine.sign_and_enqueue(request(5, "wss://relay.example"))).fuse();
    let mut context = std::task::Context::from_waker(noop_waker_ref());
    assert!(future.poll_unpin(&mut context).is_pending());
    drop(future);
    let record = block_on(Journal::operation(
        &*storage,
        OperationInstanceId::new([5; 16]).expect("instance id"),
    ))
    .expect("journal lookup")
    .expect("prepared record");
    assert_eq!(record.state().stage(), JournalStage::Prepared);
    assert!(
        block_on(Outbox::item(
            &*storage,
            radroots_storage::outbox::OutboxItemId::new([5; 16]).expect("item id"),
        ))
        .expect("outbox lookup")
        .is_none()
    );
}

#[test]
fn delivery_evaluates_any_all_quorum_required_and_partial_outcomes() {
    let sink = Arc::new(ScriptedSink::new([
        DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::accepted(),
            DeliveryOutcome::rejected(),
        ]),
        DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::accepted(),
            DeliveryOutcome::delivered(),
        ]),
        DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::delivered(),
            DeliveryOutcome::accepted(),
            DeliveryOutcome::rejected(),
        ]),
        DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::rejected(),
            DeliveryOutcome::accepted(),
        ]),
        DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::accepted(),
            DeliveryOutcome::unavailable(),
        ]),
        DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::rejected(),
            DeliveryOutcome::unavailable(),
        ]),
        DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::rejected(),
            DeliveryOutcome::unavailable(),
        ]),
        DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::unavailable(),
            DeliveryOutcome::rejected(),
        ]),
    ]));
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let ((engine, _), _) = setup_engine_with_sink(signer, sink.clone());
    let two = ["wss://one.example", "wss://two.example"];
    let three = [
        "wss://one.example",
        "wss://two.example",
        "wss://three.example",
    ];
    let required = Target::new(TransportId::NOSTR, two[1])
        .expect("required target")
        .fingerprint()
        .clone();
    let plans = [
        request_with_policy(11, &two, SatisfactionClass::Accepted, TargetPolicy::any()),
        request_with_policy(12, &two, SatisfactionClass::Accepted, TargetPolicy::all()),
        request_with_policy(
            13,
            &three,
            SatisfactionClass::Accepted,
            TargetPolicy::quorum(2).expect("quorum"),
        ),
        request_with_policy(
            14,
            &two,
            SatisfactionClass::Accepted,
            TargetPolicy::required(vec![required]).expect("required policy"),
        ),
        request_with_policy(15, &two, SatisfactionClass::Accepted, TargetPolicy::all()),
        request_with_policy(16, &two, SatisfactionClass::Accepted, TargetPolicy::all()),
        request_with_policy(17, &two, SatisfactionClass::Accepted, TargetPolicy::any()),
        request_with_policy(
            18,
            &two,
            SatisfactionClass::Accepted,
            TargetPolicy::required(vec![
                Target::new(TransportId::NOSTR, two[1])
                    .expect("terminal required target")
                    .fingerprint()
                    .clone(),
            ])
            .expect("terminal required policy"),
        ),
    ];
    for plan in plans {
        block_on(engine.sign_and_enqueue(plan)).expect("enqueue delivery plan");
    }

    let delivered = block_on(engine.deliver_pending(delivery_run(31, 8))).expect("deliver batch");
    assert_eq!(delivered.succeeded(), 8);
    assert_eq!(delivered.failed(), 0);
    let records: Vec<_> = delivered
        .outcomes()
        .iter()
        .map(|outcome| outcome.as_ref().expect("durable outcome"))
        .collect();
    assert_eq!(records[0].stage(), OutboxStage::Satisfied);
    assert_eq!(records[1].stage(), OutboxStage::Satisfied);
    assert_eq!(records[2].stage(), OutboxStage::Satisfied);
    assert_eq!(records[3].stage(), OutboxStage::Satisfied);
    assert_eq!(records[4].stage(), OutboxStage::Retryable);
    assert_eq!(records[4].satisfaction(), SatisfactionResult::Pending);
    assert_eq!(records[5].stage(), OutboxStage::Exhausted);
    assert_eq!(records[6].stage(), OutboxStage::Retryable);
    assert_eq!(records[7].stage(), OutboxStage::Exhausted);
    for record in records {
        assert_eq!(record.evidence().len(), record.request().target_set().len());
        let expected: Vec<_> = record
            .request()
            .target_set()
            .targets()
            .iter()
            .map(|target| target.fingerprint())
            .collect();
        let actual: Vec<_> = record
            .evidence()
            .iter()
            .map(|evidence| evidence.target())
            .collect();
        assert_eq!(actual, expected);
    }
    assert_eq!(sink.requests.lock().expect("request log").len(), 8);
}

#[test]
fn transport_failure_is_durable_and_retry_preserves_the_exact_plan() {
    let sink = Arc::new(ScriptedSink::new([
        DeliveryBehavior::AdapterError,
        DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::accepted(),
            DeliveryOutcome::unavailable(),
        ]),
        DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::unavailable(),
            DeliveryOutcome::accepted(),
        ]),
    ]));
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let ((engine, _), _) = setup_engine_with_sink(signer, sink.clone());
    block_on(engine.sign_and_enqueue(request_with_policy(
        21,
        &["wss://one.example", "wss://two.example"],
        SatisfactionClass::Accepted,
        TargetPolicy::all(),
    )))
    .expect("enqueue retry plan");

    let first = block_on(engine.deliver_pending(delivery_run(41, 1))).expect("first delivery");
    let first = first.outcomes()[0].as_ref().expect("durable failure");
    assert_eq!(first.stage(), OutboxStage::Retryable);
    assert_eq!(first.evidence().len(), 2);
    assert!(first.evidence().iter().all(|evidence| {
        !evidence.was_attempted() && evidence.outcome() == &DeliveryOutcome::unavailable()
    }));

    let second = block_on(engine.deliver_pending(delivery_run(42, 1))).expect("partial retry");
    let second = second.outcomes()[0].as_ref().expect("durable retry");
    assert_eq!(second.stage(), OutboxStage::Retryable);
    assert_eq!(second.last_attempt().expect("attempt").get(), 2);

    let third = block_on(engine.deliver_pending(delivery_run(43, 1))).expect("completed retry");
    let third = third.outcomes()[0].as_ref().expect("durable retry");
    assert_eq!(third.stage(), OutboxStage::Satisfied);
    assert_eq!(third.last_attempt().expect("attempt").get(), 3);
    assert_eq!(third.evidence().len(), 6);
    assert_eq!(
        third
            .latest_target_evidence(third.request().target_set().targets()[0].fingerprint())
            .expect("latest first target evidence")
            .outcome(),
        &DeliveryOutcome::unavailable()
    );
    assert!(
        third.evidence()[2]
            .outcome()
            .satisfies(SatisfactionClass::Accepted)
    );
    let requests = sink.requests.lock().expect("request log");
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0], requests[1]);
    assert_eq!(requests[1], requests[2]);
}

#[test]
fn malformed_receipts_release_work_and_expired_plans_terminalize() {
    let malformed_sink = Arc::new(ScriptedSink::new([DeliveryBehavior::MismatchedRequest]));
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let ((engine, storage), _) = setup_engine_with_sink(signer, malformed_sink);
    let enqueued = block_on(engine.sign_and_enqueue(request(31, "wss://one.example")))
        .expect("enqueue malformed receipt plan");
    let malformed =
        block_on(engine.deliver_pending(delivery_run(51, 1))).expect("malformed delivery pass");
    assert_eq!(malformed.outcomes(), &[Err(Error::InvalidDeliveryRequest)]);
    let released = block_on(Outbox::item(&*storage, enqueued.outbox().item_id()))
        .expect("released lookup")
        .expect("released record");
    assert_eq!(released.stage(), OutboxStage::Pending);
    assert!(released.lease().is_none());
    assert!(released.evidence().is_empty());

    let expired_sink = Arc::new(ScriptedSink::new([]));
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let ((engine, _), clock) = setup_engine_with_sink(signer, expired_sink.clone());
    let enqueued = block_on(engine.sign_and_enqueue(request(32, "wss://one.example")))
        .expect("enqueue expiring plan");
    clock.0.store(
        enqueued.outbox().request().deadline_unix_ms() + 1,
        Ordering::Relaxed,
    );
    let expired =
        block_on(engine.deliver_pending(delivery_run(52, 1))).expect("expired delivery pass");
    let expired = expired.outcomes()[0]
        .as_ref()
        .expect("durable terminal state");
    assert_eq!(expired.stage(), OutboxStage::Exhausted);
    assert_eq!(expired.satisfaction(), SatisfactionResult::Exhausted);
    assert!(expired.evidence()[0].outcome().is_terminal());
    assert!(
        expired_sink
            .requests
            .lock()
            .expect("request log")
            .is_empty()
    );
}

#[test]
fn delivery_run_rejects_unbounded_claims() {
    let owner = LeaseOwner::parse("sync-delivery-test").expect("lease owner");
    let seed = SyncId::new([71; 16]).expect("lease seed");
    assert_eq!(
        DeliveryRunRequest::new(owner.clone(), seed, 0, 1),
        Err(Error::InvalidDeliveryRequest)
    );
    assert_eq!(
        DeliveryRunRequest::new(owner, seed, 1_000, 0),
        Err(Error::InvalidDeliveryRequest)
    );
    let owner = LeaseOwner::parse("sync-delivery-test").expect("lease owner");
    assert_eq!(
        DeliveryRunRequest::new(owner.clone(), seed, 86_400_001, 1),
        Err(Error::InvalidDeliveryRequest)
    );
    assert_eq!(
        DeliveryRunRequest::new(
            owner,
            seed,
            1_000,
            radroots_storage::outbox::OUTBOX_CLAIM_LIMIT_MAX + 1,
        ),
        Err(Error::InvalidDeliveryRequest)
    );
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let (engine, _) = setup_engine(signer);
    let over_engine_budget = DeliveryRunRequest::new(
        LeaseOwner::parse("sync-delivery-test").expect("lease owner"),
        seed,
        10_001,
        1,
    )
    .expect("globally bounded delivery run");
    assert_eq!(
        block_on(engine.deliver_pending(over_engine_budget)),
        Err(Error::InvalidDeliveryRequest)
    );

    let valid = request(72, "wss://one.example");
    let invalid_quorum = PushRequest::new(
        valid.operation_id(),
        valid.idempotency_key().clone(),
        valid.actor().clone(),
        valid.plan().clone(),
        valid.targets().clone(),
        SatisfactionPolicy::new(
            SatisfactionClass::Accepted,
            TargetPolicy::quorum(2).expect("quorum"),
        ),
        valid.delivery_deadline_unix_ms(),
        valid.cancellation(),
    );
    assert!(matches!(invalid_quorum, Err(Error::InvalidPushRequest)));
    let absent = Target::new(TransportId::NOSTR, "wss://absent.example")
        .expect("absent target")
        .fingerprint()
        .clone();
    let invalid_required = PushRequest::new(
        valid.operation_id(),
        valid.idempotency_key().clone(),
        valid.actor().clone(),
        valid.plan().clone(),
        valid.targets().clone(),
        SatisfactionPolicy::new(
            SatisfactionClass::Accepted,
            TargetPolicy::required(vec![absent]).expect("required"),
        ),
        valid.delivery_deadline_unix_ms(),
        valid.cancellation(),
    );
    assert!(matches!(invalid_required, Err(Error::InvalidPushRequest)));
}

#[test]
fn retry_decisions_are_passive_typed_and_deadline_aware() {
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let (engine, storage) = setup_engine(signer);
    let enqueued = block_on(engine.sign_and_enqueue(request(61, "wss://one.example")))
        .expect("enqueue retry decision plan");
    let now = enqueued.outbox().created_at_unix_ms() + 1;
    assert_eq!(
        engine.retry_decision(enqueued.outbox(), now),
        Ok(SyncRetryDecision::Ready)
    );
    let claimed = block_on(Outbox::claim(
        &*storage,
        ClaimOutboxItems::new(
            LeaseOwner::parse("retry-decision-test").expect("owner"),
            LeaseId::new([81; 16]).expect("lease seed"),
            now,
            now + 100,
            1,
        )
        .expect("claim request"),
    ))
    .expect("claim")
    .pop()
    .expect("claimed plan");
    assert_eq!(
        engine.retry_decision(claimed.record(), now + 1),
        Ok(SyncRetryDecision::InFlightUntil { unix_ms: now + 100 })
    );
    assert_eq!(
        engine.retry_decision(claimed.record(), now + 100),
        Ok(SyncRetryDecision::Ready)
    );
    let deferred = block_on(Outbox::release(
        &*storage,
        claimed.record().item_id(),
        claimed.lease().id(),
        claimed.record().revision(),
        now + 2,
        Some(now + 50),
    ))
    .expect("release with deferral");
    assert_eq!(
        engine.retry_decision(&deferred, now + 3),
        Ok(SyncRetryDecision::DeferredUntil { unix_ms: now + 50 })
    );
    assert_eq!(
        engine.retry_decision(&deferred, now + 50),
        Ok(SyncRetryDecision::Ready)
    );
    assert_eq!(
        engine.retry_decision(&deferred, deferred.request().deadline_unix_ms()),
        Ok(SyncRetryDecision::Expired)
    );
    assert_eq!(
        engine.retry_decision(&deferred, 0),
        Err(Error::ClockUnavailable)
    );

    let sink = Arc::new(ScriptedSink::new([
        DeliveryBehavior::Outcomes(vec![DeliveryOutcome::accepted()]),
        DeliveryBehavior::Outcomes(vec![DeliveryOutcome::rejected()]),
    ]));
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let ((engine, _), _) = setup_engine_with_sink(signer, sink);
    block_on(engine.sign_and_enqueue(request(62, "wss://one.example")))
        .expect("enqueue satisfied plan");
    block_on(engine.sign_and_enqueue(request(63, "wss://two.example")))
        .expect("enqueue exhausted plan");
    let delivered = block_on(engine.deliver_pending(delivery_run(82, 2))).expect("deliver plans");
    assert_eq!(
        engine.retry_decision(delivered.outcomes()[0].as_ref().expect("satisfied"), now),
        Ok(SyncRetryDecision::Satisfied)
    );
    assert_eq!(
        engine.retry_decision(delivered.outcomes()[1].as_ref().expect("exhausted"), now),
        Ok(SyncRetryDecision::Exhausted)
    );
}
