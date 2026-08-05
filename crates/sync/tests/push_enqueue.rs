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
    actor::ActorSource,
    capability::{CancellationSupport, SignerCapability, SignerKind},
    error::Kind as SigningErrorKind,
    recovery::ReplayCapability,
    request::CancellationPolicy,
    status::SignerAvailability,
};
use radroots_storage::{
    EventStore, Journal, Outbox, ProjectionStore,
    atomic::AtomicStorage,
    authored_atomic::AuthoredAtomicStorage,
    authored_delivery::AuthoredDeliveryState,
    event::{EventQuery, EventQueryBounds, SourceGeneration},
    journal::{IdempotencyKey, OperationInstanceId},
    memory::MemoryStorage,
    status::StorageStatusProvider,
};
use radroots_storage_sqlite::{OpenMode, OpenOptions, Paths, SqliteStorage};
use radroots_sync::{
    Engine, PushRequest,
    policy::{Clock, DeadlinePolicy, Error, IdSource, OperationKind, SyncId, SyncStorage},
};
use radroots_transport::{
    DeliveryReceipt, DeliveryRequest, Error as TransportError, EventSink, EventSource, FetchPage,
    FetchRequest, SinkFailure, SinkStatus, SourceStatus, Target, TargetSet, TransportId,
    outcome::{DeliveryOutcome, Retryability},
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::DeliveryTargetReceipt,
};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey};

const CONTENT: &str = "frozen-content";

struct MockSink;

struct FaultStorage {
    inner: Arc<MemoryStorage>,
    fault_kind: AtomicUsize,
    remaining: AtomicUsize,
    admit_error: AtomicUsize,
    prepared: Mutex<Option<radroots_storage::authored_atomic::AuthoredAtomicOutcome>>,
}

impl FaultStorage {
    fn new(generation: u8) -> Self {
        Self {
            inner: Arc::new(MemoryStorage::new(
                SourceGeneration::new([generation; 32]).expect("generation"),
            )),
            fault_kind: AtomicUsize::new(0),
            remaining: AtomicUsize::new(0),
            admit_error: AtomicUsize::new(0),
            prepared: Mutex::new(None),
        }
    }

    fn fault_next_prepared(&self) {
        self.fault_kind.store(1, Ordering::Relaxed);
        self.remaining.store(1, Ordering::Relaxed);
    }

    fn fault_nth_artifact(&self, nth: usize) {
        self.fault_kind.store(2, Ordering::Relaxed);
        self.remaining.store(nth, Ordering::Relaxed);
    }

    fn fault_nth_plan(&self, nth: usize) {
        self.fault_kind.store(3, Ordering::Relaxed);
        self.remaining.store(nth, Ordering::Relaxed);
    }

    fn fault_prepared_identity(&self) {
        self.fault_kind.store(4, Ordering::Relaxed);
        self.remaining.store(1, Ordering::Relaxed);
    }

    fn fail_admission_with(&self, value: usize) {
        self.admit_error.store(value, Ordering::Relaxed);
    }
}

impl EventStore for FaultStorage {
    fn status(
        &self,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::status::EventStoreStatus, radroots_storage::Error>,
    > {
        EventStore::status(self.inner.as_ref())
    }
    fn admit(
        &self,
        value: radroots_storage::event::EventAdmission,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::event::AdmissionReceipt, radroots_storage::Error>,
    > {
        match self.admit_error.load(Ordering::Relaxed) {
            1 => Box::pin(async { Err(radroots_storage::Error::EventConflict) }),
            2 => Box::pin(async { Err(radroots_storage::Error::BackendUnavailable) }),
            _ => EventStore::admit(self.inner.as_ref(), value),
        }
    }
    fn query_raw(
        &self,
        value: radroots_storage::event::EventQuery,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<
            radroots_storage::event::EventPage<radroots_storage::event::StoredRawEvent>,
            radroots_storage::Error,
        >,
    > {
        EventStore::query_raw(self.inner.as_ref(), value)
    }
    fn query_verified(
        &self,
        value: radroots_storage::event::EventQuery,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<
            radroots_storage::event::EventPage<radroots_storage::event::StoredVerifiedEvent>,
            radroots_storage::Error,
        >,
    > {
        EventStore::query_verified(self.inner.as_ref(), value)
    }
    fn query_visible(
        &self,
        value: radroots_storage::event::EventQuery,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<
            radroots_storage::event::EventPage<radroots_storage::event::StoredVisibleEvent>,
            radroots_storage::Error,
        >,
    > {
        EventStore::query_visible(self.inner.as_ref(), value)
    }
    fn query_provenance(
        &self,
        id: radroots_storage::event::EventId,
        bounds: radroots_storage::event::EventQueryBounds,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<
            radroots_storage::event::EventPage<radroots_storage::event::StoredEventProvenance>,
            radroots_storage::Error,
        >,
    > {
        EventStore::query_provenance(self.inner.as_ref(), id, bounds)
    }
}

impl Journal for FaultStorage {
    fn prepare(
        &self,
        value: radroots_storage::journal::PrepareOperation,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::journal::PrepareReceipt, radroots_storage::Error>,
    > {
        Journal::prepare(self.inner.as_ref(), value)
    }
    fn operation(
        &self,
        id: OperationInstanceId,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<Option<radroots_storage::journal::OperationRecord>, radroots_storage::Error>,
    > {
        Journal::operation(self.inner.as_ref(), id)
    }
    fn by_idempotency_key(
        &self,
        id: radroots_storage::journal::OperationId,
        key: IdempotencyKey,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<Option<radroots_storage::journal::OperationRecord>, radroots_storage::Error>,
    > {
        Journal::by_idempotency_key(self.inner.as_ref(), id, key)
    }
    fn transition(
        &self,
        value: radroots_storage::journal::JournalTransition,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::journal::OperationRecord, radroots_storage::Error>,
    > {
        Journal::transition(self.inner.as_ref(), value)
    }
    fn recoverable(
        &self,
        limit: u16,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<Vec<radroots_storage::journal::OperationRecord>, radroots_storage::Error>,
    > {
        Journal::recoverable(self.inner.as_ref(), limit)
    }
}

impl Outbox for FaultStorage {
    fn enqueue(
        &self,
        value: radroots_storage::outbox::EnqueueOutboxItem,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::outbox::EnqueueReceipt, radroots_storage::Error>,
    > {
        Outbox::enqueue(self.inner.as_ref(), value)
    }
    fn item(
        &self,
        id: radroots_storage::outbox::OutboxItemId,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<Option<radroots_storage::outbox::OutboxRecord>, radroots_storage::Error>,
    > {
        Outbox::item(self.inner.as_ref(), id)
    }
    fn claim(
        &self,
        value: radroots_storage::outbox::ClaimOutboxItems,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<Vec<radroots_storage::outbox::ClaimedOutboxItem>, radroots_storage::Error>,
    > {
        Outbox::claim(self.inner.as_ref(), value)
    }
    fn record_attempt(
        &self,
        value: radroots_storage::outbox::DeliveryAttemptEvidence,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::outbox::OutboxRecord, radroots_storage::Error>,
    > {
        Outbox::record_attempt(self.inner.as_ref(), value)
    }
    fn release(
        &self,
        id: radroots_storage::outbox::OutboxItemId,
        lease: radroots_storage::outbox::LeaseId,
        revision: radroots_storage::outbox::OutboxRevision,
        at: u64,
        retry: Option<u64>,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::outbox::OutboxRecord, radroots_storage::Error>,
    > {
        Outbox::release(self.inner.as_ref(), id, lease, revision, at, retry)
    }
    fn status(
        &self,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::outbox::OutboxStatus, radroots_storage::Error>,
    > {
        Outbox::status(self.inner.as_ref())
    }
}

impl ProjectionStore for FaultStorage {
    fn status(
        &self,
        id: radroots_storage::projection::ProjectionId,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<Option<radroots_storage::projection::ProjectionStatus>, radroots_storage::Error>,
    > {
        ProjectionStore::status(self.inner.as_ref(), id)
    }
    fn checkpoint(
        &self,
        value: radroots_storage::projection::ProjectionCheckpoint,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::projection::ProjectionStatus, radroots_storage::Error>,
    > {
        ProjectionStore::checkpoint(self.inner.as_ref(), value)
    }
    fn invalidate(
        &self,
        value: radroots_storage::projection::ProjectionInvalidation,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::projection::ProjectionStatus, radroots_storage::Error>,
    > {
        ProjectionStore::invalidate(self.inner.as_ref(), value)
    }
    fn invalidation(
        &self,
        id: radroots_storage::projection::ProjectionId,
        generation: radroots_storage::projection::ProjectionGeneration,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<
            Option<radroots_storage::projection::ProjectionInvalidation>,
            radroots_storage::Error,
        >,
    > {
        ProjectionStore::invalidation(self.inner.as_ref(), id, generation)
    }
    fn request_rebuild(
        &self,
        value: radroots_storage::projection::RebuildTicket,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::projection::RebuildTicket, radroots_storage::Error>,
    > {
        ProjectionStore::request_rebuild(self.inner.as_ref(), value)
    }
    fn rebuild(
        &self,
        id: radroots_storage::projection::RebuildTicketId,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<Option<radroots_storage::projection::RebuildTicket>, radroots_storage::Error>,
    > {
        ProjectionStore::rebuild(self.inner.as_ref(), id)
    }
    fn transition_rebuild(
        &self,
        value: radroots_storage::projection::RebuildTransition,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::projection::RebuildTicket, radroots_storage::Error>,
    > {
        ProjectionStore::transition_rebuild(self.inner.as_ref(), value)
    }
    fn event_index_manifest(
        &self,
        generation: radroots_storage::projection::ProjectionGeneration,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<Option<radroots_storage::projection::EventIndexManifest>, radroots_storage::Error>,
    > {
        ProjectionStore::event_index_manifest(self.inner.as_ref(), generation)
    }
    fn put_event_index_manifest(
        &self,
        value: radroots_storage::projection::EventIndexManifest,
    ) -> radroots_transport::BoxFuture<'_, Result<(), radroots_storage::Error>> {
        ProjectionStore::put_event_index_manifest(self.inner.as_ref(), value)
    }
    fn event_index_checkpoint(
        &self,
        generation: radroots_storage::projection::ProjectionGeneration,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<Option<radroots_storage::projection::EventIndexCheckpoint>, radroots_storage::Error>,
    > {
        ProjectionStore::event_index_checkpoint(self.inner.as_ref(), generation)
    }
    fn put_event_index_checkpoint(
        &self,
        value: radroots_storage::projection::EventIndexCheckpoint,
    ) -> radroots_transport::BoxFuture<'_, Result<(), radroots_storage::Error>> {
        ProjectionStore::put_event_index_checkpoint(self.inner.as_ref(), value)
    }
}

impl AtomicStorage for FaultStorage {
    fn commit(
        &self,
        value: radroots_storage::atomic::AtomicCommit,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::atomic::AtomicCommitReceipt, radroots_storage::Error>,
    > {
        AtomicStorage::commit(self.inner.as_ref(), value)
    }
    fn receipt(
        &self,
        id: radroots_storage::atomic::AtomicCommitId,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<Option<radroots_storage::atomic::AtomicCommitReceipt>, radroots_storage::Error>,
    > {
        AtomicStorage::receipt(self.inner.as_ref(), id)
    }
}

impl StorageStatusProvider for FaultStorage {
    fn storage_status(
        &self,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::status::StorageStatus, radroots_storage::Error>,
    > {
        StorageStatusProvider::storage_status(self.inner.as_ref())
    }
}

impl AuthoredAtomicStorage for FaultStorage {
    fn execute_authored(
        &self,
        command: radroots_storage::authored_atomic::AuthoredAtomicCommand,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::authored_atomic::AuthoredAtomicReceipt, radroots_storage::Error>,
    > {
        Box::pin(async move {
            let receipt =
                AuthoredAtomicStorage::execute_authored(self.inner.as_ref(), command).await?;
            if let radroots_storage::authored_atomic::AuthoredAtomicOutcome::Prepared { .. } =
                receipt.outcome()
            {
                let mut cache = self.prepared.lock().expect("prepared cache");
                if cache.is_none() {
                    *cache = Some(receipt.outcome().clone());
                }
            }
            let kind = match receipt.outcome() {
                radroots_storage::authored_atomic::AuthoredAtomicOutcome::Prepared { .. } => 1,
                radroots_storage::authored_atomic::AuthoredAtomicOutcome::Artifact(_) => 2,
                radroots_storage::authored_atomic::AuthoredAtomicOutcome::DeliveryPlan(_) => 3,
            };
            let fault = self.fault_kind.load(Ordering::Relaxed);
            if (fault != kind && !(fault == 4 && kind == 1))
                || self.remaining.fetch_sub(1, Ordering::Relaxed) != 1
            {
                return Ok(receipt);
            }
            let cached = self
                .prepared
                .lock()
                .expect("prepared cache")
                .clone()
                .expect("prepared outcome");
            let forged = match (fault, cached) {
                (4, cached) => cached,
                (
                    1,
                    radroots_storage::authored_atomic::AuthoredAtomicOutcome::Prepared {
                        artifacts,
                        ..
                    },
                ) => radroots_storage::authored_atomic::AuthoredAtomicOutcome::Artifact(
                    artifacts[0].clone(),
                ),
                (
                    2,
                    radroots_storage::authored_atomic::AuthoredAtomicOutcome::Prepared {
                        delivery_plans,
                        ..
                    },
                ) => radroots_storage::authored_atomic::AuthoredAtomicOutcome::DeliveryPlan(
                    delivery_plans[0].clone(),
                ),
                (
                    3,
                    radroots_storage::authored_atomic::AuthoredAtomicOutcome::Prepared {
                        artifacts,
                        ..
                    },
                ) => radroots_storage::authored_atomic::AuthoredAtomicOutcome::Artifact(
                    artifacts[0].clone(),
                ),
                _ => unreachable!(),
            };
            radroots_storage::authored_atomic::AuthoredAtomicReceipt::from_durable_parts(
                receipt.commit_id(),
                receipt.digest(),
                receipt.disposition(),
                receipt.committed_at_unix_ms(),
                forged,
            )
        })
    }
    fn authored_receipt(
        &self,
        id: radroots_storage::atomic::AtomicCommitId,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<
            Option<radroots_storage::authored_atomic::AuthoredAtomicReceipt>,
            radroots_storage::Error,
        >,
    > {
        AuthoredAtomicStorage::authored_receipt(self.inner.as_ref(), id)
    }
    fn authored_operation(
        &self,
        id: OperationInstanceId,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<Option<radroots_storage::authored::AuthoredOperation>, radroots_storage::Error>,
    > {
        AuthoredAtomicStorage::authored_operation(self.inner.as_ref(), id)
    }
    fn authored_artifact(
        &self,
        id: radroots_storage::authored::AuthoredArtifactId,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<Option<radroots_storage::authored::AuthoredArtifact>, radroots_storage::Error>,
    > {
        AuthoredAtomicStorage::authored_artifact(self.inner.as_ref(), id)
    }
    fn authored_delivery_plan(
        &self,
        id: radroots_storage::authored_delivery::AuthoredDeliveryPlanId,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<
            Option<radroots_storage::authored_delivery::AuthoredDeliveryPlan>,
            radroots_storage::Error,
        >,
    > {
        AuthoredAtomicStorage::authored_delivery_plan(self.inner.as_ref(), id)
    }
}

struct MockSource;

impl EventSource for MockSource {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SourceStatus, TransportError>> {
        Box::pin(async { unreachable!("missing-sink check does not inspect source") })
    }

    fn fetch(
        &self,
        _request: FetchRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<FetchPage, TransportError>> {
        Box::pin(async { unreachable!("missing-sink check does not fetch") })
    }
}

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
    MismatchedRequest,
    Failure(Retryability),
    MismatchedFailure,
    Pending,
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
                DeliveryBehavior::Failure(retryability) => Err(SinkFailure::for_request(
                    &request,
                    "scripted_failure",
                    retryability,
                    None,
                    Some("scripted failure".to_owned()),
                    Vec::new(),
                )
                .expect("valid scripted failure")),
                DeliveryBehavior::MismatchedFailure => {
                    let mismatched = DeliveryRequest::new(
                        "mismatched-failure",
                        request.payload().clone(),
                        request.target_set().clone(),
                        request.satisfaction().clone(),
                        request.deadline_unix_ms(),
                    )
                    .expect("mismatched request");
                    Err(SinkFailure::for_request(
                        &mismatched,
                        "mismatched_failure",
                        Retryability::Terminal,
                        None,
                        None,
                        Vec::new(),
                    )
                    .expect("mismatched failure"))
                }
                DeliveryBehavior::Pending => std::future::pending().await,
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
    Uncertain(SigningErrorKind),
    Pending,
}

struct MockSigner {
    behavior: SignBehavior,
    replay: ReplayCapability,
    calls: AtomicUsize,
}

impl MockSigner {
    fn new(behavior: SignBehavior) -> Self {
        Self {
            behavior,
            replay: ReplayCapability::LocalReplaySafe,
            calls: AtomicUsize::new(0),
        }
    }

    fn with_replay(behavior: SignBehavior, replay: ReplayCapability) -> Self {
        Self {
            behavior,
            replay,
            calls: AtomicUsize::new(0),
        }
    }
}

impl Signer for MockSigner {
    fn status(
        &self,
    ) -> radroots_signing::signer::BoxFuture<'_, Result<SignerStatus, SigningError>> {
        let replay = self.replay;
        Box::pin(async move {
            Ok(SignerStatus::new(
                SignerAvailability::Ready,
                vec![SignerCapability::new(
                    if replay == ReplayCapability::LocalReplaySafe {
                        SignerKind::Local
                    } else {
                        SignerKind::Remote
                    },
                    replay,
                    CancellationSupport::BeforePublication,
                    false,
                    false,
                )],
                None,
            ))
        })
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
                SignBehavior::Uncertain(kind) => {
                    Err(SigningError::new(kind).with_possible_remote_effect())
                }
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

fn fault_engine(
    storage: Arc<FaultStorage>,
    signer: Arc<MockSigner>,
    sink: Arc<dyn EventSink>,
) -> Engine {
    let capability: Arc<dyn SyncStorage> = storage;
    Engine::builder(
        capability,
        Arc::new(TestClock(AtomicU64::new(1_800_000_200_000))),
        Arc::new(TestIds(AtomicU64::new(10))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).unwrap(),
    )
    .sink(sink)
    .signer(signer)
    .build()
    .unwrap()
}

fn execute_to_admitted(engine: &Engine, push: &PushRequest) {
    block_on(engine.sign_prepared(push.clone())).expect("sign prepared push");
    block_on(engine.admit_signed(push.operation_id())).expect("admit signed push");
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

#[test]
fn authored_storage_outcome_contracts_fail_closed_at_each_orchestration_phase() {
    let storage = Arc::new(FaultStorage::new(84));
    storage.fault_next_prepared();
    let engine = fault_engine(
        storage,
        Arc::new(MockSigner::new(SignBehavior::Pending)),
        Arc::new(MockSink),
    );
    assert_eq!(
        block_on(engine.prepare_push(request(84, "wss://fault.example"))),
        Err(Error::StorageFailed)
    );

    let storage = Arc::new(FaultStorage::new(85));
    let engine = fault_engine(
        storage.clone(),
        Arc::new(MockSigner::new(SignBehavior::Pending)),
        Arc::new(MockSink),
    );
    block_on(engine.prepare_push(request(85, "wss://fault.example"))).unwrap();
    storage.fault_prepared_identity();
    assert_eq!(
        block_on(engine.prepare_push(request(86, "wss://fault.example"))),
        Err(Error::StorageFailed)
    );

    let storage = Arc::new(FaultStorage::new(87));
    let engine = fault_engine(
        storage.clone(),
        Arc::new(MockSigner::new(SignBehavior::Success {
            completed_at_unix_ms: 1_800_000_200_500,
        })),
        Arc::new(MockSink),
    );
    let push = request(87, "wss://fault.example");
    block_on(engine.prepare_push(push.clone())).unwrap();
    storage.fault_nth_artifact(1);
    assert_eq!(
        block_on(engine.sign_prepared(push)),
        Err(Error::StorageFailed)
    );

    let storage = Arc::new(FaultStorage::new(88));
    let engine = fault_engine(
        storage.clone(),
        Arc::new(MockSigner::new(SignBehavior::Success {
            completed_at_unix_ms: 1_800_000_200_500,
        })),
        Arc::new(MockSink),
    );
    let push = request(88, "wss://fault.example");
    block_on(engine.prepare_push(push.clone())).unwrap();
    storage.fault_nth_artifact(2);
    assert_eq!(
        block_on(engine.sign_prepared(push)),
        Err(Error::StorageFailed)
    );

    let storage = Arc::new(FaultStorage::new(89));
    let engine = fault_engine(
        storage.clone(),
        Arc::new(MockSigner::new(SignBehavior::Success {
            completed_at_unix_ms: 1_800_000_200_500,
        })),
        Arc::new(MockSink),
    );
    let push = request(89, "wss://fault.example");
    block_on(engine.sign_prepared(push.clone())).unwrap();
    storage.fault_nth_artifact(2);
    assert_eq!(
        block_on(engine.admit_signed(push.operation_id())),
        Err(Error::StorageFailed)
    );

    let storage = Arc::new(FaultStorage::new(90));
    let engine = fault_engine(
        storage.clone(),
        Arc::new(MockSigner::new(SignBehavior::Error(
            SigningErrorKind::SignerRejected,
        ))),
        Arc::new(MockSink),
    );
    let push = request(90, "wss://fault.example");
    block_on(engine.prepare_push(push.clone())).unwrap();
    storage.fault_nth_artifact(2);
    assert_eq!(
        block_on(engine.sign_prepared(push)),
        Err(Error::StorageFailed)
    );

    for (byte, nth) in [(91, 1), (92, 2)] {
        let storage = Arc::new(FaultStorage::new(byte));
        let sink = Arc::new(ScriptedSink::new([DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::accepted(),
        ])]));
        let engine = fault_engine(
            storage.clone(),
            Arc::new(MockSigner::new(SignBehavior::Success {
                completed_at_unix_ms: 1_800_000_200_500,
            })),
            sink,
        );
        let push = request(byte, "wss://fault.example");
        execute_to_admitted(&engine, &push);
        storage.fault_nth_plan(nth);
        assert_eq!(
            block_on(engine.deliver_push(push.operation_id())),
            Err(Error::StorageFailed)
        );
    }
}

#[test]
fn signing_claim_recovery_respects_exact_and_non_replayable_capabilities() {
    let exact_storage = Arc::new(MemoryStorage::new(
        SourceGeneration::new([8; 32]).expect("generation"),
    ));
    let exact_pending = Arc::new(MockSigner::with_replay(
        SignBehavior::Pending,
        ReplayCapability::ExactReplayByRequestId,
    ));
    let exact_engine = Engine::builder(
        exact_storage.clone(),
        Arc::new(TestClock(AtomicU64::new(1_800_000_200_000))),
        Arc::new(TestIds(AtomicU64::new(100))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
    )
    .sink(Arc::new(MockSink))
    .signer(exact_pending.clone())
    .build()
    .expect("exact engine");
    let exact_push = request(8, "wss://relay.example");
    let mut exact_future = Box::pin(exact_engine.sign_prepared(exact_push.clone())).fuse();
    let mut context = std::task::Context::from_waker(noop_waker_ref());
    assert!(exact_future.poll_unpin(&mut context).is_pending());
    drop(exact_future);
    let claimed = block_on(exact_engine.push_status(exact_push.operation_id()))
        .expect("status")
        .expect("claimed status");
    assert!(claimed.artifact().signing_claim().is_some());

    let exact_success = Arc::new(MockSigner::with_replay(
        SignBehavior::Success {
            completed_at_unix_ms: 1_800_000_220_000,
        },
        ReplayCapability::ExactReplayByRequestId,
    ));
    let exact_recovery = Engine::builder(
        exact_storage,
        Arc::new(TestClock(AtomicU64::new(1_800_000_211_000))),
        Arc::new(TestIds(AtomicU64::new(110))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
    )
    .sink(Arc::new(MockSink))
    .signer(exact_success.clone())
    .build()
    .expect("recovery engine");
    let recovered =
        block_on(exact_recovery.sign_prepared(exact_push.clone())).expect("exact replay recovery");
    assert_eq!(
        recovered.artifact().signing_state(),
        radroots_storage::authored::SigningState::Signed
    );
    assert_eq!(exact_success.calls.load(Ordering::Relaxed), 1);
    let replay = block_on(exact_recovery.sign_prepared(exact_push)).expect("signed replay");
    assert!(replay.is_replay());
    assert_eq!(exact_success.calls.load(Ordering::Relaxed), 1);

    let unsafe_storage = Arc::new(MemoryStorage::new(
        SourceGeneration::new([9; 32]).expect("generation"),
    ));
    let unsafe_pending = Arc::new(MockSigner::with_replay(
        SignBehavior::Pending,
        ReplayCapability::NonReplayable,
    ));
    let unsafe_engine = Engine::builder(
        unsafe_storage.clone(),
        Arc::new(TestClock(AtomicU64::new(1_800_000_200_000))),
        Arc::new(TestIds(AtomicU64::new(120))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
    )
    .sink(Arc::new(MockSink))
    .signer(unsafe_pending)
    .build()
    .expect("unsafe engine");
    let unsafe_push = request(9, "wss://relay.example");
    let mut unsafe_future = Box::pin(unsafe_engine.sign_prepared(unsafe_push.clone())).fuse();
    let mut context = std::task::Context::from_waker(noop_waker_ref());
    assert!(unsafe_future.poll_unpin(&mut context).is_pending());
    drop(unsafe_future);

    let unsafe_success = Arc::new(MockSigner::with_replay(
        SignBehavior::Success {
            completed_at_unix_ms: 1_800_000_220_000,
        },
        ReplayCapability::NonReplayable,
    ));
    let unsafe_recovery = Engine::builder(
        unsafe_storage,
        Arc::new(TestClock(AtomicU64::new(1_800_000_211_000))),
        Arc::new(TestIds(AtomicU64::new(130))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
    )
    .sink(Arc::new(MockSink))
    .signer(unsafe_success.clone())
    .build()
    .expect("unsafe recovery engine");
    assert_eq!(
        block_on(unsafe_recovery.sign_prepared(unsafe_push.clone())),
        Err(Error::SigningIndeterminate)
    );
    assert_eq!(unsafe_success.calls.load(Ordering::Relaxed), 0);
    let unsafe_status = block_on(unsafe_recovery.push_status(unsafe_push.operation_id()))
        .expect("unsafe status")
        .expect("unsafe operation");
    assert_eq!(
        unsafe_status.artifact().signing_state(),
        radroots_storage::authored::SigningState::Indeterminate
    );
}

#[test]
fn signing_failures_persist_retry_indeterminate_terminal_and_cancelled_states() {
    let exact = Arc::new(MockSigner::with_replay(
        SignBehavior::Uncertain(SigningErrorKind::SignerTimeout),
        ReplayCapability::ExactReplayByRequestId,
    ));
    let (engine, storage) = setup_engine(exact);
    let retryable = request(10, "wss://relay.example");
    assert_eq!(
        block_on(engine.sign_prepared(retryable.clone())),
        Err(Error::SignerFailed)
    );
    let retryable_status = block_on(engine.push_status(retryable.operation_id()))
        .expect("retryable status")
        .expect("retryable operation");
    assert_eq!(
        retryable_status.artifact().signing_state(),
        radroots_storage::authored::SigningState::Retryable
    );
    let retry_at = retryable_status
        .artifact()
        .signing_retry()
        .expect("retry schedule")
        .not_before_unix_ms();
    let success = Arc::new(MockSigner::with_replay(
        SignBehavior::Success {
            completed_at_unix_ms: retry_at + 2,
        },
        ReplayCapability::ExactReplayByRequestId,
    ));
    let recovery = Engine::builder(
        storage,
        Arc::new(TestClock(AtomicU64::new(retry_at + 1))),
        Arc::new(TestIds(AtomicU64::new(140))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
    )
    .sink(Arc::new(MockSink))
    .signer(success.clone())
    .build()
    .expect("recovery engine");
    assert_eq!(
        block_on(recovery.sign_prepared(retryable))
            .expect("retry exact request")
            .artifact()
            .signing_state(),
        radroots_storage::authored::SigningState::Signed
    );
    assert_eq!(success.calls.load(Ordering::Relaxed), 1);

    let non_replayable = Arc::new(MockSigner::with_replay(
        SignBehavior::Uncertain(SigningErrorKind::SignerTimeout),
        ReplayCapability::NonReplayable,
    ));
    let (engine, _) = setup_engine(non_replayable);
    let uncertain = request(11, "wss://relay.example");
    assert_eq!(
        block_on(engine.sign_prepared(uncertain.clone())),
        Err(Error::SigningIndeterminate)
    );
    assert_eq!(
        block_on(engine.push_status(uncertain.operation_id()))
            .expect("indeterminate status")
            .expect("indeterminate operation")
            .artifact()
            .signing_state(),
        radroots_storage::authored::SigningState::Indeterminate
    );
    assert_eq!(
        block_on(engine.sign_prepared(uncertain)),
        Err(Error::SigningIndeterminate)
    );

    let cancelled = Arc::new(MockSigner::new(SignBehavior::Error(
        SigningErrorKind::SignerCancelled,
    )));
    let (engine, _) = setup_engine(cancelled);
    let cancelled_push = request(12, "wss://relay.example");
    assert_eq!(
        block_on(engine.sign_prepared(cancelled_push.clone())),
        Err(Error::SigningCancelled)
    );
    assert_eq!(
        block_on(engine.push_status(cancelled_push.operation_id()))
            .expect("cancelled status")
            .expect("cancelled operation")
            .artifact()
            .signing_state(),
        radroots_storage::authored::SigningState::Cancelled
    );
    assert_eq!(
        block_on(engine.sign_prepared(cancelled_push)),
        Err(Error::SignerFailed)
    );

    let terminal = Arc::new(MockSigner::new(SignBehavior::Error(
        SigningErrorKind::SignerRejected,
    )));
    let (engine, _) = setup_engine(terminal);
    let terminal_push = request(13, "wss://relay.example");
    assert_eq!(
        block_on(engine.sign_prepared(terminal_push.clone())),
        Err(Error::SignerFailed)
    );
    assert_eq!(
        block_on(engine.push_status(terminal_push.operation_id()))
            .expect("terminal status")
            .expect("terminal operation")
            .artifact()
            .signing_state(),
        radroots_storage::authored::SigningState::FailedTerminal
    );
    assert_eq!(
        block_on(engine.sign_prepared(terminal_push)),
        Err(Error::SignerFailed)
    );

    let deadline = Arc::new(MockSigner::new(SignBehavior::Error(
        SigningErrorKind::DeadlineExceeded,
    )));
    let (engine, _) = setup_engine(deadline);
    let deadline_push = request(14, "wss://relay.example");
    assert_eq!(
        block_on(engine.sign_prepared(deadline_push)),
        Err(Error::SignerDeadlineExceeded)
    );
}

#[tokio::test]
async fn sqlite_signing_and_admission_recover_across_every_reopen_boundary() {
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

    {
        let store = Arc::new(
            SqliteStorage::open(OpenOptions::new(paths.clone(), OpenMode::ReadWriteExisting))
                .await
                .expect("reopen for signing"),
        );
        let capability: Arc<dyn SyncStorage> = store;
        let signer = Arc::new(MockSigner::new(SignBehavior::Success {
            completed_at_unix_ms: 1_800_000_210_500,
        }));
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
        let replay = engine
            .prepare_push(push.clone())
            .await
            .expect("prepare replay");
        assert!(replay.is_replay());
        assert_eq!(signer.calls.load(Ordering::Relaxed), 0);
        let signed = engine
            .sign_prepared(push.clone())
            .await
            .expect("sign after reopen");
        assert_eq!(
            signed.artifact().signing_state(),
            radroots_storage::authored::SigningState::Signed
        );
        assert_eq!(
            signed.artifact().admission_state(),
            radroots_storage::authored::AdmissionState::Pending
        );
        assert!(signed.artifact().signed().is_some());
        assert_eq!(signer.calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            engine.deliver_push(push.operation_id()).await,
            Err(Error::AdmissionFailed)
        );
    }

    {
        let store = Arc::new(
            SqliteStorage::open(OpenOptions::new(paths.clone(), OpenMode::ReadWriteExisting))
                .await
                .expect("reopen for admission"),
        );
        let capability: Arc<dyn SyncStorage> = store.clone();
        let signer = Arc::new(MockSigner::new(SignBehavior::Pending));
        let engine = Engine::builder(
            capability,
            Arc::new(TestClock(AtomicU64::new(1_800_000_211_000))),
            Arc::new(TestIds(AtomicU64::new(100))),
            DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
        )
        .sink(Arc::new(MockSink))
        .signer(signer.clone())
        .build()
        .expect("engine");
        let admitted = engine
            .admit_signed(push.operation_id())
            .await
            .expect("admit after reopen");
        assert!(!admitted.is_replay());
        assert!(admitted.artifact().admission_state().is_admitted());
        let replay = engine
            .admit_signed(push.operation_id())
            .await
            .expect("admission replay");
        assert!(replay.is_replay());
        assert_eq!(signer.calls.load(Ordering::Relaxed), 0);
        let visible = store
            .query_visible(EventQuery::all(
                EventQueryBounds::first(10).expect("bounds"),
            ))
            .await
            .expect("visible events");
        assert_eq!(visible.items().len(), 1);
    }

    let store = Arc::new(
        SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadOnly))
            .await
            .expect("final read-only reopen"),
    );
    let engine = Engine::builder(
        store,
        Arc::new(TestClock(AtomicU64::new(1_800_000_212_000))),
        Arc::new(TestIds(AtomicU64::new(110))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
    )
    .sink(Arc::new(MockSink))
    .build()
    .expect("read-only engine");
    let final_status = engine
        .push_status(push.operation_id())
        .await
        .expect("final status")
        .expect("durable operation");
    assert_eq!(
        final_status.artifact().signing_state(),
        radroots_storage::authored::SigningState::Signed
    );
    assert!(final_status.artifact().admission_state().is_admitted());
    assert!(final_status.delivery_plan().request().is_some());
}

#[test]
fn authored_delivery_persists_retry_evidence_and_complete_settlement() {
    let sink = Arc::new(ScriptedSink::new([
        DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::accepted(),
            DeliveryOutcome::unavailable(),
        ]),
        DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::accepted(),
            DeliveryOutcome::accepted(),
        ]),
    ]));
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let ((engine, _), clock) = setup_engine_with_sink(signer, sink.clone());
    let push = request_with_policy(
        73,
        &["wss://one.example", "wss://two.example"],
        SatisfactionClass::Accepted,
        TargetPolicy::all(),
    );
    execute_to_admitted(&engine, &push);
    let admitted = block_on(engine.push_status(push.operation_id()))
        .expect("admitted status")
        .expect("admitted operation");
    assert_eq!(
        engine.retry_decision(admitted.delivery_plan(), 0),
        Err(Error::ClockUnavailable)
    );
    assert_eq!(
        engine.retry_decision(admitted.delivery_plan(), 1_800_000_200_100),
        Ok(SyncRetryDecision::Ready)
    );
    assert_eq!(
        engine.retry_decision(admitted.delivery_plan(), 1_800_000_300_000),
        Ok(SyncRetryDecision::Expired)
    );

    let first = block_on(engine.deliver_push(push.operation_id())).expect("first attempt");
    assert!(!first.is_replay());
    assert_eq!(first.plan().state(), AuthoredDeliveryState::Retryable);
    assert_eq!(first.plan().attempt_count(), 1);
    let retry_at = first
        .plan()
        .retry()
        .expect("durable retry")
        .not_before_unix_ms();
    assert_eq!(
        engine.retry_decision(first.plan(), retry_at - 1),
        Ok(SyncRetryDecision::DeferredUntil { unix_ms: retry_at })
    );
    assert_eq!(
        engine.retry_decision(first.plan(), retry_at),
        Ok(SyncRetryDecision::Ready)
    );
    assert_eq!(
        block_on(engine.deliver_push(push.operation_id())),
        Err(Error::DeliveryDeferred)
    );
    let pending = block_on(engine.push_status(push.operation_id()))
        .expect("pending status")
        .expect("pending operation");
    assert_eq!(pending.settlement().artifacts(), 1);
    assert_eq!(pending.settlement().signed(), 1);
    assert_eq!(pending.settlement().admitted(), 1);
    assert_eq!(pending.settlement().delivery_plans(), 1);
    assert_eq!(pending.settlement().delivery_retryable(), 1);
    assert!(!pending.settlement().is_settled());

    clock.0.store(retry_at, Ordering::Relaxed);
    let second = block_on(engine.deliver_push(push.operation_id())).expect("retry attempt");
    assert_eq!(second.plan().state(), AuthoredDeliveryState::Satisfied);
    assert_eq!(
        engine.retry_decision(second.plan(), retry_at),
        Ok(SyncRetryDecision::Satisfied)
    );
    assert_eq!(second.plan().attempt_count(), 2);
    assert_eq!(second.plan().attempts().len(), 2);
    let replay = block_on(engine.deliver_push(push.operation_id())).expect("terminal replay");
    assert!(replay.is_replay());
    assert_eq!(replay.plan(), second.plan());
    let settled = block_on(engine.push_status(push.operation_id()))
        .expect("settled status")
        .expect("settled operation");
    assert_eq!(settled.settlement().delivery_satisfied(), 1);
    assert!(settled.settlement().is_settled());
    assert!(!settled.settlement().has_failures());
    assert!(settled.settlement().is_successful());
    let requests = sink.requests.lock().expect("request log");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], requests[1]);
}

#[test]
fn invalid_authored_delivery_receipt_terminalizes_without_hot_loop() {
    let sink = Arc::new(ScriptedSink::new([DeliveryBehavior::MismatchedRequest]));
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let ((engine, _), _) = setup_engine_with_sink(signer, sink.clone());
    let push = request(74, "wss://one.example");
    execute_to_admitted(&engine, &push);

    let failed = block_on(engine.deliver_push(push.operation_id())).expect("durable failure");
    assert_eq!(failed.plan().state(), AuthoredDeliveryState::FailedTerminal);
    assert_eq!(
        engine.retry_decision(failed.plan(), 1_800_000_200_100),
        Ok(SyncRetryDecision::Exhausted)
    );
    assert_eq!(failed.plan().attempt_count(), 1);
    assert_eq!(
        failed.plan().last_failure().expect("failure").code(),
        "invalid_transport_contract"
    );
    let replay = block_on(engine.deliver_push(push.operation_id())).expect("failure replay");
    assert!(replay.is_replay());
    assert_eq!(replay.plan(), failed.plan());
    assert_eq!(sink.requests.lock().expect("request log").len(), 1);
    let status = block_on(engine.push_status(push.operation_id()))
        .expect("failed status")
        .expect("failed operation");
    assert_eq!(status.settlement().delivery_failed_terminal(), 1);
    assert!(status.settlement().is_settled());
    assert!(status.settlement().has_failures());
    assert!(!status.settlement().is_successful());
}

#[test]
fn sink_failures_deadlines_and_missing_capabilities_are_durable_and_bounded() {
    for (byte, behavior, expected) in [
        (
            77,
            DeliveryBehavior::Failure(Retryability::Retryable),
            AuthoredDeliveryState::Retryable,
        ),
        (
            78,
            DeliveryBehavior::Failure(Retryability::Terminal),
            AuthoredDeliveryState::FailedTerminal,
        ),
        (
            79,
            DeliveryBehavior::MismatchedFailure,
            AuthoredDeliveryState::FailedTerminal,
        ),
    ] {
        let sink = Arc::new(ScriptedSink::new([behavior]));
        let signer = Arc::new(MockSigner::new(SignBehavior::Success {
            completed_at_unix_ms: 1_800_000_200_500,
        }));
        let ((engine, _), _) = setup_engine_with_sink(signer, sink);
        let push = request(byte, "wss://failure.example");
        execute_to_admitted(&engine, &push);
        let delivered =
            block_on(engine.deliver_push(push.operation_id())).expect("durable failure");
        assert_eq!(delivered.plan().state(), expected);
    }

    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let ((engine, _), clock) =
        setup_engine_with_sink(signer, Arc::new(ScriptedSink::new(std::iter::empty())));
    let push = request(80, "wss://deadline.example");
    execute_to_admitted(&engine, &push);
    clock
        .0
        .store(push.delivery_deadline_unix_ms(), Ordering::Relaxed);
    let expired = block_on(engine.deliver_push(push.operation_id())).expect("deadline evidence");
    assert_eq!(
        expired.plan().state(),
        AuthoredDeliveryState::FailedTerminal
    );
    assert_eq!(
        expired.plan().last_failure().expect("failure").code(),
        "delivery_deadline_exceeded"
    );

    let storage = Arc::new(MemoryStorage::new(
        SourceGeneration::new([81; 32]).expect("generation"),
    ));
    let capability: Arc<dyn SyncStorage> = storage;
    let no_signer = Engine::builder(
        capability,
        Arc::new(TestClock(AtomicU64::new(1_800_000_200_000))),
        Arc::new(TestIds(AtomicU64::new(10))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).unwrap(),
    )
    .sink(Arc::new(MockSink))
    .build()
    .unwrap();
    assert_eq!(
        block_on(no_signer.sign_prepared(request(81, "wss://missing.example"))),
        Err(Error::MissingSigner)
    );

    let storage = Arc::new(MemoryStorage::new(
        SourceGeneration::new([82; 32]).expect("generation"),
    ));
    let capability: Arc<dyn SyncStorage> = storage;
    let no_sink = Engine::builder(
        capability,
        Arc::new(TestClock(AtomicU64::new(1_800_000_200_000))),
        Arc::new(TestIds(AtomicU64::new(10))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).unwrap(),
    )
    .source(Arc::new(MockSource))
    .build()
    .unwrap();
    assert_eq!(
        block_on(no_sink.deliver_push(SyncId::new([82; 16]).unwrap())),
        Err(Error::MissingSink)
    );

    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let (engine, _) = setup_engine(signer);
    let unsigned = request(83, "wss://unsigned.example");
    block_on(engine.prepare_push(unsigned.clone())).unwrap();
    assert_eq!(
        block_on(engine.admit_signed(unsigned.operation_id())),
        Err(Error::InvalidSignerOutput)
    );
    assert_eq!(
        block_on(engine.deliver_push(unsigned.operation_id())),
        Err(Error::InvalidSignerOutput)
    );
}

#[test]
fn active_authored_claims_and_admission_storage_failures_are_fenced() {
    let storage = Arc::new(FaultStorage::new(93));
    let engine = fault_engine(
        storage.clone(),
        Arc::new(MockSigner::new(SignBehavior::Success {
            completed_at_unix_ms: 1_800_000_200_500,
        })),
        Arc::new(MockSink),
    );
    let push = request(93, "wss://claim.example");
    block_on(engine.prepare_push(push.clone())).unwrap();
    let artifact = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap()
        .artifact()
        .clone();
    let claim = radroots_storage::authored::WorkClaim::new(
        [93; 16],
        "external-signer",
        std::num::NonZeroU64::MIN,
        1_800_000_200_000,
        1_800_000_210_000,
        artifact.revision(),
    )
    .unwrap();
    block_on(AuthoredAtomicStorage::execute_authored(
        storage.as_ref(),
        radroots_storage::authored_atomic::AuthoredAtomicCommand::Claim(
            radroots_storage::authored_atomic::ClaimAuthoredWork::new(
                radroots_storage::authored_atomic::ClaimAuthoredTarget::ArtifactSigning(
                    artifact.artifact_id(),
                ),
                claim,
            ),
        ),
    ))
    .unwrap();
    assert_eq!(
        block_on(engine.sign_prepared(push)),
        Err(Error::WorkClaimConflict)
    );

    let storage = Arc::new(FaultStorage::new(94));
    let engine = fault_engine(
        storage.clone(),
        Arc::new(MockSigner::new(SignBehavior::Success {
            completed_at_unix_ms: 1_800_000_200_500,
        })),
        Arc::new(MockSink),
    );
    let push = request(94, "wss://claim.example");
    block_on(engine.sign_prepared(push.clone())).unwrap();
    let artifact = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap()
        .artifact()
        .clone();
    let claim = radroots_storage::authored::WorkClaim::new(
        [94; 16],
        "external-admission",
        std::num::NonZeroU64::MIN,
        1_800_000_200_500,
        1_800_000_210_500,
        artifact.revision(),
    )
    .unwrap();
    block_on(AuthoredAtomicStorage::execute_authored(
        storage.as_ref(),
        radroots_storage::authored_atomic::AuthoredAtomicCommand::Claim(
            radroots_storage::authored_atomic::ClaimAuthoredWork::new(
                radroots_storage::authored_atomic::ClaimAuthoredTarget::ArtifactAdmission(
                    artifact.artifact_id(),
                ),
                claim,
            ),
        ),
    ))
    .unwrap();
    assert_eq!(
        block_on(engine.admit_signed(push.operation_id())),
        Err(Error::WorkClaimConflict)
    );

    for (byte, fault, expected) in [
        (95, 1, radroots_storage::authored::AdmissionState::Rejected),
        (96, 2, radroots_storage::authored::AdmissionState::Retryable),
    ] {
        let storage = Arc::new(FaultStorage::new(byte));
        let engine = fault_engine(
            storage.clone(),
            Arc::new(MockSigner::new(SignBehavior::Success {
                completed_at_unix_ms: 1_800_000_200_500,
            })),
            Arc::new(MockSink),
        );
        let push = request(byte, "wss://admission-error.example");
        block_on(engine.sign_prepared(push.clone())).unwrap();
        storage.fail_admission_with(fault);
        assert_eq!(
            block_on(engine.admit_signed(push.operation_id())),
            Err(Error::AdmissionFailed),
            "admission fault {fault}"
        );
        assert_eq!(
            block_on(engine.push_status(push.operation_id()))
                .unwrap()
                .unwrap()
                .artifact()
                .admission_state(),
            expected
        );
    }
}

#[test]
fn authored_delivery_claims_fence_concurrent_and_stale_workers() {
    let pending_sink = Arc::new(ScriptedSink::new([DeliveryBehavior::Pending]));
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let ((engine, storage), clock) = setup_engine_with_sink(signer, pending_sink.clone());
    let push = request(75, "wss://one.example");
    execute_to_admitted(&engine, &push);

    let mut pending = Box::pin(engine.deliver_push(push.operation_id())).fuse();
    let mut context = std::task::Context::from_waker(noop_waker_ref());
    assert!(pending.poll_unpin(&mut context).is_pending());
    let claimed = block_on(engine.push_status(push.operation_id()))
        .expect("claimed status")
        .expect("claimed operation");
    let expires_at = claimed
        .delivery_plan()
        .claim_evidence()
        .expect("delivery claim")
        .expires_at_unix_ms();
    assert_eq!(
        engine.retry_decision(claimed.delivery_plan(), expires_at - 1),
        Ok(SyncRetryDecision::InFlightUntil {
            unix_ms: expires_at,
        })
    );
    assert_eq!(
        engine.retry_decision(claimed.delivery_plan(), expires_at),
        Ok(SyncRetryDecision::Ready)
    );
    assert_eq!(
        block_on(engine.deliver_push(push.operation_id())),
        Err(Error::WorkClaimConflict)
    );
    drop(pending);

    clock.0.store(expires_at + 1, Ordering::Relaxed);
    let recovery_sink = Arc::new(ScriptedSink::new([DeliveryBehavior::Outcomes(vec![
        DeliveryOutcome::accepted(),
    ])]));
    let capability: Arc<dyn SyncStorage> = storage;
    let recovery = Engine::builder(
        capability,
        clock,
        Arc::new(TestIds(AtomicU64::new(220))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
    )
    .sink(recovery_sink.clone())
    .build()
    .expect("recovery engine");
    let recovered = block_on(recovery.deliver_push(push.operation_id())).expect("stale recovery");
    assert_eq!(recovered.plan().state(), AuthoredDeliveryState::Satisfied);
    assert_eq!(
        recovered.plan().claim_evidence(),
        None,
        "settlement clears the claim"
    );
    assert_eq!(recovery_sink.requests.lock().expect("request log").len(), 1);
}

#[tokio::test]
async fn sqlite_authored_delivery_retry_survives_reopen() {
    let directory = tempfile::tempdir().expect("database directory");
    let paths = Paths::from_directory(directory.path()).expect("paths");
    let push = request_with_policy(
        76,
        &["wss://one.example", "wss://two.example"],
        SatisfactionClass::Accepted,
        TargetPolicy::all(),
    );
    let retry_at = {
        let store = Arc::new(
            SqliteStorage::open(
                OpenOptions::new(paths.clone(), OpenMode::Create)
                    .with_source_generation(SourceGeneration::new([76; 32]).expect("generation"), 1)
                    .expect("source generation"),
            )
            .await
            .expect("open SQLite"),
        );
        let sink = Arc::new(ScriptedSink::new([DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::accepted(),
            DeliveryOutcome::unavailable(),
        ])]));
        let engine = Engine::builder(
            store,
            Arc::new(TestClock(AtomicU64::new(1_800_000_200_000))),
            Arc::new(TestIds(AtomicU64::new(230))),
            DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
        )
        .sink(sink)
        .signer(Arc::new(MockSigner::new(SignBehavior::Success {
            completed_at_unix_ms: 1_800_000_200_500,
        })))
        .build()
        .expect("engine");
        engine
            .sign_prepared(push.clone())
            .await
            .expect("sign prepared push");
        engine
            .admit_signed(push.operation_id())
            .await
            .expect("admit signed push");
        let first = engine
            .deliver_push(push.operation_id())
            .await
            .expect("first attempt");
        assert_eq!(first.plan().attempt_count(), 1);
        first
            .plan()
            .retry()
            .expect("retry schedule")
            .not_before_unix_ms()
    };

    let store = Arc::new(
        SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadWriteExisting))
            .await
            .expect("reopen SQLite"),
    );
    let sink = Arc::new(ScriptedSink::new([DeliveryBehavior::Outcomes(vec![
        DeliveryOutcome::accepted(),
        DeliveryOutcome::accepted(),
    ])]));
    let clock = Arc::new(TestClock(AtomicU64::new(retry_at - 1)));
    let engine = Engine::builder(
        store,
        clock.clone(),
        Arc::new(TestIds(AtomicU64::new(240))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
    )
    .sink(sink.clone())
    .build()
    .expect("reopen engine");
    let reopened = engine
        .push_status(push.operation_id())
        .await
        .expect("reopened status")
        .expect("reopened operation");
    assert_eq!(reopened.delivery_plan().attempt_count(), 1);
    assert_eq!(
        reopened.delivery_plan().state(),
        AuthoredDeliveryState::Retryable
    );
    assert_eq!(
        engine.deliver_push(push.operation_id()).await,
        Err(Error::DeliveryDeferred)
    );
    clock.0.store(retry_at, Ordering::Relaxed);
    let completed = engine
        .deliver_push(push.operation_id())
        .await
        .expect("retry after reopen");
    assert_eq!(completed.plan().attempt_count(), 2);
    assert_eq!(completed.plan().state(), AuthoredDeliveryState::Satisfied);
    assert_eq!(sink.requests.lock().expect("request log").len(), 1);
}
