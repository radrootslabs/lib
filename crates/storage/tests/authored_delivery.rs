use core::num::{NonZeroU32, NonZeroU64};
use radroots_event::{SignedEvent, wire::v1::Nip01EventWire};
use radroots_storage::{
    Error,
    authored::{
        AuthoredArtifactId, FailureClass, RetrySchedule, WorkClaim, WorkFailure, WorkPhase,
    },
    authored_delivery::{
        AuthoredDeliveryAttempt, AuthoredDeliveryIntent, AuthoredDeliveryPlan,
        AuthoredDeliveryPlanId, AuthoredDeliveryState, DELIVERY_PLAN_ATTEMPTS_MAX,
        DeliveryAttemptOutcome,
    },
};
use radroots_transport::{
    DeliveryReceipt, DeliveryRequest, SinkFailure, Target, TargetSet,
    outcome::{DeliveryOutcome, Retryability},
    policy::{SatisfactionClass, SatisfactionPolicy, SatisfactionState, TargetPolicy},
    sink::{DeliveryPayload, DeliveryTargetReceipt},
};

fn signed_event() -> SignedEvent {
    let mut wire = Nip01EventWire {
        id: "0".repeat(64),
        pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".to_owned(),
        created_at: 1_800_000_100,
        kind: 20_000,
        tags: Vec::new(),
        content: "delivery plan".to_owned(),
        sig: "dd".repeat(64),
        extra: Default::default(),
    };
    wire.id = wire.computed_event_id().expect("event ID").to_hex();
    let raw = serde_json::to_string(&wire).expect("raw event");
    SignedEvent::from_wire_verified_id(wire, raw).expect("signed event")
}

fn target_set() -> TargetSet {
    TargetSet::new(vec![
        Target::nostr_relay("wss://one.example").expect("one"),
        Target::nostr_relay("wss://two.example").expect("two"),
    ])
    .expect("targets")
}

fn request(policy: TargetPolicy) -> DeliveryRequest {
    DeliveryRequest::new(
        "authored-delivery",
        DeliveryPayload::new(signed_event()),
        target_set(),
        SatisfactionPolicy::new(SatisfactionClass::Accepted, policy),
        100,
    )
    .expect("request")
}

fn plan(value: u8, policy: TargetPolicy) -> AuthoredDeliveryPlan {
    AuthoredDeliveryPlan::new_bound(
        AuthoredDeliveryPlanId::new([value; 16]).expect("plan ID"),
        AuthoredArtifactId::new([9; 16]).expect("artifact ID"),
        request(policy),
        10,
    )
    .expect("plan")
}

fn bound_request(plan: &AuthoredDeliveryPlan) -> &DeliveryRequest {
    plan.request().expect("bound request")
}

fn claim(plan: &AuthoredDeliveryPlan, token: u8, acquired: u64) -> WorkClaim {
    WorkClaim::new(
        [token; 16],
        format!("worker-{token}"),
        NonZeroU64::new(u64::from(token)).expect("generation"),
        acquired,
        acquired + 20,
        plan.revision(),
    )
    .expect("claim")
}

fn receipt(request: &DeliveryRequest, outcomes: Vec<DeliveryOutcome>) -> DeliveryReceipt {
    DeliveryReceipt::for_request(
        request,
        request
            .target_set()
            .targets()
            .iter()
            .cloned()
            .zip(outcomes)
            .map(|(target, outcome)| DeliveryTargetReceipt::attempted(target, outcome))
            .collect(),
    )
    .expect("receipt")
}

fn retry(code: &str, attempt: u32, not_before: u64) -> RetrySchedule {
    let failure = WorkFailure::new(
        code,
        WorkPhase::Delivery,
        FailureClass::Retryable,
        Some(not_before),
        None,
    )
    .expect("failure");
    RetrySchedule::new(
        NonZeroU32::new(attempt).expect("attempt"),
        not_before,
        failure,
    )
    .expect("retry")
}

#[test]
fn independent_plans_claim_and_progress_without_cross_blocking() {
    let mut first = plan(1, TargetPolicy::any());
    let mut second = plan(2, TargetPolicy::all());
    let first_claim = claim(&first, 1, 11);
    let second_claim = claim(&second, 2, 11);
    first.claim(first_claim.clone(), 11).expect("first claim");
    second
        .claim(second_claim.clone(), 11)
        .expect("second claim");

    let first_receipt = receipt(
        bound_request(&first),
        vec![DeliveryOutcome::accepted(), DeliveryOutcome::unavailable()],
    );
    first
        .apply_receipt(
            first_claim.token(),
            first_claim.generation(),
            first_claim.row_revision(),
            first_receipt,
            None,
            12,
        )
        .expect("first satisfied");
    assert_eq!(first.state(), AuthoredDeliveryState::Satisfied);
    assert!(second.claim_evidence().is_some());

    let second_receipt = receipt(
        bound_request(&second),
        vec![DeliveryOutcome::accepted(), DeliveryOutcome::unavailable()],
    );
    second
        .apply_receipt(
            second_claim.token(),
            second_claim.generation(),
            second_claim.row_revision(),
            second_receipt,
            Some(retry("delivery_pending", 1, 20)),
            12,
        )
        .expect("second retry");
    assert_eq!(second.state(), AuthoredDeliveryState::Retryable);
    assert_eq!(second.retry().expect("retry").not_before_unix_ms(), 20);
    let before_early_claim = second.clone();
    let early = claim(&second, 7, 19);
    assert_eq!(
        second.claim(early, 19),
        Err(Error::DeliveryPlanClaimConflict)
    );
    assert_eq!(second, before_early_claim);
}

#[test]
fn retry_schedule_and_partial_evidence_survive_reconstruction() {
    let mut delivery = plan(3, TargetPolicy::all());
    let active = claim(&delivery, 3, 11);
    delivery.claim(active.clone(), 11).expect("claim");
    let partial = DeliveryTargetReceipt::attempted(
        bound_request(&delivery).target_set().targets()[0].clone(),
        DeliveryOutcome::accepted(),
    );
    let sink_failure = SinkFailure::for_request(
        bound_request(&delivery),
        "relay_batch_unavailable",
        Retryability::Retryable,
        Some(20),
        None,
        vec![partial],
    )
    .expect("sink failure");
    delivery
        .apply_sink_failure(
            active.token(),
            active.generation(),
            active.row_revision(),
            sink_failure,
            Some(retry("relay_batch_unavailable", 1, 20)),
            12,
        )
        .expect("retryable failure");

    let json = serde_json::to_string(&delivery).expect("plan json");
    let reopened: AuthoredDeliveryPlan = serde_json::from_str(&json).expect("reopen plan");
    assert_eq!(reopened, delivery);
    assert_eq!(reopened.attempt_count(), 1);
    assert_eq!(reopened.state(), AuthoredDeliveryState::Retryable);
    assert_eq!(
        reopened.attempts()[0].satisfaction(),
        SatisfactionState::Pending
    );
    assert_eq!(
        reopened.last_failure().expect("failure").code(),
        "relay_batch_unavailable"
    );
}

#[test]
fn terminal_partial_success_stale_claim_and_invalid_receipt_fail_closed() {
    let mut any = plan(4, TargetPolicy::any());
    let active = claim(&any, 4, 11);
    any.claim(active.clone(), 11).expect("claim");
    let partial = DeliveryTargetReceipt::attempted(
        bound_request(&any).target_set().targets()[0].clone(),
        DeliveryOutcome::accepted(),
    );
    let failure = SinkFailure::for_request(
        bound_request(&any),
        "terminal_batch_failure",
        Retryability::Terminal,
        None,
        None,
        vec![partial],
    )
    .expect("terminal failure");
    let other_request = DeliveryRequest::new(
        "other-request",
        bound_request(&any).payload().clone(),
        bound_request(&any).target_set().clone(),
        bound_request(&any).satisfaction().clone(),
        bound_request(&any).deadline_unix_ms(),
    )
    .expect("other request");
    let invalid_receipt = receipt(
        &other_request,
        vec![DeliveryOutcome::accepted(), DeliveryOutcome::accepted()],
    );
    assert_eq!(
        any.apply_receipt(
            active.token(),
            active.generation(),
            active.row_revision(),
            invalid_receipt,
            None,
            12,
        ),
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    assert_eq!(any.attempt_count(), 0);
    assert!(any.claim_evidence().is_some());
    assert_eq!(
        any.apply_sink_failure(
            &[8; 16],
            active.generation(),
            active.row_revision(),
            failure.clone(),
            None,
            12,
        ),
        Err(Error::DeliveryPlanClaimConflict)
    );
    assert_eq!(any.attempt_count(), 0);
    any.apply_sink_failure(
        active.token(),
        active.generation(),
        active.row_revision(),
        failure,
        None,
        12,
    )
    .expect("partial satisfaction");
    assert_eq!(any.state(), AuthoredDeliveryState::Satisfied);

    let mut all = plan(5, TargetPolicy::all());
    let all_claim = claim(&all, 5, 11);
    all.claim(all_claim.clone(), 11).expect("claim");
    let terminal = SinkFailure::for_request(
        bound_request(&all),
        "terminal_batch_failure",
        Retryability::Terminal,
        None,
        None,
        Vec::new(),
    )
    .expect("terminal failure");
    all.apply_sink_failure(
        all_claim.token(),
        all_claim.generation(),
        all_claim.row_revision(),
        terminal,
        None,
        12,
    )
    .expect("terminal apply");
    assert_eq!(all.state(), AuthoredDeliveryState::FailedTerminal);
}

#[test]
fn attempt_limit_is_checked_without_mutating_claimed_state() {
    let base = plan(6, TargetPolicy::all());
    let pending_receipt = receipt(
        bound_request(&base),
        vec![
            DeliveryOutcome::unavailable(),
            DeliveryOutcome::unavailable(),
        ],
    );
    let attempts: Vec<_> = (1..=DELIVERY_PLAN_ATTEMPTS_MAX)
        .map(|attempt| {
            AuthoredDeliveryAttempt::reconstruct(
                NonZeroU32::new(attempt).expect("attempt"),
                12,
                DeliveryAttemptOutcome::Receipt(pending_receipt.clone()),
                SatisfactionState::Pending,
            )
            .expect("attempt record")
        })
        .collect();
    let mut value = serde_json::to_value(&base).expect("plan value");
    value["state"] = serde_json::json!("retryable");
    value["attempts"] = serde_json::to_value(attempts).expect("attempts value");
    value["attempt_count"] = serde_json::json!(DELIVERY_PLAN_ATTEMPTS_MAX);
    let schedule = retry("delivery_pending", DELIVERY_PLAN_ATTEMPTS_MAX, 20);
    value["retry"] = serde_json::to_value(&schedule).expect("retry value");
    value["last_failure"] = serde_json::to_value(schedule.failure()).expect("failure value");
    value["updated_at_unix_ms"] = serde_json::json!(12);
    let mut saturated: AuthoredDeliveryPlan =
        serde_json::from_value(value).expect("saturated plan");
    let active = claim(&saturated, 6, 20);
    saturated.claim(active.clone(), 20).expect("claim");
    let before = saturated.clone();
    assert_eq!(
        saturated.apply_receipt(
            active.token(),
            active.generation(),
            active.row_revision(),
            pending_receipt,
            Some(retry("delivery_pending", DELIVERY_PLAN_ATTEMPTS_MAX, 30,)),
            21,
        ),
        Err(Error::DeliveryAttemptOverflow)
    );
    assert_eq!(saturated, before);
}

#[test]
fn delivery_identity_intent_state_and_attempt_contracts_are_total() {
    assert_eq!(
        AuthoredDeliveryPlanId::new([0; 16]),
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    let id = AuthoredDeliveryPlanId::new([7; 16]).unwrap();
    assert_eq!(id.as_bytes(), &[7; 16]);
    assert_eq!(AuthoredDeliveryPlanId::try_from([7; 16]).unwrap(), id);
    assert_eq!(<[u8; 16]>::from(id), [7; 16]);

    let targets = target_set();
    let policy = SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all());
    assert_eq!(
        AuthoredDeliveryIntent::new("", targets.clone(), policy.clone(), 100),
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    assert_eq!(
        AuthoredDeliveryIntent::new("intent", targets.clone(), policy.clone(), 0),
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    let invalid_policy = SatisfactionPolicy::new(
        SatisfactionClass::Accepted,
        TargetPolicy::required(vec![
            Target::nostr_relay("wss://foreign.example")
                .unwrap()
                .fingerprint()
                .clone(),
        ])
        .unwrap(),
    );
    assert_eq!(
        AuthoredDeliveryIntent::new("intent", targets.clone(), invalid_policy, 100),
        Err(Error::InvalidAuthoredDeliveryPlan)
    );

    for policy in [
        TargetPolicy::any(),
        TargetPolicy::all(),
        TargetPolicy::quorum(2).unwrap(),
        TargetPolicy::required(vec![targets.targets()[0].fingerprint().clone()]).unwrap(),
    ] {
        for class in [SatisfactionClass::Accepted, SatisfactionClass::Delivered] {
            let request = DeliveryRequest::new(
                "intent",
                DeliveryPayload::new(signed_event()),
                targets.clone(),
                SatisfactionPolicy::new(class, policy.clone()),
                100,
            )
            .unwrap();
            let intent = AuthoredDeliveryIntent::from_request(&request);
            assert_eq!(intent.request_id(), request.request_id());
            assert_eq!(intent.target_set(), request.target_set());
            assert_eq!(intent.satisfaction(), request.satisfaction());
            assert_eq!(intent.deadline_unix_ms(), 100);
            assert_eq!(
                intent.materialize(request.payload().clone()).unwrap(),
                request
            );
            AuthoredDeliveryPlan::new(id, artifact_id(9), intent, 10).unwrap();
        }
    }

    for (state, terminal) in [
        (AuthoredDeliveryState::Pending, false),
        (AuthoredDeliveryState::Retryable, false),
        (AuthoredDeliveryState::Satisfied, true),
        (AuthoredDeliveryState::Exhausted, true),
        (AuthoredDeliveryState::FailedTerminal, true),
        (AuthoredDeliveryState::Cancelled, true),
    ] {
        assert_eq!(state.is_terminal(), terminal);
    }

    let outcome = DeliveryAttemptOutcome::Receipt(receipt(
        &request(TargetPolicy::all()),
        vec![DeliveryOutcome::accepted(), DeliveryOutcome::unavailable()],
    ));
    assert_eq!(
        AuthoredDeliveryAttempt::reconstruct(
            NonZeroU32::MIN,
            0,
            outcome.clone(),
            SatisfactionState::Pending,
        ),
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    let attempt = AuthoredDeliveryAttempt::reconstruct(
        NonZeroU32::MIN,
        12,
        outcome.clone(),
        SatisfactionState::Pending,
    )
    .unwrap();
    assert_eq!(attempt.attempt(), NonZeroU32::MIN);
    assert_eq!(attempt.recorded_at_unix_ms(), 12);
    assert_eq!(attempt.outcome(), &outcome);
    assert_eq!(attempt.satisfaction(), SatisfactionState::Pending);
}

fn artifact_id(value: u8) -> AuthoredArtifactId {
    AuthoredArtifactId::new([value; 16]).unwrap()
}

fn assert_plan_json_rejected(
    mut value: serde_json::Value,
    key: &str,
    replacement: serde_json::Value,
) {
    value[key] = replacement;
    assert!(
        serde_json::from_value::<AuthoredDeliveryPlan>(value).is_err(),
        "forged field {key} was accepted"
    );
}

#[test]
fn reconstructed_delivery_plans_fail_closed_for_each_durable_invariant() {
    let base = plan(8, TargetPolicy::all());
    let value = serde_json::to_value(&base).unwrap();
    assert_plan_json_rejected(value.clone(), "created_at_unix_ms", serde_json::json!(0));
    assert_plan_json_rejected(value.clone(), "updated_at_unix_ms", serde_json::json!(9));
    assert_plan_json_rejected(
        value.clone(),
        "request_digest",
        serde_json::to_value([0_u8; 32]).unwrap(),
    );
    assert_plan_json_rejected(
        value.clone(),
        "attempt_count",
        serde_json::json!(DELIVERY_PLAN_ATTEMPTS_MAX + 1),
    );
    assert_plan_json_rejected(value.clone(), "attempt_count", serde_json::json!(1));
    assert_plan_json_rejected(value.clone(), "state", serde_json::json!("retryable"));
    assert_plan_json_rejected(value.clone(), "state", serde_json::json!("failed_terminal"));

    let mut wrong_exhausted_phase = value.clone();
    wrong_exhausted_phase["state"] = serde_json::json!("exhausted");
    wrong_exhausted_phase["last_failure"] =
        serde_json::to_value(retry_failure_for_phase(WorkPhase::Signing, 20)).unwrap();
    assert!(serde_json::from_value::<AuthoredDeliveryPlan>(wrong_exhausted_phase).is_err());
    let mut wrong_exhausted_class = value.clone();
    wrong_exhausted_class["state"] = serde_json::json!("exhausted");
    wrong_exhausted_class["last_failure"] =
        serde_json::to_value(retry_failure_for_phase(WorkPhase::Delivery, 20)).unwrap();
    assert!(serde_json::from_value::<AuthoredDeliveryPlan>(wrong_exhausted_class).is_err());

    let mut request_mismatch = value.clone();
    request_mismatch["request"]["request_id"] = serde_json::json!("different-request");
    assert!(serde_json::from_value::<AuthoredDeliveryPlan>(request_mismatch).is_err());

    let pending_receipt = receipt(
        bound_request(&base),
        vec![
            DeliveryOutcome::unavailable(),
            DeliveryOutcome::unavailable(),
        ],
    );
    let attempt = AuthoredDeliveryAttempt::reconstruct(
        NonZeroU32::MIN,
        12,
        DeliveryAttemptOutcome::Receipt(pending_receipt),
        SatisfactionState::Pending,
    )
    .unwrap();
    let mut attempt_without_request = value.clone();
    attempt_without_request["request"] = serde_json::Value::Null;
    attempt_without_request["attempts"] = serde_json::json!([attempt]);
    attempt_without_request["attempt_count"] = serde_json::json!(1);
    assert!(serde_json::from_value::<AuthoredDeliveryPlan>(attempt_without_request).is_err());

    let active = claim(&base, 8, 11);
    let mut claimed = base.clone();
    claimed.claim(active, 11).unwrap();
    let claimed_value = serde_json::to_value(&claimed).unwrap();
    assert_plan_json_rejected(
        claimed_value.clone(),
        "state",
        serde_json::json!("satisfied"),
    );
    for (field, replacement) in [
        ("token", serde_json::to_value([0_u8; 16]).unwrap()),
        ("acquired_at_unix_ms", serde_json::json!(10)),
        ("row_revision", serde_json::json!(9)),
    ] {
        let mut forged = claimed_value.clone();
        forged["claim"][field] = replacement;
        assert!(serde_json::from_value::<AuthoredDeliveryPlan>(forged).is_err());
    }

    let mut retryable = plan(9, TargetPolicy::all());
    let active = claim(&retryable, 9, 11);
    retryable.claim(active.clone(), 11).unwrap();
    retryable
        .apply_receipt(
            active.token(),
            active.generation(),
            active.row_revision(),
            receipt(
                bound_request(&retryable),
                vec![DeliveryOutcome::accepted(), DeliveryOutcome::unavailable()],
            ),
            Some(retry("delivery_pending", 1, 20)),
            12,
        )
        .unwrap();
    let retryable_value = serde_json::to_value(&retryable).unwrap();
    for (key, replacement) in [
        ("attempt_count", serde_json::json!(0)),
        ("retry", serde_json::Value::Null),
        ("last_failure", serde_json::Value::Null),
    ] {
        assert_plan_json_rejected(retryable_value.clone(), key, replacement);
    }
    for (field, replacement) in [
        ("attempt", serde_json::json!(2)),
        ("recorded_at_unix_ms", serde_json::json!(9)),
        ("satisfaction", serde_json::json!("satisfied")),
    ] {
        let mut forged = retryable_value.clone();
        forged["attempts"][0][field] = replacement;
        assert!(serde_json::from_value::<AuthoredDeliveryPlan>(forged).is_err());
    }
    for (field, replacement) in [
        ("attempt", serde_json::json!(2)),
        ("not_before_unix_ms", serde_json::json!(12)),
    ] {
        let mut forged = retryable_value.clone();
        forged["retry"][field] = replacement;
        assert!(serde_json::from_value::<AuthoredDeliveryPlan>(forged).is_err());
    }
}

#[test]
fn delivery_claim_binding_cancellation_and_retry_validation_are_transactional() {
    let mut unbound = AuthoredDeliveryPlan::new(
        AuthoredDeliveryPlanId::new([10; 16]).unwrap(),
        artifact_id(10),
        AuthoredDeliveryIntent::from_request(&request(TargetPolicy::all())),
        10,
    )
    .unwrap();
    assert!(unbound.request().is_none());
    assert_eq!(unbound.attempts(), &[]);
    assert_eq!(unbound.attempt_count(), 0);
    assert_eq!(unbound.retry(), None);
    assert_eq!(unbound.claim_evidence(), None);
    assert_eq!(unbound.last_failure(), None);
    assert_eq!(unbound.created_at_unix_ms(), 10);
    assert_eq!(unbound.updated_at_unix_ms(), 10);
    assert_eq!(unbound.revision(), NonZeroU64::MIN);
    let impossible_claim = WorkClaim::new(
        [1; 16],
        "worker",
        NonZeroU64::MIN,
        11,
        20,
        unbound.revision(),
    )
    .unwrap();
    assert_eq!(
        unbound.claim(impossible_claim, 11),
        Err(Error::DeliveryPlanClaimConflict)
    );
    assert_eq!(
        unbound.evaluate_next_attempt(&DeliveryAttemptOutcome::Receipt(receipt(
            &request(TargetPolicy::all()),
            vec![DeliveryOutcome::accepted(), DeliveryOutcome::accepted()],
        ))),
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    assert_eq!(
        unbound.bind_signed_event(signed_event(), 9),
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    unbound.bind_signed_event(signed_event(), 11).unwrap();
    assert!(unbound.request().is_some());
    assert_eq!(
        unbound.bind_signed_event(signed_event(), 12),
        Err(Error::InvalidAuthoredDeliveryPlan)
    );

    let active = claim(&unbound, 10, 12);
    let stale_revision = WorkClaim::new(
        [11; 16],
        "worker",
        NonZeroU64::new(11).unwrap(),
        12,
        20,
        NonZeroU64::MIN,
    )
    .unwrap();
    assert_eq!(
        unbound.claim(stale_revision, 12),
        Err(Error::DeliveryPlanClaimConflict)
    );
    unbound.claim(active.clone(), 12).unwrap();
    assert_eq!(
        unbound.claim(claim(&unbound, 11, 13), 13),
        Err(Error::DeliveryPlanClaimConflict)
    );
    let before = unbound.clone();
    assert_eq!(
        unbound.apply_receipt(
            active.token(),
            active.generation(),
            active.row_revision(),
            receipt(
                bound_request(&unbound),
                vec![
                    DeliveryOutcome::unavailable(),
                    DeliveryOutcome::unavailable()
                ],
            ),
            Some(
                RetrySchedule::new(
                    NonZeroU32::MIN,
                    20,
                    retry_failure_for_phase(WorkPhase::Signing, 20),
                )
                .unwrap()
            ),
            13,
        ),
        Err(Error::InvalidRetrySchedule)
    );
    assert_eq!(unbound, before);

    let mut cancelled = plan(11, TargetPolicy::all());
    cancelled.cancel(11).unwrap();
    assert_eq!(cancelled.state(), AuthoredDeliveryState::Cancelled);
    assert_eq!(
        cancelled.cancel(12),
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    assert_eq!(
        cancelled.claim(claim(&cancelled, 12, 12), 12),
        Err(Error::DeliveryPlanClaimConflict)
    );
    let mut backwards = plan(12, TargetPolicy::all());
    let before = backwards.clone();
    assert_eq!(backwards.cancel(9), Err(Error::InvalidAuthoredDeliveryPlan));
    assert_eq!(backwards, before);
}

fn retry_failure_for_phase(phase: WorkPhase, at: u64) -> WorkFailure {
    WorkFailure::new(
        "delivery_pending",
        phase,
        FailureClass::Retryable,
        Some(at),
        None,
    )
    .unwrap()
}

fn claimed_plan(value: u8) -> (AuthoredDeliveryPlan, WorkClaim) {
    let mut plan = plan(value, TargetPolicy::all());
    let active = claim(&plan, value, 11);
    plan.claim(active.clone(), 11).unwrap();
    (plan, active)
}

#[test]
fn delivery_attempt_retry_and_terminal_error_matrix_is_fail_closed() {
    let (mut satisfied, active) = claimed_plan(20);
    let accepted = receipt(
        bound_request(&satisfied),
        vec![DeliveryOutcome::accepted(), DeliveryOutcome::accepted()],
    );
    assert_eq!(
        satisfied.apply_receipt(
            active.token(),
            active.generation(),
            active.row_revision(),
            accepted,
            Some(retry("unexpected_retry", 1, 20)),
            12,
        ),
        Err(Error::InvalidRetrySchedule)
    );

    let (mut exhausted, active) = claimed_plan(21);
    let rejected = receipt(
        bound_request(&exhausted),
        vec![DeliveryOutcome::rejected(), DeliveryOutcome::rejected()],
    );
    assert_eq!(
        exhausted.apply_receipt(
            active.token(),
            active.generation(),
            active.row_revision(),
            rejected,
            Some(retry("unexpected_retry", 1, 20)),
            12,
        ),
        Err(Error::InvalidRetrySchedule)
    );

    let (mut validation_rollback, active) = claimed_plan(22);
    let pending = receipt(
        bound_request(&validation_rollback),
        vec![DeliveryOutcome::accepted(), DeliveryOutcome::unavailable()],
    );
    let before = validation_rollback.clone();
    assert_eq!(
        validation_rollback.apply_receipt(
            active.token(),
            active.generation(),
            active.row_revision(),
            pending,
            Some(retry("delivery_pending", 2, 12)),
            12,
        ),
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    assert_eq!(validation_rollback, before);

    for (value, evidence, retryability, retry_schedule, expected) in [
        (
            23,
            vec![DeliveryOutcome::accepted(), DeliveryOutcome::accepted()],
            Retryability::Terminal,
            Some(retry("sink_failure", 1, 20)),
            Err(Error::InvalidRetrySchedule),
        ),
        (
            24,
            vec![DeliveryOutcome::rejected(), DeliveryOutcome::rejected()],
            Retryability::Terminal,
            Some(retry("sink_failure", 1, 20)),
            Err(Error::InvalidRetrySchedule),
        ),
        (
            25,
            Vec::new(),
            Retryability::Retryable,
            Some(retry("different_failure", 1, 20)),
            Err(Error::InvalidRetrySchedule),
        ),
        (
            26,
            Vec::new(),
            Retryability::Terminal,
            Some(retry("sink_failure", 1, 20)),
            Err(Error::InvalidRetrySchedule),
        ),
    ] {
        let (mut plan, active) = claimed_plan(value);
        let request = bound_request(&plan);
        let partial = request
            .target_set()
            .targets()
            .iter()
            .cloned()
            .zip(evidence)
            .map(|(target, outcome)| DeliveryTargetReceipt::attempted(target, outcome))
            .collect();
        let failure = SinkFailure::for_request(
            request,
            "sink_failure",
            retryability,
            (retryability == Retryability::Retryable).then_some(20),
            None,
            partial,
        )
        .unwrap();
        let before = plan.clone();
        assert_eq!(
            plan.apply_sink_failure(
                active.token(),
                active.generation(),
                active.row_revision(),
                failure,
                retry_schedule,
                12,
            ),
            expected
        );
        assert_eq!(plan, before);
    }

    let (mut exhausted, active) = claimed_plan(27);
    let request = bound_request(&exhausted);
    let partial = request
        .target_set()
        .targets()
        .iter()
        .cloned()
        .map(|target| DeliveryTargetReceipt::attempted(target, DeliveryOutcome::rejected()))
        .collect();
    let failure = SinkFailure::for_request(
        request,
        "sink_failure",
        Retryability::Terminal,
        None,
        None,
        partial,
    )
    .unwrap();
    exhausted
        .apply_sink_failure(
            active.token(),
            active.generation(),
            active.row_revision(),
            failure,
            None,
            12,
        )
        .unwrap();
    assert_eq!(exhausted.state(), AuthoredDeliveryState::Exhausted);
}

#[test]
fn delivery_claim_guards_distinguish_generation_time_revision_and_clock() {
    let mut delivery = plan(28, TargetPolicy::all());
    let first = WorkClaim::new(
        [1; 16],
        "worker",
        NonZeroU64::new(2).unwrap(),
        11,
        20,
        delivery.revision(),
    )
    .unwrap();
    delivery.claim(first, 11).unwrap();
    let lower_generation = WorkClaim::new(
        [2; 16],
        "worker",
        NonZeroU64::MIN,
        20,
        30,
        delivery.revision(),
    )
    .unwrap();
    assert_eq!(
        delivery.claim(lower_generation, 20),
        Err(Error::DeliveryPlanClaimConflict)
    );

    let mut wrong_time = plan(29, TargetPolicy::all());
    let acquired_later = WorkClaim::new(
        [3; 16],
        "worker",
        NonZeroU64::MIN,
        12,
        20,
        wrong_time.revision(),
    )
    .unwrap();
    assert_eq!(
        wrong_time.claim(acquired_later, 11),
        Err(Error::DeliveryPlanClaimConflict)
    );

    let mut backwards = plan(30, TargetPolicy::all());
    let claim = WorkClaim::new(
        [4; 16],
        "worker",
        NonZeroU64::MIN,
        9,
        20,
        backwards.revision(),
    )
    .unwrap();
    let before = backwards.clone();
    assert_eq!(
        backwards.claim(claim, 9),
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    assert_eq!(backwards, before);
}
