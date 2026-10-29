//! Delivery edges of the private stock process-death harness.
use super::*;
use radroots_storage::{
    command::{DeliveryIntent, PrivateEnvelopeCandidate},
    inventory::*,
};
use radroots_transport::{
    Target, TargetSet,
    outcome::DeliveryOutcome,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::DeliveryTargetReceipt,
};

fn candidate(c: &Control) -> PrivateEnvelopeCandidate {
    PrivateEnvelopeCandidate::from_obtained(c.recipient, &c.raw, &c.wrapping_key.unwrap()).unwrap()
}
fn intent(c: &Control) -> DeliveryIntent {
    DeliveryIntent::new(
        &format!("{:064x}", 23),
        candidate(c).event_id(),
        &TargetSet::new(vec![Target::nostr_relay("wss://relay.example").unwrap()]).unwrap(),
        &SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        std::num::NonZeroU64::new(180000).unwrap(),
    )
    .unwrap()
}
fn source(
    binding: radroots_storage::workspace::WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    c: &Control,
) -> StockEnvelopeRequest {
    StockEnvelopeRequest::new(binding, minimum, c.operation, c.digest, c.recipient, None).unwrap()
}
fn request(
    binding: radroots_storage::workspace::WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    c: &Control,
) -> StockDeliveryRequest {
    StockDeliveryRequest::new(
        binding,
        minimum,
        c.operation,
        c.digest,
        c.recipient,
        intent(c),
    )
    .unwrap()
}
fn observation(
    attempt: &StockDeliveryAttempt,
    outcome: DeliveryOutcome,
) -> StockDeliveryObservation {
    StockDeliveryObservation::from_store_report(
        attempt,
        &DeliveryTargetReceipt::attempted(attempt.intent().targets().targets()[0].clone(), outcome),
        None,
    )
    .unwrap()
}
pub(super) async fn prepare(r: &Rig, c: &Control) -> Option<TransactionId> {
    assert!(matches!(
        r.store
            .capture_stock_envelope(
                source(r.binding.clone(), inventory_minimum(r).await, c),
                r.enrollment
                    .proof()
                    .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                    .unwrap(),
                &candidate(c),
            )
            .await
            .unwrap(),
        StockEnvelopeOutcome::Committed(_)
    ));
    if matches!(c.edge, Edge::Observation) {
        let StockDeliveryAttemptOutcome::Claimed(attempt) = r
            .store
            .prepare_stock_delivery_attempt(
                request(r.binding.clone(), inventory_minimum(r).await, c),
                r.enrollment
                    .proof()
                    .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                    .unwrap(),
            )
            .await
            .unwrap()
        else {
            panic!("new confirmed D required")
        };
        Some(attempt.transaction_id())
    } else {
        None
    }
}
pub(super) async fn apply(
    store: &NativeWorkspace,
    binding: radroots_storage::workspace::WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    proof: &AttachmentProof,
    c: &Control,
) -> TransactionId {
    if matches!(c.edge, Edge::Delivery) {
        let StockDeliveryAttemptOutcome::Claimed(attempt) = store
            .prepare_stock_delivery_attempt(
                request(binding, minimum, c),
                proof
                    .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                    .unwrap(),
            )
            .await
            .unwrap()
        else {
            panic!("new confirmed D required")
        };
        attempt.transaction_id()
    } else {
        let attempt = store
            .stock_delivery_attempt(
                source(binding.clone(), minimum, c),
                intent(c).request_id().clone(),
                proof
                    .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                    .unwrap(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(Some(attempt.transaction_id()), c.delivery_transaction);
        let StockDeliveryObservationOutcome::Committed(saved) = store
            .record_stock_delivery_observation(
                &binding,
                proof,
                &observation(&attempt, DeliveryOutcome::accepted()),
            )
            .await
            .unwrap()
        else {
            panic!("confirmed original observation required")
        };
        saved.transaction_id()
    }
}
pub(super) async fn verify(r: &Rig, c: &Control, transaction: Option<TransactionId>) {
    let minimum = inventory_minimum(r).await;
    let outer = r
        .store
        .stock_envelope(
            source(r.binding.clone(), minimum, c),
            r.enrollment
                .proof()
                .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                .unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    outer.with_json(|raw| assert_eq!(raw, c.raw));
    let loaded = r
        .store
        .stock_delivery_attempt(
            source(r.binding.clone(), minimum, c),
            intent(c).request_id().clone(),
            r.enrollment
                .proof()
                .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        loaded.is_some(),
        matches!(c.edge, Edge::Observation) || !c.before
    );
    assert_eq!(transaction.is_some(), !c.before);
    let has_attempt = loaded.is_some();
    if let Some(attempt) = loaded {
        attempt.source().with_json(|raw| assert_eq!(raw, c.raw));
        assert_eq!(attempt.intent(), &intent(c));
        assert_eq!(
            Some(attempt.transaction_id()),
            if matches!(c.edge, Edge::Delivery) {
                transaction
            } else {
                c.delivery_transaction
            }
        );
        // Recovery queries and exact existing D requests grant no new invocation.
        let StockDeliveryAttemptOutcome::Retained(repeated) = r
            .store
            .prepare_stock_delivery_attempt(
                request(r.binding.clone(), minimum, c),
                r.enrollment
                    .proof()
                    .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                    .unwrap(),
            )
            .await
            .unwrap()
        else {
            panic!("reopened D must never grant dispatch")
        };
        assert_eq!(repeated.transaction_id(), attempt.transaction_id());
        let page = r
            .store
            .stock_delivery_observations(
                source(r.binding.clone(), minimum, c),
                None,
                r.enrollment
                    .proof()
                    .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(page.next_after().is_none());
        let confirmed = matches!(c.edge, Edge::Observation) && !c.before;
        assert_eq!(page.entries().len(), usize::from(confirmed));
        if confirmed {
            let original = &page.entries()[0];
            assert_eq!(Some(original.transaction_id()), transaction);
            assert_eq!(
                original.observation(),
                &observation(&attempt, DeliveryOutcome::accepted())
            );
            assert!(original.observation().observed_at_unix_ms().is_none());
            let StockDeliveryObservationOutcome::Committed(repeated) = r
                .store
                .record_stock_delivery_observation(
                    &r.binding,
                    r.enrollment.proof(),
                    original.observation(),
                )
                .await
                .unwrap()
            else {
                panic!("original observation must remain confirmed")
            };
            assert_eq!(&*repeated, original);
            // A later class is independent evidence; it cannot replace accepted.
            let StockDeliveryObservationOutcome::Committed(later) = r
                .store
                .record_stock_delivery_observation(
                    &r.binding,
                    r.enrollment.proof(),
                    &observation(&attempt, DeliveryOutcome::unavailable()),
                )
                .await
                .unwrap()
            else {
                panic!("independent class must append")
            };
            assert_ne!(later.transaction_id(), original.transaction_id());
            let summary = r
                .store
                .stock_delivery_summary(
                    source(r.binding.clone(), minimum, c),
                    r.enrollment
                        .proof()
                        .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert!(summary.acceptance_satisfied().unwrap());
        } else {
            assert_eq!(inventory_minimum(r).await, minimum);
        }
    } else {
        assert_eq!(inventory_minimum(r).await, minimum);
    }
    let summary = r
        .store
        .stock_delivery_summary(
            source(r.binding.clone(), minimum, c),
            r.enrollment
                .proof()
                .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(summary.attempt_count(), u16::from(has_attempt));
    assert_eq!(
        summary.acceptance_satisfied().unwrap(),
        matches!(c.edge, Edge::Observation) && !c.before
    );
    if has_attempt {
        assert_eq!(summary.targets()[0].potential_attempts(), 1);
    }
    balances(r, 80, 9, 0, 1, 3).await;
    balances(r, 81, 8, 0, 2, 3).await;
}
