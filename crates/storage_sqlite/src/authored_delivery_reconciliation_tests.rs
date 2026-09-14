use super::signed_fact_fixture::{claim, ids, record};
use super::*;
use crate::OpenMode;
use radroots_storage::{
    authored::WorkClaim,
    authored_atomic::{ClaimAuthoredWork, ReconcileDeliveryFacts},
};
use tempfile::TempDir;

async fn signed(temp: &TempDir) -> SqliteStorage {
    let (store, event, signing) = signed_fact_tests::prepared(temp).await;
    store
        .execute_authored(record(event, signing, 12))
        .await
        .unwrap();
    store
}

async fn plan(store: &SqliteStorage) -> AuthoredDeliveryPlan {
    store
        .authored_delivery_plan(ids().2)
        .await
        .unwrap()
        .unwrap()
}

async fn issue(store: &SqliteStorage) -> (WorkClaim, AuthoredAtomicReceipt) {
    let active = claim(plan(store).await.revision(), 5, 13);
    let receipt = store
        .execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
            active.clone(),
        )))
        .await
        .unwrap();
    (active, receipt)
}

fn command(plan: &AuthoredDeliveryPlan) -> AuthoredAtomicCommand {
    AuthoredAtomicCommand::ReconcileDelivery(
        ReconcileDeliveryFacts::new(plan, None, None, 51).unwrap(),
    )
}

#[tokio::test]
async fn claim_and_reconciliation_provenance_reopen_exactly_and_survive_stop() {
    let temp = TempDir::new().unwrap();
    let store = signed(&temp).await;
    assert!(
        store
            .authored_delivery_history(ids().2)
            .await
            .unwrap()
            .unwrap()
            .proves_no_issued_attempt()
    );
    let (active, issued) = issue(&store).await;
    let history = store
        .authored_delivery_history(ids().2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(history.claims().len(), 1);
    assert!(history.has_unresolved_claims());
    store
        .execute_authored(delivery_fact_tests::fact(
            &plan(&store).await,
            active.clone(),
        ))
        .await
        .unwrap();
    let before = plan(&store).await;
    let command = command(&before);
    let committed = store.execute_authored(command.clone()).await.unwrap();
    let after = plan(&store).await;
    assert_eq!(after.state(), AuthoredDeliveryState::Satisfied);
    assert_eq!(after.attempt_count(), 1);
    assert_eq!(after.attempts()[0].claim_evidence(), Some(&active));
    assert_eq!(after.delivery_facts(), before.delivery_facts());
    assert_eq!(
        store
            .authored_receipt(issued.commit_id())
            .await
            .unwrap()
            .unwrap(),
        issued
    );
    assert!(
        !store
            .authored_delivery_history(ids().2)
            .await
            .unwrap()
            .unwrap()
            .has_unresolved_claims()
    );
    for sql in [
        "DELETE FROM radroots_runtime_authored_delivery_claims",
        "UPDATE radroots_runtime_authored_delivery_claims SET claim_id = claim_id",
        "DELETE FROM radroots_runtime_authored_delivery_reconciliations",
        "UPDATE radroots_runtime_authored_delivery_reconciliations SET attempt = 2",
    ] {
        assert!(sqlx::query(sql).execute(store.pool()).await.is_err());
    }
    store.close().await.unwrap();
    let store = signed_fact_tests::open(&temp, OpenMode::ReadWriteExisting).await;
    assert_eq!(plan(&store).await, after);
    let replay = store.execute_authored(command).await.unwrap();
    assert_eq!(replay.disposition(), AtomicCommitDisposition::Replay);
    assert_eq!(replay.outcome(), committed.outcome());
    store
        .execute_authored(AuthoredAtomicCommand::Cancel(
            radroots_storage::authored_atomic::CancelAuthoredWork::new(
                CancelAuthoredTarget::DeliveryPlan(ids().2),
                after.revision(),
                60,
            )
            .unwrap(),
        ))
        .await
        .unwrap();
    let stopped = plan(&store).await;
    assert_eq!(stopped.stop_requested_at_unix_ms(), Some(60));
    assert_eq!(stopped.attempts(), after.attempts());
    assert_eq!(stopped.delivery_facts(), after.delivery_facts());
    store.close().await.unwrap();
}

#[tokio::test]
async fn reconciliation_commit_failure_rolls_back_marker_plan_and_receipt() {
    let temp = TempDir::new().unwrap();
    let store = signed(&temp).await;
    let (active, _) = issue(&store).await;
    store
        .execute_authored(delivery_fact_tests::fact(&plan(&store).await, active))
        .await
        .unwrap();
    let before = plan(&store).await;
    let before_history = store
        .authored_delivery_history(ids().2)
        .await
        .unwrap()
        .unwrap();
    let command = command(&before);
    sqlx::query("CREATE TABLE reconciliation_commit_fault (parent BLOB REFERENCES radroots_runtime_authored_operations(operation_id) DEFERRABLE INITIALLY DEFERRED)").execute(store.pool()).await.unwrap();
    sqlx::query("CREATE TRIGGER reconciliation_commit_fault_trigger AFTER INSERT ON radroots_runtime_authored_delivery_reconciliations BEGIN INSERT INTO reconciliation_commit_fault VALUES (x'99999999999999999999999999999999'); END").execute(store.pool()).await.unwrap();
    assert!(store.execute_authored(command.clone()).await.is_err());
    assert_eq!(plan(&store).await, before);
    assert_eq!(
        store
            .authored_delivery_history(ids().2)
            .await
            .unwrap()
            .unwrap(),
        before_history
    );
    assert!(
        store
            .authored_receipt(command.commit_id())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM radroots_runtime_authored_delivery_reconciliations"
        )
        .fetch_one(store.pool())
        .await
        .unwrap(),
        0
    );
    sqlx::query("DROP TRIGGER reconciliation_commit_fault_trigger")
        .execute(store.pool())
        .await
        .unwrap();
    store.execute_authored(command).await.unwrap();
    assert_eq!(plan(&store).await.attempt_count(), 1);
    // The existing writer may replace these rows only within its transaction;
    // a committed missing attempt must never orphan retained reconciliation.
    assert!(
        sqlx::query("DELETE FROM radroots_runtime_authored_delivery_attempts")
            .execute(store.pool())
            .await
            .is_err()
    );
    assert_eq!(plan(&store).await.attempt_count(), 1);
    store.close().await.unwrap();
}

#[tokio::test]
async fn original_claim_commit_failure_preserves_no_issued_attempt_proof() {
    let temp = TempDir::new().unwrap();
    let store = signed(&temp).await;
    let before = plan(&store).await;
    let active = claim(before.revision(), 5, 13);
    let command = AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
        ClaimAuthoredTarget::DeliveryPlan(ids().2),
        active,
    ));
    sqlx::query("CREATE TABLE claim_index_commit_fault (parent BLOB REFERENCES radroots_runtime_authored_operations(operation_id) DEFERRABLE INITIALLY DEFERRED)").execute(store.pool()).await.unwrap();
    sqlx::query("CREATE TRIGGER claim_index_commit_fault_trigger AFTER INSERT ON radroots_runtime_authored_delivery_claims BEGIN INSERT INTO claim_index_commit_fault VALUES (x'99999999999999999999999999999999'); END").execute(store.pool()).await.unwrap();
    assert!(store.execute_authored(command.clone()).await.is_err());
    assert_eq!(plan(&store).await, before);
    assert!(
        store
            .authored_receipt(command.commit_id())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .authored_delivery_history(ids().2)
            .await
            .unwrap()
            .unwrap()
            .proves_no_issued_attempt()
    );
    sqlx::query("DROP TRIGGER claim_index_commit_fault_trigger")
        .execute(store.pool())
        .await
        .unwrap();
    store.execute_authored(command).await.unwrap();
    let issued = store
        .authored_delivery_history(ids().2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(issued.claims().len(), 1);
    assert!(issued.has_unresolved_claims());
    assert!(!issued.proves_no_issued_attempt());
    store.close().await.unwrap();
}

#[tokio::test]
async fn history_read_snapshot_cannot_mix_old_plan_with_new_claim_or_reconciliation() {
    let temp = TempDir::new().unwrap();
    let store = signed(&temp).await;
    let mut read = store.pool().begin().await.unwrap();
    let frozen = delivery_reconciliation::history(&mut read, ids().2)
        .await
        .unwrap()
        .unwrap();
    assert!(frozen.proves_no_issued_attempt());
    let (active, _) = issue(&store).await;
    assert_eq!(
        delivery_reconciliation::history(&mut read, ids().2)
            .await
            .unwrap()
            .unwrap(),
        frozen
    );
    read.rollback().await.unwrap();
    let mut read = store.pool().begin().await.unwrap();
    let issued = delivery_reconciliation::history(&mut read, ids().2)
        .await
        .unwrap()
        .unwrap();
    store
        .execute_authored(delivery_fact_tests::fact(&plan(&store).await, active))
        .await
        .unwrap();
    store
        .execute_authored(command(&plan(&store).await))
        .await
        .unwrap();
    assert_eq!(
        delivery_reconciliation::history(&mut read, ids().2)
            .await
            .unwrap()
            .unwrap(),
        issued
    );
    read.rollback().await.unwrap();
    let after = store
        .authored_delivery_history(ids().2)
        .await
        .unwrap()
        .unwrap();
    assert!(!after.has_unresolved_claims());
    assert_eq!(after.plan().attempt_count(), 1);
    let mut stale = store.pool().begin().await.unwrap();
    assert_eq!(
        delivery_reconciliation::persist(&mut stale, issued.plan()).await,
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    stale.rollback().await.unwrap();
    assert_eq!(
        store
            .authored_delivery_history(ids().2)
            .await
            .unwrap()
            .unwrap(),
        after
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn oversized_historical_claims_remain_retained_but_cannot_authorize_new_work() {
    let temp = TempDir::new().unwrap();
    let store = signed(&temp).await;
    let mut current = plan(&store).await;
    let mut transaction = store.pool().begin().await.unwrap();
    // Historical databases may exceed the new admission bound. Construct their
    // typed original receipts without invoking the now-bounded command path.
    for index in 0..1025u64 {
        let at = 13 + index * 21;
        let active = WorkClaim::new(
            [7; 16],
            "historical-worker",
            std::num::NonZeroU64::new(index + 5).unwrap(),
            at,
            at + 20,
            current.revision(),
        )
        .unwrap();
        current.claim(active.clone(), at).unwrap();
        let command = AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
            active,
        ));
        let receipt = AuthoredAtomicReceipt::new(
            &command,
            AtomicCommitDisposition::Committed,
            at,
            AuthoredAtomicOutcome::DeliveryPlan(current.clone()),
        )
        .unwrap();
        sqlx::query("INSERT INTO radroots_runtime_authored_atomic_commits (commit_id, commit_digest, phase, target_id, requested_at_unix_ms, committed_at_unix_ms, receipt) VALUES (?, ?, 'claim', ?, ?, ?, ?)")
            .bind(receipt.commit_id().as_bytes().as_slice()).bind(receipt.digest().as_bytes().as_slice()).bind(ids().2.as_bytes().as_slice()).bind(at as i64).bind(at as i64)
            .bind(serde_json::to_vec(&serde_json::json!({"outcome": receipt.outcome()})).unwrap()).execute(&mut *transaction).await.unwrap();
        sqlx::query("INSERT INTO radroots_runtime_authored_delivery_claims (plan_id, claim_id) VALUES (?, ?)")
            .bind(ids().2.as_bytes().as_slice()).bind(receipt.commit_id().as_bytes().as_slice()).execute(&mut *transaction).await.unwrap();
    }
    persist_plan(&mut transaction, &current).await.unwrap();
    transaction.commit().await.unwrap();
    let history = store
        .authored_delivery_history(ids().2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(history.claims().len(), 1024);
    assert!(history.is_truncated());
    assert!(!history.is_complete());
    assert!(!history.proves_no_issued_attempt());
    assert!(history.has_unresolved_claims());
    assert_eq!(
        history.require_pending_fact_provenance(),
        Err(Error::DeliveryAttemptOverflow)
    );
    let at = 13 + 1025 * 21;
    let active = WorkClaim::new(
        [8; 16],
        "new-worker",
        std::num::NonZeroU64::new(2048).unwrap(),
        at,
        at + 20,
        current.revision(),
    )
    .unwrap();
    let command = AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
        ClaimAuthoredTarget::DeliveryPlan(ids().2),
        active,
    ));
    assert_eq!(
        store.execute_authored(command.clone()).await,
        Err(Error::DeliveryAttemptOverflow)
    );
    assert_eq!(plan(&store).await, current);
    assert!(
        store
            .authored_receipt(command.commit_id())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM radroots_runtime_authored_delivery_claims"
        )
        .fetch_one(store.pool())
        .await
        .unwrap(),
        1025
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn forged_indexed_claim_receipt_fails_closed_instead_of_proving_absence() {
    let temp = TempDir::new().unwrap();
    let store = signed(&temp).await;
    let (_, issued) = issue(&store).await;
    let before = plan(&store).await;
    sqlx::query("INSERT INTO radroots_runtime_authored_atomic_commits (commit_id, commit_digest, phase, target_id, requested_at_unix_ms, committed_at_unix_ms, receipt) VALUES (?, ?, 'claim', ?, 13, 13, ?)")
        .bind([9u8; 16].as_slice()).bind([8u8; 32].as_slice()).bind(ids().2.as_bytes().as_slice())
        .bind(serde_json::to_vec(&serde_json::json!({"outcome": issued.outcome()})).unwrap()).execute(store.pool()).await.unwrap();
    sqlx::query(
        "INSERT INTO radroots_runtime_authored_delivery_claims (plan_id, claim_id) VALUES (?, ?)",
    )
    .bind(ids().2.as_bytes().as_slice())
    .bind([9u8; 16].as_slice())
    .execute(store.pool())
    .await
    .unwrap();
    assert!(store.authored_delivery_history(ids().2).await.is_err());
    assert_eq!(plan(&store).await, before);
    store.close().await.unwrap();
}

#[tokio::test]
async fn late_legacy_marker_can_precede_existing_marker_without_rewriting_it() {
    use radroots_storage::{
        authored::{FailureClass, RetrySchedule, WorkFailure, WorkPhase},
        authored_atomic::{ApplyDeliveryAttempt, RecordDeliveryFact, WorkFence},
        authored_delivery::DeliveryAttemptOutcome,
    };
    use radroots_transport::{
        DeliveryReceipt, outcome::DeliveryOutcome, sink::DeliveryTargetReceipt,
    };
    let retry = |attempt, at| {
        RetrySchedule::new(
            std::num::NonZeroU32::new(attempt).unwrap(),
            at,
            WorkFailure::new(
                "delivery_pending",
                WorkPhase::Delivery,
                FailureClass::Retryable,
                Some(at),
                None,
            )
            .unwrap(),
        )
        .unwrap()
    };
    let fence = |claim: &WorkClaim| {
        WorkFence::new(*claim.token(), claim.generation(), claim.row_revision()).unwrap()
    };
    let temp = TempDir::new().unwrap();
    let store = signed(&temp).await;
    let (first, _) = issue(&store).await;
    let current = plan(&store).await;
    let request = current.request().unwrap();
    let outcome = DeliveryAttemptOutcome::Receipt(
        DeliveryReceipt::for_request(
            request,
            request
                .target_set()
                .targets()
                .iter()
                .cloned()
                .map(|target| {
                    DeliveryTargetReceipt::attempted(target, DeliveryOutcome::unavailable())
                })
                .collect(),
        )
        .unwrap(),
    );
    store
        .execute_authored(AuthoredAtomicCommand::ApplyDelivery(
            ApplyDeliveryAttempt::new(
                ids().2,
                fence(&first),
                outcome.clone(),
                Some(retry(1, 18)),
                14,
            )
            .unwrap(),
        ))
        .await
        .unwrap();
    let second = claim(plan(&store).await.revision(), 6, 20);
    store
        .execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
            second.clone(),
        )))
        .await
        .unwrap();
    store
        .execute_authored(AuthoredAtomicCommand::RecordDelivery(
            RecordDeliveryFact::new(ids().2, ids().1, second.clone(), outcome.clone(), 21).unwrap(),
        ))
        .await
        .unwrap();
    store
        .execute_authored(AuthoredAtomicCommand::ReconcileDelivery(
            ReconcileDeliveryFacts::new(
                &plan(&store).await,
                Some(fence(&second)),
                Some(retry(2, 25)),
                22,
            )
            .unwrap(),
        ))
        .await
        .unwrap();
    let marker_before: Vec<u8> = sqlx::query_scalar(
        "SELECT claim_id FROM radroots_runtime_authored_delivery_reconciliations WHERE attempt = 2",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    store
        .execute_authored(AuthoredAtomicCommand::RecordDelivery(
            RecordDeliveryFact::new(ids().2, ids().1, first.clone(), outcome, 50).unwrap(),
        ))
        .await
        .unwrap();
    store
        .execute_authored(AuthoredAtomicCommand::ReconcileDelivery(
            ReconcileDeliveryFacts::new(&plan(&store).await, None, Some(retry(2, 60)), 51).unwrap(),
        ))
        .await
        .unwrap();
    let after = plan(&store).await;
    assert_eq!(after.attempt_count(), 2);
    assert_eq!(after.attempts()[0].recorded_at_unix_ms(), 14);
    assert_eq!(after.attempts()[0].claim_evidence(), Some(&first));
    assert_eq!(after.attempts()[1].recorded_at_unix_ms(), 22);
    assert_eq!(after.attempts()[1].claim_evidence(), Some(&second));
    assert_eq!(sqlx::query_scalar::<_, Vec<u8>>("SELECT claim_id FROM radroots_runtime_authored_delivery_reconciliations WHERE attempt = 2").fetch_one(store.pool()).await.unwrap(), marker_before);
    store.close().await.unwrap();
    let store = signed_fact_tests::open(&temp, OpenMode::ReadWriteExisting).await;
    assert_eq!(plan(&store).await, after);
    assert!(
        !store
            .authored_delivery_history(ids().2)
            .await
            .unwrap()
            .unwrap()
            .has_unresolved_claims()
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn history_rejects_ambiguous_preparation_and_distinguishes_missing_plan() {
    let temp = TempDir::new().unwrap();
    let store = signed(&temp).await;
    assert!(
        store
            .authored_delivery_history(AuthoredDeliveryPlanId::new([99; 16]).unwrap())
            .await
            .unwrap()
            .is_none()
    );
    let before = plan(&store).await;
    sqlx::query("INSERT INTO radroots_runtime_authored_atomic_commits (commit_id, commit_digest, phase, target_id, requested_at_unix_ms, committed_at_unix_ms, receipt) SELECT ?, commit_digest, phase, target_id, requested_at_unix_ms, committed_at_unix_ms, receipt FROM radroots_runtime_authored_atomic_commits WHERE phase = 'prepare'")
        .bind([9u8; 16].as_slice()).execute(store.pool()).await.unwrap();
    assert_eq!(
        store.authored_delivery_history(ids().2).await,
        Err(Error::AtomicWorkflowMismatch)
    );
    assert_eq!(plan(&store).await, before);
    store.close().await.unwrap();
}

#[tokio::test]
async fn missing_or_rebound_normalized_marker_cannot_return_a_valid_plan() {
    for missing in [false, true] {
        let temp = TempDir::new().unwrap();
        let store = signed(&temp).await;
        let (first, _) = issue(&store).await;
        let second = claim(plan(&store).await.revision(), 6, 40);
        let second_command = AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
            second,
        ));
        store
            .execute_authored(second_command.clone())
            .await
            .unwrap();
        store
            .execute_authored(delivery_fact_tests::fact(&plan(&store).await, first))
            .await
            .unwrap();
        store
            .execute_authored(AuthoredAtomicCommand::ReconcileDelivery(
                ReconcileDeliveryFacts::new(&plan(&store).await, None, None, 61).unwrap(),
            ))
            .await
            .unwrap();
        let before: Vec<u8> =
            sqlx::query_scalar("SELECT snapshot FROM radroots_runtime_authored_delivery_plans")
                .fetch_one(store.pool())
                .await
                .unwrap();
        // Isolated corruption fixtures deliberately remove the applicable guard.
        // The normal command path cannot erase or rebind these immutable rows.
        if missing {
            sqlx::query(
                "DROP TRIGGER radroots_runtime_authored_delivery_reconciliations_delete_guard",
            )
            .execute(store.pool())
            .await
            .unwrap();
            sqlx::query("DELETE FROM radroots_runtime_authored_delivery_reconciliations")
                .execute(store.pool())
                .await
                .unwrap();
        } else {
            sqlx::query(
                "DROP TRIGGER radroots_runtime_authored_delivery_reconciliations_update_guard",
            )
            .execute(store.pool())
            .await
            .unwrap();
            sqlx::query(
                "UPDATE radroots_runtime_authored_delivery_reconciliations SET claim_id = ?",
            )
            .bind(second_command.commit_id().as_bytes().as_slice())
            .execute(store.pool())
            .await
            .unwrap();
        }
        assert_eq!(
            store.authored_delivery_plan(ids().2).await,
            Err(Error::InvalidAuthoredDeliveryPlan)
        );
        assert_eq!(
            store.authored_delivery_history(ids().2).await,
            Err(Error::InvalidAuthoredDeliveryPlan)
        );
        assert_eq!(
            sqlx::query_scalar::<_, Vec<u8>>(
                "SELECT snapshot FROM radroots_runtime_authored_delivery_plans"
            )
            .fetch_one(store.pool())
            .await
            .unwrap(),
            before
        );
        store.close().await.unwrap();
    }
}
