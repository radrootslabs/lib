use core::num::{NonZeroU32, NonZeroU64};
use radroots_event::{SignedEvent, wire::v1::Nip01EventWire};
use radroots_storage::{
    Error,
    authored::{
        AuthoredArtifactId, FailureClass, RetrySchedule, WorkClaim, WorkFailure, WorkPhase,
    },
    authored_delivery::{
        AuthoredDeliveryAttempt, AuthoredDeliveryPlan, AuthoredDeliveryPlanId,
        AuthoredDeliveryState, DELIVERY_PLAN_ATTEMPTS_MAX, DeliveryAttemptOutcome,
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
    AuthoredDeliveryPlan::new(
        AuthoredDeliveryPlanId::new([value; 16]).expect("plan ID"),
        AuthoredArtifactId::new([9; 16]).expect("artifact ID"),
        request(policy),
        10,
    )
    .expect("plan")
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
        first.request(),
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
        second.request(),
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
        delivery.request().target_set().targets()[0].clone(),
        DeliveryOutcome::accepted(),
    );
    let sink_failure = SinkFailure::for_request(
        delivery.request(),
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
        any.request().target_set().targets()[0].clone(),
        DeliveryOutcome::accepted(),
    );
    let failure = SinkFailure::for_request(
        any.request(),
        "terminal_batch_failure",
        Retryability::Terminal,
        None,
        None,
        vec![partial],
    )
    .expect("terminal failure");
    let other_request = DeliveryRequest::new(
        "other-request",
        any.request().payload().clone(),
        any.request().target_set().clone(),
        any.request().satisfaction().clone(),
        any.request().deadline_unix_ms(),
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
        all.request(),
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
        base.request(),
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
