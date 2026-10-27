//! Untrusted data projections must not bypass the retained native graph.
use super::*;
use radroots_domain::TransactionId;
use radroots_storage::command::{
    DeliveryAttemptSummary, OperationSnapshot, PrivateEnvelopeCandidate,
};
use radroots_transport::{
    Target, TransportId, outcome::DeliveryOutcome, sink::DeliveryTargetReceipt, target::TargetScope,
};

fn targets(urls: &[&str]) -> TargetSet {
    TargetSet::new(
        urls.iter()
            .map(|url| Target::nostr_relay(url).unwrap())
            .collect(),
    )
    .unwrap()
}
fn intent(set: &TargetSet, policy: TargetPolicy) -> Result<DeliveryIntent, OperationError> {
    DeliveryIntent::new(
        &"01".repeat(32),
        EnvelopeEventId::from_bytes([3; 32]),
        set,
        &SatisfactionPolicy::new(SatisfactionClass::Accepted, policy),
        NonZeroU64::new(1_000).unwrap(),
    )
}

#[test]
fn delivery_intent_bounds_and_selection_preserve_the_full_publication_criterion() {
    let set = targets(&["wss://a.example", "wss://b.example"]);
    let original = intent(&set, TargetPolicy::all()).unwrap();
    for id in [
        String::new(),
        "01".repeat(31),
        "AA".repeat(32),
        "gg".repeat(32),
    ] {
        assert_eq!(
            DeliveryIntent::new(
                &id,
                original.event_id(),
                &set,
                original.policy(),
                original.deadline_unix_ms()
            ),
            Err(OperationError::InvalidCommand)
        );
    }
    let too_many = TargetSet::new(
        (0..17)
            .map(|i| Target::nostr_relay(format!("wss://r{i}.example")).unwrap())
            .collect(),
    )
    .unwrap();
    let duplicate_endpoint = TargetSet::new(vec![
        Target::nostr_relay_with_metadata(
            "wss://a.example",
            Some(TargetScope::parse("first").unwrap()),
            None,
        )
        .unwrap(),
        Target::nostr_relay_with_metadata(
            "wss://a.example/",
            Some(TargetScope::parse("second").unwrap()),
            None,
        )
        .unwrap(),
    ])
    .unwrap();
    let other_transport = TargetSet::new(vec![
        Target::new(TransportId::LOCAL, "test://endpoint").unwrap(),
    ])
    .unwrap();
    assert!(TargetSet::new(vec![]).is_err());
    for rejected in [too_many, duplicate_endpoint, other_transport] {
        assert_eq!(
            intent(&rejected, TargetPolicy::all()),
            Err(OperationError::InvalidCommand)
        );
    }
    assert!(
        DeliveryIntent::new(
            original.request_id().as_str(),
            original.event_id(),
            &set,
            original.policy(),
            NonZeroU64::new(253_402_300_800_000).unwrap()
        )
        .is_err()
    );
    assert!(
        DeliveryIntent::new(
            original.request_id().as_str(),
            original.event_id(),
            &set,
            &SatisfactionPolicy::new(SatisfactionClass::Delivered, TargetPolicy::all()),
            original.deadline_unix_ms()
        )
        .is_err()
    );
    assert!(intent(&set, TargetPolicy::quorum(3).unwrap()).is_err());
    let foreign = targets(&["wss://foreign.example"]);
    assert!(
        intent(
            &set,
            TargetPolicy::required(vec![foreign.targets()[0].fingerprint().clone()]).unwrap()
        )
        .is_err()
    );
    assert!(original.clone().with_selection(&foreign).is_err());
    let selected = original
        .clone()
        .with_selection(&TargetSet::new(vec![set.targets()[1].clone()]).unwrap())
        .unwrap();
    assert_eq!(selected.targets(), original.targets());
    assert_eq!(
        selected.publication_policy_digest(),
        original.publication_policy_digest()
    );
    assert_ne!(selected.digest(), original.digest());
    assert_eq!(
        selected.selected_target_set().unwrap().targets(),
        &[set.targets()[1].clone()]
    );
    assert!(intent(&set, TargetPolicy::any()).is_ok());
    assert!(intent(&set, TargetPolicy::quorum(2).unwrap()).is_ok());
}

fn outer_variant(ready: &Ready, kind: u16, time: u64, recipient: Option<u8>) -> String {
    let original = nostr::Event::from_json(ready.candidate.raw_json()).unwrap();
    let signer = keys(3);
    nostr::UnsignedEvent {
        id: None,
        pubkey: signer.public_key(),
        created_at: nostr::Timestamp::from_secs(time),
        kind: nostr::Kind::Custom(kind),
        tags: nostr::Tags::from_list(
            recipient
                .map(|id| nostr::Tag::public_key(keys(id).public_key()))
                .into_iter()
                .collect(),
        ),
        content: original.content,
    }
    .sign_with_keys(&signer)
    .unwrap()
    .as_json()
}
fn report(
    original: &DeliveryAttempt,
    intent: DeliveryIntent,
    raw: &str,
    at: UnixSeconds,
    ordinals: Vec<u8>,
) -> Result<DeliveryAttempt, OperationError> {
    DeliveryAttempt::from_store_report(
        original.work(),
        intent,
        raw,
        TransactionId::from_bytes([88; 32]),
        at,
        ordinals,
    )
}
fn observation(attempt: &DeliveryAttempt, outcome: DeliveryOutcome) -> DeliveryObservation {
    DeliveryObservation::from_store_report(
        attempt,
        &DeliveryTargetReceipt::attempted(
            attempt.intent().selected_targets().next().unwrap().clone(),
            outcome,
        ),
        None,
    )
    .unwrap()
}
async fn snapshot(ready: &Ready) -> OperationSnapshot {
    ready
        .store
        .operation_snapshot(
            ready.binding.clone(),
            ready.work.evidence().operation_id(),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn work_independent_envelope_binds_original_recipient_and_retains_exact_native_bytes() {
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 245).await;
    let obtained = ready.candidate.with_wrapping_key(|key| {
        PrivateEnvelopeCandidate::from_obtained(
            ready.work.recipient(),
            ready.candidate.raw_json(),
            key,
        )
        .unwrap()
    });
    let copied = obtained.retained_copy();
    drop(obtained);
    let bound = EnvelopeCandidate::from_private(ready.work, copied).unwrap();
    let DeliveryAttemptOutcome::Claimed(retained) = ready
        .store
        .prepare_delivery_attempt(ready.request(1), copy(&ready.proof), Some(&bound))
        .await
        .unwrap()
    else {
        panic!("actual original envelope must be retained before transport");
    };
    assert_eq!(retained.raw_json(), ready.candidate.raw_json());
    assert_eq!(retained.intent().event_id(), bound.event_id());
    assert_eq!(
        ready
            .query(&ready.store, retained.intent().request_id())
            .await
            .raw_json(),
        bound.raw_json()
    );
    assert_ne!(ready.work.recipient(), key());
    let foreign = PrivateEnvelopeCandidate::from_obtained(
        key(),
        &outer_variant(&ready, 1059, retained.retained_at().get(), Some(1)),
        &[3; 32],
    )
    .unwrap();
    assert!(matches!(
        EnvelopeCandidate::from_private(ready.work, foreign),
        Err(OperationError::InvalidEvidence)
    ));
    ready.store.close().await.unwrap();
}

#[tokio::test]
async fn malformed_carriers_and_reconstructed_attempts_cannot_replace_original_native_bytes() {
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 246).await;
    let retained = ready.retain(1).await;
    let wrapping_key = [3; 32];
    let time = retained.retained_at().get();
    for raw in [
        " ".repeat(65_537),
        outer_variant(&ready, 14, time, Some(2)),
        outer_variant(&ready, 1059, 0, Some(2)),
        outer_variant(&ready, 1059, 253_402_300_800, Some(2)),
        outer_variant(&ready, 1059, time, None),
        outer_variant(&ready, 1059, time, Some(1)),
    ] {
        assert!(matches!(
            PrivateEnvelopeCandidate::from_obtained(ready.work.recipient(), &raw, &wrapping_key),
            Err(OperationError::InvalidEvidence)
        ));
        assert!(matches!(
            EnvelopeCandidate::from_obtained(ready.work, &raw, &wrapping_key),
            Err(OperationError::InvalidEvidence)
        ));
    }
    for field in ["id", "sig"] {
        let mut wire: serde_json::Value = serde_json::from_str(ready.candidate.raw_json()).unwrap();
        wire[field] = "0".repeat(if field == "id" { 64 } else { 128 }).into();
        assert!(
            EnvelopeCandidate::from_obtained(ready.work, &wire.to_string(), &wrapping_key).is_err()
        );
    }
    assert_eq!(ready.candidate.work(), ready.work);
    assert_eq!(
        ready.candidate.created_at().get(),
        nostr::Event::from_json(ready.candidate.raw_json())
            .unwrap()
            .created_at
            .as_secs()
    );
    let mismatched = DeliveryIntent::new(
        retained.intent().request_id().as_str(),
        EnvelopeEventId::from_bytes([44; 32]),
        retained.intent().targets(),
        retained.intent().policy(),
        retained.intent().deadline_unix_ms(),
    )
    .unwrap();
    assert!(
        report(
            &retained,
            mismatched,
            retained.raw_json(),
            retained.retained_at(),
            vec![1]
        )
        .is_err()
    );
    assert!(
        report(
            &retained,
            retained.intent().clone(),
            retained.raw_json(),
            UnixSeconds::new(ready.work.queued_at().get() - 1).unwrap(),
            vec![1]
        )
        .is_err()
    );
    for ordinals in [vec![], vec![0], vec![13]] {
        assert!(
            report(
                &retained,
                retained.intent().clone(),
                retained.raw_json(),
                retained.retained_at(),
                ordinals
            )
            .is_err()
        );
    }
    let expired = DeliveryIntent::new(
        retained.intent().request_id().as_str(),
        retained.intent().event_id(),
        retained.intent().targets(),
        retained.intent().policy(),
        NonZeroU64::new(time * 1000).unwrap(),
    )
    .unwrap();
    assert!(
        report(
            &retained,
            expired,
            retained.raw_json(),
            retained.retained_at(),
            vec![1]
        )
        .is_err()
    );
    let future = outer_variant(&ready, 1059, time + 1, Some(2));
    let future_candidate =
        EnvelopeCandidate::from_obtained(ready.work, &future, &wrapping_key).unwrap();
    let future_intent = DeliveryIntent::new(
        retained.intent().request_id().as_str(),
        future_candidate.event_id(),
        retained.intent().targets(),
        retained.intent().policy(),
        retained.intent().deadline_unix_ms(),
    )
    .unwrap();
    assert!(
        report(
            &retained,
            future_intent,
            &future,
            retained.retained_at(),
            vec![1]
        )
        .is_err()
    );
    assert_eq!(
        ready
            .query(&ready.store, retained.intent().request_id())
            .await
            .raw_json(),
        retained.raw_json()
    );
    ready.store.close().await.unwrap();
}

#[tokio::test]
async fn delivery_projections_reject_foreign_payloads_targets_ordinals_and_duplicate_facts() {
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 247).await;
    let retained = ready.retain(1).await;
    let summary = DeliveryAttemptSummary::from_attempt(&retained);
    let accepted = observation(&retained, DeliveryOutcome::accepted());
    assert!(!summary.acceptance_satisfied(&[]).unwrap());
    assert!(
        summary
            .acceptance_satisfied(std::slice::from_ref(&accepted))
            .unwrap()
    );
    assert_eq!(summary.work(), retained.work());
    assert_eq!(summary.transaction_id(), retained.transaction_id());
    assert_eq!(summary.retained_at(), retained.retained_at());
    assert_eq!(summary.target_ordinals(), retained.target_ordinals());
    let foreign = Target::nostr_relay("wss://foreign.example").unwrap();
    assert!(
        DeliveryObservation::from_store_report(
            &retained,
            &DeliveryTargetReceipt::attempted(foreign, DeliveryOutcome::accepted()),
            None
        )
        .is_err()
    );
    let target = retained.intent().targets().targets()[0].clone();
    assert!(
        DeliveryObservation::from_store_report(
            &retained,
            &DeliveryTargetReceipt::attempted(target.clone(), DeliveryOutcome::delivered()),
            None
        )
        .is_err()
    );
    assert!(
        DeliveryObservation::from_store_report(
            &retained,
            &DeliveryTargetReceipt::attempted(target, DeliveryOutcome::accepted()),
            NonZeroU64::new(253_402_300_800_000)
        )
        .is_err()
    );
    let different_bytes = report(
        &retained,
        retained.intent().clone(),
        &format!("{} ", retained.raw_json()),
        retained.retained_at(),
        vec![1],
    )
    .unwrap();
    let wrong_payload = observation(&different_bytes, DeliveryOutcome::accepted());
    assert!(
        summary
            .acceptance_satisfied(std::slice::from_ref(&wrong_payload))
            .is_err()
    );
    assert!(
        snapshot(&ready)
            .await
            .with_delivery(vec![summary.clone()], vec![wrong_payload])
            .is_err()
    );
    let wrong_ordinal = report(
        &retained,
        retained.intent().clone(),
        retained.raw_json(),
        retained.retained_at(),
        vec![2],
    )
    .unwrap();
    assert!(
        snapshot(&ready)
            .await
            .with_delivery(
                vec![DeliveryAttemptSummary::from_attempt(&wrong_ordinal)],
                vec![]
            )
            .is_err()
    );
    assert!(
        snapshot(&ready)
            .await
            .with_delivery(
                vec![summary.clone()],
                vec![observation(&wrong_ordinal, DeliveryOutcome::accepted())]
            )
            .is_err()
    );
    assert!(
        snapshot(&ready)
            .await
            .with_delivery(vec![summary.clone(), summary.clone()], vec![])
            .is_err()
    );
    assert!(
        snapshot(&ready)
            .await
            .with_delivery(
                vec![summary.clone()],
                vec![accepted.clone(), accepted.clone()]
            )
            .is_err()
    );
    assert!(
        snapshot(&ready)
            .await
            .with_delivery(vec![summary.clone(); 385], vec![])
            .is_err()
    );
    assert!(
        snapshot(&ready)
            .await
            .with_delivery(vec![summary.clone()], vec![accepted.clone(); 2305])
            .is_err()
    );
    assert!(
        snapshot(&ready)
            .await
            .with_installation(None, vec![])
            .is_err()
    );
    let missing = snapshot(&ready)
        .await
        .with_delivery(vec![], vec![])
        .unwrap();
    assert!(!missing.publication_accepted(ready.work).unwrap());
    let projected = snapshot(&ready)
        .await
        .with_delivery(vec![summary], vec![accepted])
        .unwrap();
    assert!(projected.publication_accepted(ready.work).unwrap());
    assert_eq!(projected.delivery_attempts().len(), 1);
    assert!(
        ready
            .observations(&ready.store, retained.intent().request_id())
            .await
            .is_empty()
    );
    ready.store.close().await.unwrap();
}
