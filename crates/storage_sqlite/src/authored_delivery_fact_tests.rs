use super::signed_fact_fixture::{RAW, claim, ids, record};
use super::*;
use crate::OpenMode;
use radroots_storage::{
    authored::WorkClaim,
    authored_atomic::{CancelAuthoredWork, ClaimAuthoredWork, RecordDeliveryFact},
};
use radroots_transport::{DeliveryReceipt, outcome::DeliveryOutcome, sink::DeliveryTargetReceipt};
use tempfile::TempDir;

async fn prepared(temp: &TempDir) -> (SqliteStorage, WorkClaim, AuthoredAtomicReceipt) {
    let (store, event, signing) = super::signed_fact_tests::prepared(temp).await;
    store
        .execute_authored(record(event, signing, 12))
        .await
        .unwrap();
    let plan = store
        .authored_delivery_plan(ids().2)
        .await
        .unwrap()
        .unwrap();
    let active = claim(plan.revision(), 5, 13);
    let original = store
        .execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
            active.clone(),
        )))
        .await
        .unwrap();
    (store, active, original)
}

pub(super) fn fact(plan: &AuthoredDeliveryPlan, active: WorkClaim) -> AuthoredAtomicCommand {
    let request = plan.request().unwrap();
    let receipt = DeliveryReceipt::for_request(
        request,
        request
            .target_set()
            .targets()
            .iter()
            .cloned()
            .map(|target| DeliveryTargetReceipt::attempted(target, DeliveryOutcome::accepted()))
            .collect(),
    )
    .unwrap();
    AuthoredAtomicCommand::RecordDelivery(
        RecordDeliveryFact::new(
            ids().2,
            ids().1,
            active,
            DeliveryAttemptOutcome::Receipt(receipt),
            50,
        )
        .unwrap(),
    )
}

#[tokio::test]
async fn stopped_late_delivery_reopens_and_preserves_original_receipt_and_exact_raw() {
    let temp = TempDir::new().unwrap();
    let (store, active, original) = prepared(&temp).await;
    let plan = store
        .authored_delivery_plan(ids().2)
        .await
        .unwrap()
        .unwrap();
    let command = fact(&plan, active);
    store
        .execute_authored(AuthoredAtomicCommand::Cancel(
            CancelAuthoredWork::new(
                CancelAuthoredTarget::DeliveryPlan(ids().2),
                plan.revision(),
                20,
            )
            .unwrap(),
        ))
        .await
        .unwrap();
    let receipt = store.execute_authored(command.clone()).await.unwrap();
    let retained = store
        .authored_delivery_plan(ids().2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.state(), AuthoredDeliveryState::Cancelled);
    assert_eq!(retained.stop_requested_at_unix_ms(), Some(20));
    assert_eq!(
        retained.delivery_satisfaction().unwrap(),
        SatisfactionState::Satisfied
    );
    assert_eq!(
        retained.request().unwrap().payload().event().raw_json(),
        RAW
    );
    assert_eq!(retained.delivery_facts().len(), 1);
    for sql in [
        "UPDATE radroots_runtime_authored_delivery_facts SET observed_at_unix_ms = 99",
        "DELETE FROM radroots_runtime_authored_delivery_facts",
        "UPDATE radroots_runtime_authored_delivery_plans SET stop_requested_at_unix_ms = NULL",
    ] {
        assert!(sqlx::query(sql).execute(store.pool()).await.is_err());
    }
    store.close().await.unwrap();
    let store = super::signed_fact_tests::open(&temp, OpenMode::ReadWriteExisting).await;
    assert_eq!(
        store
            .authored_delivery_plan(ids().2)
            .await
            .unwrap()
            .unwrap(),
        retained
    );
    assert_eq!(
        store
            .authored_receipt(original.commit_id())
            .await
            .unwrap()
            .unwrap(),
        original
    );
    let replay = store.execute_authored(command).await.unwrap();
    assert_eq!(replay.disposition(), AtomicCommitDisposition::Replay);
    assert_eq!(replay.outcome(), receipt.outcome());
    assert_eq!(
        store
            .authored_delivery_plan(ids().2)
            .await
            .unwrap()
            .unwrap(),
        retained
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn real_delivery_commit_failure_rolls_back_facts_snapshot_and_receipt() {
    let temp = TempDir::new().unwrap();
    let (store, active, _) = prepared(&temp).await;
    let before = store
        .authored_delivery_plan(ids().2)
        .await
        .unwrap()
        .unwrap();
    let command = fact(&before, active);
    sqlx::query("CREATE TABLE delivery_fact_commit_fault (parent BLOB REFERENCES radroots_runtime_authored_operations(operation_id) DEFERRABLE INITIALLY DEFERRED)").execute(store.pool()).await.unwrap();
    sqlx::query("CREATE TRIGGER delivery_fact_commit_fault_trigger AFTER INSERT ON radroots_runtime_authored_atomic_commits WHEN NEW.phase IN ('delivery', 'cancel') BEGIN INSERT INTO delivery_fact_commit_fault VALUES (x'99999999999999999999999999999999'); END").execute(store.pool()).await.unwrap();
    let stop = AuthoredAtomicCommand::Cancel(
        CancelAuthoredWork::new(
            CancelAuthoredTarget::DeliveryPlan(ids().2),
            before.revision(),
            20,
        )
        .unwrap(),
    );
    assert!(store.execute_authored(stop.clone()).await.is_err());
    assert_eq!(
        store
            .authored_delivery_plan(ids().2)
            .await
            .unwrap()
            .unwrap(),
        before
    );
    assert!(
        store
            .authored_receipt(stop.commit_id())
            .await
            .unwrap()
            .is_none()
    );
    assert!(store.execute_authored(command.clone()).await.is_err());
    assert_eq!(
        store
            .authored_delivery_plan(ids().2)
            .await
            .unwrap()
            .unwrap(),
        before
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
            "SELECT COUNT(*) FROM radroots_runtime_authored_delivery_facts"
        )
        .fetch_one(store.pool())
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM delivery_fact_commit_fault")
            .fetch_one(store.pool())
            .await
            .unwrap(),
        0
    );
    sqlx::query("DROP TRIGGER delivery_fact_commit_fault_trigger")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("DROP TABLE delivery_fact_commit_fault")
        .execute(store.pool())
        .await
        .unwrap();
    store.close().await.unwrap();
    let store = super::signed_fact_tests::open(&temp, OpenMode::ReadWriteExisting).await;
    assert_eq!(
        store
            .authored_delivery_plan(ids().2)
            .await
            .unwrap()
            .unwrap(),
        before
    );
    store.execute_authored(command.clone()).await.unwrap();
    assert_eq!(
        store
            .authored_delivery_plan(ids().2)
            .await
            .unwrap()
            .unwrap()
            .delivery_satisfaction()
            .unwrap(),
        SatisfactionState::Satisfied
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn forged_claim_and_normalized_fact_corruption_fail_closed() {
    let temp = TempDir::new().unwrap();
    let (store, active, _) = prepared(&temp).await;
    let before = store
        .authored_delivery_plan(ids().2)
        .await
        .unwrap()
        .unwrap();
    let forged = WorkClaim::new(
        *active.token(),
        "forged-owner",
        active.generation(),
        active.acquired_at_unix_ms(),
        active.expires_at_unix_ms(),
        active.row_revision(),
    )
    .unwrap();
    assert_eq!(
        store.execute_authored(fact(&before, forged)).await,
        Err(Error::AtomicWorkflowMismatch)
    );
    assert_eq!(
        store
            .authored_delivery_plan(ids().2)
            .await
            .unwrap()
            .unwrap(),
        before
    );
    store.execute_authored(fact(&before, active)).await.unwrap();
    sqlx::query("DROP TRIGGER radroots_runtime_authored_delivery_facts_update_guard")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE radroots_runtime_authored_delivery_facts SET observed_at_unix_ms = 99")
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(
        store.authored_delivery_plan(ids().2).await,
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn delivery_read_snapshot_remains_consistent_across_a_late_commit() {
    let temp = TempDir::new().unwrap();
    let (store, active, _) = prepared(&temp).await;
    let before = store
        .authored_delivery_plan(ids().2)
        .await
        .unwrap()
        .unwrap();
    let mut read = store.pool().begin().await.unwrap();
    let frozen = load_optional_plan_tx(&mut read, ids().2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(frozen, before);
    store.execute_authored(fact(&before, active)).await.unwrap();
    validate_plan_children_tx(&mut read, &frozen).await.unwrap();
    assert_eq!(
        load_optional_plan_tx(&mut read, ids().2)
            .await
            .unwrap()
            .unwrap(),
        frozen
    );
    read.rollback().await.unwrap();
    let current = store
        .authored_delivery_plan(ids().2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.delivery_facts().len(), 1);
    assert_eq!(current.revision(), before.revision());
    let mut stale = store.pool().begin().await.unwrap();
    assert_eq!(
        delivery_facts::persist(&mut stale, &before).await,
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    stale.rollback().await.unwrap();
    assert_eq!(
        store
            .authored_delivery_plan(ids().2)
            .await
            .unwrap()
            .unwrap(),
        current
    );
    store.close().await.unwrap();
}
