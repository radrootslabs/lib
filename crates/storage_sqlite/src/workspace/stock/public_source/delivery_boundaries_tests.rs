//! Concrete endpoint limits, bounded pagination and late transaction witnesses.
use super::*;

#[tokio::test]
async fn stale_observed_attempt_count_cannot_claim_another_invocation() {
    let (r, original, _, outer) = setup(17).await;
    committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let first = request(&r, &original, &outer, 40)
        .await
        .with_expected_attempt_count(0)
        .unwrap();
    let racing = request(&r, &original, &outer, 41)
        .await
        .with_expected_attempt_count(0)
        .unwrap();
    assert!(matches!(
        r.store
            .prepare_stock_delivery_attempt(first, proof(&r))
            .await
            .unwrap(),
        StockDeliveryAttemptOutcome::Claimed(_)
    ));
    let before = inventory_minimum(&r).await;
    assert!(matches!(
        r.store
            .prepare_stock_delivery_attempt(racing, proof(&r))
            .await,
        Err(OperationError::StaleState)
    ));
    assert_eq!(inventory_minimum(&r).await, before);
    let repeated = request(&r, &original, &outer, 40)
        .await
        .with_expected_attempt_count(0)
        .unwrap();
    assert!(matches!(
        r.store
            .prepare_stock_delivery_attempt(repeated, proof(&r))
            .await
            .unwrap(),
        StockDeliveryAttemptOutcome::Retained(_)
    ));
    let next = request(&r, &original, &outer, 41)
        .await
        .with_expected_attempt_count(1)
        .unwrap();
    assert!(matches!(
        r.store
            .prepare_stock_delivery_attempt(next, proof(&r))
            .await
            .unwrap(),
        StockDeliveryAttemptOutcome::Claimed(_)
    ));
    assert!(
        request(&r, &original, &outer, 42)
            .await
            .with_expected_attempt_count(193)
            .is_err()
    );
    balances(&r, 80, 9, 0, 1, 3).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn twelve_potential_invocations_are_retained_and_original_outcomes_page_at_sixteen() {
    let (mut r, original, _, outer) = setup(17).await;
    committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let mut expected = Vec::new();
    for ordinal in 1..=12 {
        let attempt = claim(&r, &original, &outer, ordinal).await;
        assert_eq!(attempt.target_ordinals(), &[ordinal]);
        if ordinal <= 9 {
            for outcome in [DeliveryOutcome::rejected(), DeliveryOutcome::unavailable()] {
                expected.push(save(&r, &observed(&attempt, outcome, None)).await);
            }
        }
    }
    let before = inventory_minimum(&r).await;
    assert!(matches!(
        r.store
            .prepare_stock_delivery_attempt(request(&r, &original, &outer, 13).await, proof(&r))
            .await,
        Err(OperationError::DeliveryAttemptLimit)
    ));
    assert_eq!(inventory_minimum(&r).await, before);
    reopen_plain(&mut r).await;
    let summary = r
        .store
        .stock_delivery_summary(
            envelope_request(&r, &original, outer.recipient(), None).await,
            proof(&r),
        )
        .await
        .unwrap();
    assert_eq!(summary.attempt_count(), 12);
    assert_eq!(summary.targets()[0].potential_attempts(), 12);
    assert!(!summary.acceptance_satisfied().unwrap());
    let first = r
        .store
        .stock_delivery_observations(
            envelope_request(&r, &original, outer.recipient(), None).await,
            None,
            proof(&r),
        )
        .await
        .unwrap();
    assert_eq!(first.entries(), &expected[..16]);
    let second = r
        .store
        .stock_delivery_observations(
            envelope_request(&r, &original, outer.recipient(), None).await,
            first.next_after(),
            proof(&r),
        )
        .await
        .unwrap();
    assert!(first.next_after().is_some());
    assert_eq!(second.entries(), &expected[16..]);
    assert!(second.next_after().is_none());
    balances(&r, 80, 9, 0, 1, 3).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn late_extra_projection_cannot_cross_the_final_attempt_transaction() {
    let (mut r, original, _, outer) = setup(17).await;
    committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let selected = request(&r, &original, &outer, 21).await;
    let provider = reopen(&mut r).await;
    let mut db = database(&r).await;
    provider
        .gate
        .arm(Direction::Wrap, "radroots.seller_stock.authority.v17", 0);
    let mut pending = Box::pin(r.store.prepare_stock_delivery_attempt(selected, proof(&r)));
    assert!(futures::poll!(pending.as_mut()).is_pending());
    provider.gate.entered();
    sqlx::raw_sql("INSERT INTO stock_delivery_attempts(actor,operation_id,recipient,request_key,entry_id) SELECT e.actor,e.operation_id,e.recipient,randomblob(32),(SELECT operation_id FROM stock_operations WHERE family=0 LIMIT 1) FROM stock_envelopes e LIMIT 1").execute(&mut db).await.unwrap();
    provider.gate.release();
    assert_eq!(pending.await.unwrap_err().code(), "recovery_required");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_operations WHERE family=11")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    db.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn retained_sixteen_captures_original_outer_but_refuses_new_delivery_without_migration() {
    let (r, original, _, outer) = setup(16).await;
    committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    assert!(matches!(
        r.store
            .prepare_stock_delivery_attempt(request(&r, &original, &outer, 22).await, proof(&r))
            .await,
        Err(OperationError::UnsupportedOperation)
    ));
    let mut db = database(&r).await;
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(catalog, 16);
    db.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn current_operation_snapshot_retains_delivery_at_its_exact_authorized_checkpoint() {
    let (r, original, _, outer) = setup(17).await;
    committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let attempt = claim(&r, &original, &outer, 23).await;
    save(&r, &observed(&attempt, DeliveryOutcome::accepted(), None)).await;
    let snapshot = r
        .store
        .stock_operation_snapshot(
            crate::workspace::StockSnapshotRequest::new(
                r.binding.clone(),
                inventory_minimum(&r).await,
                original.attempt().operation_id(),
                original.attempt().command_digest(),
            ),
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    let delivery = snapshot.delivery().unwrap();
    for (value, recipient) in delivery.iter().zip(snapshot.recipients()) {
        assert_eq!(value.checkpoint(), snapshot.checkpoint());
        if recipient.recipient() == outer.recipient() {
            assert_eq!(value.attempt_count(), 1);
            assert!(value.acceptance_satisfied().unwrap());
        } else {
            assert!(value.source().is_none());
            assert_eq!(value.attempt_count(), 0);
        }
    }
    r.store.close().await.unwrap();
}
