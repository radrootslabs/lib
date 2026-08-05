use core::num::{NonZeroU32, NonZeroU64};
use futures_executor::block_on;
use radroots_event::{GenericEventDraft, SignedEvent, wire::v1::Nip01EventWire};
use radroots_event_codec::authoring::AuthoredEventPlan;
use radroots_storage::{
    Error,
    atomic::{AtomicCommitDigest, AtomicCommitDisposition},
    authored::{
        AdmissionState, AuthoredArtifact, AuthoredArtifactId, AuthoredOperation, FailureClass,
        RetrySchedule, SigningState, WorkClaim, WorkFailure, WorkPhase,
    },
    authored_atomic::{
        ApplyAdmissionResult, ApplyDeliveryAttempt, ApplySignedArtifact, ApplyWorkFailure,
        AuthoredAtomicCommand, AuthoredAtomicOutcome, AuthoredAtomicReceipt, AuthoredAtomicStorage,
        AuthoredWorkTarget, CancelAuthoredTarget, CancelAuthoredWork, ClaimAuthoredTarget,
        ClaimAuthoredWork, PrepareAuthoredOperation, WorkFence,
    },
    authored_delivery::{
        AuthoredDeliveryIntent, AuthoredDeliveryPlan, AuthoredDeliveryPlanId,
        AuthoredDeliveryState, DeliveryAttemptOutcome,
    },
    event::SourceGeneration,
    journal::OperationInstanceId,
    memory::MemoryStorage,
};
use radroots_transport::{
    DeliveryReceipt, SinkFailure, Target, TargetSet,
    outcome::{DeliveryOutcome, Retryability},
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::DeliveryTargetReceipt,
};

const AUTHOR: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

fn authored_plan() -> AuthoredEventPlan {
    AuthoredEventPlan::from_generic(
        GenericEventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            1_800_000_100,
            Vec::new(),
            "atomic authored plan",
            AUTHOR,
        )
        .expect("draft"),
    )
    .expect("plan")
}

fn signed(plan: &AuthoredEventPlan) -> SignedEvent {
    let wire = Nip01EventWire {
        id: plan.expected_event_id().to_hex(),
        pubkey: plan.author().to_hex(),
        created_at: plan.created_at(),
        kind: plan.body().kind(),
        tags: plan.body().tags().to_vec(),
        content: plan.body().content().to_owned(),
        sig: "dd".repeat(64),
        extra: Default::default(),
    };
    let raw = serde_json::to_string(&wire).expect("raw event");
    SignedEvent::from_wire_verified_id(wire, raw).expect("signed event")
}

fn ids() -> (
    OperationInstanceId,
    AuthoredArtifactId,
    AuthoredDeliveryPlanId,
) {
    (
        OperationInstanceId::new([1; 16]).expect("operation"),
        AuthoredArtifactId::new([2; 16]).expect("artifact"),
        AuthoredDeliveryPlanId::new([3; 16]).expect("delivery"),
    )
}

fn intent() -> AuthoredDeliveryIntent {
    AuthoredDeliveryIntent::new(
        "atomic-delivery",
        TargetSet::new(vec![
            Target::nostr_relay("wss://one.example").expect("one"),
            Target::nostr_relay("wss://two.example").expect("two"),
        ])
        .expect("targets"),
        SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::any()),
        100,
    )
    .expect("intent")
}

fn prepare(input: u8) -> (AuthoredAtomicCommand, AuthoredEventPlan) {
    let (operation_id, artifact_id, plan_id) = ids();
    let authored_plan = authored_plan();
    let artifact = AuthoredArtifact::planned(artifact_id, operation_id, 0, &authored_plan, 10)
        .expect("artifact");
    let operation = AuthoredOperation::new(operation_id, vec![artifact_id], 10).expect("operation");
    let delivery =
        AuthoredDeliveryPlan::new(plan_id, artifact_id, intent(), 10).expect("delivery plan");
    let prepare = PrepareAuthoredOperation::new(
        operation,
        vec![artifact],
        vec![delivery],
        AtomicCommitDigest::new([input; 32]),
        10,
    )
    .expect("prepare");
    (AuthoredAtomicCommand::Prepare(prepare), authored_plan)
}

fn claim(
    target: ClaimAuthoredTarget,
    revision: NonZeroU64,
    token: u8,
    at: u64,
) -> (AuthoredAtomicCommand, WorkClaim) {
    let claim = WorkClaim::new(
        [token; 16],
        format!("worker-{token}"),
        NonZeroU64::new(u64::from(token)).expect("generation"),
        at,
        at + 20,
        revision,
    )
    .expect("claim");
    (
        AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(target, claim.clone())),
        claim,
    )
}

fn fence(claim: &WorkClaim) -> WorkFence {
    WorkFence::new(*claim.token(), claim.generation(), claim.row_revision()).unwrap()
}

fn prepared_storage() -> (MemoryStorage, AuthoredEventPlan) {
    let storage = MemoryStorage::new(SourceGeneration::new([1; 32]).unwrap());
    let (prepare, plan) = prepare(7);
    block_on(storage.execute_authored(prepare)).unwrap();
    (storage, plan)
}

fn signed_storage() -> MemoryStorage {
    let (storage, plan) = prepared_storage();
    let artifact = block_on(storage.authored_artifact(ids().1))
        .unwrap()
        .unwrap();
    let (command, active) = claim(
        ClaimAuthoredTarget::ArtifactSigning(ids().1),
        artifact.revision(),
        4,
        11,
    );
    block_on(storage.execute_authored(command)).unwrap();
    let apply = ApplySignedArtifact::new(ids().1, fence(&active), signed(&plan), 12).unwrap();
    block_on(storage.execute_authored(AuthoredAtomicCommand::ApplySigned(apply))).unwrap();
    storage
}

fn claim_admission(storage: &MemoryStorage, token: u8, at: u64) -> WorkClaim {
    let artifact = block_on(storage.authored_artifact(ids().1))
        .unwrap()
        .unwrap();
    let (command, active) = claim(
        ClaimAuthoredTarget::ArtifactAdmission(ids().1),
        artifact.revision(),
        token,
        at,
    );
    block_on(storage.execute_authored(command)).unwrap();
    active
}

fn claim_delivery(storage: &MemoryStorage, token: u8, at: u64) -> WorkClaim {
    let plan = block_on(storage.authored_delivery_plan(ids().2))
        .unwrap()
        .unwrap();
    let (command, active) = claim(
        ClaimAuthoredTarget::DeliveryPlan(ids().2),
        plan.revision(),
        token,
        at,
    );
    block_on(storage.execute_authored(command)).unwrap();
    active
}

#[test]
fn preparation_is_atomic_deterministic_and_exactly_replayable() {
    let storage = MemoryStorage::new(SourceGeneration::new([1; 32]).expect("generation"));
    let (command, _) = prepare(7);
    assert_eq!(command.commit_id(), command.clone().commit_id());
    assert_eq!(command.digest(), command.clone().digest());

    let committed = block_on(storage.execute_authored(command.clone())).expect("commit");
    assert_eq!(committed.disposition(), AtomicCommitDisposition::Committed);
    let replay = block_on(storage.execute_authored(command.clone())).expect("replay");
    assert_eq!(replay.disposition(), AtomicCommitDisposition::Replay);
    assert_eq!(replay.outcome(), committed.outcome());
    assert_eq!(
        block_on(storage.authored_receipt(command.commit_id()))
            .expect("receipt")
            .expect("stored receipt")
            .outcome(),
        committed.outcome()
    );

    let (conflict, _) = prepare(8);
    assert_eq!(
        block_on(storage.execute_authored(conflict)),
        Err(Error::AtomicCommitConflict)
    );
    let operation = block_on(storage.authored_operation(ids().0))
        .expect("operation query")
        .expect("operation");
    assert_eq!(operation.artifact_ids(), &[ids().1]);
    let delivery = block_on(storage.authored_delivery_plan(ids().2))
        .expect("delivery query")
        .expect("delivery");
    assert!(delivery.request().is_none());
}

#[test]
fn signing_atomically_binds_exact_delivery_requests_and_rejects_stale_fences() {
    let storage = MemoryStorage::new(SourceGeneration::new([1; 32]).expect("generation"));
    let (prepare, plan) = prepare(7);
    block_on(storage.execute_authored(prepare)).expect("prepare");
    let artifact = block_on(storage.authored_artifact(ids().1))
        .expect("artifact query")
        .expect("artifact");
    let (claim_command, active) = claim(
        ClaimAuthoredTarget::ArtifactSigning(ids().1),
        artifact.revision(),
        4,
        11,
    );
    block_on(storage.execute_authored(claim_command)).expect("claim");

    let stale = ApplySignedArtifact::new(
        ids().1,
        WorkFence::new([8; 16], active.generation(), active.row_revision()).expect("fence"),
        signed(&plan),
        12,
    )
    .expect("stale apply");
    assert_eq!(
        block_on(storage.execute_authored(AuthoredAtomicCommand::ApplySigned(stale))),
        Err(Error::DeliveryPlanClaimConflict)
    );
    assert_eq!(
        block_on(storage.authored_artifact(ids().1))
            .expect("artifact query")
            .expect("artifact")
            .signing_state(),
        SigningState::Planned
    );

    let apply = ApplySignedArtifact::new(
        ids().1,
        WorkFence::new(*active.token(), active.generation(), active.row_revision()).expect("fence"),
        signed(&plan),
        12,
    )
    .expect("apply");
    let receipt = block_on(storage.execute_authored(AuthoredAtomicCommand::ApplySigned(apply)))
        .expect("signed");
    assert!(matches!(
        receipt.outcome(),
        AuthoredAtomicOutcome::Artifact(_)
    ));
    let artifact = block_on(storage.authored_artifact(ids().1))
        .expect("artifact query")
        .expect("artifact");
    assert_eq!(artifact.signing_state(), SigningState::Signed);
    let delivery = block_on(storage.authored_delivery_plan(ids().2))
        .expect("delivery query")
        .expect("delivery");
    assert_eq!(
        delivery
            .request()
            .expect("bound delivery")
            .payload()
            .event()
            .raw_json(),
        artifact
            .signed()
            .expect("signed artifact")
            .event()
            .raw_json()
    );
}

#[test]
fn delivery_attempt_and_work_failure_commands_preserve_atomic_state() {
    let storage = MemoryStorage::new(SourceGeneration::new([1; 32]).expect("generation"));
    let (prepare, plan) = prepare(7);
    block_on(storage.execute_authored(prepare)).expect("prepare");
    let artifact = block_on(storage.authored_artifact(ids().1))
        .expect("artifact")
        .expect("artifact");
    let (sign_claim, active) = claim(
        ClaimAuthoredTarget::ArtifactSigning(ids().1),
        artifact.revision(),
        4,
        11,
    );
    block_on(storage.execute_authored(sign_claim)).expect("claim signing");
    block_on(
        storage.execute_authored(AuthoredAtomicCommand::ApplySigned(
            ApplySignedArtifact::new(
                ids().1,
                WorkFence::new(*active.token(), active.generation(), active.row_revision())
                    .expect("fence"),
                signed(&plan),
                12,
            )
            .expect("apply"),
        )),
    )
    .expect("sign");

    let delivery = block_on(storage.authored_delivery_plan(ids().2))
        .expect("delivery")
        .expect("delivery");
    let (delivery_claim, active) = claim(
        ClaimAuthoredTarget::DeliveryPlan(ids().2),
        delivery.revision(),
        5,
        13,
    );
    block_on(storage.execute_authored(delivery_claim)).expect("claim delivery");
    let request = block_on(storage.authored_delivery_plan(ids().2))
        .expect("delivery")
        .expect("delivery")
        .request()
        .expect("bound request")
        .clone();
    let receipt = DeliveryReceipt::for_request(
        &request,
        request
            .target_set()
            .targets()
            .iter()
            .cloned()
            .map(|target| DeliveryTargetReceipt::attempted(target, DeliveryOutcome::accepted()))
            .collect(),
    )
    .expect("receipt");
    let apply = ApplyDeliveryAttempt::new(
        ids().2,
        WorkFence::new(*active.token(), active.generation(), active.row_revision()).expect("fence"),
        DeliveryAttemptOutcome::Receipt(receipt),
        None,
        14,
    )
    .expect("apply delivery");
    block_on(storage.execute_authored(AuthoredAtomicCommand::ApplyDelivery(apply)))
        .expect("deliver");
    assert_eq!(
        block_on(storage.authored_delivery_plan(ids().2))
            .expect("delivery")
            .expect("delivery")
            .state(),
        AuthoredDeliveryState::Satisfied
    );

    let retry_failure = WorkFailure::new(
        "temporary_signer_failure",
        WorkPhase::Signing,
        FailureClass::Retryable,
        Some(30),
        None,
    )
    .expect("failure");
    let retry = RetrySchedule::new(NonZeroU32::MIN, 30, retry_failure.clone()).expect("retry");
    let invalid = radroots_storage::authored_atomic::ApplyWorkFailure::new(
        AuthoredWorkTarget::Artifact(ids().1),
        WorkFence::new([9; 16], NonZeroU64::MIN, NonZeroU64::MIN).expect("fence"),
        retry_failure,
        Some(retry),
        15,
    )
    .expect("failure command");
    let before = block_on(storage.authored_artifact(ids().1))
        .expect("artifact")
        .expect("artifact");
    assert!(
        block_on(storage.execute_authored(AuthoredAtomicCommand::ApplyFailure(invalid))).is_err()
    );
    assert_eq!(
        block_on(storage.authored_artifact(ids().1))
            .expect("artifact")
            .expect("artifact"),
        before
    );
}

#[test]
fn authored_atomic_command_models_cover_every_phase_identity_and_durable_outcome() {
    assert_eq!(
        WorkFence::new([0; 16], NonZeroU64::MIN, NonZeroU64::MIN),
        Err(Error::InvalidWorkClaim)
    );
    let fence =
        WorkFence::new([7; 16], NonZeroU64::new(2).unwrap(), NonZeroU64::MIN).expect("fence");
    assert_eq!(fence.token(), &[7; 16]);
    assert_eq!(fence.generation(), NonZeroU64::new(2).unwrap());
    assert_eq!(fence.row_revision(), NonZeroU64::MIN);

    let (prepared_command, authored_plan) = prepare(7);
    let AuthoredAtomicCommand::Prepare(prepared) = prepared_command.clone() else {
        unreachable!()
    };
    assert_eq!(prepared.operation().operation_id(), ids().0);
    assert_eq!(prepared.artifacts().len(), 1);
    assert_eq!(prepared.delivery_plans().len(), 1);
    assert_eq!(prepared.input_digest(), AtomicCommitDigest::new([7; 32]));
    assert_eq!(prepared.requested_at_unix_ms(), 10);

    assert_eq!(
        PrepareAuthoredOperation::new(
            prepared.operation().clone(),
            Vec::new(),
            Vec::new(),
            AtomicCommitDigest::new([1; 32]),
            10,
        ),
        Err(Error::AtomicWorkflowMismatch)
    );
    assert_eq!(
        PrepareAuthoredOperation::new(
            prepared.operation().clone(),
            prepared.artifacts().to_vec(),
            prepared.delivery_plans().to_vec(),
            AtomicCommitDigest::new([1; 32]),
            0,
        ),
        Err(Error::AtomicWorkflowMismatch)
    );

    let work_claim =
        WorkClaim::new([8; 16], "worker", NonZeroU64::MIN, 11, 20, NonZeroU64::MIN).unwrap();
    let claim_commands = [
        AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::ArtifactSigning(ids().1),
            work_claim.clone(),
        )),
        AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::ArtifactAdmission(ids().1),
            work_claim.clone(),
        )),
        AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(ids().2),
            work_claim.clone(),
        )),
    ];
    for command in &claim_commands {
        let AuthoredAtomicCommand::Claim(value) = command else {
            unreachable!()
        };
        assert_eq!(value.claim(), &work_claim);
        let _ = value.target();
    }

    let apply_signed = ApplySignedArtifact::new(ids().1, fence.clone(), signed(&authored_plan), 12)
        .expect("signed command");
    assert_eq!(apply_signed.artifact_id(), ids().1);
    assert_eq!(apply_signed.fence(), &fence);
    assert_eq!(apply_signed.event().id(), authored_plan.expected_event_id());
    assert_eq!(apply_signed.applied_at_unix_ms(), 12);
    assert_eq!(
        ApplySignedArtifact::new(ids().1, fence.clone(), signed(&authored_plan), 0),
        Err(Error::AtomicWorkflowMismatch)
    );

    let retry_failure = WorkFailure::new(
        "temporary_admission",
        WorkPhase::Admission,
        FailureClass::Retryable,
        Some(30),
        Some("try another worker".to_owned()),
    )
    .unwrap();
    let retry = RetrySchedule::new(NonZeroU32::MIN, 30, retry_failure.clone()).unwrap();
    let apply_admission = ApplyAdmissionResult::new(
        ids().1,
        fence.clone(),
        AdmissionState::Retryable,
        Some(retry_failure.clone()),
        Some(retry.clone()),
        13,
    )
    .unwrap();
    assert_eq!(apply_admission.artifact_id(), ids().1);
    assert_eq!(apply_admission.fence(), &fence);
    assert_eq!(apply_admission.state(), AdmissionState::Retryable);
    assert_eq!(apply_admission.failure(), Some(&retry_failure));
    assert_eq!(apply_admission.retry(), Some(&retry));
    assert_eq!(apply_admission.applied_at_unix_ms(), 13);
    assert_eq!(
        ApplyAdmissionResult::new(
            ids().1,
            fence.clone(),
            AdmissionState::Inserted,
            None,
            None,
            0,
        ),
        Err(Error::AtomicWorkflowMismatch)
    );

    let request = intent()
        .materialize(radroots_transport::sink::DeliveryPayload::new(signed(
            &authored_plan,
        )))
        .unwrap();
    let receipt = DeliveryReceipt::for_request(
        &request,
        request
            .target_set()
            .targets()
            .iter()
            .cloned()
            .map(|target| {
                DeliveryTargetReceipt::attempted(
                    target,
                    DeliveryOutcome::accepted()
                        .with_detail("accepted", "accepted by relay")
                        .unwrap(),
                )
            })
            .collect(),
    )
    .unwrap();
    let apply_delivery = ApplyDeliveryAttempt::new(
        ids().2,
        fence.clone(),
        DeliveryAttemptOutcome::Receipt(receipt.clone()),
        None,
        14,
    )
    .unwrap();
    assert_eq!(apply_delivery.plan_id(), ids().2);
    assert_eq!(apply_delivery.fence(), &fence);
    assert!(matches!(
        apply_delivery.outcome(),
        DeliveryAttemptOutcome::Receipt(_)
    ));
    assert_eq!(apply_delivery.retry(), None);
    assert_eq!(apply_delivery.applied_at_unix_ms(), 14);
    assert_eq!(
        ApplyDeliveryAttempt::new(
            ids().2,
            fence.clone(),
            DeliveryAttemptOutcome::Receipt(receipt),
            None,
            0,
        ),
        Err(Error::AtomicWorkflowMismatch)
    );

    let sink_failure = SinkFailure::for_request(
        &request,
        "relay_unavailable",
        Retryability::Retryable,
        Some(30),
        None,
        Vec::new(),
    )
    .unwrap();
    let sink_command = AuthoredAtomicCommand::ApplyDelivery(
        ApplyDeliveryAttempt::new(
            ids().2,
            fence.clone(),
            DeliveryAttemptOutcome::SinkFailure(sink_failure),
            Some(retry.clone()),
            14,
        )
        .unwrap(),
    );

    let failure_commands = [
        (AuthoredWorkTarget::Artifact(ids().1), WorkPhase::Signing),
        (AuthoredWorkTarget::Artifact(ids().1), WorkPhase::Admission),
        (
            AuthoredWorkTarget::DeliveryPlan(ids().2),
            WorkPhase::Delivery,
        ),
    ]
    .map(|(target, phase)| {
        let failure = WorkFailure::new(
            "terminal_failure",
            phase,
            FailureClass::Terminal,
            None,
            Some("terminal".to_owned()),
        )
        .unwrap();
        AuthoredAtomicCommand::ApplyFailure(
            ApplyWorkFailure::new(target, fence.clone(), failure, None, 15).unwrap(),
        )
    });
    for command in &failure_commands {
        let AuthoredAtomicCommand::ApplyFailure(value) = command else {
            unreachable!()
        };
        let _ = value.target();
        assert_eq!(value.fence(), &fence);
        assert_eq!(value.failure().class(), FailureClass::Terminal);
        assert_eq!(value.retry(), None);
        assert_eq!(value.applied_at_unix_ms(), 15);
    }
    assert_eq!(
        ApplyWorkFailure::new(
            AuthoredWorkTarget::Artifact(ids().1),
            fence.clone(),
            retry_failure,
            Some(retry),
            0,
        ),
        Err(Error::AtomicWorkflowMismatch)
    );

    let cancel_commands = [
        CancelAuthoredTarget::ArtifactSigning(ids().1),
        CancelAuthoredTarget::ArtifactAdmission(ids().1),
        CancelAuthoredTarget::DeliveryPlan(ids().2),
    ]
    .map(|target| {
        AuthoredAtomicCommand::Cancel(CancelAuthoredWork::new(target, NonZeroU64::MIN, 16).unwrap())
    });
    for command in &cancel_commands {
        let AuthoredAtomicCommand::Cancel(value) = command else {
            unreachable!()
        };
        let _ = value.target();
        assert_eq!(value.expected_revision(), NonZeroU64::MIN);
        assert_eq!(value.cancelled_at_unix_ms(), 16);
    }
    assert_eq!(
        CancelAuthoredWork::new(
            CancelAuthoredTarget::ArtifactSigning(ids().1),
            NonZeroU64::MIN,
            0,
        ),
        Err(Error::AtomicWorkflowMismatch)
    );

    let mut commands = vec![
        prepared_command,
        AuthoredAtomicCommand::ApplySigned(apply_signed),
        AuthoredAtomicCommand::ApplyAdmission(apply_admission),
        AuthoredAtomicCommand::ApplyDelivery(apply_delivery),
        sink_command,
    ];
    commands.extend(claim_commands);
    commands.extend(failure_commands);
    commands.extend(cancel_commands);
    for command in commands {
        assert_ne!(command.commit_id().as_bytes(), &[0; 16]);
        assert_ne!(command.digest().as_bytes(), &[0; 32]);
        assert_ne!(command.requested_at_unix_ms(), 0);
    }

    let outcome = AuthoredAtomicOutcome::Prepared {
        operation: prepared.operation().clone(),
        artifacts: prepared.artifacts().to_vec(),
        delivery_plans: prepared.delivery_plans().to_vec(),
    };
    let command = AuthoredAtomicCommand::Prepare(prepared.clone());
    assert_eq!(
        AuthoredAtomicReceipt::new(
            &command,
            AtomicCommitDisposition::Committed,
            9,
            outcome.clone(),
        ),
        Err(Error::AtomicWorkflowMismatch)
    );
    let receipt = AuthoredAtomicReceipt::new(
        &command,
        AtomicCommitDisposition::Committed,
        10,
        outcome.clone(),
    )
    .unwrap();
    assert_eq!(receipt.commit_id(), command.commit_id());
    assert_eq!(receipt.digest(), command.digest());
    assert_eq!(receipt.disposition(), AtomicCommitDisposition::Committed);
    assert_eq!(receipt.committed_at_unix_ms(), 10);
    assert_eq!(receipt.outcome(), &outcome);
    assert_eq!(
        AuthoredAtomicReceipt::from_durable_parts(
            receipt.commit_id(),
            receipt.digest(),
            AtomicCommitDisposition::Replay,
            0,
            outcome.clone(),
        ),
        Err(Error::AtomicWorkflowMismatch)
    );
    assert!(
        AuthoredAtomicReceipt::from_durable_parts(
            receipt.commit_id(),
            receipt.digest(),
            AtomicCommitDisposition::Replay,
            11,
            outcome,
        )
        .is_ok()
    );
    let invalid_outcome = AuthoredAtomicOutcome::Prepared {
        operation: prepared.operation().clone(),
        artifacts: Vec::new(),
        delivery_plans: Vec::new(),
    };
    assert_eq!(
        AuthoredAtomicReceipt::from_durable_parts(
            receipt.commit_id(),
            receipt.digest(),
            AtomicCommitDisposition::Replay,
            11,
            invalid_outcome,
        ),
        Err(Error::AtomicWorkflowMismatch)
    );
}

#[test]
fn memory_executes_admission_results_and_every_authored_failure_phase() {
    let storage = signed_storage();
    let active = claim_admission(&storage, 5, 13);
    let inserted = ApplyAdmissionResult::new(
        ids().1,
        fence(&active),
        AdmissionState::Inserted,
        None,
        None,
        14,
    )
    .unwrap();
    block_on(storage.execute_authored(AuthoredAtomicCommand::ApplyAdmission(inserted))).unwrap();
    assert_eq!(
        block_on(storage.authored_artifact(ids().1))
            .unwrap()
            .unwrap()
            .admission_state(),
        AdmissionState::Inserted
    );

    let (storage, _) = prepared_storage();
    let artifact = block_on(storage.authored_artifact(ids().1))
        .unwrap()
        .unwrap();
    let (command, active) = claim(
        ClaimAuthoredTarget::ArtifactSigning(ids().1),
        artifact.revision(),
        4,
        11,
    );
    block_on(storage.execute_authored(command)).unwrap();
    let failure = WorkFailure::new(
        "terminal_signer_failure",
        WorkPhase::Signing,
        FailureClass::Terminal,
        None,
        None,
    )
    .unwrap();
    let command = ApplyWorkFailure::new(
        AuthoredWorkTarget::Artifact(ids().1),
        fence(&active),
        failure,
        None,
        12,
    )
    .unwrap();
    block_on(storage.execute_authored(AuthoredAtomicCommand::ApplyFailure(command))).unwrap();
    assert_eq!(
        block_on(storage.authored_artifact(ids().1))
            .unwrap()
            .unwrap()
            .signing_state(),
        SigningState::FailedTerminal
    );

    let storage = signed_storage();
    let active = claim_admission(&storage, 5, 13);
    let failure = WorkFailure::new(
        "temporary_admission_failure",
        WorkPhase::Admission,
        FailureClass::Retryable,
        Some(30),
        None,
    )
    .unwrap();
    let retry = RetrySchedule::new(NonZeroU32::MIN, 30, failure.clone()).unwrap();
    let command = ApplyWorkFailure::new(
        AuthoredWorkTarget::Artifact(ids().1),
        fence(&active),
        failure,
        Some(retry),
        14,
    )
    .unwrap();
    block_on(storage.execute_authored(AuthoredAtomicCommand::ApplyFailure(command))).unwrap();
    assert_eq!(
        block_on(storage.authored_artifact(ids().1))
            .unwrap()
            .unwrap()
            .admission_state(),
        AdmissionState::Retryable
    );

    let storage = signed_storage();
    let active = claim_admission(&storage, 5, 13);
    let failure = WorkFailure::new(
        "terminal_admission_failure",
        WorkPhase::Admission,
        FailureClass::Terminal,
        None,
        None,
    )
    .unwrap();
    let command = ApplyWorkFailure::new(
        AuthoredWorkTarget::Artifact(ids().1),
        fence(&active),
        failure,
        None,
        14,
    )
    .unwrap();
    block_on(storage.execute_authored(AuthoredAtomicCommand::ApplyFailure(command))).unwrap();
    assert_eq!(
        block_on(storage.authored_artifact(ids().1))
            .unwrap()
            .unwrap()
            .admission_state(),
        AdmissionState::Rejected
    );
}

#[test]
fn memory_executes_delivery_failures_and_rejects_phase_mismatches_atomically() {
    let storage = signed_storage();
    let active = claim_delivery(&storage, 5, 13);
    let failure = WorkFailure::new(
        "relay_unavailable",
        WorkPhase::Delivery,
        FailureClass::Retryable,
        Some(30),
        Some("relay unavailable".to_owned()),
    )
    .unwrap();
    let retry = RetrySchedule::new(NonZeroU32::MIN, 30, failure.clone()).unwrap();
    let command = ApplyWorkFailure::new(
        AuthoredWorkTarget::DeliveryPlan(ids().2),
        fence(&active),
        failure,
        Some(retry),
        14,
    )
    .unwrap();
    block_on(storage.execute_authored(AuthoredAtomicCommand::ApplyFailure(command))).unwrap();
    assert_eq!(
        block_on(storage.authored_delivery_plan(ids().2))
            .unwrap()
            .unwrap()
            .state(),
        AuthoredDeliveryState::Retryable
    );

    let storage = signed_storage();
    let active = claim_delivery(&storage, 5, 13);
    let request = block_on(storage.authored_delivery_plan(ids().2))
        .unwrap()
        .unwrap()
        .request()
        .unwrap()
        .clone();
    let sink_failure = SinkFailure::for_request(
        &request,
        "relay_terminal",
        Retryability::Terminal,
        None,
        Some("terminal".to_owned()),
        Vec::new(),
    )
    .unwrap();
    let command = ApplyDeliveryAttempt::new(
        ids().2,
        fence(&active),
        DeliveryAttemptOutcome::SinkFailure(sink_failure),
        None,
        14,
    )
    .unwrap();
    block_on(storage.execute_authored(AuthoredAtomicCommand::ApplyDelivery(command))).unwrap();
    assert_eq!(
        block_on(storage.authored_delivery_plan(ids().2))
            .unwrap()
            .unwrap()
            .state(),
        AuthoredDeliveryState::FailedTerminal
    );

    for (target, phase, class) in [
        (
            AuthoredWorkTarget::Artifact(ids().1),
            WorkPhase::Delivery,
            FailureClass::Terminal,
        ),
        (
            AuthoredWorkTarget::DeliveryPlan(ids().2),
            WorkPhase::Admission,
            FailureClass::Terminal,
        ),
        (
            AuthoredWorkTarget::DeliveryPlan(ids().2),
            WorkPhase::Delivery,
            FailureClass::Indeterminate,
        ),
    ] {
        let (storage, _) = prepared_storage();
        let failure = WorkFailure::new("mismatch", phase, class, None, None).unwrap();
        let command = ApplyWorkFailure::new(
            target,
            WorkFence::new([9; 16], NonZeroU64::MIN, NonZeroU64::MIN).unwrap(),
            failure,
            None,
            12,
        )
        .unwrap();
        assert!(
            block_on(storage.execute_authored(AuthoredAtomicCommand::ApplyFailure(command)))
                .is_err()
        );
    }
}

#[test]
fn memory_executes_all_cancellation_targets_and_revision_fences() {
    let (storage, _) = prepared_storage();
    let artifact = block_on(storage.authored_artifact(ids().1))
        .unwrap()
        .unwrap();
    let stale = CancelAuthoredWork::new(
        CancelAuthoredTarget::ArtifactSigning(ids().1),
        NonZeroU64::new(9).unwrap(),
        11,
    )
    .unwrap();
    assert_eq!(
        block_on(storage.execute_authored(AuthoredAtomicCommand::Cancel(stale))),
        Err(Error::InvalidAuthoredTransition)
    );
    let cancel = CancelAuthoredWork::new(
        CancelAuthoredTarget::ArtifactSigning(ids().1),
        artifact.revision(),
        11,
    )
    .unwrap();
    block_on(storage.execute_authored(AuthoredAtomicCommand::Cancel(cancel))).unwrap();
    assert_eq!(
        block_on(storage.authored_artifact(ids().1))
            .unwrap()
            .unwrap()
            .signing_state(),
        SigningState::Cancelled
    );

    let storage = signed_storage();
    let artifact = block_on(storage.authored_artifact(ids().1))
        .unwrap()
        .unwrap();
    let cancel = CancelAuthoredWork::new(
        CancelAuthoredTarget::ArtifactAdmission(ids().1),
        artifact.revision(),
        13,
    )
    .unwrap();
    block_on(storage.execute_authored(AuthoredAtomicCommand::Cancel(cancel))).unwrap();
    assert_eq!(
        block_on(storage.authored_artifact(ids().1))
            .unwrap()
            .unwrap()
            .admission_state(),
        AdmissionState::Cancelled
    );

    let storage = signed_storage();
    let plan = block_on(storage.authored_delivery_plan(ids().2))
        .unwrap()
        .unwrap();
    let stale = CancelAuthoredWork::new(
        CancelAuthoredTarget::DeliveryPlan(ids().2),
        NonZeroU64::MIN,
        13,
    )
    .unwrap();
    assert_eq!(
        block_on(storage.execute_authored(AuthoredAtomicCommand::Cancel(stale))),
        Err(Error::InvalidAuthoredDeliveryPlan)
    );
    let cancel = CancelAuthoredWork::new(
        CancelAuthoredTarget::DeliveryPlan(ids().2),
        plan.revision(),
        13,
    )
    .unwrap();
    block_on(storage.execute_authored(AuthoredAtomicCommand::Cancel(cancel))).unwrap();
    assert_eq!(
        block_on(storage.authored_delivery_plan(ids().2))
            .unwrap()
            .unwrap()
            .state(),
        AuthoredDeliveryState::Cancelled
    );
}
