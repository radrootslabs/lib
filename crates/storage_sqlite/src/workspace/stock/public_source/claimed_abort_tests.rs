//! Actual current cleanup, replay risk and retained contradictory evidence.
use super::*;
use crate::workspace::{
    StockDispatchOutcome, StockEvidenceCapture, StockSigningAttempt, StockUnissuedOutcome,
};
use radroots_signing::issuance::{ExactIssuer, ExactRecoveryKind, OutcomeKind};
use radroots_storage::inventory::StockEvidenceOutcome;

async fn replay_ready(r: &Rig) -> StockAcceptanceReceipt {
    let (_, receipt) = prepared(r).await;
    let candidate = candidate_with_replay(r, &receipt, 42, SigningReplay::LocalReplaySafe).await;
    bound(
        r.store
            .bind_stock_signing_intents(
                StockSigningPlanRequest::new(
                    r.binding.clone(),
                    inventory_minimum(r).await,
                    candidate,
                )
                .unwrap(),
                r.enrollment.proof(),
            )
            .await
            .unwrap(),
    );
    receipt
}
fn local() -> LocalSigner {
    LocalSigner::with_clock(SecretKey::parse(&format!("{:064x}", 3)).unwrap(), || {
        Ok(150000)
    })
    .unwrap()
}
async fn refused(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
    recipient: PublicKey,
    persist: bool,
) -> StockSigningAttempt {
    let StockClaimOutcome::Fresh(permit) = claim(r, receipt, recipient).await.unwrap() else {
        panic!("original claim")
    };
    let report = permit.report().clone();
    let cancellation = CancellationSignal::new();
    cancellation.cancel();
    let signer = local();
    let StockDispatchOutcome::Unissued(value) =
        permit.dispatch(&signer, cancellation).await.unwrap()
    else {
        panic!("actual original local refusal")
    };
    if persist {
        assert!(matches!(
            r.store
                .capture_stock_original_unissued(r.binding.clone(), &value, r.enrollment.proof())
                .await
                .unwrap(),
            StockUnissuedOutcome::Committed(_)
        ));
    }
    report
}
async fn snapshot(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
) -> crate::workspace::StockOperationSnapshot {
    r.store
        .stock_operation_snapshot(
            StockSnapshotRequest::new(
                r.binding.clone(),
                inventory_minimum(r).await,
                receipt.attempt().operation_id(),
                receipt.attempt().command_digest(),
            ),
            r.enrollment.proof(),
        )
        .await
        .unwrap()
}
#[tokio::test]
async fn current_claimed_cleanup_releases_exact_originals_once_and_preserves_every_fact() {
    for count in [1, 2] {
        let mut r = Rig::new().await;
        let receipt = Box::pin(replay_ready(&r)).await;
        let mut original = Vec::new();
        for recipient in receipt.rumor().recipients().iter().take(count) {
            original.push(refused(&r, &receipt, *recipient, true).await);
        }
        let before = inventory_minimum(&r).await;
        let first = released(abort(&r, &receipt).await.unwrap());
        assert_eq!(
            first.checkpoint().revision().get(),
            before.revision().get() + 1
        );
        balances(&r, 80, 10, 0, 0, 3).await;
        balances(&r, 81, 10, 0, 0, 3).await;
        let view = snapshot(&r, &receipt).await;
        assert_eq!(view.aborted().unwrap().attempt(), first.attempt());
        assert_eq!(
            view.recipients()
                .iter()
                .filter(|v| v.claim().is_some())
                .count(),
            count
        );
        for v in view.recipients() {
            assert_eq!(v.claim().is_some(), v.original_unissued().is_some());
            assert!(v.first_evidence().is_none() && v.replay_admission().is_none());
        }
        let acknowledged = inventory_minimum(&r).await;
        reopen_plain(&mut r).await;
        assert_eq!(
            released(abort(&r, &receipt).await.unwrap()).attempt(),
            first.attempt()
        );
        assert_eq!(inventory_minimum(&r).await, acknowledged);
        for original in &original {
            let selected = selector(&r, &receipt, original.recipient()).await;
            assert_eq!(
                r.store
                    .stock_signing_attempt(selected, r.enrollment.proof())
                    .await
                    .unwrap()
                    .unwrap()
                    .attempt_id(),
                original.attempt_id()
            );
            let StockClaimOutcome::Existing(again) =
                claim(&r, &receipt, original.recipient()).await.unwrap()
            else {
                panic!("an aborted original cannot return fresh dispatch permission");
            };
            assert_eq!(again.attempt_id(), original.attempt_id());
            assert!(
                r.store
                    .authorize_stock_exact_replay(
                        selector(&r, &receipt, original.recipient()).await,
                        r.enrollment.proof()
                    )
                    .await
                    .is_err()
            );
        }
        let mut raw = database(&r).await;
        for (query, expected) in [
            ("SELECT count(*) FROM stock_signing_claims", count as i64),
            ("SELECT count(*) FROM stock_original_unissued", count as i64),
            ("SELECT count(*) FROM stock_signing_evidence", 0),
            ("SELECT count(*) FROM stock_aborts", 1),
        ] {
            let actual: i64 = sqlx::query_scalar(query).fetch_one(&mut raw).await.unwrap();
            assert_eq!(actual, expected);
        }
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn unrecorded_original_refusal_and_untracked_retained_claims_never_authorize_cleanup() {
    for catalog in [14, 15, 16] {
        let r = Rig::with_catalog(catalog).await;
        let receipt = Box::pin(replay_ready(&r)).await;
        refused(&r, &receipt, peer(&r, &receipt), catalog == 14).await;
        let before = inventory_minimum(&r).await;
        assert_eq!(
            abort(&r, &receipt).await.unwrap_err().code(),
            "recovery_required"
        );
        assert_eq!(inventory_minimum(&r).await, before);
        balances(&r, 80, 9, 1, 0, 2).await;
        balances(&r, 81, 8, 2, 0, 2).await;
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn held_replay_permission_prevents_release_and_late_original_output_stays_installable() {
    let r = Rig::new().await;
    let receipt = Box::pin(replay_ready(&r)).await;
    for recipient in receipt.rumor().recipients() {
        refused(&r, &receipt, *recipient, true).await;
    }
    let recipient = peer(&r, &receipt);
    let permit = r
        .store
        .authorize_stock_exact_replay(
            selector(&r, &receipt, recipient).await,
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    assert!(permit.admission().is_some());
    assert_eq!(
        abort(&r, &receipt).await.unwrap_err().code(),
        "recovery_required"
    );
    balances(&r, 80, 9, 1, 0, 2).await;
    let call = ProviderCall::new(
        r.enrollment.input().participant(),
        NonZeroU64::new(250000).unwrap(),
        CancellationSignal::new(),
    );
    let (request, report) = permit.into_recovery_request(call).unwrap();
    let outcome = local().recover_exact(request).await;
    let (_, ExactRecoveryKind::Obtained(signed)) = outcome.into_parts() else {
        panic!("original recovered output")
    };
    let capture = signed
        .with_json(|raw| StockEvidenceCapture::verify(r.binding.clone(), &report, raw, None))
        .unwrap();
    assert!(matches!(
        r.store
            .capture_stock_signing_evidence(&capture, r.enrollment.proof())
            .await
            .unwrap(),
        StockEvidenceOutcome::Committed(_)
    ));
    installed(install(&r, &receipt).await.unwrap());
    assert_eq!(abort(&r, &receipt).await.unwrap_err().code(), "stale_state");
    let view = snapshot(&r, &receipt).await;
    assert!(view.aborted().is_none() && view.installation().is_some());
    assert!(
        view.recipients()
            .iter()
            .all(|v| v.original_unissued().is_some())
    );
    balances(&r, 80, 9, 0, 1, 3).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn every_claimed_cleanup_write_fault_rolls_back_release_and_keeps_original_proofs() {
    for statement in [
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operations WHEN NEW.family=7 BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operation_parts BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_aborts BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_pools BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_movements BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_order_states BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'fixture'); END",
    ] {
        let r = Rig::new().await;
        let receipt = Box::pin(replay_ready(&r)).await;
        refused(&r, &receipt, peer(&r, &receipt), true).await;
        let before = inventory_minimum(&r).await;
        let mut raw = database(&r).await;
        sqlx::query(statement).execute(&mut raw).await.unwrap();
        assert_eq!(
            abort(&r, &receipt).await.unwrap_err().code(),
            "storage_failure",
            "{statement}"
        );
        assert_eq!(inventory_minimum(&r).await, before);
        balances(&r, 80, 9, 1, 0, 2).await;
        balances(&r, 81, 8, 2, 0, 2).await;
        assert!(snapshot(&r, &receipt).await.aborted().is_none());
        sqlx::query("DROP TRIGGER fault")
            .execute(&mut raw)
            .await
            .unwrap();
        raw.close().await.unwrap();
        released(abort(&r, &receipt).await.unwrap());
        balances(&r, 80, 10, 0, 0, 3).await;
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn uncertain_claimed_cleanup_fences_owner_and_reopens_without_erasing_claims_or_facts() {
    let mut r = Rig::new().await;
    let receipt = Box::pin(replay_ready(&r)).await;
    refused(&r, &receipt, peer(&r, &receipt), true).await;
    let request = abort_request(&r, &receipt).await;
    let mut raw = database(&r).await;
    sqlx::raw_sql("CREATE TABLE cleanup_deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_aborts BEGIN INSERT INTO cleanup_deferred_fault VALUES(2); END").execute(&mut raw).await.unwrap();
    assert!(matches!(
        r.store
            .abort_proven_unissued(request, r.enrollment.proof())
            .await
            .unwrap(),
        StockAbortOutcome::Unknown(_)
    ));
    let request = StockAbortRequest::new(
        r.binding.clone(),
        receipt.checkpoint(),
        receipt.attempt().operation_id(),
        receipt.attempt().command_digest(),
    );
    assert_eq!(
        r.store
            .abort_proven_unissued(request, r.enrollment.proof())
            .await
            .unwrap_err()
            .code(),
        "recovery_required"
    );
    sqlx::raw_sql("DROP TRIGGER fault; DROP TABLE cleanup_deferred_fault")
        .execute(&mut raw)
        .await
        .unwrap();
    raw.close().await.unwrap();
    reopen_plain(&mut r).await;
    assert!(snapshot(&r, &receipt).await.aborted().is_none());
    released(abort(&r, &receipt).await.unwrap());
    balances(&r, 80, 10, 0, 0, 3).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn contradictory_late_signed_bytes_are_retained_and_fence_fresh_stock() {
    let r = Rig::new().await;
    let receipt = Box::pin(replay_ready(&r)).await;
    let report = refused(&r, &receipt, peer(&r, &receipt), true).await;
    released(abort(&r, &receipt).await.unwrap());
    // Deliberately model a root-key holder signing outside executor permission.
    // This contradicts the original qualified nonissuance fact; it is not replay.
    let intent = crate::workspace::stock::issuance::claim::intent(
        report.bound_plan(),
        report.recipient(),
        CancellationSignal::new(),
    )
    .unwrap();
    let (_, OutcomeKind::Obtained(signed)) = local().issue_exact(intent).await.into_parts() else {
        panic!("actual contradictory signature")
    };
    let text = signed.with_json(str::to_owned);
    let capture = StockEvidenceCapture::verify(r.binding.clone(), &report, &text, None).unwrap();
    let StockEvidenceOutcome::Committed(retained) = r
        .store
        .capture_stock_signing_evidence(&capture, r.enrollment.proof())
        .await
        .unwrap()
    else {
        panic!("retain obtained bytes")
    };
    assert_eq!(retained.signing_attempt_id(), report.attempt_id());
    let current = inventory_minimum(&r).await;
    let request = StockSnapshotRequest::new(
        r.binding.clone(),
        current,
        receipt.attempt().operation_id(),
        receipt.attempt().command_digest(),
    );
    assert_eq!(
        r.store
            .stock_operation_snapshot(request, r.enrollment.proof())
            .await
            .unwrap_err()
            .code(),
        "recovery_required"
    );
    let wire = serde_json::json!({"type":"stock.create_pool","context":context(&r,98),"input":{"id":pool(98),"label":"blocked after contradictory evidence","unit":"each","tick_scale":"0","initial_quantity":"1"}});
    let command = radroots_storage::inventory::InventoryRequest::from_protected_wire(
        r.binding.clone(),
        current,
        &serde_json::to_vec(&wire).unwrap(),
    )
    .unwrap();
    assert_eq!(
        r.store
            .execute_inventory(
                command,
                r.enrollment
                    .proof()
                    .with_secret(|v| AttachmentProof::from_protected_input(v))
                    .unwrap()
            )
            .await
            .unwrap_err()
            .code(),
        "recovery_required"
    );
    let mut raw = database(&r).await;
    let entry: Vec<u8> = sqlx::query_scalar("SELECT entry_id FROM stock_signing_evidence")
        .fetch_one(&mut raw)
        .await
        .unwrap();
    let decoded = Box::pin(crate::workspace::stock::issuance::evidence::read_entry(
        &mut raw,
        r.enrollment.input(),
        &r.fixture.crypto(),
        OperationId::from_bytes(entry.try_into().unwrap()),
    ))
    .await
    .unwrap()
    .unwrap();
    decoded.value.with_json(|value| assert_eq!(value, text));
    let fresh: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_pools WHERE pool_id=?")
        .bind(pool(98).as_bytes().as_slice())
        .fetch_one(&mut raw)
        .await
        .unwrap();
    assert_eq!(fresh, 0);
    raw.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn lost_claimed_cleanup_waiter_drains_before_reopen_and_keeps_original_proofs() {
    let mut r = Rig::new().await;
    let receipt = Box::pin(replay_ready(&r)).await;
    refused(&r, &receipt, peer(&r, &receipt), true).await;
    let request = abort_request(&r, &receipt).await;
    let provider = reopen(&mut r).await;
    provider.gate.arm(Direction::Wrap, r.authority_schema(), 0);
    let mut pending = Box::pin(r.store.abort_proven_unissued(request, r.enrollment.proof()));
    assert!(futures::poll!(pending.as_mut()).is_pending());
    provider.gate.entered();
    drop(pending);
    let mut closing = Box::pin(r.store.close());
    assert!(futures::poll!(closing.as_mut()).is_pending());
    provider.gate.release();
    closing.await.unwrap();
    reopen_plain(&mut r).await;
    let view = snapshot(&r, &receipt).await;
    let original = view.aborted().unwrap().attempt();
    assert_eq!(
        view.recipients()
            .iter()
            .filter(|v| v.original_unissued().is_some())
            .count(),
        1
    );
    assert_eq!(
        released(abort(&r, &receipt).await.unwrap()).attempt(),
        original
    );
    balances(&r, 80, 10, 0, 0, 3).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn damaged_original_fact_never_becomes_release_permission_or_an_automatic_repair() {
    for statement in [
        "DELETE FROM stock_original_unissued",
        "UPDATE stock_original_unissued SET entry_id=randomblob(32)",
        "UPDATE stock_operation_parts SET protected=randomblob(length(protected)) WHERE operation_id IN (SELECT entry_id FROM stock_original_unissued)",
    ] {
        let r = Rig::new().await;
        let receipt = Box::pin(replay_ready(&r)).await;
        refused(&r, &receipt, peer(&r, &receipt), true).await;
        let request = abort_request(&r, &receipt).await;
        let mut raw = database(&r).await;
        let before: Vec<(Vec<u8>, i64, Vec<u8>)> =
            sqlx::query_as("SELECT pool_id,revision,protected FROM stock_pools ORDER BY pool_id")
                .fetch_all(&mut raw)
                .await
                .unwrap();
        sqlx::query(statement).execute(&mut raw).await.unwrap();
        assert_eq!(
            r.store
                .abort_proven_unissued(request, r.enrollment.proof())
                .await
                .unwrap_err()
                .code(),
            "recovery_required",
            "{statement}"
        );
        let after: Vec<(Vec<u8>, i64, Vec<u8>)> =
            sqlx::query_as("SELECT pool_id,revision,protected FROM stock_pools ORDER BY pool_id")
                .fetch_all(&mut raw)
                .await
                .unwrap();
        assert_eq!(after, before);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_aborts")
            .fetch_one(&mut raw)
            .await
            .unwrap();
        assert_eq!(count, 0);
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}
