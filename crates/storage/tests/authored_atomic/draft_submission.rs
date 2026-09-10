use super::*;
use radroots_storage::{
    atomic::AtomicCommitId,
    authored_draft::{
        AuthoredDraft, AuthoredDraftId, AuthoredDraftRevision, AuthoredDraftStage,
        AuthoredDraftStore,
    },
    authored_draft_query::AuthoredDraftScope,
    authored_draft_submission::{AuthoredDraftSource, PrepareFromDraft},
};
use sha2::{Digest, Sha256};

fn source() -> AuthoredDraft {
    AuthoredDraft::initial(
        AuthoredDraftId::new([8; 16]).unwrap(),
        *authored_plan().author().as_bytes(),
        "fixture.partial.v1",
        b"unfinished 0.".to_vec(),
        AuthoredDraftStage::Draft,
        None,
        9,
    )
    .unwrap()
    .with_scope(AuthoredDraftScope::new([5; 32]).unwrap())
    .unwrap()
}
fn request(source: &AuthoredDraft, key: u8, id: u8) -> PrepareFromDraft {
    let AuthoredAtomicCommand::Prepare(base) = prepare(7).0 else {
        unreachable!()
    };
    let operation_id = OperationInstanceId::new([id; 16]).unwrap();
    let artifact_id = AuthoredArtifactId::new([id; 16]).unwrap();
    let delivery_id = AuthoredDeliveryPlanId::new([id; 16]).unwrap();
    let preparation = PrepareAuthoredOperation::new(
        AuthoredOperation::new(operation_id, vec![artifact_id], 10).unwrap(),
        vec![
            AuthoredArtifact::planned(artifact_id, operation_id, 0, &authored_plan(), 10).unwrap(),
        ],
        vec![
            AuthoredDeliveryPlan::new(
                delivery_id,
                artifact_id,
                base.delivery_plans()[0].intent().clone(),
                10,
            )
            .unwrap(),
        ],
        base.input_digest(),
        10,
    )
    .unwrap();
    let payload = b"complete immutable semantic intent".to_vec();
    let mut intent = AuthoredDraft::reconstruct(
        AuthoredDraftId::new([id; 16]).unwrap(),
        AuthoredDraftRevision::INITIAL,
        *source.author(),
        "fixture.intent.v1",
        payload.clone(),
        Sha256::digest(&payload).into(),
        AuthoredDraftStage::Queued,
        Some(operation_id),
        10,
        10,
    )
    .unwrap();
    if let Some(scope) = source.scope() {
        intent = intent.with_scope(scope).unwrap();
    }
    PrepareFromDraft::new(
        AtomicCommitId::new([key; 16]).unwrap(),
        AuthoredDraftSource::capture(source).unwrap(),
        intent,
        preparation,
    )
    .unwrap()
}
fn command(request: PrepareFromDraft) -> AuthoredAtomicCommand {
    AuthoredAtomicCommand::PrepareFromDraft(Box::new(request))
}

#[test]
fn submitted_request_replays_before_cas_and_compares_actual_fields_without_trusting_digest() {
    block_on(async {
        let store = MemoryStorage::default();
        let source = source();
        store
            .append_authored_draft(source.clone(), None)
            .await
            .unwrap();
        let request = request(&source, 4, 1);
        let cmd = command(request.clone());
        let receipt = store.execute_authored(cmd.clone()).await.unwrap();
        assert_eq!(receipt.disposition(), AtomicCommitDisposition::Committed);
        assert_eq!(
            receipt.outcome(),
            &AuthoredAtomicOutcome::Submitted(Box::new(request.clone()))
        );
        assert_eq!(
            store.authored_draft_head(source.draft_id()).await.unwrap(),
            Some(source.clone())
        );
        assert_eq!(
            store
                .authored_draft_head(request.intent().draft_id())
                .await
                .unwrap(),
            Some(request.intent().clone())
        );
        let newer = source
            .successor(
                b"later editing".to_vec(),
                AuthoredDraftStage::Draft,
                None,
                11,
            )
            .unwrap();
        store
            .append_authored_draft(newer, Some(source.revision()))
            .await
            .unwrap();
        let replay = store.execute_authored(cmd.clone()).await.unwrap();
        assert_eq!(replay.disposition(), AtomicCommitDisposition::Replay);
        assert_eq!(replay.outcome(), receipt.outcome());
        // This is the exact ordinary command used by later Sync signing.
        let ordinary = AuthoredAtomicCommand::Prepare(request.preparation().clone());
        let ordinary_receipt = store.execute_authored(ordinary.clone()).await.unwrap();
        assert_eq!(
            ordinary_receipt.disposition(),
            AtomicCommitDisposition::Replay
        );
        assert!(matches!(
            ordinary_receipt.outcome(),
            AuthoredAtomicOutcome::Prepared { .. }
        ));
        assert!(!receipt.matches_command(&ordinary));
        assert!(!ordinary_receipt.matches_command(&cmd));
        let base = request.preparation();
        let old_plan = &base.delivery_plans()[0];
        let changed_plan = AuthoredDeliveryPlan::new(
            old_plan.plan_id(),
            old_plan.artifact_id(),
            AuthoredDeliveryIntent::new(
                "changed-delivery-request",
                old_plan.intent().target_set().clone(),
                SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
                101,
            )
            .unwrap(),
            10,
        )
        .unwrap();
        let changed = PrepareFromDraft::new(
            request.command_id(),
            request.source().clone(),
            request.intent().clone(),
            PrepareAuthoredOperation::new(
                base.operation().clone(),
                base.artifacts().to_vec(),
                vec![changed_plan],
                base.input_digest(),
                10,
            )
            .unwrap(),
        )
        .unwrap();
        let changed = command(changed);
        assert_eq!(cmd.commit_id(), changed.commit_id());
        assert_eq!(
            cmd.digest(),
            changed.digest(),
            "caller digest is intentionally unchanged"
        );
        assert_eq!(
            store.execute_authored(changed).await,
            Err(Error::AtomicCommitConflict)
        );
        let fresh = command(self::request(&source, 6, 2));
        assert_eq!(
            store.execute_authored(fresh.clone()).await,
            Err(Error::DraftRevisionConflict)
        );
        assert!(
            store
                .authored_receipt(fresh.commit_id())
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .authored_operation(OperationInstanceId::new([2; 16]).unwrap())
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            store
                .authored_receipt(cmd.commit_id())
                .await
                .unwrap()
                .unwrap()
                .outcome(),
            receipt.outcome()
        );
    });
}

#[test]
fn intentional_identical_submissions_use_distinct_commands_and_operations() {
    block_on(async {
        let store = MemoryStorage::default();
        let source = source();
        store
            .append_authored_draft(source.clone(), None)
            .await
            .unwrap();
        for (key, id) in [(1, 1), (2, 2)] {
            let value = request(&source, key, id);
            let receipt = store
                .execute_authored(command(value.clone()))
                .await
                .unwrap();
            assert_eq!(receipt.disposition(), AtomicCommitDisposition::Committed);
            assert_eq!(
                receipt.commit_id(),
                PrepareFromDraft::commit_id_for(source.author(), value.command_id())
            );
        }
        assert_eq!(
            store
                .authored_draft_heads(*source.author(), 10)
                .await
                .unwrap()
                .len(),
            3
        );
    });
}

#[test]
fn submission_wire_and_receipts_validate_bindings_and_redact_payloads() {
    let source = source();
    let request = request(&source, 1, 2);
    let wire = serde_json::to_value(&request).unwrap();
    assert_eq!(
        serde_json::from_value::<PrepareFromDraft>(wire.clone()).unwrap(),
        request
    );
    assert!(!format!("{:?}", command(request.clone())).contains("semantic intent"));
    assert!(!format!("{request:?}").contains("atomic authored plan"));
    assert_eq!(request.source().draft_id(), source.draft_id());
    assert_eq!(request.source().revision(), source.revision());
    assert_eq!(request.source().author(), source.author());
    assert_eq!(request.source().payload_schema(), source.payload_schema());
    assert_eq!(request.source().scope(), source.scope());
    assert_eq!(request.source().payload_sha256(), source.payload_sha256());
    let invalid = [
        ("/command_id", serde_json::json!([0; 16].to_vec())),
        ("/source/author", serde_json::json!([0; 32].to_vec())),
        ("/source/payload_schema", serde_json::json!(" invalid")),
        ("/source/stage", serde_json::json!("queued")),
        ("/source/created_at_unix_ms", serde_json::json!(0)),
        ("/source/updated_at_unix_ms", serde_json::json!(8)),
        ("/source/updated_at_unix_ms", serde_json::json!(11)),
        ("/source/draft_id", serde_json::json!([2; 16].to_vec())),
        ("/source/author", serde_json::json!([1; 32].to_vec())),
        ("/source/scope", serde_json::Value::Null),
        ("/intent/revision", serde_json::json!(2)),
        ("/intent/operation_id", serde_json::json!([3; 16].to_vec())),
        ("/intent/updated_at_unix_ms", serde_json::json!(11)),
        ("/preparation/requested_at_unix_ms", serde_json::json!(0)),
        ("/preparation/artifacts", serde_json::json!([])),
    ];
    for (pointer, replacement) in invalid {
        let mut invalid = wire.clone();
        *invalid.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            serde_json::from_value::<PrepareFromDraft>(invalid).is_err(),
            "{pointer}"
        );
    }
    let cmd = command(request.clone());
    let outcome = AuthoredAtomicOutcome::Submitted(Box::new(request));
    for (id, digest, at) in [
        (AtomicCommitId::new([7; 16]).unwrap(), cmd.digest(), 10),
        (cmd.commit_id(), AtomicCommitDigest::new([7; 32]), 10),
        (cmd.commit_id(), cmd.digest(), 9),
    ] {
        assert!(
            AuthoredAtomicReceipt::from_durable_parts(
                id,
                digest,
                AtomicCommitDisposition::Committed,
                at,
                outcome.clone()
            )
            .is_err()
        );
    }
}

#[test]
fn submission_rejects_preexisting_work_state_and_mismatched_captured_times() {
    let source = source();
    let request = request(&source, 1, 2);
    let base = request.preparation();
    let reject = |operation: AuthoredOperation,
                  artifacts: Vec<AuthoredArtifact>,
                  plans: Vec<AuthoredDeliveryPlan>| {
        let preparation =
            PrepareAuthoredOperation::new(operation, artifacts, plans, base.input_digest(), 10)
                .unwrap();
        assert_eq!(
            PrepareFromDraft::new(
                request.command_id(),
                request.source().clone(),
                request.intent().clone(),
                preparation
            ),
            Err(Error::AtomicWorkflowMismatch)
        );
    };
    for (created, updated, revision) in [(9, 10, 1), (10, 11, 1), (10, 10, 2)] {
        reject(
            AuthoredOperation::reconstruct(
                base.operation().operation_id(),
                base.operation().artifact_ids().to_vec(),
                created,
                updated,
                NonZeroU64::new(revision).unwrap(),
            )
            .unwrap(),
            base.artifacts().to_vec(),
            base.delivery_plans().to_vec(),
        );
    }
    let artifact = &base.artifacts()[0];
    let mut already_signed = artifact.clone();
    already_signed
        .record_signed(signed(&authored_plan()), 10)
        .unwrap();
    let mut claimed = artifact.clone();
    claimed
        .set_signing_claim(
            WorkClaim::new(
                [1; 16],
                "existing worker",
                NonZeroU64::MIN,
                10,
                20,
                NonZeroU64::MIN,
            )
            .unwrap(),
            10,
        )
        .unwrap();
    let mut later_wire = serde_json::to_value(artifact).unwrap();
    later_wire["updated_at_unix_ms"] = serde_json::json!(11);
    let other_author = AuthoredEventPlan::from_generic(
        GenericEventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            1_800_000_100,
            vec![],
            "another author",
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )
        .unwrap(),
    )
    .unwrap();
    for changed in [
        AuthoredArtifact::imported_signed(
            artifact.artifact_id(),
            artifact.operation_id(),
            0,
            signed(&authored_plan()),
            10,
        )
        .unwrap(),
        already_signed,
        claimed,
        AuthoredArtifact::planned(
            artifact.artifact_id(),
            artifact.operation_id(),
            0,
            &authored_plan(),
            9,
        )
        .unwrap(),
        serde_json::from_value(later_wire).unwrap(),
        AuthoredArtifact::planned(
            artifact.artifact_id(),
            artifact.operation_id(),
            0,
            &other_author,
            10,
        )
        .unwrap(),
    ] {
        reject(
            base.operation().clone(),
            vec![changed],
            base.delivery_plans().to_vec(),
        );
    }
    let plan = &base.delivery_plans()[0];
    let mut cancelled = plan.clone();
    cancelled.cancel(10).unwrap();
    let mut later_wire = serde_json::to_value(plan).unwrap();
    later_wire["updated_at_unix_ms"] = serde_json::json!(11);
    for changed in [
        cancelled,
        AuthoredDeliveryPlan::new(plan.plan_id(), plan.artifact_id(), plan.intent().clone(), 9)
            .unwrap(),
        serde_json::from_value(later_wire).unwrap(),
    ] {
        reject(
            base.operation().clone(),
            base.artifacts().to_vec(),
            vec![changed],
        );
    }
    let original = request.intent();
    for (stage, operation, created) in [
        (AuthoredDraftStage::Draft, None, 10),
        (AuthoredDraftStage::Queued, original.operation_id(), 9),
    ] {
        let intent = AuthoredDraft::reconstruct(
            original.draft_id(),
            original.revision(),
            *original.author(),
            original.payload_schema(),
            original.payload().to_vec(),
            *original.payload_sha256(),
            stage,
            operation,
            created,
            10,
        )
        .unwrap()
        .with_scope(original.scope().unwrap())
        .unwrap();
        assert_eq!(
            PrepareFromDraft::new(
                request.command_id(),
                request.source().clone(),
                intent,
                base.clone()
            ),
            Err(Error::AtomicWorkflowMismatch)
        );
    }
    let mut ready = serde_json::to_value(&request).unwrap();
    ready["intent"]["stage"] = serde_json::json!("ready_to_sign");
    let ready: PrepareFromDraft = serde_json::from_value(ready).unwrap();
    assert_eq!(ready.intent().stage(), AuthoredDraftStage::ReadyToSign);
    let cmd = command(request.clone());
    let ordinary = AuthoredAtomicCommand::Prepare(base.clone());
    let submitted = AuthoredAtomicOutcome::Submitted(Box::new(request.clone()));
    assert!(
        AuthoredAtomicReceipt::new(&ordinary, AtomicCommitDisposition::Committed, 10, submitted)
            .is_err()
    );
    let prepared = AuthoredAtomicOutcome::Prepared {
        operation: base.operation().clone(),
        artifacts: base.artifacts().to_vec(),
        delivery_plans: base.delivery_plans().to_vec(),
    };
    assert!(
        AuthoredAtomicReceipt::new(&cmd, AtomicCommitDisposition::Committed, 10, prepared).is_err()
    );
}

#[test]
fn submission_cannot_adopt_unassociated_existing_intent_or_operation() {
    block_on(async {
        let source = source();
        let request = request(&source, 1, 2);
        let empty = MemoryStorage::default();
        assert_eq!(
            empty.execute_authored(command(request.clone())).await,
            Err(Error::DraftRevisionConflict)
        );
        for existing_intent in [true, false] {
            let store = MemoryStorage::default();
            store
                .append_authored_draft(source.clone(), None)
                .await
                .unwrap();
            if existing_intent {
                store
                    .append_authored_draft(request.intent().clone(), None)
                    .await
                    .unwrap();
            } else {
                store
                    .execute_authored(AuthoredAtomicCommand::Prepare(
                        request.preparation().clone(),
                    ))
                    .await
                    .unwrap();
            }
            assert_eq!(
                store.execute_authored(command(request.clone())).await,
                Err(if existing_intent {
                    Error::DraftRevisionConflict
                } else {
                    Error::AtomicCommitConflict
                })
            );
            assert!(
                store
                    .authored_receipt(request.commit_id())
                    .await
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                store.authored_draft_head(source.draft_id()).await.unwrap(),
                Some(source.clone())
            );
            assert_eq!(
                store
                    .authored_operation(request.preparation().operation().operation_id())
                    .await
                    .unwrap()
                    .is_some(),
                !existing_intent
            );
        }
    });
}
