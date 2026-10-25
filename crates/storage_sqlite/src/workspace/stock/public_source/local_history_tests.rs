//! Actual locally issued records must join the same retained causal graph.
use super::*;

type CausalMinima = (
    StockSourceCheckpointRef,
    StockListingCheckpointRef,
    radroots_storage::inventory::InventoryCheckpointRef,
);
async fn query(r: &Rig, minimum: CausalMinima) -> Result<StockHistorySnapshot, OperationError> {
    r.store
        .stock_order_history(
            r.binding.clone(),
            minimum.2,
            minimum.0,
            minimum.1,
            scope(),
            r.enrollment.proof(),
        )
        .await
}

#[tokio::test]
async fn bound_or_claimed_intents_without_actual_output_never_become_causal_records() {
    let r = Rig::new().await;
    let receipt = ready(&r).await;
    let before = history(&r).await;
    let StockClaimOutcome::Fresh(permit) = claim(&r, &receipt, peer(&r, &receipt)).await.unwrap()
    else {
        panic!("fresh claim");
    };
    drop(permit);
    let unknown = history(&r).await;
    assert_eq!(unknown.known_records(), before.known_records());
    assert!(unknown.analysis().seller_head().is_none());
    assert!(
        r.store
            .stock_agreement_installation(request(&r, &receipt).await, r.enrollment.proof())
            .await
            .unwrap()
            .is_none()
    );
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn peer_self_and_distinct_valid_encodings_supply_one_logical_record_without_installing() {
    let r = Rig::new().await;
    let receipt = ready(&r).await;
    let before = history(&r).await;
    for recipient in receipt.rumor().recipients() {
        let (report, raw) = obtained(&r, &receipt, *recipient).await;
        for text in [raw.clone(), format!("\n{raw}\n")] {
            let captured =
                StockEvidenceCapture::verify(r.binding.clone(), &report, &text, None).unwrap();
            committed(
                r.store
                    .capture_stock_signing_evidence(&captured, r.enrollment.proof())
                    .await
                    .unwrap(),
            );
            let view = history(&r).await;
            assert_eq!(view.known_records(), before.known_records() + 1);
            assert_eq!(
                view.analysis().seller_head(),
                Some(receipt.rumor().record_id())
            );
            assert_eq!(view.inventory_checkpoint(), inventory_minimum(&r).await);
        }
    }
    assert!(
        r.store
            .stock_agreement_installation(request(&r, &receipt).await, r.enrollment.proof())
            .await
            .unwrap()
            .is_none()
    );
    assert_pool(&r, 80, 9, 1, 2).await;
    let minimum = minima(&r).await;
    assert!(
        r.store
            .stock_order_request(
                r.binding.clone(),
                minimum.2,
                minimum.0,
                minimum.1,
                identity("request"),
                r.enrollment.proof()
            )
            .await
            .is_err(),
        "an issued seller decision cannot be presented as a fresh initial request"
    );
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn local_signed_records_from_another_order_do_not_advance_the_selected_order() {
    let r = Rig::new().await;
    let receipt = ready(&r).await;
    let own = scope();
    let other = radroots_domain::causal::CausalScope::new(
        own.thread_id(),
        radroots_domain::OrderId::from_bytes([0xa7; 32]),
        own.buyer(),
        own.seller(),
    )
    .unwrap();
    let minimum = minima(&r).await;
    let before = r
        .store
        .stock_order_history(
            r.binding.clone(),
            minimum.2,
            minimum.0,
            minimum.1,
            other,
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    retain(&r, &receipt, peer(&r, &receipt)).await;
    let minimum = minima(&r).await;
    let after = r
        .store
        .stock_order_history(
            r.binding.clone(),
            minimum.2,
            minimum.0,
            minimum.1,
            other,
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    assert_eq!(after.known_records(), before.known_records());
    assert!(after.analysis().seller_head().is_none());
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn inventory_minimum_rejects_restored_local_tail_with_unchanged_incoming_checkpoints() {
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    let before = minima(&r).await;
    r.store.close().await.unwrap();
    let backup = r.fixture.root().join("before-local-output.sqlite");
    std::fs::copy(r.path.join("stock_execution_v1.sqlite"), &backup).unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    retain(&r, &receipt, peer(&r, &receipt)).await;
    let acknowledged = minima(&r).await;
    assert_eq!((before.0, before.1), (acknowledged.0, acknowledged.1));
    assert_ne!(before.2, acknowledged.2);
    assert_eq!(
        query(&r, acknowledged)
            .await
            .unwrap()
            .analysis()
            .seller_head(),
        Some(receipt.rumor().record_id())
    );
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
    assert_eq!(
        query(&r, acknowledged).await.unwrap_err().code(),
        "stale_checkpoint"
    );
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn local_history_rechecks_signed_indexes_root_and_grant_after_provider_io_and_drains_loss() {
    for statement in [
        None,
        Some("UPDATE stock_signing_evidence SET evidence_digest=randomblob(32)"),
        Some("UPDATE stock_grants SET protected=randomblob(length(protected))"),
        Some("UPDATE stock_authority SET protected=randomblob(length(protected))"),
    ] {
        let mut r = Rig::new().await;
        let receipt = ready(&r).await;
        retain(&r, &receipt, peer(&r, &receipt)).await;
        let minimum = minima(&r).await;
        let provider = reopen(&mut r).await;
        let mut raw = database(&r).await;
        provider.gate.arm(
            Direction::Unwrap,
            "radroots.seller_stock.operation_part.v1",
            0,
        );
        let mut pending = Box::pin(query(&r, minimum));
        assert!(futures::poll!(pending.as_mut()).is_pending());
        provider.gate.entered();
        if let Some(statement) = statement {
            sqlx::raw_sql(statement).execute(&mut raw).await.unwrap();
            provider.gate.release();
            assert!(pending.await.is_err(), "{statement}");
        } else {
            drop(pending);
            provider.gate.release();
            reopen_plain(&mut r).await;
            assert_eq!(
                query(&r, minimum).await.unwrap().analysis().seller_head(),
                Some(receipt.rumor().record_id())
            );
            assert_eq!(minima(&r).await, minimum);
        }
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn initial_installation_refuses_clock_before_its_original_bound_record() {
    let r = Rig::new().await;
    r.clock.0.store(155, Ordering::SeqCst);
    let receipt = ready(&r).await;
    retain(&r, &receipt, peer(&r, &receipt)).await;
    let before = inventory_minimum(&r).await;
    r.clock.0.store(152, Ordering::SeqCst);
    assert_eq!(
        install(&r, &receipt).await.unwrap_err().code(),
        "workspace_unavailable",
        "fresh installation must not write a timestamp its retained decoder will reject"
    );
    assert_eq!(inventory_minimum(&r).await, before);
    r.clock.0.store(155, Ordering::SeqCst);
    let first = installed(install(&r, &receipt).await.unwrap());
    assert_eq!(
        r.store
            .stock_agreement_installation(request(&r, &receipt).await, r.enrollment.proof())
            .await
            .unwrap()
            .unwrap()
            .attempt(),
        first.attempt()
    );
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn actual_local_peer_commitment_advances_causal_history_once_before_and_after_installation() {
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    let before = history(&r).await;
    assert!(before.analysis().seller_head().is_none());
    retain(&r, &receipt, peer(&r, &receipt)).await;
    let obtained = history(&r).await;
    assert_eq!(
        obtained.known_records(),
        before.known_records() + 1,
        "actual retained local signing evidence must participate in complete local causal coverage"
    );
    assert_eq!(
        obtained.analysis().seller_head(),
        Some(receipt.rumor().record_id())
    );
    installed(install(&r, &receipt).await.unwrap());
    let committed = history(&r).await;
    assert_eq!(committed.known_records(), obtained.known_records());
    assert_eq!(
        committed.analysis().seller_head(),
        obtained.analysis().seller_head()
    );
    reopen_plain(&mut r).await;
    let reopened = history(&r).await;
    assert_eq!(reopened.known_records(), obtained.known_records());
    assert_eq!(
        reopened.analysis().seller_head(),
        obtained.analysis().seller_head()
    );
    r.store.close().await.unwrap();
}
