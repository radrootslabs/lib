//! Controlled accepted-task ownership, not native consent/stock qualification.
use super::*;
use radroots_domain::{ExpectedOrder, OrderId, RecordId, TermsDigest};
use radroots_protocol::command::{AcceptOrderInput, AllocationSelection};
use radroots_storage::workspace::seller_stock::{
    StockListingCheckpointRef, StockSourceCheckpointRef,
};

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
fn context(scope: &WorkspaceSession) -> radroots_execution::workspace::CommandContext {
    scope
        .command_context(
            operation(),
            Some(ExpectedOrder::new(
                OrderId::from_bytes([84; 32]),
                None,
                Sequence::new(0).unwrap(),
                None,
                input(5).checkpoint().clone(),
            )),
            None,
            UnixSeconds::new(150).unwrap(),
        )
        .unwrap()
}
fn source(scope: &WorkspaceSession) -> StockSourceCheckpointRef {
    StockSourceCheckpointRef::from_host_metadata(
        scope.binding().input().authority(),
        Sequence::new(2).unwrap(),
        Some(CheckpointDigest::from_bytes([22; 32])),
    )
    .unwrap()
}
fn listing(scope: &WorkspaceSession) -> StockListingCheckpointRef {
    StockListingCheckpointRef::from_host_metadata(
        scope.binding().input().authority(),
        Sequence::new(3).unwrap(),
        Some(CheckpointDigest::from_bytes([33; 32])),
    )
    .unwrap()
}
fn acceptance() -> AcceptOrderInput {
    AcceptOrderInput::new(
        RecordId::from_bytes([85; 32]),
        TermsDigest::from_bytes([86; 32]),
        AllocationSelection::automatic(),
    )
}

#[test]
fn accepted_acceptance_keeps_inputs_budget_and_permit_after_observer_loss_until_settlement() {
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
        drop(client.prepare_stock_acceptance(
            store.clone(),
            &scope,
            context(&scope),
            minimum(&scope),
            source(&scope),
            listing(&scope),
            acceptance(),
        ));
        assert!(tasks.queued.lock().unwrap().is_empty());
        let mut first = Box::pin(client.prepare_stock_acceptance(
            store.clone(),
            &scope,
            context(&scope),
            minimum(&scope),
            source(&scope),
            listing(&scope),
            acceptance(),
        ));
        let mut second = Box::pin(client.prepare_stock_acceptance(
            store.clone(),
            &scope,
            context(&scope),
            minimum(&scope),
            source(&scope),
            listing(&scope),
            acceptance(),
        ));
        assert!(futures::poll!(first.as_mut()).is_pending());
        assert!(futures::poll!(second.as_mut()).is_pending());
        assert_eq!(tasks.queued.lock().unwrap().len(), 2);
        assert!(store.calls.lock().unwrap().is_empty());
        assert_eq!(
            client
                .prepare_stock_acceptance(
                    store.clone(),
                    &scope,
                    context(&scope),
                    minimum(&scope),
                    source(&scope),
                    listing(&scope),
                    acceptance()
                )
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
        drop(first);
        drop(second);
        let mut close = Box::pin(client.close());
        assert!(futures::poll!(close.as_mut()).is_pending());
        let mut jobs = core::mem::take(&mut *tasks.queued.lock().unwrap());
        store.waiting.store(true, Ordering::SeqCst);
        assert!(futures::poll!(jobs[0].as_mut()).is_pending());
        assert!(futures::poll!(close.as_mut()).is_pending());
        store.waiting.store(false, Ordering::SeqCst);
        store.waker.wake();
        for task in jobs {
            task.await;
        }
        let calls = store.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], calls[1]);
        assert_eq!(calls[0].0, operation());
        close.await.unwrap();
    });
}

#[test]
fn host_and_foreign_context_scope_or_closed_scope_refuse_without_dispatch() {
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
        let foreign = WorkspaceSession::attach(store.clone(), input(5), proof())
            .await
            .unwrap();
        client.start().await.unwrap();
        for (selected, captured) in [(&foreign, context(&foreign)), (&scope, context(&foreign))] {
            assert_eq!(
                client
                    .prepare_stock_acceptance(
                        store.clone(),
                        selected,
                        captured,
                        minimum(&scope),
                        source(&scope),
                        listing(&scope),
                        acceptance()
                    )
                    .await
                    .unwrap_err()
                    .code(),
                Code::Unauthorized
            );
        }
        tasks.refuse.store(true, Ordering::SeqCst);
        assert_eq!(
            client
                .prepare_stock_acceptance(
                    store.clone(),
                    &scope,
                    context(&scope),
                    minimum(&scope),
                    source(&scope),
                    listing(&scope),
                    acceptance()
                )
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
        tasks.refuse.store(false, Ordering::SeqCst);
        let captured = context(&scope);
        scope.close();
        assert_eq!(
            client
                .prepare_stock_acceptance(
                    store.clone(),
                    &scope,
                    captured,
                    minimum(&scope),
                    source(&scope),
                    listing(&scope),
                    acceptance()
                )
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
        client.close().await.unwrap();
        let no_host = ClientSession::new(Host, core::num::NonZeroU16::MIN);
        let scope = no_host
            .attach_workspace(store.clone(), input(5), proof())
            .await
            .unwrap();
        no_host.start().await.unwrap();
        assert_eq!(
            no_host
                .prepare_stock_acceptance(
                    store.clone(),
                    &scope,
                    context(&scope),
                    minimum(&scope),
                    source(&scope),
                    listing(&scope),
                    acceptance()
                )
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
        no_host.close().await.unwrap();
        assert!(store.calls.lock().unwrap().is_empty());
        assert!(tasks.queued.lock().unwrap().is_empty());
    });
}

#[test]
fn lost_accepted_acceptance_task_preserves_original_identity_and_unknown_admission() {
    block_on(async {
        let store = selected_store();
        let tasks = Shared::new(Tasks::default());
        let client =
            ClientSession::new_with_task_host(Host, core::num::NonZeroU16::MIN, tasks.clone());
        let scope = client
            .attach_workspace(store.clone(), input(5), proof())
            .await
            .unwrap();
        client.start().await.unwrap();
        let mut observed = Box::pin(client.prepare_stock_acceptance(
            store.clone(),
            &scope,
            context(&scope),
            minimum(&scope),
            source(&scope),
            listing(&scope),
            acceptance(),
        ));
        assert!(futures::poll!(observed.as_mut()).is_pending());
        // Deliberately nonconforming host: silence cannot prove no T0 commit.
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
