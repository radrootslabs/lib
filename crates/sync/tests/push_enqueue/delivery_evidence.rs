use super::*;
use futures::channel::oneshot;
use radroots_storage::{
    authored_atomic::{AuthoredAtomicCommand, RecordDeliveryFact},
    authored_delivery::DeliveryAttemptOutcome,
};
use radroots_transport::policy::SatisfactionState;

type Pending = (
    DeliveryRequest,
    oneshot::Sender<Result<DeliveryReceipt, SinkFailure>>,
);

#[derive(Default)]
struct HeldSink {
    pending: Mutex<VecDeque<Pending>>,
    calls: AtomicUsize,
}

impl HeldSink {
    fn take(&self) -> Pending {
        self.pending.lock().unwrap().pop_front().unwrap()
    }
}
impl EventSink for HeldSink {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SinkStatus, TransportError>> {
        Box::pin(async { panic!("delivery must not probe transport status") })
    }
    fn deliver(
        &self,
        request: DeliveryRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<DeliveryReceipt, SinkFailure>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let (sender, receiver) = oneshot::channel();
            self.pending.lock().unwrap().push_back((request, sender));
            receiver.await.expect("host retains admitted future")
        })
    }
}
fn setup(
    byte: u8,
) -> (
    Engine,
    Arc<MemoryStorage>,
    Arc<TestClock>,
    Arc<HeldSink>,
    PushRequest,
) {
    let sink = Arc::new(HeldSink::default());
    let ((engine, storage), clock) = setup_engine_with_sink(
        Arc::new(MockSigner::new(SignBehavior::Success {
            completed_at_unix_ms: 1_800_000_200_500,
        })),
        sink.clone(),
    );
    let push = request(byte, "wss://relay.example");
    execute_to_admitted(&engine, &push);
    (engine, storage, clock, sink, push)
}
fn source_only(storage: Arc<dyn SyncStorage>, clock: Arc<dyn Clock>) -> Engine {
    Engine::builder(
        storage,
        clock,
        Arc::new(TestIds(AtomicU64::new(230))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).unwrap(),
    )
    .source(Arc::new(MockSource))
    .build()
    .unwrap()
}
fn accept((request, sender): Pending) -> DeliveryReceipt {
    let result = receipt(
        &request,
        vec![DeliveryOutcome::accepted(); request.target_set().len()],
    )
    .unwrap();
    sender.send(Ok(result.clone())).unwrap();
    result
}

#[test]
fn late_accepted_result_survives_stop_and_terminal_replay_without_sink() {
    let (engine, storage, clock, sink, push) = setup(121);
    let status = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    assert!(status.delivery_history().proves_no_issued_attempt());
    let mut future = Box::pin(engine.deliver_push(push.operation_id()));
    assert!(
        future
            .poll_unpin(&mut std::task::Context::from_waker(noop_waker_ref()))
            .is_pending()
    );
    let pending = sink.take();
    let stopped = block_on(engine.cancel_push(push.operation_id())).unwrap();
    assert!(stopped.changed());
    assert!(stopped.status().delivery_history().has_unresolved_claims());
    assert!(
        !stopped
            .status()
            .delivery_history()
            .proves_no_issued_attempt()
    );
    let stop_at = stopped.status().delivery_plan().stop_requested_at_unix_ms();
    let expected = accept(pending);
    let delivered = block_on(future).unwrap();
    assert_eq!(delivered.plan().state(), AuthoredDeliveryState::Cancelled);
    assert_eq!(delivered.plan().attempt_count(), 0);
    assert_eq!(
        delivered.plan().delivery_satisfaction().unwrap(),
        SatisfactionState::Satisfied
    );
    assert_eq!(
        delivered.plan().delivery_facts()[0].outcome(),
        &DeliveryAttemptOutcome::Receipt(expected)
    );
    let status = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    assert!(!status.delivery_history().has_unresolved_claims());
    assert_eq!(status.delivery_plan().stop_requested_at_unix_ms(), stop_at);
    assert!(
        !block_on(engine.cancel_push(push.operation_id()))
            .unwrap()
            .changed()
    );
    let replay = block_on(source_only(storage, clock).deliver_push(push.operation_id())).unwrap();
    assert!(replay.is_replay());
    assert_eq!(replay.plan(), status.delivery_plan());
    assert_eq!(sink.calls.load(Ordering::Relaxed), 1);
}

#[test]
fn expired_callback_retains_fact_but_only_fresh_invocation_reconciles() {
    let (engine, storage, clock, sink, push) = setup(122);
    let mut future = Box::pin(engine.deliver_push(push.operation_id()));
    assert!(
        future
            .poll_unpin(&mut std::task::Context::from_waker(noop_waker_ref()))
            .is_pending()
    );
    let before = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    clock.0.store(
        before
            .delivery_plan()
            .claim_evidence()
            .unwrap()
            .expires_at_unix_ms(),
        Ordering::Relaxed,
    );
    accept(sink.take());
    let late = block_on(future).unwrap();
    assert_eq!(late.plan().revision(), before.delivery_plan().revision());
    assert_eq!(late.plan().attempt_count(), 0);
    let recovery = source_only(storage, clock);
    let recovered = block_on(recovery.deliver_push(push.operation_id())).unwrap();
    assert!(recovered.is_replay());
    assert_eq!(recovered.plan().state(), AuthoredDeliveryState::Satisfied);
    assert_eq!(recovered.plan().attempt_count(), 1);
    assert_eq!(
        recovered.plan().attempts()[0].claim_evidence(),
        before.delivery_plan().claim_evidence()
    );
    assert_eq!(sink.calls.load(Ordering::Relaxed), 1);
}

#[test]
fn superseded_callback_cannot_clear_newer_claim_and_acceptance_never_regresses() {
    let (engine, _, clock, sink, push) = setup(123);
    let mut first = Box::pin(engine.deliver_push(push.operation_id()));
    assert!(
        first
            .poll_unpin(&mut std::task::Context::from_waker(noop_waker_ref()))
            .is_pending()
    );
    let first_pending = sink.take();
    let before = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    clock.0.store(
        before
            .delivery_plan()
            .claim_evidence()
            .unwrap()
            .expires_at_unix_ms(),
        Ordering::Relaxed,
    );
    let mut second = Box::pin(engine.deliver_push(push.operation_id()));
    assert!(
        second
            .poll_unpin(&mut std::task::Context::from_waker(noop_waker_ref()))
            .is_pending()
    );
    let (second_request, second_sender) = sink.take();
    assert_eq!(first_pending.0, second_request);
    let newer = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    accept(first_pending);
    let late = block_on(first).unwrap();
    assert_eq!(
        late.plan().claim_evidence(),
        newer.delivery_plan().claim_evidence()
    );
    assert_eq!(late.plan().revision(), newer.delivery_plan().revision());
    assert_eq!(
        block_on(engine.deliver_push(push.operation_id())),
        Err(Error::WorkClaimConflict)
    );
    second_sender
        .send(Err(SinkFailure::for_request(
            &second_request,
            "provider_failed",
            Retryability::Terminal,
            None,
            None,
            Vec::new(),
        )
        .unwrap()))
        .unwrap();
    let complete = block_on(second).unwrap();
    assert_eq!(complete.plan().state(), AuthoredDeliveryState::Satisfied);
    assert_eq!(complete.plan().attempt_count(), 2);
    assert_eq!(complete.plan().delivery_facts().len(), 2);
    assert_eq!(sink.calls.load(Ordering::Relaxed), 2);
}

#[test]
fn clock_loss_retains_raw_result_before_reporting_unavailable() {
    for accepted in [true, false] {
        let (engine, storage, clock, sink, push) = setup(124 + u8::from(accepted));
        let mut future = Box::pin(engine.deliver_push(push.operation_id()));
        assert!(
            future
                .poll_unpin(&mut std::task::Context::from_waker(noop_waker_ref()))
                .is_pending()
        );
        let (request, sender) = sink.take();
        let before = block_on(engine.push_status(push.operation_id()))
            .unwrap()
            .unwrap();
        let outcome = if accepted {
            DeliveryAttemptOutcome::Receipt(
                receipt(&request, vec![DeliveryOutcome::accepted()]).unwrap(),
            )
        } else {
            DeliveryAttemptOutcome::SinkFailure(
                SinkFailure::for_request(
                    &request,
                    "raw_failure",
                    Retryability::Retryable,
                    None,
                    Some("raw diagnostic".into()),
                    Vec::new(),
                )
                .unwrap(),
            )
        };
        sender
            .send(match &outcome {
                DeliveryAttemptOutcome::Receipt(value) => Ok(value.clone()),
                DeliveryAttemptOutcome::SinkFailure(value) => Err(value.clone()),
            })
            .unwrap();
        clock.0.store(0, Ordering::Relaxed);
        assert_eq!(block_on(future), Err(Error::ClockUnavailable));
        let status = block_on(engine.push_status(push.operation_id()))
            .unwrap()
            .unwrap();
        assert_eq!(
            status.delivery_plan().delivery_facts()[0].outcome(),
            &outcome
        );
        assert_eq!(
            status.delivery_plan().revision(),
            before.delivery_plan().revision()
        );
        assert_eq!(status.delivery_plan().attempt_count(), 0);
        let expires = status
            .delivery_plan()
            .claim_evidence()
            .unwrap()
            .expires_at_unix_ms();
        clock.0.store(expires, Ordering::Relaxed);
        let recovered =
            block_on(source_only(storage, clock).deliver_push(push.operation_id())).unwrap();
        assert_eq!(recovered.plan().attempt_count(), 1);
        assert_eq!(recovered.plan().attempts()[0].outcome(), &outcome);
        if !accepted {
            assert!(recovered.plan().retry().unwrap().not_before_unix_ms() >= expires + 1_000);
        }
        assert_eq!(sink.calls.load(Ordering::Relaxed), 1);
    }
}

#[test]
fn stop_before_delivery_proves_no_issued_attempt_and_stop_after_acceptance_keeps_success() {
    let (engine, _, _, sink, push) = setup(126);
    let stopped = block_on(engine.cancel_push(push.operation_id())).unwrap();
    assert!(
        stopped
            .status()
            .delivery_history()
            .proves_no_issued_attempt()
    );
    assert!(
        block_on(engine.deliver_push(push.operation_id()))
            .unwrap()
            .is_replay()
    );
    assert_eq!(sink.calls.load(Ordering::Relaxed), 0);
    let (engine, _, _, sink, push) = setup(127);
    let mut future = Box::pin(engine.deliver_push(push.operation_id()));
    assert!(
        future
            .poll_unpin(&mut std::task::Context::from_waker(noop_waker_ref()))
            .is_pending()
    );
    accept(sink.take());
    assert_eq!(
        block_on(future).unwrap().plan().state(),
        AuthoredDeliveryState::Satisfied
    );
    let stopped = block_on(engine.cancel_push(push.operation_id())).unwrap();
    assert!(stopped.changed());
    assert!(
        stopped
            .status()
            .delivery_plan()
            .stop_requested_at_unix_ms()
            .is_some()
    );
    assert_eq!(
        stopped.status().delivery_plan().state(),
        AuthoredDeliveryState::Satisfied
    );
    assert_eq!(
        stopped
            .status()
            .delivery_plan()
            .delivery_satisfaction()
            .unwrap(),
        SatisfactionState::Satisfied
    );
    assert!(
        !block_on(engine.cancel_push(push.operation_id()))
            .unwrap()
            .changed()
    );
}

#[test]
fn equal_fact_replay_preserves_first_observation_and_conflict_cannot_erase_acceptance() {
    let (engine, storage, _, sink, push) = setup(128);
    let mut future = Box::pin(engine.deliver_push(push.operation_id()));
    assert!(
        future
            .poll_unpin(&mut std::task::Context::from_waker(noop_waker_ref()))
            .is_pending()
    );
    accept(sink.take());
    let delivered = block_on(future).unwrap();
    let plan = delivered.plan();
    let fact = &plan.delivery_facts()[0];
    let repeated = AuthoredAtomicCommand::RecordDelivery(
        RecordDeliveryFact::new(
            plan.plan_id(),
            plan.artifact_id(),
            fact.claim().clone(),
            fact.outcome().clone(),
            fact.observed_at_unix_ms() + 10_000,
        )
        .unwrap(),
    );
    block_on(storage.execute_authored(repeated)).unwrap();
    let changed = DeliveryAttemptOutcome::Receipt(
        receipt(
            plan.request().unwrap(),
            vec![DeliveryOutcome::unavailable()],
        )
        .unwrap(),
    );
    let conflicting = AuthoredAtomicCommand::RecordDelivery(
        RecordDeliveryFact::new(
            plan.plan_id(),
            plan.artifact_id(),
            fact.claim().clone(),
            changed,
            fact.observed_at_unix_ms() + 10_000,
        )
        .unwrap(),
    );
    assert!(block_on(storage.execute_authored(conflicting)).is_err());
    let status = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    assert_eq!(status.delivery_plan(), plan);
}
#[test]
fn every_delivery_storage_receipt_is_checked_and_durable_results_remain_recoverable() {
    for nth in 1..=3 {
        let storage = Arc::new(FaultStorage::new(150 + nth as u8));
        let sink = Arc::new(ScriptedSink::new([DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::accepted(),
        ])]));
        let engine = fault_engine(
            storage.clone(),
            Arc::new(MockSigner::new(SignBehavior::Success {
                completed_at_unix_ms: 1_800_000_200_500,
            })),
            sink.clone(),
        );
        let push = request(150 + nth as u8, "wss://relay.example");
        execute_to_admitted(&engine, &push);
        storage.fault_nth_plan(nth);
        assert_eq!(
            block_on(engine.deliver_push(push.operation_id())),
            Err(Error::StorageFailed)
        );
        let status = block_on(engine.push_status(push.operation_id()))
            .unwrap()
            .unwrap();
        assert_eq!(sink.requests.lock().unwrap().len(), usize::from(nth > 1));
        assert_eq!(
            status.delivery_plan().delivery_facts().len(),
            usize::from(nth > 1)
        );
        if nth > 1 {
            let clock = Arc::new(TestClock(AtomicU64::new(1_800_000_250_000)));
            let recovered =
                block_on(source_only(storage, clock).deliver_push(push.operation_id())).unwrap();
            assert_eq!(recovered.plan().state(), AuthoredDeliveryState::Satisfied);
            assert_eq!(recovered.plan().attempt_count(), 1);
        }
    }
}

#[test]
fn legacy_applied_result_gets_exact_marker_without_counting_a_second_attempt() {
    legacy_reconciliation(false);
}

#[test]
fn conflicting_legacy_result_retains_both_observations_without_reconciliation() {
    legacy_reconciliation(true);
}

fn legacy_reconciliation(conflict: bool) {
    use core::num::NonZeroU32;
    use radroots_storage::{
        authored::{FailureClass, RetrySchedule, WorkFailure, WorkPhase},
        authored_atomic::{ApplyDeliveryAttempt, WorkFence},
    };
    let (engine, storage, clock, sink, push) = setup(154);
    let mut future = Box::pin(engine.deliver_push(push.operation_id()));
    assert!(
        future
            .poll_unpin(&mut std::task::Context::from_waker(noop_waker_ref()))
            .is_pending()
    );
    let (request, _sender) = sink.take();
    drop(future);
    let status = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    let plan = status.delivery_plan();
    let claim = plan.claim_evidence().unwrap();
    let at = claim.acquired_at_unix_ms() + 100;
    let outcome = DeliveryAttemptOutcome::Receipt(
        receipt(&request, vec![DeliveryOutcome::unavailable()]).unwrap(),
    );
    let retry = RetrySchedule::new(
        NonZeroU32::MIN,
        at + 1000,
        WorkFailure::new(
            "delivery_pending",
            WorkPhase::Delivery,
            FailureClass::Retryable,
            Some(at + 1000),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::ApplyDelivery(
            ApplyDeliveryAttempt::new(
                plan.plan_id(),
                WorkFence::new(*claim.token(), claim.generation(), claim.row_revision()).unwrap(),
                outcome.clone(),
                Some(retry),
                at,
            )
            .unwrap(),
        )),
    )
    .unwrap();
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::RecordDelivery(
            RecordDeliveryFact::new(
                plan.plan_id(),
                plan.artifact_id(),
                claim.clone(),
                if conflict {
                    DeliveryAttemptOutcome::Receipt(
                        receipt(&request, vec![DeliveryOutcome::accepted()]).unwrap(),
                    )
                } else {
                    outcome.clone()
                },
                at + 1,
            )
            .unwrap(),
        )),
    )
    .unwrap();
    clock.0.store(at + 2, Ordering::Relaxed);
    let result = block_on(source_only(storage, clock).deliver_push(push.operation_id()));
    if conflict {
        assert_eq!(result, Err(Error::StorageConflict));
        let current = block_on(engine.push_status(push.operation_id()))
            .unwrap()
            .unwrap();
        assert_eq!(current.delivery_plan().attempt_count(), 1);
        assert_eq!(current.delivery_plan().attempts()[0].outcome(), &outcome);
        assert_eq!(current.delivery_plan().delivery_facts().len(), 1);
        assert_eq!(
            current.delivery_plan().delivery_satisfaction().unwrap(),
            SatisfactionState::Satisfied
        );
        assert_eq!(sink.calls.load(Ordering::Relaxed), 1);
        return;
    }
    let reconciled = result.unwrap();
    assert_eq!(reconciled.plan().attempt_count(), 1);
    assert_eq!(reconciled.plan().attempts()[0].recorded_at_unix_ms(), at);
    assert_eq!(reconciled.plan().attempts()[0].outcome(), &outcome);
    assert_eq!(
        reconciled.plan().attempts()[0].claim_evidence(),
        Some(claim)
    );
    assert_eq!(sink.calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn sqlite_late_facts_reopen_after_stop_or_expiry_and_reconcile_once() {
    use radroots_storage::authored_atomic::{CancelAuthoredTarget, CancelAuthoredWork};
    struct BoundarySink {
        storage: Arc<SqliteStorage>,
        clock: Arc<TestClock>,
        id: radroots_storage::authored_delivery::AuthoredDeliveryPlanId,
        stop: bool,
    }
    impl EventSink for BoundarySink {
        fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SinkStatus, TransportError>> {
            Box::pin(async { panic!("no probe") })
        }
        fn deliver(
            &self,
            request: DeliveryRequest,
        ) -> radroots_transport::BoxFuture<'_, Result<DeliveryReceipt, SinkFailure>> {
            Box::pin(async move {
                let history = self
                    .storage
                    .authored_delivery_history(self.id)
                    .await
                    .unwrap()
                    .unwrap();
                let plan = history.plan();
                let at = plan.claim_evidence().unwrap().expires_at_unix_ms();
                self.clock.0.store(at, Ordering::Relaxed);
                if self.stop {
                    self.storage
                        .execute_authored(AuthoredAtomicCommand::Cancel(
                            CancelAuthoredWork::new(
                                CancelAuthoredTarget::DeliveryPlan(self.id),
                                plan.revision(),
                                at,
                            )
                            .unwrap(),
                        ))
                        .await
                        .unwrap();
                }
                Ok(receipt(&request, vec![DeliveryOutcome::accepted()]).unwrap())
            })
        }
    }
    for stop in [true, false] {
        let directory = tempfile::tempdir().unwrap();
        let paths = Paths::from_directory(directory.path()).unwrap();
        let storage = Arc::new(
            SqliteStorage::open(
                OpenOptions::new(paths.clone(), OpenMode::Create)
                    .with_source_generation(SourceGeneration::new([155; 32]).unwrap(), 1)
                    .unwrap(),
            )
            .await
            .unwrap(),
        );
        let clock = Arc::new(TestClock(AtomicU64::new(1_800_000_200_000)));
        let push = request(155, "wss://relay.example");
        let id = push
            .authored_preparation(1_800_000_200_000)
            .unwrap()
            .delivery_plans()[0]
            .plan_id();
        let engine = Engine::builder(
            storage.clone(),
            clock.clone(),
            Arc::new(TestIds(AtomicU64::new(10))),
            DeadlinePolicy::new(10_000, 10_000, 10_000).unwrap(),
        )
        .sink(Arc::new(BoundarySink {
            storage: storage.clone(),
            clock: clock.clone(),
            id,
            stop,
        }))
        .signer(Arc::new(MockSigner::new(SignBehavior::Success {
            completed_at_unix_ms: 1_800_000_200_500,
        })))
        .build()
        .unwrap();
        engine.sign_prepared(push.clone()).await.unwrap();
        engine.admit_signed(push.operation_id()).await.unwrap();
        let late = engine.deliver_push(push.operation_id()).await.unwrap();
        assert_eq!(late.plan().attempt_count(), 0);
        assert_eq!(
            late.plan().delivery_satisfaction().unwrap(),
            SatisfactionState::Satisfied
        );
        drop(engine);
        storage.close().await.unwrap();
        let storage = Arc::new(
            SqliteStorage::open(OpenOptions::new(paths.clone(), OpenMode::ReadWriteExisting))
                .await
                .unwrap(),
        );
        let recovery = source_only(storage.clone(), clock.clone());
        let recovered = recovery.deliver_push(push.operation_id()).await.unwrap();
        assert_eq!(recovered.plan().attempt_count(), u32::from(!stop));
        assert_eq!(
            recovered.plan().delivery_facts(),
            late.plan().delivery_facts()
        );
        let status = recovery
            .push_status(push.operation_id())
            .await
            .unwrap()
            .unwrap();
        assert!(!status.delivery_history().has_unresolved_claims());
        drop(recovery);
        storage.close().await.unwrap();
        let storage = Arc::new(
            SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadOnly))
                .await
                .unwrap(),
        );
        let readonly = source_only(storage.clone(), clock);
        let replay = readonly.deliver_push(push.operation_id()).await.unwrap();
        assert_eq!(replay.plan(), recovered.plan());
        drop(readonly);
        storage.close().await.unwrap();
    }
}
pub(super) enum Injection {
    FactBeforeClaim(Box<AuthoredAtomicCommand>),
    StopAfterClaim,
    ReplaceAfterClaim,
    StopAfterFact,
    StopAfterReconcile,
}

pub(super) async fn inject(storage: &FaultStorage, command: &AuthoredAtomicCommand, before: bool) {
    use radroots_storage::{
        authored::WorkClaim,
        authored_atomic::{
            CancelAuthoredTarget, CancelAuthoredWork, ClaimAuthoredTarget, ClaimAuthoredWork,
        },
    };
    let claim = matches!(command, AuthoredAtomicCommand::Claim(value)
        if matches!(value.target(), ClaimAuthoredTarget::DeliveryPlan(_)));
    let injection = {
        let mut armed = storage.delivery_injection.lock().unwrap();
        let ready = match armed.as_ref() {
            Some(Injection::FactBeforeClaim(_)) => before && claim,
            Some(Injection::StopAfterClaim | Injection::ReplaceAfterClaim) => !before && claim,
            Some(Injection::StopAfterFact) => {
                !before && matches!(command, AuthoredAtomicCommand::RecordDelivery(_))
            }
            Some(Injection::StopAfterReconcile) => {
                !before && matches!(command, AuthoredAtomicCommand::ReconcileDelivery(_))
            }
            None => false,
        };
        if ready { armed.take() } else { None }
    };
    let Some(injection) = injection else {
        return;
    };
    if let Injection::FactBeforeClaim(fact) = injection {
        storage.inner.execute_authored(*fact).await.unwrap();
        return;
    }
    let id = match command {
        AuthoredAtomicCommand::Claim(value) => match value.target() {
            ClaimAuthoredTarget::DeliveryPlan(id) => *id,
            _ => panic!("delivery-only injection"),
        },
        AuthoredAtomicCommand::RecordDelivery(value) => value.plan_id(),
        AuthoredAtomicCommand::ReconcileDelivery(value) => value.plan_id(),
        _ => panic!("delivery-only injection"),
    };
    let history = storage
        .inner
        .authored_delivery_history(id)
        .await
        .unwrap()
        .unwrap();
    let plan = history.plan();
    let mutation = if matches!(injection, Injection::ReplaceAfterClaim) {
        let original = plan.claim_evidence().unwrap();
        let at = original.expires_at_unix_ms();
        AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(id),
            WorkClaim::new(
                [222; 16],
                "competing-worker",
                core::num::NonZeroU64::new(2).unwrap(),
                at,
                at + 10_000,
                plan.revision(),
            )
            .unwrap(),
        ))
    } else {
        AuthoredAtomicCommand::Cancel(
            CancelAuthoredWork::new(
                CancelAuthoredTarget::DeliveryPlan(id),
                plan.revision(),
                plan.updated_at_unix_ms(),
            )
            .unwrap(),
        )
    };
    storage.inner.execute_authored(mutation).await.unwrap();
}

#[test]
fn observed_stop_or_replacement_after_claim_prevents_sink_admission() {
    for stop in [true, false] {
        let storage = Arc::new(FaultStorage::new(160));
        let sink = Arc::new(HeldSink::default());
        let engine = fault_engine(
            storage.clone(),
            Arc::new(MockSigner::new(SignBehavior::Success {
                completed_at_unix_ms: 1_800_000_200_500,
            })),
            sink.clone(),
        );
        let push = request(160, "wss://relay.example");
        execute_to_admitted(&engine, &push);
        *storage.delivery_injection.lock().unwrap() = Some(if stop {
            Injection::StopAfterClaim
        } else {
            Injection::ReplaceAfterClaim
        });
        let result = block_on(engine.deliver_push(push.operation_id()));
        if stop {
            assert_eq!(
                result.unwrap().plan().state(),
                AuthoredDeliveryState::Cancelled
            );
        } else {
            assert_eq!(result, Err(Error::WorkClaimConflict));
        }
        assert_eq!(sink.calls.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn fresh_claim_reconciles_fact_arriving_after_initial_status_without_another_effect() {
    let storage = Arc::new(FaultStorage::new(161));
    let sink = Arc::new(HeldSink::default());
    let clock = Arc::new(TestClock(AtomicU64::new(1_800_000_200_000)));
    let engine = Engine::builder(
        storage.clone(),
        clock.clone(),
        Arc::new(TestIds(AtomicU64::new(10))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).unwrap(),
    )
    .sink(sink.clone())
    .signer(Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    })))
    .build()
    .unwrap();
    let push = request(161, "wss://relay.example");
    execute_to_admitted(&engine, &push);
    let mut first = Box::pin(engine.deliver_push(push.operation_id()));
    assert!(
        first
            .poll_unpin(&mut std::task::Context::from_waker(noop_waker_ref()))
            .is_pending()
    );
    let (request, _sender) = sink.take();
    drop(first);
    let status = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    let plan = status.delivery_plan();
    let claim = plan.claim_evidence().unwrap();
    let at = claim.expires_at_unix_ms();
    let fact = AuthoredAtomicCommand::RecordDelivery(
        RecordDeliveryFact::new(
            plan.plan_id(),
            plan.artifact_id(),
            claim.clone(),
            DeliveryAttemptOutcome::Receipt(
                receipt(&request, vec![DeliveryOutcome::accepted()]).unwrap(),
            ),
            at,
        )
        .unwrap(),
    );
    *storage.delivery_injection.lock().unwrap() = Some(Injection::FactBeforeClaim(Box::new(fact)));
    clock.0.store(at, Ordering::Relaxed);
    let reconciled = block_on(engine.deliver_push(push.operation_id())).unwrap();
    assert!(reconciled.is_replay());
    assert_eq!(reconciled.plan().state(), AuthoredDeliveryState::Satisfied);
    assert_eq!(reconciled.plan().attempt_count(), 1);
    assert_eq!(sink.calls.load(Ordering::Relaxed), 1);
}

#[test]
fn receipt_return_reloads_stop_after_fact_or_reconciliation_commit() {
    for before_reconcile in [true, false] {
        let storage = Arc::new(FaultStorage::new(162));
        let sink = Arc::new(ScriptedSink::new([DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::accepted(),
        ])]));
        let engine = fault_engine(
            storage.clone(),
            Arc::new(MockSigner::new(SignBehavior::Success {
                completed_at_unix_ms: 1_800_000_200_500,
            })),
            sink.clone(),
        );
        let push = request(162, "wss://relay.example");
        execute_to_admitted(&engine, &push);
        *storage.delivery_injection.lock().unwrap() = Some(if before_reconcile {
            Injection::StopAfterFact
        } else {
            Injection::StopAfterReconcile
        });
        let delivered = block_on(engine.deliver_push(push.operation_id())).unwrap();
        assert!(delivered.plan().stop_requested_at_unix_ms().is_some());
        assert_eq!(
            delivered.plan().attempt_count(),
            u32::from(!before_reconcile)
        );
        assert_eq!(
            delivered.plan().delivery_satisfaction().unwrap(),
            SatisfactionState::Satisfied
        );
        assert_eq!(
            engine
                .retry_decision(delivered.plan(), 1_800_000_250_000)
                .unwrap(),
            SyncRetryDecision::Satisfied
        );
        assert_eq!(sink.requests.lock().unwrap().len(), 1);
    }
}

#[test]
fn forged_stop_receipt_is_not_reported_as_success() {
    let storage = Arc::new(FaultStorage::new(163));
    let engine = fault_engine(
        storage.clone(),
        Arc::new(MockSigner::new(SignBehavior::Success {
            completed_at_unix_ms: 1_800_000_200_500,
        })),
        Arc::new(MockSink),
    );
    let push = request(163, "wss://relay.example");
    execute_to_admitted(&engine, &push);
    storage.fault_nth_plan(1);
    assert_eq!(
        block_on(engine.cancel_push(push.operation_id())),
        Err(Error::StorageFailed)
    );
    let status = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    assert!(status.delivery_plan().stop_requested_at_unix_ms().is_some());
    assert!(status.delivery_history().proves_no_issued_attempt());
}
use radroots_storage::authored_atomic::{AuthoredAtomicOutcome, AuthoredAtomicReceipt};
use radroots_storage::authored_delivery::{AuthoredDeliveryHistory, AuthoredDeliveryPlan};

#[derive(Clone, Copy, Debug)]
pub(super) enum ReceiptMutation {
    Identity,
    Plan,
    Artifact,
    Request,
    Created,
    Claim,
    CommitTime,
    Attempts,
    State,
    Retry,
    StopTime,
    Revision,
}
pub(super) fn mutate_receipt(
    receipt: AuthoredAtomicReceipt,
    mutation: ReceiptMutation,
) -> AuthoredAtomicReceipt {
    let AuthoredAtomicOutcome::DeliveryPlan(plan) = receipt.outcome() else {
        panic!("delivery receipt");
    };
    let mut wire = serde_json::to_value(plan).unwrap();
    match mutation {
        ReceiptMutation::Identity | ReceiptMutation::CommitTime => {}
        ReceiptMutation::Plan => wire["plan_id"] = serde_json::json!(vec![241; 16]),
        ReceiptMutation::Artifact => wire["artifact_id"] = serde_json::json!(vec![242; 16]),
        ReceiptMutation::Request => {
            let original = plan.request().unwrap();
            let changed = DeliveryRequest::new(
                "different-request",
                original.payload().clone(),
                original.target_set().clone(),
                original.satisfaction().clone(),
                original.deadline_unix_ms(),
            )
            .unwrap();
            let other = AuthoredDeliveryPlan::new_bound(
                plan.plan_id(),
                plan.artifact_id(),
                changed,
                plan.created_at_unix_ms(),
            )
            .unwrap();
            let other = serde_json::to_value(other).unwrap();
            for key in ["request", "intent", "request_digest"] {
                wire[key] = other[key].clone();
            }
        }
        ReceiptMutation::Created => {
            wire["created_at_unix_ms"] = serde_json::json!(plan.created_at_unix_ms() - 1)
        }
        ReceiptMutation::Claim => wire["claim"]["owner"] = serde_json::json!("another-worker"),
        ReceiptMutation::Attempts => {
            let outcome = DeliveryAttemptOutcome::Receipt(receipt_for_pending(plan));
            wire["attempts"] = serde_json::json!([{"attempt":1,"recorded_at_unix_ms":plan.created_at_unix_ms(),"outcome":outcome,"satisfaction":"pending"}]);
            wire["attempt_count"] = serde_json::json!(1);
        }
        ReceiptMutation::State => {
            wire["state"] = serde_json::json!("pending");
            wire["retry"] = serde_json::Value::Null;
            wire["last_failure"] = serde_json::Value::Null;
        }
        ReceiptMutation::Retry => {
            let at = plan.retry().unwrap().not_before_unix_ms() + 1000;
            wire["retry"]["not_before_unix_ms"] = serde_json::json!(at);
            wire["retry"]["failure"]["retry_after_unix_ms"] = serde_json::json!(at);
            wire["last_failure"] = wire["retry"]["failure"].clone();
        }
        ReceiptMutation::StopTime => {
            wire["stop_requested_at_unix_ms"] =
                serde_json::json!(plan.stop_requested_at_unix_ms().unwrap() - 1)
        }
        ReceiptMutation::Revision => {
            wire["revision"] = serde_json::json!(plan.revision().get() + 1)
        }
    }
    let changed: AuthoredDeliveryPlan =
        serde_json::from_value(wire).expect("valid but incorrectly bound projection");
    AuthoredAtomicReceipt::from_durable_parts(
        if matches!(mutation, ReceiptMutation::Identity) {
            radroots_storage::atomic::AtomicCommitId::new([243; 16]).unwrap()
        } else {
            receipt.commit_id()
        },
        receipt.digest(),
        receipt.disposition(),
        receipt.committed_at_unix_ms() + u64::from(matches!(mutation, ReceiptMutation::CommitTime)),
        AuthoredAtomicOutcome::DeliveryPlan(changed),
    )
    .unwrap()
}
fn receipt_for_pending(plan: &AuthoredDeliveryPlan) -> DeliveryReceipt {
    receipt(
        plan.request().unwrap(),
        vec![DeliveryOutcome::unavailable()],
    )
    .unwrap()
}

#[test]
fn claim_projection_must_match_every_original_binding_before_transport() {
    for mutation in [
        ReceiptMutation::Identity,
        ReceiptMutation::Plan,
        ReceiptMutation::Artifact,
        ReceiptMutation::Request,
        ReceiptMutation::Created,
        ReceiptMutation::Claim,
        ReceiptMutation::CommitTime,
        ReceiptMutation::Attempts,
    ] {
        let storage = Arc::new(FaultStorage::new(171));
        let sink = Arc::new(HeldSink::default());
        let engine = fault_engine(
            storage.clone(),
            Arc::new(MockSigner::new(SignBehavior::Success {
                completed_at_unix_ms: 1_800_000_200_500,
            })),
            sink.clone(),
        );
        let push = request(171, "wss://relay.example");
        execute_to_admitted(&engine, &push);
        *storage.receipt_mutation.lock().unwrap() = Some(mutation);
        assert_eq!(
            block_on(engine.deliver_push(push.operation_id())),
            Err(Error::StorageFailed),
            "{mutation:?}"
        );
        assert_eq!(sink.calls.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn stop_projection_must_match_identity_request_time_and_revision() {
    for mutation in [
        ReceiptMutation::Identity,
        ReceiptMutation::Plan,
        ReceiptMutation::Artifact,
        ReceiptMutation::Request,
        ReceiptMutation::StopTime,
        ReceiptMutation::Revision,
    ] {
        let storage = Arc::new(FaultStorage::new(172));
        let engine = fault_engine(
            storage.clone(),
            Arc::new(MockSigner::new(SignBehavior::Success {
                completed_at_unix_ms: 1_800_000_200_500,
            })),
            Arc::new(MockSink),
        );
        let push = request(172, "wss://relay.example");
        execute_to_admitted(&engine, &push);
        *storage.receipt_mutation.lock().unwrap() = Some(mutation);
        assert_eq!(
            block_on(engine.cancel_push(push.operation_id())),
            Err(Error::StorageFailed),
            "{mutation:?}"
        );
        let current = block_on(engine.push_status(push.operation_id()))
            .unwrap()
            .unwrap();
        assert!(current.delivery_history().proves_no_issued_attempt());
        assert!(
            current
                .delivery_plan()
                .stop_requested_at_unix_ms()
                .is_some()
        );
    }
}

#[test]
fn claim_projection_cannot_replace_retained_retry_state_or_backoff() {
    for mutation in [ReceiptMutation::State, ReceiptMutation::Retry] {
        let storage = Arc::new(FaultStorage::new(173));
        let clock = Arc::new(TestClock(AtomicU64::new(1_800_000_200_000)));
        let sink = Arc::new(ScriptedSink::new([DeliveryBehavior::Failure(
            Retryability::Retryable,
        )]));
        let engine = Engine::builder(
            storage.clone(),
            clock.clone(),
            Arc::new(TestIds(AtomicU64::new(10))),
            DeadlinePolicy::new(10_000, 10_000, 10_000).unwrap(),
        )
        .sink(sink.clone())
        .signer(Arc::new(MockSigner::new(SignBehavior::Success {
            completed_at_unix_ms: 1_800_000_200_500,
        })))
        .build()
        .unwrap();
        let push = request(173, "wss://relay.example");
        execute_to_admitted(&engine, &push);
        let first = block_on(engine.deliver_push(push.operation_id())).unwrap();
        clock.0.store(
            first.plan().retry().unwrap().not_before_unix_ms(),
            Ordering::Relaxed,
        );
        *storage.receipt_mutation.lock().unwrap() = Some(mutation);
        assert_eq!(
            block_on(engine.deliver_push(push.operation_id())),
            Err(Error::StorageFailed),
            "{mutation:?}"
        );
        assert_eq!(sink.requests.lock().unwrap().len(), 1);
    }
}

#[derive(Clone, Copy)]
pub(super) enum HistoryMutation {
    Plan,
    Artifact,
    MissingFact,
    MissingStop,
}
pub(super) fn mutate_history(
    history: AuthoredDeliveryHistory,
    mutation: HistoryMutation,
) -> AuthoredDeliveryHistory {
    let mut wire = serde_json::to_value(history.plan()).unwrap();
    match mutation {
        HistoryMutation::Plan => wire["plan_id"] = serde_json::json!(vec![244; 16]),
        HistoryMutation::Artifact => wire["artifact_id"] = serde_json::json!(vec![245; 16]),
        HistoryMutation::MissingFact => wire["delivery_facts"] = serde_json::json!([]),
        HistoryMutation::MissingStop => {
            wire["stop_requested_at_unix_ms"] = serde_json::Value::Null;
            wire["state"] = serde_json::json!("pending");
        }
    }
    AuthoredDeliveryHistory::new(serde_json::from_value(wire).unwrap(), None).unwrap()
}

#[test]
fn wrong_history_and_lost_committed_fact_or_stop_never_become_success() {
    for (nth, mutation, stop) in [
        (1, HistoryMutation::Plan, false),
        (1, HistoryMutation::Artifact, false),
        (2, HistoryMutation::Plan, false),
        (3, HistoryMutation::MissingFact, false),
        (2, HistoryMutation::MissingStop, true),
    ] {
        let storage = Arc::new(FaultStorage::new(174));
        let sink = Arc::new(ScriptedSink::new([DeliveryBehavior::Outcomes(vec![
            DeliveryOutcome::accepted(),
        ])]));
        let engine = fault_engine(
            storage.clone(),
            Arc::new(MockSigner::new(SignBehavior::Success {
                completed_at_unix_ms: 1_800_000_200_500,
            })),
            sink.clone(),
        );
        let push = request(174, "wss://relay.example");
        execute_to_admitted(&engine, &push);
        *storage.history_mutation.lock().unwrap() = Some((nth, mutation));
        if stop {
            assert_eq!(
                block_on(engine.cancel_push(push.operation_id())),
                Err(Error::StorageFailed)
            );
        } else {
            assert_eq!(
                block_on(engine.deliver_push(push.operation_id())),
                Err(Error::StorageFailed)
            );
        }
        let actual = block_on(engine.push_status(push.operation_id()))
            .unwrap()
            .unwrap();
        assert_eq!(
            actual.delivery_plan().delivery_facts().len(),
            usize::from(nth == 3)
        );
    }
}

#[test]
fn clock_expiry_before_sink_and_future_fact_before_reconciliation_fail_closed() {
    struct ExpiringClock(AtomicUsize);
    impl Clock for ExpiringClock {
        fn now_unix_ms(&self) -> Result<u64, Error> {
            Ok(1_800_000_220_000 + 10_000 * self.0.fetch_add(1, Ordering::Relaxed) as u64)
        }
    }
    let (engine, storage, _, sink, push) = setup(175);
    let boundary = Engine::builder(
        storage,
        Arc::new(ExpiringClock(AtomicUsize::new(0))),
        Arc::new(TestIds(AtomicU64::new(230))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).unwrap(),
    )
    .sink(sink.clone())
    .build()
    .unwrap();
    assert_eq!(
        block_on(boundary.deliver_push(push.operation_id())),
        Err(Error::WorkClaimConflict)
    );
    assert_eq!(sink.calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        block_on(engine.deliver_push(SyncId::new([231; 16]).unwrap())),
        Err(Error::StorageFailed)
    );
    let (engine, storage, clock, sink, push) = setup(176);
    let mut future = Box::pin(engine.deliver_push(push.operation_id()));
    assert!(
        future
            .poll_unpin(&mut std::task::Context::from_waker(noop_waker_ref()))
            .is_pending()
    );
    let (request, _sender) = sink.take();
    drop(future);
    let status = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    let plan = status.delivery_plan();
    let claim = plan.claim_evidence().unwrap();
    let at = claim.expires_at_unix_ms();
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::RecordDelivery(
            RecordDeliveryFact::new(
                plan.plan_id(),
                plan.artifact_id(),
                claim.clone(),
                DeliveryAttemptOutcome::Receipt(
                    receipt(&request, vec![DeliveryOutcome::accepted()]).unwrap(),
                ),
                at + 1000,
            )
            .unwrap(),
        )),
    )
    .unwrap();
    clock.0.store(at, Ordering::Relaxed);
    assert_eq!(
        block_on(engine.deliver_push(push.operation_id())),
        Err(Error::ClockUnavailable)
    );
    let current = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    assert_eq!(current.delivery_plan().attempt_count(), 0);
    assert_eq!(current.delivery_plan().delivery_facts().len(), 1);
}

#[test]
fn unbound_preparation_and_stop_keep_honest_retry_and_no_issued_proof() {
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let (engine, _) = setup_engine(signer);
    let push = request(177, "wss://relay.example");
    block_on(engine.prepare_push(push.clone())).unwrap();
    let status = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    assert!(status.delivery_plan().request().is_none());
    assert!(status.delivery_history().proves_no_issued_attempt());
    assert_eq!(
        engine
            .retry_decision(status.delivery_plan(), 1_800_000_200_001)
            .unwrap(),
        SyncRetryDecision::Ready
    );
    let stopped = block_on(engine.cancel_push(push.operation_id())).unwrap();
    assert!(
        stopped
            .status()
            .delivery_history()
            .proves_no_issued_attempt()
    );
    assert_eq!(
        engine
            .retry_decision(stopped.status().delivery_plan(), 1_800_000_200_010)
            .unwrap(),
        SyncRetryDecision::Exhausted
    );
}
