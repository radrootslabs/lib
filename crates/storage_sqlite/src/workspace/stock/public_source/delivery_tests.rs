//! Real stock D transactions and retained normalized reports, without network IO.
use super::*;
use radroots_storage::{
    command::{DeliveryIntent, DeliveryObservationKind},
    inventory::{
        StockDeliveryAttempt, StockDeliveryAttemptOutcome, StockDeliveryObservation,
        StockDeliveryObservationOutcome, StockDeliveryObservationReceipt, StockDeliveryRequest,
    },
};
use radroots_transport::{
    Target, TargetSet,
    outcome::DeliveryOutcome,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::DeliveryTargetReceipt,
};
#[path = "delivery_boundaries_tests.rs"]
mod boundaries;

fn intent(outer: &PrivateEnvelopeCandidate, request: u8) -> DeliveryIntent {
    DeliveryIntent::new(
        &format!("{request:064x}"),
        outer.event_id(),
        &TargetSet::new(vec![Target::nostr_relay("wss://relay.example").unwrap()]).unwrap(),
        &SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        NonZeroU64::new(180000).unwrap(),
    )
    .unwrap()
}
pub(super) async fn request(
    r: &Rig,
    original: &StockAcceptanceReceipt,
    outer: &PrivateEnvelopeCandidate,
    number: u8,
) -> StockDeliveryRequest {
    StockDeliveryRequest::new(
        r.binding.clone(),
        inventory_minimum(r).await,
        original.attempt().operation_id(),
        original.attempt().command_digest(),
        outer.recipient(),
        intent(outer, number),
    )
    .unwrap()
}
pub(super) async fn claim(
    r: &Rig,
    original: &StockAcceptanceReceipt,
    outer: &PrivateEnvelopeCandidate,
    number: u8,
) -> Box<StockDeliveryAttempt> {
    let outcome = r
        .store
        .prepare_stock_delivery_attempt(request(r, original, outer, number).await, proof(r))
        .await
        .unwrap();
    let StockDeliveryAttemptOutcome::Claimed(value) = outcome else {
        panic!("only a newly confirmed D can qualify one potential invocation")
    };
    value
}
pub(super) fn observed(
    attempt: &StockDeliveryAttempt,
    outcome: DeliveryOutcome,
    at: Option<NonZeroU64>,
) -> StockDeliveryObservation {
    StockDeliveryObservation::from_store_report(
        attempt,
        &DeliveryTargetReceipt::attempted(attempt.intent().targets().targets()[0].clone(), outcome),
        at,
    )
    .unwrap()
}
pub(super) async fn save(
    r: &Rig,
    observation: &StockDeliveryObservation,
) -> StockDeliveryObservationReceipt {
    let StockDeliveryObservationOutcome::Committed(receipt) = r
        .store
        .record_stock_delivery_observation(&r.binding, r.enrollment.proof(), observation)
        .await
        .unwrap()
    else {
        panic!("confirmed original observation required")
    };
    *receipt
}

#[tokio::test]
async fn fresh_attempt_existing_request_and_reopen_keep_original_outer_and_stock_effects() {
    let (mut r, original, _, outer) = setup(17).await;
    committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let first = claim(&r, &original, &outer, 1).await;
    assert_eq!(first.target_ordinals(), &[1]);
    first
        .source()
        .with_json(|raw| assert_eq!(raw, outer.raw_json()));
    let before = inventory_minimum(&r).await;
    let StockDeliveryAttemptOutcome::Retained(existing) = r
        .store
        .prepare_stock_delivery_attempt(request(&r, &original, &outer, 1).await, proof(&r))
        .await
        .unwrap()
    else {
        panic!("existing request must never qualify another invocation")
    };
    assert_eq!(existing.transaction_id(), first.transaction_id());
    assert_eq!(inventory_minimum(&r).await, before);
    reopen_plain(&mut r).await;
    let loaded = r
        .store
        .stock_delivery_attempt(
            envelope_request(&r, &original, outer.recipient(), None).await,
            first.intent().request_id().clone(),
            proof(&r),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.transaction_id(), first.transaction_id());
    assert_eq!(loaded.checkpoint(), first.checkpoint());
    assert_eq!(loaded.intent(), first.intent());
    loaded
        .source()
        .with_json(|raw| assert_eq!(raw, outer.raw_json()));
    let summary = r
        .store
        .stock_delivery_summary(
            envelope_request(&r, &original, outer.recipient(), None).await,
            proof(&r),
        )
        .await
        .unwrap();
    assert_eq!(summary.attempt_count(), 1);
    assert_eq!(summary.targets()[0].potential_attempts(), 1);
    assert!(!summary.acceptance_satisfied().unwrap());
    balances(&r, 80, 9, 0, 1, 3).await;
    balances(&r, 81, 8, 0, 2, 3).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn first_null_observation_is_immutable_other_classes_append_and_acceptance_is_sticky() {
    let (mut r, original, _, outer) = setup(17).await;

    committed_envelope(capture(&r, &original, &outer, None).await.unwrap());

    let attempt = claim(&r, &original, &outer, 2).await;

    let accepted = observed(&attempt, DeliveryOutcome::accepted(), None);
    let first = save(&r, &accepted).await;

    let again = observed(
        &attempt,
        DeliveryOutcome::accepted(),
        NonZeroU64::new(151000),
    );
    assert_eq!(save(&r, &again).await, first);
    let later = observed(
        &attempt,
        DeliveryOutcome::unavailable(),
        NonZeroU64::new(152000),
    );
    let second = save(&r, &later).await;
    assert!(second.checkpoint().revision() > first.checkpoint().revision());
    reopen_plain(&mut r).await;
    let page = r
        .store
        .stock_delivery_observations(
            envelope_request(&r, &original, outer.recipient(), None).await,
            None,
            proof(&r),
        )
        .await
        .unwrap();
    assert_eq!(page.entries(), &[first, second]);
    assert!(page.next_after().is_none());
    assert!(
        page.entries()[0]
            .observation()
            .observed_at_unix_ms()
            .is_none()
    );
    let summary = r
        .store
        .stock_delivery_summary(
            envelope_request(&r, &original, outer.recipient(), None).await,
            proof(&r),
        )
        .await
        .unwrap();
    assert!(summary.acceptance_satisfied().unwrap());
    assert!(summary.targets()[0].accepted());
    assert_eq!(
        summary.targets()[0].last_observation_kind(),
        Some(DeliveryObservationKind::Unknown)
    );
    balances(&r, 80, 9, 0, 1, 3).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn obtained_outcomes_survive_clock_loss_expiry_and_revocation_without_authorizing_reads() {
    for mode in ["clock", "expired", "revoked"] {
        let (r, original, _, outer) = setup(17).await;
        committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
        let attempt = claim(&r, &original, &outer, 3).await;
        let observation = observed(&attempt, DeliveryOutcome::accepted(), None);
        let read = envelope_request(&r, &original, outer.recipient(), None).await;
        match mode {
            "clock" => r.clock.0.store(0, Ordering::SeqCst),
            "expired" => r.clock.0.store(350, Ordering::SeqCst),
            "revoked" => {
                r.store
                    .revoke_stock_grant(
                        r.binding.clone(),
                        inventory_minimum(&r).await,
                        r.enrollment.proof(),
                        radroots_domain::TransactionId::from_bytes([99; 32]),
                        r.enrollment.input().grant_id(),
                    )
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let retained = save(&r, &observation).await;
        assert!(retained.observation().observed_at_unix_ms().is_none());
        assert!(
            r.store
                .stock_delivery_summary(read, proof(&r))
                .await
                .is_err()
        );
        r.clock.0.store(150, Ordering::SeqCst);
        if mode != "revoked" {
            let page = r
                .store
                .stock_delivery_observations(
                    envelope_request(&r, &original, outer.recipient(), None).await,
                    None,
                    proof(&r),
                )
                .await
                .unwrap();
            assert_eq!(page.entries(), &[retained]);
        }
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn wrong_route_event_request_and_target_cannot_create_delivery_or_outcome_records() {
    use radroots_transport::{DeliveryReceipt, sink::DeliveryRequest};
    let (r, original, _, outer) = setup(17).await;
    committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let before = inventory_minimum(&r).await;
    let route = DeliveryIntent::new(
        &format!("{:064x}", 4),
        outer.event_id(),
        &TargetSet::new(vec![Target::nostr_relay("wss://other.example").unwrap()]).unwrap(),
        &SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        NonZeroU64::new(180000).unwrap(),
    )
    .unwrap();
    let req = StockDeliveryRequest::new(
        r.binding.clone(),
        before,
        original.attempt().operation_id(),
        original.attempt().command_digest(),
        outer.recipient(),
        route,
    )
    .unwrap();
    assert!(matches!(
        r.store.prepare_stock_delivery_attempt(req, proof(&r)).await,
        Err(OperationError::UnverifiedApproval)
    ));
    let wrong_event = DeliveryIntent::new(
        &format!("{:064x}", 4),
        radroots_domain::EnvelopeEventId::from_bytes([9; 32]),
        intent(&outer, 4).targets(),
        intent(&outer, 4).policy(),
        NonZeroU64::new(180000).unwrap(),
    )
    .unwrap();
    let req = StockDeliveryRequest::new(
        r.binding.clone(),
        before,
        original.attempt().operation_id(),
        original.attempt().command_digest(),
        outer.recipient(),
        wrong_event,
    )
    .unwrap();
    assert!(matches!(
        r.store.prepare_stock_delivery_attempt(req, proof(&r)).await,
        Err(OperationError::InvalidEvidence)
    ));
    assert_eq!(inventory_minimum(&r).await, before);
    let attempt = claim(&r, &original, &outer, 4).await;
    let actual = attempt.transport_request().unwrap();
    let wrong_request = DeliveryRequest::new(
        format!("{:064x}", 5),
        actual.payload().clone(),
        actual.target_set().clone(),
        actual.satisfaction().clone(),
        actual.deadline_unix_ms(),
    )
    .unwrap();
    let receipt = DeliveryReceipt::for_request(
        &wrong_request,
        vec![DeliveryTargetReceipt::attempted(
            actual.target_set().targets()[0].clone(),
            DeliveryOutcome::accepted(),
        )],
    )
    .unwrap();
    assert!(matches!(
        StockDeliveryObservation::from_receipt(
            &attempt,
            &receipt,
            actual.target_set().targets()[0].fingerprint(),
            None
        ),
        Err(OperationError::InvalidEvidence)
    ));
    let foreign = DeliveryTargetReceipt::attempted(
        Target::nostr_relay("wss://other.example").unwrap(),
        DeliveryOutcome::accepted(),
    );
    assert!(matches!(
        StockDeliveryObservation::from_store_report(&attempt, &foreign, None),
        Err(OperationError::InvalidEvidence)
    ));
    let page = r
        .store
        .stock_delivery_observations(
            envelope_request(&r, &original, outer.recipient(), None).await,
            None,
            proof(&r),
        )
        .await
        .unwrap();
    assert!(page.entries().is_empty());
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn failed_attempt_and_observation_writes_roll_back_before_same_request_can_be_confirmed() {
    for (attempt_fault, observation_fault) in [
        (
            "CREATE TRIGGER fault BEFORE INSERT ON stock_operations BEGIN SELECT RAISE(ABORT,'fixture'); END",
            "CREATE TRIGGER fault BEFORE INSERT ON stock_operations BEGIN SELECT RAISE(ABORT,'fixture'); END",
        ),
        (
            "CREATE TRIGGER fault BEFORE INSERT ON stock_operation_parts BEGIN SELECT RAISE(ABORT,'fixture'); END",
            "CREATE TRIGGER fault BEFORE INSERT ON stock_operation_parts BEGIN SELECT RAISE(ABORT,'fixture'); END",
        ),
        (
            "CREATE TRIGGER fault BEFORE INSERT ON stock_delivery_attempts BEGIN SELECT RAISE(ABORT,'fixture'); END",
            "CREATE TRIGGER fault BEFORE INSERT ON stock_delivery_observations BEGIN SELECT RAISE(ABORT,'fixture'); END",
        ),
        (
            "CREATE TRIGGER fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'fixture'); END",
            "CREATE TRIGGER fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'fixture'); END",
        ),
    ] {
        let (r, original, _, outer) = setup(17).await;
        committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
        let before = inventory_minimum(&r).await;
        let mut db = database(&r).await;
        sqlx::raw_sql(attempt_fault).execute(&mut db).await.unwrap();
        assert_eq!(
            r.store
                .prepare_stock_delivery_attempt(request(&r, &original, &outer, 6).await, proof(&r))
                .await
                .unwrap_err()
                .code(),
            "storage_failure"
        );
        assert_eq!(inventory_minimum(&r).await, before);
        sqlx::raw_sql("DROP TRIGGER fault")
            .execute(&mut db)
            .await
            .unwrap();
        let attempt = claim(&r, &original, &outer, 6).await;
        let observation = observed(&attempt, DeliveryOutcome::accepted(), None);
        let before = inventory_minimum(&r).await;
        sqlx::raw_sql(observation_fault)
            .execute(&mut db)
            .await
            .unwrap();
        assert_eq!(
            r.store
                .record_stock_delivery_observation(&r.binding, r.enrollment.proof(), &observation)
                .await
                .unwrap_err()
                .code(),
            "storage_failure"
        );
        assert_eq!(inventory_minimum(&r).await, before);
        sqlx::raw_sql("DROP TRIGGER fault")
            .execute(&mut db)
            .await
            .unwrap();
        save(&r, &observation).await;
        balances(&r, 80, 9, 0, 1, 3).await;
        db.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn unknown_observation_commit_keeps_original_owner_until_same_owner_reconciliation() {
    let (r, original, _, outer) = setup(17).await;
    committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let attempt = claim(&r, &original, &outer, 7).await;
    let observation = observed(&attempt, DeliveryOutcome::accepted(), None);
    let mut db = database(&r).await;
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_delivery_observations BEGIN INSERT INTO deferred_fault VALUES(2); END").execute(&mut db).await.unwrap();
    assert!(matches!(
        r.store
            .record_stock_delivery_observation(&r.binding, r.enrollment.proof(), &observation)
            .await
            .unwrap(),
        StockDeliveryObservationOutcome::Unknown(_)
    ));
    let source = StockEnvelopeRequest::new(
        r.binding.clone(),
        attempt.checkpoint(),
        original.attempt().operation_id(),
        original.attempt().command_digest(),
        outer.recipient(),
        None,
    )
    .unwrap();
    assert_eq!(
        r.store
            .stock_delivery_summary(source, proof(&r))
            .await
            .unwrap_err()
            .code(),
        "recovery_required"
    );
    assert!(
        NativeWorkspace::open_seller_stock(
            &r.path,
            r.enrollment.input().clone(),
            r.fixture.crypto(),
            r.clock.clone()
        )
        .await
        .is_err()
    );
    sqlx::raw_sql("DROP TRIGGER fault; DROP TABLE deferred_fault")
        .execute(&mut db)
        .await
        .unwrap();
    let StockDeliveryObservationOutcome::Committed(retained) = r
        .store
        .reconcile_stock_delivery_observation(&r.binding, r.enrollment.proof(), &observation)
        .await
        .unwrap()
    else {
        panic!("same owner must reconcile obtained output")
    };
    assert_eq!(retained.observation(), &observation);
    let page = r
        .store
        .stock_delivery_observations(
            envelope_request(&r, &original, outer.recipient(), None).await,
            None,
            proof(&r),
        )
        .await
        .unwrap();
    assert_eq!(page.entries(), &[*retained]);
    db.close().await.unwrap();
    r.store.close().await.unwrap();
}
