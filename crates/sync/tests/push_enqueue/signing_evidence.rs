use super::*;
use futures::channel::oneshot;
use radroots_signing::{AuthoredSignEvidence, SigningIntentId, SigningOperationId};
use radroots_storage::{
    authored::{AdmissionState, FailureClass, SigningState, WorkFailure, WorkPhase},
    authored_atomic::{ApplyWorkFailure, AuthoredAtomicCommand, AuthoredWorkTarget, WorkFence},
};

const NOW: u64 = 1_800_000_200_000;
type Pending = (
    SignRequest,
    oneshot::Sender<Result<AuthoredSignEvidence, SigningError>>,
);

#[derive(Default)]
struct HeldSigner {
    pending: Mutex<VecDeque<Pending>>,
    evidence_calls: AtomicUsize,
    legacy_calls: AtomicUsize,
}

impl HeldSigner {
    fn take(&self) -> Pending {
        self.pending
            .lock()
            .unwrap()
            .pop_front()
            .expect("started request")
    }
}

impl Signer for HeldSigner {
    fn status(
        &self,
    ) -> radroots_signing::signer::BoxFuture<'_, Result<SignerStatus, SigningError>> {
        Box::pin(async {
            Ok(SignerStatus::new(
                SignerAvailability::Ready,
                vec![SignerCapability::new(
                    SignerKind::Remote,
                    ReplayCapability::ExactReplayByRequestId,
                    CancellationSupport::BeforeAndAfterPublication,
                    false,
                    false,
                )],
                None,
            ))
        })
    }

    fn sign(
        &self,
        _: SignRequest,
    ) -> radroots_signing::signer::BoxFuture<'_, Result<SignReceipt, SigningError>> {
        self.legacy_calls.fetch_add(1, Ordering::Relaxed);
        Box::pin(async { Err(SigningError::new(SigningErrorKind::InternalError)) })
    }

    fn sign_authored_evidence(
        &self,
        request: SignRequest,
    ) -> radroots_signing::signer::BoxFuture<'_, Result<AuthoredSignEvidence, SigningError>> {
        Box::pin(async move {
            self.evidence_calls.fetch_add(1, Ordering::Relaxed);
            let (sender, receiver) = oneshot::channel();
            self.pending.lock().unwrap().push_back((request, sender));
            receiver
                .await
                .map_err(|_| SigningError::new(SigningErrorKind::SignerUnavailable))?
        })
    }
}

fn engine(
    storage: Arc<dyn SyncStorage>,
    clock: Arc<dyn Clock>,
    signer: Option<Arc<dyn Signer>>,
) -> Engine {
    let builder = Engine::builder(
        storage,
        clock,
        Arc::new(TestIds(AtomicU64::new(100))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).unwrap(),
    )
    .sink(Arc::new(MockSink));
    if let Some(signer) = signer {
        builder.signer(signer).build().unwrap()
    } else {
        builder.build().unwrap()
    }
}

fn setup() -> (Engine, Arc<MemoryStorage>, Arc<TestClock>, Arc<HeldSigner>) {
    let storage = Arc::new(MemoryStorage::new(SourceGeneration::new([17; 32]).unwrap()));
    let clock = Arc::new(TestClock(AtomicU64::new(NOW)));
    let signer = Arc::new(HeldSigner::default());
    (
        engine(storage.clone(), clock.clone(), Some(signer.clone())),
        storage,
        clock,
        signer,
    )
}

fn complete((request, sender): Pending, at: u64) -> SignedEvent {
    let event = signed_event(&request);
    let evidence = AuthoredSignEvidence::from_signed_event(&request, event.clone(), at).unwrap();
    sender.send(Ok(evidence)).unwrap();
    event
}

#[test]
fn missing_observation_clock_preserves_uncertain_non_replayable_attempt() {
    struct FaultClock(AtomicUsize);
    impl Clock for FaultClock {
        fn now_unix_ms(&self) -> Result<u64, Error> {
            match self.0.fetch_add(1, Ordering::Relaxed) {
                2 => Err(Error::ClockUnavailable),
                0 | 1 => Ok(NOW),
                _ => Ok(NOW + 11_000),
            }
        }
    }
    let storage = Arc::new(MemoryStorage::new(SourceGeneration::new([37; 32]).unwrap()));
    let signer = Arc::new(MockSigner::with_replay(
        SignBehavior::Success {
            completed_at_unix_ms: NOW,
        },
        ReplayCapability::NonReplayable,
    ));
    let engine = engine(
        storage,
        Arc::new(FaultClock(AtomicUsize::new(0))),
        Some(signer.clone()),
    );
    let push = request(37, "wss://relay.example");
    assert_eq!(
        block_on(engine.sign_prepared(push.clone())),
        Err(Error::ClockUnavailable)
    );
    let status = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    assert!(status.artifact().signing_claim().is_some());
    assert!(status.artifact().signed().is_none());
    assert!(status.artifact().last_failure().is_none());
    assert_eq!(
        block_on(engine.sign_prepared(push)),
        Err(Error::SigningIndeterminate)
    );
    assert_eq!(signer.calls.load(Ordering::Relaxed), 1);
}

#[test]
fn late_evidence_survives_durable_cancel_and_cannot_schedule_more_work() {
    let (engine, storage, clock, signer) = setup();
    let push = request(31, "wss://relay.example");
    let mut future = Box::pin(engine.sign_prepared(push.clone())).fuse();
    assert!(
        future
            .poll_unpin(&mut std::task::Context::from_waker(noop_waker_ref()))
            .is_pending()
    );
    let pending = signer.take();
    block_on(engine.cancel_push(push.operation_id())).unwrap();
    clock.0.store(NOW + 11_000, Ordering::Release);
    let event = complete(pending, NOW + 11_000);
    assert_eq!(block_on(future), Err(Error::SigningCancelled));
    let status = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    assert_eq!(status.artifact().signing_state(), SigningState::Cancelled);
    assert_eq!(status.artifact().signed().unwrap().event(), &event);
    assert_eq!(
        status.delivery_plan().state(),
        AuthoredDeliveryState::Cancelled
    );
    assert!(block_on(engine.admit_signed(push.operation_id())).is_err());
    assert_eq!(
        block_on(engine.sign_prepared(push)),
        Err(Error::SigningCancelled)
    );
    assert_eq!(signer.evidence_calls.load(Ordering::Relaxed), 1);
    assert_eq!(signer.legacy_calls.load(Ordering::Relaxed), 0);
    assert!(
        block_on(storage.query_visible(EventQuery::all(EventQueryBounds::first(10).unwrap())))
            .unwrap()
            .items()
            .is_empty()
    );
}

#[test]
fn superseded_attempts_retain_first_bytes_and_replay_without_a_signer() {
    let (engine, storage, clock, signer) = setup();
    let push = request(32, "wss://relay.example");
    let mut first = Box::pin(engine.sign_prepared(push.clone())).fuse();
    let mut context = std::task::Context::from_waker(noop_waker_ref());
    assert!(first.poll_unpin(&mut context).is_pending());
    let first_result = signer.take();
    clock.0.store(NOW + 11_000, Ordering::Release);
    let mut second = Box::pin(engine.sign_prepared(push.clone())).fuse();
    assert!(second.poll_unpin(&mut context).is_pending());
    let second_result = signer.take();
    assert_eq!(
        first_result.0.signer_request_id(),
        second_result.0.signer_request_id()
    );
    let event = complete(first_result, NOW + 11_000);
    assert_eq!(block_on(first), Err(Error::SignerDeadlineExceeded));
    let retained = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    assert_eq!(complete(second_result, NOW + 11_000), event);
    let result = block_on(second).unwrap();
    assert_eq!(result.artifact(), retained.artifact());
    assert_eq!(result.artifact().signed().unwrap().event(), &event);
    let recovered = self::engine(storage, clock, None);
    let replay = block_on(recovered.sign_prepared(push)).unwrap();
    assert!(replay.is_replay());
    assert_eq!(replay.artifact(), retained.artifact());
    assert_eq!(signer.evidence_calls.load(Ordering::Relaxed), 2);
    assert_eq!(signer.legacy_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn stale_signer_failure_cannot_overwrite_another_workers_signed_fact() {
    let (engine, _, clock, signer) = setup();
    let push = request(33, "wss://relay.example");
    let mut first = Box::pin(engine.sign_prepared(push.clone())).fuse();
    let mut context = std::task::Context::from_waker(noop_waker_ref());
    assert!(first.poll_unpin(&mut context).is_pending());
    let first_result = signer.take();
    clock.0.store(NOW + 11_000, Ordering::Release);
    let mut second = Box::pin(engine.sign_prepared(push.clone())).fuse();
    assert!(second.poll_unpin(&mut context).is_pending());
    complete(signer.take(), NOW + 11_000);
    let signed = block_on(second).unwrap();
    first_result
        .1
        .send(Err(SigningError::new(SigningErrorKind::SignerRejected)))
        .unwrap();
    assert_eq!(block_on(first), Err(Error::SignerFailed));
    let status = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    assert_eq!(status.artifact(), signed.artifact());
    assert!(status.artifact().last_failure().is_none());
}

#[test]
fn valid_signature_under_another_operation_or_artifact_is_rejected() {
    for change_operation in [false, true] {
        let (engine, _, _, signer) = setup();
        let push = request(34, "wss://relay.example");
        let mut future = Box::pin(engine.sign_prepared(push.clone())).fuse();
        assert!(
            future
                .poll_unpin(&mut std::task::Context::from_waker(noop_waker_ref()))
                .is_pending()
        );
        let (original, sender) = signer.take();
        let intent = if change_operation {
            SigningIntentId::new(
                SigningOperationId::new([99; 16]).unwrap(),
                original.intent_id().artifact_id(),
            )
        } else {
            SigningIntentId::new(
                original.intent_id().operation_id(),
                radroots_signing::AuthoredArtifactId::new([99; 16]).unwrap(),
            )
        };
        let other = SignRequest::new(
            original.operation_kind(),
            intent,
            original.actor().clone(),
            original.authored_plan().unwrap().clone(),
            original.policy(),
        )
        .unwrap();
        let evidence =
            AuthoredSignEvidence::from_signed_event(&other, signed_event(&other), NOW).unwrap();
        sender.send(Ok(evidence)).unwrap();
        assert_eq!(block_on(future), Err(Error::SignerFailed));
        let status = block_on(engine.push_status(push.operation_id()))
            .unwrap()
            .unwrap();
        assert!(status.artifact().signed().is_none());
        assert!(status.delivery_plan().request().is_none());
        assert!(status.delivery_plan().attempts().is_empty());
    }
}

#[test]
fn cancelled_indeterminate_operation_keeps_late_facts_without_admission() {
    let (engine, storage, clock, signer) = setup();
    let push = request(35, "wss://relay.example");
    let mut future = Box::pin(engine.sign_prepared(push.clone())).fuse();
    assert!(
        future
            .poll_unpin(&mut std::task::Context::from_waker(noop_waker_ref()))
            .is_pending()
    );
    let pending = signer.take();
    let status = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    let claim = status.artifact().signing_claim().unwrap();
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::ApplyFailure(
            ApplyWorkFailure::new(
                AuthoredWorkTarget::Artifact(status.artifact().artifact_id()),
                WorkFence::new(*claim.token(), claim.generation(), claim.row_revision()).unwrap(),
                WorkFailure::new(
                    "signing_unknown",
                    WorkPhase::Signing,
                    FailureClass::Indeterminate,
                    None,
                    None,
                )
                .unwrap(),
                None,
                NOW + 1,
            )
            .unwrap(),
        )),
    )
    .unwrap();
    clock.0.store(NOW + 2, Ordering::Release);
    let stopped = block_on(engine.cancel_push(push.operation_id())).unwrap();
    assert_eq!(
        stopped.status().artifact().signing_state(),
        SigningState::Indeterminate
    );
    let event = complete(pending, NOW + 2);
    assert_eq!(block_on(future), Err(Error::SigningCancelled));
    let status = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    assert_eq!(status.artifact().signed().unwrap().event(), &event);
    assert_eq!(
        status.artifact().admission_state(),
        AdmissionState::Cancelled
    );
    assert_eq!(
        block_on(engine.admit_signed(push.operation_id())),
        Err(Error::AdmissionFailed)
    );
    assert!(status.delivery_plan().attempts().is_empty());
}

#[tokio::test]
async fn late_sqlite_fact_reopens_without_requiring_missing_credentials() {
    let directory = tempfile::tempdir().unwrap();
    let paths = Paths::from_directory(directory.path()).unwrap();
    let clock = Arc::new(TestClock(AtomicU64::new(NOW)));
    let store = Arc::new(
        SqliteStorage::open(
            OpenOptions::new(paths.clone(), OpenMode::Create)
                .with_source_generation(SourceGeneration::new([36; 32]).unwrap(), 1)
                .unwrap(),
        )
        .await
        .unwrap(),
    );
    let signer = Arc::new(BoundaryViolatingSigner {
        violation: BoundaryViolation::CompletesAfterDeadline,
        clock: clock.clone(),
    });
    let original = engine(store.clone(), clock.clone(), Some(signer));
    let push = request(36, "wss://relay.example");
    assert_eq!(
        original.sign_prepared(push.clone()).await,
        Err(Error::SignerDeadlineExceeded)
    );
    let before = original
        .push_status(push.operation_id())
        .await
        .unwrap()
        .unwrap();
    assert!(before.artifact().signed().is_some());
    drop(original);
    store.close().await.unwrap();
    let store = Arc::new(
        SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadWriteExisting))
            .await
            .unwrap(),
    );
    let unavailable = Arc::new(MockSigner::new(SignBehavior::Error(
        SigningErrorKind::SignerUnavailable,
    )));
    let recovered = engine(store.clone(), clock, Some(unavailable.clone()));
    let replay = recovered.sign_prepared(push).await.unwrap();
    assert!(replay.is_replay());
    assert_eq!(replay.artifact(), before.artifact());
    assert_eq!(unavailable.calls.load(Ordering::Relaxed), 0);
    drop(recovered);
    store.close().await.unwrap();
}
