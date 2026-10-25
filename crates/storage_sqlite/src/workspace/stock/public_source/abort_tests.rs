//! Actual explicit release, retained originals and subsequent authorized work.
use super::super::super::super::super::super::retain as retain_source;
use super::super::super::super::super::{
    command as acceptance_command, command_snapshot as acceptance_command_snapshot,
    consent as acceptance_consent, inspect_named, request as acceptance_request,
};
use super::*;
use radroots_storage::inventory::{
    InventoryStore, StockAbortOutcome, StockAbortReceipt, StockAbortRequest,
};
#[path = "abort_failures_tests.rs"]
mod failures;

async fn abort_request(r: &Rig, receipt: &StockAcceptanceReceipt) -> StockAbortRequest {
    StockAbortRequest::new(
        r.binding.clone(),
        inventory_minimum(r).await,
        receipt.attempt().operation_id(),
        receipt.attempt().command_digest(),
    )
}
pub(super) async fn abort(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
) -> Result<StockAbortOutcome, OperationError> {
    let store: &dyn InventoryStore = &r.store;
    store
        .abort_proven_unissued(
            abort_request(r, receipt).await,
            r.enrollment
                .proof()
                .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
        )
        .await
}
pub(super) fn released(value: StockAbortOutcome) -> StockAbortReceipt {
    let StockAbortOutcome::Aborted(value) = value else {
        panic!("abort must confirm");
    };
    *value
}
#[tokio::test]
async fn unclaimed_preparation_aborts_before_or_after_t1_and_originals_survive_new_order_attempt() {
    for has_plan in [false, true] {
        let mut r = Rig::new().await;
        let (command, receipt) = Box::pin(prepared(&r)).await;
        if has_plan {
            bound(bind(&r, &receipt, 42).await.unwrap());
        }
        let before = inventory_minimum(&r).await;
        let first = released(abort(&r, &receipt).await.unwrap());
        let shared: &dyn InventoryStore = &r.store;
        let retained = shared
            .stock_preparation_abort(
                abort_request(&r, &receipt).await,
                r.enrollment
                    .proof()
                    .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                    .unwrap(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retained.attempt(), first.attempt());
        assert_eq!(
            first.attempt().operation_id(),
            receipt.attempt().operation_id()
        );
        assert_eq!(
            first.checkpoint().revision().get(),
            before.revision().get() + 1
        );
        balances(&r, 80, 10, 0, 0, 3).await;
        balances(&r, 81, 10, 0, 0, 3).await;
        assert!(history(&r).await.analysis().seller_head().is_none());
        let acknowledged = inventory_minimum(&r).await;
        reopen_plain(&mut r).await;
        let replay = released(abort(&r, &receipt).await.unwrap());
        assert_eq!(replay.attempt(), first.attempt());
        assert_eq!(inventory_minimum(&r).await, acknowledged);
        assert!(matches!(
            r.store
                .prepare_stock_acceptance(
                    acceptance_request(&r, &command).await,
                    r.enrollment.proof()
                )
                .await
                .unwrap(),
            StockAcceptanceOutcome::Aborted(_)
        ));
        if !has_plan {
            assert!(bind(&r, &receipt, 42).await.is_err());
        }
        for recipient in receipt.rumor().recipients() {
            assert!(claim(&r, &receipt, *recipient).await.is_err());
        }
        assert!(install(&r, &receipt).await.is_err());
        let next = acceptance_command(&r, 91, false).await;
        acceptance_consent(&r, &next).await;
        let next = prepare(&r, &next).await.unwrap();
        assert_eq!(next.order_id(), receipt.order_id());
        assert_ne!(
            next.attempt().operation_id(),
            receipt.attempt().operation_id()
        );
        balances(&r, 80, 7, 3, 0, 4).await;
        balances(&r, 81, 10, 0, 0, 3).await;
        let current = inventory_minimum(&r).await;
        let view = r
            .store
            .stock_operation_snapshot(
                StockSnapshotRequest::new(
                    r.binding.clone(),
                    current,
                    receipt.attempt().operation_id(),
                    receipt.attempt().command_digest(),
                ),
                r.enrollment.proof(),
            )
            .await
            .unwrap();
        assert_eq!(view.aborted().unwrap().attempt(), first.attempt());
        assert!(view.installation().is_none());
        assert!(
            view.recipients()
                .iter()
                .all(|value| value.claim().is_none() && value.first_evidence().is_none())
        );
        assert_eq!(inventory_minimum(&r).await, current);
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn claims_actual_output_and_installation_never_become_abortable_absence() {
    for state in [0, 1, 2, 3] {
        let r = Rig::new().await;
        let receipt = Box::pin(ready(&r)).await;
        if state < 2 {
            let recipient = if state == 0 {
                peer(&r, &receipt)
            } else {
                r.enrollment.input().participant()
            };
            let StockClaimOutcome::Fresh(permit) = claim(&r, &receipt, recipient).await.unwrap()
            else {
                panic!("fresh claim");
            };
            drop(permit);
        } else {
            retain(&r, &receipt, peer(&r, &receipt)).await;
            if state == 3 {
                installed(install(&r, &receipt).await.unwrap());
            }
        }
        let before = inventory_minimum(&r).await;
        assert!(abort(&r, &receipt).await.is_err());
        assert_eq!(inventory_minimum(&r).await, before);
        if state == 3 {
            balances(&r, 80, 9, 0, 1, 3).await;
            balances(&r, 81, 8, 0, 2, 3).await;
        } else {
            balances(&r, 80, 9, 1, 0, 2).await;
            balances(&r, 81, 8, 2, 0, 2).await;
        }
        assert!(
            r.store
                .stock_preparation_abort(abort_request(&r, &receipt).await, r.enrollment.proof())
                .await
                .unwrap()
                .is_none()
        );
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn abort_releases_only_own_quantities_after_another_order_commits_and_stock_is_added() {
    let r = Rig::new().await;
    let (_, first) = Box::pin(prepared(&r)).await;
    retain_source(&r, "request_other_order").await;
    let command = acceptance_command_snapshot(
        &r,
        91,
        false,
        &inspect_named(&r, "request_other_order").await,
    );
    acceptance_consent(&r, &command).await;
    let other = prepare(&r, &command).await.unwrap();
    bound(bind(&r, &other, 42).await.unwrap());
    retain(&r, &other, peer(&r, &other)).await;
    installed(install(&r, &other).await.unwrap());
    balances(&r, 80, 6, 1, 3, 4).await;
    physical(&r,serde_json::json!({"type":"stock.adjust","context":context(&r,95),"input":{"pool_id":pool(80),"expected_version":"4","direction":"add","amount":"5","reason":"available correction before explicit cleanup"}})).await;
    released(abort(&r, &first).await.unwrap());
    balances(&r, 80, 12, 0, 3, 6).await;
    balances(&r, 81, 10, 0, 0, 3).await;
    assert!(abort(&r, &other).await.is_err());
    let before = inventory_minimum(&r).await;
    let replay = released(abort(&r, &first).await.unwrap());
    assert_eq!(replay.released_pools()[0].balances().introduced().get(), 15);
    assert_eq!(inventory_minimum(&r).await, before);
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn explicit_future_promise_aborts_without_invented_pools_or_movements() {
    let r = Rig::new().await;
    r.retain(&r.capture("listing_fixed")).await;
    retain_source(&r, "request_future_promise").await;
    let command = acceptance_command_snapshot(
        &r,
        90,
        false,
        &inspect_named(&r, "request_future_promise").await,
    );
    acceptance_consent(&r, &command).await;
    let receipt = prepare(&r, &command).await.unwrap();
    let outcome = released(abort(&r, &receipt).await.unwrap());
    assert!(outcome.released_pools().is_empty());
    let mut raw = database(&r).await;
    for query in [
        "SELECT count(*) FROM stock_pools",
        "SELECT count(*) FROM stock_movements",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(query)
                .fetch_one(&mut raw)
                .await
                .unwrap(),
            0
        );
    }
    raw.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn each_abort_write_failure_rolls_back_all_release_index_and_order_effects() {
    for statement in [
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operations WHEN NEW.family=7 BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operation_parts WHEN (SELECT family FROM stock_operations WHERE operation_id=NEW.operation_id)=7 BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_aborts BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_pools WHEN OLD.pool_id=(SELECT min(pool_id) FROM stock_pools) BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_pools WHEN OLD.pool_id=(SELECT max(pool_id) FROM stock_pools) BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_movements WHEN (SELECT count(*) FROM stock_movements m JOIN stock_operations o ON o.actor=m.actor AND o.operation_id=m.operation_id WHERE o.family=7)=0 BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_movements WHEN (SELECT count(*) FROM stock_movements m JOIN stock_operations o ON o.actor=m.actor AND o.operation_id=m.operation_id WHERE o.family=7)=1 BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_order_states BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'fault'); END",
    ] {
        let r = Rig::new().await;
        let (_, receipt) = Box::pin(prepared(&r)).await;
        let before = inventory_minimum(&r).await;
        let mut raw = database(&r).await;
        sqlx::raw_sql(statement).execute(&mut raw).await.unwrap();
        assert_eq!(
            abort(&r, &receipt).await.unwrap_err().code(),
            "storage_failure",
            "{statement}"
        );
        assert_eq!(inventory_minimum(&r).await, before);
        balances(&r, 80, 9, 1, 0, 2).await;
        balances(&r, 81, 8, 2, 0, 2).await;
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT (SELECT count(*) FROM stock_aborts)+(SELECT count(*) FROM stock_operations WHERE family=7)+(SELECT count(*) FROM stock_order_states WHERE phase<>0)").fetch_one(&mut raw).await.unwrap(),0);
        sqlx::raw_sql("DROP TRIGGER fault")
            .execute(&mut raw)
            .await
            .unwrap();
        released(abort(&r, &receipt).await.unwrap());
        balances(&r, 80, 10, 0, 0, 3).await;
        balances(&r, 81, 10, 0, 0, 3).await;
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn fresh_abort_refuses_time_before_original_preparation_and_preserves_historical_replay() {
    for has_plan in [false, true] {
        let r = Rig::new().await;
        r.clock.0.store(155, Ordering::SeqCst);
        let (_, receipt) = Box::pin(prepared(&r)).await;
        if has_plan {
            bound(bind(&r, &receipt, 42).await.unwrap());
        }
        let before = inventory_minimum(&r).await;
        r.clock.0.store(152, Ordering::SeqCst);
        assert_eq!(
            abort(&r, &receipt).await.unwrap_err().code(),
            "workspace_unavailable"
        );
        assert_eq!(inventory_minimum(&r).await, before);
        r.clock.0.store(155, Ordering::SeqCst);
        let original = released(abort(&r, &receipt).await.unwrap());
        let current = inventory_minimum(&r).await;
        r.clock.0.store(152, Ordering::SeqCst);
        assert_eq!(
            released(abort(&r, &receipt).await.unwrap()).attempt(),
            original.attempt()
        );
        assert_eq!(inventory_minimum(&r).await, current);
        r.store.close().await.unwrap();
    }
}
