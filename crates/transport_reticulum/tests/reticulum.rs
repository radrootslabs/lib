use radroots_transport::capability::{Availability, Maturity};
use radroots_transport::sink::EventSink;
use radroots_transport::source::{EventSource, FetchBounds, FetchRequest};
use radroots_transport::target::TargetScope;
use radroots_transport::{
    RadrootsTransportCapabilityAvailability, RadrootsTransportCapabilityMaturity,
    RadrootsTransportDeliveryRequest, RadrootsTransportDeliveryTargetStatus,
    RadrootsTransportImplementationState, RadrootsTransportPayload,
    RadrootsTransportSatisfactionClass, RadrootsTransportSatisfactionPolicy, Target, TargetSet,
    TransportId,
};
use radroots_transport_reticulum::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RADROOTS_RETICULUM_SCOPE_ID,
    RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE, RadrootsReticulumAgentEndpoint,
    RadrootsReticulumBehavior, RadrootsReticulumEndpoint, RadrootsReticulumError,
    RadrootsReticulumFetchRequest, RadrootsReticulumProfile, RadrootsReticulumTransport,
    ReticulumDestinationV1, ReticulumDuplicateFragmentBehaviorV1, ReticulumFragmentIntegrityV1,
    ReticulumFragmentationModeV1, ReticulumGatewaySemanticsV1, ReticulumPrivacySemanticsV1,
};
#[cfg(feature = "serde")]
use serde_json::Value;

fn reticulum_target(uri: &str) -> Target {
    assert_eq!(uri, RADROOTS_RETICULUM_ENDPOINT_URI);
    ReticulumDestinationV1::local()
        .transport_target()
        .expect("reticulum target")
}

fn scoped_reticulum_target(scope: &str) -> Target {
    Target::new_with_metadata(
        TransportId::RETICULUM,
        RADROOTS_RETICULUM_ENDPOINT_URI,
        Some(TargetScope::parse(scope).expect("scope")),
        None,
    )
    .expect("scoped reticulum target")
}

fn nostr_target() -> Target {
    Target::new(TransportId::NOSTR, "wss://relay.example").expect("nostr target")
}

fn delivery_request(targets: Vec<Target>) -> RadrootsTransportDeliveryRequest {
    RadrootsTransportDeliveryRequest::new(
        "reticulum-delivery",
        reticulum_payload(),
        TargetSet::new(targets).expect("target set"),
        RadrootsTransportSatisfactionPolicy::any_accepted(),
    )
    .expect("delivery request")
}

fn reticulum_payload() -> RadrootsTransportPayload {
    RadrootsTransportPayload::mesh_frame_cbor("reticulum-message", [1_u8, 2, 3]).expect("payload")
}

#[test]
fn default_profile_is_configured_deferred_until_implemented_and_rejecting() {
    let profile = RadrootsReticulumProfile::default();
    let status = profile.status();

    assert_eq!(profile.profile_id(), "transport.reticulum.default");
    assert_eq!(profile.endpoint().as_str(), RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(profile.scope().as_str(), RADROOTS_RETICULUM_SCOPE_ID);
    assert_eq!(profile.agent_endpoint(), None);
    assert_eq!(
        profile.behavior(),
        RadrootsReticulumBehavior::RejectDeliveryAttempts
    );
    assert_eq!(
        status.transport_status.implementation,
        RadrootsTransportImplementationState::Real
    );
    assert_eq!(
        status.transport_status.maturity,
        RadrootsTransportCapabilityMaturity::Preview
    );
    assert_eq!(
        status.transport_status.availability,
        RadrootsTransportCapabilityAvailability::Unavailable
    );
    assert!(status.transport_status.configured);
    assert_eq!(
        status.transport_status.profile_id.as_deref(),
        Some("transport.reticulum.default")
    );
    assert_eq!(
        status.transport_status.endpoint_uri.as_deref(),
        Some(RADROOTS_RETICULUM_ENDPOINT_URI)
    );
    assert_eq!(
        profile.destination().uri().as_str(),
        RADROOTS_RETICULUM_ENDPOINT_URI
    );
    assert_eq!(
        profile.destination().routing().scope.as_str(),
        RADROOTS_RETICULUM_SCOPE_ID
    );
    assert_eq!(
        status.destination.routing().gateway,
        ReticulumGatewaySemanticsV1::NoGatewayForwarding
    );
    assert_eq!(
        status.destination.routing().privacy,
        ReticulumPrivacySemanticsV1::CanonicalSignedEventBytesOnly
    );
    assert!(status.capability_report.delivery_required);
    assert!(!status.capability_report.fetch_required);
    assert!(!status.capability_report.can_deliver);
    assert!(!status.capability_report.can_fetch);
    assert!(!status.capability_report.can_discover);
    assert!(!status.capability_report.can_forward_gateway);
    assert!(!status.capability_report.can_observe_receipts);
    assert_eq!(
        status.capability_report.destination.fingerprint(),
        status.destination.fingerprint()
    );
    assert_eq!(
        status.capability_report.payload_policy.fragment_policy.mode,
        ReticulumFragmentationModeV1::Unsupported
    );
    assert_eq!(
        status
            .capability_report
            .payload_policy
            .fragment_policy
            .max_fragment_count,
        1
    );
    assert_eq!(
        status
            .capability_report
            .payload_policy
            .fragment_policy
            .duplicate_fragment_behavior,
        ReticulumDuplicateFragmentBehaviorV1::Reject
    );
    assert_eq!(
        status
            .capability_report
            .payload_policy
            .fragment_policy
            .integrity_verification,
        ReticulumFragmentIntegrityV1::PayloadDigest
    );
    assert_eq!(
        status.transport_status.message,
        RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE
    );
    assert!(!status.transport_status.usable_for_delivery);
    assert_eq!(status.scope.as_str(), RADROOTS_RETICULUM_SCOPE_ID);
    assert_eq!(status.agent_endpoint, None);
}

#[test]
fn endpoint_and_profile_validation_are_strict_and_canonical() {
    let endpoint =
        RadrootsReticulumEndpoint::parse(RADROOTS_RETICULUM_ENDPOINT_URI).expect("endpoint");
    assert_eq!(endpoint.as_str(), RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(endpoint.to_string(), RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(
        endpoint.clone().into_string(),
        RADROOTS_RETICULUM_ENDPOINT_URI
    );
    assert_eq!(
        RadrootsReticulumEndpoint::default().as_str(),
        RADROOTS_RETICULUM_ENDPOINT_URI
    );

    assert_eq!(
        RadrootsReticulumEndpoint::parse(" ").expect_err("empty endpoint"),
        RadrootsReticulumError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumEndpoint::parse("reticulum:").expect_err("empty endpoint"),
        RadrootsReticulumError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumEndpoint::parse("https://target").expect_err("wrong scheme"),
        RadrootsReticulumError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumEndpoint::parse("RETICULUM:unavailable").expect_err("case drift endpoint"),
        RadrootsReticulumError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumEndpoint::parse(" reticulum:unavailable")
            .expect_err("leading whitespace endpoint"),
        RadrootsReticulumError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumEndpoint::parse("reticulum:unavailable ")
            .expect_err("trailing whitespace endpoint"),
        RadrootsReticulumError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumEndpoint::parse("reticulum:custom").expect_err("custom endpoint"),
        RadrootsReticulumError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumEndpoint::parse("reticulum:bad target").expect_err("whitespace endpoint"),
        RadrootsReticulumError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumEndpoint::parse("reticulum:bad\ntarget").expect_err("control endpoint"),
        RadrootsReticulumError::InvalidEndpoint
    );
    let agent_endpoint = RadrootsReticulumAgentEndpoint::parse("reticulum-agent://localhost:19999")
        .expect("agent endpoint");
    assert_eq!(agent_endpoint.as_str(), "reticulum-agent://localhost:19999");
    assert_eq!(
        agent_endpoint.to_string(),
        "reticulum-agent://localhost:19999"
    );
    assert_eq!(
        agent_endpoint.clone().into_string(),
        "reticulum-agent://localhost:19999"
    );
    let local_agent_endpoint =
        RadrootsReticulumAgentEndpoint::parse("reticulum-agent:local-controller")
            .expect("local agent endpoint");
    assert_eq!(
        local_agent_endpoint.as_str(),
        "reticulum-agent:local-controller"
    );
    for invalid_agent in [
        "",
        " reticulum-agent://localhost",
        "reticulum-agent:",
        "reticulum agent",
        "agent",
        "https://localhost:19999",
        "ws://localhost:19999",
        "reticulum://localhost:19999",
        "RETICULUM-AGENT://localhost:19999",
    ] {
        assert_eq!(
            RadrootsReticulumAgentEndpoint::parse(invalid_agent)
                .expect_err("invalid agent endpoint"),
            RadrootsReticulumError::InvalidAgentEndpoint
        );
    }
    assert_eq!(
        RadrootsReticulumProfile::new(
            "transport reticulum",
            endpoint,
            TargetScope::parse(RADROOTS_RETICULUM_SCOPE_ID).expect("Reticulum scope"),
            None,
            RadrootsReticulumBehavior::RejectDeliveryAttempts,
        )
        .expect_err("profile id whitespace"),
        RadrootsReticulumError::InvalidProfileId
    );
    assert_eq!(
        RadrootsReticulumProfile::new(
            "",
            RadrootsReticulumEndpoint::default(),
            TargetScope::parse(RADROOTS_RETICULUM_SCOPE_ID).expect("Reticulum scope"),
            None,
            RadrootsReticulumBehavior::RejectDeliveryAttempts,
        )
        .expect_err("empty profile id"),
        RadrootsReticulumError::InvalidProfileId
    );
    let profile = RadrootsReticulumProfile::new(
        "transport.reticulum.custom",
        RadrootsReticulumEndpoint::default(),
        TargetScope::parse(RADROOTS_RETICULUM_SCOPE_ID).expect("Reticulum scope"),
        Some(agent_endpoint),
        RadrootsReticulumBehavior::DeferDeliveryPlans,
    )
    .expect("custom behavior profile");
    assert_eq!(profile.profile_id(), "transport.reticulum.custom");
    assert_eq!(profile.endpoint().as_str(), RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(
        profile.behavior(),
        RadrootsReticulumBehavior::DeferDeliveryPlans
    );
    assert_eq!(
        profile.agent_endpoint().map(|endpoint| endpoint.as_str()),
        Some("reticulum-agent://localhost:19999")
    );
    assert_eq!(
        profile
            .capability_report()
            .destination
            .routing()
            .scope
            .as_str(),
        RADROOTS_RETICULUM_SCOPE_ID
    );
}

#[test]
fn direct_reticulum_delivery_accepts_any_typed_scope_as_inert_metadata() {
    let transport = RadrootsReticulumTransport::default();
    let request = delivery_request(vec![scoped_reticulum_target("farm-north.mesh_1")]);
    let receipt = transport.deliver(request).expect("delivery receipt");

    assert_eq!(receipt.target_receipts().len(), 1);
    assert_eq!(
        receipt.target_receipts()[0]
            .target
            .scope()
            .map(|scope| scope.as_str()),
        Some("farm-north.mesh_1")
    );
    assert_eq!(
        receipt.target_receipts()[0].status,
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
    );
    assert_eq!(
        receipt.satisfied_target_count(RadrootsTransportSatisfactionClass::Accepted),
        0
    );

    let deferred_transport = RadrootsReticulumTransport::new(
        RadrootsReticulumProfile::default()
            .with_behavior(RadrootsReticulumBehavior::DeferDeliveryPlans),
    );
    let deferred = deferred_transport
        .deliver(delivery_request(vec![scoped_reticulum_target(
            "farm-south.mesh_2",
        )]))
        .expect("deferred delivery receipt");
    assert_eq!(
        deferred.target_receipts()[0]
            .target
            .scope()
            .map(|scope| scope.as_str()),
        Some("farm-south.mesh_2")
    );
    assert_eq!(
        deferred.target_receipts()[0].status,
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
    );
    assert_eq!(
        deferred.satisfied_target_count(RadrootsTransportSatisfactionClass::Accepted),
        0
    );
}

#[test]
fn final_transport_spis_report_unavailable_reticulum_source_and_sink() {
    let transport = RadrootsReticulumTransport::default();
    let target_set = TargetSet::new(vec![reticulum_target(RADROOTS_RETICULUM_ENDPOINT_URI)])
        .expect("target set");
    let sink_status =
        futures::executor::block_on(EventSink::status(&transport)).expect("sink status");
    assert_eq!(sink_status.transport_id(), TransportId::RETICULUM);
    assert_eq!(sink_status.maturity(), Maturity::Preview);
    assert_eq!(sink_status.availability(), Availability::Unavailable);
    assert!(!sink_status.capabilities().can_deliver());

    let source_status =
        futures::executor::block_on(EventSource::status(&transport)).expect("source status");
    assert_eq!(source_status.transport_id(), TransportId::RETICULUM);
    assert_eq!(source_status.maturity(), Maturity::Preview);
    assert_eq!(source_status.availability(), Availability::Unavailable);
    assert!(!source_status.capabilities().can_fetch());

    let fetch = futures::executor::block_on(EventSource::fetch(
        &transport,
        FetchRequest::new(
            "core-fetch",
            target_set,
            FetchBounds::new(10, 10_000).expect("fetch bounds"),
        )
        .expect("fetch request"),
    ))
    .expect("fetch page");
    assert!(fetch.events().is_empty());
    assert_eq!(fetch.target_outcomes().len(), 1);
}

#[test]
fn reject_delivery_attempts_returns_unavailable_without_success_or_nostr_routing() {
    let transport = RadrootsReticulumTransport::default();
    let request = delivery_request(vec![reticulum_target(RADROOTS_RETICULUM_ENDPOINT_URI)]);
    let receipt = transport.deliver(request).expect("delivery receipt");

    assert_eq!(receipt.target_receipts().len(), 1);
    assert_eq!(
        receipt.satisfied_target_count(RadrootsTransportSatisfactionClass::Accepted),
        0
    );
    for target_receipt in receipt.target_receipts() {
        assert_eq!(target_receipt.target.kind(), &TransportId::RETICULUM);
        assert_eq!(
            target_receipt.status,
            RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
        );
        assert_eq!(
            target_receipt.outcome.code.as_deref(),
            Some("transport_unavailable")
        );
        assert_eq!(
            target_receipt.outcome.message.as_deref(),
            Some(RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE)
        );
    }
}

#[test]
fn noncanonical_reticulum_targets_are_rejected() {
    for invalid in [
        " reticulum:unavailable",
        "reticulum:unavailable ",
        "RETICULUM:unavailable",
        "reticulum:Unavailable",
        "reticulum:temporary",
        "reticulum:unavailable-alt",
        "reticulum:custom",
    ] {
        let result = Target::new(TransportId::RETICULUM, invalid)
            .and_then(|target| ReticulumDestinationV1::from_target(&target).map(|_| target));
        assert_eq!(
            result.expect_err("noncanonical Reticulum target"),
            radroots_transport::RadrootsTransportError::InvalidTargetUri
        );
    }
}

#[test]
fn deferred_delivery_plan_mode_never_counts_as_satisfied() {
    let transport = RadrootsReticulumTransport::new(
        RadrootsReticulumProfile::default()
            .with_behavior(RadrootsReticulumBehavior::DeferDeliveryPlans),
    );
    let request = delivery_request(vec![reticulum_target(RADROOTS_RETICULUM_ENDPOINT_URI)]);
    let receipt = transport.deliver(request).expect("delivery receipt");

    assert_eq!(receipt.target_receipts().len(), 1);
    assert_eq!(
        receipt.satisfied_target_count(RadrootsTransportSatisfactionClass::Accepted),
        0
    );
    assert_eq!(
        receipt.target_receipts()[0].status,
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
    );
    assert_eq!(
        receipt.target_receipts()[0].outcome.code.as_deref(),
        Some("deferred_until_implemented")
    );
    assert!(
        !RadrootsTransportSatisfactionPolicy::any_accepted()
            .is_satisfied_by(
                1,
                receipt.satisfied_target_count(RadrootsTransportSatisfactionClass::Accepted)
            )
            .expect("satisfaction check")
    );
}

#[test]
fn non_reticulum_targets_are_rejected_without_nostr_routing() {
    let transport = RadrootsReticulumTransport::default();
    let err = transport
        .deliver(delivery_request(vec![nostr_target()]))
        .expect_err("non-reticulum target");

    assert_eq!(err, RadrootsReticulumError::NonReticulumTarget);
}

#[test]
fn reticulum_target_constructor_always_supplies_typed_scope() {
    let target = reticulum_target(RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(
        target.scope().map(|scope| scope.as_str()),
        Some(RADROOTS_RETICULUM_SCOPE_ID)
    );
}

#[test]
fn fetch_reports_deferred_until_implemented_without_observed_events() {
    let transport = RadrootsReticulumTransport::default();
    assert_eq!(
        transport.profile().profile_id(),
        "transport.reticulum.default"
    );
    assert_eq!(
        transport.status().transport_status.implementation,
        RadrootsTransportImplementationState::Real
    );
    let receipt = transport
        .fetch(RadrootsReticulumFetchRequest::new("fetch-1", 10).expect("fetch request"))
        .expect("fetch receipt");

    assert_eq!(receipt.request_id, "fetch-1");
    assert_eq!(receipt.endpoint_uri, RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(receipt.observed_event_count, 0);
    assert_eq!(
        receipt.implementation,
        RadrootsTransportImplementationState::Real
    );
    assert_eq!(receipt.scope.as_str(), RADROOTS_RETICULUM_SCOPE_ID);
    assert_eq!(receipt.agent_endpoint, None);
    assert_eq!(
        receipt.outcome.status,
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
    );
    assert_eq!(
        RadrootsReticulumFetchRequest::new("fetch-0", 0).expect_err("zero limit"),
        RadrootsReticulumError::InvalidFetchLimit
    );
    assert_eq!(
        transport
            .fetch(RadrootsReticulumFetchRequest {
                request_id: "fetch-public-zero".to_owned(),
                max_events: 0,
            })
            .expect_err("zero limit at transport boundary"),
        RadrootsReticulumError::InvalidFetchLimit
    );
    let deferred_transport = RadrootsReticulumTransport::new(
        RadrootsReticulumProfile::default()
            .with_behavior(RadrootsReticulumBehavior::DeferDeliveryPlans),
    );
    let deferred = deferred_transport
        .fetch(RadrootsReticulumFetchRequest::new("fetch-deferred", 1).expect("fetch"))
        .expect("fetch receipt");
    assert_eq!(
        deferred.outcome.status,
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
    );
}

#[test]
fn configured_agent_endpoint_is_metadata_only_for_status_delivery_and_fetch() {
    let agent_endpoint = RadrootsReticulumAgentEndpoint::parse("reticulum-agent://localhost:19999")
        .expect("agent endpoint");
    let transport = RadrootsReticulumTransport::new(
        RadrootsReticulumProfile::default().with_agent_endpoint(agent_endpoint),
    );
    let status = transport.status();
    assert_eq!(
        status
            .agent_endpoint
            .as_ref()
            .map(|endpoint| endpoint.as_str()),
        Some("reticulum-agent://localhost:19999")
    );
    assert_eq!(
        status.transport_status.implementation,
        RadrootsTransportImplementationState::Real
    );
    assert!(!status.transport_status.usable_for_delivery);
    assert!(!status.transport_status.capabilities.deliver);
    assert!(!status.transport_status.capabilities.fetch);
    assert!(!status.transport_status.capabilities.discovery);
    assert!(!status.transport_status.capabilities.gateway_forwarding);
    assert!(!status.transport_status.capabilities.receipt_observation);

    let receipt = transport
        .deliver(delivery_request(vec![reticulum_target(
            RADROOTS_RETICULUM_ENDPOINT_URI,
        )]))
        .expect("delivery receipt");
    assert_eq!(
        receipt.target_receipts()[0].status,
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
    );
    let fetch = transport
        .fetch(RadrootsReticulumFetchRequest::new("fetch-agent", 1).expect("fetch"))
        .expect("fetch receipt");
    assert_eq!(
        fetch
            .agent_endpoint
            .as_ref()
            .map(|endpoint| endpoint.as_str()),
        Some("reticulum-agent://localhost:19999")
    );
    assert_eq!(fetch.observed_event_count, 0);
    assert_eq!(
        fetch.implementation,
        RadrootsTransportImplementationState::Real
    );
}

#[test]
#[cfg(feature = "serde")]
fn public_models_round_trip_through_serde() {
    let profile = RadrootsReticulumProfile::default()
        .with_behavior(RadrootsReticulumBehavior::DeferDeliveryPlans);
    let json = serde_json::to_string(&profile).expect("profile json");
    let decoded: RadrootsReticulumProfile = serde_json::from_str(&json).expect("profile decode");

    assert_eq!(decoded, profile);
}

#[test]
#[cfg(feature = "serde")]
fn destination_deserialization_revalidates_canonical_identity() {
    let destination = ReticulumDestinationV1::local();
    let canonical = serde_json::to_value(&destination).expect("serialize destination");
    assert_eq!(
        serde_json::from_value::<ReticulumDestinationV1>(canonical.clone())
            .expect("deserialize canonical destination"),
        destination
    );

    let mut forged_fingerprint = canonical.clone();
    forged_fingerprint
        .as_object_mut()
        .expect("destination object")
        .insert("fingerprint".to_owned(), Value::String("0".repeat(64)));
    assert!(serde_json::from_value::<ReticulumDestinationV1>(forged_fingerprint).is_err());

    let mut forged_scope = canonical.clone();
    forged_scope
        .get_mut("routing")
        .and_then(Value::as_object_mut)
        .expect("routing object")
        .insert("scope".to_owned(), Value::String("remote".to_owned()));
    assert!(serde_json::from_value::<ReticulumDestinationV1>(forged_scope).is_err());

    let mut nested_unknown = canonical.clone();
    nested_unknown
        .get_mut("routing")
        .and_then(Value::as_object_mut)
        .expect("routing object")
        .insert("unexpected".to_owned(), Value::Bool(true));
    assert!(serde_json::from_value::<ReticulumDestinationV1>(nested_unknown).is_err());

    let mut top_level_unknown = canonical;
    top_level_unknown
        .as_object_mut()
        .expect("destination object")
        .insert("unexpected".to_owned(), Value::Bool(true));
    assert!(serde_json::from_value::<ReticulumDestinationV1>(top_level_unknown).is_err());
}

#[test]
fn destination_rejects_non_reticulum_targets() {
    let local = Target::new(TransportId::LOCAL, "local:memory").expect("local target");
    assert!(ReticulumDestinationV1::from_target(&local).is_err());
}

#[test]
fn reticulum_source_remains_inert_without_runtime_delivery_hooks() {
    let source = include_str!("../src/lib.rs").to_ascii_lowercase();
    for forbidden in [
        "socket",
        "rnsd",
        "python",
        "identity",
        "send_mesh",
        "fallback",
        "nostr",
    ] {
        assert!(
            !source.contains(forbidden),
            "Reticulum source contains forbidden runtime hook {forbidden}"
        );
    }
}

#[test]
fn reticulum_errors_and_defaults_are_stable() {
    assert_eq!(
        RadrootsReticulumBehavior::default(),
        RadrootsReticulumBehavior::RejectDeliveryAttempts
    );
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
            RadrootsReticulumError::InvalidFetchLimit,
            "Reticulum fetch limit must be greater than zero",
        ),
        (
            RadrootsReticulumError::NonReticulumTarget,
            "Reticulum transport received a non-Reticulum target",
        ),
    ];

    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
    }
}
