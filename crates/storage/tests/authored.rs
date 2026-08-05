use core::num::{NonZeroU32, NonZeroU64};
use radroots_event::{GenericEventDraft, SignedEvent, wire::v1::Nip01EventWire};
use radroots_event_codec::authoring::AuthoredEventPlan;
use radroots_storage::{
    Error,
    authored::{
        AdmissionState, ArtifactOrigin, AuthoredArtifact, AuthoredArtifactId, AuthoredOperation,
        FailureClass, OperationSettlement, RetrySchedule, SigningState, WorkClaim, WorkFailure,
        WorkPhase,
    },
    journal::OperationInstanceId,
};

const AUTHOR: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const CREATED_AT: u64 = 1_800_000_100;

fn operation_id() -> OperationInstanceId {
    OperationInstanceId::new([1; 16]).expect("operation ID")
}

fn artifact_id(value: u8) -> AuthoredArtifactId {
    AuthoredArtifactId::new([value; 16]).expect("artifact ID")
}

fn plan() -> AuthoredEventPlan {
    AuthoredEventPlan::from_generic(
        GenericEventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            CREATED_AT,
            Vec::new(),
            "durable authored plan",
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

fn retry_failure(phase: WorkPhase, at: u64) -> WorkFailure {
    WorkFailure::new(
        "temporary_failure",
        phase,
        FailureClass::Retryable,
        Some(at),
        Some("temporary failure".to_owned()),
    )
    .expect("failure")
}

#[test]
fn claim_failure_and_retry_models_are_bounded_and_fenced() {
    let revision = NonZeroU64::MIN;
    let claim =
        WorkClaim::new([2; 16], "worker-1", NonZeroU64::MIN, 10, 20, revision).expect("claim");
    assert!(claim.matches_fence(&[2; 16], NonZeroU64::MIN, revision, 10));
    assert!(!claim.matches_fence(&[2; 16], NonZeroU64::MIN, revision, 20));
    assert_eq!(
        WorkClaim::new([0; 16], "worker", NonZeroU64::MIN, 10, 20, revision),
        Err(Error::InvalidWorkClaim)
    );

    let failure = retry_failure(WorkPhase::Signing, 30);
    let retry = RetrySchedule::new(NonZeroU32::MIN, 30, failure.clone()).expect("retry");
    assert_eq!(retry.attempt().get(), 1);
    assert_eq!(retry.failure(), &failure);
    assert_eq!(
        WorkFailure::new(
            "INVALID",
            WorkPhase::Signing,
            FailureClass::Terminal,
            None,
            None,
        ),
        Err(Error::InvalidWorkFailure)
    );
    assert_eq!(
        RetrySchedule::new(NonZeroU32::MIN, 31, retry_failure(WorkPhase::Signing, 30),),
        Err(Error::InvalidRetrySchedule)
    );
}

#[test]
fn planned_artifacts_enforce_exact_signing_and_admission_transitions() {
    let plan = plan();
    let mut artifact = AuthoredArtifact::planned(artifact_id(2), operation_id(), 0, &plan, 10)
        .expect("planned artifact");
    let claim = WorkClaim::new(
        [3; 16],
        "signer",
        NonZeroU64::MIN,
        11,
        20,
        artifact.revision(),
    )
    .expect("claim");
    artifact
        .set_signing_claim(claim, 11)
        .expect("claim signing");
    artifact.record_signed(signed(&plan), 12).expect("sign");
    assert_eq!(artifact.signing_state(), SigningState::Signed);
    assert!(artifact.signed().is_some());
    assert!(artifact.signing_claim().is_none());
    artifact
        .record_admission(AdmissionState::Duplicate, None, None, 13)
        .expect("admit duplicate");
    assert!(artifact.admission_state().is_admitted());

    let other = AuthoredEventPlan::from_generic(
        GenericEventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            CREATED_AT,
            Vec::new(),
            "different",
            AUTHOR,
        )
        .expect("other draft"),
    )
    .expect("other plan");
    let mut mismatched = AuthoredArtifact::planned(artifact_id(3), operation_id(), 1, &plan, 10)
        .expect("planned artifact");
    assert_eq!(
        mismatched.record_signed(signed(&other), 11),
        Err(Error::InvalidAuthoredArtifact)
    );
    assert_eq!(mismatched.signing_state(), SigningState::Planned);
    assert!(mismatched.signed().is_none());
}

#[test]
fn imported_artifacts_are_non_resignable_and_settlement_preserves_order() {
    let plan = plan();
    let imported =
        AuthoredArtifact::imported_signed(artifact_id(2), operation_id(), 0, signed(&plan), 10)
            .expect("imported");
    assert_eq!(imported.origin(), ArtifactOrigin::ImportedSigned);
    assert!(!imported.origin().is_resignable());

    let mut planned =
        AuthoredArtifact::planned(artifact_id(3), operation_id(), 1, &plan, 10).expect("planned");
    planned.cancel_signing(11).expect("cancel");
    let operation =
        AuthoredOperation::new(operation_id(), vec![artifact_id(2), artifact_id(3)], 10)
            .expect("operation");
    let settlement =
        OperationSettlement::evaluate(&operation, &[imported, planned]).expect("settlement");
    assert_eq!(settlement.artifacts(), 2);
    assert_eq!(settlement.signed(), 1);
    assert_eq!(settlement.pending(), 1);
    assert_eq!(settlement.cancelled(), 1);
    assert!(!settlement.is_settled());

    assert_eq!(
        AuthoredOperation::new(operation_id(), vec![artifact_id(2), artifact_id(2)], 10,),
        Err(Error::InvalidAuthoredOperation)
    );
}

#[test]
fn durable_models_round_trip_and_reject_forged_state() {
    let plan = plan();
    let mut artifact =
        AuthoredArtifact::planned(artifact_id(2), operation_id(), 0, &plan, 10).expect("planned");
    let failure = retry_failure(WorkPhase::Signing, 20);
    artifact
        .record_signing_failure(
            failure.clone(),
            Some(RetrySchedule::new(NonZeroU32::MIN, 20, failure).expect("retry")),
            11,
        )
        .expect("retryable signing");
    let json = serde_json::to_string(&artifact).expect("artifact json");
    assert_eq!(
        serde_json::from_str::<AuthoredArtifact>(&json).expect("artifact round trip"),
        artifact
    );

    let mut forged: serde_json::Value = serde_json::from_str(&json).expect("artifact value");
    forged["signing_state"] = serde_json::json!("signed");
    assert!(serde_json::from_value::<AuthoredArtifact>(forged).is_err());

    let exact = artifact.plan().expect("durable plan");
    let mut corrupt = exact.wire_json().to_vec();
    corrupt[0] ^= 1;
    assert_eq!(
        radroots_storage::authored::DurableAuthoredPlan::reconstruct(corrupt),
        Err(Error::InvalidAuthoredArtifact)
    );

    let imported =
        AuthoredArtifact::imported_signed(artifact_id(4), operation_id(), 0, signed(&plan), 10)
            .expect("imported");
    let mut forged_digest = serde_json::to_value(imported).expect("imported value");
    forged_digest["signed"]["raw_json_sha256"] =
        serde_json::to_value([0_u8; 32]).expect("digest value");
    assert!(serde_json::from_value::<AuthoredArtifact>(forged_digest).is_err());

    let operation =
        AuthoredOperation::new(operation_id(), vec![artifact_id(2), artifact_id(3)], 10)
            .expect("operation");
    let operation_json = serde_json::to_string(&operation).expect("operation json");
    assert_eq!(
        serde_json::from_str::<AuthoredOperation>(&operation_json).expect("operation round trip"),
        operation
    );
    let mut forged_operation: serde_json::Value =
        serde_json::from_str(&operation_json).expect("operation value");
    forged_operation["artifact_ids"][1] = forged_operation["artifact_ids"][0].clone();
    assert!(serde_json::from_value::<AuthoredOperation>(forged_operation).is_err());
}
