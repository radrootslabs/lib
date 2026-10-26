//! Mechanical task ownership with a controlled port, not SQL qualification.
use super::*;
use radroots_execution::{Code, RetryClass, session::tasks::TaskHost};
use radroots_storage::{command::OperationError, inventory::*};
#[path = "workspace_acceptance.rs"]
mod acceptance;
#[path = "workspace_evidence.rs"]
mod evidence;

#[derive(Default)]
struct Tasks {
    queued: Mutex<Vec<HostFuture<'static, ()>>>,
    refuse: AtomicBool,
}
impl TaskHost for Tasks {
    fn submit(&self, task: HostFuture<'static, ()>) -> radroots_execution::Result<()> {
        if self.refuse.load(Ordering::SeqCst) {
            return Err(radroots_execution::Error::operation(
                OperationError::UnsupportedOperation,
                radroots_execution::OperationContext::proposed(None),
            ));
        }
        let mut queued = self.queued.lock().unwrap();
        assert!(
            queued.len() < 2,
            "the owner's byte budget must bound admission first"
        );
        queued.push(task);
        Ok(())
    }
}
struct Store {
    capture: evidence::Control,
    model: Shared<Model>,
    calls: Mutex<Vec<(OperationId, CommandDigest)>>,
    waiting: AtomicBool,
    waker: AtomicWaker,
}
impl WorkspaceStore for Store {
    fn authenticate(
        &self,
        input: AttachmentInput,
        proof: AttachmentProof,
    ) -> radroots_storage::event::BoxFuture<'_, Result<WorkspaceBinding, WorkspaceError>> {
        self.model.authenticate(input, proof)
    }
    fn close_workspace(
        &self,
    ) -> radroots_storage::event::BoxFuture<'_, Result<(), WorkspaceError>> {
        Box::pin(async { Ok(()) })
    }
}
impl InventoryStore for Store {
    fn reconcile_stock_evidence<'a>(
        &'a self,
        request: &'a StockEvidenceRequest,
        proof: &'a AttachmentProof,
    ) -> radroots_storage::event::BoxFuture<'a, Result<StockEvidenceOutcome, OperationError>> {
        self.capture.reconciliations.fetch_add(1, Ordering::SeqCst);
        evidence::capture(self, request, proof)
    }
    fn capture_stock_evidence<'a>(
        &'a self,
        request: &'a StockEvidenceRequest,
        proof: &'a AttachmentProof,
    ) -> radroots_storage::event::BoxFuture<'a, Result<StockEvidenceOutcome, OperationError>> {
        evidence::capture(self, request, proof)
    }
    fn prepare_stock_acceptance(
        &self,
        request: StockAcceptanceRequest,
        proof: AttachmentProof,
    ) -> radroots_storage::event::BoxFuture<'_, Result<StockAcceptanceOutcome, OperationError>>
    {
        Box::pin(async move {
            assert_eq!(proof.with_secret(|bytes| *bytes), [7; 32]);
            assert_eq!(request.binding().input(), &input(5));
            assert_eq!(request.minimum_checkpoint().revision().get(), 0);
            assert_eq!(request.source_checkpoint().revision().get(), 2);
            assert_eq!(request.listing_checkpoint().revision().get(), 3);
            self.calls
                .lock()
                .unwrap()
                .push((request.attempt().operation_id(), request.command().digest()));
            poll_fn(|cx| {
                self.waker.register(cx.waker());
                if self.waiting.load(Ordering::SeqCst) {
                    Poll::Pending
                } else {
                    Poll::Ready(())
                }
            })
            .await;
            // Port mechanics only; this reports no actual T0 or SQL effect.
            Ok(StockAcceptanceOutcome::Unknown(request.attempt()))
        })
    }
    fn abort_proven_unissued(
        &self,
        request: StockAbortRequest,
        proof: AttachmentProof,
    ) -> radroots_storage::event::BoxFuture<'_, Result<StockAbortOutcome, OperationError>> {
        Box::pin(async move {
            assert_eq!(proof.with_secret(|bytes| *bytes), [7; 32]);
            assert_eq!(request.binding().input(), &input(5));
            self.calls
                .lock()
                .unwrap()
                .push((request.operation_id(), request.command_digest()));
            poll_fn(|cx| {
                self.waker.register(cx.waker());
                if self.waiting.load(Ordering::SeqCst) {
                    Poll::Pending
                } else {
                    Poll::Ready(())
                }
            })
            .await;
            // A mechanical report only. No absence, release or SQL proof.
            Ok(StockAbortOutcome::Unknown(request.attempt()))
        })
    }
    fn stock_preparation_abort(
        &self,
        _request: StockAbortRequest,
        _proof: AttachmentProof,
    ) -> radroots_storage::event::BoxFuture<'_, Result<Option<StockAbortReceipt>, OperationError>>
    {
        Box::pin(async { Err(OperationError::MissingHistory) })
    }
    fn option_mapping(
        &self,
        _binding: WorkspaceBinding,
        _minimum: InventoryCheckpointRef,
        _key: radroots_domain::stock::OptionMappingKey,
        _proof: AttachmentProof,
    ) -> radroots_storage::event::BoxFuture<
        '_,
        Result<Option<radroots_domain::stock::OptionPoolMapping>, OperationError>,
    > {
        panic!("unexpected port call")
    }
    fn execute_inventory(
        &self,
        _request: InventoryRequest,
        _proof: AttachmentProof,
    ) -> radroots_storage::event::BoxFuture<'_, Result<InventoryOutcome, OperationError>> {
        panic!("unexpected port call")
    }
    fn inventory_operation(
        &self,
        _binding: WorkspaceBinding,
        _minimum: InventoryCheckpointRef,
        _operation: OperationId,
        _digest: Option<CommandDigest>,
        _proof: AttachmentProof,
    ) -> radroots_storage::event::BoxFuture<'_, Result<Option<InventoryReceipt>, OperationError>>
    {
        panic!("unexpected port call")
    }
    fn pool(
        &self,
        _binding: WorkspaceBinding,
        _minimum: InventoryCheckpointRef,
        _pool: PoolId,
        _proof: AttachmentProof,
    ) -> radroots_storage::event::BoxFuture<
        '_,
        Result<Option<radroots_domain::stock::PoolView>, OperationError>,
    > {
        Box::pin(async move {
            self.capture.queries.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        })
    }
}
fn store() -> Shared<Store> {
    Shared::new(Store {
        capture: evidence::Control::default(),
        model: new_model(),
        calls: Mutex::new(vec![]),
        waiting: AtomicBool::new(false),
        waker: AtomicWaker::new(),
    })
}
fn minimum(scope: &WorkspaceSession) -> InventoryCheckpointRef {
    InventoryCheckpointRef::from_host_metadata(
        scope.binding().input().authority(),
        Sequence::new(0).unwrap(),
        None,
    )
    .unwrap()
}
fn operation() -> OperationId {
    OperationId::from_bytes([80; 32])
}
fn digest() -> CommandDigest {
    CommandDigest::from_bytes([81; 32])
}

#[test]
fn admitted_inventory_work_survives_observer_loss_bounds_bytes_and_blocks_close_until_settled() {
    block_on(async {
        let store = store();
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
        let minimum = minimum(&scope);
        drop(client.abort_proven_unissued(store.clone(), &scope, minimum, operation(), digest()));
        assert!(tasks.queued.lock().unwrap().is_empty());
        let mut first = Box::pin(client.abort_proven_unissued(
            store.clone(),
            &scope,
            minimum,
            operation(),
            digest(),
        ));
        let mut second = Box::pin(client.abort_proven_unissued(
            store.clone(),
            &scope,
            minimum,
            operation(),
            digest(),
        ));
        assert!(futures::poll!(first.as_mut()).is_pending());
        assert!(futures::poll!(second.as_mut()).is_pending());
        assert_eq!(tasks.queued.lock().unwrap().len(), 2);
        assert!(store.calls.lock().unwrap().is_empty());
        assert_eq!(
            client
                .abort_proven_unissued(store.clone(), &scope, minimum, operation(), digest())
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
        drop(first);
        drop(second);
        let mut closing = Box::pin(client.close());
        assert!(futures::poll!(closing.as_mut()).is_pending());
        assert!(
            scope
                .command_context(operation(), None, None, UnixSeconds::new(150).unwrap())
                .is_err()
        );
        let mut jobs = core::mem::take(&mut *tasks.queued.lock().unwrap());
        store.waiting.store(true, Ordering::SeqCst);
        assert!(futures::poll!(jobs[0].as_mut()).is_pending());
        assert!(futures::poll!(closing.as_mut()).is_pending());
        store.waiting.store(false, Ordering::SeqCst);
        store.waker.wake();
        for task in jobs {
            task.await;
        }
        assert_eq!(
            *store.calls.lock().unwrap(),
            vec![(operation(), digest()); 2]
        );
        closing.await.unwrap();
        assert_eq!(
            client
                .abort_proven_unissued(store, &scope, minimum, operation(), digest())
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
    });
}

#[test]
fn task_refusal_foreign_scope_closed_scope_and_missing_host_dispatch_nothing() {
    block_on(async {
        let store = store();
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
        let foreign = WorkspaceSession::attach(store.clone(), input(5), proof())
            .await
            .unwrap();
        client.start().await.unwrap();
        let minimum = minimum(&scope);
        assert_eq!(
            client
                .abort_proven_unissued(store.clone(), &foreign, minimum, operation(), digest())
                .await
                .unwrap_err()
                .code(),
            Code::Unauthorized
        );
        tasks.refuse.store(true, Ordering::SeqCst);
        assert_eq!(
            client
                .abort_proven_unissued(store.clone(), &scope, minimum, operation(), digest())
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
        tasks.refuse.store(false, Ordering::SeqCst);
        let mut observed = Box::pin(client.abort_proven_unissued(
            store.clone(),
            &scope,
            minimum,
            operation(),
            digest(),
        ));
        assert!(futures::poll!(observed.as_mut()).is_pending());
        let task = tasks.queued.lock().unwrap().pop().unwrap();
        task.await;
        let StockAbortOutcome::Unknown(anchor) = observed.await.unwrap() else {
            panic!("must preserve uncertainty")
        };
        assert_eq!(anchor.operation_id(), operation());
        assert_eq!(anchor.command_digest(), digest());
        assert!(anchor.transaction_id().is_none());
        let mut query = Box::pin(client.stock_preparation_abort(
            store.clone(),
            &scope,
            minimum,
            operation(),
            digest(),
        ));
        assert!(futures::poll!(query.as_mut()).is_pending());
        let task = tasks.queued.lock().unwrap().pop().unwrap();
        task.await;
        let error = query.await.unwrap_err();
        assert_eq!(error.code(), Code::MissingHistory);
        assert_eq!(error.context().operation_id(), Some(operation()));
        assert_eq!(error.retry(), RetryClass::AfterStateRefresh);
        scope.close();
        assert_eq!(
            client
                .abort_proven_unissued(store.clone(), &scope, minimum, operation(), digest())
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
        client.close().await.unwrap();
        let without_host = ClientSession::new(Host, core::num::NonZeroU16::MIN);
        let scope = without_host
            .attach_workspace(store.clone(), input(5), proof())
            .await
            .unwrap();
        without_host.start().await.unwrap();
        assert_eq!(
            without_host
                .abort_proven_unissued(store.clone(), &scope, minimum, operation(), digest())
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
        without_host.close().await.unwrap();
        assert_eq!(store.calls.lock().unwrap().len(), 1);
        assert!(tasks.queued.lock().unwrap().is_empty());
    });
}

#[test]
fn loss_of_an_accepted_admission_task_never_reports_known_nonadmission() {
    block_on(async {
        let store = store();
        // Port metadata only. This test deliberately drops an accepted host
        // task; it does not qualify that host or any native transaction.
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
        let tasks = Shared::new(Tasks::default());
        let client =
            ClientSession::new_with_task_host(Host, core::num::NonZeroU16::MIN, tasks.clone());
        let scope = client
            .attach_workspace(store.clone(), input(5), proof())
            .await
            .unwrap();
        client.start().await.unwrap();
        let input = radroots_protocol::command::CreatePoolInput::new(
            PoolId::from_bytes([82; 32]),
            "private model stock",
            radroots_domain::stock::PoolUnit::new(
                radroots_domain::listing::MeasurementUnit::Each,
                0,
            )
            .unwrap(),
            Quantity::new(
                "1".parse().unwrap(),
                radroots_domain::listing::MeasurementUnit::Each,
            )
            .unwrap(),
        )
        .unwrap();
        let context = scope
            .command_context(operation(), None, None, UnixSeconds::new(150).unwrap())
            .unwrap();
        let mut observed = Box::pin(client.create_stock_pool(
            store.clone(),
            &scope,
            context,
            minimum(&scope),
            input,
        ));
        assert!(futures::poll!(observed.as_mut()).is_pending());
        // Simulate a nonconforming host losing its accepted task. Silence
        // cannot prove the store never committed, and must not authorize retry.
        drop(tasks.queued.lock().unwrap().pop().unwrap());
        let error = observed.await.unwrap_err();
        assert_eq!(error.code(), Code::CommitUnknown);
        assert_eq!(
            error.context().admission(),
            radroots_execution::Admission::Unknown
        );
        assert_eq!(error.context().operation_id(), Some(operation()));
        assert_eq!(error.retry(), RetryClass::ManualReconciliation);
        assert!(store.calls.lock().unwrap().is_empty());
        client.close().await.unwrap();
    });
}
