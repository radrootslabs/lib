//! Synthetic report shape only; actual stock linkage is a native-store duty.
use super::*;
use radroots_storage::{
    command::{DeliveryIntent, DeliveryObservationKind},
    inventory::{
        StockDeliveryObservationPage, StockDeliveryRequest, StockDeliverySummary,
        StockDeliveryTargetProgress,
    },
};
use radroots_transport::{
    Target, TargetSet,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
};

fn fixture() -> (StockEnvelopeRequest, StockEnvelopeReceipt, DeliveryIntent) {
    let original = attempt(1, 7, 8, true);
    let request = StockEnvelopeRequest::new(
        binding(WorkspaceScope::SellerStockOwner),
        checkpoint(0),
        original.operation_id(),
        original.command_digest(),
        key(),
        None,
    )
    .unwrap();
    let source = StockEnvelopeReceipt::from_committed_store(
        &request,
        original,
        evidence(original, key()),
        TransactionId::from_bytes([12; 32]),
        EnvelopeEventId::from_bytes([14; 32]),
        [15; 32],
        checkpoint(1),
    )
    .unwrap();
    let targets = TargetSet::new(vec![
        Target::nostr_relay("wss://one.example").unwrap(),
        Target::nostr_relay("wss://two.example").unwrap(),
    ])
    .unwrap();
    let intent = DeliveryIntent::new(
        &"a".repeat(64),
        source.event_id(),
        &targets,
        &SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        core::num::NonZeroU64::new(300000).unwrap(),
    )
    .unwrap();
    (request, source, intent)
}
fn progress(intent: &DeliveryIntent, accepted: &[bool; 2]) -> Vec<StockDeliveryTargetProgress> {
    intent
        .targets()
        .targets()
        .iter()
        .zip(accepted)
        .map(|(target, accepted)| {
            StockDeliveryTargetProgress::from_store_report(
                target.fingerprint().clone(),
                1,
                *accepted,
                Some(if *accepted {
                    DeliveryObservationKind::Accepted
                } else {
                    DeliveryObservationKind::Disconnected
                }),
                None,
            )
            .unwrap()
        })
        .collect()
}

#[test]
fn summary_keeps_the_complete_original_transport_criterion_and_never_promotes_capture_only() {
    let (request, source, intent) = fixture();
    let captured = StockDeliverySummary::from_store_report(
        &request,
        Some(source),
        None,
        0,
        vec![],
        checkpoint(3),
    )
    .unwrap();
    assert!(!captured.acceptance_satisfied().unwrap());
    assert!(captured.original_intent().is_none());
    for (accepted, expected) in [([true, false], false), ([true, true], true)] {
        let value = StockDeliverySummary::from_store_report(
            &request,
            Some(source),
            Some(intent.clone()),
            1,
            progress(&intent, &accepted),
            checkpoint(3),
        )
        .unwrap();
        assert_eq!(value.acceptance_satisfied().unwrap(), expected);
        assert_eq!(value.original_intent(), Some(&intent));
        assert_eq!(value.attempt_count(), 1);
        assert_eq!(value.targets().len(), 2);
        assert_eq!(format!("{value:?}"), "StockDeliverySummary(<redacted>)");
    }
}

#[test]
fn endpoint_caps_and_summary_scope_reject_inconsistent_reports() {
    let (request, source, intent) = fixture();
    let fp = intent.targets().targets()[0].fingerprint();
    assert!(
        StockDeliveryTargetProgress::from_store_report(fp.clone(), 13, false, None, None).is_err()
    );
    assert!(
        StockDeliveryTargetProgress::from_store_report(fp.clone(), 0, true, None, None).is_err()
    );
    assert!(
        StockDeliveryTargetProgress::from_store_report(
            fp.clone(),
            1,
            false,
            None,
            core::num::NonZeroU64::new(1)
        )
        .is_err()
    );
    for count in [0, 3, 193] {
        assert!(
            StockDeliverySummary::from_store_report(
                &request,
                Some(source),
                Some(intent.clone()),
                count,
                progress(&intent, &[false, false]),
                checkpoint(3)
            )
            .is_err()
        );
    }
    let mut wrong_order = progress(&intent, &[false, false]);
    wrong_order.reverse();
    assert!(
        StockDeliverySummary::from_store_report(
            &request,
            Some(source),
            Some(intent.clone()),
            1,
            wrong_order,
            checkpoint(3)
        )
        .is_err()
    );
    assert!(
        StockDeliverySummary::from_store_report(
            &request,
            None,
            Some(intent.clone()),
            1,
            progress(&intent, &[false, false]),
            checkpoint(3)
        )
        .is_err()
    );
}

#[test]
fn stock_intent_refuses_public_artifacts_and_empty_outcome_pages_do_not_advance() {
    let (request, source, intent) = fixture();
    let public = DeliveryIntent::public(
        &"b".repeat(64),
        source.event_id(),
        intent.targets(),
        intent.policy(),
        intent.deadline_unix_ms(),
    )
    .unwrap();
    assert!(
        StockDeliveryRequest::new(
            request.binding().clone(),
            checkpoint(0),
            request.operation_id(),
            request.command_digest(),
            request.recipient(),
            public
        )
        .is_err()
    );
    assert!(
        StockDeliveryObservationPage::from_store_report(&request, vec![], true, checkpoint(3))
            .is_err()
    );
    let page =
        StockDeliveryObservationPage::from_store_report(&request, vec![], false, checkpoint(3))
            .unwrap();
    assert!(page.entries().is_empty());
    assert!(page.next_after().is_none());
    assert_eq!(
        format!("{page:?}"),
        "StockDeliveryObservationPage(<redacted>)"
    );
}
