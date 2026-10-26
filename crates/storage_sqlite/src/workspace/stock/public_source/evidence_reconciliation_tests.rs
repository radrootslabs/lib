//! Real SQLite original-evidence reconciliation on the same namespace owner.
use super::*;
use radroots_storage::inventory::{InventoryStore, StockEvidenceRequest};

async fn fault(r: &Rig, unknown: bool) -> SqliteConnection {
    let mut connection = database(r).await;
    if unknown {
        sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_signing_evidence BEGIN INSERT INTO deferred_fault VALUES(2); END").execute(&mut connection).await.unwrap();
    } else {
        sqlx::raw_sql("CREATE TRIGGER fault BEFORE INSERT ON stock_signing_evidence BEGIN SELECT RAISE(ABORT,'synthetic original capture failure'); END").execute(&mut connection).await.unwrap();
    }
    connection
}
async fn remove_fault(connection: &mut SqliteConnection, unknown: bool) {
    sqlx::raw_sql(if unknown {
        "DROP TRIGGER fault; DROP TABLE deferred_fault"
    } else {
        "DROP TRIGGER fault"
    })
    .execute(connection)
    .await
    .unwrap();
}
async fn writer_remains_owned(r: &Rig) {
    let error = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(error, WorkspaceError::WriterActive);
}
async fn request(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
) -> (StockSigningAttempt, StockEvidenceRequest, String) {
    let recipient = receipt.rumor().recipients()[0];
    let (report, raw) = obtained(r, receipt, recipient).await;
    let request = StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None)
        .unwrap()
        .into_request();
    (report, request, raw)
}
async fn reconcile(
    r: &Rig,
    request: &StockEvidenceRequest,
) -> Result<StockEvidenceOutcome, OperationError> {
    let port: &dyn InventoryStore = &r.store;
    port.reconcile_stock_evidence(request, r.enrollment.proof())
        .await
}

#[tokio::test]
async fn same_owner_capture_reconciliation_resolves_real_rollback_and_unknown_commit_without_clock_or_reissue()
 {
    for unknown in [false, true] {
        let r = Rig::new().await;
        let preparation = ready(&r).await;
        let (report, request, raw) = request(&r, &preparation).await;
        let before = inventory_minimum(&r).await;
        let mut database = fault(&r, unknown).await;
        let first =
            InventoryStore::capture_stock_evidence(&r.store, &request, r.enrollment.proof()).await;
        if unknown {
            assert!(matches!(first.unwrap(), StockEvidenceOutcome::Unknown(_)));
            assert_eq!(
                InventoryStore::capture_stock_evidence(&r.store, &request, r.enrollment.proof())
                    .await
                    .unwrap_err(),
                OperationError::RecoveryRequired
            );
        } else {
            assert!(first.is_err());
        }
        writer_remains_owned(&r).await;
        remove_fault(&mut database, unknown).await;
        let original_clock = r.clock.0.load(Ordering::SeqCst);
        r.clock.0.store(0, Ordering::SeqCst);
        let retained = committed(reconcile(&r, &request).await.unwrap());
        assert_eq!(retained.signing_attempt_id(), report.attempt_id());
        assert_eq!(retained.observed_at_unix_ms(), None);
        assert_eq!(retained.digest(), &request.evidence_digest());
        assert_eq!(committed(reconcile(&r, &request).await.unwrap()), retained);
        request.with_json(|value| assert_eq!(value, raw));
        r.clock.0.store(original_clock, Ordering::SeqCst);
        assert_eq!(
            inventory_minimum(&r).await.revision().get(),
            before.revision().get() + 1
        );
        let StockClaimOutcome::Existing(original) =
            claim(&r, &preparation, report.recipient()).await.unwrap()
        else {
            panic!("reconciliation minted a fresh claim")
        };
        assert_eq!(original.attempt_id(), report.attempt_id());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_signing_evidence")
                .fetch_one(&mut database)
                .await
                .unwrap(),
            1
        );
        writer_remains_owned(&r).await;
        assert_pool(&r, 80, 9, 1, 2).await;
        database.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn refused_stock_recovery_preflight_keeps_writer_and_cached_original_authorization_for_exact_retry()
 {
    let mut r = Rig::new().await;
    let preparation = ready(&r).await;
    let (report, request, raw) = request(&r, &preparation).await;
    let provider = reopen(&mut r).await;
    let mut database = fault(&r, true).await;
    assert!(matches!(
        InventoryStore::capture_stock_evidence(&r.store, &request, r.enrollment.proof())
            .await
            .unwrap(),
        StockEvidenceOutcome::Unknown(_)
    ));
    remove_fault(&mut database, true).await;
    provider
        .gate
        .arm(Direction::Unwrap, "radroots.seller_stock.authority.v13", 0);
    let mut recovery = Box::pin(reconcile(&r, &request));
    assert!(futures::poll!(recovery.as_mut()).is_pending());
    provider.gate.entered();
    // The authorized old snapshot has already been read. Future schema then
    // refuses read-only preflight after close; no migration or repair occurs.
    sqlx::raw_sql("PRAGMA user_version=14")
        .execute(&mut database)
        .await
        .unwrap();
    provider.gate.release();
    assert!(recovery.await.is_err());
    writer_remains_owned(&r).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&mut database)
            .await
            .unwrap(),
        14
    );
    let wrong = AttachmentProof::from_protected_input(&[99; 32]).unwrap();
    assert!(
        InventoryStore::reconcile_stock_evidence(&r.store, &request, &wrong)
            .await
            .is_err()
    );
    request.with_json(|value| assert_eq!(value, raw));
    sqlx::raw_sql("PRAGMA user_version=13")
        .execute(&mut database)
        .await
        .unwrap();
    let retained = committed(reconcile(&r, &request).await.unwrap());
    assert_eq!(retained.signing_attempt_id(), report.attempt_id());
    assert_eq!(committed(reconcile(&r, &request).await.unwrap()), retained);
    assert_pool(&r, 80, 9, 1, 2).await;
    database.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn lost_reconciliation_observer_drains_the_actual_replacement_and_original_capture_before_reopen()
 {
    let mut r = Rig::new().await;
    let preparation = ready(&r).await;
    let (report, request, raw) = request(&r, &preparation).await;
    let provider = reopen(&mut r).await;
    let mut database = fault(&r, true).await;
    assert!(matches!(
        InventoryStore::capture_stock_evidence(&r.store, &request, r.enrollment.proof())
            .await
            .unwrap(),
        StockEvidenceOutcome::Unknown(_)
    ));
    remove_fault(&mut database, true).await;
    provider
        .gate
        .arm(Direction::Unwrap, "radroots.seller_stock.authority.v13", 0);
    let mut recovery = Box::pin(reconcile(&r, &request));
    assert!(futures::poll!(recovery.as_mut()).is_pending());
    provider.gate.entered();
    drop(recovery);
    provider.gate.release();
    r.store.close().await.unwrap();
    database.close().await.unwrap();
    reopen_plain(&mut r).await;
    let retained = committed(reconcile(&r, &request).await.unwrap());
    let stored = r
        .store
        .stock_signing_evidence(
            selector(&r, &preparation, report.recipient()).await,
            *retained.digest(),
            r.enrollment.proof(),
        )
        .await
        .unwrap()
        .unwrap();
    stored.with_json(|value| assert_eq!(value, raw));
    let StockClaimOutcome::Existing(original) =
        claim(&r, &preparation, report.recipient()).await.unwrap()
    else {
        panic!("lost observer minted a claim")
    };
    assert_eq!(original.attempt_id(), report.attempt_id());
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}
