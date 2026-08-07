//! Client-scoped access to the canonical synchronization engine.

#[cfg(feature = "sync")]
use std::sync::Arc;

#[cfg(feature = "sync")]
use radroots_storage::{authored_delivery::AuthoredDeliveryPlan, projection::ProjectionId};
#[cfg(feature = "sync")]
use radroots_sync::{
    Engine, PullReceipt, PullRequest, PushRequest, SyncStatus,
    ingest::{AdmissionPolicy, IngestBatchReceipt, IngestReceipt},
    policy::Error,
    projection::{Reducer, RefreshReceipt, RefreshRequest},
    push::{
        AdmissionRunReceipt, DeliveryExecutionReceipt, PushCancellationReceipt, PushPreparation,
        PushStatus, SigningRunReceipt,
    },
};
#[cfg(feature = "sync")]
use radroots_transport::source::ObservedEvent;

/// Explicit host policy for SDK-composed synchronization.
///
/// Selecting this policy opts into the system clock and operating-system
/// randomness for operation IDs. It creates no executor, timer, or worker.
#[cfg(feature = "sync")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostPolicy {
    deadlines: radroots_sync::policy::DeadlinePolicy,
}

#[cfg(feature = "sync")]
impl HostPolicy {
    /// Creates a bounded policy for pull, signing, and delivery calls.
    pub fn new(
        pull_timeout_ms: u64,
        sign_timeout_ms: u64,
        delivery_timeout_ms: u64,
    ) -> Result<Self, radroots_sync::policy::Error> {
        Ok(Self {
            deadlines: radroots_sync::policy::DeadlinePolicy::new(
                pull_timeout_ms,
                sign_timeout_ms,
                delivery_timeout_ms,
            )?,
        })
    }

    /// Returns the ordinary bounded native-host policy.
    #[must_use]
    pub fn standard() -> Self {
        Self::new(30_000, 30_000, 30_000).expect("static host deadlines are valid")
    }

    pub(crate) fn composition(
        self,
    ) -> (
        Arc<dyn radroots_sync::policy::Clock>,
        Arc<dyn radroots_sync::policy::IdSource>,
        radroots_sync::policy::DeadlinePolicy,
    ) {
        (Arc::new(SystemClock), Arc::new(RandomIds), self.deadlines)
    }
}

#[cfg(feature = "sync")]
impl Default for HostPolicy {
    fn default() -> Self {
        Self::standard()
    }
}

#[cfg(feature = "sync")]
struct SystemClock;

#[cfg(feature = "sync")]
impl radroots_sync::policy::Clock for SystemClock {
    fn now_unix_ms(&self) -> Result<u64, radroots_sync::policy::Error> {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|duration| u64::try_from(duration.as_millis()).ok())
            .filter(|value| *value != 0)
            .ok_or(radroots_sync::policy::Error::ClockUnavailable)
    }
}

#[cfg(feature = "sync")]
struct RandomIds;

#[cfg(feature = "sync")]
impl radroots_sync::policy::IdSource for RandomIds {
    fn next_id(
        &self,
        _operation: radroots_sync::policy::OperationKind,
    ) -> Result<radroots_sync::policy::SyncId, radroots_sync::policy::Error> {
        radroots_sync::policy::SyncId::new(*uuid::Uuid::new_v4().as_bytes())
    }
}

/// Borrowed client operations over one explicitly composed sync engine.
///
/// This type owns no scheduling, retries, status strings, outbox state, or
/// projection state. Every method delegates once to the canonical engine and
/// returns its native receipt or error.
#[cfg(feature = "sync")]
#[derive(Clone, Copy)]
pub struct Operations<'a> {
    engine: &'a Engine,
}

#[cfg(feature = "sync")]
impl<'a> Operations<'a> {
    pub(crate) const fn new(engine: &'a Engine) -> Self {
        Self { engine }
    }

    /// Runs one caller-bounded pull and canonical ingest sequence.
    pub async fn pull(
        &self,
        request: PullRequest,
        admission: &dyn AdmissionPolicy,
    ) -> Result<PullReceipt, Error> {
        self.engine.pull(request, admission).await
    }

    /// Verifies and atomically ingests one observed event.
    pub async fn ingest(
        &self,
        observed: ObservedEvent,
        admission: &dyn AdmissionPolicy,
    ) -> Result<IngestReceipt, Error> {
        self.engine.ingest(observed, admission).await
    }

    /// Ingests a bounded caller-owned batch while preserving partial outcomes.
    pub async fn ingest_batch(
        &self,
        observed: Vec<ObservedEvent>,
        admission: &dyn AdmissionPolicy,
    ) -> IngestBatchReceipt {
        self.engine.ingest_batch(observed, admission).await
    }

    /// Runs one bounded projection refresh through its owning reducer.
    pub async fn refresh_projection(
        &self,
        request: RefreshRequest,
        reducer: &dyn Reducer,
    ) -> Result<RefreshReceipt, Error> {
        self.engine.refresh_projection(request, reducer).await
    }

    /// Atomically persists one complete authored operation before any side effect.
    pub async fn prepare_push(&self, request: PushRequest) -> Result<PushPreparation, Error> {
        self.engine.prepare_push(request).await
    }

    /// Returns the complete durable state of one prepared push operation.
    pub async fn push_status(
        &self,
        operation_id: radroots_sync::policy::SyncId,
    ) -> Result<Option<PushStatus>, Error> {
        self.engine.push_status(operation_id).await
    }

    /// Cancels every remaining local phase while preserving durable evidence.
    pub async fn cancel_push(
        &self,
        operation_id: radroots_sync::policy::SyncId,
    ) -> Result<PushCancellationReceipt, Error> {
        self.engine.cancel_push(operation_id).await
    }

    /// Runs one bounded signing phase for an exactly prepared operation.
    pub async fn sign_prepared(&self, request: PushRequest) -> Result<SigningRunReceipt, Error> {
        self.engine.sign_prepared(request).await
    }

    /// Runs one bounded local-admission phase for a durably signed artifact.
    pub async fn admit_signed(
        &self,
        operation_id: radroots_sync::policy::SyncId,
    ) -> Result<AdmissionRunReceipt, Error> {
        self.engine.admit_signed(operation_id).await
    }

    /// Prepares, signs, and locally admits one authored operation.
    ///
    /// Preparation commits the complete recoverable intent before the signer
    /// can be invoked. Delivery remains an explicit caller-driven phase.
    pub async fn submit_push(&self, request: PushRequest) -> Result<PushStatus, Error> {
        let operation_id = request.operation_id();
        self.sign_prepared(request).await?;
        self.admit_signed(operation_id).await?;
        self.push_status(operation_id)
            .await?
            .ok_or(Error::StorageFailed)
    }

    /// Runs one bounded delivery attempt for one durable authored plan.
    pub async fn deliver_push(
        &self,
        operation_id: radroots_sync::policy::SyncId,
    ) -> Result<DeliveryExecutionReceipt, Error> {
        self.engine.deliver_push(operation_id).await
    }

    /// Returns the native passive sync status without starting recovery work.
    pub async fn status(&self, projections: &[ProjectionId]) -> Result<SyncStatus, Error> {
        self.engine.status(projections).await
    }

    /// Returns the native host scheduling decision for one durable plan.
    pub fn retry_decision(
        &self,
        plan: &AuthoredDeliveryPlan,
        now_unix_ms: u64,
    ) -> Result<radroots_protocol::runtime::v1::SyncRetryDecision, Error> {
        self.engine.retry_decision(plan, now_unix_ms)
    }
}

#[cfg(feature = "sync")]
impl std::fmt::Debug for Operations<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Operations")
            .field("engine", &"<borrowed canonical engine>")
            .finish()
    }
}

#[cfg(all(test, feature = "sync", feature = "memory"))]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    };

    use radroots_event::{
        GenericEventDraft, SignedEvent, contract::AuthorRole, wire::Nip01EventWire,
    };
    use radroots_event_codec::authoring::AuthoredEventPlan;
    use radroots_identity::PublicKey;
    use radroots_protocol::runtime::v1::SyncCapabilityState;
    use radroots_signing::{Actor, actor::ActorSource, request::CancellationPolicy};
    use radroots_storage::{
        event::{SourceGeneration, StoredVisibleEvent},
        journal::IdempotencyKey,
        memory::MemoryStorage,
        projection::{
            ProjectionGeneration, ProjectionId, RawSourceDigest, RebuildFailure, RebuildTicketId,
        },
    };
    use radroots_sync::{
        Engine, PullRequest, PushRequest,
        ingest::RegistryPolicy,
        policy::{Clock, DeadlinePolicy, Error, IdSource, OperationKind, SyncId, SyncStorage},
        projection::{Reducer, ReducerError, RefreshRequest, RefreshState},
        pull::PullTermination,
    };
    use radroots_transport::{
        Error as TransportError, EventSource, FetchPage, FetchRequest, SourceStatus, Target,
        TargetSet, TransportId,
        capability::{Availability, Maturity, SourceCapabilities},
        policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
        source::{EventProvenance, NextPage, ObservedEvent},
    };

    use crate::{ClientBuilder, error::ErrorKind};

    const PUBLIC_KEY: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

    struct FixedClock;
    struct SequenceIds(AtomicU8);
    struct CancelledSource;

    impl Clock for FixedClock {
        fn now_unix_ms(&self) -> Result<u64, Error> {
            Ok(1_700_000_000_000)
        }
    }

    impl IdSource for SequenceIds {
        fn next_id(&self, _operation: OperationKind) -> Result<SyncId, Error> {
            let next = self.0.fetch_add(1, Ordering::Relaxed);
            SyncId::new([next; 16])
        }
    }

    impl EventSource for CancelledSource {
        fn status(
            &self,
        ) -> radroots_transport::BoxFuture<'_, Result<SourceStatus, TransportError>> {
            Box::pin(async {
                Ok(SourceStatus::new(
                    TransportId::NOSTR,
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
            request: FetchRequest,
        ) -> radroots_transport::BoxFuture<'_, Result<FetchPage, TransportError>> {
            Box::pin(async move {
                FetchPage::for_request(
                    &request,
                    Vec::new(),
                    Vec::new(),
                    NextPage::Cancelled { resume_from: None },
                )
            })
        }
    }

    struct EmptyReducer {
        id: ProjectionId,
        generation: ProjectionGeneration,
    }

    impl Reducer for EmptyReducer {
        fn projection_id(&self) -> &ProjectionId {
            &self.id
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
            Ok(())
        }

        fn reduce(
            &self,
            events: &[StoredVisibleEvent],
            prior_projected_rows: u64,
            _rebuild_ticket: Option<RebuildTicketId>,
        ) -> Result<u64, ReducerError> {
            assert!(events.is_empty());
            Ok(prior_projected_rows)
        }

        fn abort_rebuild(
            &self,
            _ticket_id: RebuildTicketId,
            _failure: RebuildFailure,
        ) -> Result<(), ReducerError> {
            Ok(())
        }
    }

    fn target() -> Target {
        Target::nostr_relay("wss://sync.example").expect("target")
    }

    fn engine(storage: Arc<MemoryStorage>) -> Engine {
        let capability: Arc<dyn SyncStorage> = storage;
        Engine::builder(
            capability,
            Arc::new(FixedClock),
            Arc::new(SequenceIds(AtomicU8::new(1))),
            DeadlinePolicy::new(1_000, 1_000, 1_000).expect("deadlines"),
        )
        .source(Arc::new(CancelledSource))
        .build()
        .expect("engine")
    }

    fn invalid_observation() -> ObservedEvent {
        let mut wire = Nip01EventWire {
            id: "0".repeat(64),
            pubkey: PUBLIC_KEY.to_owned(),
            created_at: 1_800_000_100,
            kind: 0,
            tags: vec![],
            content: "invalid signature fixture".to_owned(),
            sig: "42".repeat(64),
            extra: Default::default(),
        };
        wire.id = wire.computed_event_id().expect("event id").to_hex();
        let raw = format!(
            "{{\"id\":\"{}\",\"pubkey\":\"{}\",\"created_at\":{},\"kind\":0,\"tags\":[],\"content\":\"invalid signature fixture\",\"sig\":\"{}\"}}",
            wire.id, wire.pubkey, wire.created_at, wire.sig
        );
        let event = SignedEvent::from_wire_verified_id(wire, raw).expect("signed event");
        let target = target();
        let provenance = EventProvenance::new(
            TransportId::NOSTR,
            target.fingerprint().clone(),
            1_700_000_000_000,
        )
        .expect("provenance");
        ObservedEvent::new(event, provenance)
    }

    fn push_request() -> PushRequest {
        let actor = Actor::new(
            PublicKey::from_hex(PUBLIC_KEY).expect("public key"),
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Any],
        )
        .expect("actor");
        let plan = AuthoredEventPlan::from_generic(
            GenericEventDraft::new(
                "radroots.social.geochat.v1",
                20_000,
                1_700_000_000,
                Vec::new(),
                "content",
                PUBLIC_KEY,
            )
            .expect("draft"),
        )
        .expect("authored plan");
        PushRequest::new(
            SyncId::new([8; 16]).expect("operation id"),
            IdempotencyKey::parse("sdk-sync-wrapper").expect("idempotency key"),
            actor,
            plan,
            TargetSet::new(vec![target()]).expect("targets"),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
            1_700_000_001_000,
            CancellationPolicy::PreservePublishedRequest,
        )
        .expect("push request")
    }

    #[tokio::test]
    async fn operations_delegate_native_pull_ingest_projection_push_delivery_and_status() {
        let storage = Arc::new(MemoryStorage::new(
            SourceGeneration::new([3; 32]).expect("generation"),
        ));
        let client = ClientBuilder::new()
            .storage(storage.clone())
            .sync_engine(engine(storage))
            .build()
            .expect("client");
        let operations = client.sync().expect("open client").expect("sync");
        let policy = RegistryPolicy::verified();

        let pull = operations
            .pull(
                PullRequest::new(TargetSet::new(vec![target()]).expect("targets"), 10, 1)
                    .expect("pull request"),
                &policy,
            )
            .await
            .expect("pull");
        assert_eq!(pull.termination(), PullTermination::Cancelled);

        let ingest = operations
            .ingest_batch(vec![invalid_observation(), invalid_observation()], &policy)
            .await;
        assert_eq!(ingest.accepted(), 0);
        assert_eq!(ingest.rejected(), 2);
        assert!(
            ingest
                .outcomes()
                .iter()
                .all(|outcome| matches!(outcome, Err(Error::VerificationFailed)))
        );

        let projection_id = ProjectionId::parse("sdk.sync.test").expect("projection id");
        let generation = ProjectionGeneration::new([5; 32]).expect("generation");
        let projection = operations
            .refresh_projection(
                RefreshRequest::new(projection_id.clone(), generation, 10, 1)
                    .expect("refresh request"),
                &EmptyReducer {
                    id: projection_id.clone(),
                    generation,
                },
            )
            .await
            .expect("projection");
        assert_eq!(projection.state(), RefreshState::Complete);

        let push = push_request();
        let operation_id = push.operation_id();
        let preparation = operations
            .prepare_push(push.clone())
            .await
            .expect("durable preparation");
        assert!(!preparation.is_replay());
        assert_eq!(
            operations.sign_prepared(push).await,
            Err(Error::MissingSigner)
        );
        assert!(
            operations
                .push_status(operation_id)
                .await
                .expect("push status")
                .is_some()
        );
        assert_eq!(
            operations.deliver_push(operation_id).await,
            Err(Error::MissingSink)
        );

        let status = operations
            .status(std::slice::from_ref(&projection_id))
            .await
            .expect("status");
        assert_eq!(status.source().state(), SyncCapabilityState::Available);
        assert_eq!(status.sink().state(), SyncCapabilityState::Unsupported);
        assert_eq!(status.projections().len(), 1);

        client.close().await.expect("close");
        assert_eq!(
            client.sync().expect_err("closed").kind(),
            ErrorKind::ClientClosed
        );
    }
}
