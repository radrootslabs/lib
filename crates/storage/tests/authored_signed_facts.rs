use std::num::NonZeroU64;

use futures_executor::block_on;
use radroots_event::SignedEvent;
use radroots_storage::{
    Error,
    atomic::AtomicCommitDisposition,
    authored::{
        AdmissionState, AuthoredArtifact, FailureClass, SigningState, WorkClaim, WorkFailure,
        WorkPhase,
    },
    authored_atomic::{
        ApplySignedArtifact, ApplyWorkFailure, AuthoredAtomicCommand, AuthoredAtomicReceipt,
        AuthoredAtomicStorage, AuthoredWorkTarget, CancelAuthoredTarget, CancelAuthoredWork,
        ClaimAuthoredTarget, ClaimAuthoredWork, RecordSignedArtifact, WorkFence,
    },
    authored_delivery::AuthoredDeliveryState,
    event::SourceGeneration,
    memory::MemoryStorage,
};

#[path = "authored_signing/fixture.rs"]
mod fixture;
use fixture::*;

fn prepared() -> (MemoryStorage, SignedEvent, WorkClaim, AuthoredAtomicReceipt) {
    let storage = MemoryStorage::new(SourceGeneration::new([1; 32]).unwrap());
    let (preparation, event) = prepare();
    block_on(storage.execute_authored(preparation)).unwrap();
    let active = claim(NonZeroU64::new(1).unwrap(), 4, 11);
    let receipt = block_on(storage.execute_authored(AuthoredAtomicCommand::Claim(
        ClaimAuthoredWork::new(
            ClaimAuthoredTarget::ArtifactSigning(ids().1),
            active.clone(),
        ),
    )))
    .unwrap();
    (storage, event, active, receipt)
}

fn artifact(storage: &MemoryStorage) -> AuthoredArtifact {
    block_on(storage.authored_artifact(ids().1))
        .unwrap()
        .unwrap()
}

fn fence(active: &WorkClaim) -> WorkFence {
    WorkFence::new(*active.token(), active.generation(), active.row_revision()).unwrap()
}

#[test]
fn expired_claim_retains_exact_facts_without_relaxing_active_fences() {
    let (storage, event, active, original) = prepared();
    let stale = AuthoredAtomicCommand::ApplySigned(
        ApplySignedArtifact::new(ids().1, fence(&active), event.clone(), 40).unwrap(),
    );
    assert_eq!(
        block_on(storage.execute_authored(stale.clone())),
        Err(Error::DeliveryPlanClaimConflict)
    );
    let fact = record(event.clone(), active.clone(), 40);
    assert_ne!(fact.commit_id(), stale.commit_id());
    let receipt = block_on(storage.execute_authored(fact.clone())).unwrap();
    assert_eq!(receipt.disposition(), AtomicCommitDisposition::Committed);
    let signed = artifact(&storage);
    assert_eq!(signed.signing_state(), SigningState::Signed);
    assert_eq!(signed.signed().unwrap().event().raw_json(), RAW);
    assert!(signed.signing_claim().is_none());
    let delivery = block_on(storage.authored_delivery_plan(ids().2))
        .unwrap()
        .unwrap();
    assert_eq!(delivery.request().unwrap().payload().event(), &event);
    assert!(delivery.attempts().is_empty());
    assert_eq!(
        block_on(storage.authored_receipt(original.commit_id()))
            .unwrap()
            .unwrap(),
        original
    );
    let later = record(event, active, 90);
    assert_eq!(later.commit_id(), fact.commit_id());
    let replay = block_on(storage.execute_authored(later)).unwrap();
    assert_eq!(replay.disposition(), AtomicCommitDisposition::Replay);
    assert_eq!(replay.committed_at_unix_ms(), 40);
    assert_eq!(artifact(&storage), signed);
}

#[test]
fn cancellation_and_terminal_failure_survive_late_signed_bytes() {
    for cancelled in [false, true] {
        let (storage, event, active, _) = prepared();
        let stop = if cancelled {
            AuthoredAtomicCommand::Cancel(
                CancelAuthoredWork::new(
                    CancelAuthoredTarget::ArtifactSigning(ids().1),
                    artifact(&storage).revision(),
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
        let stop_receipt = block_on(storage.execute_authored(stop)).unwrap();
        let stopped = artifact(&storage);
        // An older observation still records the fact without moving row time back.
        let fact_receipt = block_on(storage.execute_authored(record(event, active, 15))).unwrap();
        assert_eq!(fact_receipt.committed_at_unix_ms(), 20);
        let retained = artifact(&storage);
        assert_eq!(retained.signing_state(), stopped.signing_state());
        assert_eq!(retained.last_failure(), stopped.last_failure());
        assert_eq!(retained.updated_at_unix_ms(), 20);
        assert_eq!(retained.admission_state(), AdmissionState::Pending);
        assert_eq!(retained.signed().unwrap().event().raw_json(), RAW);
        let delivery = block_on(storage.authored_delivery_plan(ids().2))
            .unwrap()
            .unwrap();
        assert!(delivery.request().is_none());
        for target in [
            ClaimAuthoredTarget::ArtifactSigning(ids().1),
            ClaimAuthoredTarget::ArtifactAdmission(ids().1),
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
        ] {
            let revision = if matches!(target, ClaimAuthoredTarget::DeliveryPlan(_)) {
                delivery.revision()
            } else {
                retained.revision()
            };
            assert!(
                block_on(storage.execute_authored(AuthoredAtomicCommand::Claim(
                    ClaimAuthoredWork::new(target, claim(revision, 8, 50)),
                )))
                .is_err()
            );
        }
        assert_eq!(artifact(&storage), retained);
        assert_eq!(
            block_on(storage.authored_receipt(stop_receipt.commit_id()))
                .unwrap()
                .unwrap(),
            stop_receipt
        );
        let operation = block_on(storage.authored_operation(ids().0))
            .unwrap()
            .unwrap();
        let settlement = radroots_storage::authored::OperationSettlement::evaluate(
            &operation,
            std::slice::from_ref(&retained),
        )
        .unwrap();
        assert_eq!(settlement.signed(), 1);
        assert_eq!(settlement.cancelled(), u16::from(cancelled));
        assert_eq!(settlement.failed_terminal(), u16::from(!cancelled));
        #[cfg(feature = "serde")]
        assert_eq!(
            serde_json::from_str::<AuthoredArtifact>(&serde_json::to_string(&retained).unwrap())
                .unwrap(),
            retained
        );
    }
}

#[test]
fn superseded_attempts_cannot_overwrite_first_exact_bytes_or_restart_stopped_delivery() {
    let (storage, event, first, _) = prepared();
    let second = claim(artifact(&storage).revision(), 5, 40);
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::ArtifactSigning(ids().1),
            second.clone(),
        ))),
    )
    .unwrap();
    let plan = block_on(storage.authored_delivery_plan(ids().2))
        .unwrap()
        .unwrap();
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::Cancel(
            CancelAuthoredWork::new(
                CancelAuthoredTarget::DeliveryPlan(ids().2),
                plan.revision(),
                41,
            )
            .unwrap(),
        )),
    )
    .unwrap();
    block_on(storage.execute_authored(record(event.clone(), first.clone(), 42))).unwrap();
    let retained = artifact(&storage);
    assert!(retained.signing_claim().is_none());
    block_on(storage.execute_authored(record(event, second.clone(), 43))).unwrap();
    assert_eq!(artifact(&storage), retained);
    let alternate = fixture::event(&format!(" {RAW} "));
    assert_eq!(alternate.id(), retained.signed().unwrap().event().id());
    assert_eq!(
        block_on(storage.execute_authored(record(alternate, second, 44))),
        Err(Error::AtomicCommitConflict)
    );
    assert_eq!(artifact(&storage), retained);
    let plan = block_on(storage.authored_delivery_plan(ids().2))
        .unwrap()
        .unwrap();
    assert_eq!(plan.state(), AuthoredDeliveryState::Cancelled);
    assert!(plan.request().is_none());
    assert!(plan.attempts().is_empty());
}

#[test]
fn unrelated_plan_or_attempt_provenance_rolls_back_without_fact_receipts() {
    let (storage, event, active, original) = prepared();
    let before = artifact(&storage);
    let wrong_claims = [
        WorkClaim::new(
            [9; 16],
            active.owner(),
            active.generation(),
            11,
            31,
            active.row_revision(),
        )
        .unwrap(),
        WorkClaim::new(
            *active.token(),
            "different-worker",
            active.generation(),
            11,
            31,
            active.row_revision(),
        )
        .unwrap(),
        WorkClaim::new(
            *active.token(),
            active.owner(),
            NonZeroU64::new(9).unwrap(),
            11,
            31,
            active.row_revision(),
        )
        .unwrap(),
        WorkClaim::new(
            *active.token(),
            active.owner(),
            active.generation(),
            12,
            31,
            active.row_revision(),
        )
        .unwrap(),
        WorkClaim::new(
            *active.token(),
            active.owner(),
            active.generation(),
            11,
            32,
            active.row_revision(),
        )
        .unwrap(),
        WorkClaim::new(
            *active.token(),
            active.owner(),
            active.generation(),
            11,
            31,
            NonZeroU64::new(9).unwrap(),
        )
        .unwrap(),
    ];
    let expected = record(event.clone(), active.clone(), 50);
    for wrong in wrong_claims {
        let command = record(event.clone(), wrong, 50);
        assert_ne!(command.commit_id(), expected.commit_id());
        assert!(block_on(storage.execute_authored(command.clone())).is_err());
        assert!(
            block_on(storage.authored_receipt(command.commit_id()))
                .unwrap()
                .is_none()
        );
    }
    let wrong_operation = RecordSignedArtifact::new(
        radroots_storage::journal::OperationInstanceId::new([9; 16]).unwrap(),
        ids().1,
        active.clone(),
        event.clone(),
        50,
    )
    .unwrap();
    assert!(
        block_on(storage.execute_authored(AuthoredAtomicCommand::RecordSigned(wrong_operation)))
            .is_err()
    );
    let mismatch = record(fixture::event(OTHER_RAW), active.clone(), 50);
    assert!(block_on(storage.execute_authored(mismatch.clone())).is_err());
    assert!(
        block_on(storage.authored_receipt(mismatch.commit_id()))
            .unwrap()
            .is_none()
    );
    assert_eq!(artifact(&storage), before);
    let AuthoredAtomicCommand::RecordSigned(value) = expected else {
        unreachable!()
    };
    assert_eq!(value.operation_id(), ids().0);
    assert_eq!(value.artifact_id(), ids().1);
    assert_eq!(value.claim(), &active);
    assert_eq!(value.event(), &event);
    assert_eq!(value.observed_at_unix_ms(), 50);
    assert_eq!(value.clone(), value);
    let (preparation, _) = prepare();
    let wrong_outcome = block_on(storage.execute_authored(preparation)).unwrap();
    assert!(value.apply_to(&mut before.clone(), &wrong_outcome).is_err());
    assert!(value.apply_to(&mut before.clone(), &original).is_ok());
    #[cfg(feature = "serde")]
    {
        let mut regressed = serde_json::to_value(&before).unwrap();
        regressed["signing_claim"] = serde_json::Value::Null;
        regressed["updated_at_unix_ms"] = serde_json::Value::from(10);
        let mut regressed = serde_json::from_value::<AuthoredArtifact>(regressed).unwrap();
        assert!(value.apply_to(&mut regressed, &original).is_err());
    }
}

#[test]
fn invalid_signature_and_pre_attempt_observation_never_become_facts() {
    let (_, event, active, _) = prepared();
    for at in [0, 10] {
        assert!(
            RecordSignedArtifact::new(ids().0, ids().1, active.clone(), event.clone(), at).is_err()
        );
    }
    let mut value: serde_json::Value = serde_json::from_str(RAW).unwrap();
    value["sig"] = serde_json::Value::String("ff".repeat(64));
    let hostile = fixture::event(&serde_json::to_string(&value).unwrap());
    assert_eq!(
        RecordSignedArtifact::new(ids().0, ids().1, active, hostile, 50),
        Err(Error::InvalidAuthoredArtifact)
    );
}

#[test]
fn indeterminate_signing_is_resolved_by_original_verified_evidence() {
    let (storage, event, active, _) = prepared();
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::ApplyFailure(
            ApplyWorkFailure::new(
                AuthoredWorkTarget::Artifact(ids().1),
                fence(&active),
                WorkFailure::new(
                    "signing_unknown",
                    WorkPhase::Signing,
                    FailureClass::Indeterminate,
                    None,
                    None,
                )
                .unwrap(),
                None,
                20,
            )
            .unwrap(),
        )),
    )
    .unwrap();
    assert_eq!(
        artifact(&storage).signing_state(),
        SigningState::Indeterminate
    );
    block_on(storage.execute_authored(record(event, active, 40))).unwrap();
    let retained = artifact(&storage);
    assert_eq!(retained.signing_state(), SigningState::Signed);
    assert!(retained.last_failure().is_none());
    assert_eq!(retained.signed().unwrap().event().raw_json(), RAW);
}

#[test]
fn signed_fact_receipts_require_exact_outcome_and_monotonic_commit_time() {
    let (storage, event, active, _) = prepared();
    let command = record(event, active, 40);
    let unsigned = artifact(&storage);
    assert!(
        AuthoredAtomicReceipt::new(
            &command,
            AtomicCommitDisposition::Committed,
            40,
            radroots_storage::authored_atomic::AuthoredAtomicOutcome::Artifact(unsigned),
        )
        .is_err()
    );
    let receipt = block_on(storage.execute_authored(command.clone())).unwrap();
    assert!(receipt.matches_command(&command));
    assert!(
        AuthoredAtomicReceipt::new(
            &command,
            AtomicCommitDisposition::Committed,
            39,
            receipt.outcome().clone(),
        )
        .is_err()
    );
    let wrong = radroots_storage::authored_atomic::AuthoredAtomicOutcome::DeliveryPlan(
        block_on(storage.authored_delivery_plan(ids().2))
            .unwrap()
            .unwrap(),
    );
    assert!(
        AuthoredAtomicReceipt::new(
            &command,
            AtomicCommitDisposition::Committed,
            40,
            wrong.clone()
        )
        .is_err()
    );
    let malformed = AuthoredAtomicReceipt::from_durable_parts(
        command.commit_id(),
        command.digest(),
        AtomicCommitDisposition::Committed,
        40,
        wrong,
    )
    .unwrap();
    assert!(!malformed.matches_command(&command));
}

#[cfg(feature = "serde")]
#[test]
fn stopped_signed_snapshots_cannot_erase_the_required_terminal_failure() {
    let (storage, event, active, _) = prepared();
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::ApplyFailure(
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
        )),
    )
    .unwrap();
    block_on(storage.execute_authored(record(event, active, 40))).unwrap();
    let retained = artifact(&storage);
    let mut forged = serde_json::to_value(&retained).unwrap();
    forged["last_failure"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<AuthoredArtifact>(forged).is_err());
}
