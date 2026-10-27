//! Actual replay admission transactions, original identity and uncertainty.
use super::*;
use crate::workspace::{StockReplayAdmissionReceipt, StockSnapshotRequest};
use sqlx::Connection;

async fn prepared_claim(r: &Rig) -> (StockAcceptanceReceipt, radroots_domain::SigningAttemptId) {
    let receipt = local_ready(r).await;
    let StockClaimOutcome::Fresh(permit) = claim(r, &receipt, receipt.rumor().recipients()[0])
        .await
        .unwrap()
    else {
        panic!("fresh original claim")
    };
    let id = permit.report().attempt_id();
    drop(permit);
    (receipt, id)
}
async fn marker(
    r: &Rig,
    original: &StockAcceptanceReceipt,
) -> Result<Option<StockReplayAdmissionReceipt>, OperationError> {
    marker_at(r, original, inventory_minimum(r).await).await
}
async fn marker_at(
    r: &Rig,
    original: &StockAcceptanceReceipt,
    minimum: radroots_storage::inventory::InventoryCheckpointRef,
) -> Result<Option<StockReplayAdmissionReceipt>, OperationError> {
    let snapshot = r
        .store
        .stock_operation_snapshot(
            StockSnapshotRequest::new(
                r.binding.clone(),
                minimum,
                original.attempt().operation_id(),
                original.attempt().command_digest(),
            ),
            r.enrollment.proof(),
        )
        .await?;
    Ok(snapshot.recipients()[0].replay_admission())
}
async fn pools(r: &Rig) -> Vec<(Vec<u8>, i64, Vec<u8>)> {
    let mut connection = database(r).await;
    let values =
        sqlx::query_as("SELECT pool_id,revision,protected FROM stock_pools ORDER BY pool_id")
            .fetch_all(&mut connection)
            .await
            .unwrap();
    connection.close().await.unwrap();
    values
}
#[tokio::test]
async fn current_permit_follows_one_durable_admission_and_reauthorization_preserves_original_marker_and_stock()
 {
    let mut r = Rig::new().await;
    let (original, claim) = prepared_claim(&r).await;
    let recipient = original.rumor().recipients()[0];
    assert!(marker(&r, &original).await.unwrap().is_none());
    let before = inventory_minimum(&r).await;
    let stock = pools(&r).await;
    let permit = authorize(&r, &original, recipient).await.unwrap();
    let first = permit
        .admission()
        .expect("current permit must follow committed admission");
    assert_eq!(first.signing_attempt_id(), claim);
    assert_eq!(first.recipient(), recipient);
    assert_eq!(
        first.attempt().operation_id(),
        original.attempt().operation_id()
    );
    assert_eq!(
        first.attempt().command_digest(),
        original.attempt().command_digest()
    );
    assert!(first.attempt().transaction_id().is_some());
    assert_eq!(
        first.checkpoint().revision().get(),
        before.revision().get() + 1
    );
    assert_eq!(marker(&r, &original).await.unwrap(), Some(first));
    assert_eq!(pools(&r).await, stock);
    drop(permit);
    r.clock.0.store(151, Ordering::SeqCst);
    let again = authorize(&r, &original, recipient).await.unwrap();
    assert_eq!(again.admission(), Some(first));
    assert_eq!(again.authorized_at(), UnixSeconds::new(151).unwrap());
    assert_eq!(inventory_minimum(&r).await, first.checkpoint());
    drop(again);
    reopen_plain(&mut r).await;
    assert_eq!(marker(&r, &original).await.unwrap(), Some(first));
    assert_eq!(pools(&r).await, stock);
    let mut connection = database(&r).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_replay_admissions")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let movements:i64=sqlx::query_scalar("SELECT count(*) FROM stock_movements WHERE operation_id IN (SELECT entry_id FROM stock_replay_admissions)").fetch_one(&mut connection).await.unwrap();
    assert_eq!(movements, 0);
    connection.close().await.unwrap();
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn every_replay_admission_write_failure_returns_no_permit_and_rolls_back_original_state() {
    for fault in [
        "CREATE TRIGGER replay_fault BEFORE INSERT ON stock_operations WHEN NEW.family=9 BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER replay_fault BEFORE INSERT ON stock_operation_parts BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER replay_fault BEFORE INSERT ON stock_replay_admissions BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER replay_fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'fixture'); END",
    ] {
        let r = Rig::new().await;
        let (original, _) = prepared_claim(&r).await;
        let recipient = original.rumor().recipients()[0];
        let before = inventory_minimum(&r).await;
        let stock = pools(&r).await;
        let mut connection = database(&r).await;
        sqlx::query(fault).execute(&mut connection).await.unwrap();
        assert_eq!(
            authorize(&r, &original, recipient).await.unwrap_err(),
            OperationError::StorageFailure
        );
        assert_eq!(inventory_minimum(&r).await, before);
        assert!(marker(&r, &original).await.unwrap().is_none());
        assert_eq!(pools(&r).await, stock);
        sqlx::query("DROP TRIGGER replay_fault")
            .execute(&mut connection)
            .await
            .unwrap();
        connection.close().await.unwrap();
        assert!(
            authorize(&r, &original, recipient)
                .await
                .unwrap()
                .admission()
                .is_some()
        );
        assert_pool(&r, 80, 9, 1, 2).await;
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn uncertain_replay_admission_returns_no_permit_fences_owner_and_reopens_original_claim() {
    let mut r = Rig::new().await;
    let (original, claim) = prepared_claim(&r).await;
    let recipient = original.rumor().recipients()[0];
    let stock = pools(&r).await;
    let minimum = inventory_minimum(&r).await;
    let mut connection = database(&r).await;
    sqlx::raw_sql("CREATE TABLE replay_deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER replay_fault AFTER INSERT ON stock_replay_admissions BEGIN INSERT INTO replay_deferred_fault VALUES(2); END").execute(&mut connection).await.unwrap();
    assert_eq!(
        r.store
            .authorize_stock_exact_replay(
                StockClaimRequest::new(
                    r.binding.clone(),
                    minimum,
                    original.attempt().operation_id(),
                    original.attempt().command_digest(),
                    recipient
                ),
                r.enrollment.proof()
            )
            .await
            .unwrap_err(),
        OperationError::RecoveryRequired
    );
    assert_eq!(
        r.store
            .authorize_stock_exact_replay(
                StockClaimRequest::new(
                    r.binding.clone(),
                    minimum,
                    original.attempt().operation_id(),
                    original.attempt().command_digest(),
                    recipient
                ),
                r.enrollment.proof()
            )
            .await
            .unwrap_err(),
        OperationError::RecoveryRequired
    );
    sqlx::raw_sql("DROP TRIGGER replay_fault; DROP TABLE replay_deferred_fault")
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
    reopen_plain(&mut r).await;
    assert!(marker(&r, &original).await.unwrap().is_none());
    assert_eq!(pools(&r).await, stock);
    let permit = authorize(&r, &original, recipient).await.unwrap();
    assert_eq!(permit.report().attempt_id(), claim);
    assert_eq!(permit.admission().unwrap().signing_attempt_id(), claim);
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn replay_admission_projection_and_protected_body_damage_are_never_read_as_absence_or_permission()
 {
    for fault in [
        "DELETE FROM stock_replay_admissions",
        "UPDATE stock_replay_admissions SET recipient=randomblob(32)",
        "UPDATE stock_operation_parts SET protected=randomblob(length(protected)) WHERE operation_id IN (SELECT entry_id FROM stock_replay_admissions)",
    ] {
        let r = Rig::new().await;
        let (original, _) = prepared_claim(&r).await;
        let recipient = original.rumor().recipients()[0];
        drop(authorize(&r, &original, recipient).await.unwrap());
        let minimum = inventory_minimum(&r).await;
        let mut connection = database(&r).await;
        sqlx::query(fault).execute(&mut connection).await.unwrap();
        connection.close().await.unwrap();
        assert_eq!(
            marker_at(&r, &original, minimum).await.unwrap_err().code(),
            "recovery_required",
            "{fault}"
        );
        assert_eq!(
            r.store
                .authorize_stock_exact_replay(
                    StockClaimRequest::new(
                        r.binding.clone(),
                        minimum,
                        original.attempt().operation_id(),
                        original.attempt().command_digest(),
                        recipient
                    ),
                    r.enrollment.proof()
                )
                .await
                .unwrap_err()
                .code(),
            "recovery_required",
            "{fault}"
        );
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn existing_replay_marker_never_renews_the_original_window_or_allows_a_backwards_clock() {
    let r = Rig::new().await;
    let (original, _) = prepared_claim(&r).await;
    let recipient = original.rumor().recipients()[0];
    r.clock.0.store(160, Ordering::SeqCst);
    let first = authorize(&r, &original, recipient)
        .await
        .unwrap()
        .admission()
        .unwrap();
    r.clock.0.store(159, Ordering::SeqCst);
    assert!(authorize(&r, &original, recipient).await.is_err());
    r.clock
        .0
        .store(original.first_issue_deadline().get(), Ordering::SeqCst);
    assert_eq!(
        authorize(&r, &original, recipient).await.unwrap_err(),
        OperationError::DeadlineExceeded
    );
    r.clock.0.store(160, Ordering::SeqCst);
    assert_eq!(marker(&r, &original).await.unwrap(), Some(first));
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn retained_13_14_replay_stays_untracked_without_migration_or_stock_effect() {
    for catalog in [13, 14] {
        let mut r = Rig::with_catalog(catalog).await;
        let (original, claim) = prepared_claim(&r).await;
        let recipient = original.rumor().recipients()[0];
        let before = inventory_minimum(&r).await;
        let stock = pools(&r).await;
        let permit = authorize(&r, &original, recipient).await.unwrap();
        assert_eq!(permit.report().attempt_id(), claim);
        assert!(permit.admission().is_none());
        drop(permit);
        assert_eq!(inventory_minimum(&r).await, before);
        assert!(marker(&r, &original).await.unwrap().is_none());
        assert_eq!(pools(&r).await, stock);
        reopen_plain(&mut r).await;
        let permit = authorize(&r, &original, recipient).await.unwrap();
        assert!(permit.admission().is_none());
        let (request, report) = permit.into_recovery_request(call(&r, 250000)).unwrap();
        assert_eq!(report.attempt_id(), claim);
        let expected = request.original().with_event(|_, id| id);
        let outcome = signer().recover_exact(request).await;
        assert_eq!(outcome.binding(), report.binding());
        let (_, ExactRecoveryKind::Obtained(signed)) = outcome.into_parts() else {
            panic!("retained exact local replay");
        };
        assert_eq!(signed.event_id(), expected);
        assert_eq!(inventory_minimum(&r).await, before);
        assert_eq!(pools(&r).await, stock);
        let mut connection = database(&r).await;
        let actual: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(actual, catalog);
        let markers: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_schema WHERE name='stock_replay_admissions'",
        )
        .fetch_one(&mut connection)
        .await
        .unwrap();
        assert_eq!(markers, 0);
        connection.close().await.unwrap();
        assert_pool(&r, 80, 9, 1, 2).await;
        r.store.close().await.unwrap();
    }
}
