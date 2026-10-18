use super::*;
use core::{cell::Cell, num::NonZeroU64};
use radroots_nostr::{
    key::parse_secret_key,
    signing::{LocalSigner, OsNonceSource},
};
use radroots_signing::{
    issuance::{OutcomeKind, SigningIntent},
    provider::{ParticipantProvider, ProviderCall},
    request::CancellationSignal,
};
use radroots_storage::command::{
    CapturedSeal, ClaimAttemptOutcome, ClaimAttemptRequest, EvidenceCaptureOutcome, SealEvidence,
    SigningAttempt,
};
thread_local! {static CALLS:Cell<usize>=const {Cell::new(0)};static MISSING:Cell<bool>=const {Cell::new(false)};}
fn signer_clock() -> Result<u64, radroots_signing::Error> {
    let count = CALLS.with(|calls| {
        let value = calls.get() + 1;
        calls.set(value);
        value
    });
    if count > 1 && MISSING.with(Cell::get) {
        Err(radroots_signing::Error::new(
            radroots_signing::error::Kind::InternalError,
        ))
    } else {
        Ok(100_000)
    }
}
fn selection(
    binding: &WorkspaceBinding,
    prepared: &PreparedOperation,
    plan_digest: [u8; 32],
    recipient: PublicKey,
) -> ClaimAttemptRequest {
    ClaimAttemptRequest::new(
        binding.clone(),
        prepared.identity().operation_id(),
        prepared.identity().command_digest(),
        plan_digest,
        recipient,
    )
}
fn claim(outcome: ClaimAttemptOutcome) -> SigningAttempt {
    match outcome {
        ClaimAttemptOutcome::Claimed(value) => value,
        _ => panic!("first actual claim must confirm its commit"),
    }
}
async fn prepare(
    fixture: &Fixture,
    store: &NativeWorkspace,
    binding: &WorkspaceBinding,
    proof: &AttachmentProof,
    id: u8,
    large: bool,
) -> (PreparedOperation, [u8; 32], Vec<PublicKey>) {
    let mut captured = peer_request(binding, id);
    if large {
        let mut value: serde_json::Value =
            serde_json::from_slice(captured.command().protected_wire()).unwrap();
        value["input"]["text"] = "\0".repeat(3500).into();
        captured = AdmissionRequest::from_protected_wire(
            binding.clone(),
            &serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
    let prepared = committed(
        store
            .admit_command_and_hold(captured, copy(proof))
            .await
            .unwrap(),
    );
    let plan = plan(prepared.request().command(), 100, "native-issuance");
    let digest = *plan.digest();
    let recipients = plan.seals().iter().map(|seal| seal.recipient()).collect();
    bound(
        store
            .bind_signing_intents(binding.clone(), plan, copy(proof))
            .await
            .unwrap(),
    );
    let _ = fixture;
    (prepared, digest, recipients)
}
async fn obtain(attempt: &SigningAttempt, missing: bool) -> SealEvidence {
    CALLS.with(|calls| calls.set(0));
    MISSING.with(|value| value.set(missing));
    // Published synthetic key in a qualified local provider, isolated from all host custody.
    let signer = LocalSigner::with_clock(
        parse_secret_key("0000000000000000000000000000000000000000000000000000000000000001")
            .unwrap(),
        signer_clock,
    )
    .unwrap()
    .with_nonce_source(Arc::new(OsNonceSource));
    let observed = signer.provider_state().await.unwrap();
    assert_eq!(observed.identity(), key());
    assert_eq!(
        observed.exact_issuance(),
        Some(radroots_signing::recovery::ReplayCapability::NonReplayable)
    );
    let intent = SigningIntent::from_chat_seal(
        attempt.binding().operation_id(),
        attempt.bound_plan().prepared().identity().command_digest(),
        attempt.seal(),
        ProviderCall::new(key(), attempt.deadline_unix_ms(), CancellationSignal::new()),
    )
    .unwrap();
    assert_eq!(intent.binding(), attempt.binding());
    let outcome = signer.sign_exact(intent).await;
    assert_eq!(outcome.binding(), attempt.binding());
    let (_, kind) = outcome.into_parts();
    let OutcomeKind::Obtained(artifact) = kind else {
        panic!("actual local exact signing must obtain")
    };
    assert_eq!(artifact.observed_at_unix_ms().is_none(), missing);
    artifact.with_json(|raw| {
        SealEvidence::verify(attempt, raw, artifact.observed_at_unix_ms()).unwrap()
    })
}
fn captured(outcome: EvidenceCaptureOutcome) -> radroots_storage::command::EvidenceReceipt {
    match outcome {
        EvidenceCaptureOutcome::Committed(value) => value,
        _ => panic!("fixture capture must confirm commit"),
    }
}
#[tokio::test]
async fn real_exact_peer_and_self_signatures_capture_reopen_and_never_reclaim_original_intents() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 130, false).await;
    let mut originals = Vec::new();
    for recipient in recipients {
        let first = claim(
            store
                .claim_attempt(
                    selection(&binding, &prepared, digest, recipient),
                    copy(&proof),
                )
                .await
                .unwrap(),
        );
        let signature = obtain(&first, true).await;
        let raw = signature.raw_json().to_owned();
        let receipt = captured(
            store
                .capture_evidence(input.clone(), &proof, &signature)
                .await
                .unwrap(),
        );
        assert_eq!(receipt.observed_at_unix_ms(), None);
        let duplicate = store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipient),
                copy(&proof),
            )
            .await
            .unwrap();
        assert!(matches!(duplicate, ClaimAttemptOutcome::Retained(_)));
        let repeated = SealEvidence::verify(&first, &raw, NonZeroU64::new(111_000)).unwrap();
        assert_eq!(
            captured(
                store
                    .capture_evidence(input.clone(), &proof, &repeated)
                    .await
                    .unwrap()
            ),
            receipt
        );
        originals.push((recipient, raw, receipt, first.attempt_id()));
    }
    store.close().await.unwrap();
    fixture.clock.now.store(151, Ordering::SeqCst);
    let reopened = fixture.reopen(input).await;
    for (recipient, raw, receipt, id) in originals {
        let original = reopened
            .signing_attempt(
                selection(&binding, &prepared, digest, recipient),
                copy(&proof),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(original.attempt_id(), id);
        let recovered: CapturedSeal = reopened
            .obtained_evidence(
                selection(&binding, &prepared, digest, recipient),
                copy(&proof),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(recovered.evidence().raw_json(), raw);
        assert_eq!(recovered.receipt(), receipt);
        assert!(matches!(
            reopened
                .claim_attempt(
                    selection(&binding, &prepared, digest, recipient),
                    copy(&proof)
                )
                .await
                .unwrap(),
            ClaimAttemptOutcome::Retained(_)
        ));
    }
    reopened.close().await.unwrap();
}
#[tokio::test]
async fn late_revoked_expired_and_missing_clock_capture_retains_bytes_without_new_rights() {
    for mode in 0..3 {
        let fixture = Fixture::new();
        let (store, input, proof, binding) = fixture.create().await;
        let (prepared, digest, recipients) =
            prepare(&fixture, &store, &binding, &proof, 131, false).await;
        let first = claim(
            store
                .claim_attempt(
                    selection(&binding, &prepared, digest, recipients[0]),
                    copy(&proof),
                )
                .await
                .unwrap(),
        );
        let evidence = obtain(&first, true).await;
        if mode == 0 {
            fixture.clock.now.store(201, Ordering::SeqCst);
        }
        if mode == 1 {
            store.revoke(input.clone(), &proof).await.unwrap();
        }
        if mode == 2 {
            fixture.clock.now.store(0, Ordering::SeqCst);
        }
        let receipt = captured(
            store
                .capture_evidence(input.clone(), &proof, &evidence)
                .await
                .unwrap(),
        );
        assert_eq!(receipt.observed_at_unix_ms(), None);
        assert!(
            store
                .claim_attempt(
                    selection(&binding, &prepared, digest, recipients[1]),
                    copy(&proof)
                )
                .await
                .is_err()
        );
        store.close().await.unwrap();
        let reopened = fixture.reopen(input.clone()).await;
        assert_eq!(
            captured(
                reopened
                    .capture_evidence(input, &proof, &evidence)
                    .await
                    .unwrap()
            ),
            receipt
        );
        reopened.close().await.unwrap();
    }
}
#[tokio::test]
async fn one_recipient_obtained_does_not_invent_the_other_and_failed_capture_keeps_caller_original()
{
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 132, true).await;
    let first = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let evidence = obtain(&first, false).await;
    let raw = evidence.raw_json().to_owned();
    assert!(raw.len() > 16384);
    let second = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[1]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let mut database = fixture.database().await;
    sqlx::raw_sql("CREATE TRIGGER refuse_evidence BEFORE INSERT ON seal_evidence_parts WHEN NEW.part=2 BEGIN SELECT RAISE(ABORT,'synthetic capture failure'); END").execute(&mut database).await.unwrap();
    assert!(matches!(
        store
            .capture_evidence(input.clone(), &proof, &evidence)
            .await,
        Err(OperationError::StorageFailure)
    ));
    assert_eq!(evidence.raw_json(), raw);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM seal_evidence")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::raw_sql("DROP TRIGGER refuse_evidence")
        .execute(&mut database)
        .await
        .unwrap();
    database.close().await.unwrap();
    captured(
        store
            .capture_evidence(input, &proof, &evidence)
            .await
            .unwrap(),
    );
    assert!(
        store
            .obtained_evidence(
                selection(&binding, &prepared, digest, recipients[1]),
                copy(&proof)
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, second.seal().recipient()),
                copy(&proof)
            )
            .await
            .unwrap(),
        ClaimAttemptOutcome::Retained(_)
    ));
    store.close().await.unwrap();
}
#[tokio::test]
async fn verification_refuses_wrong_field_signature_and_binding_before_capture() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 133, false).await;
    let first = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let evidence = obtain(&first, false).await;
    let second = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[1]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    assert!(!evidence.matches_attempt(&second));
    for (field, value) in [
        ("sig", serde_json::json!("00".repeat(64))),
        ("id", serde_json::json!("00".repeat(32))),
        ("pubkey", serde_json::json!(keys(2).public_key().to_hex())),
        ("content", serde_json::json!("changed")),
        ("kind", serde_json::json!(14)),
        ("created_at", serde_json::json!(98)),
        ("tags", serde_json::json!([["p", "changed"]])),
    ] {
        let mut changed: serde_json::Value = serde_json::from_str(evidence.raw_json()).unwrap();
        changed[field] = value;
        assert!(
            matches!(
                SealEvidence::verify(&first, &changed.to_string(), None),
                Err(OperationError::InvalidEvidence)
            ),
            "{field}"
        );
    }
    assert!(SealEvidence::verify(&first, &"x".repeat(65537), None).is_err());
    store.close().await.unwrap();
}

#[tokio::test]
async fn lost_claim_observer_reopens_as_retained_potential_issuance_not_a_second_dispatch() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 140, false).await;
    let (arrived, resume) = fixture.clock.arm(1);
    let mut first = store.claim_attempt(
        selection(&binding, &prepared, digest, recipients[0]),
        copy(&proof),
    );
    assert!(futures::poll!(first.as_mut()).is_pending());
    reached(arrived).await;
    drop(first);
    resume.send(()).unwrap();
    store.close().await.unwrap();
    let reopened = fixture.reopen(input).await;
    assert!(matches!(
        reopened
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof)
            )
            .await
            .unwrap(),
        ClaimAttemptOutcome::Retained(_)
    ));
    assert!(
        reopened
            .obtained_evidence(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof)
            )
            .await
            .unwrap()
            .is_none()
    );
    reopened.close().await.unwrap();
}
#[tokio::test]
async fn accepted_capture_survives_lost_observer_and_conservative_byte_exhaustion() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 141, true).await;
    let first = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let evidence = obtain(&first, true).await;
    let mut database = fixture.database().await;
    let transaction = database.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let mut waiting = Vec::new();
    for _ in 0..16 {
        let mut call = store.capture_evidence(input.clone(), &proof, &evidence);
        assert!(futures::poll!(call.as_mut()).is_pending());
        waiting.push(call);
    }
    assert!(matches!(
        store
            .capture_evidence(input.clone(), &proof, &evidence)
            .await,
        Err(OperationError::Workspace(WorkspaceError::Unavailable))
    ));
    drop(waiting.remove(0));
    transaction.rollback().await.unwrap();
    database.close().await.unwrap();
    let mut retained = None;
    for call in waiting {
        let receipt = captured(call.await.unwrap());
        if let Some(original) = retained {
            assert_eq!(receipt, original);
        } else {
            retained = Some(receipt);
        }
    }
    store.close().await.unwrap();
    let reopened = fixture.reopen(input).await;
    let original = reopened
        .obtained_evidence(
            selection(&binding, &prepared, digest, recipients[0]),
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original.evidence().raw_json(), evidence.raw_json());
    assert_eq!(Some(original.receipt()), retained);
    reopened.close().await.unwrap();
}
#[tokio::test]
async fn actual_claim_commit_uncertainty_latches_and_reopen_never_infers_a_safe_reissue() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 142, false).await;
    let mut database = fixture.database().await;
    sqlx::raw_sql("CREATE TABLE claim_commit_fault (parent INTEGER REFERENCES workspace_grant(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER claim_commit_fault_trigger AFTER INSERT ON signing_attempts BEGIN INSERT INTO claim_commit_fault VALUES(2); END").execute(&mut database).await.unwrap();
    database.close().await.unwrap();
    let outcome = store
        .claim_attempt(
            selection(&binding, &prepared, digest, recipients[0]),
            copy(&proof),
        )
        .await
        .unwrap();
    match outcome {
        ClaimAttemptOutcome::Unknown(anchor) => {
            assert_eq!(anchor.operation_id(), prepared.identity().operation_id());
            assert_eq!(
                anchor.command_digest(),
                prepared.identity().command_digest()
            );
            assert!(anchor.transaction_id().is_some());
        }
        _ => panic!("unconfirmed commit must stay unknown"),
    }
    assert!(matches!(
        store
            .operation_snapshot(
                binding.clone(),
                prepared.identity().operation_id(),
                None,
                copy(&proof)
            )
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    assert!(matches!(
        store
            .signing_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof)
            )
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    store.close().await.unwrap();
    let mut database = fixture.database().await;
    sqlx::raw_sql("DROP TRIGGER claim_commit_fault_trigger; DROP TABLE claim_commit_fault")
        .execute(&mut database)
        .await
        .unwrap();
    database.close().await.unwrap();
    let reopened = fixture.reopen(input).await;
    assert!(
        reopened
            .signing_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof)
            )
            .await
            .unwrap()
            .is_none()
    );
    reopened.close().await.unwrap();
}
#[tokio::test]
async fn uncertain_capture_keeps_original_bytes_until_safe_reopen_and_exact_retry() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 143, true).await;
    let first = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let evidence = obtain(&first, true).await;
    let raw = evidence.raw_json().to_owned();
    let mut database = fixture.database().await;
    sqlx::raw_sql("CREATE TRIGGER capture_unknown BEFORE INSERT ON seal_evidence_parts WHEN NEW.part=2 BEGIN SELECT RAISE(ROLLBACK,'synthetic unknown capture'); END").execute(&mut database).await.unwrap();
    database.close().await.unwrap();
    let outcome = store
        .capture_evidence(input.clone(), &proof, &evidence)
        .await
        .unwrap();
    match outcome {
        EvidenceCaptureOutcome::Unknown(anchor) => {
            assert_eq!(anchor.operation_id(), prepared.identity().operation_id());
            assert_eq!(
                anchor.command_digest(),
                prepared.identity().command_digest()
            );
        }
        _ => panic!("uncertain capture must not report a known commit/rollback"),
    }
    assert_eq!(evidence.raw_json(), raw);
    store.close().await.unwrap();
    let mut database = fixture.database().await;
    sqlx::raw_sql("DROP TRIGGER capture_unknown")
        .execute(&mut database)
        .await
        .unwrap();
    database.close().await.unwrap();
    let reopened = fixture.reopen(input.clone()).await;
    captured(
        reopened
            .capture_evidence(input, &proof, &evidence)
            .await
            .unwrap(),
    );
    let original = reopened
        .obtained_evidence(
            selection(&binding, &prepared, digest, recipients[0]),
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original.evidence().raw_json(), raw);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn missing_preparation_plan_and_attempt_are_distinct_from_wrong_plan_identity() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create().await;
    let request = peer_request(&binding, 145);
    let operation = request.command().context().operation_id();
    let command = request.command().digest();
    let selector =
        |digest| ClaimAttemptRequest::new(binding.clone(), operation, command, digest, key());
    assert!(
        store
            .signing_attempt(selector([0; 32]), copy(&proof))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .obtained_evidence(selector([0; 32]), copy(&proof))
            .await
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        store.claim_attempt(selector([0; 32]), copy(&proof)).await,
        Err(OperationError::RecoveryRequired)
    ));
    let prepared = committed(
        store
            .admit_command_and_hold(request, copy(&proof))
            .await
            .unwrap(),
    );
    assert!(
        store
            .signing_attempt(selector([0; 32]), copy(&proof))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .obtained_evidence(selector([0; 32]), copy(&proof))
            .await
            .unwrap()
            .is_none()
    );
    let retained = plan(prepared.request().command(), 100, "missing-claim");
    let digest = *retained.digest();
    bound(
        store
            .bind_signing_intents(binding.clone(), retained, copy(&proof))
            .await
            .unwrap(),
    );
    assert!(
        store
            .signing_attempt(selector(digest), copy(&proof))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .obtained_evidence(selector(digest), copy(&proof))
            .await
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        store.claim_attempt(selector([0; 32]), copy(&proof)).await,
        Err(OperationError::IdempotencyConflict)
    ));
    assert!(matches!(
        store.signing_attempt(selector([0; 32]), copy(&proof)).await,
        Err(OperationError::IdempotencyConflict)
    ));
    assert!(matches!(
        store
            .obtained_evidence(selector([0; 32]), copy(&proof))
            .await,
        Err(OperationError::IdempotencyConflict)
    ));
    store.close().await.unwrap();
}

#[tokio::test]
async fn fresh_claim_refuses_rollback_and_expiry_before_and_inside_the_transaction() {
    for inside in [false, true] {
        for now in [99, 150] {
            let fixture = Fixture::new();
            let (store, _, proof, binding) = fixture.create().await;
            let (prepared, digest, recipients) =
                prepare(&fixture, &store, &binding, &proof, 146, false).await;
            let request = selection(&binding, &prepared, digest, recipients[0]);
            let result = if inside {
                let (arrived, resume) = fixture.clock.arm(1);
                let mut call = store.claim_attempt(request, copy(&proof));
                assert!(futures::poll!(call.as_mut()).is_pending());
                reached(arrived).await;
                fixture.clock.now.store(now, Ordering::SeqCst);
                resume.send(()).unwrap();
                call.await
            } else {
                fixture.clock.now.store(now, Ordering::SeqCst);
                store.claim_attempt(request, copy(&proof)).await
            };
            assert!(
                if now == 99 {
                    matches!(
                        result,
                        Err(OperationError::Workspace(WorkspaceError::Unavailable))
                    )
                } else {
                    matches!(result, Err(OperationError::DeadlineExceeded))
                },
                "inside={inside} now={now}: {result:?}"
            );
            fixture.clock.now.store(100, Ordering::SeqCst);
            assert!(
                store
                    .signing_attempt(
                        selection(&binding, &prepared, digest, recipients[0]),
                        copy(&proof)
                    )
                    .await
                    .unwrap()
                    .is_none()
            );
            store.close().await.unwrap();
            let mut database = fixture.database().await;
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM signing_attempts")
                .fetch_one(&mut database)
                .await
                .unwrap();
            assert_eq!(count, 0);
            database.close().await.unwrap();
        }
    }
}

#[tokio::test]
async fn known_claim_insert_rollback_leaves_no_invocation_and_allows_one_new_claim() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 147, false).await;
    let mut database = fixture.database().await;
    sqlx::raw_sql("CREATE TRIGGER refuse_claim BEFORE INSERT ON signing_attempts BEGIN SELECT RAISE(ABORT,'synthetic claim failure'); END").execute(&mut database).await.unwrap();
    assert!(matches!(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof)
            )
            .await,
        Err(OperationError::StorageFailure)
    ));
    assert!(
        store
            .signing_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof)
            )
            .await
            .unwrap()
            .is_none()
    );
    sqlx::raw_sql("DROP TRIGGER refuse_claim")
        .execute(&mut database)
        .await
        .unwrap();
    let original = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let retained = match store
        .claim_attempt(
            selection(&binding, &prepared, digest, recipients[0]),
            copy(&proof),
        )
        .await
        .unwrap()
    {
        ClaimAttemptOutcome::Retained(value) => value,
        _ => panic!("second claim must remain retained"),
    };
    assert_eq!(retained.attempt_id(), original.attempt_id());
    database.close().await.unwrap();
    store.close().await.unwrap();
}
