use futures::executor::block_on;
use radroots_event::{SignedEvent, wire::Nip01EventWire};
use radroots_transport::{
    DeliveryRequest, Error as TransportError, EventSink, EventSource, FetchRequest, Target,
    TargetSet, TransportId,
    capability::{Availability, Maturity},
    outcome::{DeliveryOutcomeKind, FetchTargetState},
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::DeliveryPayload,
    source::FetchBounds,
    target::{TargetLabel, TargetScope},
};
use radroots_transport_reticulum::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RADROOTS_RETICULUM_SCOPE_ID,
    RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE, RETICULUM_V1_MAX_PAYLOAD_BYTES,
    RadrootsReticulumAgentEndpoint, RadrootsReticulumBehavior, RadrootsReticulumEndpoint,
    RadrootsReticulumError, RadrootsReticulumProfile, RadrootsReticulumTransport,
    ReticulumCapabilityReportV1, ReticulumDestinationV1, ReticulumDuplicateFragmentBehaviorV1,
    ReticulumFragmentIntegrityV1, ReticulumFragmentPolicyV1, ReticulumFragmentationModeV1,
    ReticulumGatewaySemanticsV1, ReticulumPayloadPolicyV1, ReticulumPrivacySemanticsV1,
    ReticulumRoutingMetadataV1,
};

fn signed_event() -> SignedEvent {
    let raw = r#"{"id":"56bfc78223bb2221bad82b539efdec1ade0f56d0eb0e1f592fd387df4b2ceee0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1700000001,"kind":0,"tags":[],"content":"{}","sig":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}"#;
    SignedEvent::from_wire_verified_id(Nip01EventWire::parse_json(raw).expect("wire event"), raw)
        .expect("signed event")
}

fn scope() -> TargetScope {
    TargetScope::parse(RADROOTS_RETICULUM_SCOPE_ID).expect("Reticulum scope")
}

fn target() -> Target {
    ReticulumDestinationV1::local()
        .transport_target()
        .expect("Reticulum target")
}

fn target_set(target: Target) -> TargetSet {
    TargetSet::new(vec![target]).expect("target set")
}

fn delivery(target: Target) -> DeliveryRequest {
    DeliveryRequest::new(
        "reticulum-delivery",
        DeliveryPayload::new(signed_event()),
        target_set(target),
        SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        1_800_000_200_000,
    )
    .expect("delivery request")
}

fn fetch(target: Target) -> FetchRequest {
    FetchRequest::new(
        "reticulum-fetch",
        target_set(target),
        FetchBounds::new(10, 1_800_000_200_000).expect("fetch bounds"),
    )
    .expect("fetch request")
}

#[test]
fn payload_fragment_and_routing_policies_are_explicitly_inert() {
    let fragments = ReticulumFragmentPolicyV1::unsupported();
    assert_eq!(fragments.mode, ReticulumFragmentationModeV1::Unsupported);
    assert_eq!(fragments.max_fragment_count, 1);
    assert_eq!(
        fragments.max_reassembled_bytes,
        RETICULUM_V1_MAX_PAYLOAD_BYTES
    );
    assert_eq!(
        fragments.duplicate_fragment_behavior,
        ReticulumDuplicateFragmentBehaviorV1::Reject
    );
    assert_eq!(
        fragments.integrity_verification,
        ReticulumFragmentIntegrityV1::PayloadDigest
    );

    let payload = ReticulumPayloadPolicyV1::v1();
    assert_eq!(payload.max_payload_bytes, RETICULUM_V1_MAX_PAYLOAD_BYTES);
    assert_eq!(payload.fragment_policy, fragments);

    let routing = ReticulumRoutingMetadataV1::local();
    assert_eq!(routing.scope.as_str(), RADROOTS_RETICULUM_SCOPE_ID);
    assert_eq!(
        routing.gateway,
        ReticulumGatewaySemanticsV1::NoGatewayForwarding
    );
    assert_eq!(
        routing.privacy,
        ReticulumPrivacySemanticsV1::CanonicalSignedEventBytesOnly
    );

    let report = ReticulumCapabilityReportV1::unavailable_local();
    assert!(report.delivery_required);
    assert!(!report.fetch_required);
    assert!(!report.can_deliver);
    assert!(!report.can_fetch);
    assert!(!report.can_discover);
    assert!(!report.can_forward_gateway);
    assert!(!report.can_observe_receipts);
    assert_eq!(report.payload_policy, payload);
}

#[test]
fn endpoint_and_agent_endpoint_values_are_strict_and_canonical() {
    let endpoint = RadrootsReticulumEndpoint::default();
    assert_eq!(endpoint.as_str(), RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(endpoint.to_string(), RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(
        endpoint.clone().into_string(),
        RADROOTS_RETICULUM_ENDPOINT_URI
    );
    assert_eq!(
        RadrootsReticulumEndpoint::parse(RADROOTS_RETICULUM_ENDPOINT_URI).unwrap(),
        endpoint
    );
    for invalid in ["", " ", "RETICULUM:local", "reticulum:remote"] {
        assert_eq!(
            RadrootsReticulumEndpoint::parse(invalid).unwrap_err(),
            RadrootsReticulumError::InvalidEndpoint
        );
    }

    for valid in [
        "reticulum-agent:local-controller",
        "reticulum-agent://localhost:19999",
    ] {
        let agent = RadrootsReticulumAgentEndpoint::parse(valid).expect("agent endpoint");
        assert_eq!(agent.as_str(), valid);
        assert_eq!(agent.to_string(), valid);
        assert_eq!(agent.into_string(), valid);
    }
    for invalid in [
        "",
        " reticulum-agent:local",
        "reticulum-agent:local ",
        "reticulum-agent:local\n",
        "reticulum agent:local",
        "RETICULUM-AGENT:local",
        "reticulum-agent:",
    ] {
        assert_eq!(
            RadrootsReticulumAgentEndpoint::parse(invalid).unwrap_err(),
            RadrootsReticulumError::InvalidAgentEndpoint,
            "{invalid:?}"
        );
    }
}

#[test]
fn destinations_preserve_transport_identity_scope_and_optional_labels() {
    let local = ReticulumDestinationV1::local();
    assert_eq!(local.uri().as_str(), RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(local.routing().scope.as_str(), RADROOTS_RETICULUM_SCOPE_ID);
    assert_eq!(local.label(), None);

    let label = TargetLabel::parse("Local node").expect("label");
    let labeled = ReticulumDestinationV1::new(
        RADROOTS_RETICULUM_ENDPOINT_URI,
        TargetScope::parse("farm.mesh").expect("scope"),
        Some(label.clone()),
    )
    .expect("destination");
    assert_eq!(labeled.label(), Some(&label));
    assert_ne!(labeled.fingerprint(), local.fingerprint());

    let transport_target = labeled.transport_target().expect("transport target");
    assert_eq!(transport_target.kind(), &TransportId::RETICULUM);
    assert_eq!(
        ReticulumDestinationV1::from_target(&transport_target).expect("round trip"),
        labeled
    );

    let local_target = Target::local("local:memory").expect("local target");
    assert_eq!(
        ReticulumDestinationV1::from_target(&local_target).unwrap_err(),
        TransportError::InvalidTargetUri
    );
    let wrong_uri = Target::new_with_metadata(
        TransportId::RETICULUM,
        "reticulum:remote",
        Some(scope()),
        None,
    )
    .expect("wrong URI target");
    assert_eq!(
        ReticulumDestinationV1::from_target(&wrong_uri).unwrap_err(),
        TransportError::InvalidTargetUri
    );
    let missing_scope =
        Target::new(TransportId::RETICULUM, RADROOTS_RETICULUM_ENDPOINT_URI).expect("target");
    assert_eq!(
        ReticulumDestinationV1::from_target(&missing_scope).unwrap_err(),
        TransportError::EmptyTargetScope
    );
}

#[test]
fn profiles_expose_canonical_preview_configuration() {
    let default = RadrootsReticulumProfile::default();
    assert_eq!(default.profile_id(), "transport.reticulum.default");
    assert_eq!(default.endpoint().as_str(), RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(default.scope().as_str(), RADROOTS_RETICULUM_SCOPE_ID);
    assert_eq!(default.agent_endpoint(), None);
    assert_eq!(
        default.behavior(),
        RadrootsReticulumBehavior::RejectDeliveryAttempts
    );
    assert_eq!(
        default.destination(),
        &default.capability_report().destination
    );

    assert_eq!(
        RadrootsReticulumProfile::new(
            "",
            RadrootsReticulumEndpoint::default(),
            scope(),
            None,
            RadrootsReticulumBehavior::RejectDeliveryAttempts,
        )
        .unwrap_err(),
        RadrootsReticulumError::InvalidProfileId
    );
    assert_eq!(
        RadrootsReticulumProfile::new(
            "transport reticulum",
            RadrootsReticulumEndpoint::default(),
            scope(),
            None,
            RadrootsReticulumBehavior::RejectDeliveryAttempts,
        )
        .unwrap_err(),
        RadrootsReticulumError::InvalidProfileId
    );

    let agent =
        RadrootsReticulumAgentEndpoint::parse("reticulum-agent:local").expect("agent endpoint");
    let custom = RadrootsReticulumProfile::new(
        "transport.reticulum.custom",
        RadrootsReticulumEndpoint::default(),
        TargetScope::parse("farm.mesh").expect("scope"),
        Some(agent.clone()),
        RadrootsReticulumBehavior::DeferDeliveryPlans,
    )
    .expect("profile");
    assert_eq!(custom.profile_id(), "transport.reticulum.custom");
    assert_eq!(custom.agent_endpoint(), Some(&agent));
    assert_eq!(
        custom.behavior(),
        RadrootsReticulumBehavior::DeferDeliveryPlans
    );

    let updated = default
        .with_behavior(RadrootsReticulumBehavior::DeferDeliveryPlans)
        .with_agent_endpoint(agent.clone());
    assert_eq!(updated.agent_endpoint(), Some(&agent));
    assert_eq!(
        updated.behavior(),
        RadrootsReticulumBehavior::DeferDeliveryPlans
    );
    assert_eq!(
        RadrootsReticulumBehavior::default(),
        RadrootsReticulumBehavior::RejectDeliveryAttempts
    );
}

#[test]
fn canonical_source_and_sink_return_request_bound_unavailable_evidence() {
    let transport = RadrootsReticulumTransport::default();
    assert_eq!(
        transport.profile().profile_id(),
        "transport.reticulum.default"
    );
    let transport = RadrootsReticulumTransport::new(transport.profile().clone());

    let sink_status = block_on(EventSink::status(&transport)).expect("sink status");
    assert_eq!(sink_status.transport_id(), TransportId::RETICULUM);
    assert_eq!(sink_status.maturity(), Maturity::Preview);
    assert_eq!(sink_status.availability(), Availability::Unavailable);
    assert!(!sink_status.capabilities().can_deliver());
    assert_eq!(
        sink_status.message(),
        RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE
    );

    let source_status = block_on(EventSource::status(&transport)).expect("source status");
    assert_eq!(source_status.transport_id(), TransportId::RETICULUM);
    assert_eq!(source_status.maturity(), Maturity::Preview);
    assert_eq!(source_status.availability(), Availability::Unavailable);
    assert!(!source_status.capabilities().can_fetch());
    assert_eq!(
        source_status.message(),
        RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE
    );

    let delivery = delivery(target());
    let receipt =
        block_on(EventSink::deliver(&transport, delivery.clone())).expect("delivery receipt");
    receipt
        .validate_for_request(&delivery)
        .expect("request bound");
    assert_eq!(receipt.target_receipts().len(), 1);
    assert!(!receipt.target_receipts()[0].was_attempted());
    assert_eq!(
        receipt.target_receipts()[0].outcome().kind(),
        DeliveryOutcomeKind::Unavailable
    );
    assert_eq!(
        receipt.target_receipts()[0].outcome().code(),
        Some("transport_unavailable")
    );

    let fetch = fetch(target());
    let page = block_on(EventSource::fetch(&transport, fetch.clone())).expect("fetch page");
    page.validate_for_request(&fetch).expect("request bound");
    assert!(page.events().is_empty());
    assert_eq!(page.target_outcomes().len(), 1);
    assert_eq!(
        page.target_outcomes()[0].state(),
        FetchTargetState::Unavailable
    );
    assert_eq!(
        page.target_outcomes()[0].message(),
        Some(RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE)
    );
}

#[test]
fn source_and_sink_reject_noncanonical_targets_before_adapter_effects() {
    let transport = RadrootsReticulumTransport::default();
    let invalid_targets = [
        Target::nostr_relay("wss://relay.example").expect("Nostr target"),
        Target::new_with_metadata(
            TransportId::RETICULUM,
            "reticulum:remote",
            Some(scope()),
            None,
        )
        .expect("wrong URI target"),
        Target::new(TransportId::RETICULUM, RADROOTS_RETICULUM_ENDPOINT_URI)
            .expect("unscoped target"),
    ];

    for invalid in invalid_targets {
        let failure = block_on(EventSink::deliver(&transport, delivery(invalid.clone())))
            .expect_err("sink must reject target");
        assert_eq!(failure.code(), "invalid_transport_contract");
        assert_eq!(
            block_on(EventSource::fetch(&transport, fetch(invalid))).unwrap_err(),
            TransportError::InvalidTargetUri
        );
    }
}

#[test]
fn reticulum_error_messages_are_stable() {
    let cases = [
        (
            RadrootsReticulumError::InvalidEndpoint,
            "invalid Reticulum endpoint",
        ),
        (
            RadrootsReticulumError::InvalidAgentEndpoint,
            "invalid Reticulum agent endpoint",
        ),
        (
            RadrootsReticulumError::InvalidProfileId,
            "invalid Reticulum profile id",
        ),
        (
            RadrootsReticulumError::NonReticulumTarget,
            "Reticulum transport received a non-Reticulum target",
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
#[cfg(feature = "serde")]
fn public_models_round_trip_and_destination_identity_is_revalidated() {
    let profile = RadrootsReticulumProfile::default()
        .with_behavior(RadrootsReticulumBehavior::DeferDeliveryPlans);
    let encoded = serde_json::to_string(&profile).expect("profile JSON");
    assert_eq!(
        serde_json::from_str::<RadrootsReticulumProfile>(&encoded).expect("profile round trip"),
        profile
    );

    let destination = ReticulumDestinationV1::new(
        RADROOTS_RETICULUM_ENDPOINT_URI,
        scope(),
        Some(TargetLabel::parse("Local node").expect("label")),
    )
    .expect("destination");
    let value = serde_json::to_value(&destination).expect("destination JSON");
    assert_eq!(
        serde_json::from_value::<ReticulumDestinationV1>(value.clone())
            .expect("destination round trip"),
        destination
    );

    for (path, forged) in [
        ("uri", serde_json::json!("reticulum:remote")),
        ("label", serde_json::json!(" Other node ")),
        ("fingerprint", serde_json::json!("0".repeat(64))),
    ] {
        let mut invalid = value.clone();
        invalid[path] = forged;
        assert!(
            serde_json::from_value::<ReticulumDestinationV1>(invalid).is_err(),
            "forged {path} must fail"
        );
    }

    let mut routing = value;
    routing["routing"]["scope"] = serde_json::json!("remote");
    assert!(serde_json::from_value::<ReticulumDestinationV1>(routing).is_err());
}
