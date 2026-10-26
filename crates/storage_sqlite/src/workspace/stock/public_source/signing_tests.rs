//! Actual T1 retention, error windows and original recovery. No stock release
//! or firm agreement is inferred from a bound signing plan.
use super::*;
#[path = "claim_tests.rs"]
mod claims;
use crate::workspace::{StockBoundSigningPlan, StockSigningPlanOutcome, StockSigningPlanRequest};
use radroots_protocol::{
    commerce::{CommerceSeal, CommerceSigningPlan},
    intent::SigningReplay,
    nip59::CapturedDmRoute,
};
use radroots_signing::commerce::VerifiedCommerceSigningPlan;
use radroots_signing::provider::{NonceSource, ParticipantProvider, PrivateText, ProviderError};
struct TestNonce(u8);
impl NonceSource for TestNonce {
    fn next_nonce(&self) -> Result<[u8; 32], ProviderError> {
        Ok([self.0; 32])
    }
}
async fn candidate(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
    nonce: u8,
) -> VerifiedCommerceSigningPlan {
    candidate_with_replay(r, receipt, nonce, SigningReplay::NonReplayable).await
}
async fn candidate_with_replay(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
    nonce: u8,
    replay: SigningReplay,
) -> VerifiedCommerceSigningPlan {
    let provider =
        LocalSigner::with_clock(SecretKey::parse(&format!("{:064x}", 3)).unwrap(), || {
            Ok(150000)
        })
        .unwrap()
        .with_nonce_source(Arc::new(TestNonce(nonce)));
    assert_eq!(provider.public_key(), r.enrollment.input().participant());
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/commerce_signing.v1.json"
    ))
    .unwrap();
    let rumor = receipt.rumor();
    let call = || {
        ProviderCall::new(
            rumor.author(),
            NonZeroU64::new(receipt.first_issue_deadline().get() * 1000).unwrap(),
            CancellationSignal::new(),
        )
    };
    let mut seals = Vec::new();
    let mut routes = Vec::new();
    for recipient in rumor.recipients() {
        let encrypted = provider
            .encrypt_private(
                call(),
                *recipient,
                PrivateText::new(rumor.protected_wire()).unwrap(),
            )
            .await
            .unwrap();
        seals.push(
            CommerceSeal::new(
                receipt.attempt().operation_id(),
                receipt.attempt().command_digest(),
                rumor,
                *recipient,
                receipt.prepared_at(),
                encrypted.as_str(),
            )
            .unwrap(),
        );
        let row = vectors["stock_routes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["recipient"].as_str().unwrap() == recipient.to_hex())
            .unwrap();
        routes.push(
            CapturedDmRoute::from_observed_descriptor(
                *recipient,
                row["event_id"].as_str().unwrap().parse().unwrap(),
                row["raw"].as_str().unwrap(),
                &["wss://relay.example".into()],
            )
            .unwrap(),
        );
    }
    let plan = CommerceSigningPlan::new(
        receipt.attempt().operation_id(),
        receipt.attempt().command_digest(),
        rumor,
        seals.try_into().unwrap(),
        routes.try_into().unwrap(),
        replay,
    )
    .unwrap();
    VerifiedCommerceSigningPlan::verify(&provider, call(), rumor, plan)
        .await
        .unwrap()
}
async fn bind(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
    nonce: u8,
) -> Result<StockSigningPlanOutcome, OperationError> {
    let request = StockSigningPlanRequest::new(
        r.binding.clone(),
        inventory_minimum(r).await,
        candidate(r, receipt, nonce).await,
    )
    .unwrap();
    r.store
        .bind_stock_signing_intents(request, r.enrollment.proof())
        .await
}
fn bound(outcome: StockSigningPlanOutcome) -> StockBoundSigningPlan {
    match outcome {
        StockSigningPlanOutcome::Bound(value) => *value,
        _ => panic!("T1 must confirm"),
    }
}
async fn retained(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
) -> Result<Option<StockBoundSigningPlan>, OperationError> {
    r.store
        .stock_signing_plan(
            r.binding.clone(),
            inventory_minimum(r).await,
            receipt.attempt().operation_id(),
            receipt.attempt().command_digest(),
            r.enrollment.proof(),
        )
        .await
}
async fn prepared(r: &Rig) -> (CapturedOrderAcceptance, StockAcceptanceReceipt) {
    seed(r, "10").await;
    let command = command(r, 90, true).await;
    consent(r, &command).await;
    let receipt = prepare(r, &command).await.unwrap();
    (command, receipt)
}

#[tokio::test]
async fn fresh_t1_refuses_damaged_current_pool_backing() {
    let r = Rig::new().await;
    let (_, receipt) = prepared(&r).await;
    let mut raw = database(&r).await;
    sqlx::raw_sql("UPDATE stock_pools SET protected=randomblob(length(protected))")
        .execute(&mut raw)
        .await
        .unwrap();
    assert!(
        bind(&r, &receipt, 42).await.is_err(),
        "current backing must be authenticated before new intent binding"
    );
    raw.close().await.unwrap();
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn actual_t1_binds_complete_original_peer_self_plan_and_reopens_without_another_hold_or_signer()
 {
    let mut r = Rig::new().await;
    let (command, receipt) = prepared(&r).await;
    assert!(retained(&r, &receipt).await.unwrap().is_none());
    let original = bound(bind(&r, &receipt, 42).await.unwrap());
    assert_eq!(original.plan().seals().len(), 2);
    assert_eq!(original.plan().routes().len(), 2);
    assert_eq!(
        original.checkpoint().revision().get(),
        receipt.checkpoint().revision().get() + 1
    );
    assert_ne!(original.intent_digests()[0], original.intent_digests()[1]);
    let again = bound(bind(&r, &receipt, 42).await.unwrap());
    assert_eq!(again.attempt(), original.attempt());
    assert_eq!(
        again.plan().protected_wire(),
        original.plan().protected_wire()
    );
    assert_eq!(
        bind(&r, &receipt, 43).await.unwrap_err().code(),
        "idempotency_conflict"
    );
    assert_pool(&r, 80, 9, 1, 2).await;
    assert_pool(&r, 81, 8, 2, 2).await;
    r.store.close().await.unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    r.clock.0.store(229, Ordering::SeqCst);
    let recovered = retained(&r, &receipt).await.unwrap().unwrap();
    assert_eq!(recovered.attempt(), original.attempt());
    assert_eq!(recovered.intent_digests(), original.intent_digests());
    assert_eq!(
        recovered.plan().protected_wire(),
        original.plan().protected_wire()
    );
    assert_eq!(
        prepare(&r, &command).await.unwrap().attempt(),
        receipt.attempt()
    );
    assert_pool(&r, 80, 9, 1, 2).await;
    assert_pool(&r, 81, 8, 2, 2).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn t1_refuses_missing_t0_wrong_digest_and_fresh_expiry_while_preserving_existing_holds() {
    let r = Rig::new().await;
    let (_, receipt) = prepared(&r).await;
    let minimum = inventory_minimum(&r).await;
    assert_eq!(
        r.store
            .stock_signing_plan(
                r.binding.clone(),
                minimum,
                OperationId::from_bytes([99; 32]),
                receipt.attempt().command_digest(),
                r.enrollment.proof()
            )
            .await
            .unwrap_err()
            .code(),
        "stale_state"
    );
    assert_eq!(
        r.store
            .stock_signing_plan(
                r.binding.clone(),
                minimum,
                receipt.attempt().operation_id(),
                radroots_domain::CommandDigest::from_bytes([99; 32]),
                r.enrollment.proof()
            )
            .await
            .unwrap_err()
            .code(),
        "idempotency_conflict"
    );
    let request = StockSigningPlanRequest::new(
        r.binding.clone(),
        minimum,
        candidate(&r, &receipt, 42).await,
    )
    .unwrap();
    r.clock.0.store(230, Ordering::SeqCst);
    assert_eq!(
        r.store
            .bind_stock_signing_intents(request, r.enrollment.proof())
            .await
            .unwrap_err()
            .code(),
        "deadline_exceeded"
    );
    r.clock.0.store(150, Ordering::SeqCst);
    assert_eq!(inventory_minimum(&r).await, minimum);
    assert!(retained(&r, &receipt).await.unwrap().is_none());
    assert_pool(&r, 80, 9, 1, 2).await;
    assert_pool(&r, 81, 8, 2, 2).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn each_actual_t1_write_failure_rolls_back_plan_and_keeps_original_stock_claims() {
    for statement in [
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operations WHEN NEW.family=3 BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operation_parts BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'fixture'); END",
    ] {
        let r = Rig::new().await;
        let (_, receipt) = prepared(&r).await;
        let before = inventory_minimum(&r).await;
        let mut raw = database(&r).await;
        sqlx::raw_sql(statement).execute(&mut raw).await.unwrap();
        assert_eq!(
            bind(&r, &receipt, 42).await.unwrap_err().code(),
            "storage_failure"
        );
        assert_eq!(inventory_minimum(&r).await, before);
        assert!(retained(&r, &receipt).await.unwrap().is_none());
        assert_pool(&r, 80, 9, 1, 2).await;
        assert_pool(&r, 81, 8, 2, 2).await;
        sqlx::raw_sql("DROP TRIGGER fault")
            .execute(&mut raw)
            .await
            .unwrap();
        bound(bind(&r, &receipt, 42).await.unwrap());
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn t1_commit_uncertainty_fences_owner_until_drain_and_original_reopen_reconciliation() {
    let mut r = Rig::new().await;
    let (_, receipt) = prepared(&r).await;
    let before = inventory_minimum(&r).await;
    let mut raw = database(&r).await;
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_operations WHEN NEW.family=3 BEGIN INSERT INTO deferred_fault VALUES(2); END").execute(&mut raw).await.unwrap();
    let StockSigningPlanOutcome::Unknown(attempt) = bind(&r, &receipt, 42).await.unwrap() else {
        panic!("COMMIT must remain unknown");
    };
    assert_eq!(attempt.operation_id(), receipt.attempt().operation_id());
    assert!(attempt.transaction_id().is_some());
    assert_eq!(
        r.store
            .stock_signing_plan(
                r.binding.clone(),
                before,
                receipt.attempt().operation_id(),
                receipt.attempt().command_digest(),
                r.enrollment.proof()
            )
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
    assert!(retained(&r, &receipt).await.unwrap().is_none());
    assert_eq!(inventory_minimum(&r).await, before);
    assert_pool(&r, 80, 9, 1, 2).await;
    assert_pool(&r, 81, 8, 2, 2).await;
    bound(bind(&r, &receipt, 42).await.unwrap());
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn dropped_t1_observer_keeps_owned_work_and_close_drains_before_original_reopen() {
    let mut r = Rig::new().await;
    let (_, receipt) = prepared(&r).await;
    let request = StockSigningPlanRequest::new(
        r.binding.clone(),
        inventory_minimum(&r).await,
        candidate(&r, &receipt, 42).await,
    )
    .unwrap();
    let provider = reopen(&mut r).await;
    provider
        .gate
        .arm(Direction::Wrap, "radroots.seller_stock.authority.v14", 0);
    let mut pending = Box::pin(
        r.store
            .bind_stock_signing_intents(request, r.enrollment.proof()),
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
    r.store.close().await.unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    assert!(retained(&r, &receipt).await.unwrap().is_some());
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn t1_final_permission_order_root_and_clock_witnesses_refuse_changes_after_provider_io() {
    for statement in [
        "UPDATE stock_pools SET protected=randomblob(length(protected))",
        "UPDATE stock_grants SET protected=randomblob(length(protected))",
        "UPDATE stock_orders SET protected=randomblob(length(protected))",
        "UPDATE stock_authority SET protected=randomblob(length(protected))",
        "clock_expired",
        "clock_rollback",
    ] {
        let mut r = Rig::new().await;
        let (_, receipt) = prepared(&r).await;
        let request = StockSigningPlanRequest::new(
            r.binding.clone(),
            inventory_minimum(&r).await,
            candidate(&r, &receipt, 42).await,
        )
        .unwrap();
        let provider = reopen(&mut r).await;
        let mut raw = database(&r).await;
        provider
            .gate
            .arm(Direction::Wrap, "radroots.seller_stock.authority.v14", 0);
        let mut pending = Box::pin(
            r.store
                .bind_stock_signing_intents(request, r.enrollment.proof()),
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
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_operations WHERE family=3")
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
async fn protected_t1_presence_parts_and_identity_cannot_be_replaced_by_index_deletion_or_corruption()
 {
    for statement in [
        "UPDATE stock_operations SET protected=randomblob(length(protected)) WHERE family=3",
        "UPDATE stock_operations SET related_operation=randomblob(32) WHERE family=3",
        "DELETE FROM stock_operation_parts WHERE operation_id IN (SELECT operation_id FROM stock_operations WHERE family=3)",
        "DELETE FROM stock_operations WHERE family=3",
    ] {
        let r = Rig::new().await;
        let (_, receipt) = prepared(&r).await;
        bound(bind(&r, &receipt, 42).await.unwrap());
        let minimum = inventory_minimum(&r).await;
        let mut raw = database(&r).await;
        sqlx::raw_sql(statement).execute(&mut raw).await.unwrap();
        assert_eq!(
            r.store
                .stock_signing_plan(
                    r.binding.clone(),
                    minimum,
                    receipt.attempt().operation_id(),
                    receipt.attempt().command_digest(),
                    r.enrollment.proof()
                )
                .await
                .unwrap_err()
                .code(),
            "recovery_required",
            "{statement}"
        );
        let request = StockSigningPlanRequest::new(
            r.binding.clone(),
            minimum,
            candidate(&r, &receipt, 43).await,
        )
        .unwrap();
        assert_eq!(
            r.store
                .bind_stock_signing_intents(request, r.enrollment.proof())
                .await
                .unwrap_err()
                .code(),
            "recovery_required",
            "{statement}"
        );
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn replacement_recovery_only_grant_recovers_expired_t1_and_cannot_bind_missing_intents() {
    for existing in [false, true] {
        let r = Rig::new().await;
        let (command, receipt) = prepared(&r).await;
        let original = if existing {
            Some(bound(bind(&r, &receipt, 42).await.unwrap()))
        } else {
            None
        };
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
        let recovered = r
            .store
            .stock_signing_plan(
                binding.clone(),
                inventory_minimum(&r).await,
                receipt.attempt().operation_id(),
                receipt.attempt().command_digest(),
                child.proof(),
            )
            .await
            .unwrap();
        assert_eq!(recovered.is_some(), existing);
        if let (Some(recovered), Some(original)) = (recovered, original) {
            assert_eq!(recovered.attempt(), original.attempt());
            assert_eq!(
                recovered.plan().protected_wire(),
                original.plan().protected_wire()
            );
            assert_eq!(recovered.intent_digests(), original.intent_digests());
        } else {
            let request = StockSigningPlanRequest::new(
                binding,
                inventory_minimum(&r).await,
                candidate(&r, &receipt, 42).await,
            )
            .unwrap();
            assert_eq!(
                r.store
                    .bind_stock_signing_intents(request, child.proof())
                    .await
                    .unwrap_err()
                    .code(),
                "unauthorized"
            );
        }
        assert_pool(&r, 80, 9, 1, 2).await;
        assert_pool(&r, 81, 8, 2, 2).await;
        r.store.close().await.unwrap();
    }
}
