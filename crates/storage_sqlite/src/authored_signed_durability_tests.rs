use super::{
    tests::{fence, ids, prepare, signed},
    *,
};
use crate::{OpenMode, OpenOptions, Paths};
use core::num::NonZeroU64;
use radroots_storage::{
    authored::WorkClaim,
    authored_atomic::{ApplySignedArtifact, ClaimAuthoredWork},
    event::SourceGeneration,
};
use tempfile::TempDir;

async fn open(temp: &TempDir, mode: OpenMode) -> SqliteStorage {
    let options = OpenOptions::new(Paths::from_directory(temp.path()).unwrap(), mode);
    let options = if mode == OpenMode::Create {
        options
            .with_source_generation(SourceGeneration::new([9; 32]).unwrap(), 9)
            .unwrap()
    } else {
        options
    };
    SqliteStorage::open(options).await.unwrap()
}

#[tokio::test]
async fn signed_artifact_commit_failure_rolls_back_and_retries_exactly_after_reopen() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp, OpenMode::Create).await;
    let (preparation, plan) = prepare();
    store.execute_authored(preparation).await.unwrap();
    let initial = store.authored_artifact(ids().1).await.unwrap().unwrap();
    let claim = WorkClaim::new(
        [4; 16],
        "sqlite-signer",
        NonZeroU64::MIN,
        11,
        50,
        initial.revision(),
    )
    .unwrap();
    store
        .execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::ArtifactSigning(ids().1),
            claim.clone(),
        )))
        .await
        .unwrap();
    let artifact_before = store.authored_artifact(ids().1).await.unwrap().unwrap();
    let delivery_before = store
        .authored_delivery_plan(ids().2)
        .await
        .unwrap()
        .unwrap();
    let operation_before = store.authored_operation(ids().0).await.unwrap().unwrap();
    assert!(artifact_before.signed().is_none());
    assert!(delivery_before.request().is_none());

    // This storage fixture has a verified event ID. Cryptographic signature
    // verification belongs to signing/Sync, not this transaction boundary.
    let event = signed(&plan);
    let command = AuthoredAtomicCommand::ApplySigned(
        ApplySignedArtifact::new(ids().1, fence(&claim), event.clone(), 12).unwrap(),
    );
    sqlx::query("CREATE TABLE signed_commit_fault (parent BLOB REFERENCES radroots_runtime_authored_operations(operation_id) DEFERRABLE INITIALLY DEFERRED)")
        .execute(store.pool()).await.unwrap();
    // Every statement, including the final receipt INSERT, succeeds. Only
    // SQLite's actual COMMIT rejects the deferred foreign-key violation.
    sqlx::query("CREATE TRIGGER signed_commit_fault_trigger AFTER INSERT ON radroots_runtime_authored_atomic_commits BEGIN INSERT INTO signed_commit_fault VALUES (x'99999999999999999999999999999999'); END")
        .execute(store.pool()).await.unwrap();
    assert!(store.execute_authored(command.clone()).await.is_err());
    assert!(
        store
            .authored_receipt(command.commit_id())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store.authored_artifact(ids().1).await.unwrap().unwrap(),
        artifact_before
    );
    assert_eq!(
        store
            .authored_delivery_plan(ids().2)
            .await
            .unwrap()
            .unwrap(),
        delivery_before
    );
    assert_eq!(
        store.authored_operation(ids().0).await.unwrap().unwrap(),
        operation_before
    );
    let failed_writes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM signed_commit_fault")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(failed_writes, 0);
    sqlx::query("DROP TRIGGER signed_commit_fault_trigger")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("DROP TABLE signed_commit_fault")
        .execute(store.pool())
        .await
        .unwrap();
    store.close().await.unwrap();

    let store = open(&temp, OpenMode::ReadWriteExisting).await;
    assert!(
        store
            .authored_receipt(command.commit_id())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store.authored_artifact(ids().1).await.unwrap().unwrap(),
        artifact_before
    );
    assert_eq!(
        store
            .authored_delivery_plan(ids().2)
            .await
            .unwrap()
            .unwrap(),
        delivery_before
    );
    assert_eq!(
        store.authored_operation(ids().0).await.unwrap().unwrap(),
        operation_before
    );
    let receipt = store.execute_authored(command.clone()).await.unwrap();
    assert_eq!(receipt.disposition(), AtomicCommitDisposition::Committed);
    store.close().await.unwrap();

    let store = open(&temp, OpenMode::ReadWriteExisting).await;
    let artifact = store.authored_artifact(ids().1).await.unwrap().unwrap();
    let delivery = store
        .authored_delivery_plan(ids().2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(artifact.signing_state(), SigningState::Signed);
    assert_eq!(
        artifact.signed().unwrap().event().raw_json(),
        event.raw_json()
    );
    assert_eq!(
        delivery.request().unwrap().payload().event().raw_json(),
        event.raw_json()
    );
    assert_eq!(
        store
            .authored_receipt(command.commit_id())
            .await
            .unwrap()
            .unwrap()
            .outcome(),
        receipt.outcome()
    );
    assert_eq!(
        store.execute_authored(command).await.unwrap().disposition(),
        AtomicCommitDisposition::Replay
    );
    assert_eq!(
        store.authored_artifact(ids().1).await.unwrap().unwrap(),
        artifact
    );
    assert_eq!(
        store
            .authored_delivery_plan(ids().2)
            .await
            .unwrap()
            .unwrap(),
        delivery
    );
    store.close().await.unwrap();
}
