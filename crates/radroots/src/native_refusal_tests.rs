//! Actual ordinary-session refusal ownership, SQL faults and lost observation.
use super::{
    AcceptOrderInput, AccessPolicy, AllocationSelection, BackendKind, CapturedOrderAcceptance,
    KeyVersion, NativeWorkspace, Ordering, ProviderError, Rig, SecretId, SecretMaterial, SecretRef,
    StockSnapshotRequest, UnixSeconds, WorkspaceClock, WorkspaceCrypto,
};
use sqlx::Connection;

#[tokio::test]
async fn actual_original_refusals_allow_explicit_cleanup_and_aborted_status_never_reissues() {
    let r = Rig::new().await;
    r.control.fail_peer.store(true, Ordering::SeqCst);
    r.control.fail_self.store(true, Ordering::SeqCst);
    let (command, original) = r.accept().await;
    assert!(!original.aborted());
    assert!(original.installation().is_none());
    for facts in original.recipients() {
        assert!(facts.evidence.is_none());
        assert_eq!(
            facts.original_unissued.unwrap().claim,
            facts.claim.unwrap().id
        );
    }
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    let (_, _, minimum) = r.minima().await;
    let radroots_storage::inventory::StockAbortOutcome::Aborted(released) = r
        .client
        .operations(&r.scope)
        .abort_proven_unissued(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap()
    else {
        panic!("actual cleanup must be confirmed");
    };
    let (_, _, minimum) = r.minima().await;
    let observed = r
        .client
        .orders(&r.scope)
        .acceptance_status(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert!(observed.aborted());
    assert!(observed.installation().is_none());
    assert!(observed.recipients() == original.recipients());
    r.control.fail_peer.store(false, Ordering::SeqCst);
    r.control.fail_self.store(false, Ordering::SeqCst);
    let resumed = r
        .client
        .orders(&r.scope)
        .resume_acceptance(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert!(resumed.aborted());
    assert!(resumed.recipients() == original.recipients());
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    assert_eq!(r.control.recoveries.load(Ordering::SeqCst), 0);
    assert_eq!(
        r.client
            .operations(&r.scope)
            .preparation_abort(minimum, command.context().operation_id(), command.digest())
            .await
            .unwrap()
            .unwrap()
            .attempt(),
        released.attempt()
    );
    let mut raw = sqlx::SqliteConnection::connect(&format!(
        "sqlite://{}",
        r._temp
            .path()
            .join("stock/stock_execution_v1.sqlite")
            .display()
    ))
    .await
    .unwrap();
    let facts: (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM stock_signing_claims),(SELECT count(*) FROM stock_original_unissued),(SELECT count(*) FROM stock_signing_evidence),(SELECT count(*) FROM stock_replay_admissions),(SELECT count(*) FROM stock_aborts)",
    )
    .fetch_one(&mut raw)
    .await
    .unwrap();
    assert_eq!(facts, (2, 2, 0, 0, 1));
    raw.close().await.unwrap();
    r.client.close().await.unwrap();
}

async fn fault(r: &Rig, uncertain: bool) -> sqlx::SqliteConnection {
    let path = r._temp.path().join("stock/stock_execution_v1.sqlite");
    let mut connection = sqlx::SqliteConnection::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    if uncertain {
        sqlx::raw_sql("PRAGMA foreign_keys=ON; CREATE TABLE deferred_refusal_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER controlled_refusal_fault AFTER INSERT ON stock_original_unissued BEGIN INSERT INTO deferred_refusal_fault VALUES(2); END").execute(&mut connection).await.unwrap();
    } else {
        sqlx::query("CREATE TRIGGER controlled_refusal_fault BEFORE INSERT ON stock_original_unissued BEGIN SELECT RAISE(ABORT,'original_refusal_fault'); END").execute(&mut connection).await.unwrap();
    }
    connection
}
async fn remove_fault(connection: &mut sqlx::SqliteConnection) {
    sqlx::raw_sql(
        "DROP TRIGGER controlled_refusal_fault; DROP TABLE IF EXISTS deferred_refusal_fault",
    )
    .execute(&mut *connection)
    .await
    .unwrap();
}
fn assert_fact(status: &crate::orders::AcceptanceStatus, command: &CapturedOrderAcceptance) {
    assert!(status.installation().is_some());
    let value = status
        .recipients()
        .iter()
        .find(|v| v.original_unissued.is_some())
        .unwrap();
    let receipt = value.original_unissued.unwrap();
    assert_eq!(receipt.claim, value.claim.unwrap().id);
    assert_eq!(
        receipt.attempt.operation_id(),
        command.context().operation_id()
    );
    assert_eq!(receipt.attempt.command_digest(), command.digest());
    assert!(receipt.attempt.transaction_id().is_some());
    assert_eq!(receipt.reason, ProviderError::Cancelled);
    assert!(value.evidence.is_none());
    assert!(!status.aborted());
}

#[tokio::test]
async fn actual_refusal_append_failure_is_owned_and_explicit_drain_needs_no_clock_or_reissue() {
    let r = Rig::new().await;
    r.control.fail_self.store(true, Ordering::SeqCst);
    let mut connection = fault(&r, false).await;
    let (command, context) = r.captured().await;
    let (private, public, minimum) = r.minima().await;
    let input = || {
        AcceptOrderInput::new(
            command.input().request(),
            command.input().terms_digest(),
            AllocationSelection::automatic(),
        )
    };
    let failure = r
        .client
        .orders(&r.scope)
        .accept(context, minimum, private, public, input())
        .await
        .unwrap_err();
    assert_eq!(failure.code(), crate::Code::StorageFailure);
    assert_eq!(failure.context().admission(), crate::Admission::Confirmed);
    assert_eq!(
        failure.context().operation_id(),
        Some(command.context().operation_id())
    );
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    let context = r
        .scope
        .command_context(
            command.context().operation_id(),
            command.context().expected().cloned(),
            command.context().approval().cloned(),
            UnixSeconds::new(230).unwrap(),
        )
        .unwrap();
    let blocked = r
        .client
        .orders(&r.scope)
        .accept(context, minimum, private, public, input())
        .await
        .unwrap_err();
    assert_eq!(blocked.code(), crate::Code::RecoveryRequired);
    remove_fault(&mut connection).await;
    connection.close().await.unwrap();
    r.clock.0.store(u64::MAX, Ordering::SeqCst);
    assert!(r.clock.now().is_err());
    r.client.stock(&r.scope).drain_evidence().await.unwrap();
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    assert_eq!(r.control.recoveries.load(Ordering::SeqCst), 0);
    r.clock.0.store(150, Ordering::SeqCst);
    let status = r
        .client
        .orders(&r.scope)
        .acceptance_status(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert_fact(&status, &command);
    r.client.close().await.unwrap();
    assert!(r.provider.upgrade().is_none());
}

#[tokio::test]
async fn lost_refusal_waiter_and_failed_close_retain_owner_through_rollback_and_commit_uncertainty()
{
    for uncertain in [false, true] {
        let r = Rig::new().await;
        r.control.fail_self.store(true, Ordering::SeqCst);
        r.control.pause_refused.store(true, Ordering::SeqCst);
        let (command, context) = r.captured().await;
        let (private, public, minimum) = r.minima().await;
        let input = AcceptOrderInput::new(
            command.input().request(),
            command.input().terms_digest(),
            AllocationSelection::automatic(),
        );
        let client = r.client.clone();
        let scope = r.scope.clone();
        let observer = tokio::spawn(async move {
            client
                .orders(&scope)
                .accept(context, minimum, private, public, input)
                .await
        });
        // Ordinary acceptance now retains and delivers the installed peer
        // before reaching this self-signing barrier. Keep the same assertions
        // while bounding observation of that larger debug-build prefix.
        tokio::time::timeout(
            std::time::Duration::from_secs(120),
            r.control.entered.notified(),
        )
        .await
        .unwrap();
        assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
        observer.abort();
        assert!(observer.await.unwrap_err().is_cancelled());
        let mut connection = fault(&r, uncertain).await;
        r.control.release.notify_one();
        let error = r.client.close().await.unwrap_err();
        assert_eq!(
            error.code(),
            if uncertain {
                crate::Code::RecoveryRequired
            } else {
                crate::Code::StorageFailure
            }
        );
        assert_eq!(error.context().admission(), crate::Admission::Confirmed);
        assert_eq!(
            error.context().operation_id(),
            Some(command.context().operation_id())
        );
        assert!(r.provider.upgrade().is_some());
        remove_fault(&mut connection).await;
        connection.close().await.unwrap();
        r.clock.0.store(u64::MAX, Ordering::SeqCst);
        assert!(r.clock.now().is_err());
        r.client.close().await.unwrap();
        assert!(r.provider.upgrade().is_none());
        assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
        assert_eq!(r.control.recoveries.load(Ordering::SeqCst), 0);
        r.clock.0.store(150, Ordering::SeqCst);
        let crypto = WorkspaceCrypto::new(
            r.vault.clone(),
            SecretRef::new(
                SecretId::parse("workspace-key").unwrap(),
                BackendKind::File,
                KeyVersion::new(1).unwrap(),
            ),
            SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
            AccessPolicy::standard(),
        )
        .unwrap();
        let reopened = NativeWorkspace::open_seller_stock(
            &r._temp.path().join("stock"),
            r.enrollment.input().clone(),
            crypto,
            r.clock.clone(),
        )
        .await
        .unwrap();
        let binding = reopened
            .authenticate(r.enrollment.input().clone(), r.enrollment.proof())
            .await
            .unwrap();
        let snapshot = reopened
            .stock_operation_snapshot(
                StockSnapshotRequest::new(
                    binding,
                    minimum,
                    command.context().operation_id(),
                    command.digest(),
                ),
                r.enrollment.proof(),
            )
            .await
            .unwrap();
        assert!(snapshot.installation().is_some());
        let facts = snapshot
            .recipients()
            .iter()
            .filter_map(|v| v.original_unissued())
            .collect::<Vec<_>>();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].reason(), ProviderError::Cancelled);
        assert_eq!(
            snapshot
                .recipients()
                .iter()
                .filter(|v| v.first_evidence().is_some())
                .count(),
            1
        );
        assert!(snapshot.aborted().is_none());
        reopened.close().await.unwrap();
    }
}
