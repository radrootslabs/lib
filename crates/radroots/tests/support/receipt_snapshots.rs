use super::issuance::{claim, obtain, prepare, selection};
use super::*;
use radroots_execution::{
    BusinessInstallation, Delivery, Issuance, OperationReceipt, RecoveryRequirement,
    WaitTermination,
};
use radroots_storage::command::{OperationSnapshot, RecipientSnapshot, RetainedOperation};

#[tokio::test]
async fn structural_reports_reject_incomplete_recipient_sets_and_cross_operation_evidence() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 180, false).await;
    let first = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let bytes = obtain(&first, true).await;
    store
        .capture_evidence(input.clone(), &proof, &bytes)
        .await
        .unwrap();
    let original = store
        .operation_snapshot(
            binding.clone(),
            prepared.identity().operation_id(),
            None,
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    let (other, other_digest, other_recipients) =
        prepare(&fixture, &store, &binding, &proof, 181, false).await;
    let other_claim = claim(
        store
            .claim_attempt(
                selection(&binding, &other, other_digest, other_recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let other_bytes = obtain(&other_claim, true).await;
    store
        .capture_evidence(input, &proof, &other_bytes)
        .await
        .unwrap();
    let other = store
        .operation_snapshot(
            binding.clone(),
            other.identity().operation_id(),
            None,
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    let first = original.recipients()[0];
    let second = original.recipients()[1];
    let foreign = other.recipients()[0];
    let cases = [
        (vec![], 100),
        (vec![first], 100),
        (vec![first, second, first], 100),
        (vec![second, first], 100),
        (vec![first, first], 100),
        (vec![first, second], 0),
        (
            vec![
                RecipientSnapshot::from_store_report(
                    first.recipient(),
                    None,
                    first.claim(),
                    first.obtained(),
                ),
                second,
            ],
            100,
        ),
        (
            vec![
                RecipientSnapshot::from_store_report(
                    first.recipient(),
                    first.expected_event(),
                    None,
                    first.obtained(),
                ),
                second,
            ],
            100,
        ),
        (
            vec![
                RecipientSnapshot::from_store_report(
                    first.recipient(),
                    first.expected_event(),
                    foreign.claim(),
                    foreign.obtained(),
                ),
                second,
            ],
            100,
        ),
        (
            vec![
                RecipientSnapshot::from_store_report(
                    first.recipient(),
                    first.expected_event(),
                    first.claim(),
                    foreign.obtained(),
                ),
                second,
            ],
            100,
        ),
    ];
    for (recipients, time) in cases {
        let bound = store
            .signing_plan(
                binding.clone(),
                prepared.identity().operation_id(),
                prepared.identity().command_digest(),
                copy(&proof),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            OperationSnapshot::from_store_report(
                RetainedOperation::Planned(Box::new(bound)),
                recipients,
                UnixSeconds::new(time).unwrap()
            ),
            Err(OperationError::RecoveryRequired)
        ));
    }
    let report = store
        .operation(
            binding,
            prepared.identity().operation_id(),
            None,
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        OperationSnapshot::from_store_report(
            RetainedOperation::Prepared(report),
            original.recipients().to_vec(),
            UnixSeconds::new(100).unwrap()
        ),
        Err(OperationError::RecoveryRequired)
    ));
    store.close().await.unwrap();
}

#[tokio::test]
async fn expiry_blocks_remaining_first_issuance_without_erasing_obtained_or_unknown_facts() {
    for mode in 0u8..=3 {
        let fixture = Fixture::new();
        let (store, input, proof, binding) = fixture.create().await;
        let (prepared, planned) = if mode == 0 {
            (
                committed(
                    store
                        .admit_command_and_hold(peer_request(&binding, 174), copy(&proof))
                        .await
                        .unwrap(),
                ),
                None,
            )
        } else {
            let (prepared, digest, recipients) =
                prepare(&fixture, &store, &binding, &proof, 174 + mode, false).await;
            (prepared, Some((digest, recipients)))
        };
        if let Some((digest, recipients)) = planned
            && mode >= 2
        {
            for recipient in recipients.into_iter().take(if mode == 2 { 1 } else { 2 }) {
                let original = claim(
                    store
                        .claim_attempt(
                            selection(&binding, &prepared, digest, recipient),
                            copy(&proof),
                        )
                        .await
                        .unwrap(),
                );
                if mode == 3 {
                    let bytes = obtain(&original, true).await;
                    store
                        .capture_evidence(input.clone(), &proof, &bytes)
                        .await
                        .unwrap();
                }
            }
        }
        fixture.clock.now.store(151, Ordering::SeqCst);
        let snapshot = store
            .operation_snapshot(
                binding,
                prepared.identity().operation_id(),
                None,
                copy(&proof),
            )
            .await
            .unwrap()
            .unwrap();
        let receipt = OperationReceipt::from_snapshot(&snapshot);
        assert!(receipt.durable());
        assert_eq!(
            receipt.admission_transaction_id(),
            prepared.transaction_id()
        );
        assert_eq!(
            receipt.business_installation(),
            BusinessInstallation::NotInstalled
        );
        assert_eq!(receipt.delivery(), Delivery::NotStarted);
        match mode {
            0 | 1 => {
                assert_eq!(
                    receipt.error().unwrap().code(),
                    radroots_execution::Code::DeadlineExceeded
                );
                assert!(
                    receipt
                        .recovery()
                        .contains(&RecoveryRequirement::ManualReconciliation)
                );
                assert!(!matches!(
                    receipt.issuance(),
                    Issuance::Obtained | Issuance::ProvablyUnissued
                ));
            }
            2 => {
                assert_eq!(receipt.issuance(), Issuance::Unknown);
                assert_eq!(
                    receipt.error().unwrap().code(),
                    radroots_execution::Code::SigningUnknown
                );
                assert!(
                    receipt
                        .recovery()
                        .contains(&RecoveryRequirement::ReconcileIssuance)
                );
            }
            3 => {
                assert_eq!(receipt.issuance(), Issuance::Obtained);
                assert_eq!(receipt.error(), None);
            }
            _ => unreachable!(),
        }
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn full_receipts_keep_obtained_and_unknown_recipients_separate_and_bind_original_provenance()
{
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 173, false).await;
    let first = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let first_bytes = obtain(&first, true).await;
    store
        .capture_evidence(input.clone(), &proof, &first_bytes)
        .await
        .unwrap();
    let second = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[1]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let snapshot = store
        .operation_snapshot(
            binding.clone(),
            prepared.identity().operation_id(),
            None,
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    let view = OperationReceipt::from_snapshot(&snapshot);
    assert_eq!(view.command_digest(), prepared.identity().command_digest());
    assert_eq!(view.artifacts()[0].issuance(), Issuance::Obtained);
    assert_eq!(view.artifacts()[1].issuance(), Issuance::Unknown);
    assert_eq!(view.issuance(), Issuance::Unknown);
    assert_eq!(
        view.business_installation(),
        BusinessInstallation::NotInstalled
    );
    assert_eq!(view.business_result(), None);
    assert_eq!(view.delivery(), Delivery::NotStarted);
    assert!(view.delivery_observations().is_empty());
    assert!(view.conflicting_records().is_empty());
    assert!(view.conflicting_sources().is_empty());
    assert_eq!(
        view.missing_sources(),
        binding.input().checkpoint().missing()
    );
    assert!(
        view.recovery()
            .contains(&RecoveryRequirement::InstallObtainedEvidence)
    );
    assert!(
        view.recovery()
            .contains(&RecoveryRequirement::ReconcileIssuance)
    );
    let original = &view.artifacts()[0];
    assert_eq!(
        original.role(),
        radroots_protocol::intent::ArtifactRole::BusinessSeal
    );
    assert_eq!(original.recipient(), Some(first.seal().recipient()));
    assert_eq!(
        original.expected_event().unwrap().as_bytes(),
        &first.seal().event_id()
    );
    assert_eq!(original.attempt_id(), Some(first.attempt_id()));
    assert_eq!(
        original.claim_transaction_id(),
        Some(first.transaction_id())
    );
    assert_eq!(original.intent_digest(), Some(*first.binding().digest()));
    assert_eq!(original.obtained_digest(), Some(*first_bytes.digest()));
    assert_eq!(
        original.capture_transaction_id(),
        snapshot.recipients()[0]
            .obtained()
            .map(|value| value.transaction_id())
    );
    assert_eq!(original.observed_at_unix_ms(), None);
    let second_bytes = obtain(&second, false).await;
    store
        .capture_evidence(input, &proof, &second_bytes)
        .await
        .unwrap();
    let snapshot = store
        .operation_snapshot(
            binding.clone(),
            prepared.identity().operation_id(),
            None,
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    let completed = OperationReceipt::from_snapshot(&snapshot);
    assert_eq!(completed.issuance(), Issuance::Obtained);
    assert_eq!(completed.error(), None);
    assert_eq!(
        completed.artifacts()[1].observed_at_unix_ms(),
        second_bytes.observed_at_unix_ms()
    );
    assert_ne!(
        completed.operation_checkpoint(),
        view.operation_checkpoint()
    );
    let checkpoint = completed.operation_checkpoint();
    fixture.clock.now.store(110, Ordering::SeqCst);
    let later = store
        .operation_snapshot(
            binding,
            prepared.identity().operation_id(),
            None,
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    let later = OperationReceipt::from_snapshot(&later);
    assert_eq!(later.observed_at().get(), 110);
    assert_eq!(later.operation_checkpoint(), checkpoint);
    assert_eq!(later.artifacts()[1].attempt_id(), Some(second.attempt_id()));
    assert_eq!(later.issuance_window(), prepared.issuance_window());
    store.close().await.unwrap();
}

#[tokio::test]
async fn coherent_snapshots_retain_t0_t1_and_independent_recipient_claim_and_capture_after_reopen()
{
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let request = peer_request(&binding, 170);
    let operation = request.command().context().operation_id();
    let digest = request.command().digest();
    assert!(
        store
            .operation_snapshot(binding.clone(), operation, Some(digest), copy(&proof))
            .await
            .unwrap()
            .is_none()
    );
    let prepared = committed(
        store
            .admit_command_and_hold(request, copy(&proof))
            .await
            .unwrap(),
    );
    let snapshot = store
        .operation_snapshot(binding.clone(), operation, None, copy(&proof))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        snapshot.prepared().transaction_id(),
        prepared.transaction_id()
    );
    assert!(snapshot.bound_plan().is_none());
    let receipt = OperationReceipt::from_snapshot(&snapshot);
    assert!(receipt.durable());
    assert_eq!(receipt.issuance(), Issuance::None);
    assert_eq!(
        receipt.business_installation(),
        BusinessInstallation::NotInstalled
    );
    assert_eq!(receipt.delivery(), Delivery::NotStarted);
    assert!(receipt.business_result().is_none());
    assert!(receipt.record_ids().is_empty());
    assert_eq!(receipt.context(), prepared.request().command().context());
    assert_eq!(
        receipt.admission_transaction_id(),
        prepared.transaction_id()
    );
    assert_eq!(receipt.issuance_window(), prepared.issuance_window());
    assert_eq!(receipt.authority_checkpoint(), binding.input().checkpoint());
    let initial_checkpoint = receipt.operation_checkpoint();
    assert_eq!(snapshot.message_id(), None);
    assert_eq!(snapshot.recipients().len(), 2);
    assert!(
        snapshot
            .recipients()
            .iter()
            .all(|value| value.expected_event().is_none()
                && value.claim().is_none()
                && value.obtained().is_none())
    );
    let plan = plan(prepared.request().command(), 100, "snapshot");
    let plan_digest = *plan.digest();
    let message = plan.message_id();
    let plan_receipt = bound(
        store
            .bind_signing_intents(binding.clone(), plan, copy(&proof))
            .await
            .unwrap(),
    );
    let snapshot = store
        .operation_snapshot(binding.clone(), operation, Some(digest), copy(&proof))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.message_id(), Some(message));
    let receipt = OperationReceipt::from_snapshot(&snapshot);
    assert_eq!(receipt.issuance(), Issuance::Pending);
    assert_ne!(receipt.operation_checkpoint(), initial_checkpoint);
    assert_eq!(
        snapshot.bound_plan().unwrap().transaction_id(),
        plan_receipt.transaction_id()
    );
    assert!(
        snapshot
            .recipients()
            .iter()
            .all(|value| value.expected_event().is_some() && value.claim().is_none())
    );
    let first = snapshot.recipients()[0].recipient();
    let second = snapshot.recipients()[1].recipient();
    let attempt = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, plan_digest, first),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let snapshot = store
        .operation_snapshot(binding.clone(), operation, None, copy(&proof))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        snapshot.recipients()[0].claim().unwrap().attempt_id(),
        attempt.attempt_id()
    );
    assert!(snapshot.recipients()[0].obtained().is_none());
    let receipt = OperationReceipt::from_snapshot(&snapshot);
    assert_eq!(receipt.issuance(), Issuance::Unknown);
    assert_eq!(receipt.artifacts()[0].issuance(), Issuance::Unknown);
    assert_eq!(receipt.artifacts()[1].issuance(), Issuance::Pending);
    assert_eq!(
        receipt.error().unwrap().code(),
        radroots_execution::Code::SigningUnknown
    );
    assert!(
        receipt
            .recovery()
            .contains(&RecoveryRequirement::ReconcileIssuance)
    );
    assert!(snapshot.recipients()[1].claim().is_none());
    let evidence = obtain(&attempt, true).await;
    let receipt = match store
        .capture_evidence(input.clone(), &proof, &evidence)
        .await
        .unwrap()
    {
        radroots_storage::command::EvidenceCaptureOutcome::Committed(value) => value,
        _ => panic!("capture must confirm"),
    };
    let snapshot = store
        .operation_snapshot(binding.clone(), operation, None, copy(&proof))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.recipients()[0].obtained(), Some(receipt));
    let view = OperationReceipt::from_snapshot(&snapshot);
    assert_eq!(view.issuance(), Issuance::Pending);
    assert_eq!(view.artifacts()[0].issuance(), Issuance::Obtained);
    assert_eq!(view.artifacts()[1].issuance(), Issuance::Pending);
    assert_eq!(view.artifacts()[0].observed_at_unix_ms(), None);
    assert!(
        view.recovery()
            .contains(&RecoveryRequirement::InstallObtainedEvidence)
    );
    assert_eq!(view.error(), None);
    assert_eq!(format!("{view:?}"), "OperationReceipt(<redacted>)");
    assert_eq!(
        format!("{:?}", view.artifacts()[0]),
        "ArtifactObservation(<redacted>)"
    );
    let captured_checkpoint = view.operation_checkpoint();
    let stopped = view.with_wait_termination(WaitTermination::DeadlineExceeded);
    assert_eq!(
        stopped.wait_termination(),
        WaitTermination::DeadlineExceeded
    );
    assert_eq!(stopped.operation_checkpoint(), captured_checkpoint);
    assert_eq!(stopped.issuance(), Issuance::Pending);
    assert_eq!(
        stopped.business_installation(),
        BusinessInstallation::NotInstalled
    );
    assert_eq!(stopped.delivery(), Delivery::NotStarted);
    assert!(stopped.durable());
    assert_eq!(
        snapshot.recipients()[0]
            .obtained()
            .unwrap()
            .observed_at_unix_ms(),
        None
    );
    assert_eq!(snapshot.recipients()[1].recipient(), second);
    assert!(snapshot.recipients()[1].claim().is_none());
    assert_eq!(format!("{snapshot:?}"), "OperationSnapshot(<redacted>)");
    store.close().await.unwrap();
    fixture.clock.now.store(151, Ordering::SeqCst);
    let reopened = fixture.reopen(input.clone()).await;
    let snapshot = reopened
        .operation_snapshot(binding.clone(), operation, Some(digest), copy(&proof))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.prepared().identity(), prepared.identity());
    assert_eq!(snapshot.message_id(), Some(message));
    assert_eq!(snapshot.recipients()[0].obtained(), Some(receipt));
    let view = OperationReceipt::from_snapshot(&snapshot);
    assert_eq!(view.operation_checkpoint(), captured_checkpoint);
    assert_eq!(view.message_id(), Some(message));
    assert!(snapshot.recipients()[1].obtained().is_none());
    assert!(matches!(
        reopened
            .operation_snapshot(
                binding.clone(),
                operation,
                Some(CommandDigest::from_bytes([0; 32])),
                copy(&proof)
            )
            .await,
        Err(OperationError::IdempotencyConflict)
    ));
    reopened.revoke(input, &proof).await.unwrap();
    assert!(matches!(
        reopened
            .operation_snapshot(binding, operation, None, copy(&proof))
            .await,
        Err(OperationError::Workspace(WorkspaceError::GrantRevoked))
    ));
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn complete_snapshot_rejects_extra_recipient_claims_and_orphan_parts_without_repair() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 171, false).await;
    claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let mut database = fixture.database().await;
    sqlx::query("PRAGMA foreign_keys=OFF")
        .execute(&mut database)
        .await
        .unwrap();
    let third = keys(3).public_key();
    for number in 3u8..=12 {
        sqlx::query("INSERT INTO signing_attempts(workspace,operation_id,recipient,attempt_id,transaction_id,plan_digest,intent_digest,protected) SELECT workspace,operation_id,?,?,?,plan_digest,intent_digest,protected FROM signing_attempts LIMIT 1")
            .bind(keys(number).public_key().as_bytes().as_slice()).bind([number;32].as_slice()).bind([number;32].as_slice()).execute(&mut database).await.unwrap();
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
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM signing_attempts")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(count, 11);
    sqlx::query("DELETE FROM signing_attempts WHERE recipient!=?")
        .bind(recipients[0].as_bytes().as_slice())
        .execute(&mut database)
        .await
        .unwrap();
    for number in 3u8..=12 {
        sqlx::query("INSERT INTO seal_evidence_parts(workspace,operation_id,recipient,part,protected) VALUES(?,?,?,1,zeroblob(1))")
            .bind(binding.input().workspace_id().as_bytes().as_slice()).bind(prepared.identity().operation_id().as_bytes().as_slice()).bind(keys(number).public_key().as_bytes().as_slice()).execute(&mut database).await.unwrap();
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
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM seal_evidence_parts")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(count, 10);
    sqlx::query("DELETE FROM seal_evidence_parts")
        .execute(&mut database)
        .await
        .unwrap();
    let missing = OperationId::from_bytes([237; 32]);
    sqlx::query("INSERT INTO seal_evidence_parts(workspace,operation_id,recipient,part,protected) VALUES(?,?,?,1,zeroblob(1))")
        .bind(binding.input().workspace_id().as_bytes().as_slice()).bind(missing.as_bytes().as_slice()).bind(third.as_bytes().as_slice()).execute(&mut database).await.unwrap();
    assert!(matches!(
        store
            .operation_snapshot(binding.clone(), missing, None, copy(&proof))
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM seal_evidence_parts")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(count, 1);
    sqlx::query("DELETE FROM seal_evidence_parts")
        .execute(&mut database)
        .await
        .unwrap();
    assert!(
        store
            .operation_snapshot(
                binding,
                prepared.identity().operation_id(),
                None,
                copy(&proof)
            )
            .await
            .unwrap()
            .is_some()
    );
    database.close().await.unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn losing_a_snapshot_observer_does_not_detach_the_owner_or_prevent_safe_close() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, _, _) = prepare(&fixture, &store, &binding, &proof, 172, false).await;
    let (arrived, resume) = fixture.clock.arm(1);
    let mut query = store.operation_snapshot(
        binding.clone(),
        prepared.identity().operation_id(),
        None,
        copy(&proof),
    );
    assert!(futures::poll!(query.as_mut()).is_pending());
    reached(arrived).await;
    drop(query);
    resume.send(()).unwrap();
    store.close().await.unwrap();
    let reopened = fixture.reopen(input).await;
    assert_eq!(
        reopened
            .operation_snapshot(
                binding,
                prepared.identity().operation_id(),
                None,
                copy(&proof)
            )
            .await
            .unwrap()
            .unwrap()
            .prepared()
            .transaction_id(),
        prepared.transaction_id()
    );
    reopened.close().await.unwrap();
}
