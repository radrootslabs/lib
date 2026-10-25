//! Actual claim transactions; only a newly confirmed claim returns a permit.
use super::*;
#[path = "evidence_tests.rs"]
mod evidence;
use crate::workspace::{StockClaimOutcome, StockClaimRequest, StockSigningAttempt};
use radroots_signing::issuance::OutcomeKind;

async fn selector(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
    recipient: PublicKey,
) -> StockClaimRequest {
    StockClaimRequest::new(
        r.binding.clone(),
        inventory_minimum(r).await,
        receipt.attempt().operation_id(),
        receipt.attempt().command_digest(),
        recipient,
    )
}
async fn claim(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
    recipient: PublicKey,
) -> Result<StockClaimOutcome, OperationError> {
    r.store
        .claim_stock_signing_attempt(selector(r, receipt, recipient).await, r.enrollment.proof())
        .await
}
async fn read(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
    recipient: PublicKey,
) -> Result<Option<StockSigningAttempt>, OperationError> {
    r.store
        .stock_signing_attempt(selector(r, receipt, recipient).await, r.enrollment.proof())
        .await
}
async fn ready(r: &Rig) -> StockAcceptanceReceipt {
    let (_, receipt) = prepared(r).await;
    bound(bind(r, &receipt, 42).await.unwrap());
    receipt
}

#[tokio::test]
async fn fresh_claim_refuses_damaged_current_pool_backing() {
    let r = Rig::new().await;
    let receipt = ready(&r).await;
    let mut raw = database(&r).await;
    sqlx::raw_sql("UPDATE stock_pools SET protected=randomblob(length(protected))")
        .execute(&mut raw)
        .await
        .unwrap();
    assert!(
        claim(&r, &receipt, receipt.rumor().recipients()[0])
            .await
            .is_err(),
        "current backing must be authenticated before a new dispatch permit"
    );
    raw.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn fresh_t1_and_claim_accept_legitimate_available_stock_changes_without_reallocating() {
    let r = Rig::new().await;
    let (_, receipt) = prepared(&r).await;
    physical(&r,serde_json::json!({"type":"stock.adjust","context":context(&r,95),"input":{"pool_id":pool(80),"expected_version":"2","direction":"add","amount":"1","reason":"restock available backing"}})).await;
    bound(bind(&r, &receipt, 42).await.unwrap());
    physical(&r,serde_json::json!({"type":"stock.adjust","context":context(&r,96),"input":{"pool_id":pool(81),"expected_version":"2","direction":"remove","amount":"1","reason":"remove available backing"}})).await;
    assert!(matches!(
        claim(&r, &receipt, receipt.rumor().recipients()[0])
            .await
            .unwrap(),
        StockClaimOutcome::Fresh(_)
    ));
    assert_pool(&r, 80, 10, 1, 3).await;
    let value = r
        .store
        .pool(
            r.binding.clone(),
            inventory_minimum(&r).await,
            pool(81),
            r.enrollment
                .proof()
                .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                .unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    let snapshot = value.snapshot();
    let physical = snapshot.balances().physical();
    assert_eq!(snapshot.version().get(), 3);
    assert_eq!(snapshot.balances().introduced().get(), 10);
    assert_eq!(
        (
            physical.available.get(),
            physical.preparatory_hold.get(),
            physical.removed.get()
        ),
        (7, 2, 1)
    );
    r.store.close().await.unwrap();
}
async fn reopen_plain(r: &mut Rig) {
    r.store.close().await.unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn each_recipient_gets_one_confirmed_permit_and_original_reopen_never_redispatches() {
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    let signer = LocalSigner::with_clock(SecretKey::parse(&format!("{:064x}", 3)).unwrap(), || {
        Ok(150000)
    })
    .unwrap();
    let mut originals = Vec::new();
    for recipient in receipt.rumor().recipients() {
        assert!(read(&r, &receipt, *recipient).await.unwrap().is_none());
        let before = inventory_minimum(&r).await;
        let StockClaimOutcome::Fresh(permit) = claim(&r, &receipt, *recipient).await.unwrap()
        else {
            panic!("new committed claim must return a permit");
        };
        assert_eq!(permit.report().recipient(), *recipient);
        let (intent, report) = permit
            .into_signing_intent(CancellationSignal::new())
            .unwrap();
        let actual = signer.sign_exact(intent).await;
        assert_eq!(actual.binding(), report.binding());
        let (_, OutcomeKind::Obtained(artifact)) = actual.into_parts() else {
            panic!("actual exact signer must obtain bytes");
        };
        artifact.with_json(|raw| assert!(raw.contains("\"sig\"")));
        let after = inventory_minimum(&r).await;
        assert_eq!(after.revision().get(), before.revision().get() + 1);
        let StockClaimOutcome::Existing(again) = claim(&r, &receipt, *recipient).await.unwrap()
        else {
            panic!("existing claim must not return another permit");
        };
        assert_eq!(again.attempt_id(), report.attempt_id());
        assert_eq!(again.binding(), report.binding());
        assert_eq!(inventory_minimum(&r).await, after);
        originals.push(report);
    }
    assert_ne!(originals[0].attempt_id(), originals[1].attempt_id());
    reopen_plain(&mut r).await;
    r.clock.0.store(229, Ordering::SeqCst);
    for report in originals {
        let StockClaimOutcome::Existing(again) =
            claim(&r, &receipt, report.recipient()).await.unwrap()
        else {
            panic!("reopen must not redispatch");
        };
        assert_eq!(again.attempt(), report.attempt());
        assert_eq!(again.binding(), report.binding());
        assert_eq!(
            again.bound_plan().plan().protected_wire(),
            report.bound_plan().plan().protected_wire()
        );
    }
    assert_pool(&r, 80, 9, 1, 2).await;
    assert_pool(&r, 81, 8, 2, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn claim_requires_original_t1_and_refuses_wrong_identity_digest_recipient_and_expiry() {
    let r = Rig::new().await;
    let (_, receipt) = prepared(&r).await;
    let recipient = receipt.rumor().recipients()[0];
    assert_eq!(
        claim(&r, &receipt, recipient).await.unwrap_err().code(),
        "stale_state"
    );
    bound(bind(&r, &receipt, 42).await.unwrap());
    let minimum = inventory_minimum(&r).await;
    for (operation, digest, recipient, code) in [
        (
            OperationId::from_bytes([99; 32]),
            receipt.attempt().command_digest(),
            recipient,
            "stale_state",
        ),
        (
            receipt.attempt().operation_id(),
            radroots_domain::CommandDigest::from_bytes([99; 32]),
            recipient,
            "idempotency_conflict",
        ),
        (
            receipt.attempt().operation_id(),
            receipt.attempt().command_digest(),
            participant(),
            "invalid_command",
        ),
    ] {
        let request =
            StockClaimRequest::new(r.binding.clone(), minimum, operation, digest, recipient);
        assert_eq!(
            r.store
                .claim_stock_signing_attempt(request, r.enrollment.proof())
                .await
                .unwrap_err()
                .code(),
            code
        );
    }
    r.clock.0.store(230, Ordering::SeqCst);
    assert_eq!(
        claim(&r, &receipt, recipient).await.unwrap_err().code(),
        "deadline_exceeded"
    );
    r.clock.0.store(150, Ordering::SeqCst);
    assert!(read(&r, &receipt, recipient).await.unwrap().is_none());
    assert_eq!(inventory_minimum(&r).await, minimum);
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn each_actual_claim_write_failure_rolls_back_without_dispatch_permission() {
    for statement in [
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operations WHEN NEW.family=4 BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operation_parts BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_signing_claims BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'fixture'); END",
    ] {
        let r = Rig::new().await;
        let receipt = ready(&r).await;
        let recipient = receipt.rumor().recipients()[0];
        let before = inventory_minimum(&r).await;
        let mut raw = database(&r).await;
        sqlx::raw_sql(statement).execute(&mut raw).await.unwrap();
        assert_eq!(
            claim(&r, &receipt, recipient).await.unwrap_err().code(),
            "storage_failure"
        );
        assert_eq!(inventory_minimum(&r).await, before);
        assert!(read(&r, &receipt, recipient).await.unwrap().is_none());
        sqlx::raw_sql("DROP TRIGGER fault")
            .execute(&mut raw)
            .await
            .unwrap();
        assert!(matches!(
            claim(&r, &receipt, recipient).await.unwrap(),
            StockClaimOutcome::Fresh(_)
        ));
        assert_pool(&r, 80, 9, 1, 2).await;
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn unknown_claim_commit_fences_the_owner_and_requires_actual_reopen_reconciliation() {
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    let recipient = receipt.rumor().recipients()[0];
    let before = inventory_minimum(&r).await;
    let request = selector(&r, &receipt, recipient).await;
    let mut raw = database(&r).await;
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_signing_claims BEGIN INSERT INTO deferred_fault VALUES(2); END").execute(&mut raw).await.unwrap();
    let StockClaimOutcome::Unknown(anchor) = r
        .store
        .claim_stock_signing_attempt(request, r.enrollment.proof())
        .await
        .unwrap()
    else {
        panic!("uncertain COMMIT cannot authorize dispatch");
    };
    assert_eq!(anchor.operation_id(), receipt.attempt().operation_id());
    assert!(anchor.transaction_id().is_some());
    let request = StockClaimRequest::new(
        r.binding.clone(),
        before,
        receipt.attempt().operation_id(),
        receipt.attempt().command_digest(),
        recipient,
    );
    assert_eq!(
        r.store
            .stock_signing_attempt(request, r.enrollment.proof())
            .await
            .unwrap_err()
            .code(),
        "recovery_required"
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
    assert!(read(&r, &receipt, recipient).await.unwrap().is_none());
    assert_eq!(inventory_minimum(&r).await, before);
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn lost_claim_observer_drains_and_reopens_as_existing_without_a_permit() {
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    let recipient = receipt.rumor().recipients()[0];
    let request = selector(&r, &receipt, recipient).await;
    let provider = reopen(&mut r).await;
    provider
        .gate
        .arm(Direction::Wrap, "radroots.seller_stock.authority.v12", 0);
    let mut pending = Box::pin(
        r.store
            .claim_stock_signing_attempt(request, r.enrollment.proof()),
    );
    assert!(futures::poll!(pending.as_mut()).is_pending());
    provider.gate.entered();
    drop(pending);
    assert!(matches!(
        NativeWorkspace::open_seller_stock(
            &r.path,
            r.enrollment.input().clone(),
            r.fixture.crypto(),
            r.clock.clone()
        )
        .await,
        Err(WorkspaceError::WriterActive)
    ));
    provider.gate.release();
    reopen_plain(&mut r).await;
    assert!(matches!(
        claim(&r, &receipt, recipient).await.unwrap(),
        StockClaimOutcome::Existing(_)
    ));
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn protected_claim_or_projection_damage_never_becomes_permission_to_reissue() {
    for statement in [
        "DELETE FROM stock_signing_claims",
        "UPDATE stock_signing_claims SET recipient=randomblob(32)",
        "UPDATE stock_operations SET protected=randomblob(length(protected)) WHERE family=4",
        "DELETE FROM stock_operation_parts WHERE operation_id IN (SELECT operation_id FROM stock_operations WHERE family=4)",
        "PRAGMA foreign_keys=OFF; DELETE FROM stock_operation_parts WHERE operation_id IN (SELECT operation_id FROM stock_operations WHERE family=4); DELETE FROM stock_signing_claims; DELETE FROM stock_operations WHERE family=4",
    ] {
        let r = Rig::new().await;
        let receipt = ready(&r).await;
        let recipient = receipt.rumor().recipients()[0];
        assert!(matches!(
            claim(&r, &receipt, recipient).await.unwrap(),
            StockClaimOutcome::Fresh(_)
        ));
        let request = selector(&r, &receipt, recipient).await;
        let mut raw = database(&r).await;
        sqlx::raw_sql(statement).execute(&mut raw).await.unwrap();
        assert!(
            r.store
                .claim_stock_signing_attempt(request, r.enrollment.proof())
                .await
                .is_err(),
            "{statement}"
        );
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn claim_rechecks_original_plan_projection_grant_root_and_clock_after_provider_io() {
    for statement in [
        "UPDATE stock_pools SET protected=randomblob(length(protected))",
        "UPDATE stock_grants SET protected=randomblob(length(protected))",
        "UPDATE stock_operations SET protected=randomblob(length(protected)) WHERE family=3",
        "UPDATE stock_authority SET protected=randomblob(length(protected))",
        "clock_expired",
        "clock_rollback",
    ] {
        let mut r = Rig::new().await;
        let receipt = ready(&r).await;
        let recipient = receipt.rumor().recipients()[0];
        let request = selector(&r, &receipt, recipient).await;
        let provider = reopen(&mut r).await;
        let mut raw = database(&r).await;
        provider
            .gate
            .arm(Direction::Wrap, "radroots.seller_stock.authority.v12", 0);
        let mut pending = Box::pin(
            r.store
                .claim_stock_signing_attempt(request, r.enrollment.proof()),
        );
        assert!(futures::poll!(pending.as_mut()).is_pending());
        provider.gate.entered();
        match statement {
            "clock_expired" => r.clock.0.store(230, Ordering::SeqCst),
            "clock_rollback" => r.clock.0.store(149, Ordering::SeqCst),
            _ => {
                sqlx::raw_sql(statement).execute(&mut raw).await.unwrap();
            }
        }
        provider.gate.release();
        assert!(pending.await.is_err(), "{statement}");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_operations WHERE family=4")
                .fetch_one(&mut raw)
                .await
                .unwrap(),
            0
        );
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn replacement_recovery_only_can_read_original_claims_but_cannot_create_missing_claims() {
    let r = Rig::new().await;
    let (command, receipt) = prepared(&r).await;
    bound(bind(&r, &receipt, 42).await.unwrap());
    let first = receipt.rumor().recipients()[0];
    let missing = receipt.rumor().recipients()[1];
    let StockClaimOutcome::Fresh(permit) = claim(&r, &receipt, first).await.unwrap() else {
        panic!("original claim");
    };
    let original = permit.report().clone();
    drop(permit);
    r.clock.0.store(250, Ordering::SeqCst);
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
    for recipient in [first, missing] {
        let request = StockClaimRequest::new(
            binding.clone(),
            inventory_minimum(&r).await,
            receipt.attempt().operation_id(),
            receipt.attempt().command_digest(),
            recipient,
        );
        if recipient == first {
            let StockClaimOutcome::Existing(value) = r
                .store
                .claim_stock_signing_attempt(request, child.proof())
                .await
                .unwrap()
            else {
                panic!("recovery cannot produce a permit");
            };
            assert_eq!(value.attempt_id(), original.attempt_id());
            assert_eq!(value.binding(), original.binding());
        } else {
            assert_eq!(
                r.store
                    .claim_stock_signing_attempt(request, child.proof())
                    .await
                    .unwrap_err()
                    .code(),
                "unauthorized"
            );
        }
    }
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}
