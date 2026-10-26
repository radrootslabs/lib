//! Controlled owner/lifecycle tests, not signature or SQLite qualification.
use super::*;
use std::sync::atomic::AtomicUsize;

#[derive(Default)]
pub(super) struct Control {
    pub(super) reconciliations: AtomicUsize,
    pub(super) queries: AtomicUsize,
    mode: AtomicUsize,
    calls: Mutex<Vec<(OperationId, CommandDigest, SigningAttemptId, String)>>,
}
pub(super) fn capture<'a>(
    store: &'a Store,
    request: &'a StockEvidenceRequest,
    credential: &'a AttachmentProof,
) -> radroots_storage::event::BoxFuture<'a, Result<StockEvidenceOutcome, OperationError>> {
    Box::pin(async move {
        assert_eq!(credential.with_secret(|value| *value), [7; 32]);
        assert_eq!(request.binding().input(), &input(5));
        store.capture.calls.lock().unwrap().push((
            request.operation_id(),
            request.command_digest(),
            request.signing_attempt_id(),
            request.with_json(str::to_owned),
        ));
        poll_fn(|cx| {
            store.waker.register(cx.waker());
            if store.waiting.load(Ordering::SeqCst) {
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;
        let mode = store.capture.mode.load(Ordering::SeqCst);
        let original = request.attempt();
        if mode == 2 {
            return Err(OperationError::StorageFailure);
        }
        if mode == 0 {
            return Ok(StockEvidenceOutcome::Unknown(original));
        }
        let anchor = radroots_storage::command::CommitAttempt::new(
            original.workspace_id(),
            if mode == 3 || mode == 4 {
                OperationId::from_bytes([123; 32])
            } else {
                original.operation_id()
            },
            original.command_digest(),
            Some(TransactionId::from_bytes([124; 32])),
        );
        if mode == 4 {
            return Ok(StockEvidenceOutcome::Unknown(anchor));
        }
        let receipt = StockEvidenceReceipt::from_committed_store(
            anchor,
            if mode == 6 {
                SigningAttemptId::from_bytes([125; 32])
            } else {
                request.signing_attempt_id()
            },
            request.recipient(),
            if mode == 5 {
                [126; 32]
            } else {
                request.evidence_digest()
            },
            request.observed_at_unix_ms(),
        )?;
        Ok(StockEvidenceOutcome::Committed(receipt))
    })
}
fn selected_store() -> Shared<Store> {
    let store = store();
    *store.model.binding.lock().unwrap() = WorkspaceBinding::from_authenticated_host(
        input(5),
        radroots_storage::workspace::WorkspaceScope::SellerStockOwner,
        ActorRef::new(key(), ActorRole::Seller, None, Some(input(5).grant_id())),
        "device",
        Sequence::new(1).unwrap(),
        UnixSeconds::new(200).unwrap(),
        UnixSeconds::new(100).unwrap(),
    )
    .unwrap();
    store
}
fn request(scope: &WorkspaceSession, op: u8) -> StockEvidenceRequest {
    StockEvidenceRequest::from_obtained_wire(
        scope.binding().clone(),
        OperationId::from_bytes([op; 32]),
        digest(),
        SigningAttemptId::from_bytes([83; 32]),
        key(),
        [84; 32],
        "synthetic retained material; this controlled port authenticates no signature",
        None,
    )
    .unwrap()
}
fn creation() -> radroots_protocol::command::CreatePoolInput {
    use radroots_domain::{listing::MeasurementUnit, stock::PoolUnit};
    radroots_protocol::command::CreatePoolInput::new(
        PoolId::from_bytes([85; 32]),
        "synthetic inventory",
        PoolUnit::new(MeasurementUnit::Each, 0).unwrap(),
        Quantity::new("1".parse().unwrap(), MeasurementUnit::Each).unwrap(),
    )
    .unwrap()
}
fn context(scope: &WorkspaceSession) -> radroots_execution::workspace::CommandContext {
    scope
        .command_context(
            OperationId::from_bytes([86; 32]),
            None,
            None,
            UnixSeconds::new(150).unwrap(),
        )
        .unwrap()
}
#[derive(Clone)]
struct Lifecycle(Shared<Mutex<Vec<&'static str>>>, Option<Shared<Tasks>>);
impl Lifecycle {
    fn record(&self, event: &'static str) -> HostFuture<'_, radroots_execution::Result<()>> {
        Box::pin(async move {
            self.0.lock().unwrap().push(event);
            Ok(())
        })
    }
}
impl SessionHost for Lifecycle {
    fn start(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        self.record("start")
    }
    fn quiesce(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        self.record("quiesce")
    }
    fn drain(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        Box::pin(async move {
            self.0.lock().unwrap().push("drain");
            if let Some(tasks) = &self.1 {
                let jobs = core::mem::take(&mut *tasks.queued.lock().unwrap());
                for job in jobs {
                    job.await;
                }
            }
            Ok(())
        })
    }
    fn disconnect(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        self.record("disconnect")
    }
    fn dispose_callbacks(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        self.record("callbacks")
    }
    fn close_storage(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        self.record("storage")
    }
}

#[test]
fn unknown_capture_retains_material_after_observer_loss_blocks_fresh_work_and_closes_only_after_exact_retry()
 {
    block_on(async {
        let store = selected_store();
        let tasks = Shared::new(Tasks::default());
        let trace = Shared::new(Mutex::new(Vec::new()));
        let client = ClientSession::new_with_task_host(
            Lifecycle(trace.clone(), None),
            core::num::NonZeroU16::new(8).unwrap(),
            tasks.clone(),
        );
        let scope = client
            .attach_workspace(store.clone(), input(5), proof())
            .await
            .unwrap();
        client.start().await.unwrap();
        let original = request(&scope, 90);
        let original_wire = original.with_json(str::to_owned);
        let mut observed =
            Box::pin(client.capture_stock_evidence(store.clone(), &scope, &original));
        assert!(futures::poll!(observed.as_mut()).is_pending());
        // A mutation admitted earlier must recheck before its actual store call.
        let mut queued_mutation = Box::pin(client.create_stock_pool(
            store.clone(),
            &scope,
            context(&scope),
            minimum(&scope),
            creation(),
        ));
        assert!(futures::poll!(queued_mutation.as_mut()).is_pending());
        assert_eq!(
            client
                .capture_stock_evidence(store.clone(), &scope, &original)
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
        drop(observed);
        drop(original);
        let jobs = core::mem::take(&mut *tasks.queued.lock().unwrap());
        for job in jobs {
            job.await;
        }
        assert_eq!(
            queued_mutation.await.unwrap_err().code(),
            Code::RecoveryRequired
        );
        assert_eq!(
            client
                .create_stock_pool(
                    store.clone(),
                    &scope,
                    context(&scope),
                    minimum(&scope),
                    creation()
                )
                .await
                .unwrap_err()
                .code(),
            Code::RecoveryRequired
        );
        assert!(store.calls.lock().unwrap().is_empty());
        let mut read = Box::pin(client.stock_pool(
            store.clone(),
            &scope,
            minimum(&scope),
            PoolId::from_bytes([85; 32]),
        ));
        assert!(futures::poll!(read.as_mut()).is_pending());
        let job = tasks.queued.lock().unwrap().pop().unwrap();
        job.await;
        assert!(read.await.unwrap().is_none());
        assert_eq!(store.capture.queries.load(Ordering::SeqCst), 1);
        assert_eq!(
            client.close().await.unwrap_err().code(),
            Code::RecoveryRequired
        );
        assert_eq!(*trace.lock().unwrap(), vec!["start", "quiesce", "drain"]);
        store.capture.mode.store(1, Ordering::SeqCst);
        client.close().await.unwrap();
        assert_eq!(
            *trace.lock().unwrap(),
            vec![
                "start",
                "quiesce",
                "drain",
                "disconnect",
                "callbacks",
                "storage"
            ]
        );
        let calls = store.capture.calls.lock().unwrap();
        assert!(calls.len() >= 3);
        assert!(store.capture.reconciliations.load(Ordering::SeqCst) >= 2);
        assert!(calls.iter().all(|value| value == &calls[0]));
        assert_eq!(calls[0].0, OperationId::from_bytes([90; 32]));
        assert_eq!(calls[0].3, original_wire);
    });
}

#[test]
fn storage_errors_and_foreign_receipts_keep_original_capture_until_a_matching_confirmed_append() {
    block_on(async {
        let store = selected_store();
        let tasks = Shared::new(Tasks::default());
        let client = ClientSession::new_with_task_host(
            Host,
            core::num::NonZeroU16::new(8).unwrap(),
            tasks.clone(),
        );
        let scope = client
            .attach_workspace(store.clone(), input(5), proof())
            .await
            .unwrap();
        client.start().await.unwrap();
        let original = request(&scope, 91);
        let mut observed =
            Box::pin(client.capture_stock_evidence(store.clone(), &scope, &original));
        assert!(futures::poll!(observed.as_mut()).is_pending());
        let job = tasks.queued.lock().unwrap().pop().unwrap();
        job.await;
        assert!(matches!(
            observed.await.unwrap(),
            StockEvidenceOutcome::Unknown(_)
        ));
        for mode in 2..=6 {
            store.capture.mode.store(mode, Ordering::SeqCst);
            let error = client.drain_stock_evidence().await.unwrap_err();
            assert_eq!(
                error.code(),
                if mode == 2 {
                    Code::StorageFailure
                } else {
                    Code::RecoveryRequired
                }
            );
            assert_eq!(error.retry(), RetryClass::ManualReconciliation);
            assert_eq!(
                client
                    .create_stock_pool(
                        store.clone(),
                        &scope,
                        context(&scope),
                        minimum(&scope),
                        creation()
                    )
                    .await
                    .unwrap_err()
                    .code(),
                Code::RecoveryRequired
            );
        }
        store.capture.mode.store(1, Ordering::SeqCst);
        client.drain_stock_evidence().await.unwrap();
        assert_eq!(store.capture.reconciliations.load(Ordering::SeqCst), 6);
        {
            let calls = store.capture.calls.lock().unwrap();
            assert!(calls.iter().all(|value| value == &calls[0]));
        }
        client.close().await.unwrap();
        original.with_json(|raw| assert!(raw.contains("synthetic retained material")));
    });
}

#[test]
fn unpolled_foreign_closed_missing_and_refused_hosts_leave_original_input_and_execute_no_capture() {
    block_on(async {
        let store = selected_store();
        let tasks = Shared::new(Tasks::default());
        let client = ClientSession::new_with_task_host(
            Host,
            core::num::NonZeroU16::new(8).unwrap(),
            tasks.clone(),
        );
        let scope = client
            .attach_workspace(store.clone(), input(5), proof())
            .await
            .unwrap();
        client.start().await.unwrap();
        let original = request(&scope, 92);
        drop(client.capture_stock_evidence(store.clone(), &scope, &original));
        assert!(tasks.queued.lock().unwrap().is_empty());
        let foreign = WorkspaceSession::attach(store.clone(), input(5), proof())
            .await
            .unwrap();
        assert_eq!(
            client
                .capture_stock_evidence(store.clone(), &foreign, &original)
                .await
                .unwrap_err()
                .code(),
            Code::Unauthorized
        );
        for selected in [
            input(9),
            radroots_storage::workspace::seller_stock::SellerAuthoritySelection::from_attachment(
                &input(5),
            )
            .attachment(GrantId::from_bytes([99; 32]))
            .unwrap(),
        ] {
            let actor = ActorRef::new(key(), ActorRole::Seller, None, Some(selected.grant_id()));
            let binding = WorkspaceBinding::from_authenticated_host(
                selected,
                radroots_storage::workspace::WorkspaceScope::SellerStockOwner,
                actor,
                "device",
                Sequence::new(1).unwrap(),
                UnixSeconds::new(200).unwrap(),
                UnixSeconds::new(100).unwrap(),
            )
            .unwrap();
            let forged = StockEvidenceRequest::from_obtained_wire(
                binding,
                original.operation_id(),
                original.command_digest(),
                original.signing_attempt_id(),
                original.recipient(),
                *original.intent_digest(),
                "synthetic foreign descriptor",
                None,
            )
            .unwrap();
            assert_eq!(
                client
                    .capture_stock_evidence(store.clone(), &scope, &forged)
                    .await
                    .unwrap_err()
                    .code(),
                Code::Unauthorized
            );
        }
        tasks.refuse.store(true, Ordering::SeqCst);
        assert_eq!(
            client
                .capture_stock_evidence(store.clone(), &scope, &original)
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
        tasks.refuse.store(false, Ordering::SeqCst);
        scope.close();
        assert!(
            client
                .capture_stock_evidence(store.clone(), &scope, &original)
                .await
                .is_err()
        );
        client.close().await.unwrap();
        let no_host = ClientSession::new(Host, core::num::NonZeroU16::new(8).unwrap());
        let scope = no_host
            .attach_workspace(store.clone(), input(5), proof())
            .await
            .unwrap();
        no_host.start().await.unwrap();
        assert!(
            no_host
                .capture_stock_evidence(store.clone(), &scope, &original)
                .await
                .is_err()
        );
        no_host.close().await.unwrap();
        assert!(store.capture.calls.lock().unwrap().is_empty());
        original.with_json(|raw| assert!(raw.contains("synthetic retained material")));
    });
}

#[test]
fn close_waits_for_the_active_capture_and_keeps_original_material_after_the_observer_is_dropped() {
    block_on(async {
        let store = selected_store();
        store.capture.mode.store(1, Ordering::SeqCst);
        store.waiting.store(true, Ordering::SeqCst);
        let tasks = Shared::new(Tasks::default());
        let trace = Shared::new(Mutex::new(Vec::new()));
        let client = ClientSession::new_with_task_host(
            Lifecycle(trace.clone(), None),
            core::num::NonZeroU16::new(8).unwrap(),
            tasks.clone(),
        );
        let scope = client
            .attach_workspace(store.clone(), input(5), proof())
            .await
            .unwrap();
        client.start().await.unwrap();
        let original = request(&scope, 93);
        let mut observed =
            Box::pin(client.capture_stock_evidence(store.clone(), &scope, &original));
        assert!(futures::poll!(observed.as_mut()).is_pending());
        let mut job = tasks.queued.lock().unwrap().pop().unwrap();
        assert!(futures::poll!(job.as_mut()).is_pending());
        drop(observed);
        drop(original);
        let mut closing = Box::pin(client.close());
        assert!(futures::poll!(closing.as_mut()).is_pending());
        assert_eq!(*trace.lock().unwrap(), vec!["start", "quiesce", "drain"]);
        store.waiting.store(false, Ordering::SeqCst);
        store.waker.wake();
        job.await;
        closing.await.unwrap();
        assert_eq!(
            *trace.lock().unwrap(),
            vec![
                "start",
                "quiesce",
                "drain",
                "disconnect",
                "callbacks",
                "storage"
            ]
        );
        assert_eq!(store.capture.calls.lock().unwrap().len(), 1);
    });
}

#[test]
fn close_drives_the_host_owned_active_task_before_retrying_its_retained_capture() {
    block_on(async {
        let store = selected_store();
        store.capture.mode.store(1, Ordering::SeqCst);
        store.waiting.store(true, Ordering::SeqCst);
        let tasks = Shared::new(Tasks::default());
        let trace = Shared::new(Mutex::new(Vec::new()));
        let client = ClientSession::new_with_task_host(
            Lifecycle(trace.clone(), Some(tasks.clone())),
            core::num::NonZeroU16::new(8).unwrap(),
            tasks.clone(),
        );
        let scope = client
            .attach_workspace(store.clone(), input(5), proof())
            .await
            .unwrap();
        client.start().await.unwrap();
        let original = request(&scope, 94);
        let mut observed =
            Box::pin(client.capture_stock_evidence(store.clone(), &scope, &original));
        assert!(futures::poll!(observed.as_mut()).is_pending());
        let mut job = tasks.queued.lock().unwrap().pop().unwrap();
        assert!(futures::poll!(job.as_mut()).is_pending());
        tasks.queued.lock().unwrap().push(job);
        drop(observed);
        drop(original);
        let mut closing = Box::pin(client.close());
        assert!(futures::poll!(closing.as_mut()).is_pending());
        assert_eq!(*trace.lock().unwrap(), vec!["start", "quiesce", "drain"]);
        store.waiting.store(false, Ordering::SeqCst);
        store.waker.wake();
        assert!(matches!(
            futures::poll!(closing.as_mut()),
            Poll::Ready(Ok(()))
        ));
        assert_eq!(
            *trace.lock().unwrap(),
            vec![
                "start",
                "quiesce",
                "drain",
                "disconnect",
                "callbacks",
                "storage"
            ]
        );
        assert_eq!(store.capture.calls.lock().unwrap().len(), 1);
    });
}
