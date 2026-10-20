use super::*;
use radroots_storage::command::EvidenceCaptureOutcome;

struct Issued {
    fixture: Fixture,
    store: Arc<NativeWorkspace>,
    input: AttachmentInput,
    proof: AttachmentProof,
    binding: WorkspaceBinding,
    attempt: SigningAttempt,
    evidence: SealEvidence,
}
impl Issued {
    async fn new(id: u8, large: bool) -> Self {
        let fixture = Fixture::new();
        let (store, input, proof, binding) = fixture.create().await;
        let (prepared, digest, recipients) =
            prepare(&fixture, &store, &binding, &proof, id, large).await;
        let attempt = claim(
            store
                .claim_attempt(
                    selection(&binding, &prepared, digest, recipients[0]),
                    copy(&proof),
                )
                .await
                .unwrap(),
        );
        let original = obtain(&attempt, true).await;
        let raw = format!(" \n{}\t", original.raw_json());
        let evidence =
            SealEvidence::verify(&attempt, &raw, original.observed_at_unix_ms()).unwrap();
        Self {
            fixture,
            store,
            input,
            proof,
            binding,
            attempt,
            evidence,
        }
    }
    async fn assert_same_writer(&self) {
        assert!(matches!(
            NativeWorkspace::open_existing(
                &self.fixture.path,
                self.input.clone(),
                self.fixture.crypto(),
                self.fixture.clock.clone()
            )
            .await,
            Err(WorkspaceError::WriterActive)
        ));
    }
}
fn confirmed(outcome: EvidenceCaptureOutcome) -> radroots_storage::command::EvidenceReceipt {
    match outcome {
        EvidenceCaptureOutcome::Committed(receipt) => receipt,
        _ => panic!("actual original capture must confirm"),
    }
}

#[tokio::test]
async fn original_capture_recovery_retries_known_rollback_without_reissuing_and_preserves_exact_bytes()
 {
    let issued = Issued::new(101, false).await;
    let original = issued.evidence.raw_json().to_owned();
    let mut database = issued.fixture.database().await;
    sqlx::query("CREATE TRIGGER capture_insert_fault BEFORE INSERT ON seal_evidence BEGIN SELECT RAISE(ABORT,'synthetic capture fault'); END").execute(&mut database).await.unwrap();
    assert!(
        issued
            .store
            .capture_evidence(issued.input.clone(), &issued.proof, &issued.evidence)
            .await
            .is_err()
    );
    assert!(
        issued
            .store
            .reconcile_unknown(issued.input.clone(), &issued.proof, &issued.evidence)
            .await
            .is_err()
    );
    issued.assert_same_writer().await;
    sqlx::query("DROP TRIGGER capture_insert_fault")
        .execute(&mut database)
        .await
        .unwrap();
    let first = confirmed(
        issued
            .store
            .reconcile_unknown(issued.input.clone(), &issued.proof, &issued.evidence)
            .await
            .unwrap(),
    );
    assert_eq!(first.observed_at_unix_ms(), None);
    let replay = confirmed(
        issued
            .store
            .reconcile_unknown(issued.input.clone(), &issued.proof, &issued.evidence)
            .await
            .unwrap(),
    );
    assert_eq!(first, replay);
    let retained = issued
        .store
        .obtained_evidence(
            selection(
                &issued.binding,
                issued.attempt.bound_plan().prepared(),
                *issued.attempt.bound_plan().plan().digest(),
                issued.attempt.seal().unwrap().recipient(),
            ),
            copy(&issued.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.evidence().raw_json(), original);
    assert_eq!(retained.evidence().observed_at_unix_ms(), None);
    database.close().await.unwrap();
    issued.store.close().await.unwrap();
}

#[tokio::test]
async fn unknown_capture_recovery_preserves_writer_during_refused_preflight_and_resumes_exact_original()
 {
    let issued = Issued::new(102, true).await;
    let original = issued.evidence.raw_json().to_owned();
    let mut database = issued.fixture.database().await;
    sqlx::raw_sql("CREATE TABLE capture_commit_fault (parent INTEGER REFERENCES workspace_grant(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER capture_commit_fault_trigger AFTER INSERT ON seal_evidence BEGIN INSERT INTO capture_commit_fault VALUES(2); END").execute(&mut database).await.unwrap();
    assert!(matches!(
        issued
            .store
            .capture_evidence(issued.input.clone(), &issued.proof, &issued.evidence)
            .await,
        Ok(EvidenceCaptureOutcome::Unknown(_))
    ));
    assert!(
        issued
            .store
            .operation_snapshot(
                issued.binding.clone(),
                issued.evidence.operation_id(),
                None,
                copy(&issued.proof)
            )
            .await
            .is_err()
    );
    // The extra fault objects are deliberately outside the exact real schema.
    // Recovery closes the old connection but must retain namespace authority.
    assert!(
        issued
            .store
            .reconcile_unknown(issued.input.clone(), &issued.proof, &issued.evidence)
            .await
            .is_err()
    );
    issued.assert_same_writer().await;
    let wrong = AttachmentProof::from_protected_input(&[0; 32]).unwrap();
    assert_eq!(
        issued
            .store
            .reconcile_unknown(issued.input.clone(), &wrong, &issued.evidence)
            .await
            .unwrap_err()
            .code(),
        "unauthorized"
    );
    sqlx::raw_sql("DROP TRIGGER capture_commit_fault_trigger; DROP TABLE capture_commit_fault")
        .execute(&mut database)
        .await
        .unwrap();
    issued.fixture.clock.now.store(0, Ordering::SeqCst);
    let captured = confirmed(
        issued
            .store
            .reconcile_unknown(issued.input.clone(), &issued.proof, &issued.evidence)
            .await
            .unwrap(),
    );
    assert_eq!(captured.observed_at_unix_ms(), None);
    assert_eq!(captured.digest(), issued.evidence.digest());
    issued.fixture.clock.now.store(160, Ordering::SeqCst);
    let snapshot = issued
        .store
        .operation_snapshot(
            issued.binding.clone(),
            issued.evidence.operation_id(),
            Some(issued.evidence.command_digest()),
            copy(&issued.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        snapshot.prepared().issuance_window(),
        issued.attempt.bound_plan().prepared().issuance_window()
    );
    assert!(snapshot.installation().is_none());
    assert!(snapshot.delivery_attempts().is_empty());
    let retained = issued
        .store
        .obtained_evidence(
            selection(
                &issued.binding,
                issued.attempt.bound_plan().prepared(),
                *issued.attempt.bound_plan().plan().digest(),
                issued.attempt.seal().unwrap().recipient(),
            ),
            copy(&issued.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.evidence().raw_json(), original);
    assert_eq!(retained.evidence().observed_at_unix_ms(), None);
    issued.assert_same_writer().await;
    database.close().await.unwrap();
    issued.store.close().await.unwrap();
}
