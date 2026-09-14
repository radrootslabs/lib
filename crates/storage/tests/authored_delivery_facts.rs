use futures_executor::block_on;
use radroots_storage::{
    Error,
    atomic::{AtomicCommitDigest, AtomicCommitDisposition},
    authored::WorkClaim,
    authored_atomic::{
        ApplyDeliveryAttempt, AuthoredAtomicCommand, AuthoredAtomicOutcome, AuthoredAtomicReceipt,
        AuthoredAtomicStorage, CancelAuthoredTarget, CancelAuthoredWork, ClaimAuthoredTarget,
        ClaimAuthoredWork, RecordDeliveryFact, WorkFence,
    },
    authored_delivery::{
        AuthoredDeliveryPlan, AuthoredDeliveryState, DELIVERY_PLAN_ATTEMPTS_MAX,
        DeliveryAttemptOutcome,
    },
    event::SourceGeneration,
    memory::MemoryStorage,
};
use radroots_transport::{
    DeliveryReceipt, SinkFailure,
    outcome::{DeliveryOutcome, Retryability},
    policy::SatisfactionState,
    sink::DeliveryTargetReceipt,
};
use std::num::NonZeroU64;

#[path = "authored_signing/fixture.rs"]
mod fixture;
use fixture::*;

#[path = "authored_delivery/reconciliation_tests.rs"]
mod reconciliation;

fn plan(storage: &MemoryStorage) -> AuthoredDeliveryPlan {
    block_on(storage.authored_delivery_plan(ids().2))
        .unwrap()
        .unwrap()
}

fn prepared() -> (MemoryStorage, WorkClaim, AuthoredAtomicReceipt) {
    let storage = MemoryStorage::new(SourceGeneration::new([1; 32]).unwrap());
    let (command, event) = prepare();
    block_on(storage.execute_authored(command)).unwrap();
    let signing = claim(NonZeroU64::MIN, 1, 11);
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::ArtifactSigning(ids().1),
            signing.clone(),
        ))),
    )
    .unwrap();
    block_on(storage.execute_authored(record(event, signing, 12))).unwrap();
    let delivery = claim(plan(&storage).revision(), 2, 13);
    let receipt = block_on(storage.execute_authored(AuthoredAtomicCommand::Claim(
        ClaimAuthoredWork::new(ClaimAuthoredTarget::DeliveryPlan(ids().2), delivery.clone()),
    )))
    .unwrap();
    (storage, delivery, receipt)
}

fn outcome(plan: &AuthoredDeliveryPlan, accepted: bool) -> DeliveryAttemptOutcome {
    let request = plan.request().unwrap();
    DeliveryAttemptOutcome::Receipt(
        DeliveryReceipt::for_request(
            request,
            request
                .target_set()
                .targets()
                .iter()
                .cloned()
                .map(|target| {
                    DeliveryTargetReceipt::attempted(
                        target,
                        if accepted {
                            DeliveryOutcome::accepted()
                        } else {
                            DeliveryOutcome::unavailable()
                        },
                    )
                })
                .collect(),
        )
        .unwrap(),
    )
}

fn fact(
    plan: &AuthoredDeliveryPlan,
    claim: WorkClaim,
    accepted: bool,
    at: u64,
) -> AuthoredAtomicCommand {
    AuthoredAtomicCommand::RecordDelivery(
        RecordDeliveryFact::new(
            plan.plan_id(),
            plan.artifact_id(),
            claim,
            outcome(plan, accepted),
            at,
        )
        .unwrap(),
    )
}

fn stop(storage: &MemoryStorage, at: u64) {
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::Cancel(
            CancelAuthoredWork::new(
                CancelAuthoredTarget::DeliveryPlan(ids().2),
                plan(storage).revision(),
                at,
            )
            .unwrap(),
        )),
    )
    .unwrap();
}

#[test]
fn expired_cancelled_delivery_retains_exact_evidence_and_idempotent_first_time() {
    let (storage, active, original) = prepared();
    stop(&storage, 20);
    let before = plan(&storage);
    let command = fact(&before, active.clone(), true, 50);
    let receipt = block_on(storage.execute_authored(command.clone())).unwrap();
    assert!(receipt.matches_command(&command));
    let after = plan(&storage);
    assert_eq!(after.state(), AuthoredDeliveryState::Cancelled);
    assert_eq!(after.stop_requested_at_unix_ms(), Some(20));
    assert_eq!(after.request().unwrap().payload().event().raw_json(), RAW);
    assert_eq!(
        after.delivery_satisfaction().unwrap(),
        SatisfactionState::Satisfied
    );
    assert_eq!(after.revision(), before.revision());
    assert_eq!(after.updated_at_unix_ms(), before.updated_at_unix_ms());
    assert_eq!(after.attempt_count(), 0);
    assert_eq!(after.delivery_facts()[0].claim(), &active);
    assert_eq!(after.delivery_facts()[0].observed_at_unix_ms(), 50);
    let replay_command = fact(&after, active.clone(), true, 90);
    assert_eq!(replay_command.commit_id(), command.commit_id());
    let replay = block_on(storage.execute_authored(replay_command)).unwrap();
    assert_eq!(replay.disposition(), AtomicCommitDisposition::Replay);
    assert_eq!(replay.committed_at_unix_ms(), 50);
    assert_eq!(plan(&storage), after);
    assert_eq!(
        block_on(storage.authored_receipt(original.commit_id()))
            .unwrap()
            .unwrap(),
        original
    );
    stop(&storage, 99);
    assert_eq!(plan(&storage), after);
    assert!(block_on(storage.execute_authored(fact(&after, active.clone(), false, 95))).is_err());
    let fenced = AuthoredAtomicCommand::ApplyDelivery(
        ApplyDeliveryAttempt::new(
            ids().2,
            WorkFence::new(*active.token(), active.generation(), active.row_revision()).unwrap(),
            outcome(&after, true),
            None,
            50,
        )
        .unwrap(),
    );
    assert_eq!(
        block_on(storage.execute_authored(fenced)),
        Err(Error::DeliveryPlanClaimConflict)
    );
    assert!(
        block_on(
            storage.execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
                ClaimAuthoredTarget::DeliveryPlan(ids().2),
                claim(after.revision(), 9, 100)
            )))
        )
        .is_err()
    );
}

#[test]
fn superseded_result_does_not_steal_new_claim_or_regress_acceptance() {
    let (storage, old, _) = prepared();
    let newer = claim(plan(&storage).revision(), 3, 40);
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
            newer.clone(),
        ))),
    )
    .unwrap();
    let before = plan(&storage);
    block_on(storage.execute_authored(fact(&before, old, true, 30))).unwrap();
    let after = plan(&storage);
    assert_eq!(after.claim_evidence(), Some(&newer));
    assert_eq!(after.revision(), before.revision());
    assert_eq!(after.updated_at_unix_ms(), 40);
    let partial = SinkFailure::for_request(
        after.request().unwrap(),
        "sink_lost",
        Retryability::Retryable,
        Some(65),
        Some("connection lost".into()),
        vec![],
    )
    .unwrap();
    let command = AuthoredAtomicCommand::RecordDelivery(
        RecordDeliveryFact::new(
            ids().2,
            ids().1,
            newer.clone(),
            DeliveryAttemptOutcome::SinkFailure(partial.clone()),
            45,
        )
        .unwrap(),
    );
    block_on(storage.execute_authored(command)).unwrap();
    assert_eq!(
        plan(&storage).delivery_satisfaction().unwrap(),
        SatisfactionState::Satisfied
    );
    assert_eq!(plan(&storage).claim_evidence(), Some(&newer));
    let retry = radroots_storage::authored::RetrySchedule::new(
        std::num::NonZeroU32::MIN,
        65,
        radroots_storage::authored::WorkFailure::new(
            "sink_lost",
            radroots_storage::authored::WorkPhase::Delivery,
            radroots_storage::authored::FailureClass::Retryable,
            Some(65),
            Some("connection lost".into()),
        )
        .unwrap(),
    )
    .unwrap();
    let fenced = AuthoredAtomicCommand::ApplyDelivery(
        ApplyDeliveryAttempt::new(
            ids().2,
            WorkFence::new(*newer.token(), newer.generation(), newer.row_revision()).unwrap(),
            DeliveryAttemptOutcome::SinkFailure(partial),
            Some(retry.clone()),
            46,
        )
        .unwrap(),
    );
    block_on(storage.execute_authored(fenced)).unwrap();
    let transitioned = plan(&storage);
    assert_eq!(transitioned.state(), AuthoredDeliveryState::Retryable);
    assert_eq!(transitioned.retry(), Some(&retry));
    assert_eq!(transitioned.attempt_count(), 1);
    assert_eq!(
        transitioned.delivery_satisfaction().unwrap(),
        SatisfactionState::Satisfied
    );
    stop(&storage, 47);
    assert_eq!(
        plan(&storage).delivery_satisfaction().unwrap(),
        SatisfactionState::Satisfied
    );
}

#[test]
fn backend_provenance_rejects_every_forged_complete_claim_and_wrong_artifact() {
    let (storage, original, _) = prepared();
    let before = plan(&storage);
    for changed in 0..6 {
        let forged = WorkClaim::new(
            if changed == 0 {
                [9; 16]
            } else {
                *original.token()
            },
            if changed == 1 {
                "forged"
            } else {
                original.owner()
            },
            if changed == 2 {
                NonZeroU64::new(99).unwrap()
            } else {
                original.generation()
            },
            if changed == 3 {
                14
            } else {
                original.acquired_at_unix_ms()
            },
            if changed == 4 {
                90
            } else {
                original.expires_at_unix_ms()
            },
            if changed == 5 {
                NonZeroU64::new(99).unwrap()
            } else {
                original.row_revision()
            },
        )
        .unwrap();
        let command = fact(&before, forged, true, 100);
        assert_eq!(
            block_on(storage.execute_authored(command.clone())),
            Err(Error::AtomicWorkflowMismatch)
        );
        assert!(
            block_on(storage.authored_receipt(command.commit_id()))
                .unwrap()
                .is_none()
        );
        assert_eq!(plan(&storage), before);
    }
    let wrong = AuthoredAtomicCommand::RecordDelivery(
        RecordDeliveryFact::new(
            ids().2,
            radroots_storage::authored::AuthoredArtifactId::new([99; 16]).unwrap(),
            original.clone(),
            outcome(&before, true),
            50,
        )
        .unwrap(),
    );
    assert_eq!(
        block_on(storage.execute_authored(wrong)),
        Err(Error::AtomicWorkflowMismatch)
    );
    assert!(
        RecordDeliveryFact::new(ids().2, ids().1, original, outcome(&before, true), 12).is_err()
    );
}

#[test]
fn late_fact_rejects_rebound_raw_request_and_caller_receipt_with_wrong_time() {
    let (storage, active, original) = prepared();
    let before = plan(&storage);
    let value =
        RecordDeliveryFact::new(ids().2, ids().1, active.clone(), outcome(&before, true), 50)
            .unwrap();
    let request = before
        .intent()
        .materialize(radroots_transport::sink::DeliveryPayload::new(event(
            OTHER_RAW,
        )))
        .unwrap();
    let mut wire = serde_json::to_value(&before).unwrap();
    wire["request"] = serde_json::to_value(request).unwrap();
    let mut rebound: AuthoredDeliveryPlan = serde_json::from_value(wire).unwrap();
    let saved = rebound.clone();
    assert_eq!(
        value.apply_to(&mut rebound, &original),
        Err(Error::AtomicWorkflowMismatch)
    );
    assert_eq!(rebound, saved);
    let wrong = AuthoredAtomicReceipt::from_durable_parts(
        original.commit_id(),
        original.digest(),
        AtomicCommitDisposition::Committed,
        14,
        original.outcome().clone(),
    )
    .unwrap();
    let mut unchanged = before.clone();
    assert_eq!(
        value.apply_to(&mut unchanged, &wrong),
        Err(Error::AtomicWorkflowMismatch)
    );
    assert_eq!(unchanged, before);
}

#[test]
fn invalid_result_binding_rolls_back_and_changed_failure_details_have_distinct_ids() {
    let (storage, active, _) = prepared();
    let before = plan(&storage);
    let request = before.request().unwrap();
    let wrong = radroots_transport::DeliveryRequest::new(
        "wrong-request",
        request.payload().clone(),
        request.target_set().clone(),
        request.satisfaction().clone(),
        request.deadline_unix_ms(),
    )
    .unwrap();
    let wrong = DeliveryReceipt::for_request(
        &wrong,
        wrong
            .target_set()
            .targets()
            .iter()
            .cloned()
            .map(|target| DeliveryTargetReceipt::attempted(target, DeliveryOutcome::accepted()))
            .collect(),
    )
    .unwrap();
    let command = AuthoredAtomicCommand::RecordDelivery(
        RecordDeliveryFact::new(
            ids().2,
            ids().1,
            active.clone(),
            DeliveryAttemptOutcome::Receipt(wrong),
            50,
        )
        .unwrap(),
    );
    assert_eq!(
        block_on(storage.execute_authored(command.clone())),
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    assert_eq!(plan(&storage), before);
    assert!(
        block_on(storage.authored_receipt(command.commit_id()))
            .unwrap()
            .is_none()
    );
    let mut identities = std::collections::BTreeSet::new();
    for retry in [None, Some(65), Some(66)] {
        for message in [
            None,
            Some("connection lost".to_owned()),
            Some("connection closed".to_owned()),
        ] {
            let failure = SinkFailure::for_request(
                request,
                "sink_lost",
                Retryability::Retryable,
                retry,
                message,
                vec![],
            )
            .unwrap();
            let command = AuthoredAtomicCommand::RecordDelivery(
                RecordDeliveryFact::new(
                    ids().2,
                    ids().1,
                    active.clone(),
                    DeliveryAttemptOutcome::SinkFailure(failure),
                    50,
                )
                .unwrap(),
            );
            assert!(identities.insert(*command.commit_id().as_bytes()));
        }
    }
    assert_eq!(identities.len(), 9);
}

#[test]
fn stop_after_satisfaction_and_legacy_cancelled_snapshot_preserve_intent() {
    let (storage, active, _) = prepared();
    let before = plan(&storage);
    let command = AuthoredAtomicCommand::ApplyDelivery(
        ApplyDeliveryAttempt::new(
            ids().2,
            WorkFence::new(*active.token(), active.generation(), active.row_revision()).unwrap(),
            outcome(&before, true),
            None,
            14,
        )
        .unwrap(),
    );
    block_on(storage.execute_authored(command)).unwrap();
    stop(&storage, 15);
    let after = plan(&storage);
    assert_eq!(after.state(), AuthoredDeliveryState::Satisfied);
    assert_eq!(after.stop_requested_at_unix_ms(), Some(15));
    assert_eq!(
        after.delivery_satisfaction().unwrap(),
        SatisfactionState::Satisfied
    );
    let (storage, _, _) = prepared();
    stop(&storage, 20);
    let expected = plan(&storage);
    let mut legacy = serde_json::to_value(&expected).unwrap();
    legacy.as_object_mut().unwrap().remove("delivery_facts");
    legacy
        .as_object_mut()
        .unwrap()
        .remove("stop_requested_at_unix_ms");
    assert_eq!(
        serde_json::from_value::<AuthoredDeliveryPlan>(legacy).unwrap(),
        expected
    );
    let mut pending = before.clone();
    assert!(pending.request_stop(9).is_err());
    assert_eq!(pending, before);
    assert!(pending.request_stop(12).is_err());
    assert_eq!(pending, before);
}

#[test]
fn receipt_binding_rejects_wrong_outcome_digest_and_observation_time() {
    let (storage, active, _) = prepared();
    let before = plan(&storage);
    let command = fact(&before, active, true, 50);
    assert!(
        AuthoredAtomicReceipt::new(
            &command,
            AtomicCommitDisposition::Committed,
            50,
            AuthoredAtomicOutcome::DeliveryPlan(before.clone())
        )
        .is_err()
    );
    let receipt = block_on(storage.execute_authored(command.clone())).unwrap();
    assert!(
        AuthoredAtomicReceipt::new(
            &command,
            AtomicCommitDisposition::Committed,
            49,
            receipt.outcome().clone()
        )
        .is_err()
    );
    let wrong = AuthoredAtomicReceipt::from_durable_parts(
        receipt.commit_id(),
        AtomicCommitDigest::new([99; 32]),
        AtomicCommitDisposition::Committed,
        50,
        receipt.outcome().clone(),
    )
    .unwrap();
    assert!(!wrong.matches_command(&command));
    let wrong = AuthoredAtomicReceipt::from_durable_parts(
        receipt.commit_id(),
        receipt.digest(),
        AtomicCommitDisposition::Committed,
        50,
        AuthoredAtomicOutcome::Artifact(
            block_on(storage.authored_artifact(ids().1))
                .unwrap()
                .unwrap(),
        ),
    )
    .unwrap();
    assert!(!wrong.matches_command(&command));
}

#[test]
fn fact_capacity_and_structural_corruption_fail_without_evicting_evidence() {
    let (storage, active, original) = prepared();
    let before = plan(&storage);
    let command = fact(&before, active.clone(), true, 50);
    block_on(storage.execute_authored(command)).unwrap();
    let after = plan(&storage);
    let base = serde_json::to_value(&after).unwrap();
    for (path, value) in [
        ("observed_at_unix_ms", serde_json::json!(1)),
        ("claim", serde_json::json!(null)),
        ("outcome", serde_json::json!(null)),
    ] {
        let mut corrupt = base.clone();
        corrupt["delivery_facts"][0][path] = value;
        assert!(serde_json::from_value::<AuthoredDeliveryPlan>(corrupt).is_err());
    }
    let mut duplicate = base.clone();
    duplicate["delivery_facts"]
        .as_array_mut()
        .unwrap()
        .push(base["delivery_facts"][0].clone());
    assert!(serde_json::from_value::<AuthoredDeliveryPlan>(duplicate).is_err());
    let mut full = base.clone();
    full["revision"] = serde_json::json!(2048);
    full["claim"] = serde_json::Value::Null;
    let entries = (1..=DELIVERY_PLAN_ATTEMPTS_MAX)
        .map(|index| {
            let mut entry = base["delivery_facts"][0].clone();
            let claim = WorkClaim::new(
                [7; 16],
                "capacity",
                NonZeroU64::new(u64::from(index)).unwrap(),
                13,
                33,
                NonZeroU64::new(u64::from(index)).unwrap(),
            )
            .unwrap();
            entry["claim"] = serde_json::to_value(claim).unwrap();
            entry
        })
        .collect::<Vec<_>>();
    full["delivery_facts"] = serde_json::to_value(entries).unwrap();
    let mut full_plan: AuthoredDeliveryPlan = serde_json::from_value(full.clone()).unwrap();
    let checkpoint = full_plan.clone();
    let value =
        RecordDeliveryFact::new(ids().2, ids().1, active, outcome(&before, true), 50).unwrap();
    assert_eq!(
        value.apply_to(&mut full_plan, &original),
        Err(Error::DeliveryAttemptOverflow)
    );
    assert_eq!(full_plan, checkpoint);
    assert!(
        full_plan
            .claim(claim(full_plan.revision(), 9, 60), 60)
            .is_err()
    );
    let mut replay = full.clone();
    replay["delivery_facts"][0] = base["delivery_facts"][0].clone();
    let mut replay_plan: AuthoredDeliveryPlan = serde_json::from_value(replay).unwrap();
    let checkpoint = replay_plan.clone();
    value.apply_to(&mut replay_plan, &original).unwrap();
    assert_eq!(replay_plan, checkpoint);
    full["delivery_facts"]
        .as_array_mut()
        .unwrap()
        .push(base["delivery_facts"][0].clone());
    assert!(serde_json::from_value::<AuthoredDeliveryPlan>(full).is_err());
}

#[test]
fn corrupt_original_claim_receipts_and_rebound_current_rows_are_rejected() {
    let (storage, active, original) = prepared();
    let before = plan(&storage);
    let value =
        RecordDeliveryFact::new(ids().2, ids().1, active, outcome(&before, true), 50).unwrap();
    let AuthoredAtomicOutcome::DeliveryPlan(original_plan) = original.outcome() else {
        unreachable!()
    };
    for updates in [
        vec![("plan_id", serde_json::json!(vec![9_u8; 16]))],
        vec![("artifact_id", serde_json::json!(vec![9_u8; 16]))],
        vec![("created_at_unix_ms", serde_json::json!(9))],
        vec![("request", serde_json::Value::Null)],
        vec![("claim", serde_json::Value::Null)],
    ] {
        let mut wire = serde_json::to_value(original_plan).unwrap();
        for (key, replacement) in updates {
            wire[key] = replacement;
        }
        let altered: AuthoredDeliveryPlan = serde_json::from_value(wire).unwrap();
        let forged = AuthoredAtomicReceipt::from_durable_parts(
            original.commit_id(),
            original.digest(),
            AtomicCommitDisposition::Committed,
            original.committed_at_unix_ms(),
            AuthoredAtomicOutcome::DeliveryPlan(altered),
        )
        .unwrap();
        let mut current = before.clone();
        assert_eq!(
            value.apply_to(&mut current, &forged),
            Err(Error::AtomicWorkflowMismatch)
        );
        assert_eq!(current, before);
    }
    for updates in [
        vec![("plan_id", serde_json::json!(vec![9_u8; 16]))],
        vec![("artifact_id", serde_json::json!(vec![9_u8; 16]))],
        vec![("created_at_unix_ms", serde_json::json!(9))],
        vec![
            ("claim", serde_json::Value::Null),
            ("revision", serde_json::json!(2)),
        ],
        vec![
            ("claim", serde_json::Value::Null),
            ("updated_at_unix_ms", serde_json::json!(12)),
        ],
    ] {
        let mut wire = serde_json::to_value(&before).unwrap();
        for (key, replacement) in updates {
            wire[key] = replacement;
        }
        let mut current: AuthoredDeliveryPlan = serde_json::from_value(wire).unwrap();
        let unchanged = current.clone();
        assert_eq!(
            value.apply_to(&mut current, &original),
            Err(Error::AtomicWorkflowMismatch)
        );
        assert_eq!(current, unchanged);
    }
    let wrong_kind = AuthoredAtomicReceipt::from_durable_parts(
        original.commit_id(),
        original.digest(),
        AtomicCommitDisposition::Committed,
        original.committed_at_unix_ms(),
        AuthoredAtomicOutcome::Artifact(
            block_on(storage.authored_artifact(ids().1))
                .unwrap()
                .unwrap(),
        ),
    )
    .unwrap();
    let mut current = before.clone();
    assert_eq!(
        value.apply_to(&mut current, &wrong_kind),
        Err(Error::AtomicWorkflowMismatch)
    );
    let wrong_digest = AuthoredAtomicReceipt::from_durable_parts(
        original.commit_id(),
        AtomicCommitDigest::new([99; 32]),
        AtomicCommitDisposition::Committed,
        original.committed_at_unix_ms(),
        original.outcome().clone(),
    )
    .unwrap();
    assert_eq!(
        value.apply_to(&mut current, &wrong_digest),
        Err(Error::AtomicWorkflowMismatch)
    );
    assert_eq!(current, before);
}

#[test]
fn forged_fact_snapshots_cannot_bypass_stop_time_claim_or_request_invariants() {
    let (storage, active, _) = prepared();
    let before = plan(&storage);
    block_on(storage.execute_authored(fact(&before, active, true, 50))).unwrap();
    let current = plan(&storage);
    let base = serde_json::to_value(&current).unwrap();
    for at in [9, 12, 99] {
        let mut wire = base.clone();
        wire["stop_requested_at_unix_ms"] = serde_json::json!(at);
        assert!(serde_json::from_value::<AuthoredDeliveryPlan>(wire).is_err());
    }
    for (at, revision) in [(9, 2), (15, 2), (13, 3)] {
        let forged = WorkClaim::new(
            [8; 16],
            "invalid-fact",
            NonZeroU64::MIN,
            at,
            at + 20,
            NonZeroU64::new(revision).unwrap(),
        )
        .unwrap();
        let mut wire = base.clone();
        wire["delivery_facts"][0]["claim"] = serde_json::to_value(forged).unwrap();
        assert!(serde_json::from_value::<AuthoredDeliveryPlan>(wire).is_err());
    }
    let mut wire = base;
    wire["request"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<AuthoredDeliveryPlan>(wire).is_err());
}

#[test]
fn receipt_matching_checks_each_plan_fact_and_monotonic_row_time() {
    let (storage, active, _) = prepared();
    let before = plan(&storage);
    let command = fact(&before, active, true, 50);
    let receipt = block_on(storage.execute_authored(command.clone())).unwrap();
    let current = plan(&storage);
    let base = serde_json::to_value(&current).unwrap();
    for key in ["plan_id", "artifact_id"] {
        let mut wire = base.clone();
        wire[key] = serde_json::json!(vec![9_u8; 16]);
        let altered: AuthoredDeliveryPlan = serde_json::from_value(wire).unwrap();
        let forged = AuthoredAtomicReceipt::from_durable_parts(
            receipt.commit_id(),
            receipt.digest(),
            AtomicCommitDisposition::Committed,
            50,
            AuthoredAtomicOutcome::DeliveryPlan(altered.clone()),
        )
        .unwrap();
        assert!(!forged.matches_command(&command));
        assert!(
            AuthoredAtomicReceipt::new(
                &command,
                AtomicCommitDisposition::Committed,
                50,
                AuthoredAtomicOutcome::DeliveryPlan(altered)
            )
            .is_err()
        );
    }
    for change_claim in [false, true] {
        let mut wire = base.clone();
        if change_claim {
            let forged = WorkClaim::new(
                [8; 16],
                "different",
                NonZeroU64::MIN,
                13,
                33,
                NonZeroU64::new(2).unwrap(),
            )
            .unwrap();
            wire["delivery_facts"][0]["claim"] = serde_json::to_value(forged).unwrap();
        } else {
            wire["delivery_facts"][0]["outcome"] =
                serde_json::to_value(outcome(&current, false)).unwrap();
        }
        let altered: AuthoredDeliveryPlan = serde_json::from_value(wire).unwrap();
        let forged = AuthoredAtomicReceipt::from_durable_parts(
            receipt.commit_id(),
            receipt.digest(),
            AtomicCommitDisposition::Committed,
            50,
            AuthoredAtomicOutcome::DeliveryPlan(altered),
        )
        .unwrap();
        assert!(!forged.matches_command(&command));
    }
    let mut stopped = current;
    stopped.request_stop(100).unwrap();
    assert!(
        AuthoredAtomicReceipt::new(
            &command,
            AtomicCommitDisposition::Committed,
            60,
            AuthoredAtomicOutcome::DeliveryPlan(stopped)
        )
        .is_err()
    );
}
