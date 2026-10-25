use super::*;

#[tokio::test]
async fn replacement_recover_only_consent_can_abort_after_first_issue_expiry() {
    for has_plan in [false, true] {
        let r = Rig::new().await;
        let (command, receipt) = Box::pin(prepared(&r)).await;
        if has_plan {
            bound(bind(&r, &receipt, 42).await.unwrap());
        }
        r.clock.0.store(250, Ordering::SeqCst);
        assert_eq!(
            abort(&r, &receipt).await.unwrap_err(),
            OperationError::DeadlineExceeded
        );
        let child = StockGrantEnrollment::prepare(
            &SellerAuthoritySelection::from_attachment(r.enrollment.input()),
            "recovery operator".into(),
            UnixSeconds::new(290).unwrap(),
            StockGrantScope::full_inventory(),
        )
        .unwrap();
        assert!(matches!(
            r.store
                .enroll_stock_grant(
                    r.binding.clone(),
                    inventory_minimum(&r).await,
                    r.enrollment.proof(),
                    &child
                )
                .await
                .unwrap(),
            StockGrantOutcome::Committed(_)
        ));
        let binding = r
            .store
            .authenticate(child.input().clone(), child.proof())
            .await
            .unwrap();
        let consent = StockConsentCapture::prepare(
            SellerAuthoritySelection::from_attachment(r.enrollment.input()),
            child.input().grant_id(),
            StockConsentMode::RecoverOnly,
            StockResources::all_pools(),
            UnixSeconds::new(280).unwrap(),
            CapturedOrderAcceptance::parse(command.protected_wire()).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            r.store
                .issue_stock_consent(
                    r.binding.clone(),
                    inventory_minimum(&r).await,
                    &consent,
                    r.enrollment.proof()
                )
                .await
                .unwrap(),
            StockConsentOutcome::Committed(_)
        ));
        let first = released(
            r.store
                .abort_proven_unissued(
                    StockAbortRequest::new(
                        binding.clone(),
                        inventory_minimum(&r).await,
                        receipt.attempt().operation_id(),
                        receipt.attempt().command_digest(),
                    ),
                    child.proof(),
                )
                .await
                .unwrap(),
        );
        assert_eq!(first.aborted_at().get(), 250);
        balances(&r, 80, 10, 0, 0, 3).await;
        balances(&r, 81, 10, 0, 0, 3).await;
        let current = inventory_minimum(&r).await;
        let replay = released(
            r.store
                .abort_proven_unissued(
                    StockAbortRequest::new(
                        binding,
                        current,
                        receipt.attempt().operation_id(),
                        receipt.attempt().command_digest(),
                    ),
                    child.proof(),
                )
                .await
                .unwrap(),
        );
        assert_eq!(first.attempt(), replay.attempt());
        assert_eq!(inventory_minimum(&r).await, current);
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn late_order_grant_pool_and_unrelated_preparation_changes_never_commit_a_release() {
    for damage in 0..5 {
        let mut r = Rig::new().await;
        let (_, receipt) = Box::pin(prepared(&r)).await;
        if damage == 4 {
            retain_source(&r, "request_other_order").await;
            let command = acceptance_command_snapshot(
                &r,
                91,
                false,
                &inspect_named(&r, "request_other_order").await,
            );
            acceptance_consent(&r, &command).await;
            prepare(&r, &command).await.unwrap();
        }
        let provider = reopen(&mut r).await;
        let selected = abort_request(&r, &receipt).await;
        let mut raw = database(&r).await;
        let root: Vec<u8> =
            sqlx::query_scalar("SELECT protected FROM stock_authority WHERE singleton=1")
                .fetch_one(&mut raw)
                .await
                .unwrap();
        provider
            .gate
            .arm(Direction::Wrap, "radroots.seller_stock.authority.v13", 0);
        let mut pending = Box::pin(
            r.store
                .abort_proven_unissued(selected, r.enrollment.proof()),
        );
        assert!(futures::poll!(pending.as_mut()).is_pending());
        provider.gate.entered();
        match damage {
            0 => {
                sqlx::raw_sql("UPDATE stock_order_states SET phase=1")
                    .execute(&mut raw)
                    .await
                    .unwrap();
            }
            1 => {
                sqlx::raw_sql("UPDATE stock_grants SET protected=randomblob(length(protected))")
                    .execute(&mut raw)
                    .await
                    .unwrap();
            }
            2 => {
                sqlx::raw_sql("UPDATE stock_pools SET protected=randomblob(length(protected))")
                    .execute(&mut raw)
                    .await
                    .unwrap();
            }
            3 => {
                sqlx::raw_sql("UPDATE stock_orders SET protected=randomblob(length(protected))")
                    .execute(&mut raw)
                    .await
                    .unwrap();
            }
            _ => {
                sqlx::query("UPDATE stock_orders SET protected=randomblob(length(protected)) WHERE operation_id<>?")
                .bind(receipt.attempt().operation_id().as_bytes().as_slice()).execute(&mut raw).await.unwrap();
            }
        }
        let pools: Vec<(Vec<u8>, i64, Vec<u8>)> =
            sqlx::query_as("SELECT pool_id,revision,protected FROM stock_pools ORDER BY pool_id")
                .fetch_all(&mut raw)
                .await
                .unwrap();
        provider.gate.release();
        assert!(pending.await.is_err(), "damage {damage}");
        let after: Vec<(Vec<u8>, i64, Vec<u8>)> =
            sqlx::query_as("SELECT pool_id,revision,protected FROM stock_pools ORDER BY pool_id")
                .fetch_all(&mut raw)
                .await
                .unwrap();
        assert_eq!(after, pools);
        assert_eq!(
            sqlx::query_scalar::<_, Vec<u8>>(
                "SELECT protected FROM stock_authority WHERE singleton=1"
            )
            .fetch_one(&mut raw)
            .await
            .unwrap(),
            root
        );
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT (SELECT count(*) FROM stock_aborts)+(SELECT count(*) FROM stock_operations WHERE family=7)").fetch_one(&mut raw).await.unwrap(),0);
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn acknowledged_abort_refuses_restored_tail_without_erasing_original_history() {
    let mut r = Rig::new().await;
    let (_, receipt) = Box::pin(prepared(&r)).await;
    r.store.close().await.unwrap();
    let backup = r.fixture.root().join("before-abort.sqlite");
    std::fs::copy(r.path.join("stock_execution_v1.sqlite"), &backup).unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    let acknowledged = released(abort(&r, &receipt).await.unwrap()).checkpoint();
    r.store.close().await.unwrap();
    std::fs::copy(&backup, r.path.join("stock_execution_v1.sqlite")).unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    let selected = StockAbortRequest::new(
        r.binding.clone(),
        acknowledged,
        receipt.attempt().operation_id(),
        receipt.attempt().command_digest(),
    );
    assert_eq!(
        r.store
            .stock_preparation_abort(selected, r.enrollment.proof())
            .await
            .unwrap_err()
            .code(),
        "stale_checkpoint"
    );
    balances(&r, 80, 9, 1, 0, 2).await;
    balances(&r, 81, 8, 2, 0, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn unknown_abort_commit_fences_owner_and_reopens_original_identity_without_signing() {
    let mut r = Rig::new().await;
    let (_, receipt) = Box::pin(prepared(&r)).await;
    let selected = abort_request(&r, &receipt).await;
    let mut raw = database(&r).await;
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_aborts BEGIN INSERT INTO deferred_fault VALUES(2); END").execute(&mut raw).await.unwrap();
    let StockAbortOutcome::Unknown(anchor) = abort(&r, &receipt).await.unwrap() else {
        panic!("unknown commit");
    };
    assert_eq!(anchor.operation_id(), receipt.attempt().operation_id());
    assert_eq!(
        r.store
            .abort_proven_unissued(selected, r.enrollment.proof())
            .await
            .unwrap_err(),
        OperationError::RecoveryRequired
    );
    r.store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER fault; DROP TABLE deferred_fault")
        .execute(&mut raw)
        .await
        .unwrap();
    raw.close().await.unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    let outcome = released(abort(&r, &receipt).await.unwrap());
    assert_eq!(outcome.attempt().operation_id(), anchor.operation_id());
    balances(&r, 80, 10, 0, 0, 3).await;
    balances(&r, 81, 10, 0, 0, 3).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn losing_abort_observer_drains_original_admitted_work_before_reopen() {
    let mut r = Rig::new().await;
    let (_, receipt) = Box::pin(prepared(&r)).await;
    let provider = reopen(&mut r).await;
    let selected = abort_request(&r, &receipt).await;
    provider
        .gate
        .arm(Direction::Wrap, "radroots.seller_stock.authority.v13", 0);
    let mut pending = Box::pin(
        r.store
            .abort_proven_unissued(selected, r.enrollment.proof()),
    );
    assert!(futures::poll!(pending.as_mut()).is_pending());
    provider.gate.entered();
    drop(pending);
    provider.gate.release();
    reopen_plain(&mut r).await;
    let original = r
        .store
        .stock_preparation_abort(abort_request(&r, &receipt).await, r.enrollment.proof())
        .await
        .unwrap()
        .unwrap();
    let before = inventory_minimum(&r).await;
    assert_eq!(
        released(abort(&r, &receipt).await.unwrap()).attempt(),
        original.attempt()
    );
    assert_eq!(inventory_minimum(&r).await, before);
    balances(&r, 80, 10, 0, 0, 3).await;
    balances(&r, 81, 10, 0, 0, 3).await;
    r.store.close().await.unwrap();
}
