use super::*;
use crate::{OpenMode, OpenOptions, Paths};
use radroots_storage::{
    authored::WorkClaim,
    authored_atomic::{
        ApplySignedArtifact, ApplyWorkFailure, CancelAuthoredWork, ClaimAuthoredWork,
    },
    event::SourceGeneration,
};
use tempfile::TempDir;

use super::signed_fact_fixture as fixture;
use fixture::*;

pub(super) async fn open(temp: &TempDir, mode: OpenMode) -> SqliteStorage {
    let options = OpenOptions::new(Paths::from_directory(temp.path()).unwrap(), mode);
    let options = if matches!(mode, OpenMode::Create) {
        options
            .with_source_generation(SourceGeneration::new([9; 32]).unwrap(), 9)
            .unwrap()
    } else {
        options
    };
    SqliteStorage::open(options).await.unwrap()
}

pub(super) async fn prepared(
    temp: &TempDir,
) -> (SqliteStorage, radroots_event::SignedEvent, WorkClaim) {
    let store = open(temp, OpenMode::Create).await;
    let (preparation, event) = prepare();
    store.execute_authored(preparation).await.unwrap();
    let artifact = store.authored_artifact(ids().1).await.unwrap().unwrap();
    let active = claim(artifact.revision(), 4, 11);
    store
        .execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::ArtifactSigning(ids().1),
            active.clone(),
        )))
        .await
        .unwrap();
    (store, event, active)
}

fn fence(claim: &WorkClaim) -> WorkFence {
    WorkFence::new(*claim.token(), claim.generation(), claim.row_revision()).unwrap()
}

#[tokio::test]
async fn late_stopped_signature_reopens_with_exact_bytes_and_no_scheduling_authority() {
    for cancelled in [false, true] {
        let temp = TempDir::new().unwrap();
        let (store, event, active) = prepared(&temp).await;
        let artifact = store.authored_artifact(ids().1).await.unwrap().unwrap();
        let stop = if cancelled {
            AuthoredAtomicCommand::Cancel(
                CancelAuthoredWork::new(
                    CancelAuthoredTarget::ArtifactSigning(ids().1),
                    artifact.revision(),
                    20,
                )
                .unwrap(),
            )
        } else {
            AuthoredAtomicCommand::ApplyFailure(
                ApplyWorkFailure::new(
                    AuthoredWorkTarget::Artifact(ids().1),
                    fence(&active),
                    WorkFailure::new(
                        "signing_stopped",
                        WorkPhase::Signing,
                        FailureClass::Terminal,
                        None,
                        None,
                    )
                    .unwrap(),
                    None,
                    20,
                )
                .unwrap(),
            )
        };
        let stop_receipt = store.execute_authored(stop).await.unwrap();
        let command = record(event, active, 40);
        let receipt = store.execute_authored(command.clone()).await.unwrap();
        let retained = store.authored_artifact(ids().1).await.unwrap().unwrap();
        assert_eq!(
            retained.signing_state(),
            if cancelled {
                SigningState::Cancelled
            } else {
                SigningState::FailedTerminal
            }
        );
        assert_eq!(retained.signed().unwrap().event().raw_json(), RAW);
        let (physical, stop): (String, Option<String>) = sqlx::query_as(
            "SELECT signing_state, signing_stop FROM radroots_runtime_authored_artifacts",
        )
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(physical, "signed");
        assert_eq!(
            stop.as_deref(),
            Some(if cancelled {
                "cancelled"
            } else {
                "failed_terminal"
            })
        );
        for mutation in [
            "UPDATE radroots_runtime_authored_artifacts SET signed_raw_json = x'7b7d'",
            "UPDATE radroots_runtime_authored_artifacts SET signed_raw_sha256 = zeroblob(32)",
            "UPDATE radroots_runtime_authored_artifacts SET signing_stop = NULL",
        ] {
            assert!(sqlx::query(mutation).execute(store.pool()).await.is_err());
        }
        for target in [
            ClaimAuthoredTarget::ArtifactSigning(ids().1),
            ClaimAuthoredTarget::ArtifactAdmission(ids().1),
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
        ] {
            assert!(
                store
                    .execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
                        target,
                        claim(retained.revision(), 8, 50),
                    )))
                    .await
                    .is_err()
            );
        }
        store.close().await.unwrap();
        let store = open(&temp, OpenMode::ReadWriteExisting).await;
        assert_eq!(
            store.authored_artifact(ids().1).await.unwrap().unwrap(),
            retained
        );
        assert_eq!(
            store
                .authored_receipt(stop_receipt.commit_id())
                .await
                .unwrap()
                .unwrap(),
            stop_receipt
        );
        let replay = store.execute_authored(command).await.unwrap();
        assert_eq!(replay.disposition(), AtomicCommitDisposition::Replay);
        assert_eq!(replay.outcome(), receipt.outcome());
        let delivery = store
            .authored_delivery_plan(ids().2)
            .await
            .unwrap()
            .unwrap();
        assert!(delivery.request().is_none());
        assert!(delivery.attempts().is_empty());
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn actual_commit_failure_rolls_back_late_fact_binding_and_receipt_then_retries() {
    let temp = TempDir::new().unwrap();
    let (store, event, active) = prepared(&temp).await;
    let before = store.authored_artifact(ids().1).await.unwrap().unwrap();
    let command = record(event, active, 40);
    sqlx::query("CREATE TABLE signed_fact_commit_fault (parent BLOB REFERENCES radroots_runtime_authored_operations(operation_id) DEFERRABLE INITIALLY DEFERRED)")
        .execute(store.pool()).await.unwrap();
    sqlx::query("CREATE TRIGGER signed_fact_commit_fault_trigger AFTER INSERT ON radroots_runtime_authored_atomic_commits WHEN NEW.phase = 'signing' BEGIN INSERT INTO signed_fact_commit_fault VALUES (x'99999999999999999999999999999999'); END")
        .execute(store.pool()).await.unwrap();
    assert!(store.execute_authored(command.clone()).await.is_err());
    assert_eq!(
        store.authored_artifact(ids().1).await.unwrap().unwrap(),
        before
    );
    assert!(
        store
            .authored_delivery_plan(ids().2)
            .await
            .unwrap()
            .unwrap()
            .request()
            .is_none()
    );
    assert!(
        store
            .authored_receipt(command.commit_id())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM signed_fact_commit_fault")
            .fetch_one(store.pool())
            .await
            .unwrap(),
        0
    );
    sqlx::query("DROP TRIGGER signed_fact_commit_fault_trigger")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("DROP TABLE signed_fact_commit_fault")
        .execute(store.pool())
        .await
        .unwrap();
    store.close().await.unwrap();
    let store = open(&temp, OpenMode::ReadWriteExisting).await;
    assert_eq!(
        store.authored_artifact(ids().1).await.unwrap().unwrap(),
        before
    );
    let receipt = store.execute_authored(command.clone()).await.unwrap();
    assert_eq!(receipt.disposition(), AtomicCommitDisposition::Committed);
    assert_eq!(
        store
            .authored_delivery_plan(ids().2)
            .await
            .unwrap()
            .unwrap()
            .request()
            .unwrap()
            .payload()
            .event()
            .raw_json(),
        RAW
    );
    store.close().await.unwrap();
    let store = open(&temp, OpenMode::ReadOnly).await;
    assert_eq!(
        store
            .authored_receipt(command.commit_id())
            .await
            .unwrap()
            .unwrap(),
        receipt
    );
    assert!(store.execute_authored(command).await.is_err());
    store.close().await.unwrap();
}

#[tokio::test]
async fn stale_active_fence_and_altered_provenance_cannot_install_late_facts() {
    let temp = TempDir::new().unwrap();
    let (store, event, active) = prepared(&temp).await;
    let before = store.authored_artifact(ids().1).await.unwrap().unwrap();
    assert!(
        store
            .execute_authored(AuthoredAtomicCommand::ApplySigned(
                ApplySignedArtifact::new(ids().1, fence(&active), event.clone(), 40,).unwrap()
            ))
            .await
            .is_err()
    );
    let wrong_owner = WorkClaim::new(
        *active.token(),
        "wrong-owner",
        active.generation(),
        11,
        31,
        active.row_revision(),
    )
    .unwrap();
    for command in [
        record(event.clone(), wrong_owner, 40),
        record(fixture::event(OTHER_RAW), active.clone(), 40),
    ] {
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
            before
        );
    }
    let command = record(event, active.clone(), 40);
    store.execute_authored(command).await.unwrap();
    let retained = store.authored_artifact(ids().1).await.unwrap().unwrap();
    let alternate = record(fixture::event(&format!(" {RAW} ")), active, 50);
    assert_eq!(
        store.execute_authored(alternate.clone()).await,
        Err(Error::AtomicCommitConflict)
    );
    assert!(
        store
            .authored_receipt(alternate.commit_id())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store.authored_artifact(ids().1).await.unwrap().unwrap(),
        retained
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn failure_while_binding_delivery_rolls_back_the_first_signed_fact() {
    let temp = TempDir::new().unwrap();
    let (store, event, active) = prepared(&temp).await;
    let before = store.authored_artifact(ids().1).await.unwrap().unwrap();
    let command = record(event, active, 40);
    sqlx::query("CREATE TRIGGER signed_fact_binding_fault BEFORE UPDATE ON radroots_runtime_authored_delivery_plans BEGIN SELECT RAISE(ABORT, 'fixture delivery binding failure'); END")
        .execute(store.pool()).await.unwrap();
    assert!(store.execute_authored(command.clone()).await.is_err());
    assert_eq!(
        store.authored_artifact(ids().1).await.unwrap().unwrap(),
        before
    );
    assert!(
        store
            .authored_receipt(command.commit_id())
            .await
            .unwrap()
            .is_none()
    );
    sqlx::query("DROP TRIGGER signed_fact_binding_fault")
        .execute(store.pool())
        .await
        .unwrap();
    store.execute_authored(command).await.unwrap();
    store.close().await.unwrap();
}
