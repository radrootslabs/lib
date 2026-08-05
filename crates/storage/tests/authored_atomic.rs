use core::num::{NonZeroU32, NonZeroU64};
use futures_executor::block_on;
use radroots_event::{GenericEventDraft, SignedEvent, wire::v1::Nip01EventWire};
use radroots_event_codec::authoring::AuthoredEventPlan;
use radroots_storage::{
    Error,
    atomic::{AtomicCommitDigest, AtomicCommitDisposition},
    authored::{
        AuthoredArtifact, AuthoredArtifactId, AuthoredOperation, FailureClass, RetrySchedule,
        SigningState, WorkClaim, WorkFailure, WorkPhase,
    },
    authored_atomic::{
        ApplyDeliveryAttempt, ApplySignedArtifact, AuthoredAtomicCommand, AuthoredAtomicOutcome,
        AuthoredAtomicStorage, AuthoredWorkTarget, ClaimAuthoredTarget, ClaimAuthoredWork,
        PrepareAuthoredOperation, WorkFence,
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
    DeliveryReceipt, Target, TargetSet,
    outcome::DeliveryOutcome,
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
