//! Synthetic report shape only; actual stock linkage is a native-store duty.
use super::*;
use radroots_storage::{
    command::{DeliveryIntent, DeliveryObservationKind},
    inventory::{
        StockDeliveryObservationPage, StockDeliveryRequest, StockDeliverySummary,
        StockDeliveryTargetProgress, StockRewrapAdmission, StockRewrapPage, StockRewrapRequest,
        StockRewrapState,
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
fn admission(
    request: &StockEnvelopeRequest,
    original: StockEnvelopeReceipt,
    index: u8,
) -> StockRewrapAdmission {
    StockRewrapAdmission::from_store_report(
        &StockRewrapRequest::new(
            StockEnvelopeRequest::new(
                request.binding().clone(),
                request.minimum_checkpoint(),
                request.operation_id(),
                request.command_digest(),
                request.recipient(),
                None,
            )
            .unwrap(),
            OperationId::from_bytes([index + 1; 32]),
        ),
        original,
        TransactionId::from_bytes([index + 128; 32]),
        radroots_domain::UnixSeconds::new(150).unwrap(),
        radroots_domain::UnixSeconds::new(180).unwrap(),
        checkpoint(u64::from(index) + 2),
    )
    .unwrap()
}
fn rewrapped(
    request: &StockEnvelopeRequest,
    original: StockEnvelopeReceipt,
    index: u8,
) -> StockEnvelopeReceipt {
    let event = EnvelopeEventId::from_bytes([index + 64; 32]);
    let selected = StockEnvelopeRequest::new(
        request.binding().clone(),
        request.minimum_checkpoint(),
        request.operation_id(),
        request.command_digest(),
        request.recipient(),
        None,
    )
    .unwrap()
    .with_envelope(event);
    StockEnvelopeReceipt::from_committed_store(
        &selected,
        radroots_storage::command::CommitAttempt::new(
            original.attempt().workspace_id(),
            original.attempt().operation_id(),
            original.attempt().command_digest(),
            Some(TransactionId::from_bytes([index + 32; 32])),
        ),
        original.evidence(),
        original.installation_transaction_id(),
        event,
        [index + 96; 32],
        checkpoint(u64::from(index) + 3),
    )
    .unwrap()
    .with_rewrap_admission(&admission(request, original, index))
    .unwrap()
}

#[test]
fn selected_envelope_retains_a_separate_first_policy_source_without_weakening_original_reports() {
    let (request, original, intent) = fixture();
    let new = rewrapped(&request, original, 1);
    let selected = StockEnvelopeRequest::new(
        request.binding().clone(),
        request.minimum_checkpoint(),
        request.operation_id(),
        request.command_digest(),
        request.recipient(),
        None,
    )
    .unwrap()
    .with_envelope(new.event_id());
    assert!(
        StockDeliverySummary::from_store_report(
            &selected,
            Some(new),
            Some(intent.clone()),
            1,
            progress(&intent, &[false, false]),
            checkpoint(4)
        )
        .is_err()
    );
    let summary = StockDeliverySummary::from_store_report_with_policy_source(
        &selected,
        Some(new),
        Some(original),
        Some(intent.clone()),
        1,
        1,
        progress(&intent, &[false, false]),
        checkpoint(4),
    )
    .unwrap();
    assert_eq!(summary.source(), Some(new));
    assert_eq!(summary.publication_policy_source(), Some(original));
    assert_eq!(summary.original_intent(), Some(&intent));
    assert!(!summary.acceptance_satisfied().unwrap());
    let wrong_policy = rewrapped(&request, original, 2);
    assert!(
        StockDeliverySummary::from_store_report_with_policy_source(
            &selected,
            Some(new),
            Some(wrong_policy),
            Some(intent.clone()),
            1,
            1,
            progress(&intent, &[false, false]),
            checkpoint(5)
        )
        .is_err()
    );
    assert!(
        StockDeliveryRequest::new(
            request.binding().clone(),
            request.minimum_checkpoint(),
            request.operation_id(),
            request.command_digest(),
            request.recipient(),
            intent
        )
        .unwrap()
        .with_envelope(new.event_id())
        .is_err()
    );
}

#[test]
fn additional_envelope_pages_reject_duplicate_identities_oversized_pages_and_false_cursors() {
    let (request, original, _) = fixture();
    let entries = (0..16)
        .map(|index| {
            StockRewrapState::from_store_report(
                admission(&request, original, index),
                Some(rewrapped(&request, original, index)),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let cursor = entries[15].admission().checkpoint().revision();
    let page = StockRewrapPage::from_store_report(&request, entries.clone(), true, checkpoint(18))
        .unwrap();
    assert_eq!(page.entries(), entries.as_slice());
    assert_eq!(page.next_after(), Some(cursor));
    assert_eq!(page.checkpoint(), checkpoint(18));
    assert!(
        StockRewrapPage::from_store_report(
            &request,
            vec![entries[0], entries[0]],
            false,
            checkpoint(18)
        )
        .is_err()
    );
    assert!(
        StockRewrapPage::from_store_report(
            &request,
            (0..17)
                .map(|index| StockRewrapState::from_store_report(
                    admission(&request, original, index),
                    Some(rewrapped(&request, original, index))
                )
                .unwrap())
                .collect(),
            false,
            checkpoint(19)
        )
        .is_err()
    );
    assert!(
        StockRewrapPage::from_store_report(&request, entries.clone(), false, checkpoint(1))
            .is_err()
    );
    assert!(
        StockRewrapPage::from_store_report(&request, vec![entries[0]], true, checkpoint(18))
            .is_err()
    );
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

#[test]
fn scoped_delivery_counts_refuse_observations_without_a_selected_invocation() {
    let (request, original, intent) = fixture();
    let new = rewrapped(&request, original, 1);
    let selected = StockEnvelopeRequest::new(
        request.binding().clone(),
        request.minimum_checkpoint(),
        request.operation_id(),
        request.command_digest(),
        request.recipient(),
        None,
    )
    .unwrap()
    .with_envelope(new.event_id());
    let none = StockDeliverySummary::from_store_report_with_policy_source(
        &selected,
        Some(new),
        Some(original),
        Some(intent.clone()),
        1,
        0,
        progress(&intent, &[false, false])
            .into_iter()
            .map(|target| {
                StockDeliveryTargetProgress::from_store_report(
                    target.target().clone(),
                    target.potential_attempts(),
                    false,
                    None,
                    None,
                )
                .unwrap()
            })
            .collect(),
        checkpoint(5),
    )
    .unwrap();
    assert_eq!(none.attempt_count(), 1);
    assert_eq!(none.selected_attempt_count(), 0);
    assert!(!none.acceptance_satisfied().unwrap());
    assert!(
        StockDeliverySummary::from_store_report_with_policy_source(
            &selected,
            Some(new),
            Some(original),
            Some(intent.clone()),
            1,
            2,
            progress(&intent, &[false, false]),
            checkpoint(5)
        )
        .is_err()
    );
    assert!(
        StockDeliverySummary::from_store_report_with_policy_source(
            &selected,
            Some(new),
            Some(original),
            Some(intent.clone()),
            1,
            0,
            progress(&intent, &[true, false]),
            checkpoint(5)
        )
        .is_err()
    );
}
