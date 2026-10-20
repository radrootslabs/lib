use super::*;

#[tokio::test]
async fn a_retry_cannot_replace_the_original_publication_target_policy() {
    let ready = Ready::new(
        &[
            String::from("ws://127.0.0.1:9"),
            String::from("ws://127.0.0.1:10"),
        ],
        243,
    )
    .await;
    let original = ready.retain(1).await;
    let narrowed = TargetSet::new(vec![original.intent().targets().targets()[0].clone()]).unwrap();
    let replacement = DeliveryIntent::new(
        &"02".repeat(32),
        original.intent().event_id(),
        &narrowed,
        original.intent().policy(),
        original.intent().deadline_unix_ms(),
    )
    .unwrap();
    let request =
        DeliveryAttemptRequest::new(ready.binding.clone(), ready.work, replacement).unwrap();
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(request, copy(&ready.proof), None)
            .await,
        Err(OperationError::IdempotencyConflict)
    ));
    ready.store.close().await.unwrap();
}

fn selected_request(
    ready: &Ready,
    id: u8,
    policy: &SatisfactionPolicy,
    index: usize,
) -> DeliveryAttemptRequest {
    let original = ready.request(id);
    let selected =
        TargetSet::new(vec![original.intent().targets().targets()[index].clone()]).unwrap();
    let intent = DeliveryIntent::new(
        original.intent().request_id().as_str(),
        original.intent().event_id(),
        original.intent().targets(),
        policy,
        original.intent().deadline_unix_ms(),
    )
    .unwrap()
    .with_selection(&selected)
    .unwrap();
    DeliveryAttemptRequest::new(ready.binding.clone(), ready.work, intent).unwrap()
}

// Synthetic normalized adapter data exercises storage/policy composition here.
// The separate actual loopback cases qualify remote frame and ACK behavior.
fn synthetic_accepted(attempt: &DeliveryAttempt) -> DeliveryObservation {
    use radroots_transport::{
        DeliveryReceipt, outcome::DeliveryOutcome, sink::DeliveryTargetReceipt,
    };
    let request = attempt.transport_request().unwrap();
    let selected = &attempt.intent().selection()[0];
    let receipts = request
        .target_set()
        .targets()
        .iter()
        .map(|target| {
            if target.fingerprint() == selected {
                DeliveryTargetReceipt::attempted(target.clone(), DeliveryOutcome::accepted())
            } else {
                DeliveryTargetReceipt::skipped(target.clone(), DeliveryOutcome::unavailable())
                    .unwrap()
            }
        })
        .collect();
    let receipt = DeliveryReceipt::for_request(&request, receipts).unwrap();
    DeliveryObservation::from_receipt(attempt, &receipt, selected, None).unwrap()
}

#[tokio::test]
async fn exhausted_optional_endpoint_does_not_starve_required_endpoint_or_weaken_the_original_policy()
 {
    let ready = Ready::new(
        &[
            String::from("ws://127.0.0.1:9"),
            String::from("ws://127.0.0.1:10"),
        ],
        244,
    )
    .await;
    let base = ready.request(1);
    let required = base.intent().targets().targets()[1].fingerprint().clone();
    let policy = SatisfactionPolicy::new(
        SatisfactionClass::Accepted,
        TargetPolicy::required(vec![required]).unwrap(),
    );
    for ordinal in 1..=12u8 {
        let selected = selected_request(&ready, ordinal, &policy, 0);
        let candidate = if ordinal == 1 {
            Some(&ready.candidate)
        } else {
            None
        };
        let DeliveryAttemptOutcome::Claimed(attempt) = ready
            .store
            .prepare_delivery_attempt(selected, copy(&ready.proof), candidate)
            .await
            .unwrap()
        else {
            panic!("new selected potential attempt expected")
        };
        assert_eq!(attempt.target_ordinals(), &[ordinal]);
        assert_eq!(attempt.intent().targets().len(), 2);
        assert_eq!(attempt.intent().selection().len(), 1);
    }
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(
                selected_request(&ready, 13, &policy, 0),
                copy(&ready.proof),
                None
            )
            .await,
        Err(OperationError::DeliveryAttemptLimit)
    ));
    let DeliveryAttemptOutcome::Claimed(required) = ready
        .store
        .prepare_delivery_attempt(
            selected_request(&ready, 14, &policy, 1),
            copy(&ready.proof),
            None,
        )
        .await
        .unwrap()
    else {
        panic!("required endpoint must progress independently")
    };
    assert_eq!(required.target_ordinals(), &[1]);
    assert_eq!(required.raw_json(), ready.candidate.raw_json());
    captured(
        ready
            .store
            .record_delivery_observation(
                ready.binding.clone(),
                copy(&ready.proof),
                synthetic_accepted(&required),
            )
            .await
            .unwrap(),
    );
    let e = ready.work.evidence();
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
    assert_eq!(snapshot.publication_accepted(ready.work), Ok(true));
    let receipt =
        radroots_execution::OperationReceipt::<radroots_domain::ChatMessage>::try_from_snapshot(
            &snapshot,
        )
        .unwrap();
    assert_eq!(receipt.delivery(), radroots_execution::Delivery::Unknown);
    assert!(
        !receipt
            .recovery()
            .contains(&radroots_execution::RecoveryRequirement::ResumeDelivery)
    );
    ready.store.close().await.unwrap();
}

#[tokio::test]
async fn original_quorum_accumulates_acceptance_across_independent_selected_attempts() {
    let ready = Ready::new(
        &[
            String::from("ws://127.0.0.1:9"),
            String::from("ws://127.0.0.1:10"),
            String::from("ws://127.0.0.1:11"),
        ],
        245,
    )
    .await;
    let policy = SatisfactionPolicy::new(
        SatisfactionClass::Accepted,
        TargetPolicy::quorum(2).unwrap(),
    );
    for index in 0..2 {
        let selected = selected_request(&ready, index as u8 + 1, &policy, index);
        let candidate = if index == 0 {
            Some(&ready.candidate)
        } else {
            None
        };
        let DeliveryAttemptOutcome::Claimed(attempt) = ready
            .store
            .prepare_delivery_attempt(selected, copy(&ready.proof), candidate)
            .await
            .unwrap()
        else {
            panic!("fresh selected attempt required")
        };
        captured(
            ready
                .store
                .record_delivery_observation(
                    ready.binding.clone(),
                    copy(&ready.proof),
                    synthetic_accepted(&attempt),
                )
                .await
                .unwrap(),
        );
        let e = ready.work.evidence();
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
        assert_eq!(snapshot.publication_accepted(ready.work), Ok(index == 1));
        let receipt = radroots_execution::OperationReceipt::<radroots_domain::ChatMessage>::try_from_snapshot(&snapshot).unwrap();
        assert_eq!(
            receipt
                .recovery()
                .contains(&radroots_execution::RecoveryRequirement::ResumeDelivery),
            index == 0
        );
    }
    ready.store.close().await.unwrap();
}

#[tokio::test]
async fn a_fresh_delivery_deadline_after_first_issuance_expiry_reuses_original_bytes_and_policy() {
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 246).await;
    let original = ready.retain(1).await;
    ready
        .fixture
        .clock
        .now
        .store(ready.work.queued_at().get() + 121, Ordering::SeqCst);
    let next = DeliveryIntent::new(
        &"02".repeat(32),
        original.intent().event_id(),
        original.intent().targets(),
        original.intent().policy(),
        NonZeroU64::new((ready.work.queued_at().get() + 180) * 1000).unwrap(),
    )
    .unwrap();
    assert_eq!(
        next.publication_policy_digest(),
        original.intent().publication_policy_digest()
    );
    assert_ne!(next.digest(), original.intent().digest());
    let DeliveryAttemptOutcome::Claimed(next) = ready
        .store
        .prepare_delivery_attempt(
            DeliveryAttemptRequest::new(ready.binding.clone(), ready.work, next).unwrap(),
            copy(&ready.proof),
            None,
        )
        .await
        .unwrap()
    else {
        panic!("current-access delivery may retain a fresh attempt without reissuing S")
    };
    assert_eq!(next.raw_json(), original.raw_json());
    assert_eq!(next.target_ordinals(), &[2]);
    let retained = ready
        .query(&ready.store, original.intent().request_id())
        .await;
    assert_eq!(
        retained.intent().deadline_unix_ms(),
        original.intent().deadline_unix_ms()
    );
    let mut database = ready.fixture.database().await;
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM signing_attempts),(SELECT count(*) FROM artifact_evidence),(SELECT count(*) FROM chat_messages)").fetch_one(&mut database).await.unwrap();
    assert_eq!(counts, (1, 1, 1));
    database.close().await.unwrap();
    ready.store.close().await.unwrap();
}

#[tokio::test]
async fn actual_loopback_wrong_ack_timeout_disconnect_and_rejection_remain_distinct_after_reopen() {
    for (case, expected) in [
        (0, DeliveryObservationKind::TimedOut),
        (1, DeliveryObservationKind::Disconnected),
        (2, DeliveryObservationKind::Rejected),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ready = Ready::new(
            &[format!("ws://{}", listener.local_addr().unwrap())],
            237 + case,
        )
        .await;
        let attempt = ready.retain(1).await;
        let exact = String::from(attempt.raw_json());
        let (release, released) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(tcp).await.unwrap();
            let raw = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if let Message::Text(raw) = socket.next().await.unwrap().unwrap() {
                        break raw.to_string();
                    }
                }
            })
            .await
            .unwrap();
            assert_eq!(raw, format!("[\"EVENT\",{exact}]"));
            let event: serde_json::Value = serde_json::from_str(&raw).unwrap();
            match case {
                0 => {
                    socket
                        .send(Message::Text(
                            json!(["OK", "ff".repeat(32), true, ""]).to_string().into(),
                        ))
                        .await
                        .unwrap();
                    let _ = released.await;
                }
                1 => {
                    socket.close(None).await.unwrap();
                }
                2 => {
                    socket
                        .send(Message::Text(
                            json!(["OK", event[1]["id"], false, "restricted: synthetic refusal"])
                                .to_string()
                                .into(),
                        ))
                        .await
                        .unwrap();
                }
                _ => unreachable!(),
            }
        });
        let request = attempt.transport_request().unwrap();
        let target = request.target_set().targets()[0].fingerprint();
        let transport = radroots_transport_nostr_native::create_transport(ready.config.clone());
        let result = transport.deliver(request.clone()).await;
        let observation = match result {
            Ok(receipt) => {
                assert!(!receipt.is_satisfied(&request).unwrap());
                DeliveryObservation::from_receipt(&attempt, &receipt, target, None).unwrap()
            }
            Err(failure) => {
                DeliveryObservation::from_failure(&attempt, &failure, target, None).unwrap()
            }
        };
        let _ = release.send(());
        server.await.unwrap();
        assert_eq!(observation.kind(), expected, "actual case {case}");
        let first = captured(
            ready
                .store
                .record_delivery_observation(ready.binding.clone(), copy(&ready.proof), observation)
                .await
                .unwrap(),
        );
        ready.store.close().await.unwrap();
        let reopened = ready.fixture.reopen(ready.input.clone()).await;
        assert_eq!(
            ready
                .observations(&reopened, attempt.intent().request_id())
                .await,
            vec![first]
        );
        let e = ready.work.evidence();
        let snapshot = reopened
            .operation_snapshot(
                ready.binding.clone(),
                e.operation_id(),
                None,
                copy(&ready.proof),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(snapshot.installed_message().is_some());
        let view = radroots_execution::OperationReceipt::<radroots_domain::ChatMessage>::try_from_snapshot(&snapshot).unwrap();
        assert!(
            view.recovery()
                .contains(&radroots_execution::RecoveryRequirement::ResumeDelivery)
        );
        assert_eq!(view.delivery_observations().len(), 1);
        reopened.close().await.unwrap();
    }
}

fn not_attempted(attempt: &DeliveryAttempt) -> DeliveryObservation {
    let request = attempt.transport_request().unwrap();
    DeliveryObservation::from_failure(
        attempt,
        &radroots_transport::SinkFailure::transport_closed(&request),
        request.target_set().targets()[0].fingerprint(),
        None,
    )
    .unwrap()
}

#[tokio::test]
async fn actual_observation_rollback_and_commit_uncertainty_preserve_retained_outer_and_business_facts()
 {
    for commit in [false, true] {
        let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 240 + u8::from(commit)).await;
        let attempt = ready.retain(1).await;
        let observation = not_attempted(&attempt);
        let mut database = ready.fixture.database().await;
        if commit {
            sqlx::raw_sql("CREATE TABLE observation_commit_fault (parent INTEGER REFERENCES workspace_grant(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER observation_commit_fault_trigger AFTER INSERT ON artifact_delivery_observations BEGIN INSERT INTO observation_commit_fault VALUES(2); END").execute(&mut database).await.unwrap();
        } else {
            sqlx::raw_sql("CREATE TRIGGER observation_insert_fault BEFORE INSERT ON artifact_delivery_observations BEGIN SELECT RAISE(ABORT,'synthetic observation fault'); END").execute(&mut database).await.unwrap();
        }
        database.close().await.unwrap();
        let result = ready
            .store
            .record_delivery_observation(
                ready.binding.clone(),
                copy(&ready.proof),
                observation.clone(),
            )
            .await;
        if commit {
            assert!(matches!(result, Ok(DeliveryObservationOutcome::Unknown(_))));
        } else {
            assert!(matches!(result, Err(OperationError::StorageFailure)));
        }
        ready.store.close().await.unwrap();
        let mut database = ready.fixture.database().await;
        if commit {
            sqlx::raw_sql("DROP TRIGGER observation_commit_fault_trigger; DROP TABLE observation_commit_fault").execute(&mut database).await.unwrap();
        } else {
            sqlx::query("DROP TRIGGER observation_insert_fault")
                .execute(&mut database)
                .await
                .unwrap();
        }
        let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM artifact_delivery_attempts),(SELECT count(*) FROM artifact_delivery_observations),(SELECT count(*) FROM chat_messages)").fetch_one(&mut database).await.unwrap();
        assert_eq!(counts, (1, 0, 1));
        database.close().await.unwrap();
        let reopened = ready.fixture.reopen(ready.input.clone()).await;
        assert_eq!(
            ready
                .query(&reopened, attempt.intent().request_id())
                .await
                .raw_json(),
            attempt.raw_json()
        );
        assert!(
            ready
                .observations(&reopened, attempt.intent().request_id())
                .await
                .is_empty()
        );
        assert_eq!(
            captured(
                reopened
                    .record_delivery_observation(
                        ready.binding.clone(),
                        copy(&ready.proof),
                        observation.clone()
                    )
                    .await
                    .unwrap()
            ),
            observation
        );
        reopened.close().await.unwrap();
    }
}

#[tokio::test]
async fn corrupted_delivery_ciphertext_and_oversized_observation_refuse_the_complete_snapshot() {
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 242).await;
    let attempt = ready.retain(1).await;
    captured(
        ready
            .store
            .record_delivery_observation(
                ready.binding.clone(),
                copy(&ready.proof),
                not_attempted(&attempt),
            )
            .await
            .unwrap(),
    );
    let mut database = ready.fixture.database().await;
    let original: Vec<u8> =
        sqlx::query_scalar("SELECT protected FROM artifact_delivery_parts WHERE part=1")
            .fetch_one(&mut database)
            .await
            .unwrap();
    let mut wrong = original.clone();
    wrong[0] ^= 1;
    sqlx::query("UPDATE artifact_delivery_parts SET protected=? WHERE part=1")
        .bind(wrong)
        .execute(&mut database)
        .await
        .unwrap();
    let e = ready.work.evidence();
    assert!(
        ready
            .store
            .operation_snapshot(
                ready.binding.clone(),
                e.operation_id(),
                None,
                copy(&ready.proof)
            )
            .await
            .is_err()
    );
    sqlx::query("UPDATE artifact_delivery_parts SET protected=? WHERE part=1")
        .bind(original)
        .execute(&mut database)
        .await
        .unwrap();
    let original: Vec<u8> =
        sqlx::query_scalar("SELECT protected FROM artifact_delivery_observations")
            .fetch_one(&mut database)
            .await
            .unwrap();
    sqlx::query("PRAGMA ignore_check_constraints=ON")
        .execute(&mut database)
        .await
        .unwrap();
    sqlx::query("UPDATE artifact_delivery_observations SET protected=zeroblob(262145)")
        .execute(&mut database)
        .await
        .unwrap();
    assert!(
        ready
            .store
            .operation_snapshot(
                ready.binding.clone(),
                e.operation_id(),
                None,
                copy(&ready.proof)
            )
            .await
            .is_err()
    );
    sqlx::query("UPDATE artifact_delivery_observations SET protected=?")
        .bind(original)
        .execute(&mut database)
        .await
        .unwrap();
    database.close().await.unwrap();
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
    assert_eq!(snapshot.delivery_attempts().len(), 1);
    assert_eq!(snapshot.delivery_observations().len(), 1);
    ready.store.close().await.unwrap();
}
