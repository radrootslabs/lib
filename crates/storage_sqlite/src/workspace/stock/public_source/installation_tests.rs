//! Real T2 transactions over original T0/T1 and authenticated peer evidence.
#[path = "abort_tests.rs"]
mod aborts;
#[path = "envelope_tests.rs"]
mod envelopes;
#[path = "local_history_tests.rs"]
mod local_history;
#[cfg(unix)]
#[path = "process_death_tests.rs"]
mod process_death;
use super::*;
use crate::workspace::{
    StockInstallationOutcome, StockInstallationReceipt, StockInstallationRequest,
    StockSnapshotRequest,
};
async fn request(r: &Rig, receipt: &StockAcceptanceReceipt) -> StockInstallationRequest {
    StockInstallationRequest::new(
        r.binding.clone(),
        inventory_minimum(r).await,
        receipt.attempt().operation_id(),
        receipt.attempt().command_digest(),
    )
}
async fn install(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
) -> Result<StockInstallationOutcome, OperationError> {
    r.store
        .install_stock_agreement(request(r, receipt).await, r.enrollment.proof())
        .await
}
fn installed(value: StockInstallationOutcome) -> StockInstallationReceipt {
    let StockInstallationOutcome::Installed(value) = value else {
        panic!("installation must confirm")
    };
    *value
}
async fn retain(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
    recipient: PublicKey,
) -> StockEvidenceReceipt {
    let (report, raw) = obtained(r, receipt, recipient).await;
    let captured = StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None).unwrap();
    committed(
        r.store
            .capture_stock_signing_evidence(&captured, r.enrollment.proof())
            .await
            .unwrap(),
    )
}
fn peer(r: &Rig, receipt: &StockAcceptanceReceipt) -> PublicKey {
    *receipt
        .rumor()
        .recipients()
        .iter()
        .find(|recipient| **recipient != r.enrollment.input().participant())
        .unwrap()
}
async fn balances(r: &Rig, id: u8, available: u64, preparatory: u64, committed: u64, version: u64) {
    let value = r
        .store
        .pool(
            r.binding.clone(),
            inventory_minimum(r).await,
            pool(id),
            r.enrollment
                .proof()
                .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                .unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    let actual = value.snapshot();
    let physical = actual.balances().physical();
    assert_eq!(
        (
            physical.available.get(),
            physical.preparatory_hold.get(),
            physical.committed_hold.get(),
            actual.version().get()
        ),
        (available, preparatory, committed, version)
    );
    assert!(
        radroots_domain::stock::PoolBalances::new(actual.balances().introduced(), physical).is_ok()
    );
}
#[tokio::test]
async fn peer_only_t2_installs_original_agreement_and_two_work_items_once_across_reopen() {
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    let peer = peer(&r, &receipt);
    let evidence = retain(&r, &receipt, peer).await;
    let before = inventory_minimum(&r).await;
    let first = installed(install(&r, &receipt).await.unwrap());
    assert_eq!(first.record_id(), receipt.rumor().record_id());
    assert_eq!(first.order_id(), receipt.order_id());
    assert_eq!(
        first.attempt().operation_id(),
        receipt.attempt().operation_id()
    );
    assert_eq!(
        first.checkpoint().revision().get(),
        before.revision().get() + 1
    );
    assert_eq!(first.publication_work().len(), 2);
    for work in first.publication_work() {
        assert_eq!(
            work.evidence_digest(),
            if work.recipient() == peer {
                Some(evidence.digest())
            } else {
                None
            }
        );
    }
    assert_eq!(format!("{first:?}"), "StockInstallationReceipt(<redacted>)");
    balances(&r, 80, 9, 0, 1, 3).await;
    balances(&r, 81, 8, 0, 2, 3).await;
    let current = inventory_minimum(&r).await;
    let again = installed(install(&r, &receipt).await.unwrap());
    assert_eq!(first.attempt(), again.attempt());
    assert_eq!(inventory_minimum(&r).await, current);
    reopen_plain(&mut r).await;
    let recovered = r
        .store
        .stock_agreement_installation(request(&r, &receipt).await, r.enrollment.proof())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.attempt(), first.attempt());
    assert_eq!(recovered.publication_work(), first.publication_work());
    let snapshot = r
        .store
        .stock_operation_snapshot(
            StockSnapshotRequest::new(
                r.binding.clone(),
                inventory_minimum(&r).await,
                receipt.attempt().operation_id(),
                receipt.attempt().command_digest(),
            ),
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.installation().unwrap().attempt(), first.attempt());
    assert!(matches!(
        claim(&r, &receipt, peer).await.unwrap(),
        StockClaimOutcome::Existing(_)
    ));
    let mut db = database(&r).await;
    for (sql, expected) in [
        ("SELECT count(*) FROM stock_agreements", 1),
        ("SELECT count(*) FROM stock_publication_work", 2),
        ("SELECT count(*) FROM stock_orders WHERE revision=1", 1),
        ("SELECT count(*) FROM stock_operations WHERE family=6", 1),
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(sql)
                .fetch_one(&mut db)
                .await
                .unwrap(),
            expected
        );
    }
    db.close().await.unwrap();
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn no_evidence_or_self_only_evidence_never_installs_or_releases_the_peer_obligation() {
    let r = Rig::new().await;
    let receipt = ready(&r).await;
    let before = inventory_minimum(&r).await;
    assert_eq!(
        install(&r, &receipt).await.unwrap_err(),
        OperationError::InvalidEvidence
    );
    assert_eq!(inventory_minimum(&r).await, before);
    retain(&r, &receipt, r.enrollment.input().participant()).await;
    let before = inventory_minimum(&r).await;
    assert_eq!(
        install(&r, &receipt).await.unwrap_err(),
        OperationError::InvalidEvidence
    );
    assert_eq!(inventory_minimum(&r).await, before);
    assert!(
        r.store
            .stock_agreement_installation(request(&r, &receipt).await, r.enrollment.proof())
            .await
            .unwrap()
            .is_none()
    );
    assert_pool(&r, 80, 9, 1, 2).await;
    assert_pool(&r, 81, 8, 2, 2).await;
    retain(&r, &receipt, peer(&r, &receipt)).await;
    let value = installed(install(&r, &receipt).await.unwrap());
    assert!(
        value
            .publication_work()
            .iter()
            .all(|value| value.evidence_digest().is_some())
    );
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn obtained_peer_installs_after_first_issue_expiry_and_preserves_available_corrections() {
    let r = Rig::new().await;
    let (command, receipt) = prepared(&r).await;
    bound(bind(&r, &receipt, 42).await.unwrap());
    retain(&r, &receipt, peer(&r, &receipt)).await;
    physical(&r,serde_json::json!({"type":"stock.adjust","context":context(&r,95),"input":{"pool_id":pool(80),"expected_version":"2","direction":"add","amount":"1","reason":"available correction before installation"}})).await;
    r.clock.0.store(250, Ordering::SeqCst);
    assert_eq!(
        install(&r, &receipt).await.unwrap_err(),
        OperationError::DeadlineExceeded
    );
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
    let selected = StockInstallationRequest::new(
        binding,
        inventory_minimum(&r).await,
        receipt.attempt().operation_id(),
        receipt.attempt().command_digest(),
    );
    let first = installed(
        r.store
            .install_stock_agreement(selected, child.proof())
            .await
            .unwrap(),
    );
    assert_eq!(first.installed_at().get(), 250);
    balances(&r, 80, 10, 0, 1, 4).await;
    balances(&r, 81, 8, 0, 2, 3).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn explicit_future_promise_installs_without_manufacturing_physical_stock_or_movements() {
    let r = Rig::new().await;
    r.retain(&r.capture("listing_fixed")).await;
    super::super::super::super::super::retain(&r, "request_future_promise").await;
    let snapshot = inspect_named(&r, "request_future_promise").await;
    let command = command_snapshot(&r, 90, false, &snapshot);
    consent(&r, &command).await;
    let receipt = prepare(&r, &command).await.unwrap();
    bound(bind(&r, &receipt, 42).await.unwrap());
    retain(&r, &receipt, peer(&r, &receipt)).await;
    let first = installed(install(&r, &receipt).await.unwrap());
    assert!(first.installed_pools().is_empty());
    let mut db = database(&r).await;
    for sql in [
        "SELECT count(*) FROM stock_pools",
        "SELECT count(*) FROM stock_movements",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(sql)
                .fetch_one(&mut db)
                .await
                .unwrap(),
            0
        );
    }
    assert_eq!(
        installed(install(&r, &receipt).await.unwrap()).attempt(),
        first.attempt()
    );
    db.close().await.unwrap();
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn every_t2_write_failure_rolls_back_all_pool_agreement_and_work_effects() {
    for statement in [
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operations WHEN NEW.family=6 BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operation_parts WHEN (SELECT family FROM stock_operations WHERE operation_id=NEW.operation_id)=6 BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_agreements BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_publication_work WHEN (SELECT count(*) FROM stock_publication_work)=1 BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_pools WHEN OLD.pool_id=(SELECT max(pool_id) FROM stock_pools) BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_movements BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_order_states BEGIN SELECT RAISE(ABORT,'fault'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'fault'); END",
    ] {
        let r = Rig::new().await;
        let receipt = ready(&r).await;
        retain(&r, &receipt, peer(&r, &receipt)).await;
        let before = inventory_minimum(&r).await;
        let mut db = database(&r).await;
        sqlx::raw_sql(statement).execute(&mut db).await.unwrap();
        assert_eq!(
            install(&r, &receipt).await.unwrap_err().code(),
            "storage_failure",
            "{statement}"
        );
        assert_eq!(inventory_minimum(&r).await, before);
        assert_pool(&r, 80, 9, 1, 2).await;
        assert_pool(&r, 81, 8, 2, 2).await;
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT (SELECT count(*) FROM stock_agreements)+(SELECT count(*) FROM stock_publication_work)").fetch_one(&mut db).await.unwrap(),0);
        sqlx::raw_sql("DROP TRIGGER fault")
            .execute(&mut db)
            .await
            .unwrap();
        installed(install(&r, &receipt).await.unwrap());
        db.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn uncertain_t2_fences_owner_and_reopens_original_evidence_without_another_signature() {
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    let peer = peer(&r, &receipt);
    retain(&r, &receipt, peer).await;
    let selected = request(&r, &receipt).await;
    let before = inventory_minimum(&r).await;
    let mut db = database(&r).await;
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_agreements BEGIN INSERT INTO deferred_fault VALUES(2); END").execute(&mut db).await.unwrap();
    let StockInstallationOutcome::Unknown(anchor) = r
        .store
        .install_stock_agreement(selected, r.enrollment.proof())
        .await
        .unwrap()
    else {
        panic!("COMMIT must be uncertain")
    };
    assert_eq!(anchor.operation_id(), receipt.attempt().operation_id());
    assert!(anchor.transaction_id().is_some());
    let stale = StockInstallationRequest::new(
        r.binding.clone(),
        before,
        receipt.attempt().operation_id(),
        receipt.attempt().command_digest(),
    );
    assert_eq!(
        r.store
            .stock_agreement_installation(stale, r.enrollment.proof())
            .await
            .unwrap_err()
            .code(),
        "recovery_required"
    );
    r.store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER fault;DROP TABLE deferred_fault")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    assert_eq!(inventory_minimum(&r).await, before);
    installed(install(&r, &receipt).await.unwrap());
    assert!(matches!(
        claim(&r, &receipt, peer).await.unwrap(),
        StockClaimOutcome::Existing(_)
    ));
    balances(&r, 80, 9, 0, 1, 3).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn lost_t2_observer_drains_and_reopens_one_atomic_installation() {
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    retain(&r, &receipt, peer(&r, &receipt)).await;
    let selected = request(&r, &receipt).await;
    let provider = reopen(&mut r).await;
    provider.gate.arm(Direction::Wrap, r.authority_schema(), 0);
    let mut pending = Box::pin(
        r.store
            .install_stock_agreement(selected, r.enrollment.proof()),
    );
    assert!(futures::poll!(pending.as_mut()).is_pending());
    provider.gate.entered();
    drop(pending);
    provider.gate.release();
    reopen_plain(&mut r).await;
    let first = r
        .store
        .stock_agreement_installation(request(&r, &receipt).await, r.enrollment.proof())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        installed(install(&r, &receipt).await.unwrap()).attempt(),
        first.attempt()
    );
    balances(&r, 80, 9, 0, 1, 3).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn late_t2_backing_or_grant_or_evidence_change_never_commits_partial_installation() {
    for statement in [
        "UPDATE stock_grants SET protected=randomblob(length(protected))",
        "UPDATE stock_pools SET protected=randomblob(length(protected))",
        "UPDATE stock_signing_evidence SET evidence_digest=randomblob(32)",
        "UPDATE stock_operations SET protected=randomblob(length(protected)) WHERE family=3",
    ] {
        let mut r = Rig::new().await;
        let receipt = ready(&r).await;
        retain(&r, &receipt, peer(&r, &receipt)).await;
        let selected = request(&r, &receipt).await;
        let provider = reopen(&mut r).await;
        let mut db = database(&r).await;
        provider.gate.arm(Direction::Wrap, r.authority_schema(), 0);
        let mut pending = Box::pin(
            r.store
                .install_stock_agreement(selected, r.enrollment.proof()),
        );
        assert!(futures::poll!(pending.as_mut()).is_pending());
        provider.gate.entered();
        sqlx::raw_sql(statement).execute(&mut db).await.unwrap();
        provider.gate.release();
        assert!(pending.await.is_err(), "{statement}");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_agreements")
                .fetch_one(&mut db)
                .await
                .unwrap(),
            0
        );
        db.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn installed_projection_and_original_graph_damage_are_not_treated_as_fresh_absence() {
    for statement in [
        "DELETE FROM stock_publication_work",
        "UPDATE stock_publication_work SET recipient=randomblob(32)",
        "UPDATE stock_agreements SET record_id=randomblob(32)",
        "UPDATE stock_agreements SET commit_sequence=2",
        "UPDATE stock_operations SET protected=randomblob(length(protected)) WHERE family=6",
        "UPDATE stock_movements SET protected=randomblob(length(protected)) WHERE operation_id IN(SELECT operation_id FROM stock_operations WHERE family=6)",
        "DELETE FROM stock_signing_evidence",
    ] {
        let r = Rig::new().await;
        let receipt = ready(&r).await;
        retain(&r, &receipt, peer(&r, &receipt)).await;
        installed(install(&r, &receipt).await.unwrap());
        let selected = request(&r, &receipt).await;
        let mut db = database(&r).await;
        sqlx::raw_sql(statement).execute(&mut db).await.unwrap();
        assert!(
            r.store
                .stock_agreement_installation(selected, r.enrollment.proof())
                .await
                .is_err(),
            "{statement}"
        );
        db.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}
