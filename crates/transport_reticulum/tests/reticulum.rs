use radroots_transport::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RADROOTS_RETICULUM_SCOPE_ID,
    RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE, RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT,
    RadrootsTransport, RadrootsTransportCapabilityAvailability,
    RadrootsTransportCapabilityMaturity, RadrootsTransportDeliveryRequest,
    RadrootsTransportDeliveryTargetStatus, RadrootsTransportFetchRequest,
    RadrootsTransportImplementationState, RadrootsTransportKind, RadrootsTransportMeshScopeId,
    RadrootsTransportPayload, RadrootsTransportSatisfactionClass,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportTarget, RadrootsTransportTargetSet,
    ReticulumDuplicateFragmentBehaviorV1, ReticulumFragmentIntegrityV1,
    ReticulumFragmentationModeV1, ReticulumGatewaySemanticsV1, ReticulumPrivacySemanticsV1,
};
#[cfg(feature = "serde")]
use radroots_transport::{
    RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES, RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES,
};
use radroots_transport_reticulum::{
    RadrootsReticulumAgentEndpoint, RadrootsReticulumBehavior, RadrootsReticulumEndpoint,
    RadrootsReticulumError, RadrootsReticulumFetchRequest, RadrootsReticulumProfile,
    RadrootsReticulumTransport,
};

fn reticulum_target(uri: &str) -> RadrootsTransportTarget {
    assert_eq!(uri, RADROOTS_RETICULUM_ENDPOINT_URI);
    RadrootsTransportTarget::reticulum().expect("reticulum target")
}

fn scoped_reticulum_target(scope: &str) -> RadrootsTransportTarget {
    RadrootsTransportTarget::reticulum_with_metadata(
        RADROOTS_RETICULUM_ENDPOINT_URI,
        Some(RadrootsTransportMeshScopeId::parse(scope).expect("scope")),
        None,
    )
    .expect("scoped reticulum target")
}

fn nostr_target() -> RadrootsTransportTarget {
    RadrootsTransportTarget::nostr_relay("wss://relay.example").expect("nostr target")
}

fn delivery_request(targets: Vec<RadrootsTransportTarget>) -> RadrootsTransportDeliveryRequest {
    RadrootsTransportDeliveryRequest::new(
        "reticulum-delivery",
        reticulum_payload(),
        RadrootsTransportTargetSet::new(targets).expect("target set"),
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
        status.transport_status().implementation(),
        RadrootsTransportImplementationState::Real
    );
    assert_eq!(
        status.transport_status().maturity(),
        RadrootsTransportCapabilityMaturity::Preview
    );
    assert_eq!(
        status.transport_status().availability(),
        RadrootsTransportCapabilityAvailability::Unavailable
    );
    assert!(status.transport_status().is_configured());
    assert_eq!(
        status.transport_status().profile_id(),
        Some("transport.reticulum.default")
    );
    assert_eq!(
        status.transport_status().endpoint_uri(),
        Some(RADROOTS_RETICULUM_ENDPOINT_URI)
    );
    assert_eq!(
        profile.destination().uri().as_str(),
        RADROOTS_RETICULUM_ENDPOINT_URI
    );
    assert_eq!(
        profile.destination().routing().scope().as_str(),
        RADROOTS_RETICULUM_SCOPE_ID
    );
    assert_eq!(
        status.destination().routing().gateway(),
        ReticulumGatewaySemanticsV1::NoGatewayForwarding
    );
    assert_eq!(
        status.destination().routing().privacy(),
        ReticulumPrivacySemanticsV1::CanonicalSignedEventBytesOnly
    );
    assert!(status.capability_report().is_delivery_required());
    assert!(!status.capability_report().is_fetch_required());
    assert!(!status.capability_report().can_deliver());
    assert!(!status.capability_report().can_fetch());
    assert!(!status.capability_report().can_discover());
    assert!(!status.capability_report().can_forward_gateway());
    assert!(!status.capability_report().can_observe_receipts());
    assert_eq!(
        status.capability_report().destination().fingerprint(),
        status.destination().fingerprint()
    );
    assert_eq!(
        status
            .capability_report()
            .payload_policy()
            .fragment_policy()
            .mode(),
        ReticulumFragmentationModeV1::Unsupported
    );
    assert_eq!(
        status
            .capability_report()
            .payload_policy()
            .fragment_policy()
            .max_fragment_count(),
        1
    );
    assert_eq!(
        status
            .capability_report()
            .payload_policy()
            .fragment_policy()
            .duplicate_fragment_behavior(),
        ReticulumDuplicateFragmentBehaviorV1::Reject
    );
    assert_eq!(
        status
            .capability_report()
            .payload_policy()
            .fragment_policy()
            .integrity_verification(),
        ReticulumFragmentIntegrityV1::PayloadDigest
    );
    assert_eq!(
        status.transport_status().message(),
        RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE
    );
    assert!(!status.transport_status().is_usable_for_delivery());
    assert_eq!(status.scope().as_str(), RADROOTS_RETICULUM_SCOPE_ID);
    assert_eq!(status.agent_endpoint(), None);
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
            RadrootsTransportMeshScopeId::local_reticulum(),
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
            RadrootsTransportMeshScopeId::local_reticulum(),
            None,
            RadrootsReticulumBehavior::RejectDeliveryAttempts,
        )
        .expect_err("empty profile id"),
        RadrootsReticulumError::InvalidProfileId
    );
    let profile = RadrootsReticulumProfile::new(
        "transport.reticulum.custom",
        RadrootsReticulumEndpoint::default(),
        RadrootsTransportMeshScopeId::local_reticulum(),
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
            .destination()
            .routing()
            .scope()
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
            .target()
            .scope()
            .map(|scope| scope.as_str()),
        Some("farm-north.mesh_1")
    );
    assert_eq!(
        receipt.target_receipts()[0].status(),
        RadrootsTransportDeliveryTargetStatus::FailedRetryable
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
            .target()
            .scope()
            .map(|scope| scope.as_str()),
        Some("farm-south.mesh_2")
    );
    assert_eq!(
        deferred.target_receipts()[0].status(),
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
    );
    assert_eq!(
        deferred.satisfied_target_count(RadrootsTransportSatisfactionClass::Accepted),
        0
    );
}

#[test]
fn core_transport_trait_reports_reticulum_status_delivery_and_fetch() {
    let transport = RadrootsReticulumTransport::default();
    let target_set =
        RadrootsTransportTargetSet::new(vec![reticulum_target(RADROOTS_RETICULUM_ENDPOINT_URI)])
            .expect("target set");
    let status = futures::executor::block_on(RadrootsTransport::status(&transport))
        .expect("transport status");
    assert_eq!(status.kind(), &RadrootsTransportKind::Reticulum);
    assert_eq!(
        status.implementation(),
        RadrootsTransportImplementationState::Real
    );
    assert_eq!(
        status.maturity(),
        RadrootsTransportCapabilityMaturity::Preview
    );
    assert_eq!(
        status.availability(),
        RadrootsTransportCapabilityAvailability::Unavailable
    );
    assert!(!status.is_usable_for_delivery());
    assert!(!status.capabilities().can_deliver());
    assert!(!status.capabilities().can_fetch());

    let delivery = futures::executor::block_on(RadrootsTransport::deliver(
        &transport,
        RadrootsTransportDeliveryRequest::new(
            "core-delivery",
            reticulum_payload(),
            target_set.clone(),
            RadrootsTransportSatisfactionPolicy::any_accepted(),
        )
        .expect("delivery request"),
    ))
    .expect("delivery receipt");
    assert_eq!(
        delivery.target_receipts()[0].status(),
        RadrootsTransportDeliveryTargetStatus::FailedRetryable
    );

    let fetch = futures::executor::block_on(RadrootsTransport::fetch(
        &transport,
        RadrootsTransportFetchRequest::new("core-fetch", target_set).expect("fetch request"),
    ))
    .expect("fetch receipt");
    assert_eq!(fetch.fetched_count(), 0);
    assert_eq!(
        fetch.target_receipts()[0].status(),
        RadrootsTransportDeliveryTargetStatus::FailedRetryable
    );
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
        assert_eq!(
            target_receipt.target().kind(),
            &RadrootsTransportKind::Reticulum
        );
        assert_eq!(
            target_receipt.status(),
            RadrootsTransportDeliveryTargetStatus::FailedRetryable
        );
        assert_eq!(target_receipt.outcome().code(), "transport_unavailable");
        assert_eq!(
            target_receipt.outcome().message(),
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
        assert_eq!(
            RadrootsTransportTarget::new(RadrootsTransportKind::Reticulum, invalid)
                .expect_err("noncanonical Reticulum target"),
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
        receipt.target_receipts()[0].status(),
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
    );
    assert_eq!(
        receipt.target_receipts()[0].outcome().code(),
        "deferred_until_implemented"
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
fn fetch_reports_authoritative_unavailable_or_deferred_outcomes() {
    let transport = RadrootsReticulumTransport::default();
    assert_eq!(
        transport.profile().profile_id(),
        "transport.reticulum.default"
    );
    assert_eq!(
        transport.status().transport_status().implementation(),
        RadrootsTransportImplementationState::Real
    );
    let receipt = transport
        .fetch(RadrootsReticulumFetchRequest::new("fetch-1", 10).expect("fetch request"))
        .expect("fetch receipt");

    assert_eq!(receipt.request_id(), "fetch-1");
    assert_eq!(receipt.endpoint_uri(), RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(receipt.observed_event_count(), 0);
    assert_eq!(
        receipt.implementation(),
        RadrootsTransportImplementationState::Real
    );
    assert_eq!(receipt.scope().as_str(), RADROOTS_RETICULUM_SCOPE_ID);
    assert_eq!(receipt.agent_endpoint(), None);
    assert_eq!(
        receipt.outcome().status(),
        RadrootsTransportDeliveryTargetStatus::FailedRetryable
    );
    assert_eq!(
        RadrootsReticulumFetchRequest::new("fetch-0", 0).expect_err("zero limit"),
        RadrootsReticulumError::InvalidFetchLimit
    );
    assert_eq!(
        RadrootsReticulumFetchRequest::new(
            "fetch-over",
            u16::try_from(RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT + 1)
                .expect("one-over limit fits u16"),
        )
        .expect_err("one-over limit"),
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
        deferred.outcome().status(),
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
        status.agent_endpoint().map(|endpoint| endpoint.as_str()),
        Some("reticulum-agent://localhost:19999")
    );
    assert_eq!(
        status.transport_status().implementation(),
        RadrootsTransportImplementationState::Real
    );
    assert!(!status.transport_status().is_usable_for_delivery());
    assert!(!status.transport_status().capabilities().can_deliver());
    assert!(!status.transport_status().capabilities().can_fetch());
    assert!(!status.transport_status().capabilities().can_discover());
    assert!(
        !status
            .transport_status()
            .capabilities()
            .can_forward_gateway()
    );
    assert!(
        !status
            .transport_status()
            .capabilities()
            .can_observe_receipts()
    );

    let receipt = transport
        .deliver(delivery_request(vec![reticulum_target(
            RADROOTS_RETICULUM_ENDPOINT_URI,
        )]))
        .expect("delivery receipt");
    assert_eq!(
        receipt.target_receipts()[0].status(),
        RadrootsTransportDeliveryTargetStatus::FailedRetryable
    );
    let fetch = transport
        .fetch(RadrootsReticulumFetchRequest::new("fetch-agent", 1).expect("fetch"))
        .expect("fetch receipt");
    assert_eq!(
        fetch.agent_endpoint().map(|endpoint| endpoint.as_str()),
        Some("reticulum-agent://localhost:19999")
    );
    assert_eq!(fetch.observed_event_count(), 0);
    assert_eq!(
        fetch.implementation(),
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
fn transport_bounds_reticulum_public_wire_is_strict_and_revalidated() {
    let exact_request = RadrootsReticulumFetchRequest::new(
        "r".repeat(RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES),
        u16::try_from(RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT)
            .expect("event maximum fits u16"),
    )
    .expect("exact fetch request");
    assert_eq!(
        exact_request.request_id().len(),
        RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES
    );
    assert_eq!(
        usize::from(exact_request.max_events()),
        RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT
    );
    assert_eq!(
        RadrootsReticulumFetchRequest::new(
            "r".repeat(RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES + 1),
            1,
        )
        .expect_err("one-over request id"),
        RadrootsReticulumError::InvalidFetchRequestId
    );

    let exact_agent = format!(
        "reticulum-agent:{}",
        "a".repeat(RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES - "reticulum-agent:".len())
    );
    assert_eq!(
        RadrootsReticulumAgentEndpoint::parse(exact_agent)
            .expect("exact agent endpoint")
            .as_str()
            .len(),
        RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES
    );
    assert_eq!(
        RadrootsReticulumAgentEndpoint::parse(format!(
            "reticulum-agent:{}",
            "a".repeat(RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES - "reticulum-agent:".len() + 1)
        ))
        .expect_err("one-over agent endpoint"),
        RadrootsReticulumError::InvalidAgentEndpoint
    );

    let transport = RadrootsReticulumTransport::default();
    let status = transport.status();
    let receipt = transport
        .fetch(RadrootsReticulumFetchRequest::new("fetch-wire", 1).expect("fetch request"))
        .expect("fetch receipt");
    let request_wire = serde_json::to_value(&exact_request).expect("request wire");
    let status_wire = serde_json::to_value(&status).expect("status wire");
    let receipt_wire = serde_json::to_value(&receipt).expect("receipt wire");

    let mut request_over = request_wire.clone();
    request_over["max_events"] =
        serde_json::Value::from(RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT + 1);
    assert!(serde_json::from_value::<RadrootsReticulumFetchRequest>(request_over).is_err());
    let mut status_forged = status_wire.clone();
    status_forged["capability_report"]["can_deliver"] = serde_json::Value::Bool(true);
    assert!(
        serde_json::from_value::<radroots_transport_reticulum::RadrootsReticulumStatus>(
            status_forged
        )
        .is_err()
    );
    let mut receipt_forged = receipt_wire.clone();
    receipt_forged["observed_event_count"] = serde_json::Value::from(1);
    assert!(
        serde_json::from_value::<radroots_transport_reticulum::RadrootsReticulumFetchReceipt>(
            receipt_forged
        )
        .is_err()
    );

    let mut request_unknown = request_wire;
    request_unknown["unexpected"] = serde_json::Value::Bool(true);
    assert!(serde_json::from_value::<RadrootsReticulumFetchRequest>(request_unknown).is_err());
    let mut status_unknown = status_wire;
    status_unknown["unexpected"] = serde_json::Value::Bool(true);
    assert!(
        serde_json::from_value::<radroots_transport_reticulum::RadrootsReticulumStatus>(
            status_unknown
        )
        .is_err()
    );
    let mut receipt_unknown = receipt_wire;
    receipt_unknown["unexpected"] = serde_json::Value::Bool(true);
    assert!(
        serde_json::from_value::<radroots_transport_reticulum::RadrootsReticulumFetchReceipt>(
            receipt_unknown
        )
        .is_err()
    );
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
            "Reticulum fetch limit must be between 1 and 1000",
        ),
        (
            RadrootsReticulumError::InvalidFetchRequestId,
            "invalid Reticulum fetch request id",
        ),
        (
            RadrootsReticulumError::InvalidFetchReceipt,
            "invalid Reticulum fetch receipt",
        ),
        (
            RadrootsReticulumError::InvalidStatus,
            "invalid Reticulum status",
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
