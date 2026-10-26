//! Actual original-claim local recovery, current authorization and existing S/T2.
use super::*;
use crate::workspace::{
    StockEvidenceCapture, StockExactReplayPermit, StockInstallationOutcome,
    StockInstallationRequest,
};
use radroots_signing::issuance::ExactRecoveryKind;
use radroots_storage::inventory::StockEvidenceOutcome;

async fn local_ready(r: &Rig) -> StockAcceptanceReceipt {
    local_ready_command(r).await.1
}
async fn local_ready_command(r: &Rig) -> (CapturedOrderAcceptance, StockAcceptanceReceipt) {
    let (command, receipt) = prepared(r).await;
    let candidate = candidate_with_replay(r, &receipt, 42, SigningReplay::LocalReplaySafe).await;
    let request =
        StockSigningPlanRequest::new(r.binding.clone(), inventory_minimum(r).await, candidate)
            .unwrap();
    let plan = bound(
        r.store
            .bind_stock_signing_intents(request, r.enrollment.proof())
            .await
            .unwrap(),
    );
    assert_eq!(plan.plan().replay(), SigningReplay::LocalReplaySafe);
    (command, receipt)
}
async fn authorize(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
    recipient: PublicKey,
) -> Result<StockExactReplayPermit, OperationError> {
    r.store
        .authorize_stock_exact_replay(selector(r, receipt, recipient).await, r.enrollment.proof())
        .await
}
fn call(r: &Rig, deadline: u64) -> ProviderCall {
    ProviderCall::new(
        r.enrollment.input().participant(),
        NonZeroU64::new(deadline).unwrap(),
        CancellationSignal::new(),
    )
}
fn signer() -> LocalSigner {
    LocalSigner::with_clock(SecretKey::parse(&format!("{:064x}", 3)).unwrap(), || {
        Ok(150000)
    })
    .unwrap()
}

#[tokio::test]
async fn original_local_claim_reopens_recovers_exact_s_and_installs_once_without_another_hold() {
    let mut r = Rig::new().await;
    let receipt = Box::pin(local_ready(&r)).await;
    let recipient = receipt
        .rumor()
        .recipients()
        .iter()
        .copied()
        .find(|key| *key != r.enrollment.input().participant())
        .unwrap();
    let StockClaimOutcome::Fresh(first) = claim(&r, &receipt, recipient).await.unwrap() else {
        panic!("first claim")
    };
    let original_attempt = first.report().attempt_id();
    let original_binding = first.report().binding();
    drop(first); // Actual dispatch result is unavailable; the durable claim stays.
    let acknowledged = inventory_minimum(&r).await;
    reopen_plain(&mut r).await;
    let permitted = authorize(&r, &receipt, recipient).await.unwrap();
    assert_eq!(permitted.report().attempt_id(), original_attempt);
    assert_eq!(permitted.report().binding(), original_binding);
    assert_eq!(inventory_minimum(&r).await, acknowledged);
    let (request, report) = permitted.into_recovery_request(call(&r, 250000)).unwrap();
    assert_eq!(request.binding(), original_binding);
    let expected_event = request.original().with_event(|_, id| id);
    let outcome = signer().recover_exact(request).await;
    assert_eq!(outcome.binding(), original_binding);
    let (_, ExactRecoveryKind::Obtained(signed)) = outcome.into_parts() else {
        panic!("qualified original S")
    };
    assert_eq!(signed.event_id(), expected_event);
    let capture = signed
        .with_json(|raw| {
            StockEvidenceCapture::verify(
                r.binding.clone(),
                &report,
                raw,
                signed.observed_at_unix_ms(),
            )
        })
        .unwrap();
    assert!(matches!(
        r.store
            .capture_stock_signing_evidence(&capture, r.enrollment.proof())
            .await
            .unwrap(),
        StockEvidenceOutcome::Committed(_)
    ));
    let request = || {
        StockInstallationRequest::new(
            r.binding.clone(),
            acknowledged,
            receipt.attempt().operation_id(),
            receipt.attempt().command_digest(),
        )
    };
    let StockInstallationOutcome::Installed(installed) = r
        .store
        .install_stock_agreement(request(), r.enrollment.proof())
        .await
        .unwrap()
    else {
        panic!("original T2")
    };
    let first_install = installed.attempt();
    let current = inventory_minimum(&r).await;
    let StockInstallationOutcome::Installed(again) = r
        .store
        .install_stock_agreement(request(), r.enrollment.proof())
        .await
        .unwrap()
    else {
        panic!("same installation")
    };
    assert_eq!(again.attempt(), first_install);
    assert_eq!(inventory_minimum(&r).await, current);
    let selected = r
        .store
        .pool(
            r.binding.clone(),
            current,
            pool(80),
            r.enrollment
                .proof()
                .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                .unwrap(),
        )
        .await
        .unwrap()
        .unwrap()
        .snapshot();
    assert_eq!(selected.balances().physical().available.get(), 9);
    assert_eq!(selected.balances().physical().preparatory_hold.get(), 0);
    assert_eq!(selected.balances().physical().committed_hold.get(), 1);
    assert!(matches!(
        claim(&r, &receipt, recipient).await.unwrap(),
        StockClaimOutcome::Existing(_)
    ));
    let installed_replay = authorize(&r, &receipt, recipient).await.unwrap();
    assert_eq!(installed_replay.report().binding(), original_binding);
    assert_eq!(inventory_minimum(&r).await, current);
    drop(installed_replay);
    reopen_plain(&mut r).await;
    let original = read(&r, &receipt, recipient).await.unwrap().unwrap();
    assert_eq!(original.attempt_id(), original_attempt);
    assert_eq!(original.binding(), original_binding);
    assert_eq!(inventory_minimum(&r).await, current);
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn ordinary_outbox_refuses_exact_replay_without_adopting_stock_authority() {
    let r = Rig::new().await;
    let receipt = local_ready(&r).await;
    let request = selector(&r, &receipt, receipt.rumor().recipients()[0]).await;
    let (outbox, _, proof) = NativeWorkspace::initialize_local_outbox(
        &r.path.with_file_name("separate-outbox"),
        crate::workspace::LocalOutboxConfig::seller(
            r.enrollment.input().participant(),
            r.enrollment.input().participant(),
            "synthetic-recovery-outbox",
            UnixSeconds::new(500).unwrap(),
        )
        .unwrap(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        outbox
            .authorize_stock_exact_replay(request, &proof)
            .await
            .unwrap_err(),
        OperationError::UnsupportedOperation
    );
    let evidence = radroots_storage::inventory::StockEvidenceRequest::from_obtained_wire(
        r.binding.clone(),
        receipt.attempt().operation_id(),
        receipt.attempt().command_digest(),
        radroots_domain::SigningAttemptId::from_bytes([95; 32]),
        receipt.rumor().recipients()[0],
        [96; 32],
        "synthetic unsupported descriptor",
        None,
    )
    .unwrap();
    assert_eq!(
        radroots_storage::inventory::InventoryStore::capture_stock_evidence(
            &outbox, &evidence, &proof
        )
        .await
        .unwrap_err(),
        OperationError::UnsupportedOperation,
    );
    assert_eq!(
        radroots_storage::inventory::InventoryStore::reconcile_stock_evidence(
            &outbox, &evidence, &proof
        )
        .await
        .unwrap_err(),
        OperationError::UnsupportedOperation,
    );
    outbox.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn missing_claim_nonreplayable_policy_and_original_expiry_never_mint_replay_permission() {
    for mode in [0, 1, 2] {
        let r = Rig::new().await;
        let receipt = if mode == 1 {
            ready(&r).await
        } else {
            local_ready(&r).await
        };
        let recipient = receipt.rumor().recipients()[0];
        if mode != 0 {
            assert!(matches!(
                claim(&r, &receipt, recipient).await.unwrap(),
                StockClaimOutcome::Fresh(_)
            ));
        }
        let request = selector(&r, &receipt, recipient).await;
        let before = inventory_minimum(&r).await;
        if mode == 2 {
            r.clock
                .0
                .store(receipt.first_issue_deadline().get(), Ordering::SeqCst);
        }
        assert!(
            r.store
                .authorize_stock_exact_replay(request, r.enrollment.proof())
                .await
                .is_err()
        );
        assert_eq!(inventory_minimum(&r).await, before);
        assert_pool(&r, 80, 9, 1, 2).await;
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn wrong_original_digest_recipient_and_damaged_claim_or_backing_refuse_without_new_effect() {
    for mode in [0, 1, 2, 3] {
        let r = Rig::new().await;
        let receipt = local_ready(&r).await;
        let recipient = receipt.rumor().recipients()[0];
        assert!(matches!(
            claim(&r, &receipt, recipient).await.unwrap(),
            StockClaimOutcome::Fresh(_)
        ));
        let request = StockClaimRequest::new(
            r.binding.clone(),
            inventory_minimum(&r).await,
            receipt.attempt().operation_id(),
            if mode == 0 {
                radroots_domain::CommandDigest::from_bytes([99; 32])
            } else {
                receipt.attempt().command_digest()
            },
            if mode == 1 {
                (4..=6)
                    .map(|value| {
                        LocalSigner::with_clock(
                            SecretKey::parse(&format!("{value:064x}")).unwrap(),
                            || Ok(150000),
                        )
                        .unwrap()
                        .public_key()
                    })
                    .find(|key| !receipt.rumor().recipients().contains(key))
                    .unwrap()
            } else {
                recipient
            },
        );
        let mut raw = database(&r).await;
        if mode == 2 {
            sqlx::raw_sql("UPDATE stock_pools SET protected=randomblob(length(protected))")
                .execute(&mut raw)
                .await
                .unwrap();
        }
        if mode == 3 {
            sqlx::raw_sql("DELETE FROM stock_signing_claims")
                .execute(&mut raw)
                .await
                .unwrap();
        }
        assert!(
            r.store
                .authorize_stock_exact_replay(request, r.enrollment.proof())
                .await
                .is_err()
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_operations WHERE family=5")
            .fetch_one(&mut raw)
            .await
            .unwrap();
        assert_eq!(count, 0);
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn replacement_recover_only_grant_reads_the_original_but_cannot_authorize_local_recomputation()
 {
    let r = Rig::new().await;
    let (command, receipt) = local_ready_command(&r).await;
    let recipient = receipt.rumor().recipients()[0];
    let StockClaimOutcome::Fresh(first) = claim(&r, &receipt, recipient).await.unwrap() else {
        panic!("first claim")
    };
    let original = first.report().attempt_id();
    drop(first);
    // Stay inside the original issuance window: refusal must be actual rights,
    // rather than merely an already-expired original operation.
    r.clock.0.store(160, Ordering::SeqCst);
    let child = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        "recovery reader".into(),
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
    let minimum = inventory_minimum(&r).await;
    let request = || {
        StockClaimRequest::new(
            binding.clone(),
            minimum,
            receipt.attempt().operation_id(),
            receipt.attempt().command_digest(),
            recipient,
        )
    };
    assert_eq!(
        r.store
            .stock_signing_attempt(request(), child.proof())
            .await
            .unwrap()
            .unwrap()
            .attempt_id(),
        original
    );
    assert_eq!(
        r.store
            .authorize_stock_exact_replay(request(), child.proof())
            .await
            .unwrap_err(),
        OperationError::Unauthorized
    );
    assert_eq!(inventory_minimum(&r).await, minimum);
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn replay_authorization_rechecks_actual_late_root_grant_plan_claim_backing_and_clock_changes()
{
    for statement in [
        "UPDATE stock_pools SET protected=randomblob(length(protected))",
        "UPDATE stock_grants SET protected=randomblob(length(protected))",
        "UPDATE stock_operations SET protected=randomblob(length(protected)) WHERE family=3",
        "DELETE FROM stock_signing_claims",
        "UPDATE stock_authority SET protected=randomblob(length(protected))",
        "clock_expired",
        "clock_rollback",
    ] {
        let mut r = Rig::new().await;
        let receipt = local_ready(&r).await;
        let recipient = receipt.rumor().recipients()[0];
        assert!(matches!(
            claim(&r, &receipt, recipient).await.unwrap(),
            StockClaimOutcome::Fresh(_)
        ));
        let request = selector(&r, &receipt, recipient).await;
        let provider = reopen(&mut r).await;
        let mut raw = database(&r).await;
        provider
            .gate
            .arm(Direction::Unwrap, "radroots.seller_stock.authority.v13", 0);
        let mut pending = Box::pin(
            r.store
                .authorize_stock_exact_replay(request, r.enrollment.proof()),
        );
        assert!(futures::poll!(pending.as_mut()).is_pending());
        provider.gate.entered();
        match statement {
            "clock_expired" => r
                .clock
                .0
                .store(receipt.first_issue_deadline().get(), Ordering::SeqCst),
            "clock_rollback" => r.clock.0.store(149, Ordering::SeqCst),
            _ => {
                sqlx::raw_sql(statement).execute(&mut raw).await.unwrap();
            }
        }
        provider.gate.release();
        assert!(pending.await.is_err(), "{statement}");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_operations WHERE family=5")
                .fetch_one(&mut raw)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_operations WHERE family=4")
                .fetch_one(&mut raw)
                .await
                .unwrap(),
            1
        );
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn observer_loss_drains_authorization_without_signing_or_a_new_claim_and_reopen_keeps_original()
 {
    let mut r = Rig::new().await;
    let receipt = local_ready(&r).await;
    let recipient = receipt.rumor().recipients()[0];
    let StockClaimOutcome::Fresh(first) = claim(&r, &receipt, recipient).await.unwrap() else {
        panic!("first claim")
    };
    let original = first.report().attempt_id();
    drop(first);
    let before = inventory_minimum(&r).await;
    let request = selector(&r, &receipt, recipient).await;
    let provider = reopen(&mut r).await;
    provider
        .gate
        .arm(Direction::Unwrap, "radroots.seller_stock.authority.v13", 0);
    let mut pending = Box::pin(
        r.store
            .authorize_stock_exact_replay(request, r.enrollment.proof()),
    );
    assert!(futures::poll!(pending.as_mut()).is_pending());
    provider.gate.entered();
    drop(pending);
    let mut closing = Box::pin(r.store.close());
    assert!(futures::poll!(closing.as_mut()).is_pending());
    provider.gate.release();
    closing.await.unwrap();
    reopen_plain(&mut r).await;
    assert_eq!(inventory_minimum(&r).await, before);
    let permitted = authorize(&r, &receipt, recipient).await.unwrap();
    assert_eq!(permitted.report().attempt_id(), original);
    drop(permitted);
    assert_eq!(inventory_minimum(&r).await, before);
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn uncertain_original_claim_never_authorizes_replay_and_reopen_does_not_create_a_missing_claim()
 {
    let mut r = Rig::new().await;
    let receipt = local_ready(&r).await;
    let recipient = receipt.rumor().recipients()[0];
    let before = inventory_minimum(&r).await;
    let request = selector(&r, &receipt, recipient).await;
    let mut raw = database(&r).await;
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_signing_claims BEGIN INSERT INTO deferred_fault VALUES(2); END").execute(&mut raw).await.unwrap();
    assert!(matches!(
        r.store
            .claim_stock_signing_attempt(request, r.enrollment.proof())
            .await
            .unwrap(),
        StockClaimOutcome::Unknown(_)
    ));
    let request = || {
        StockClaimRequest::new(
            r.binding.clone(),
            before,
            receipt.attempt().operation_id(),
            receipt.attempt().command_digest(),
            recipient,
        )
    };
    assert_eq!(
        r.store
            .authorize_stock_exact_replay(request(), r.enrollment.proof())
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
    reopen_plain(&mut r).await;
    assert!(authorize(&r, &receipt, recipient).await.is_err());
    assert_eq!(inventory_minimum(&r).await, before);
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}
