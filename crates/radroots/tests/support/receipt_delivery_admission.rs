use super::*;
use radroots_domain::{EnvelopeEventId, TransactionId};
use radroots_transport::{
    outcome::DeliveryOutcome,
    sink::{DeliveryRequestId, DeliveryTargetReceipt},
};

#[tokio::test]
async fn native_delivery_matches_original_signed_root_slash_route_to_canonical_target() {
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9/")], 250).await;
    let retained = ready.retain(1).await;
    assert_eq!(
        retained.intent().targets().targets()[0].uri().as_str(),
        "ws://127.0.0.1:9"
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
async fn self_copy_delivery_refuses_the_original_peer_as_temporary_author() {
    use radroots_storage::command::ClaimAttemptRequest;
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 251).await;
    let e = ready.work.evidence();
    let original = ready
        .store
        .operation_snapshot(
            ready.binding.clone(),
            e.operation_id(),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    let plan = *original.bound_plan().unwrap().plan().digest();
    let installation = original.installation().unwrap().transaction_id();
    let claim = issuance::claim(
        ready
            .store
            .claim_attempt(
                ClaimAttemptRequest::new(
                    ready.binding.clone(),
                    e.operation_id(),
                    e.command_digest(),
                    plan,
                    key(),
                ),
                copy(&ready.proof),
            )
            .await
            .unwrap(),
    );
    let fields = claim.seal().unwrap().fields();
    let participant = keys(1);
    let signed = nostr::UnsignedEvent {
        id: Some(nostr::EventId::from_slice(&claim.seal().unwrap().event_id()).unwrap()),
        pubkey: participant.public_key(),
        created_at: nostr::Timestamp::from_secs(fields.created_at),
        kind: nostr::Kind::Custom(13),
        tags: nostr::Tags::new(),
        content: fields.content.to_owned(),
    }
    .sign_with_keys(&participant)
    .unwrap();
    let evidence = ArtifactEvidence::verify(&claim, &signed.as_json(), None).unwrap();
    assert!(matches!(
        ready
            .store
            .capture_evidence(ready.input.clone(), &ready.proof, &evidence)
            .await
            .unwrap(),
        EvidenceCaptureOutcome::Committed(_)
    ));
    assert!(matches!(
        ready
            .store
            .install_transition_and_outbox(
                InstallationRequest::new(
                    ready.binding.clone(),
                    e.operation_id(),
                    e.command_digest(),
                    plan
                ),
                copy(&ready.proof)
            )
            .await
            .unwrap(),
        InstallationOutcome::Committed(_)
    ));
    let snapshot = ready
        .store
        .operation_snapshot(
            ready.binding.clone(),
            e.operation_id(),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        snapshot.installation().unwrap().transaction_id(),
        installation
    );
    assert_eq!(snapshot.publications().len(), 2);
    let work = *snapshot
        .publications()
        .iter()
        .find(|work| work.recipient() == key())
        .unwrap();
    let peer = keys(2);
    let content = nostr::nips::nip44::encrypt(
        peer.secret_key(),
        &participant.public_key(),
        evidence.raw_json(),
        nostr::nips::nip44::Version::V2,
    )
    .unwrap();
    let outer = nostr::UnsignedEvent {
        id: None,
        pubkey: peer.public_key(),
        created_at: nostr::Timestamp::from_secs(work.queued_at().get()),
        kind: nostr::Kind::Custom(1059),
        tags: nostr::Tags::from_list(vec![nostr::Tag::public_key(participant.public_key())]),
        content,
    }
    .sign_with_keys(&peer)
    .unwrap();
    let candidate = EnvelopeCandidate::from_obtained(
        work,
        &outer.as_json(),
        &peer.secret_key().to_secret_bytes(),
    )
    .unwrap();
    let targets = ready.request(1).intent().targets().clone();
    let deadline = NonZeroU64::new(work.queued_at().get() * 1000 + 60_000).unwrap();
    let policy = SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all());
    let intent = DeliveryIntent::new(
        &"ee".repeat(32),
        candidate.event_id(),
        &targets,
        &policy,
        deadline,
    )
    .unwrap();
    match ready
        .store
        .prepare_delivery_attempt(
            DeliveryAttemptRequest::new(ready.binding.clone(), work, intent).unwrap(),
            copy(&ready.proof),
            Some(&candidate),
        )
        .await
    {
        Err(OperationError::InvalidEvidence) => {}
        Ok(DeliveryAttemptOutcome::Claimed(_)) => {
            panic!("native D freshly claimed an outer signed by the original peer")
        }
        Err(error) => panic!("unexpected refusal: {}", error.code()),
        Ok(_) => panic!("unexpected retained or unknown native outcome"),
    }
    let wrapped = wrap_retained_seal(
        evidence.raw_json(),
        key(),
        NonZeroU64::new(work.queued_at().get()).unwrap(),
        &OsNonceSource,
    )
    .unwrap();
    let candidate = wrapped.with_wrapping_key(|key| {
        EnvelopeCandidate::from_obtained(work, wrapped.signed_json(), key).unwrap()
    });
    let intent = DeliveryIntent::new(
        &"ef".repeat(32),
        candidate.event_id(),
        &targets,
        &policy,
        deadline,
    )
    .unwrap();
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(
                DeliveryAttemptRequest::new(ready.binding.clone(), work, intent).unwrap(),
                copy(&ready.proof),
                Some(&candidate)
            )
            .await
            .unwrap(),
        DeliveryAttemptOutcome::Claimed(_)
    ));
    ready.store.close().await.unwrap();
}

fn signed_candidate(ready: &Ready, scalar: u8, content: String) -> EnvelopeCandidate {
    let signer = keys(scalar);
    let recipient = nostr::PublicKey::from_slice(ready.work.recipient().as_bytes()).unwrap();
    let event = nostr::UnsignedEvent {
        id: None,
        pubkey: signer.public_key(),
        created_at: nostr::Timestamp::from_secs(ready.work.queued_at().get()),
        kind: nostr::Kind::Custom(1059),
        tags: nostr::Tags::from_list(vec![nostr::Tag::public_key(recipient)]),
        content,
    }
    .sign_with_keys(&signer)
    .unwrap();
    EnvelopeCandidate::from_obtained(
        ready.work,
        &event.as_json(),
        &signer.secret_key().to_secret_bytes(),
    )
    .unwrap()
}
async fn rebound_work(ready: &Ready) -> PublicationWork {
    let snapshot = ready
        .store
        .operation_snapshot(
            ready.binding.clone(),
            ready.work.evidence().operation_id(),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    PublicationWork::from_committed_store(
        snapshot.bound_plan().unwrap(),
        snapshot.installation().unwrap(),
        ready.work.evidence(),
        TransactionId::from_bytes([91; 32]),
        ready.work.queued_at(),
    )
    .unwrap()
}

#[tokio::test]
async fn native_delivery_independently_checks_temporary_keys_exact_seal_and_original_work() {
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 248).await;
    let receiver = nostr::PublicKey::from_slice(ready.work.recipient().as_bytes()).unwrap();
    let temp = keys(3);
    let wrong_plaintext = nostr::nips::nip44::encrypt(
        temp.secret_key(),
        &receiver,
        "another private record",
        nostr::nips::nip44::Version::V2,
    )
    .unwrap();
    let bad_key =
        EnvelopeCandidate::from_obtained(ready.work, ready.candidate.raw_json(), &[1; 32]).unwrap();
    let zero_key =
        EnvelopeCandidate::from_obtained(ready.work, ready.candidate.raw_json(), &[0; 32]).unwrap();
    for candidate in [
        signed_candidate(&ready, 1, String::from("participant outer")),
        signed_candidate(&ready, 2, String::from("recipient outer")),
        signed_candidate(&ready, 3, String::from("invalid cipher")),
        signed_candidate(&ready, 3, wrong_plaintext),
        bad_key,
        zero_key,
    ] {
        assert!(matches!(
            ready
                .store
                .prepare_delivery_attempt(ready.request(1), copy(&ready.proof), Some(&candidate))
                .await,
            Err(OperationError::InvalidEvidence)
        ));
    }
    let work = rebound_work(&ready).await;
    let request = DeliveryAttemptRequest::new(
        ready.binding.clone(),
        work,
        ready.request(1).intent().clone(),
    )
    .unwrap();
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(request, copy(&ready.proof), None)
            .await,
        Err(OperationError::InvalidEvidence)
    ));
    let candidate = ready.candidate.with_wrapping_key(|key| {
        EnvelopeCandidate::from_obtained(work, ready.candidate.raw_json(), key).unwrap()
    });
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(ready.request(1), copy(&ready.proof), Some(&candidate))
            .await,
        Err(OperationError::InvalidEvidence)
    ));
    let base = ready.request(1);
    let changed = DeliveryIntent::new(
        base.intent().request_id().as_str(),
        EnvelopeEventId::from_bytes([90; 32]),
        base.intent().targets(),
        base.intent().policy(),
        base.intent().deadline_unix_ms(),
    )
    .unwrap();
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(
                DeliveryAttemptRequest::new(ready.binding.clone(), ready.work, changed).unwrap(),
                copy(&ready.proof),
                Some(&ready.candidate)
            )
            .await,
        Err(OperationError::InvalidEvidence)
    ));
    let retained = ready.retain(1).await;
    let replay = ready
        .store
        .prepare_delivery_attempt(ready.request(1), copy(&ready.proof), Some(&ready.candidate))
        .await
        .unwrap();
    let DeliveryAttemptOutcome::Retained(replayed) = replay else {
        panic!("same potential invocation must never be freshly claimed")
    };
    assert_eq!(replayed.transaction_id(), retained.transaction_id());
    let different = ready.candidate.with_wrapping_key(|key| {
        EnvelopeCandidate::from_obtained(
            ready.work,
            &format!("{} ", ready.candidate.raw_json()),
            key,
        )
        .unwrap()
    });
    for id in [1, 2] {
        assert!(matches!(
            ready
                .store
                .prepare_delivery_attempt(ready.request(id), copy(&ready.proof), Some(&different))
                .await,
            Err(OperationError::IdempotencyConflict)
        ));
    }
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
async fn native_delivery_refuses_backward_clock_expired_attempt_and_unbound_observations() {
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 249).await;
    let first = ready.retain(1).await;
    let time = ready.work.queued_at().get();
    ready.fixture.clock.now.store(time + 1, Ordering::SeqCst);
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(ready.request(2), copy(&ready.proof), None)
            .await
            .unwrap(),
        DeliveryAttemptOutcome::Claimed(_)
    ));
    ready.fixture.clock.now.store(time, Ordering::SeqCst);
    assert!(
        ready
            .store
            .prepare_delivery_attempt(ready.request(3), copy(&ready.proof), None)
            .await
            .is_err()
    );
    ready.fixture.clock.now.store(time + 61, Ordering::SeqCst);
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(ready.request(3), copy(&ready.proof), None)
            .await,
        Err(OperationError::DeadlineExceeded)
    ));
    for id in ["short".to_string(), "AA".repeat(32)] {
        let e = ready.work.evidence();
        assert!(matches!(
            ready
                .store
                .delivery_attempt(
                    ready.binding.clone(),
                    e.operation_id(),
                    e.command_digest(),
                    e.target(),
                    DeliveryRequestId::parse(id).unwrap(),
                    copy(&ready.proof)
                )
                .await,
            Err(OperationError::InvalidCommand)
        ));
    }
    let work = rebound_work(&ready).await;
    let target = DeliveryTargetReceipt::attempted(
        first.intent().targets().targets()[0].clone(),
        DeliveryOutcome::accepted(),
    );
    // Structural data is deliberately untrusted; these are not remote ACKs.
    for (reported_work, raw, ordinals) in [
        (work, first.raw_json().to_owned(), vec![1]),
        (
            first.work().private().unwrap(),
            format!("{} ", first.raw_json()),
            vec![1],
        ),
        (
            first.work().private().unwrap(),
            first.raw_json().to_owned(),
            vec![2],
        ),
    ] {
        let attempt = DeliveryAttempt::from_store_report(
            reported_work,
            first.intent().clone(),
            &raw,
            first.transaction_id(),
            first.retained_at(),
            ordinals,
        )
        .unwrap();
        let observation = DeliveryObservation::from_store_report(&attempt, &target, None).unwrap();
        assert!(
            ready
                .store
                .record_delivery_observation(ready.binding.clone(), copy(&ready.proof), observation)
                .await
                .is_err()
        );
    }
    assert!(
        ready
            .observations(&ready.store, first.intent().request_id())
            .await
            .is_empty()
    );
    assert_eq!(
        ready
            .query(&ready.store, first.intent().request_id())
            .await
            .raw_json(),
        first.raw_json()
    );
    ready.store.close().await.unwrap();
}

#[tokio::test]
async fn native_observation_query_never_crosses_original_request_or_recipient() {
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 252).await;
    let attempt = ready.retain(1).await;
    // Synthetic normalized data tests query scope; loopback cases own ACK proof.
    let target = DeliveryTargetReceipt::attempted(
        attempt.intent().targets().targets()[0].clone(),
        DeliveryOutcome::accepted(),
    );
    let observation = DeliveryObservation::from_store_report(&attempt, &target, None).unwrap();
    assert!(matches!(
        ready
            .store
            .record_delivery_observation(ready.binding.clone(), copy(&ready.proof), observation)
            .await
            .unwrap(),
        DeliveryObservationOutcome::Committed(_)
    ));
    assert_eq!(
        ready
            .observations(&ready.store, attempt.intent().request_id())
            .await
            .len(),
        1
    );
    assert!(
        ready
            .observations(
                &ready.store,
                &DeliveryRequestId::parse("ab".repeat(32)).unwrap()
            )
            .await
            .is_empty()
    );
    let e = ready.work.evidence();
    assert!(
        ready
            .store
            .delivery_observations(
                ready.binding.clone(),
                e.operation_id(),
                e.command_digest(),
                radroots_protocol::plan::SigningTarget::BusinessSeal(key()),
                attempt.intent().request_id().clone(),
                copy(&ready.proof)
            )
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        ready
            .observations(&ready.store, attempt.intent().request_id())
            .await
            .len(),
        1
    );
    ready.store.close().await.unwrap();
}
