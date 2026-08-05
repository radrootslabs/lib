use radroots_event::{SignedEvent, wire::v1::Nip01EventWire};
use radroots_transport::{
    DeliveryReceipt, DeliveryRequest, Error, SinkFailure, Target, TargetSet,
    outcome::{DeliveryOutcome, DeliveryOutcomeKind, Retryability},
    policy::{
        SatisfactionClass, SatisfactionPolicy, SatisfactionState, TargetPolicy,
        evaluate_satisfaction,
    },
    sink::{DeliveryPayload, DeliveryTargetReceipt},
};

fn payload() -> DeliveryPayload {
    let raw = r#"{"id":"56bfc78223bb2221bad82b539efdec1ade0f56d0eb0e1f592fd387df4b2ceee0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1700000001,"kind":0,"tags":[],"content":"{}","sig":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}"#;
    let wire = Nip01EventWire::parse_json(raw).expect("wire event");
    DeliveryPayload::new(
        SignedEvent::from_wire_verified_id(wire, raw).expect("signed delivery event"),
    )
}

fn targets() -> TargetSet {
    TargetSet::new(vec![
        Target::nostr_relay("wss://one.example").expect("first"),
        Target::nostr_relay("wss://two.example").expect("second"),
        Target::nostr_relay("wss://three.example").expect("third"),
    ])
    .expect("targets")
}

fn request(policy: SatisfactionPolicy) -> DeliveryRequest {
    DeliveryRequest::new(
        "delivery-request",
        payload(),
        targets(),
        policy,
        1_700_000_100_000,
    )
    .expect("delivery request")
}

fn mixed_receipt(request: &DeliveryRequest) -> DeliveryReceipt {
    let targets = request.target_set().targets();
    DeliveryReceipt::for_request(
        request,
        vec![
            DeliveryTargetReceipt::attempted(
                targets[2].clone(),
                DeliveryOutcome::unavailable()
                    .with_detail("offline", "relay unavailable")
                    .expect("normalized detail"),
            ),
            DeliveryTargetReceipt::attempted(targets[0].clone(), DeliveryOutcome::delivered()),
            DeliveryTargetReceipt::attempted(targets[1].clone(), DeliveryOutcome::accepted()),
        ],
    )
    .expect("mixed receipt")
}

#[test]
fn any_all_quorum_and_required_targets_are_exact() {
    let any = request(SatisfactionPolicy::new(
        SatisfactionClass::Accepted,
        TargetPolicy::any(),
    ));
    assert!(mixed_receipt(&any).is_satisfied(&any).expect("any"));

    let all = request(SatisfactionPolicy::new(
        SatisfactionClass::Accepted,
        TargetPolicy::all(),
    ));
    assert!(!mixed_receipt(&all).is_satisfied(&all).expect("all"));

    let quorum = request(SatisfactionPolicy::new(
        SatisfactionClass::Accepted,
        TargetPolicy::quorum(2).expect("quorum"),
    ));
    assert!(
        mixed_receipt(&quorum)
            .is_satisfied(&quorum)
            .expect("quorum")
    );

    let delivered = request(SatisfactionPolicy::new(
        SatisfactionClass::Delivered,
        TargetPolicy::quorum(2).expect("quorum"),
    ));
    assert!(
        !mixed_receipt(&delivered)
            .is_satisfied(&delivered)
            .expect("delivered quorum")
    );

    let selected = targets();
    let required = TargetPolicy::required(vec![
        selected.targets()[0].fingerprint().clone(),
        selected.targets()[1].fingerprint().clone(),
    ])
    .expect("required targets");
    let required_request = request(SatisfactionPolicy::new(
        SatisfactionClass::Accepted,
        required,
    ));
    assert!(
        mixed_receipt(&required_request)
            .is_satisfied(&required_request)
            .expect("required")
    );

    let unsatisfied = TargetPolicy::required(vec![
        selected.targets()[0].fingerprint().clone(),
        selected.targets()[2].fingerprint().clone(),
    ])
    .expect("required targets");
    let unsatisfied_request = request(SatisfactionPolicy::new(
        SatisfactionClass::Accepted,
        unsatisfied,
    ));
    assert!(
        !mixed_receipt(&unsatisfied_request)
            .is_satisfied(&unsatisfied_request)
            .expect("required unsatisfied")
    );
}

#[test]
fn policies_and_requests_reject_empty_duplicate_and_impossible_inputs() {
    assert_eq!(
        TargetPolicy::quorum(0).expect_err("zero quorum"),
        Error::InvalidSatisfactionPolicy
    );
    assert_eq!(
        TargetPolicy::required(Vec::new()).expect_err("empty required"),
        Error::EmptyRequiredTargetSet
    );
    let set = targets();
    let duplicate = set.targets()[0].fingerprint().clone();
    assert_eq!(
        TargetPolicy::required(vec![duplicate.clone(), duplicate]).expect_err("duplicate required"),
        Error::DuplicateRequiredTargetFingerprint
    );
    assert_eq!(
        DeliveryRequest::new(
            "request",
            payload(),
            set.clone(),
            SatisfactionPolicy::new(
                SatisfactionClass::Accepted,
                TargetPolicy::quorum(4).expect("nonzero quorum"),
            ),
            1,
        )
        .expect_err("impossible quorum"),
        Error::InvalidSatisfactionPolicy
    );
    assert_eq!(
        DeliveryRequest::new(
            "",
            payload(),
            set.clone(),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::any()),
            1,
        )
        .expect_err("empty id"),
        Error::EmptyDeliveryRequestId
    );
    assert_eq!(
        DeliveryRequest::new(
            "request",
            payload(),
            set,
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::any()),
            0,
        )
        .expect_err("zero deadline"),
        Error::InvalidDeliveryDeadline
    );
    assert_eq!(
        TargetSet::new(Vec::new()).expect_err("empty target set"),
        Error::EmptyTargetSet
    );
}

#[test]
fn receipts_reject_duplicate_missing_unexpected_and_false_attempts() {
    let request = request(SatisfactionPolicy::new(
        SatisfactionClass::Accepted,
        TargetPolicy::all(),
    ));
    let first = request.target_set().targets()[0].clone();
    let second = request.target_set().targets()[1].clone();
    let third = request.target_set().targets()[2].clone();
    let accepted = DeliveryTargetReceipt::attempted(first.clone(), DeliveryOutcome::accepted());
    assert_eq!(
        DeliveryTargetReceipt::skipped(first.clone(), DeliveryOutcome::accepted())
            .expect_err("unattempted success"),
        Error::DeliveryTargetReceiptAttemptMismatch
    );
    assert_eq!(
        DeliveryReceipt::for_request(&request, vec![accepted.clone(), accepted])
            .expect_err("duplicate receipt"),
        Error::DuplicateDeliveryTargetReceipt
    );
    assert_eq!(
        DeliveryReceipt::for_request(
            &request,
            vec![
                DeliveryTargetReceipt::attempted(first, DeliveryOutcome::accepted()),
                DeliveryTargetReceipt::attempted(second, DeliveryOutcome::accepted()),
            ],
        )
        .expect_err("missing receipt"),
        Error::MissingDeliveryTargetReceipt
    );
    let foreign = Target::nostr_relay("wss://foreign.example").expect("foreign");
    assert_eq!(
        DeliveryReceipt::for_request(
            &request,
            vec![
                DeliveryTargetReceipt::attempted(foreign, DeliveryOutcome::accepted()),
                DeliveryTargetReceipt::attempted(third, DeliveryOutcome::accepted()),
            ],
        )
        .expect_err("unexpected receipt"),
        Error::UnexpectedDeliveryTargetReceipt
    );
}

#[test]
fn retryability_and_terminality_are_explicit_normalized_data() {
    let unavailable = DeliveryOutcome::unavailable();
    assert_eq!(unavailable.kind(), DeliveryOutcomeKind::Unavailable);
    assert_eq!(unavailable.retryability(), Retryability::Retryable);
    assert!(unavailable.is_retryable());
    assert!(!unavailable.is_terminal());

    let rejected = DeliveryOutcome::rejected();
    assert!(rejected.is_terminal());
    assert!(!rejected.is_retryable());
    assert_eq!(
        DeliveryOutcome::failed(Retryability::NotApplicable).expect_err("unclassified failure"),
        Error::InvalidDeliveryOutcome
    );
    assert!(
        DeliveryOutcome::failed(Retryability::Retryable)
            .expect("retryable failure")
            .is_retryable()
    );
    assert!(
        DeliveryOutcome::failed(Retryability::Terminal)
            .expect("terminal failure")
            .is_terminal()
    );
    assert_eq!(
        DeliveryOutcome::unavailable()
            .with_detail("INVALID", "relay unavailable")
            .expect_err("invalid code"),
        Error::InvalidDeliveryOutcome
    );
}

#[test]
fn canonical_evaluator_covers_pending_exhausted_and_historical_evidence() {
    let set = targets();
    let policy = SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all());
    assert_eq!(
        evaluate_satisfaction(&policy, &set, core::iter::empty()).expect("empty evidence"),
        SatisfactionState::Pending
    );

    let first = set.targets()[0].fingerprint();
    let second = set.targets()[1].fingerprint();
    let third = set.targets()[2].fingerprint();
    let accepted = DeliveryOutcome::accepted();
    let terminal = DeliveryOutcome::rejected();
    let retryable = DeliveryOutcome::unavailable();
    assert_eq!(
        evaluate_satisfaction(
            &policy,
            &set,
            [(first, &accepted), (second, &terminal), (third, &retryable),],
        )
        .expect("mixed evidence"),
        SatisfactionState::Exhausted
    );
    assert_eq!(
        evaluate_satisfaction(
            &policy,
            &set,
            [
                (first, &accepted),
                (second, &accepted),
                (third, &accepted),
                (first, &terminal),
            ],
        )
        .expect("historical success"),
        SatisfactionState::Satisfied
    );

    let foreign = Target::nostr_relay("wss://foreign.example").expect("foreign");
    assert_eq!(
        evaluate_satisfaction(&policy, &set, [(foreign.fingerprint(), &accepted)])
            .expect_err("foreign evidence"),
        Error::UnexpectedDeliveryTargetReceipt
    );
}

#[test]
fn policy_introspection_validation_and_wire_variants_are_complete() {
    let set = targets();
    let any = TargetPolicy::any();
    assert!(any.is_any());
    assert!(!any.is_all());
    assert_eq!(any.quorum_threshold(), None);
    assert_eq!(any.required_targets(), None);

    let all = TargetPolicy::all();
    assert!(!all.is_any());
    assert!(all.is_all());
    assert_eq!(all.quorum_threshold(), None);
    assert_eq!(all.required_targets(), None);

    let quorum = TargetPolicy::quorum(2).expect("quorum");
    assert!(!quorum.is_any());
    assert!(!quorum.is_all());
    assert_eq!(quorum.quorum_threshold(), Some(2));
    assert_eq!(quorum.required_targets(), None);

    let fingerprint = set.targets()[0].fingerprint().clone();
    let required = TargetPolicy::required(vec![fingerprint.clone()]).expect("required");
    assert!(!required.is_any());
    assert!(!required.is_all());
    assert_eq!(required.quorum_threshold(), None);
    assert_eq!(
        required.required_targets(),
        Some(core::slice::from_ref(&fingerprint))
    );

    let policy = SatisfactionPolicy::new(SatisfactionClass::Delivered, required);
    assert_eq!(policy.class(), SatisfactionClass::Delivered);
    assert_eq!(
        policy.targets().required_targets(),
        Some(core::slice::from_ref(&fingerprint))
    );
    policy
        .validate_for(&set)
        .expect("required target is requested");

    let foreign = Target::nostr_relay("wss://foreign.example").expect("foreign");
    let impossible = SatisfactionPolicy::new(
        SatisfactionClass::Accepted,
        TargetPolicy::required(vec![foreign.fingerprint().clone()]).expect("required"),
    );
    assert_eq!(
        impossible.validate_for(&set).unwrap_err(),
        Error::RequiredTargetNotRequested
    );

    #[cfg(feature = "serde")]
    for (wire, assertion) in [
        (serde_json::json!("any"), 0_u8),
        (serde_json::json!("all"), 1),
        (serde_json::json!({"quorum": 2}), 2),
        (serde_json::json!({"required": [fingerprint.as_str()]}), 3),
    ] {
        let decoded: TargetPolicy = serde_json::from_value(wire).expect("target policy");
        match assertion {
            0 => assert!(decoded.is_any()),
            1 => assert!(decoded.is_all()),
            2 => assert_eq!(decoded.quorum_threshold(), Some(2)),
            3 => assert_eq!(
                decoded.required_targets(),
                Some(core::slice::from_ref(&fingerprint))
            ),
            _ => unreachable!(),
        }
    }

    #[cfg(feature = "serde")]
    for invalid in [
        serde_json::json!({"quorum": 0}),
        serde_json::json!({"required": []}),
        serde_json::json!({"required": [fingerprint.as_str(), fingerprint.as_str()]}),
    ] {
        assert!(serde_json::from_value::<TargetPolicy>(invalid).is_err());
    }
}

#[test]
fn sink_failures_retain_validated_retry_timing_and_partial_evidence() {
    let request = request(SatisfactionPolicy::new(
        SatisfactionClass::Accepted,
        TargetPolicy::all(),
    ));
    let partial = DeliveryTargetReceipt::attempted(
        request.target_set().targets()[0].clone(),
        DeliveryOutcome::accepted(),
    );
    let failure = SinkFailure::for_request(
        &request,
        "relay_batch_unavailable",
        Retryability::Retryable,
        Some(1_700_000_200_000),
        Some("relay batch unavailable".to_owned()),
        vec![partial.clone()],
    )
    .expect("sink failure");
    assert_eq!(failure.code(), "relay_batch_unavailable");
    assert_eq!(failure.retryability(), Retryability::Retryable);
    assert_eq!(failure.retry_after_unix_ms(), Some(1_700_000_200_000));
    assert_eq!(failure.message(), Some("relay batch unavailable"));
    assert_eq!(failure.partial_evidence(), core::slice::from_ref(&partial));
    assert_eq!(
        SinkFailure::for_request(
            &request,
            "terminal_failure",
            Retryability::Terminal,
            Some(1),
            None,
            Vec::new(),
        )
        .expect_err("terminal retry timing"),
        Error::InvalidDeliveryOutcome
    );
    assert_eq!(
        SinkFailure::for_request(
            &request,
            "invalid_retry_time",
            Retryability::Retryable,
            Some(0),
            None,
            Vec::new(),
        )
        .expect_err("zero retry timing"),
        Error::InvalidDeliveryOutcome
    );
    assert_eq!(
        SinkFailure::for_request(
            &request,
            "duplicate_evidence",
            Retryability::Retryable,
            None,
            None,
            vec![partial.clone(), partial],
        )
        .expect_err("duplicate evidence"),
        Error::DuplicateDeliveryTargetReceipt
    );
}

#[test]
#[cfg(feature = "serde")]
fn serde_revalidates_policy_outcome_and_receipt_invariants() {
    let request = request(SatisfactionPolicy::new(
        SatisfactionClass::Accepted,
        TargetPolicy::all(),
    ));
    let receipt = mixed_receipt(&request);
    let encoded_request = serde_json::to_string(&request).expect("request json");
    assert_eq!(
        serde_json::from_str::<DeliveryRequest>(&encoded_request).expect("request round trip"),
        request
    );
    let encoded_receipt = serde_json::to_string(&receipt).expect("receipt json");
    assert_eq!(
        serde_json::from_str::<DeliveryReceipt>(&encoded_receipt).expect("receipt round trip"),
        receipt
    );

    let mut forged_outcome =
        serde_json::to_value(DeliveryOutcome::accepted()).expect("outcome value");
    forged_outcome["retryability"] = serde_json::json!("retryable");
    assert!(serde_json::from_value::<DeliveryOutcome>(forged_outcome).is_err());

    let mut forged_attempt = serde_json::to_value(&receipt).expect("receipt value");
    forged_attempt["target_receipts"][0]["attempted"] = false.into();
    assert!(serde_json::from_value::<DeliveryReceipt>(forged_attempt).is_err());

    let mut missing = serde_json::to_value(&receipt).expect("receipt value");
    missing["target_receipts"]
        .as_array_mut()
        .expect("receipts array")
        .pop();
    assert!(serde_json::from_value::<DeliveryReceipt>(missing).is_err());

    let failure = SinkFailure::for_request(
        &request,
        "relay_unavailable",
        Retryability::Retryable,
        Some(1_700_000_200_000),
        None,
        vec![receipt.target_receipts()[0].clone()],
    )
    .expect("sink failure");
    let encoded_failure = serde_json::to_string(&failure).expect("failure json");
    assert_eq!(
        serde_json::from_str::<SinkFailure>(&encoded_failure).expect("failure round trip"),
        failure
    );
}
