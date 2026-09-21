use super::*;
use futures::channel::oneshot;
use radroots_storage::authored_delivery::DeliveryAttemptOutcome;
use radroots_transport::policy::SatisfactionState;

type Pending = (
    DeliveryRequest,
    TargetSet,
    oneshot::Sender<Result<DeliveryReceipt, SinkFailure>>,
);

#[derive(Default)]
struct SelectedSink {
    pending: Mutex<VecDeque<Pending>>,
    calls: AtomicUsize,
}

impl EventSink for SelectedSink {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SinkStatus, TransportError>> {
        Box::pin(async { panic!("no status probe") })
    }
    fn deliver(
        &self,
        _: DeliveryRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<DeliveryReceipt, SinkFailure>> {
        Box::pin(async { panic!("selected attempts must not fall back") })
    }
    fn deliver_selected(
        &self,
        request: DeliveryRequest,
        selected: TargetSet,
    ) -> radroots_transport::BoxFuture<'_, Result<DeliveryReceipt, SinkFailure>> {
        Box::pin(async move {
            request.validate_target_selection(&selected).unwrap();
            self.calls.fetch_add(1, Ordering::SeqCst);
            let (send, receive) = oneshot::channel();
            self.pending
                .lock()
                .unwrap()
                .push_back((request, selected, send));
            receive.await.unwrap()
        })
    }
}

fn selected_receipt(request: &DeliveryRequest, selected: &TargetSet) -> DeliveryReceipt {
    DeliveryReceipt::for_request(
        request,
        request
            .target_set()
            .targets()
            .iter()
            .map(|target| {
                if selected.targets().contains(target) {
                    DeliveryTargetReceipt::attempted(target.clone(), DeliveryOutcome::accepted())
                } else {
                    DeliveryTargetReceipt::skipped(target.clone(), DeliveryOutcome::unavailable())
                        .unwrap()
                }
            })
            .collect(),
    )
    .unwrap()
}

fn poll_pending(future: &mut (impl std::future::Future + Unpin)) {
    assert!(
        future
            .poll_unpin(&mut std::task::Context::from_waker(noop_waker_ref()))
            .is_pending()
    );
}

fn engine(storage: Arc<dyn SyncStorage>, clock: Arc<TestClock>, sink: Arc<SelectedSink>) -> Engine {
    Engine::builder(
        storage,
        clock,
        Arc::new(TestIds(AtomicU64::new(10))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).unwrap(),
    )
    .sink(sink)
    .signer(Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    })))
    .build()
    .unwrap()
}

fn push() -> PushRequest {
    request_with_policy(
        211,
        &["wss://one.example", "wss://two.example"],
        SatisfactionClass::Accepted,
        TargetPolicy::all(),
    )
}

#[test]
fn invalid_selection_cannot_claim_and_out_of_selection_results_fail_closed() {
    for failure in [false, true] {
        let storage = Arc::new(MemoryStorage::new(
            SourceGeneration::new([211; 32]).unwrap(),
        ));
        let clock = Arc::new(TestClock(AtomicU64::new(1_800_000_200_000)));
        let sink = Arc::new(SelectedSink::default());
        let engine = engine(storage, clock, sink.clone());
        let push = push();
        execute_to_admitted(&engine, &push);
        let before = block_on(engine.push_status(push.operation_id()))
            .unwrap()
            .unwrap();
        let original = before.delivery_plan().request().unwrap();
        let foreign =
            TargetSet::new(vec![Target::nostr_relay("wss://foreign.example").unwrap()]).unwrap();
        assert!(matches!(
            block_on(engine.deliver_push_selected(push.operation_id(), foreign)),
            Err(Error::InvalidDeliveryRequest)
        ));
        let unchanged = block_on(engine.push_status(push.operation_id()))
            .unwrap()
            .unwrap();
        assert_eq!(unchanged.delivery_plan(), before.delivery_plan());
        assert_eq!(sink.calls.load(Ordering::SeqCst), 0);
        let selected = TargetSet::new(vec![original.target_set().targets()[0].clone()]).unwrap();
        let mut future = Box::pin(engine.deliver_push_selected(push.operation_id(), selected));
        poll_pending(&mut future);
        let (request, _, sender) = sink.pending.lock().unwrap().pop_front().unwrap();
        assert_eq!(&request, original);
        let forbidden = DeliveryTargetReceipt::attempted(
            request.target_set().targets()[1].clone(),
            DeliveryOutcome::accepted(),
        );
        let result = if failure {
            Err(SinkFailure::for_request(
                &request,
                "upstream_failure",
                Retryability::Retryable,
                None,
                None,
                vec![forbidden],
            )
            .unwrap())
        } else {
            Ok(receipt(&request, vec![DeliveryOutcome::accepted(); 2]).unwrap())
        };
        sender.send(result).unwrap();
        let result = block_on(future).unwrap();
        assert_eq!(result.plan().request(), Some(original));
        let DeliveryAttemptOutcome::SinkFailure(failure) =
            result.plan().delivery_facts()[0].outcome()
        else {
            panic!("invalid adapter evidence is never acceptance")
        };
        assert_eq!(failure.code(), "invalid_transport_contract");
        assert!(failure.partial_evidence().is_empty());
        assert_ne!(
            result.plan().delivery_satisfaction().unwrap(),
            SatisfactionState::Satisfied
        );
    }
}

#[test]
fn selected_late_acceptance_survives_stop_without_scheduling_new_targets() {
    let storage = Arc::new(MemoryStorage::new(
        SourceGeneration::new([212; 32]).unwrap(),
    ));
    let clock = Arc::new(TestClock(AtomicU64::new(1_800_000_200_000)));
    let sink = Arc::new(SelectedSink::default());
    let engine = engine(storage.clone(), clock.clone(), sink.clone());
    let push = push();
    execute_to_admitted(&engine, &push);
    let before = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    let request = before.delivery_plan().request().unwrap();
    let selected = TargetSet::new(vec![request.target_set().targets()[0].clone()]).unwrap();
    let mut future = Box::pin(engine.deliver_push_selected(push.operation_id(), selected.clone()));
    poll_pending(&mut future);
    block_on(engine.cancel_push(push.operation_id())).unwrap();
    let (captured, captured_selection, sender) = sink.pending.lock().unwrap().pop_front().unwrap();
    assert_eq!(&captured, request);
    assert_eq!(captured_selection, selected);
    let expected = selected_receipt(&captured, &selected);
    sender.send(Ok(expected.clone())).unwrap();
    let late = block_on(future).unwrap();
    assert_eq!(late.plan().state(), AuthoredDeliveryState::Cancelled);
    assert_eq!(late.plan().attempt_count(), 0);
    assert_eq!(
        late.plan().delivery_facts()[0].outcome(),
        &DeliveryAttemptOutcome::Receipt(expected)
    );
    let recovery = delivery_evidence::source_only(storage, clock);
    let replay = block_on(recovery.deliver_push_selected(push.operation_id(), selected)).unwrap();
    assert!(replay.is_replay());
    assert_eq!(replay.plan(), late.plan());
    assert_eq!(sink.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sqlite_selected_facts_survive_reopen_and_next_target_finishes_same_request() {
    let directory = tempfile::tempdir().unwrap();
    let paths = Paths::from_directory(directory.path()).unwrap();
    let storage = Arc::new(
        SqliteStorage::open(
            OpenOptions::new(paths.clone(), OpenMode::Create)
                .with_source_generation(SourceGeneration::new([213; 32]).unwrap(), 1)
                .unwrap(),
        )
        .await
        .unwrap(),
    );
    let clock = Arc::new(TestClock(AtomicU64::new(1_800_000_200_000)));
    let sink = Arc::new(SelectedSink::default());
    let first_engine = engine(storage.clone(), clock.clone(), sink.clone());
    let push = push();
    first_engine.sign_prepared(push.clone()).await.unwrap();
    first_engine
        .admit_signed(push.operation_id())
        .await
        .unwrap();
    let before = first_engine
        .push_status(push.operation_id())
        .await
        .unwrap()
        .unwrap();
    let original = before.delivery_plan().request().unwrap().clone();
    let selected_a = TargetSet::new(vec![original.target_set().targets()[0].clone()]).unwrap();
    let first = {
        let mut future =
            Box::pin(first_engine.deliver_push_selected(push.operation_id(), selected_a.clone()));
        // SQLite storage needs an executor turn before the sink is reached.
        let responder = async {
            let pending = loop {
                if let Some(pending) = sink.pending.lock().unwrap().pop_front() {
                    break pending;
                }
                tokio::task::yield_now().await;
            };
            assert_eq!(pending.0, original);
            assert_eq!(pending.1, selected_a);
            pending
                .2
                .send(Ok(selected_receipt(&pending.0, &pending.1)))
                .unwrap();
        };
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let (result, ()) = tokio::join!(&mut future, responder);
            result.unwrap()
        })
        .await
        .unwrap()
    };
    assert_ne!(first.plan().state(), AuthoredDeliveryState::Satisfied);
    assert_eq!(first.plan().attempt_count(), 1);
    clock.0.store(
        first.plan().retry().unwrap().not_before_unix_ms(),
        Ordering::SeqCst,
    );
    drop(first_engine);
    storage.close().await.unwrap();
    let storage = Arc::new(
        SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadWriteExisting))
            .await
            .unwrap(),
    );
    let second_engine = engine(storage.clone(), clock.clone(), sink.clone());
    let recovered = second_engine
        .push_status(push.operation_id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        recovered.delivery_plan().delivery_facts(),
        first.plan().delivery_facts()
    );
    let selected_b = TargetSet::new(vec![original.target_set().targets()[1].clone()]).unwrap();
    let responder = async {
        let pending = loop {
            if let Some(pending) = sink.pending.lock().unwrap().pop_front() {
                break pending;
            }
            tokio::task::yield_now().await;
        };
        assert_eq!(pending.0, original);
        assert_eq!(pending.1, selected_b);
        pending
            .2
            .send(Ok(selected_receipt(&pending.0, &pending.1)))
            .unwrap();
    };
    let finished = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let (result, ()) = tokio::join!(
            second_engine.deliver_push_selected(push.operation_id(), selected_b.clone()),
            responder
        );
        result.unwrap()
    })
    .await
    .unwrap();
    assert_eq!(finished.plan().request(), Some(&original));
    assert_eq!(finished.plan().state(), AuthoredDeliveryState::Satisfied);
    assert_eq!(finished.plan().attempt_count(), 2);
    assert_eq!(finished.plan().delivery_facts().len(), 2);
    assert_eq!(sink.calls.load(Ordering::SeqCst), 2);
    drop(second_engine);
    storage.close().await.unwrap();
}
