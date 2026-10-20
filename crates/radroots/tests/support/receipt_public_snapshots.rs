//! Structural report consistency over actual retained public command facts.
//! These transformations neither alter the journal nor grant dispatch rights.
use super::*;
use radroots_domain::TransactionId;
use radroots_storage::command::{
    ArtifactSnapshot, BoundSigningPlan, CapturedArtifact, ChatInstallation, ClaimAttemptRequest,
    DeliveryAttempt, DeliveryAttemptRequest, DeliveryAttemptSummary, DeliveryIntent,
    ListingInstallation, OperationSnapshot, PublicPublicationWork, PublicationArtifact,
    RetainedOperation, SigningTarget,
};

async fn original_plan(ready: &Ready, id: u8) -> BoundSigningPlan {
    let snapshot = ready.snapshot(id).await;
    ready
        .store
        .signing_plan(
            ready.scope.binding().clone(),
            OperationId::from_bytes([id; 32]),
            snapshot.prepared().identity().command_digest(),
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap()
}

async fn bare(ready: &Ready, id: u8) -> OperationSnapshot {
    let snapshot = ready.snapshot(id).await;
    OperationSnapshot::from_store_report(
        RetainedOperation::Planned(Box::new(original_plan(ready, id).await)),
        snapshot.artifacts().to_vec(),
        snapshot.observed_at(),
    )
    .unwrap()
}

async fn installed(ready: &Ready, id: u8) -> OperationSnapshot {
    let snapshot = ready.snapshot(id).await;
    bare(ready, id)
        .await
        .with_public_installation(
            snapshot.listing_installation(),
            snapshot.public_publications().to_vec(),
        )
        .unwrap()
}

async fn original_capture(ready: &Ready, id: u8) -> CapturedArtifact {
    let plan = original_plan(ready, id).await;
    ready
        .store
        .obtained_evidence(
            ClaimAttemptRequest::for_artifact(
                ready.scope.binding().clone(),
                OperationId::from_bytes([id; 32]),
                plan.prepared().identity().command_digest(),
                *plan.plan().digest(),
                SigningTarget::PublicEvent,
            ),
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn public_reports_cannot_cross_actual_authenticated_workspaces_for_the_same_signer() {
    let relay = Relay::new().await;
    let first = Ready::new(&relay, StartMode::Online, PublicSigner::new(), true).await;
    let second = Ready::new(&relay, StartMode::Online, PublicSigner::new(), true).await;
    assert_ne!(first.input.workspace_id(), second.input.workspace_id());
    for ready in [&first, &second] {
        ready
            .client
            .listings()
            .publish(ready.context(224, now() + 120), input(224))
            .await
            .unwrap();
    }
    let original = first.snapshot(224).await;
    let foreign = second.snapshot(224).await;
    let work = original.public_publications()[0];
    let intent = original.delivery_attempts()[0].intent().clone();
    assert!(DeliveryAttemptRequest::new(second.scope.binding().clone(), work, intent).is_err());
    assert!(
        ListingInstallation::from_committed_store(
            &original_plan(&first, 224).await,
            foreign.public_publications()[0].evidence(),
            original.listing_installation().unwrap().transaction_id(),
            original.listing_installation().unwrap().installed_at(),
        )
        .is_err()
    );
    let captured = original_capture(&first, 224).await;
    let foreign_capture = original_capture(&second, 224).await;
    let foreign_attempt = second
        .store
        .signing_attempt(
            ClaimAttemptRequest::for_artifact(
                second.scope.binding().clone(),
                OperationId::from_bytes([224; 32]),
                foreign.prepared().identity().command_digest(),
                *foreign_capture.attempt().bound_plan().plan().digest(),
                SigningTarget::PublicEvent,
            ),
            copy(&second.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(!captured.evidence().matches_attempt(&foreign_attempt));
    assert!(
        CapturedArtifact::from_committed_store(
            foreign_attempt,
            captured.evidence().retained_copy(),
            captured.receipt().transaction_id(),
        )
        .is_err()
    );
    assert_eq!(
        first.snapshot(224).await.checkpoint_digest(),
        original.checkpoint_digest()
    );
    assert_eq!(
        second.snapshot(224).await.checkpoint_digest(),
        foreign.checkpoint_digest()
    );
    assert_eq!(relay.count.load(Ordering::SeqCst), 2);
    for ready in [&first, &second] {
        assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
        assert_eq!(ready.signer.private_calls.load(Ordering::SeqCst), 0);
        ready.client.close().await.unwrap();
    }
    stop(relay).await;
}

#[tokio::test]
async fn public_reports_require_original_plan_obtained_basis_and_atomic_installation_links() {
    let relay = Relay::new().await;
    let ready = Ready::new(&relay, StartMode::Online, PublicSigner::new(), true).await;
    for id in [220, 221] {
        ready
            .client
            .listings()
            .publish(ready.context(id, now() + 120), input(id))
            .await
            .unwrap();
    }
    let original = ready.snapshot(220).await;
    let other = ready.snapshot(221).await;
    let installation = original.listing_installation().unwrap();
    let work = original.public_publications()[0];
    let before = original.checkpoint_digest();
    assert!(
        bare(&ready, 220)
            .await
            .with_public_installation(None, vec![work])
            .is_err()
    );
    for works in [
        vec![],
        vec![work, work],
        other.public_publications().to_vec(),
    ] {
        assert!(
            bare(&ready, 220)
                .await
                .with_public_installation(Some(installation), works)
                .is_err()
        );
    }
    assert!(
        bare(&ready, 220)
            .await
            .with_public_installation(other.listing_installation(), vec![work],)
            .is_err()
    );

    // Work may describe a later transaction, but cannot masquerade as the
    // initial atomic T2 pair required by this snapshot profile.
    for (transaction, time) in [
        (
            TransactionId::from_bytes([222; 32]),
            installation.installed_at(),
        ),
        (
            installation.transaction_id(),
            UnixSeconds::new(installation.installed_at().get() + 1).unwrap(),
        ),
    ] {
        let changed = PublicPublicationWork::from_committed_store(
            &original_plan(&ready, 220).await,
            installation,
            work.evidence(),
            transaction,
            time,
        )
        .unwrap();
        assert!(
            bare(&ready, 220)
                .await
                .with_public_installation(Some(installation), vec![changed])
                .is_err()
        );
    }
    let artifact = original.artifacts()[0];
    let unknown = ArtifactSnapshot::from_store_report(
        SigningTarget::PublicEvent,
        artifact.expected_event(),
        artifact.claim(),
        None,
    );
    let incomplete = OperationSnapshot::from_store_report(
        RetainedOperation::Planned(Box::new(original_plan(&ready, 220).await)),
        vec![unknown],
        original.observed_at(),
    )
    .unwrap();
    assert!(
        incomplete
            .with_public_installation(Some(installation), vec![work])
            .is_err()
    );
    let preparation = ready
        .store
        .operation(
            ready.scope.binding().clone(),
            OperationId::from_bytes([220; 32]),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    let unplanned = OperationSnapshot::from_store_report(
        RetainedOperation::Prepared(preparation),
        vec![ArtifactSnapshot::from_store_report(
            SigningTarget::PublicEvent,
            None,
            None,
            None,
        )],
        original.observed_at(),
    )
    .unwrap();
    assert!(
        unplanned
            .with_public_installation(Some(installation), vec![work])
            .is_err()
    );

    // An independently verified public signature supplies no private subject,
    // business seal or chat installation through any retained projection.
    assert!(artifact.recipient().is_err());
    assert!(artifact.claim().unwrap().recipient().is_err());
    assert!(artifact.obtained().unwrap().recipient().is_err());
    assert!(PublicationArtifact::Public(work).private().is_err());
    let plan = original_plan(&ready, 220).await;
    let selector = ClaimAttemptRequest::for_artifact(
        ready.scope.binding().clone(),
        OperationId::from_bytes([220; 32]),
        original.prepared().identity().command_digest(),
        *plan.plan().digest(),
        SigningTarget::PublicEvent,
    );
    assert!(selector.recipient().is_err());
    let attempt = ready
        .store
        .signing_attempt(selector, copy(&ready.proof))
        .await
        .unwrap()
        .unwrap();
    assert!(attempt.seal().is_err());
    assert!(
        ListingInstallation::from_committed_store(
            &plan,
            other.public_publications()[0].evidence(),
            installation.transaction_id(),
            installation.installed_at(),
        )
        .is_err()
    );
    let other_plan = original_plan(&ready, 221).await;
    let other_attempt = ready
        .store
        .signing_attempt(
            ClaimAttemptRequest::for_artifact(
                ready.scope.binding().clone(),
                OperationId::from_bytes([221; 32]),
                other.prepared().identity().command_digest(),
                *other_plan.plan().digest(),
                SigningTarget::PublicEvent,
            ),
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    let captured = ready
        .store
        .obtained_evidence(
            ClaimAttemptRequest::for_artifact(
                ready.scope.binding().clone(),
                OperationId::from_bytes([220; 32]),
                original.prepared().identity().command_digest(),
                *plan.plan().digest(),
                SigningTarget::PublicEvent,
            ),
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(
        CapturedArtifact::from_committed_store(
            other_attempt,
            captured.evidence().retained_copy(),
            captured.receipt().transaction_id(),
        )
        .is_err()
    );
    assert!(
        ListingInstallation::from_committed_store(
            &plan,
            work.evidence(),
            installation.transaction_id(),
            UnixSeconds::new(plan.bound_at().get() - 1).unwrap(),
        )
        .is_err()
    );
    assert!(
        ChatInstallation::from_committed_store(
            &plan,
            work.evidence(),
            installation.transaction_id(),
            installation.installed_at(),
        )
        .is_err()
    );
    let earlier = UnixSeconds::new(installation.installed_at().get() - 1).unwrap();
    assert!(
        PublicPublicationWork::from_committed_store(
            &plan,
            installation,
            work.evidence(),
            installation.transaction_id(),
            earlier,
        )
        .is_err()
    );
    assert!(
        installed(&ready, 220)
            .await
            .with_installation(None, vec![])
            .is_err()
    );
    assert_eq!(ready.snapshot(220).await.checkpoint_digest(), before);
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 2);
    assert_eq!(ready.signer.private_calls.load(Ordering::SeqCst), 0);
    assert_eq!(relay.count.load(Ordering::SeqCst), 2);
    ready.client.close().await.unwrap();
    stop(relay).await;
}

fn descriptor(
    original: &DeliveryAttempt,
    id: &str,
    transaction: TransactionId,
    ordinal: u8,
    policy: &SatisfactionPolicy,
) -> DeliveryAttemptSummary {
    let intent = DeliveryIntent::public(
        id,
        original.intent().event_id(),
        original.intent().targets(),
        policy,
        original.intent().deadline_unix_ms(),
    )
    .unwrap()
    .with_selection(&original.intent().selected_target_set().unwrap())
    .unwrap();
    let report = DeliveryAttempt::from_store_report(
        original.work(),
        intent,
        original.raw_json(),
        transaction,
        original.retained_at(),
        vec![ordinal],
    )
    .unwrap();
    DeliveryAttemptSummary::from_attempt(&report)
}

#[tokio::test]
async fn public_delivery_reports_refuse_unbacked_duplicate_renumbered_or_redefined_history() {
    let relay = Relay::new().await;
    let ready = Ready::new(&relay, StartMode::Online, PublicSigner::new(), true).await;
    ready
        .client
        .listings()
        .publish(ready.context(222, now() + 120), input(222))
        .await
        .unwrap();
    let snapshot = ready.snapshot(222).await;
    let summary = &snapshot.delivery_attempts()[0];
    let original = ready
        .store
        .delivery_attempt(
            ready.scope.binding().clone(),
            OperationId::from_bytes([222; 32]),
            snapshot.prepared().identity().command_digest(),
            SigningTarget::PublicEvent,
            summary.intent().request_id().clone(),
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    let observations = snapshot.delivery_observations().to_vec();
    assert_eq!(observations.len(), 1);
    assert!(
        bare(&ready, 222)
            .await
            .publication_accepted(original.work())
            .is_err()
    );

    // Re-encoding even an authentic event cannot replace its original retained
    // bytes. Corrupted signed fields and an oversized carrier refuse too.
    let spaced = format!(" {}", original.raw_json());
    nostr::Event::from_json(&spaced).unwrap().verify().unwrap();
    let signed = nostr::Event::from_json(original.raw_json()).unwrap();
    let keys = route_key(1);
    assert_eq!(signed.pubkey, keys.public_key());
    for (kind, content) in [
        (nostr::Kind::TextNote, signed.content.clone()),
        (signed.kind, "Another authentic event".to_owned()),
    ] {
        let foreign = nostr::UnsignedEvent {
            id: None,
            pubkey: signed.pubkey,
            created_at: signed.created_at,
            kind,
            tags: signed.tags.clone(),
            content,
        }
        .sign_with_keys(&keys)
        .unwrap();
        foreign.verify().unwrap();
        assert!(
            DeliveryAttempt::from_store_report(
                original.work(),
                original.intent().clone(),
                &foreign.as_json(),
                original.transaction_id(),
                original.retained_at(),
                vec![1],
            )
            .is_err()
        );
    }
    let mut wrong_kind: serde_json::Value = serde_json::from_str(original.raw_json()).unwrap();
    wrong_kind["kind"] = json!(1);
    let mut wrong_id: serde_json::Value = serde_json::from_str(original.raw_json()).unwrap();
    wrong_id["id"] = json!("00".repeat(32));
    for raw in [
        spaced,
        wrong_kind.to_string(),
        wrong_id.to_string(),
        " ".repeat(65_537),
        "{".to_owned(),
    ] {
        assert!(
            DeliveryAttempt::from_store_report(
                original.work(),
                original.intent().clone(),
                &raw,
                original.transaction_id(),
                original.retained_at(),
                vec![1],
            )
            .is_err()
        );
    }
    let private_intent = DeliveryIntent::new(
        original.intent().request_id().as_str(),
        original.intent().event_id(),
        original.intent().targets(),
        original.intent().policy(),
        original.intent().deadline_unix_ms(),
    )
    .unwrap();
    assert!(
        DeliveryAttempt::from_store_report(
            original.work(),
            private_intent,
            original.raw_json(),
            original.transaction_id(),
            original.retained_at(),
            vec![1],
        )
        .is_err()
    );
    let id = if summary.intent().request_id().as_str() == "01".repeat(32) {
        "02".repeat(32)
    } else {
        "01".repeat(32)
    };
    let tx = TransactionId::from_bytes([223; 32]);
    assert_ne!(tx, summary.transaction_id());
    let second = descriptor(&original, &id, tx, 2, summary.intent().policy());
    let alternate = SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::any());
    let cases = [
        vec![summary.clone(), summary.clone()],
        vec![
            summary.clone(),
            descriptor(
                &original,
                &id,
                summary.transaction_id(),
                2,
                summary.intent().policy(),
            ),
        ],
        vec![
            summary.clone(),
            descriptor(&original, &id, tx, 1, summary.intent().policy()),
        ],
        vec![second.clone()],
        vec![
            summary.clone(),
            descriptor(&original, &id, tx, 2, &alternate),
        ],
        vec![summary.clone(); 385],
    ];
    for attempts in cases {
        assert!(
            installed(&ready, 222)
                .await
                .with_delivery(attempts, observations.clone())
                .is_err()
        );
    }
    assert!(
        bare(&ready, 222)
            .await
            .with_delivery(vec![summary.clone()], observations.clone())
            .is_err()
    );
    assert!(
        installed(&ready, 222)
            .await
            .with_delivery(vec![], observations.clone())
            .is_err()
    );
    assert!(
        installed(&ready, 222)
            .await
            .with_delivery(vec![summary.clone()], vec![observations[0].clone(); 2])
            .is_err()
    );
    assert!(
        installed(&ready, 222)
            .await
            .with_delivery(vec![summary.clone()], vec![observations[0].clone(); 2305])
            .is_err()
    );
    let renumbered = [
        descriptor(
            &original,
            summary.intent().request_id().as_str(),
            summary.transaction_id(),
            2,
            summary.intent().policy(),
        ),
        descriptor(&original, &id, tx, 1, summary.intent().policy()),
    ];
    assert!(
        installed(&ready, 222)
            .await
            .with_delivery(renumbered.into(), observations.clone())
            .is_err()
    );
    let valid = installed(&ready, 222)
        .await
        .with_delivery(vec![summary.clone(), second], observations)
        .unwrap();
    assert!(valid.publication_accepted(original.work()).unwrap());
    assert_eq!(
        ready.snapshot(222).await.checkpoint_digest(),
        snapshot.checkpoint_digest()
    );
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(relay.count.load(Ordering::SeqCst), 1);
    ready.client.close().await.unwrap();
    stop(relay).await;
}

#[tokio::test]
async fn public_delivery_queries_require_exact_bounded_request_and_artifact_selectors() {
    use radroots_transport::sink::DeliveryRequestId;
    let relay = Relay::new().await;
    let ready = Ready::new(&relay, StartMode::Online, PublicSigner::new(), true).await;
    ready
        .client
        .listings()
        .publish(ready.context(223, now() + 120), input(223))
        .await
        .unwrap();
    let snapshot = ready.snapshot(223).await;
    let summary = &snapshot.delivery_attempts()[0];
    let operation = OperationId::from_bytes([223; 32]);
    let digest = snapshot.prepared().identity().command_digest();
    let query = |target, request| {
        ready.store.delivery_attempt(
            ready.scope.binding().clone(),
            operation,
            digest,
            target,
            request,
            copy(&ready.proof),
        )
    };
    let actual = query(
        SigningTarget::PublicEvent,
        summary.intent().request_id().clone(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(actual.work(), summary.work());
    assert_eq!(actual.envelope_digest(), summary.envelope_digest());
    for id in ["short".to_owned(), "GG".repeat(32)] {
        assert!(matches!(
            query(
                SigningTarget::PublicEvent,
                DeliveryRequestId::parse(id).unwrap()
            )
            .await,
            Err(OperationError::InvalidCommand)
        ));
    }
    let missing = if summary.intent().request_id().as_str() == "00".repeat(32) {
        "11".repeat(32)
    } else {
        "00".repeat(32)
    };
    assert!(
        query(
            SigningTarget::PublicEvent,
            DeliveryRequestId::parse(missing).unwrap()
        )
        .await
        .unwrap()
        .is_none()
    );
    assert!(
        query(
            SigningTarget::BusinessSeal(key()),
            summary.intent().request_id().clone()
        )
        .await
        .unwrap()
        .is_none()
    );
    assert_eq!(
        ready.snapshot(223).await.checkpoint_digest(),
        snapshot.checkpoint_digest()
    );
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(ready.signer.private_calls.load(Ordering::SeqCst), 0);
    assert_eq!(relay.count.load(Ordering::SeqCst), 1);
    ready.client.close().await.unwrap();
    stop(relay).await;
}
