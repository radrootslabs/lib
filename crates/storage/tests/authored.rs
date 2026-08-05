use core::num::{NonZeroU32, NonZeroU64};
use radroots_event::{GenericEventDraft, SignedEvent, wire::v1::Nip01EventWire};
use radroots_event_codec::authoring::AuthoredEventPlan;
use radroots_storage::{
    Error,
    authored::{
        AUTHORED_OPERATION_ARTIFACTS_MAX, AdmissionState, ArtifactOrigin, AuthoredArtifact,
        AuthoredArtifactId, AuthoredOperation, FailureClass, OperationSettlement, RetrySchedule,
        SigningState, WORK_CLAIM_OWNER_MAX_BYTES, WORK_FAILURE_CODE_MAX_BYTES,
        WORK_FAILURE_DIAGNOSTIC_MAX_BYTES, WorkClaim, WorkFailure, WorkPhase,
    },
    authored_delivery::{AuthoredDeliveryPlan, AuthoredDeliveryPlanId},
    journal::OperationInstanceId,
};
use radroots_transport::{
    DeliveryReceipt, DeliveryRequest, SinkFailure, Target, TargetSet,
    outcome::{DeliveryOutcome, Retryability},
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::{DeliveryPayload, DeliveryTargetReceipt},
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

#[test]
fn claim_validation_and_fence_dimensions_are_independent() {
    let revision = NonZeroU64::new(7).unwrap();
    let generation = NonZeroU64::new(3).unwrap();
    let claim = WorkClaim::new([9; 16], "worker", generation, 10, 20, revision).unwrap();
    assert_eq!(claim.token(), &[9; 16]);
    assert_eq!(claim.owner(), "worker");
    assert_eq!(claim.generation(), generation);
    assert_eq!(claim.acquired_at_unix_ms(), 10);
    assert_eq!(claim.expires_at_unix_ms(), 20);
    assert_eq!(claim.row_revision(), revision);
    assert!(claim.matches_fence(&[9; 16], generation, revision, 10));
    assert!(claim.matches_fence(&[9; 16], generation, revision, 19));
    assert!(!claim.matches_fence(&[8; 16], generation, revision, 10));
    assert!(!claim.matches_fence(&[9; 16], NonZeroU64::MIN, revision, 10));
    assert!(!claim.matches_fence(&[9; 16], generation, NonZeroU64::MIN, 10));
    assert!(!claim.matches_fence(&[9; 16], generation, revision, 9));
    assert!(!claim.matches_fence(&[9; 16], generation, revision, 20));

    for (token, owner, acquired, expires) in [
        ([0; 16], "worker".to_owned(), 10, 20),
        ([1; 16], String::new(), 10, 20),
        ([1; 16], " worker".to_owned(), 10, 20),
        ([1; 16], "worker ".to_owned(), 10, 20),
        ([1; 16], "worker\nname".to_owned(), 10, 20),
        ([1; 16], "x".repeat(WORK_CLAIM_OWNER_MAX_BYTES + 1), 10, 20),
        ([1; 16], "worker".to_owned(), 0, 20),
        ([1; 16], "worker".to_owned(), 10, 10),
        ([1; 16], "worker".to_owned(), 10, 9),
    ] {
        assert_eq!(
            WorkClaim::new(token, owner, generation, acquired, expires, revision),
            Err(Error::InvalidWorkClaim)
        );
    }

    let json = serde_json::to_string(&claim).unwrap();
    assert_eq!(serde_json::from_str::<WorkClaim>(&json).unwrap(), claim);
    let mut invalid: serde_json::Value = serde_json::from_str(&json).unwrap();
    invalid["owner"] = serde_json::json!("");
    assert!(serde_json::from_value::<WorkClaim>(invalid).is_err());
}

#[test]
fn failure_and_retry_models_cover_every_class_and_boundary() {
    for phase in [
        WorkPhase::Signing,
        WorkPhase::Admission,
        WorkPhase::Delivery,
    ] {
        for class in [
            FailureClass::Retryable,
            FailureClass::Terminal,
            FailureClass::Indeterminate,
        ] {
            let retry_after = (class == FailureClass::Retryable).then_some(20);
            let failure = WorkFailure::new(
                "failure.code-1",
                phase,
                class,
                retry_after,
                Some("safe detail".to_owned()),
            )
            .unwrap();
            assert_eq!(failure.code(), "failure.code-1");
            assert_eq!(failure.phase(), phase);
            assert_eq!(failure.class(), class);
            assert_eq!(failure.retry_after_unix_ms(), retry_after);
            assert_eq!(failure.diagnostic(), Some("safe detail"));
            failure.validate().unwrap();
            assert_eq!(
                serde_json::from_str::<WorkFailure>(&serde_json::to_string(&failure).unwrap())
                    .unwrap(),
                failure
            );
        }
    }

    for (code, class, retry_after, diagnostic) in [
        ("", FailureClass::Terminal, None, None),
        ("INVALID", FailureClass::Terminal, None, None),
        ("bad/code", FailureClass::Terminal, None, None),
        (
            "x".repeat(WORK_FAILURE_CODE_MAX_BYTES + 1).leak(),
            FailureClass::Terminal,
            None,
            None,
        ),
        ("failure", FailureClass::Retryable, Some(0), None),
        ("failure", FailureClass::Terminal, Some(20), None),
        ("failure", FailureClass::Indeterminate, Some(20), None),
        ("failure", FailureClass::Terminal, None, Some("".to_owned())),
        (
            "failure",
            FailureClass::Terminal,
            None,
            Some(" diagnostic".to_owned()),
        ),
        (
            "failure",
            FailureClass::Terminal,
            None,
            Some("x".repeat(WORK_FAILURE_DIAGNOSTIC_MAX_BYTES + 1)),
        ),
    ] {
        assert_eq!(
            WorkFailure::new(code, WorkPhase::Signing, class, retry_after, diagnostic),
            Err(Error::InvalidWorkFailure)
        );
    }

    let initial_failure = retry_failure(WorkPhase::Delivery, 30);
    let retry = RetrySchedule::new(NonZeroU32::MIN, 30, initial_failure.clone()).unwrap();
    assert_eq!(retry.attempt(), NonZeroU32::MIN);
    assert_eq!(retry.not_before_unix_ms(), 30);
    assert_eq!(retry.failure(), &initial_failure);
    let next_failure = retry_failure(WorkPhase::Delivery, 40);
    assert_eq!(
        retry
            .next_attempt(40, next_failure)
            .unwrap()
            .attempt()
            .get(),
        2
    );
    assert_eq!(
        RetrySchedule::new(NonZeroU32::MIN, 0, initial_failure.clone()),
        Err(Error::InvalidRetrySchedule)
    );
    assert_eq!(
        RetrySchedule::new(
            NonZeroU32::MIN,
            30,
            WorkFailure::new(
                "terminal",
                WorkPhase::Delivery,
                FailureClass::Terminal,
                None,
                None,
            )
            .unwrap(),
        ),
        Err(Error::InvalidRetrySchedule)
    );
    assert_eq!(
        RetrySchedule::new(NonZeroU32::MIN, 31, initial_failure),
        Err(Error::InvalidRetrySchedule)
    );

    let mut maximum = serde_json::to_value(&retry).unwrap();
    maximum["attempt"] = serde_json::json!(u32::MAX);
    let maximum: RetrySchedule = serde_json::from_value(maximum).unwrap();
    assert_eq!(
        maximum.next_attempt(40, retry_failure(WorkPhase::Delivery, 40)),
        Err(Error::InvalidRetrySchedule)
    );
}

#[test]
fn operation_construction_reconstruction_and_accessors_are_bounded() {
    assert_eq!(
        AuthoredArtifactId::new([0; 16]),
        Err(Error::InvalidAuthoredArtifact)
    );
    let id = artifact_id(2);
    assert_eq!(id.as_bytes(), &[2; 16]);
    assert_eq!(AuthoredArtifactId::try_from([2; 16]).unwrap(), id);
    assert_eq!(<[u8; 16]>::from(id), [2; 16]);

    let operation = AuthoredOperation::new(operation_id(), vec![id], 10).unwrap();
    assert_eq!(operation.operation_id(), operation_id());
    assert_eq!(operation.artifact_ids(), &[id]);
    assert_eq!(operation.created_at_unix_ms(), 10);
    assert_eq!(operation.updated_at_unix_ms(), 10);
    assert_eq!(operation.revision(), NonZeroU64::MIN);
    assert_eq!(
        AuthoredOperation::reconstruct(
            operation_id(),
            vec![id],
            10,
            11,
            NonZeroU64::new(2).unwrap(),
        )
        .unwrap()
        .updated_at_unix_ms(),
        11
    );

    for (ids, created, updated) in [
        (Vec::new(), 10, 10),
        (vec![id, id], 10, 10),
        (
            (0..=AUTHORED_OPERATION_ARTIFACTS_MAX)
                .map(|index| artifact_id((index % 254 + 1) as u8))
                .collect(),
            10,
            10,
        ),
        (vec![id], 0, 10),
        (vec![id], 10, 9),
    ] {
        assert_eq!(
            AuthoredOperation::reconstruct(operation_id(), ids, created, updated, NonZeroU64::MIN,),
            Err(Error::InvalidAuthoredOperation)
        );
    }
}

fn claim_for(artifact: &AuthoredArtifact, token: u8, generation: u64, acquired: u64) -> WorkClaim {
    WorkClaim::new(
        [token; 16],
        format!("worker-{token}"),
        NonZeroU64::new(generation).unwrap(),
        acquired,
        acquired + 10,
        artifact.revision(),
    )
    .unwrap()
}

fn failure(phase: WorkPhase, class: FailureClass, at: Option<u64>) -> WorkFailure {
    WorkFailure::new("operation_failure", phase, class, at, None).unwrap()
}

#[test]
fn signing_claim_failure_terminal_indeterminate_and_cancel_paths_are_fenced() {
    let authored_plan = plan();
    let mut retryable =
        AuthoredArtifact::planned(artifact_id(10), operation_id(), 0, &authored_plan, 10).unwrap();
    let first_claim = claim_for(&retryable, 1, 1, 11);
    retryable
        .set_signing_claim(first_claim.clone(), 11)
        .unwrap();
    assert_eq!(retryable.signing_claim(), Some(&first_claim));
    assert_eq!(
        retryable.set_signing_claim(claim_for(&retryable, 2, 2, 12), 12),
        Err(Error::InvalidAuthoredTransition)
    );
    let replacement = claim_for(&retryable, 3, 2, 21);
    retryable
        .set_signing_claim(replacement.clone(), 21)
        .unwrap();
    assert_eq!(retryable.signing_claim(), Some(&replacement));

    let retry_failure = failure(WorkPhase::Signing, FailureClass::Retryable, Some(40));
    let retry = RetrySchedule::new(NonZeroU32::MIN, 40, retry_failure.clone()).unwrap();
    retryable
        .record_signing_failure(retry_failure.clone(), Some(retry.clone()), 22)
        .unwrap();
    assert_eq!(retryable.signing_state(), SigningState::Retryable);
    assert_eq!(retryable.signing_retry(), Some(&retry));
    assert_eq!(retryable.last_failure(), Some(&retry_failure));
    assert_eq!(
        retryable.set_signing_claim(claim_for(&retryable, 4, 3, 39), 39),
        Err(Error::InvalidAuthoredTransition)
    );
    let retry_claim = claim_for(&retryable, 4, 3, 40);
    retryable.set_signing_claim(retry_claim, 40).unwrap();
    retryable.record_signed(signed(&authored_plan), 41).unwrap();
    assert_eq!(retryable.signing_state(), SigningState::Signed);
    assert_eq!(retryable.signing_retry(), None);
    assert_eq!(retryable.last_failure(), None);

    let mut terminal =
        AuthoredArtifact::planned(artifact_id(11), operation_id(), 0, &authored_plan, 10).unwrap();
    let terminal_failure = failure(WorkPhase::Signing, FailureClass::Terminal, None);
    terminal
        .record_signing_failure(terminal_failure.clone(), None, 11)
        .unwrap();
    assert_eq!(terminal.signing_state(), SigningState::FailedTerminal);
    assert_eq!(terminal.last_failure(), Some(&terminal_failure));
    assert_eq!(
        terminal.cancel_signing(12),
        Err(Error::InvalidAuthoredTransition)
    );

    let mut indeterminate =
        AuthoredArtifact::planned(artifact_id(12), operation_id(), 0, &authored_plan, 10).unwrap();
    let indeterminate_failure = failure(WorkPhase::Signing, FailureClass::Indeterminate, None);
    indeterminate
        .record_signing_failure(indeterminate_failure.clone(), None, 11)
        .unwrap();
    assert_eq!(indeterminate.signing_state(), SigningState::Indeterminate);

    let mut cancelled =
        AuthoredArtifact::planned(artifact_id(13), operation_id(), 0, &authored_plan, 10).unwrap();
    cancelled.cancel_signing(11).unwrap();
    assert_eq!(cancelled.signing_state(), SigningState::Cancelled);
    assert_eq!(cancelled.updated_at_unix_ms(), 11);

    let mut imported = AuthoredArtifact::imported_signed(
        artifact_id(14),
        operation_id(),
        0,
        signed(&authored_plan),
        10,
    )
    .unwrap();
    assert_eq!(
        imported.record_signed(signed(&authored_plan), 11),
        Err(Error::InvalidAuthoredTransition)
    );
    assert_eq!(
        imported.cancel_signing(11),
        Err(Error::InvalidAuthoredTransition)
    );

    let mut invalid =
        AuthoredArtifact::planned(artifact_id(15), operation_id(), 0, &authored_plan, 10).unwrap();
    assert_eq!(
        invalid.record_signing_failure(
            failure(WorkPhase::Admission, FailureClass::Terminal, None),
            None,
            11,
        ),
        Err(Error::InvalidAuthoredTransition)
    );
    assert_eq!(
        invalid.record_signing_failure(
            failure(WorkPhase::Signing, FailureClass::Retryable, Some(20)),
            None,
            11,
        ),
        Err(Error::InvalidAuthoredTransition)
    );
    assert_eq!(
        invalid.record_signing_failure(
            failure(WorkPhase::Signing, FailureClass::Terminal, None),
            Some(
                RetrySchedule::new(
                    NonZeroU32::MIN,
                    20,
                    failure(WorkPhase::Signing, FailureClass::Retryable, Some(20)),
                )
                .unwrap(),
            ),
            11,
        ),
        Err(Error::InvalidAuthoredTransition)
    );
    let before = invalid.clone();
    assert_eq!(
        invalid.cancel_signing(9),
        Err(Error::InvalidAuthoredTransition)
    );
    assert_eq!(invalid, before);
}

fn signed_artifact(value: u8) -> AuthoredArtifact {
    let authored_plan = plan();
    let mut artifact =
        AuthoredArtifact::planned(artifact_id(value), operation_id(), 0, &authored_plan, 10)
            .unwrap();
    artifact.record_signed(signed(&authored_plan), 11).unwrap();
    artifact
}

#[test]
fn admission_claim_and_result_paths_enforce_failure_coherence() {
    let mut inserted = signed_artifact(20);
    let active = claim_for(&inserted, 1, 1, 12);
    inserted.set_admission_claim(active.clone(), 12).unwrap();
    assert_eq!(inserted.admission_claim(), Some(&active));
    inserted
        .record_admission(AdmissionState::Inserted, None, None, 13)
        .unwrap();
    assert_eq!(inserted.admission_state(), AdmissionState::Inserted);
    assert!(inserted.admission_state().is_admitted());
    assert_eq!(inserted.admission_claim(), None);

    let mut duplicate = signed_artifact(21);
    duplicate
        .record_admission(AdmissionState::Duplicate, None, None, 12)
        .unwrap();
    assert!(duplicate.admission_state().is_admitted());

    let mut retryable = signed_artifact(22);
    let retry_failure = failure(WorkPhase::Admission, FailureClass::Retryable, Some(30));
    let retry = RetrySchedule::new(NonZeroU32::MIN, 30, retry_failure.clone()).unwrap();
    retryable
        .record_admission(
            AdmissionState::Retryable,
            Some(retry_failure.clone()),
            Some(retry.clone()),
            12,
        )
        .unwrap();
    assert_eq!(retryable.admission_retry(), Some(&retry));
    assert_eq!(retryable.last_failure(), Some(&retry_failure));
    assert_eq!(
        retryable.set_admission_claim(claim_for(&retryable, 2, 2, 29), 29),
        Err(Error::InvalidAuthoredTransition)
    );
    let active = claim_for(&retryable, 2, 2, 30);
    retryable.set_admission_claim(active, 30).unwrap();

    for (value, state) in [
        (23, AdmissionState::Rejected),
        (24, AdmissionState::Cancelled),
    ] {
        let mut artifact = signed_artifact(value);
        let terminal = failure(WorkPhase::Admission, FailureClass::Terminal, None);
        artifact
            .record_admission(state, Some(terminal.clone()), None, 12)
            .unwrap();
        assert_eq!(artifact.admission_state(), state);
        assert_eq!(artifact.last_failure(), Some(&terminal));
    }

    let invalid_cases = [
        (AdmissionState::Pending, None, None),
        (
            AdmissionState::Inserted,
            Some(failure(WorkPhase::Admission, FailureClass::Terminal, None)),
            None,
        ),
        (AdmissionState::Rejected, None, None),
        (
            AdmissionState::Retryable,
            Some(failure(
                WorkPhase::Admission,
                FailureClass::Retryable,
                Some(30),
            )),
            None,
        ),
        (
            AdmissionState::Rejected,
            Some(failure(WorkPhase::Signing, FailureClass::Terminal, None)),
            None,
        ),
    ];
    for (index, (state, failure, retry)) in invalid_cases.into_iter().enumerate() {
        let mut artifact = signed_artifact(30 + index as u8);
        assert_eq!(
            artifact.record_admission(state, failure, retry, 12),
            Err(Error::InvalidAuthoredTransition)
        );
    }

    let mut unsigned =
        AuthoredArtifact::planned(artifact_id(40), operation_id(), 0, &plan(), 10).unwrap();
    assert_eq!(
        unsigned.set_admission_claim(claim_for(&unsigned, 1, 1, 11), 11),
        Err(Error::InvalidAuthoredTransition)
    );
}

fn assert_artifact_json_rejected(
    mut value: serde_json::Value,
    key: &str,
    replacement: serde_json::Value,
) {
    value[key] = replacement;
    assert!(
        serde_json::from_value::<AuthoredArtifact>(value).is_err(),
        "forged artifact field {key} was accepted"
    );
}

#[test]
fn authored_artifact_reconstruction_rejects_each_incoherent_durable_state() {
    let authored_plan = plan();
    let planned =
        AuthoredArtifact::planned(artifact_id(50), operation_id(), 0, &authored_plan, 10).unwrap();
    let planned_value = serde_json::to_value(&planned).unwrap();
    assert_artifact_json_rejected(
        planned_value.clone(),
        "created_at_unix_ms",
        serde_json::json!(0),
    );
    assert_artifact_json_rejected(
        planned_value.clone(),
        "updated_at_unix_ms",
        serde_json::json!(9),
    );
    assert_artifact_json_rejected(planned_value.clone(), "plan", serde_json::Value::Null);
    assert_artifact_json_rejected(
        planned_value.clone(),
        "signing_state",
        serde_json::json!("signed"),
    );
    assert_artifact_json_rejected(
        planned_value.clone(),
        "admission_state",
        serde_json::json!("inserted"),
    );
    assert_artifact_json_rejected(
        planned_value.clone(),
        "signing_state",
        serde_json::json!("retryable"),
    );
    let mut admission_without_signed = planned_value.clone();
    admission_without_signed["admission_state"] = serde_json::json!("retryable");
    assert!(serde_json::from_value::<AuthoredArtifact>(admission_without_signed).is_err());

    let imported = AuthoredArtifact::imported_signed(
        artifact_id(51),
        operation_id(),
        0,
        signed(&authored_plan),
        10,
    )
    .unwrap();
    let imported_value = serde_json::to_value(&imported).unwrap();
    let mut imported_with_plan = imported_value.clone();
    imported_with_plan["plan"] = planned_value["plan"].clone();
    assert!(serde_json::from_value::<AuthoredArtifact>(imported_with_plan).is_err());
    assert_artifact_json_rejected(
        imported_value.clone(),
        "signing_state",
        serde_json::json!("planned"),
    );
    assert_artifact_json_rejected(imported_value.clone(), "signed", serde_json::Value::Null);

    let mut claimed = planned.clone();
    let active = claim_for(&claimed, 1, 1, 11);
    claimed.set_signing_claim(active, 11).unwrap();
    let claimed_value = serde_json::to_value(&claimed).unwrap();
    assert_artifact_json_rejected(claimed_value.clone(), "revision", serde_json::json!(9));
    assert_artifact_json_rejected(
        claimed_value.clone(),
        "updated_at_unix_ms",
        serde_json::json!(12),
    );
    assert_artifact_json_rejected(
        claimed_value.clone(),
        "signing_state",
        serde_json::json!("cancelled"),
    );

    let mut signing_retry = planned.clone();
    let signing_failure = failure(WorkPhase::Signing, FailureClass::Retryable, Some(20));
    signing_retry
        .record_signing_failure(
            signing_failure.clone(),
            Some(RetrySchedule::new(NonZeroU32::MIN, 20, signing_failure).unwrap()),
            11,
        )
        .unwrap();
    let retry_value = serde_json::to_value(&signing_retry).unwrap();
    let mut wrong_retry_phase = retry_value.clone();
    wrong_retry_phase["signing_retry"]["failure"]["phase"] = serde_json::json!("admission");
    assert!(serde_json::from_value::<AuthoredArtifact>(wrong_retry_phase).is_err());
    assert_artifact_json_rejected(retry_value.clone(), "last_failure", serde_json::Value::Null);

    let mut signed_pending = planned.clone();
    signed_pending
        .record_signed(signed(&authored_plan), 11)
        .unwrap();
    let signed_value = serde_json::to_value(&signed_pending).unwrap();
    let mut admission_claimed = signed_pending.clone();
    let active = claim_for(&admission_claimed, 2, 2, 12);
    admission_claimed.set_admission_claim(active, 12).unwrap();
    let admission_claimed_value = serde_json::to_value(&admission_claimed).unwrap();
    assert_artifact_json_rejected(
        admission_claimed_value.clone(),
        "revision",
        serde_json::json!(9),
    );
    assert_artifact_json_rejected(
        admission_claimed_value.clone(),
        "updated_at_unix_ms",
        serde_json::json!(13),
    );
    assert_artifact_json_rejected(
        admission_claimed_value.clone(),
        "admission_state",
        serde_json::json!("inserted"),
    );
    let mut claim_without_signed = admission_claimed_value;
    claim_without_signed["signing_state"] = serde_json::json!("planned");
    claim_without_signed["signed"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<AuthoredArtifact>(claim_without_signed).is_err());

    let admission_failure = failure(WorkPhase::Admission, FailureClass::Retryable, Some(30));
    let mut admission_retry = signed_pending.clone();
    admission_retry
        .record_admission(
            AdmissionState::Retryable,
            Some(admission_failure.clone()),
            Some(RetrySchedule::new(NonZeroU32::MIN, 30, admission_failure).unwrap()),
            12,
        )
        .unwrap();
    let mut wrong_admission_phase = serde_json::to_value(admission_retry).unwrap();
    wrong_admission_phase["admission_retry"]["failure"]["phase"] = serde_json::json!("signing");
    assert!(serde_json::from_value::<AuthoredArtifact>(wrong_admission_phase).is_err());

    let terminal = failure(WorkPhase::Admission, FailureClass::Terminal, None);
    let mut unexpected_failure = signed_value;
    unexpected_failure["last_failure"] = serde_json::to_value(terminal).unwrap();
    assert!(serde_json::from_value::<AuthoredArtifact>(unexpected_failure).is_err());
}

#[test]
fn authored_transition_guards_reject_every_independent_stale_or_incoherent_input() {
    let authored_plan = plan();
    let mut signing =
        AuthoredArtifact::planned(artifact_id(60), operation_id(), 0, &authored_plan, 10).unwrap();
    let active = claim_for(&signing, 1, 2, 11);
    signing.set_signing_claim(active, 11).unwrap();
    let lower_generation = claim_for(&signing, 2, 1, 21);
    assert_eq!(
        signing.set_signing_claim(lower_generation, 21),
        Err(Error::InvalidAuthoredTransition)
    );
    let wrong_revision = WorkClaim::new(
        [3; 16],
        "worker",
        NonZeroU64::new(3).unwrap(),
        21,
        30,
        NonZeroU64::MIN,
    )
    .unwrap();
    assert_eq!(
        signing.set_signing_claim(wrong_revision, 21),
        Err(Error::InvalidAuthoredTransition)
    );
    let wrong_time = WorkClaim::new(
        [4; 16],
        "worker",
        NonZeroU64::new(4).unwrap(),
        22,
        30,
        signing.revision(),
    )
    .unwrap();
    assert_eq!(
        signing.set_signing_claim(wrong_time, 21),
        Err(Error::InvalidAuthoredTransition)
    );

    let mut admission = signed_artifact(61);
    let active = claim_for(&admission, 5, 2, 12);
    admission.set_admission_claim(active, 12).unwrap();
    let lower_generation = claim_for(&admission, 6, 1, 22);
    assert_eq!(
        admission.set_admission_claim(lower_generation, 22),
        Err(Error::InvalidAuthoredTransition)
    );
    let wrong_revision = WorkClaim::new(
        [7; 16],
        "worker",
        NonZeroU64::new(3).unwrap(),
        22,
        30,
        NonZeroU64::MIN,
    )
    .unwrap();
    assert_eq!(
        admission.set_admission_claim(wrong_revision, 22),
        Err(Error::InvalidAuthoredTransition)
    );
    let wrong_time = WorkClaim::new(
        [8; 16],
        "worker",
        NonZeroU64::new(4).unwrap(),
        23,
        30,
        admission.revision(),
    )
    .unwrap();
    assert_eq!(
        admission.set_admission_claim(wrong_time, 22),
        Err(Error::InvalidAuthoredTransition)
    );

    let invalid_retry_failure = failure(WorkPhase::Admission, FailureClass::Retryable, Some(30));
    let other_retry_failure = WorkFailure::new(
        "other_failure",
        WorkPhase::Admission,
        FailureClass::Retryable,
        Some(30),
        None,
    )
    .unwrap();
    let other_retry = RetrySchedule::new(NonZeroU32::MIN, 30, other_retry_failure).unwrap();
    for (state, failure, retry) in [
        (
            AdmissionState::Retryable,
            Some(invalid_retry_failure.clone()),
            Some(other_retry),
        ),
        (
            AdmissionState::Retryable,
            Some(failure(WorkPhase::Admission, FailureClass::Terminal, None)),
            Some(RetrySchedule::new(NonZeroU32::MIN, 30, invalid_retry_failure.clone()).unwrap()),
        ),
        (
            AdmissionState::Duplicate,
            Some(failure(WorkPhase::Admission, FailureClass::Terminal, None)),
            None,
        ),
        (
            AdmissionState::Cancelled,
            Some(failure(WorkPhase::Admission, FailureClass::Terminal, None)),
            Some(RetrySchedule::new(NonZeroU32::MIN, 30, invalid_retry_failure.clone()).unwrap()),
        ),
    ] {
        let mut artifact = signed_artifact(70);
        assert_eq!(
            artifact.record_admission(state, failure, retry, 12),
            Err(Error::InvalidAuthoredTransition)
        );
    }
}

fn delivery_request() -> DeliveryRequest {
    let targets = TargetSet::new(vec![
        Target::nostr_relay("wss://one.example").unwrap(),
        Target::nostr_relay("wss://two.example").unwrap(),
    ])
    .unwrap();
    DeliveryRequest::new(
        "settlement-delivery",
        DeliveryPayload::new(signed(&plan())),
        targets,
        SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        100,
    )
    .unwrap()
}

fn settlement_plan(value: u8, artifact: AuthoredArtifactId) -> AuthoredDeliveryPlan {
    AuthoredDeliveryPlan::new_bound(
        AuthoredDeliveryPlanId::new([value; 16]).unwrap(),
        artifact,
        delivery_request(),
        10,
    )
    .unwrap()
}

fn claim_delivery(plan: &AuthoredDeliveryPlan, token: u8) -> WorkClaim {
    WorkClaim::new(
        [token; 16],
        "delivery-worker",
        NonZeroU64::new(u64::from(token)).unwrap(),
        11,
        20,
        plan.revision(),
    )
    .unwrap()
}

fn delivery_receipt(plan: &AuthoredDeliveryPlan, outcome: DeliveryOutcome) -> DeliveryReceipt {
    let request = plan.request().unwrap();
    DeliveryReceipt::for_request(
        request,
        request
            .target_set()
            .targets()
            .iter()
            .cloned()
            .map(|target| DeliveryTargetReceipt::attempted(target, outcome.clone()))
            .collect(),
    )
    .unwrap()
}

#[test]
fn settlement_counts_every_artifact_and_delivery_terminal_class() {
    let authored_plan = plan();
    let mut artifacts = Vec::new();
    for ordinal in 0_u16..11 {
        artifacts.push(
            AuthoredArtifact::planned(
                artifact_id(80 + ordinal as u8),
                operation_id(),
                ordinal,
                &authored_plan,
                10,
            )
            .unwrap(),
        );
    }
    let signing_retry = failure(WorkPhase::Signing, FailureClass::Retryable, Some(20));
    artifacts[1]
        .record_signing_failure(
            signing_retry.clone(),
            Some(RetrySchedule::new(NonZeroU32::MIN, 20, signing_retry).unwrap()),
            11,
        )
        .unwrap();
    artifacts[2]
        .record_signing_failure(
            failure(WorkPhase::Signing, FailureClass::Indeterminate, None),
            None,
            11,
        )
        .unwrap();
    artifacts[3]
        .record_signing_failure(
            failure(WorkPhase::Signing, FailureClass::Terminal, None),
            None,
            11,
        )
        .unwrap();
    artifacts[4].cancel_signing(11).unwrap();
    for artifact in &mut artifacts[5..] {
        artifact.record_signed(signed(&authored_plan), 11).unwrap();
    }
    let admission_retry = failure(WorkPhase::Admission, FailureClass::Retryable, Some(20));
    artifacts[6]
        .record_admission(
            AdmissionState::Retryable,
            Some(admission_retry.clone()),
            Some(RetrySchedule::new(NonZeroU32::MIN, 20, admission_retry).unwrap()),
            12,
        )
        .unwrap();
    for (index, state) in [
        (7, AdmissionState::Rejected),
        (8, AdmissionState::Cancelled),
    ] {
        artifacts[index]
            .record_admission(
                state,
                Some(failure(WorkPhase::Admission, FailureClass::Terminal, None)),
                None,
                12,
            )
            .unwrap();
    }
    artifacts[9]
        .record_admission(AdmissionState::Inserted, None, None, 12)
        .unwrap();
    artifacts[10]
        .record_admission(AdmissionState::Duplicate, None, None, 12)
        .unwrap();

    let operation = AuthoredOperation::new(
        operation_id(),
        artifacts
            .iter()
            .map(AuthoredArtifact::artifact_id)
            .collect(),
        10,
    )
    .unwrap();
    let artifact = artifacts[9].artifact_id();
    let mut plans: Vec<_> = (100_u8..106)
        .map(|value| settlement_plan(value, artifact))
        .collect();
    let active = claim_delivery(&plans[1], 1);
    plans[1].claim(active.clone(), 11).unwrap();
    let pending = delivery_receipt(&plans[1], DeliveryOutcome::unavailable());
    let retry_failure = failure(WorkPhase::Delivery, FailureClass::Retryable, Some(20));
    plans[1]
        .apply_receipt(
            active.token(),
            active.generation(),
            active.row_revision(),
            pending,
            Some(RetrySchedule::new(NonZeroU32::MIN, 20, retry_failure).unwrap()),
            12,
        )
        .unwrap();
    let active = claim_delivery(&plans[2], 2);
    plans[2].claim(active.clone(), 11).unwrap();
    let accepted = delivery_receipt(&plans[2], DeliveryOutcome::accepted());
    plans[2]
        .apply_receipt(
            active.token(),
            active.generation(),
            active.row_revision(),
            accepted,
            None,
            12,
        )
        .unwrap();
    let active = claim_delivery(&plans[3], 3);
    plans[3].claim(active.clone(), 11).unwrap();
    let rejected = delivery_receipt(&plans[3], DeliveryOutcome::rejected());
    plans[3]
        .apply_receipt(
            active.token(),
            active.generation(),
            active.row_revision(),
            rejected,
            None,
            12,
        )
        .unwrap();
    let active = claim_delivery(&plans[4], 4);
    plans[4].claim(active.clone(), 11).unwrap();
    let request = plans[4].request().unwrap();
    let terminal = SinkFailure::for_request(
        request,
        "terminal",
        Retryability::Terminal,
        None,
        None,
        Vec::new(),
    )
    .unwrap();
    plans[4]
        .apply_sink_failure(
            active.token(),
            active.generation(),
            active.row_revision(),
            terminal,
            None,
            12,
        )
        .unwrap();
    plans[5].cancel(11).unwrap();

    let settlement =
        OperationSettlement::evaluate_complete(&operation, &artifacts, &plans).unwrap();
    assert_eq!(settlement.artifacts(), 11);
    assert_eq!(settlement.signed(), 6);
    assert_eq!(settlement.admitted(), 2);
    assert_eq!(settlement.pending(), 2);
    assert_eq!(settlement.retryable(), 2);
    assert_eq!(settlement.indeterminate(), 1);
    assert_eq!(settlement.failed_terminal(), 2);
    assert_eq!(settlement.cancelled(), 2);
    assert_eq!(settlement.delivery_plans(), 6);
    assert_eq!(settlement.delivery_satisfied(), 1);
    assert_eq!(settlement.delivery_pending(), 1);
    assert_eq!(settlement.delivery_retryable(), 1);
    assert_eq!(settlement.delivery_exhausted(), 1);
    assert_eq!(settlement.delivery_failed_terminal(), 1);
    assert_eq!(settlement.delivery_cancelled(), 1);
    assert!(!settlement.is_settled());
    assert!(settlement.has_failures());
    assert!(!settlement.is_successful());

    assert_eq!(
        OperationSettlement::evaluate_complete(
            &operation,
            &artifacts,
            &[plans[0].clone(), plans[0].clone()]
        ),
        Err(Error::InvalidAuthoredOperation)
    );
    let foreign = settlement_plan(110, artifact_id(120));
    assert_eq!(
        OperationSettlement::evaluate_complete(&operation, &artifacts, &[foreign]),
        Err(Error::InvalidAuthoredOperation)
    );
}
