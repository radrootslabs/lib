use super::*;
use radroots_storage::{
    authored::{FailureClass, RetrySchedule, WorkFailure, WorkPhase},
    authored_atomic::ReconcileDeliveryFacts,
    authored_delivery::AuthoredDeliveryHistory,
};
use std::num::NonZeroU32;

fn history(storage: &MemoryStorage) -> AuthoredDeliveryHistory {
    block_on(storage.authored_delivery_history(ids().2))
        .unwrap()
        .unwrap()
}

fn fence(claim: &WorkClaim) -> WorkFence {
    WorkFence::new(*claim.token(), claim.generation(), claim.row_revision()).unwrap()
}

fn reconcile(
    plan: &AuthoredDeliveryPlan,
    authority: Option<WorkFence>,
    retry: Option<RetrySchedule>,
    at: u64,
) -> AuthoredAtomicCommand {
    AuthoredAtomicCommand::ReconcileDelivery(
        ReconcileDeliveryFacts::new(plan, authority, retry, at).unwrap(),
    )
}

fn retry(attempt: u32, at: u64) -> RetrySchedule {
    RetrySchedule::new(
        NonZeroU32::new(attempt).unwrap(),
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
}

#[test]
fn history_distinguishes_no_issued_work_from_stopped_unresolved_work() {
    let untouched = MemoryStorage::new(SourceGeneration::new([71; 32]).unwrap());
    assert!(
        block_on(untouched.authored_delivery_history(ids().2))
            .unwrap()
            .is_none()
    );
    block_on(untouched.execute_authored(prepare().0)).unwrap();
    let before = history(&untouched);
    assert!(before.is_complete());
    assert!(before.proves_no_issued_attempt());
    assert!(!before.has_unresolved_claims());
    stop(&untouched, 20);
    assert!(history(&untouched).proves_no_issued_attempt());

    let (storage, active, _) = prepared();
    let issued = history(&storage);
    assert_eq!(issued.claims().len(), 1);
    assert_eq!(issued.claims()[0].claim(), &active);
    assert_eq!(issued.claims()[0].prior_attempt_count(), 0);
    assert!(issued.has_unresolved_claims());
    assert!(!issued.proves_no_issued_attempt());
    stop(&storage, 20);
    assert!(history(&storage).has_unresolved_claims());
    block_on(storage.execute_authored(fact(&plan(&storage), active, true, 50))).unwrap();
    let observed = history(&storage);
    assert!(!observed.has_unresolved_claims());
    assert!(!observed.proves_no_issued_attempt());
    assert_eq!(observed.plan().state(), AuthoredDeliveryState::Cancelled);
    assert_eq!(
        observed.plan().delivery_satisfaction().unwrap(),
        SatisfactionState::Satisfied
    );
}

#[test]
fn exact_current_fence_reconciles_once_and_preserves_raw_fact_provenance() {
    let (storage, active, original) = prepared();
    block_on(storage.execute_authored(fact(&plan(&storage), active.clone(), true, 14))).unwrap();
    let before = plan(&storage);
    let command = reconcile(&before, Some(fence(&active)), None, 15);
    let receipt = block_on(storage.execute_authored(command.clone())).unwrap();
    let after = plan(&storage);
    assert!(receipt.matches_command(&command));
    assert_eq!(after.state(), AuthoredDeliveryState::Satisfied);
    assert_eq!(after.attempt_count(), 1);
    assert_eq!(after.attempts()[0].claim_evidence(), Some(&active));
    assert_eq!(
        after.attempts()[0].outcome(),
        before.delivery_facts()[0].outcome()
    );
    assert_eq!(after.attempts()[0].recorded_at_unix_ms(), 15);
    assert_eq!(after.delivery_facts(), before.delivery_facts());
    assert_eq!(after.pending_delivery_facts().count(), 0);
    assert!(!history(&storage).has_unresolved_claims());
    let replay = block_on(storage.execute_authored(command)).unwrap();
    assert_eq!(replay.disposition(), AtomicCommitDisposition::Replay);
    assert_eq!(plan(&storage), after);
    assert_eq!(
        block_on(storage.authored_receipt(original.commit_id()))
            .unwrap()
            .unwrap(),
        original
    );
    assert_eq!(
        ReconcileDeliveryFacts::new(&after, None, None, 16),
        Err(Error::AtomicWorkflowMismatch)
    );
}

#[test]
fn distinct_current_reconciliation_survives_legacy_generation_identity_reuse() {
    let (storage, first, _) = prepared();
    let first_plan = plan(&storage);
    block_on(storage.execute_authored(fact(&first_plan, first.clone(), false, 14))).unwrap();
    let first_command = reconcile(&plan(&storage), Some(fence(&first)), Some(retry(1, 18)), 15);
    block_on(storage.execute_authored(first_command.clone())).unwrap();
    let second = claim(plan(&storage).revision(), 2, 20);
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
            second.clone(),
        ))),
    )
    .unwrap();
    block_on(storage.execute_authored(fact(&plan(&storage), second.clone(), false, 21))).unwrap();
    let second_command = reconcile(
        &plan(&storage),
        Some(fence(&second)),
        Some(retry(2, 25)),
        22,
    );
    assert_ne!(first_command.commit_id(), second_command.commit_id());
    let legacy = |claim: &WorkClaim, at| {
        AuthoredAtomicCommand::ApplyDelivery(
            ApplyDeliveryAttempt::new(ids().2, fence(claim), outcome(&first_plan, false), None, at)
                .unwrap(),
        )
    };
    assert_eq!(
        legacy(&first, 15).commit_id(),
        legacy(&second, 22).commit_id()
    );
    block_on(storage.execute_authored(second_command)).unwrap();
    let after = plan(&storage);
    assert_eq!(after.state(), AuthoredDeliveryState::Retryable);
    assert_eq!(after.attempt_count(), 2);
    assert_eq!(after.retry().unwrap().not_before_unix_ms(), 25);
    assert_eq!(after.attempts()[0].claim_evidence(), Some(&first));
    assert_eq!(after.attempts()[1].claim_evidence(), Some(&second));
    assert!(!history(&storage).has_unresolved_claims());
}

#[test]
fn late_fact_cannot_take_a_newer_lease_and_its_marker_cannot_resolve_that_lease() {
    let (storage, old, _) = prepared();
    let newer = claim(plan(&storage).revision(), 3, 40);
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
            newer.clone(),
        ))),
    )
    .unwrap();
    block_on(storage.execute_authored(fact(&plan(&storage), old.clone(), true, 50))).unwrap();
    let before = plan(&storage);
    for authority in [None, Some(fence(&old))] {
        let command = reconcile(&before, authority, None, 51);
        assert_eq!(
            block_on(storage.execute_authored(command)),
            Err(Error::DeliveryPlanClaimConflict)
        );
        assert_eq!(plan(&storage), before);
    }
    block_on(storage.execute_authored(reconcile(&before, Some(fence(&newer)), None, 51))).unwrap();
    let after = history(&storage);
    assert_eq!(after.plan().attempts()[0].claim_evidence(), Some(&old));
    assert!(
        after.has_unresolved_claims(),
        "the new worker has no final result yet"
    );
    assert_eq!(
        after.plan().delivery_satisfaction().unwrap(),
        SatisfactionState::Satisfied
    );
}

#[test]
fn fresh_observer_can_reconcile_after_expiry_but_expired_worker_cannot() {
    let (storage, old, _) = prepared();
    block_on(storage.execute_authored(fact(&plan(&storage), old.clone(), true, 50))).unwrap();
    let before = plan(&storage);
    assert_eq!(
        block_on(storage.execute_authored(reconcile(&before, Some(fence(&old)), None, 51))),
        Err(Error::DeliveryPlanClaimConflict)
    );
    block_on(storage.execute_authored(reconcile(&before, None, None, 51))).unwrap();
    assert_eq!(plan(&storage).state(), AuthoredDeliveryState::Satisfied);
    assert_eq!(plan(&storage).attempts()[0].claim_evidence(), Some(&old));
}

#[test]
fn exact_fact_set_and_revision_races_fail_without_partial_scheduling() {
    let (storage, old, _) = prepared();
    let newer = claim(plan(&storage).revision(), 3, 40);
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
            newer.clone(),
        ))),
    )
    .unwrap();
    block_on(storage.execute_authored(fact(&plan(&storage), old.clone(), true, 50))).unwrap();
    let before = plan(&storage);
    let stale = reconcile(&before, Some(fence(&newer)), None, 55);
    block_on(storage.execute_authored(fact(&before, newer.clone(), false, 51))).unwrap();
    let current = plan(&storage);
    assert_eq!(current.revision(), before.revision());
    assert_eq!(
        block_on(storage.execute_authored(stale)),
        Err(Error::DeliveryPlanClaimConflict)
    );
    assert_eq!(plan(&storage), current);
    block_on(storage.execute_authored(reconcile(&current, Some(fence(&newer)), None, 55))).unwrap();
    let settled = plan(&storage);
    assert_eq!(settled.attempt_count(), 2);
    assert_eq!(settled.state(), AuthoredDeliveryState::Satisfied);
    assert_eq!(settled.attempts()[0].claim_evidence(), Some(&old));
    assert_eq!(settled.attempts()[1].claim_evidence(), Some(&newer));
    assert!(
        settled
            .attempts()
            .iter()
            .all(|attempt| attempt.recorded_at_unix_ms() == 55)
    );

    let (stopped, active, _) = prepared();
    block_on(stopped.execute_authored(fact(&plan(&stopped), active, true, 50))).unwrap();
    let stale = reconcile(&plan(&stopped), None, None, 51);
    stop(&stopped, 52);
    let before = plan(&stopped);
    assert_eq!(
        block_on(stopped.execute_authored(stale)),
        Err(Error::DeliveryPlanClaimConflict)
    );
    assert_eq!(
        block_on(stopped.execute_authored(reconcile(&before, None, None, 53))),
        Err(Error::InvalidAuthoredTransition)
    );
    assert_eq!(plan(&stopped), before);
}

#[test]
fn historical_application_is_resolved_without_rewriting_its_snapshot_shape() {
    let (storage, active, _) = prepared();
    let command = AuthoredAtomicCommand::ApplyDelivery(
        ApplyDeliveryAttempt::new(
            ids().2,
            fence(&active),
            outcome(&plan(&storage), true),
            None,
            14,
        )
        .unwrap(),
    );
    block_on(storage.execute_authored(command)).unwrap();
    let legacy = history(&storage);
    assert!(!legacy.has_unresolved_claims());
    let attempt = &legacy.plan().attempts()[0];
    assert!(attempt.claim_evidence().is_none());
    assert!(
        !serde_json::to_value(attempt)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("claim")
    );
    let restored: AuthoredDeliveryPlan =
        serde_json::from_slice(&serde_json::to_vec(legacy.plan()).unwrap()).unwrap();
    assert_eq!(restored, *legacy.plan());
}

#[test]
fn missing_truncated_or_malformed_history_never_proves_absence() {
    let storage = MemoryStorage::new(SourceGeneration::new([72; 32]).unwrap());
    let original = block_on(storage.execute_authored(prepare().0)).unwrap();
    let initial = plan(&storage);
    let unknown = AuthoredDeliveryHistory::new(initial.clone(), None).unwrap();
    assert!(!unknown.is_complete());
    assert!(!unknown.proves_no_issued_attempt());
    assert!(unknown.has_unresolved_claims());
    let mut bounded = AuthoredDeliveryHistory::new(initial, Some(&original)).unwrap();
    bounded.mark_truncated();
    assert!(bounded.is_truncated());
    assert!(!bounded.proves_no_issued_attempt());
    assert!(bounded.has_unresolved_claims());
    assert_eq!(
        bounded.require_pending_fact_provenance(),
        Err(Error::DeliveryAttemptOverflow)
    );
    assert_eq!(
        bounded.push_claim(&original),
        Err(Error::AtomicWorkflowMismatch)
    );

    let (storage, _, issued) = prepared();
    let mut duplicate = history(&storage);
    assert_eq!(
        duplicate.push_claim(&issued),
        Err(Error::AtomicWorkflowMismatch)
    );
    assert!(AuthoredDeliveryHistory::new(plan(&storage), Some(&issued)).is_err());
    let incomplete = AuthoredDeliveryHistory::new(plan(&storage), Some(&original)).unwrap();
    assert_eq!(incomplete.validate(), Err(Error::AtomicWorkflowMismatch));
}

#[test]
fn forged_or_duplicate_reconciliation_markers_fail_snapshot_validation() {
    let (storage, active, _) = prepared();
    block_on(storage.execute_authored(fact(&plan(&storage), active.clone(), true, 14))).unwrap();
    block_on(storage.execute_authored(reconcile(&plan(&storage), Some(fence(&active)), None, 15)))
        .unwrap();
    let valid = serde_json::to_value(plan(&storage)).unwrap();
    let mut missing_fact = valid.clone();
    missing_fact["delivery_facts"] = serde_json::json!([]);
    assert!(serde_json::from_value::<AuthoredDeliveryPlan>(missing_fact).is_err());
    let mut forged = valid.clone();
    forged["attempts"][0]["claim"] = serde_json::to_value(claim(NonZeroU64::MIN, 9, 13)).unwrap();
    assert!(serde_json::from_value::<AuthoredDeliveryPlan>(forged).is_err());
    let mut duplicate = valid;
    let mut second = duplicate["attempts"][0].clone();
    second["attempt"] = serde_json::json!(2);
    duplicate["attempts"].as_array_mut().unwrap().push(second);
    duplicate["attempt_count"] = serde_json::json!(2);
    assert!(serde_json::from_value::<AuthoredDeliveryPlan>(duplicate).is_err());
}

#[test]
fn issued_claim_limit_rejects_new_work_atomically_and_keeps_original_replay() {
    let (storage, _, first) = prepared();
    for index in 1..DELIVERY_PLAN_ATTEMPTS_MAX {
        let at = 40 + u64::from(index) * 21;
        let active = WorkClaim::new(
            [7; 16],
            "bounded-worker",
            NonZeroU64::new(u64::from(index) + 2).unwrap(),
            at,
            at + 20,
            plan(&storage).revision(),
        )
        .unwrap();
        block_on(
            storage.execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
                ClaimAuthoredTarget::DeliveryPlan(ids().2),
                active,
            ))),
        )
        .unwrap();
    }
    let before = plan(&storage);
    let at = 40 + u64::from(DELIVERY_PLAN_ATTEMPTS_MAX) * 21;
    let active = WorkClaim::new(
        [8; 16],
        "overflow-worker",
        NonZeroU64::new(2048).unwrap(),
        at,
        at + 20,
        before.revision(),
    )
    .unwrap();
    let command = AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
        ClaimAuthoredTarget::DeliveryPlan(ids().2),
        active,
    ));
    assert_eq!(
        block_on(storage.execute_authored(command.clone())),
        Err(Error::DeliveryAttemptOverflow)
    );
    assert_eq!(plan(&storage), before);
    assert!(
        block_on(storage.authored_receipt(command.commit_id()))
            .unwrap()
            .is_none()
    );
    let history = history(&storage);
    assert_eq!(history.claims().len(), DELIVERY_PLAN_ATTEMPTS_MAX as usize);
    assert!(history.is_complete());
    assert!(history.has_unresolved_claims());
    assert_eq!(
        history.clone().push_claim(&first),
        Err(Error::DeliveryAttemptOverflow)
    );
    let AuthoredAtomicOutcome::DeliveryPlan(original) = first.outcome() else {
        unreachable!()
    };
    let original = AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
        ClaimAuthoredTarget::DeliveryPlan(ids().2),
        original.claim_evidence().unwrap().clone(),
    ));
    assert_eq!(
        block_on(storage.execute_authored(original))
            .unwrap()
            .disposition(),
        AtomicCommitDisposition::Replay
    );
    assert_eq!(plan(&storage), before);
}

#[test]
fn reconciliation_rejects_future_facts_zero_time_and_invalid_retry_without_mutation() {
    for accepted in [false, true] {
        let (storage, active, _) = prepared();
        block_on(storage.execute_authored(fact(&plan(&storage), active.clone(), accepted, 30)))
            .unwrap();
        let before = plan(&storage);
        assert_eq!(
            ReconcileDeliveryFacts::new(&before, None, None, 0),
            Err(Error::AtomicWorkflowMismatch)
        );
        assert_eq!(
            block_on(storage.execute_authored(reconcile(&before, Some(fence(&active)), None, 29))),
            Err(Error::AtomicWorkflowMismatch)
        );
        let invalid = if accepted { Some(retry(1, 60)) } else { None };
        assert_eq!(
            block_on(storage.execute_authored(reconcile(&before, None, invalid, 50))),
            Err(Error::InvalidRetrySchedule)
        );
        if !accepted {
            assert_eq!(
                block_on(storage.execute_authored(reconcile(
                    &before,
                    None,
                    Some(retry(2, 60)),
                    50
                ))),
                Err(Error::InvalidRetrySchedule)
            );
        }
        assert_eq!(plan(&storage), before);
    }
}

#[test]
fn raw_sink_failure_controls_retry_diagnostic_and_provider_backoff() {
    let (storage, active, _) = prepared();
    let failure = SinkFailure::for_request(
        plan(&storage).request().unwrap(),
        "sink_lost",
        Retryability::Retryable,
        Some(65),
        Some("connection lost".into()),
        vec![],
    )
    .unwrap();
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::RecordDelivery(
            RecordDeliveryFact::new(
                ids().2,
                ids().1,
                active,
                DeliveryAttemptOutcome::SinkFailure(failure.clone()),
                50,
            )
            .unwrap(),
        )),
    )
    .unwrap();
    let before = plan(&storage);
    for (code, diagnostic, at) in [
        ("other", Some("connection lost"), 65),
        ("sink_lost", None, 65),
        ("sink_lost", Some("connection lost"), 64),
    ] {
        let retry = RetrySchedule::new(
            NonZeroU32::MIN,
            at,
            WorkFailure::new(
                code,
                WorkPhase::Delivery,
                FailureClass::Retryable,
                None,
                diagnostic.map(str::to_owned),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            block_on(storage.execute_authored(reconcile(&before, None, Some(retry), 51))),
            Err(Error::InvalidRetrySchedule)
        );
        assert_eq!(plan(&storage), before);
    }
    let retry = RetrySchedule::new(
        NonZeroU32::MIN,
        65,
        WorkFailure::new(
            "sink_lost",
            WorkPhase::Delivery,
            FailureClass::Retryable,
            Some(65),
            Some("connection lost".into()),
        )
        .unwrap(),
    )
    .unwrap();
    block_on(storage.execute_authored(reconcile(&before, None, Some(retry.clone()), 51))).unwrap();
    let after = plan(&storage);
    assert_eq!(after.state(), AuthoredDeliveryState::Retryable);
    assert_eq!(after.retry(), Some(&retry));
    assert_eq!(
        after.attempts()[0].outcome(),
        &DeliveryAttemptOutcome::SinkFailure(failure)
    );
}

#[test]
fn matching_late_fact_marks_legacy_attempt_without_inventing_another_attempt() {
    for accepted in [false, true] {
        let (storage, active, _) = prepared();
        let command = AuthoredAtomicCommand::ApplyDelivery(
            ApplyDeliveryAttempt::new(
                ids().2,
                fence(&active),
                outcome(&plan(&storage), false),
                Some(retry(1, 18)),
                14,
            )
            .unwrap(),
        );
        let original = block_on(storage.execute_authored(command)).unwrap();
        block_on(storage.execute_authored(fact(&plan(&storage), active.clone(), accepted, 50)))
            .unwrap();
        let before = plan(&storage);
        let command = reconcile(&before, None, Some(retry(1, 60)), 51);
        if accepted {
            assert_eq!(
                block_on(storage.execute_authored(command)),
                Err(Error::AtomicWorkflowMismatch)
            );
            assert_eq!(plan(&storage), before);
            // Conflicting raw facts remain factual evidence, not a replacement
            // for the immutable earlier attempt or authority for another effect.
            assert_eq!(
                before.delivery_satisfaction().unwrap(),
                SatisfactionState::Satisfied
            );
        } else {
            block_on(storage.execute_authored(command.clone())).unwrap();
            let after = plan(&storage);
            assert_eq!(after.attempt_count(), 1);
            assert_eq!(after.attempts()[0].recorded_at_unix_ms(), 14);
            assert_eq!(after.attempts()[0].claim_evidence(), Some(&active));
            assert_eq!(after.pending_delivery_facts().count(), 0);
            assert_eq!(
                block_on(storage.execute_authored(command))
                    .unwrap()
                    .disposition(),
                AtomicCommitDisposition::Replay
            );
        }
        assert_eq!(
            block_on(storage.authored_receipt(original.commit_id()))
                .unwrap()
                .unwrap(),
            original
        );
    }
}

#[test]
fn partial_acceptance_and_terminal_evidence_settle_without_retry_authority() {
    for (retryability, partial, expected) in [
        (
            Retryability::Terminal,
            None,
            AuthoredDeliveryState::FailedTerminal,
        ),
        (
            Retryability::Retryable,
            Some(true),
            AuthoredDeliveryState::Satisfied,
        ),
        (
            Retryability::Retryable,
            Some(false),
            AuthoredDeliveryState::Exhausted,
        ),
        (
            Retryability::Terminal,
            Some(false),
            AuthoredDeliveryState::Exhausted,
        ),
    ] {
        let (storage, active, _) = prepared();
        let before = plan(&storage);
        let evidence = partial
            .map(|accepted| {
                DeliveryTargetReceipt::attempted(
                    before.request().unwrap().target_set().targets()[0].clone(),
                    if accepted {
                        DeliveryOutcome::accepted()
                    } else {
                        DeliveryOutcome::rejected()
                    },
                )
            })
            .into_iter()
            .collect();
        let failure = SinkFailure::for_request(
            before.request().unwrap(),
            "sink_lost",
            retryability,
            None,
            None,
            evidence,
        )
        .unwrap();
        block_on(
            storage.execute_authored(AuthoredAtomicCommand::RecordDelivery(
                RecordDeliveryFact::new(
                    ids().2,
                    ids().1,
                    active,
                    DeliveryAttemptOutcome::SinkFailure(failure),
                    50,
                )
                .unwrap(),
            )),
        )
        .unwrap();
        let before = plan(&storage);
        assert_eq!(
            block_on(storage.execute_authored(reconcile(&before, None, Some(retry(1, 60)), 51))),
            Err(Error::InvalidRetrySchedule)
        );
        assert_eq!(plan(&storage), before);
        block_on(storage.execute_authored(reconcile(&before, None, None, 51))).unwrap();
        assert_eq!(plan(&storage).state(), expected);
        assert!(plan(&storage).retry().is_none());
    }
}

#[test]
fn earlier_unresolved_claim_cannot_adopt_a_new_workers_legacy_attempt() {
    let (storage, first, _) = prepared();
    let second = claim(plan(&storage).revision(), 3, 40);
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
            second.clone(),
        ))),
    )
    .unwrap();
    block_on(storage.execute_authored(fact(&plan(&storage), first, false, 41))).unwrap();
    block_on(storage.execute_authored(reconcile(
        &plan(&storage),
        Some(fence(&second)),
        Some(retry(1, 43)),
        42,
    )))
    .unwrap();
    let third = claim(plan(&storage).revision(), 4, 44);
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
            third.clone(),
        ))),
    )
    .unwrap();
    let legacy = AuthoredAtomicCommand::ApplyDelivery(
        ApplyDeliveryAttempt::new(
            ids().2,
            fence(&third),
            outcome(&plan(&storage), false),
            Some(retry(2, 47)),
            45,
        )
        .unwrap(),
    );
    block_on(storage.execute_authored(legacy)).unwrap();
    assert!(history(&storage).has_unresolved_claims());
    block_on(storage.execute_authored(fact(&plan(&storage), second.clone(), false, 50))).unwrap();
    block_on(storage.execute_authored(reconcile(&plan(&storage), None, Some(retry(3, 65)), 61)))
        .unwrap();
    let after = plan(&storage);
    assert_eq!(after.attempt_count(), 3);
    assert!(after.attempts()[1].claim_evidence().is_none());
    assert_eq!(after.attempts()[2].claim_evidence(), Some(&second));
    assert!(!history(&storage).has_unresolved_claims());
}

fn restored_receipt(
    original: &AuthoredAtomicReceipt,
    plan: AuthoredDeliveryPlan,
    at: u64,
) -> AuthoredAtomicReceipt {
    AuthoredAtomicReceipt::from_durable_parts(
        original.commit_id(),
        original.digest(),
        AtomicCommitDisposition::Committed,
        at,
        AuthoredAtomicOutcome::DeliveryPlan(plan),
    )
    .unwrap()
}

#[test]
fn original_claim_history_rejects_each_mismatched_binding() {
    let (storage, _, original) = prepared();
    let current = plan(&storage);
    for field in ["plan_id", "artifact_id", "request", "created_at_unix_ms"] {
        let mut wire = serde_json::to_value(&current).unwrap();
        match field {
            "plan_id" => {
                wire[field] = serde_json::to_value(
                    radroots_storage::authored_delivery::AuthoredDeliveryPlanId::new([99; 16])
                        .unwrap(),
                )
                .unwrap()
            }
            "artifact_id" => {
                wire[field] = serde_json::to_value(
                    radroots_storage::authored::AuthoredArtifactId::new([99; 16]).unwrap(),
                )
                .unwrap()
            }
            "request" => wire[field] = serde_json::Value::Null,
            _ => wire[field] = serde_json::json!(9),
        }
        let altered = serde_json::from_value(wire).unwrap();
        let forged = restored_receipt(&original, altered, 13);
        let mut history = AuthoredDeliveryHistory::new(current.clone(), None).unwrap();
        assert_eq!(
            history.push_claim(&forged),
            Err(Error::AtomicWorkflowMismatch),
            "{field}"
        );
        assert!(history.claims().is_empty());
    }
    let mut rebound = serde_json::to_value(&current).unwrap();
    rebound["request"] = serde_json::to_value(
        current
            .intent()
            .materialize(radroots_transport::sink::DeliveryPayload::new(event(
                OTHER_RAW,
            )))
            .unwrap(),
    )
    .unwrap();
    let mut history =
        AuthoredDeliveryHistory::new(serde_json::from_value(rebound).unwrap(), None).unwrap();
    assert_eq!(
        history.push_claim(&original),
        Err(Error::AtomicWorkflowMismatch)
    );
    let mut history = AuthoredDeliveryHistory::new(current.clone(), None).unwrap();
    assert_eq!(
        history.push_claim(&restored_receipt(&original, current.clone(), 14)),
        Err(Error::AtomicWorkflowMismatch)
    );
    let corrupt_digest = AuthoredAtomicReceipt::from_durable_parts(
        original.commit_id(),
        AtomicCommitDigest::new([99; 32]),
        AtomicCommitDisposition::Committed,
        13,
        original.outcome().clone(),
    )
    .unwrap();
    assert_eq!(
        history.push_claim(&corrupt_digest),
        Err(Error::AtomicWorkflowMismatch)
    );
    // A structurally valid future original cannot explain the current row.
    for (revision, at) in [
        (current.revision().get() + 1, 13),
        (current.revision().get(), 14),
    ] {
        let mut wire = serde_json::to_value(&current).unwrap();
        let active = claim(NonZeroU64::new(revision - 1).unwrap(), 8, at);
        wire["revision"] = serde_json::json!(revision);
        wire["updated_at_unix_ms"] = serde_json::json!(at);
        wire["claim"] = serde_json::to_value(&active).unwrap();
        let future = serde_json::from_value(wire).unwrap();
        let command = AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
            active,
        ));
        let receipt = AuthoredAtomicReceipt::new(
            &command,
            AtomicCommitDisposition::Committed,
            at,
            AuthoredAtomicOutcome::DeliveryPlan(future),
        )
        .unwrap();
        assert_eq!(
            history.push_claim(&receipt),
            Err(Error::AtomicWorkflowMismatch)
        );
    }
}

#[test]
fn preparation_history_requires_exact_initial_identity_and_monotonic_row() {
    let storage = MemoryStorage::new(SourceGeneration::new([73; 32]).unwrap());
    let original = block_on(storage.execute_authored(prepare().0)).unwrap();
    let initial = plan(&storage);
    for field in ["plan_id", "artifact_id", "created_at_unix_ms"] {
        let mut wire = serde_json::to_value(&initial).unwrap();
        match field {
            "plan_id" => {
                wire[field] = serde_json::to_value(
                    radroots_storage::authored_delivery::AuthoredDeliveryPlanId::new([99; 16])
                        .unwrap(),
                )
                .unwrap()
            }
            "artifact_id" => {
                wire[field] = serde_json::to_value(
                    radroots_storage::authored::AuthoredArtifactId::new([99; 16]).unwrap(),
                )
                .unwrap()
            }
            _ => wire[field] = serde_json::json!(9),
        }
        assert!(
            AuthoredDeliveryHistory::new(serde_json::from_value(wire).unwrap(), Some(&original))
                .is_err(),
            "{field}"
        );
    }
    let intent = radroots_storage::authored_delivery::AuthoredDeliveryIntent::new(
        "different-intent",
        initial.intent().target_set().clone(),
        initial.intent().satisfaction().clone(),
        100,
    )
    .unwrap();
    let changed = AuthoredDeliveryPlan::new(ids().2, ids().1, intent, 10).unwrap();
    assert!(AuthoredDeliveryHistory::new(changed, Some(&original)).is_err());
    for field in ["revision", "updated_at_unix_ms"] {
        let mut wire = serde_json::to_value(&initial).unwrap();
        wire[field] = serde_json::json!(12);
        let altered = serde_json::from_value(wire).unwrap();
        let AuthoredAtomicOutcome::Prepared {
            operation,
            artifacts,
            ..
        } = original.outcome()
        else {
            unreachable!()
        };
        let receipt = AuthoredAtomicReceipt::from_durable_parts(
            original.commit_id(),
            original.digest(),
            AtomicCommitDisposition::Committed,
            12,
            AuthoredAtomicOutcome::Prepared {
                operation: operation.clone(),
                artifacts: artifacts.clone(),
                delivery_plans: vec![altered],
            },
        )
        .unwrap();
        assert!(
            AuthoredDeliveryHistory::new(initial.clone(), Some(&receipt)).is_err(),
            "{field}"
        );
    }
}

#[test]
fn reconciliation_receipts_cannot_substitute_a_different_plan_or_result() {
    let (storage, active, _) = prepared();
    block_on(storage.execute_authored(fact(&plan(&storage), active.clone(), false, 14))).unwrap();
    let command = reconcile(
        &plan(&storage),
        Some(fence(&active)),
        Some(retry(1, 20)),
        15,
    );
    let receipt = block_on(storage.execute_authored(command.clone())).unwrap();
    let after = plan(&storage);
    for field in [
        "plan_id",
        "revision",
        "updated_at_unix_ms",
        "claim",
        "stop",
        "pending",
        "facts",
        "retry",
    ] {
        let mut wire = serde_json::to_value(&after).unwrap();
        match field {
            "plan_id" => {
                wire[field] = serde_json::to_value(
                    radroots_storage::authored_delivery::AuthoredDeliveryPlanId::new([99; 16])
                        .unwrap(),
                )
                .unwrap()
            }
            "revision" => wire[field] = serde_json::json!(after.revision().get() + 1),
            "updated_at_unix_ms" => wire[field] = serde_json::json!(16),
            "claim" => {
                wire[field] = serde_json::to_value(claim(
                    NonZeroU64::new(after.revision().get() - 1).unwrap(),
                    9,
                    15,
                ))
                .unwrap();
            }
            "stop" => {
                wire["stop_requested_at_unix_ms"] = serde_json::json!(15);
                wire["state"] = serde_json::json!("cancelled");
                wire["retry"] = serde_json::Value::Null;
                wire["last_failure"] = serde_json::Value::Null;
            }
            "pending" => wire["attempts"][0]["claim"] = serde_json::Value::Null,
            "facts" => wire["delivery_facts"][0]["observed_at_unix_ms"] = serde_json::json!(15),
            _ => {
                let other = retry(1, 21);
                wire["retry"] = serde_json::to_value(&other).unwrap();
                wire["last_failure"] = serde_json::to_value(other.failure()).unwrap();
            }
        }
        let altered: AuthoredDeliveryPlan = serde_json::from_value(wire).unwrap();
        let wrong = restored_receipt(&receipt, altered.clone(), 15);
        assert!(!wrong.matches_command(&command), "{field}");
        assert_eq!(
            AuthoredAtomicReceipt::new(
                &command,
                AtomicCommitDisposition::Committed,
                15,
                AuthoredAtomicOutcome::DeliveryPlan(altered)
            ),
            Err(Error::AtomicWorkflowMismatch),
            "{field}"
        );
    }
    let artifact = block_on(storage.authored_artifact(ids().1))
        .unwrap()
        .unwrap();
    let wrong = AuthoredAtomicReceipt::from_durable_parts(
        receipt.commit_id(),
        receipt.digest(),
        AtomicCommitDisposition::Committed,
        15,
        AuthoredAtomicOutcome::Artifact(artifact.clone()),
    )
    .unwrap();
    assert!(!wrong.matches_command(&command));
    assert!(
        AuthoredAtomicReceipt::new(
            &command,
            AtomicCommitDisposition::Committed,
            15,
            AuthoredAtomicOutcome::Artifact(artifact)
        )
        .is_err()
    );
}

#[test]
fn missing_fact_claims_and_prepopulated_preparation_remain_uncertain() {
    let (storage, active, _) = prepared();
    let original = block_on(storage.authored_receipt(prepare().0.commit_id()))
        .unwrap()
        .unwrap();
    let AuthoredAtomicOutcome::Prepared {
        operation,
        artifacts,
        ..
    } = original.outcome()
    else {
        unreachable!()
    };
    let prepopulated = AuthoredAtomicReceipt::from_durable_parts(
        original.commit_id(),
        original.digest(),
        AtomicCommitDisposition::Committed,
        13,
        AuthoredAtomicOutcome::Prepared {
            operation: operation.clone(),
            artifacts: artifacts.clone(),
            delivery_plans: vec![plan(&storage)],
        },
    )
    .unwrap();
    let legacy = AuthoredDeliveryHistory::new(plan(&storage), Some(&prepopulated)).unwrap();
    assert!(!legacy.is_complete());
    assert!(!legacy.proves_no_issued_attempt());
    assert!(legacy.has_unresolved_claims());
    block_on(storage.execute_authored(fact(&plan(&storage), active, true, 50))).unwrap();
    stop(&storage, 51);
    let unknown = AuthoredDeliveryHistory::new(plan(&storage), None).unwrap();
    assert_eq!(
        unknown.require_pending_fact_provenance(),
        Err(Error::AtomicWorkflowMismatch)
    );
    let incomplete = AuthoredDeliveryHistory::new(plan(&storage), Some(&original)).unwrap();
    assert_eq!(incomplete.validate(), Err(Error::AtomicWorkflowMismatch));
}

#[test]
fn later_legacy_attempt_outside_old_lease_cannot_resolve_old_claim() {
    let (storage, _, _) = prepared();
    let newer = claim(plan(&storage).revision(), 3, 40);
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
            newer.clone(),
        ))),
    )
    .unwrap();
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::ApplyDelivery(
            ApplyDeliveryAttempt::new(
                ids().2,
                fence(&newer),
                outcome(&plan(&storage), true),
                None,
                41,
            )
            .unwrap(),
        )),
    )
    .unwrap();
    let history = history(&storage);
    assert_eq!(history.plan().attempt_count(), 1);
    assert_eq!(history.plan().state(), AuthoredDeliveryState::Satisfied);
    assert!(history.has_unresolved_claims());
}
