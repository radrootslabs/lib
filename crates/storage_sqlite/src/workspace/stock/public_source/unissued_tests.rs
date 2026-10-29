//! Actual catalog14 original-negative transactions; no release authority.
use super::*;
use crate::workspace::{StockUnissuedInvocation, StockUnissuedOutcome, StockUnissuedReceipt};

fn committed(value: StockUnissuedOutcome) -> StockUnissuedReceipt {
    let StockUnissuedOutcome::Committed(value) = value else {
        panic!("confirmed original-negative append")
    };
    *value
}
async fn observation(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
    mode: usize,
) -> StockUnissuedInvocation {
    let recipient = receipt.rumor().recipients()[0];
    let StockClaimOutcome::Fresh(permit) = claim(r, receipt, recipient).await.unwrap() else {
        panic!("fresh original claim")
    };
    CLOCK_MODE.store(mode, Ordering::SeqCst);
    CLOCK_CALLS.store(0, Ordering::SeqCst);
    let signer =
        LocalSigner::with_clock(SecretKey::parse(&format!("{:064x}", 3)).unwrap(), clock).unwrap();
    let cancel = CancellationSignal::new();
    if mode == 3 {
        cancel.cancel();
    }
    let StockDispatchOutcome::Unissued(value) = permit.dispatch(&signer, cancel).await.unwrap()
    else {
        panic!("actual original local refusal")
    };
    assert_eq!(CLOCK_CALLS.load(Ordering::SeqCst), 1);
    value
}
async fn capture(
    r: &Rig,
    value: &StockUnissuedInvocation,
) -> Result<StockUnissuedOutcome, OperationError> {
    r.store
        .capture_stock_original_unissued(r.binding.clone(), value, r.enrollment.proof())
        .await
}
async fn read_fact(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
) -> Result<Option<StockUnissuedReceipt>, OperationError> {
    r.store
        .stock_original_unissued(
            selector(r, receipt, receipt.rumor().recipients()[0]).await,
            r.enrollment.proof(),
        )
        .await
}
#[tokio::test]
async fn retained_v14_original_refusals_are_durable_idempotent_and_keep_holds_without_clock() {
    for mode in 0..4 {
        let mut r = Rig::with_catalog(14).await;
        let receipt = ready(&r).await;
        assert!(read_fact(&r, &receipt).await.unwrap().is_none());
        let value = observation(&r, &receipt, mode).await;
        let before = inventory_minimum(&r).await;
        r.clock.0.store(u64::MAX, Ordering::SeqCst);
        assert!(r.clock.now().is_err());
        let captured = committed(capture(&r, &value).await.unwrap());
        assert_eq!(captured.signing_attempt_id(), value.attempt().attempt_id());
        assert_eq!(captured.reason(), value.reason());
        assert_eq!(captured.recipient(), value.attempt().recipient());
        assert_eq!(
            captured.attempt().operation_id(),
            receipt.attempt().operation_id()
        );
        assert_eq!(
            captured.checkpoint().revision().get(),
            before.revision().get() + 1
        );
        assert!(captured.attempt().transaction_id().is_some());
        assert_eq!(committed(capture(&r, &value).await.unwrap()), captured);
        assert_eq!(CLOCK_CALLS.load(Ordering::SeqCst), 1);
        r.clock.0.store(150, Ordering::SeqCst);
        assert_eq!(read_fact(&r, &receipt).await.unwrap(), Some(captured));
        let minimum = inventory_minimum(&r).await;
        assert_eq!(minimum.revision(), captured.checkpoint().revision());
        let abort = radroots_storage::inventory::StockAbortRequest::new(
            r.binding.clone(),
            minimum,
            receipt.attempt().operation_id(),
            receipt.attempt().command_digest(),
        );
        assert!(matches!(
            radroots_storage::inventory::InventoryStore::abort_proven_unissued(
                &r.store,
                abort,
                r.enrollment
                    .proof()
                    .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                    .unwrap()
            )
            .await,
            Err(OperationError::RecoveryRequired)
        ));
        assert_pool(&r, 80, 9, 1, 2).await;
        assert_pool(&r, 81, 8, 2, 2).await;
        drop(value);
        reopen_plain(&mut r).await;
        assert_eq!(read_fact(&r, &receipt).await.unwrap(), Some(captured));
        assert!(matches!(
            claim(&r, &receipt, captured.recipient()).await.unwrap(),
            StockClaimOutcome::Existing(_)
        ));
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn every_original_negative_write_failure_preserves_owned_proof_and_all_holds() {
    for statement in [
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operations WHEN NEW.family=8 BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operation_parts BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_original_unissued BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'fixture'); END",
    ] {
        let r = Rig::new().await;
        let receipt = ready(&r).await;
        let value = observation(&r, &receipt, 1).await;
        let original = value.attempt().attempt_id();
        let before = inventory_minimum(&r).await;
        let mut raw = database(&r).await;
        sqlx::raw_sql(statement).execute(&mut raw).await.unwrap();
        assert_eq!(
            capture(&r, &value).await.unwrap_err().code(),
            "storage_failure"
        );
        assert_eq!(value.attempt().attempt_id(), original);
        assert_eq!(inventory_minimum(&r).await, before);
        assert!(read_fact(&r, &receipt).await.unwrap().is_none());
        sqlx::raw_sql("DROP TRIGGER fault")
            .execute(&mut raw)
            .await
            .unwrap();
        raw.close().await.unwrap();
        assert_eq!(
            committed(capture(&r, &value).await.unwrap()).signing_attempt_id(),
            original
        );
        assert_eq!(CLOCK_CALLS.load(Ordering::SeqCst), 1);
        assert_pool(&r, 80, 9, 1, 2).await;
        assert_pool(&r, 81, 8, 2, 2).await;
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn unknown_original_negative_commit_reconciles_same_owner_without_clock_or_reissue() {
    let r = Rig::new().await;
    let receipt = ready(&r).await;
    let value = observation(&r, &receipt, 1).await;
    let original = value.attempt().attempt_id();
    let mut raw = database(&r).await;
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_original_unissued BEGIN INSERT INTO deferred_fault VALUES(2); END")
        .execute(&mut raw).await.unwrap();
    let StockUnissuedOutcome::Unknown(attempt) = capture(&r, &value).await.unwrap() else {
        panic!("real deferred COMMIT failure")
    };
    assert!(attempt.transaction_id().is_some());
    assert!(matches!(
        capture(&r, &value).await,
        Err(OperationError::RecoveryRequired)
    ));
    assert_eq!(value.attempt().attempt_id(), original);
    sqlx::raw_sql("DROP TRIGGER fault; DROP TABLE deferred_fault")
        .execute(&mut raw)
        .await
        .unwrap();
    raw.close().await.unwrap();
    r.clock.0.store(u64::MAX, Ordering::SeqCst);
    assert!(r.clock.now().is_err());
    let captured = committed(
        r.store
            .reconcile_stock_original_unissued(r.binding.clone(), &value, r.enrollment.proof())
            .await
            .unwrap(),
    );
    assert_eq!(captured.signing_attempt_id(), original);
    r.clock.0.store(150, Ordering::SeqCst);
    assert_eq!(read_fact(&r, &receipt).await.unwrap(), Some(captured));
    assert_eq!(CLOCK_CALLS.load(Ordering::SeqCst), 1);
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn original_negative_projections_and_protected_bodies_are_authenticated_after_reopen() {
    for statement in [
        "UPDATE stock_original_unissued SET reason=CASE WHEN reason=8 THEN 7 ELSE 8 END",
        "DELETE FROM stock_original_unissued",
        "UPDATE stock_operation_parts SET protected=randomblob(length(protected)) WHERE operation_id IN (SELECT entry_id FROM stock_original_unissued)",
    ] {
        let mut r = Rig::new().await;
        let receipt = ready(&r).await;
        let value = observation(&r, &receipt, 1).await;
        committed(capture(&r, &value).await.unwrap());
        r.store.close().await.unwrap();
        let mut raw = database(&r).await;
        sqlx::raw_sql(statement).execute(&mut raw).await.unwrap();
        raw.close().await.unwrap();
        match NativeWorkspace::open_seller_stock(
            &r.path,
            r.enrollment.input().clone(),
            r.fixture.crypto(),
            r.clock.clone(),
        )
        .await
        {
            Ok(store) => {
                r.store = store;
                assert!(
                    read_fact(&r, &receipt).await.is_err(),
                    "tampered original-negative ancestry must refuse"
                );
                r.store.close().await.unwrap();
            }
            Err(error) => assert!(matches!(error, WorkspaceError::RecoveryRequired)),
        }
    }
}
#[tokio::test]
async fn later_valid_evidence_does_not_erase_original_refusal_or_release_backing() {
    let r = Rig::new().await;
    let receipt = ready(&r).await;
    let value = observation(&r, &receipt, 1).await;
    let negative = committed(capture(&r, &value).await.unwrap());
    // Controlled independent late positive input; not a second dispatch permit
    // or qualification of an external key owner or recovery adapter.
    fn late_clock() -> Result<u64, radroots_signing::Error> {
        Ok(150000)
    }
    let report = value.attempt();
    let seal = report
        .bound_plan()
        .plan()
        .seals()
        .iter()
        .find(|v| v.recipient() == report.recipient())
        .unwrap();
    let intent = radroots_signing::issuance::SigningIntent::from_commerce_seal(
        seal,
        radroots_signing::provider::ProviderCall::new(
            seal.fields().author,
            core::num::NonZeroU64::new(
                report
                    .bound_plan()
                    .preparation()
                    .first_issue_deadline()
                    .get()
                    * 1000,
            )
            .unwrap(),
            CancellationSignal::new(),
        ),
    )
    .unwrap();
    let signer = LocalSigner::with_clock(
        SecretKey::parse(&format!("{:064x}", 3)).unwrap(),
        late_clock,
    )
    .unwrap();
    let (_, OutcomeKind::Obtained(artifact)) =
        radroots_signing::provider::ParticipantProvider::sign_exact(&signer, intent)
            .await
            .into_parts()
    else {
        panic!("independent valid original signature")
    };
    let evidence = artifact
        .with_json(|raw| {
            crate::workspace::StockEvidenceCapture::verify(
                r.binding.clone(),
                report,
                raw,
                artifact.observed_at_unix_ms(),
            )
        })
        .unwrap();
    let radroots_storage::inventory::StockEvidenceOutcome::Committed(positive) = r
        .store
        .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
        .await
        .unwrap()
    else {
        panic!("late evidence retained")
    };
    assert_eq!(positive.signing_attempt_id(), negative.signing_attempt_id());
    assert_eq!(read_fact(&r, &receipt).await.unwrap(), Some(negative));
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn retained_original_refusal_survives_expired_and_revoked_permission_without_pool_effects() {
    for expired in [false, true] {
        let r = Rig::new().await;
        let receipt = ready(&r).await;
        let value = observation(&r, &receipt, 1).await;
        let read_request = selector(&r, &receipt, value.attempt().recipient()).await;
        let mut raw = database(&r).await;
        let before: Vec<(Vec<u8>, i64, Vec<u8>)> =
            sqlx::query_as("SELECT pool_id,revision,protected FROM stock_pools ORDER BY pool_id")
                .fetch_all(&mut raw)
                .await
                .unwrap();
        let movements: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_movements")
            .fetch_one(&mut raw)
            .await
            .unwrap();
        if expired {
            r.clock.0.store(350, Ordering::SeqCst);
        } else {
            r.store
                .revoke_stock_grant(
                    r.binding.clone(),
                    inventory_minimum(&r).await,
                    r.enrollment.proof(),
                    radroots_domain::TransactionId::from_bytes([94; 32]),
                    r.enrollment.input().grant_id(),
                )
                .await
                .unwrap();
        }
        assert!(
            r.store
                .stock_original_unissued(read_request, r.enrollment.proof())
                .await
                .is_err(),
            "fresh read permission really refuses"
        );
        let retained = committed(capture(&r, &value).await.unwrap());
        assert_eq!(committed(capture(&r, &value).await.unwrap()), retained);
        assert_eq!(retained.signing_attempt_id(), value.attempt().attempt_id());
        let after: Vec<(Vec<u8>, i64, Vec<u8>)> =
            sqlx::query_as("SELECT pool_id,revision,protected FROM stock_pools ORDER BY pool_id")
                .fetch_all(&mut raw)
                .await
                .unwrap();
        assert_eq!(
            after, before,
            "all protected pool bytes and versions unchanged"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_movements")
                .fetch_one(&mut raw)
                .await
                .unwrap(),
            movements
        );
        assert_eq!(CLOCK_CALLS.load(Ordering::SeqCst), 1);
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn lost_original_negative_append_waiter_drains_and_reopens_the_same_fact() {
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    let value = observation(&r, &receipt, 1).await;
    let original = value.attempt().attempt_id();
    let reason = value.reason();
    let provider = reopen(&mut r).await;
    provider.gate.arm(Direction::Wrap, r.authority_schema(), 0);
    let mut pending = Box::pin(capture(&r, &value));
    assert!(futures::poll!(pending.as_mut()).is_pending());
    provider.gate.entered();
    drop(pending);
    drop(value);
    provider.gate.release();
    reopen_plain(&mut r).await;
    let retained = read_fact(&r, &receipt).await.unwrap().unwrap();
    assert_eq!(retained.signing_attempt_id(), original);
    assert_eq!(retained.reason(), reason);
    assert_eq!(CLOCK_CALLS.load(Ordering::SeqCst), 1);
    assert!(matches!(
        claim(&r, &receipt, retained.recipient()).await.unwrap(),
        StockClaimOutcome::Existing(_)
    ));
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}
