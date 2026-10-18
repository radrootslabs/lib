use super::issuance::{claim, obtain, prepare, selection};
use super::*;
use radroots_execution::{
    BusinessInstallation, Delivery, Issuance, OperationReceipt, RecoveryRequirement,
};
use radroots_storage::command::{
    ChatInstallation, ClaimAttemptOutcome, EvidenceCaptureOutcome, EvidenceReceipt,
    InstallationOutcome, InstallationRequest, OperationSnapshot, SigningAttempt,
};
#[path = "receipt_installation_refusal.rs"]
mod refusal;
#[path = "receipt_installation_reports.rs"]
mod reports;

fn request_install(
    binding: &WorkspaceBinding,
    prepared: &PreparedOperation,
    plan: [u8; 32],
) -> InstallationRequest {
    InstallationRequest::new(
        binding.clone(),
        prepared.identity().operation_id(),
        prepared.identity().command_digest(),
        plan,
    )
}
fn installed(outcome: InstallationOutcome) -> ChatInstallation {
    match outcome {
        InstallationOutcome::Committed(value) => *value,
        InstallationOutcome::Unknown(_) => panic!("fixture must observe confirmed T2"),
    }
}
async fn capture(
    store: &NativeWorkspace,
    input: &AttachmentInput,
    proof: &AttachmentProof,
    attempt: &SigningAttempt,
) -> EvidenceReceipt {
    let evidence = obtain(attempt, true).await;
    match store
        .capture_evidence(input.clone(), proof, &evidence)
        .await
        .unwrap()
    {
        EvidenceCaptureOutcome::Committed(value) => value,
        EvidenceCaptureOutcome::Unknown(_) => panic!("fixture must confirm original capture"),
    }
}
async fn snapshot(
    store: &NativeWorkspace,
    binding: &WorkspaceBinding,
    prepared: &PreparedOperation,
    proof: &AttachmentProof,
) -> OperationSnapshot {
    store
        .operation_snapshot(
            binding.clone(),
            prepared.identity().operation_id(),
            Some(prepared.identity().command_digest()),
            copy(proof),
        )
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn actual_peer_installation_and_late_self_capture_keep_one_original_message_and_distinct_work()
 {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 220, true).await;
    assert!(matches!(
        store
            .install_transition_and_outbox(
                request_install(&binding, &prepared, digest),
                copy(&proof)
            )
            .await,
        Err(OperationError::InvalidEvidence)
    ));
    let peer = *recipients
        .iter()
        .find(|recipient| **recipient != key())
        .unwrap();
    let peer_claim = claim(
        store
            .claim_attempt(selection(&binding, &prepared, digest, peer), copy(&proof))
            .await
            .unwrap(),
    );
    let self_claim = claim(
        store
            .claim_attempt(selection(&binding, &prepared, digest, key()), copy(&proof))
            .await
            .unwrap(),
    );
    let peer_capture = capture(&store, &input, &proof, &peer_claim).await;
    // Obtain within the original first-issue window, then delay only persistence.
    let self_bytes = obtain(&self_claim, true).await;
    let before = snapshot(&store, &binding, &prepared, &proof).await;
    assert!(before.installed_message().is_none());
    fixture.clock.now.store(110, Ordering::SeqCst);
    let first = installed(
        store
            .install_transition_and_outbox(
                request_install(&binding, &prepared, digest),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    assert_eq!(first.basis(), peer_capture);
    assert_eq!(first.installed_at().get(), 110);
    let state = snapshot(&store, &binding, &prepared, &proof).await;
    assert_eq!(state.installation(), Some(first));
    assert_eq!(state.publications().len(), 1);
    assert_eq!(state.publications()[0].evidence(), peer_capture);
    assert_ne!(before.checkpoint_digest(), state.checkpoint_digest());
    let view = OperationReceipt::from_snapshot(&state);
    assert_eq!(
        view.business_installation(),
        BusinessInstallation::Installed
    );
    assert_eq!(view.issuance(), Issuance::Unknown);
    assert_eq!(view.delivery(), Delivery::Pending);
    assert!(view.delivery_observations().is_empty());
    assert_eq!(
        view.installation_transaction_id(),
        Some(first.transaction_id())
    );
    let message = view.business_result().unwrap().clone();
    assert_eq!(message.id(), first.message_id());
    assert_eq!(message.created_at().get(), 100);
    assert_eq!(message.text(), "\0".repeat(3500));
    assert_eq!(message.peer(), peer);
    assert_eq!(
        view.artifacts()
            .iter()
            .filter(|value| value.publication_transaction_id().is_some())
            .count(),
        1
    );
    store.close().await.unwrap();
    fixture.clock.now.store(151, Ordering::SeqCst);
    let reopened = fixture.reopen(input.clone()).await;
    let replay = installed(
        reopened
            .install_transition_and_outbox(
                request_install(&binding, &prepared, digest),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    assert_eq!(replay, first);
    assert_eq!(
        snapshot(&reopened, &binding, &prepared, &proof)
            .await
            .installed_message(),
        Some(&message)
    );
    let captured = reopened
        .capture_evidence(input, &proof, &self_bytes)
        .await
        .unwrap();
    assert!(matches!(captured, EvidenceCaptureOutcome::Committed(_)));
    let late = snapshot(&reopened, &binding, &prepared, &proof).await;
    let late = OperationReceipt::from_snapshot(&late);
    assert_eq!(
        late.business_installation(),
        BusinessInstallation::Installed
    );
    assert_eq!(late.issuance(), Issuance::Obtained);
    assert!(
        late.recovery()
            .contains(&RecoveryRequirement::InstallObtainedEvidence)
    );
    assert!(
        late.recovery()
            .contains(&RecoveryRequirement::ResumeDelivery)
    );
    assert!(late.error().is_none());
    let extended = installed(
        reopened
            .install_transition_and_outbox(
                request_install(&binding, &prepared, digest),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    assert_eq!(extended, first);
    let complete = snapshot(&reopened, &binding, &prepared, &proof).await;
    assert_eq!(complete.publications().len(), 2);
    assert_eq!(complete.installed_message(), Some(&message));
    let peer_work = complete
        .publications()
        .iter()
        .find(|value| value.evidence().recipient() == peer)
        .unwrap();
    let self_work = complete
        .publications()
        .iter()
        .find(|value| value.evidence().recipient() == key())
        .unwrap();
    assert_eq!(peer_work.transaction_id(), first.transaction_id());
    assert_ne!(self_work.transaction_id(), first.transaction_id());
    assert_eq!(self_work.queued_at().get(), 151);
    let receipt = OperationReceipt::from_snapshot(&complete);
    assert_eq!(receipt.installed_at(), Some(first.installed_at()));
    assert_eq!(receipt.business_result(), Some(&message));
    assert_eq!(receipt.recovery(), &[RecoveryRequirement::ResumeDelivery]);
    assert!(matches!(
        reopened
            .claim_attempt(selection(&binding, &prepared, digest, peer), copy(&proof))
            .await
            .unwrap(),
        ClaimAttemptOutcome::Retained(_)
    ));
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn actual_work_insert_failure_rolls_back_the_message_but_keeps_original_claim_and_evidence() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 221, false).await;
    let attempt = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let original = capture(&store, &input, &proof, &attempt).await;
    let mut database = fixture.database().await;
    sqlx::raw_sql("CREATE TRIGGER refuse_chat_work BEFORE INSERT ON chat_publication_work BEGIN SELECT RAISE(ABORT,'synthetic insert failure'); END").execute(&mut database).await.unwrap();
    assert!(matches!(
        store
            .install_transition_and_outbox(
                request_install(&binding, &prepared, digest),
                copy(&proof)
            )
            .await,
        Err(OperationError::StorageFailure)
    ));
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM chat_messages),(SELECT count(*) FROM chat_publication_work)",
    )
    .fetch_one(&mut database)
    .await
    .unwrap();
    assert_eq!(counts, (0, 0));
    let state = snapshot(&store, &binding, &prepared, &proof).await;
    assert!(state.installation().is_none());
    assert_eq!(
        state
            .recipients()
            .iter()
            .filter_map(|value| value.obtained())
            .collect::<Vec<_>>(),
        vec![original]
    );
    sqlx::query("DROP TRIGGER refuse_chat_work")
        .execute(&mut database)
        .await
        .unwrap();
    database.close().await.unwrap();
    installed(
        store
            .install_transition_and_outbox(
                request_install(&binding, &prepared, digest),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    assert!(
        snapshot(&store, &binding, &prepared, &proof)
            .await
            .installed_message()
            .is_some()
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn actual_t2_commit_uncertainty_latches_and_reopen_reconciles_original_obtained_facts() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 222, false).await;
    let attempt = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let original = capture(&store, &input, &proof, &attempt).await;
    let mut database = fixture.database().await;
    sqlx::raw_sql("CREATE TABLE chat_commit_fault (parent INTEGER REFERENCES workspace_grant(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER chat_commit_fault_trigger AFTER INSERT ON chat_publication_work BEGIN INSERT INTO chat_commit_fault VALUES(2); END").execute(&mut database).await.unwrap();
    database.close().await.unwrap();
    let InstallationOutcome::Unknown(anchor) = store
        .install_transition_and_outbox(request_install(&binding, &prepared, digest), copy(&proof))
        .await
        .unwrap()
    else {
        panic!("actual failed commit must remain anchored unknown")
    };
    assert_eq!(anchor.operation_id(), prepared.identity().operation_id());
    assert_eq!(
        anchor.command_digest(),
        prepared.identity().command_digest()
    );
    assert!(anchor.transaction_id().is_some());
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
    store.close().await.unwrap();
    let mut database = fixture.database().await;
    sqlx::raw_sql("DROP TRIGGER chat_commit_fault_trigger; DROP TABLE chat_commit_fault")
        .execute(&mut database)
        .await
        .unwrap();
    database.close().await.unwrap();
    let reopened = fixture.reopen(input).await;
    let state = snapshot(&reopened, &binding, &prepared, &proof).await;
    assert!(state.installation().is_none());
    assert_eq!(
        state
            .recipients()
            .iter()
            .filter_map(|value| value.obtained())
            .collect::<Vec<_>>(),
        vec![original]
    );
    installed(
        reopened
            .install_transition_and_outbox(
                request_install(&binding, &prepared, digest),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    assert!(
        snapshot(&reopened, &binding, &prepared, &proof)
            .await
            .installed_message()
            .is_some()
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn accepted_installation_survives_lost_observer_and_safe_close_drains_the_original_work() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 223, false).await;
    let attempt = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    capture(&store, &input, &proof, &attempt).await;
    let (arrived, resume) = fixture.clock.arm(1);
    let owner = store.clone();
    let selected = request_install(&binding, &prepared, digest);
    let secret = copy(&proof);
    let observer =
        tokio::spawn(async move { owner.install_transition_and_outbox(selected, secret).await });
    reached(arrived).await;
    observer.abort();
    assert!(observer.await.unwrap_err().is_cancelled());
    resume.send(()).unwrap();
    store.close().await.unwrap();
    let reopened = fixture.reopen(input).await;
    let state = snapshot(&reopened, &binding, &prepared, &proof).await;
    assert!(state.installed_message().is_some());
    assert_eq!(state.publications().len(), 1);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn predecessor_schema_five_retains_actual_claims_and_obtained_bytes_without_automatic_upgrade()
 {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 224, false).await;
    let attempt = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    capture(&store, &input, &proof, &attempt).await;
    store.close().await.unwrap();
    let mut database = fixture.database().await;
    sqlx::raw_sql(
        "DROP TABLE chat_publication_work; DROP TABLE chat_messages; PRAGMA user_version=5",
    )
    .execute(&mut database)
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_schema")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(count, 11);
    database.close().await.unwrap();
    let path = fixture.path.join("execution_v1.sqlite");
    let original = std::fs::read(&path).unwrap();
    assert!(matches!(
        NativeWorkspace::open_existing(
            &fixture.path,
            input,
            fixture.crypto(),
            fixture.clock.clone()
        )
        .await,
        Err(WorkspaceError::RecoveryRequired)
    ));
    assert_eq!(std::fs::read(path).unwrap(), original);
}
