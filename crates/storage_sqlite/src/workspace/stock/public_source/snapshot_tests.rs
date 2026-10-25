//! Coherent real native recovery; query results never become signing permits.
use super::*;
use crate::workspace::stock::issuance::evidence::selection;
use crate::workspace::{
    StockOperationSnapshot, StockSigningProgress as Progress, StockSnapshotRequest,
};

async fn request(r: &Rig, receipt: &StockAcceptanceReceipt) -> StockSnapshotRequest {
    StockSnapshotRequest::new(
        r.binding.clone(),
        inventory_minimum(r).await,
        receipt.attempt().operation_id(),
        receipt.attempt().command_digest(),
    )
}
async fn snapshot(r: &Rig, receipt: &StockAcceptanceReceipt) -> StockOperationSnapshot {
    r.store
        .stock_operation_snapshot(request(r, receipt).await, r.enrollment.proof())
        .await
        .unwrap()
}
#[tokio::test]
async fn original_t0_t1_unknown_and_partial_obtained_states_form_one_coherent_reopened_view() {
    let mut r = Rig::new().await;
    let (_, receipt) = prepared(&r).await;
    let initial = snapshot(&r, &receipt).await;
    assert!(initial.signing_plan().is_none());
    assert!(
        initial
            .recipients()
            .iter()
            .all(|value| value.progress() == Progress::Unbound)
    );
    bound(bind(&r, &receipt, 42).await.unwrap());
    let planned = snapshot(&r, &receipt).await;
    assert!(planned.signing_plan().is_some());
    assert!(
        planned
            .recipients()
            .iter()
            .all(|value| value.progress() == Progress::Bound)
    );
    let recipient = receipt.rumor().recipients()[0];
    let StockClaimOutcome::Fresh(permit) = claim(&r, &receipt, recipient).await.unwrap() else {
        panic!("first claim");
    };
    let (intent, report) = permit
        .into_signing_intent(CancellationSignal::new())
        .unwrap();
    let unknown = snapshot(&r, &receipt).await;
    assert_eq!(unknown.recipients()[0].progress(), Progress::Unknown);
    assert_eq!(
        unknown.recipients()[0].claim().unwrap().attempt_id(),
        report.attempt_id()
    );
    let signer = LocalSigner::with_clock(SecretKey::parse(&format!("{:064x}", 3)).unwrap(), || {
        Ok(150000)
    })
    .unwrap();
    let (_, OutcomeKind::Obtained(signed)) = signer.sign_exact(intent).await.into_parts() else {
        panic!("actual signing");
    };
    let raw = signed.with_json(str::to_owned);
    let mut first = None;
    for text in [raw.clone(), format!("\n{raw}\n")] {
        let value = StockEvidenceCapture::verify(r.binding.clone(), &report, &text, None).unwrap();
        let receipt = committed(
            r.store
                .capture_stock_signing_evidence(&value, r.enrollment.proof())
                .await
                .unwrap(),
        );
        first.get_or_insert(receipt);
    }
    let partial = snapshot(&r, &receipt).await;
    assert_eq!(partial.recipients()[0].progress(), Progress::Obtained);
    assert_eq!(partial.recipients()[0].evidence_count().get(), 2);
    assert_eq!(partial.recipients()[0].first_evidence(), first);
    assert_eq!(partial.recipients()[1].progress(), Progress::Bound);
    assert!(partial.recipients()[1].claim().is_none());
    assert_eq!(partial.preparation().attempt(), receipt.attempt());
    assert_eq!(partial.checkpoint(), inventory_minimum(&r).await);
    assert_eq!(format!("{partial:?}"), "StockOperationSnapshot(<redacted>)");
    reopen_plain(&mut r).await;
    r.clock.0.store(229, Ordering::SeqCst);
    let recovered = snapshot(&r, &receipt).await;
    assert_eq!(recovered.recipients(), partial.recipients());
    assert_eq!(
        recovered.signing_plan().unwrap().plan().protected_wire(),
        partial.signing_plan().unwrap().plan().protected_wire()
    );
    assert!(matches!(
        claim(&r, &receipt, recipient).await.unwrap(),
        StockClaimOutcome::Existing(_)
    ));
    assert_pool(&r, 80, 9, 1, 2).await;
    assert_pool(&r, 81, 8, 2, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn valid_evidence_for_another_selector_is_refused_and_metadata_witnesses_detect_new_outputs()
{
    let r = Rig::new().await;
    let receipt = ready(&r).await;
    let recipients = *receipt.rumor().recipients();
    let mut originals = Vec::new();
    for recipient in recipients {
        let (report, raw) = obtained(&r, &receipt, recipient).await;
        let value = StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None).unwrap();
        committed(
            r.store
                .capture_stock_signing_evidence(&value, r.enrollment.proof())
                .await
                .unwrap(),
        );
        originals.push((report, raw));
    }
    let mut rawdb = database(&r).await;
    let other = selection::page(
        &mut rawdb,
        r.enrollment.input().participant(),
        receipt.attempt().operation_id(),
        recipients[1],
        None,
    )
    .await
    .unwrap()[0];
    assert!(
        selection::open(
            &mut rawdb,
            r.enrollment.input(),
            &r.fixture.crypto(),
            receipt.attempt().operation_id(),
            recipients[0],
            other
        )
        .await
        .is_err()
    );
    let summary = selection::Summary::read(
        &mut rawdb,
        r.enrollment.input().participant(),
        receipt.attempt().operation_id(),
        recipients[0],
    )
    .await
    .unwrap();
    let prior = selection::page(
        &mut rawdb,
        r.enrollment.input().participant(),
        receipt.attempt().operation_id(),
        recipients[0],
        None,
    )
    .await
    .unwrap();
    let (report, raw) = &originals[0];
    let extra =
        StockEvidenceCapture::verify(r.binding.clone(), report, &format!("\n{raw}\n"), None)
            .unwrap();
    committed(
        r.store
            .capture_stock_signing_evidence(&extra, r.enrollment.proof())
            .await
            .unwrap(),
    );
    let mut tx = rawdb.begin_with("BEGIN IMMEDIATE").await.unwrap();
    assert_eq!(
        summary
            .confirm(
                &mut tx,
                r.enrollment.input().participant(),
                receipt.attempt().operation_id(),
                recipients[0]
            )
            .await,
        Err(OperationError::RecoveryRequired)
    );
    assert_ne!(
        selection::page(
            &mut tx,
            r.enrollment.input().participant(),
            receipt.attempt().operation_id(),
            recipients[0],
            None
        )
        .await
        .unwrap()
        .len(),
        prior.len()
    );
    tx.rollback().await.unwrap();
    let value = snapshot(&r, &receipt).await;
    assert_eq!(value.recipients()[0].evidence_count().get(), 2);
    assert_eq!(value.recipients()[1].evidence_count().get(), 1);
    rawdb.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn snapshot_refuses_wrong_digest_missing_original_and_protected_graph_damage() {
    for statement in [
        "UPDATE stock_signing_claims SET recipient=randomblob(32)",
        "DELETE FROM stock_signing_evidence",
        "UPDATE stock_operations SET protected=randomblob(length(protected)) WHERE family=3",
    ] {
        let r = Rig::new().await;
        let receipt = ready(&r).await;
        let recipient = receipt.rumor().recipients()[0];
        let (report, raw) = obtained(&r, &receipt, recipient).await;
        let value = StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None).unwrap();
        committed(
            r.store
                .capture_stock_signing_evidence(&value, r.enrollment.proof())
                .await
                .unwrap(),
        );
        let minimum = inventory_minimum(&r).await;
        for (operation, digest, code) in [
            (
                OperationId::from_bytes([99; 32]),
                receipt.attempt().command_digest(),
                "stale_state",
            ),
            (
                receipt.attempt().operation_id(),
                radroots_domain::CommandDigest::from_bytes([99; 32]),
                "idempotency_conflict",
            ),
        ] {
            assert_eq!(
                r.store
                    .stock_operation_snapshot(
                        StockSnapshotRequest::new(r.binding.clone(), minimum, operation, digest),
                        r.enrollment.proof()
                    )
                    .await
                    .unwrap_err()
                    .code(),
                code
            );
        }
        let request = request(&r, &receipt).await;
        let mut rawdb = database(&r).await;
        sqlx::raw_sql(statement).execute(&mut rawdb).await.unwrap();
        assert!(
            r.store
                .stock_operation_snapshot(request, r.enrollment.proof())
                .await
                .is_err(),
            "{statement}"
        );
        rawdb.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn dropped_snapshot_observation_drains_and_late_changes_never_return_a_mixed_view() {
    for statement in [
        None,
        Some("UPDATE stock_grants SET protected=randomblob(length(protected))"),
        Some("DELETE FROM stock_signing_evidence"),
    ] {
        let mut r = Rig::new().await;
        let receipt = ready(&r).await;
        let recipient = receipt.rumor().recipients()[0];
        let (report, raw) = obtained(&r, &receipt, recipient).await;
        let value = StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None).unwrap();
        committed(
            r.store
                .capture_stock_signing_evidence(&value, r.enrollment.proof())
                .await
                .unwrap(),
        );
        let request = request(&r, &receipt).await;
        let provider = reopen(&mut r).await;
        let mut rawdb = database(&r).await;
        provider.gate.arm(
            Direction::Unwrap,
            "radroots.seller_stock.operation_part.v1",
            0,
        );
        let mut pending = Box::pin(
            r.store
                .stock_operation_snapshot(request, r.enrollment.proof()),
        );
        assert!(futures::poll!(pending.as_mut()).is_pending());
        provider.gate.entered();
        if let Some(statement) = statement {
            sqlx::raw_sql(statement).execute(&mut rawdb).await.unwrap();
            provider.gate.release();
            assert!(pending.await.is_err());
        } else {
            drop(pending);
            provider.gate.release();
            reopen_plain(&mut r).await;
            assert_eq!(
                snapshot(&r, &receipt).await.recipients()[0].progress(),
                Progress::Obtained
            );
        }
        rawdb.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn replacement_recovery_only_reads_original_obtained_state_after_first_issue_expiry() {
    let r = Rig::new().await;
    let (command, receipt) = prepared(&r).await;
    bound(bind(&r, &receipt, 42).await.unwrap());
    let recipient = receipt.rumor().recipients()[0];
    let (report, raw) = obtained(&r, &receipt, recipient).await;
    let evidence = StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None).unwrap();
    let first = committed(
        r.store
            .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
            .await
            .unwrap(),
    );
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
    let scope = StockConsentCapture::prepare(
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
                &scope,
                r.enrollment.proof()
            )
            .await
            .unwrap(),
        StockConsentOutcome::Committed(_)
    ));
    let request = StockSnapshotRequest::new(
        binding,
        inventory_minimum(&r).await,
        receipt.attempt().operation_id(),
        receipt.attempt().command_digest(),
    );
    let recovered = r
        .store
        .stock_operation_snapshot(request, child.proof())
        .await
        .unwrap();
    assert_eq!(recovered.preparation().attempt(), receipt.attempt());
    assert_eq!(recovered.recipients()[0].first_evidence(), Some(first));
    assert_eq!(
        recovered.recipients()[0].claim().unwrap().attempt_id(),
        report.attempt_id()
    );
    assert_eq!(recovered.recipients()[1].progress(), Progress::Bound);
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn uncertain_owner_never_returns_a_snapshot_until_original_reopen_reconciles() {
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    let recipient = receipt.rumor().recipients()[0];
    let (report, raw) = obtained(&r, &receipt, recipient).await;
    let evidence = StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None).unwrap();
    let request = request(&r, &receipt).await;
    let mut rawdb = database(&r).await;
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_signing_evidence BEGIN INSERT INTO deferred_fault VALUES(2); END").execute(&mut rawdb).await.unwrap();
    assert!(matches!(
        r.store
            .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
            .await
            .unwrap(),
        StockEvidenceOutcome::Unknown(_)
    ));
    assert_eq!(
        r.store
            .stock_operation_snapshot(request, r.enrollment.proof())
            .await
            .unwrap_err()
            .code(),
        "recovery_required"
    );
    r.store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER fault; DROP TABLE deferred_fault")
        .execute(&mut rawdb)
        .await
        .unwrap();
    rawdb.close().await.unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    let recovered = snapshot(&r, &receipt).await;
    assert_eq!(recovered.recipients()[0].progress(), Progress::Unknown);
    assert_eq!(recovered.recipients()[0].evidence_count().get(), 0);
    assert_eq!(
        recovered.recipients()[0].claim().unwrap().attempt_id(),
        report.attempt_id()
    );
    r.store.close().await.unwrap();
}
