use radroots_transport::{
    RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI, RADROOTS_RETICULUM_PREVIEW_SCOPE_ID,
    RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE, RadrootsTransport, RadrootsTransportDeliveryRequest,
    RadrootsTransportDeliveryTargetStatus, RadrootsTransportFetchRequest,
    RadrootsTransportImplementationState, RadrootsTransportKind, RadrootsTransportMeshScopeId,
    RadrootsTransportPayload, RadrootsTransportSatisfactionClass,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportTarget, RadrootsTransportTargetSet,
};
use radroots_transport_reticulum::{
    RadrootsReticulumPreviewAgentEndpoint, RadrootsReticulumPreviewBehavior,
    RadrootsReticulumPreviewEndpoint, RadrootsReticulumPreviewError,
    RadrootsReticulumPreviewFetchRequest, RadrootsReticulumPreviewProfile,
    RadrootsReticulumPreviewTransport,
};

fn reticulum_target(uri: &str) -> RadrootsTransportTarget {
    assert_eq!(uri, RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI);
    RadrootsTransportTarget::reticulum_preview().expect("reticulum target")
}

fn scoped_reticulum_target(scope: &str) -> RadrootsTransportTarget {
    RadrootsTransportTarget::reticulum_preview_with_metadata(
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
        "reticulum-preview-delivery",
        preview_payload(),
        RadrootsTransportTargetSet::new(targets).expect("target set"),
        RadrootsTransportSatisfactionPolicy::any_accepted(),
    )
}

fn preview_payload() -> RadrootsTransportPayload {
    RadrootsTransportPayload::mesh_frame_cbor("preview-message", [1_u8, 2, 3]).expect("payload")
}

#[test]
fn default_profile_is_configured_preview_unavailable_and_rejecting() {
    let profile = RadrootsReticulumPreviewProfile::default();
    let status = profile.status();

    assert_eq!(profile.profile_id(), "transport.reticulum.preview");
    assert_eq!(
        profile.endpoint().as_str(),
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI
    );
    assert_eq!(
        profile.scope().as_str(),
        RADROOTS_RETICULUM_PREVIEW_SCOPE_ID
    );
    assert_eq!(profile.agent_endpoint(), None);
    assert_eq!(
        profile.behavior(),
        RadrootsReticulumPreviewBehavior::RejectDeliveryAttempts
    );
    assert_eq!(
        status.transport_status.implementation,
        RadrootsTransportImplementationState::PreviewUnavailable
    );
    assert!(status.transport_status.configured);
    assert_eq!(
        status.transport_status.profile_id.as_deref(),
        Some("transport.reticulum.preview")
    );
    assert_eq!(
        status.transport_status.endpoint_uri.as_deref(),
        Some(RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI)
    );
    assert_eq!(
        status.transport_status.message,
        RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE
    );
    assert!(!status.transport_status.usable_for_delivery);
    assert_eq!(status.scope.as_str(), RADROOTS_RETICULUM_PREVIEW_SCOPE_ID);
    assert_eq!(status.agent_endpoint, None);
}

#[test]
fn endpoint_and_profile_validation_are_strict_and_canonical() {
    let endpoint = RadrootsReticulumPreviewEndpoint::parse(RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI)
        .expect("endpoint");
    assert_eq!(endpoint.as_str(), RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI);
    assert_eq!(
        endpoint.to_string(),
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI
    );
    assert_eq!(
        endpoint.clone().into_string(),
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI
    );
    assert_eq!(
        RadrootsReticulumPreviewEndpoint::default().as_str(),
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI
    );

    assert_eq!(
        RadrootsReticulumPreviewEndpoint::parse(" ").expect_err("empty endpoint"),
        RadrootsReticulumPreviewError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumPreviewEndpoint::parse("reticulum:").expect_err("empty endpoint"),
        RadrootsReticulumPreviewError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumPreviewEndpoint::parse("https://target").expect_err("wrong scheme"),
        RadrootsReticulumPreviewError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumPreviewEndpoint::parse("RETICULUM:Preview-Unavailable")
            .expect_err("case drift endpoint"),
        RadrootsReticulumPreviewError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumPreviewEndpoint::parse(" reticulum:preview-unavailable")
            .expect_err("leading whitespace endpoint"),
        RadrootsReticulumPreviewError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumPreviewEndpoint::parse("reticulum:preview-unavailable ")
            .expect_err("trailing whitespace endpoint"),
        RadrootsReticulumPreviewError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumPreviewEndpoint::parse("reticulum:custom").expect_err("custom endpoint"),
        RadrootsReticulumPreviewError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumPreviewEndpoint::parse("reticulum:bad target")
            .expect_err("whitespace endpoint"),
        RadrootsReticulumPreviewError::InvalidEndpoint
    );
    assert_eq!(
        RadrootsReticulumPreviewEndpoint::parse("reticulum:bad\ntarget")
            .expect_err("control endpoint"),
        RadrootsReticulumPreviewError::InvalidEndpoint
    );
    let agent_endpoint =
        RadrootsReticulumPreviewAgentEndpoint::parse("reticulum-agent://localhost:19999")
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
        RadrootsReticulumPreviewAgentEndpoint::parse("reticulum-agent:local-controller")
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
            RadrootsReticulumPreviewAgentEndpoint::parse(invalid_agent)
                .expect_err("invalid agent endpoint"),
            RadrootsReticulumPreviewError::InvalidAgentEndpoint
        );
    }
    assert_eq!(
        RadrootsReticulumPreviewProfile::new(
            "transport reticulum",
            endpoint,
            RadrootsTransportMeshScopeId::local_preview(),
            None,
            RadrootsReticulumPreviewBehavior::RejectDeliveryAttempts,
        )
        .expect_err("profile id whitespace"),
        RadrootsReticulumPreviewError::InvalidProfileId
    );
    assert_eq!(
        RadrootsReticulumPreviewProfile::new(
            "",
            RadrootsReticulumPreviewEndpoint::default(),
            RadrootsTransportMeshScopeId::local_preview(),
            None,
            RadrootsReticulumPreviewBehavior::RejectDeliveryAttempts,
        )
        .expect_err("empty profile id"),
        RadrootsReticulumPreviewError::InvalidProfileId
    );
    let profile = RadrootsReticulumPreviewProfile::new(
        "transport.reticulum.custom",
        RadrootsReticulumPreviewEndpoint::default(),
        RadrootsTransportMeshScopeId::local_preview(),
        Some(agent_endpoint),
        RadrootsReticulumPreviewBehavior::DeferDeliveryPlans,
    )
    .expect("custom behavior profile");
    assert_eq!(profile.profile_id(), "transport.reticulum.custom");
    assert_eq!(
        profile.endpoint().as_str(),
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI
    );
    assert_eq!(
        profile.behavior(),
        RadrootsReticulumPreviewBehavior::DeferDeliveryPlans
    );
    assert_eq!(
        profile.agent_endpoint().map(|endpoint| endpoint.as_str()),
        Some("reticulum-agent://localhost:19999")
    );
}

#[test]
fn direct_preview_delivery_accepts_any_typed_reticulum_scope_as_inert_metadata() {
    let transport = RadrootsReticulumPreviewTransport::default();
    let request = delivery_request(vec![scoped_reticulum_target("farm-north.preview_1")]);
    let receipt = transport.deliver(request).expect("delivery receipt");

    assert_eq!(receipt.target_receipts.len(), 1);
    assert_eq!(
        receipt.target_receipts[0]
            .target
            .scope
            .as_ref()
            .map(|scope| scope.as_str()),
        Some("farm-north.preview_1")
    );
    assert_eq!(
        receipt.target_receipts[0].status,
        RadrootsTransportDeliveryTargetStatus::PreviewUnavailable
    );
    assert_eq!(
        receipt.satisfied_target_count(RadrootsTransportSatisfactionClass::Accepted),
        0
    );

    let deferred_transport = RadrootsReticulumPreviewTransport::new(
        RadrootsReticulumPreviewProfile::default()
            .with_behavior(RadrootsReticulumPreviewBehavior::DeferDeliveryPlans),
    );
    let deferred = deferred_transport
        .deliver(delivery_request(vec![scoped_reticulum_target(
            "farm-south.preview_2",
        )]))
        .expect("deferred delivery receipt");
    assert_eq!(
        deferred.target_receipts[0]
            .target
            .scope
            .as_ref()
            .map(|scope| scope.as_str()),
        Some("farm-south.preview_2")
    );
    assert_eq!(
        deferred.target_receipts[0].status,
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
    );
    assert_eq!(
        deferred.satisfied_target_count(RadrootsTransportSatisfactionClass::Accepted),
        0
    );
}

#[test]
fn core_transport_trait_reports_preview_status_delivery_and_fetch() {
    let transport = RadrootsReticulumPreviewTransport::default();
    let target_set = RadrootsTransportTargetSet::new(vec![reticulum_target(
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI,
    )])
    .expect("target set");
    let status = futures::executor::block_on(RadrootsTransport::status(&transport))
        .expect("transport status");
    assert_eq!(status.kind, RadrootsTransportKind::Reticulum);
    assert_eq!(
        status.implementation,
        RadrootsTransportImplementationState::PreviewUnavailable
    );
    assert!(!status.usable_for_delivery);
    assert!(!status.capabilities.deliver);
    assert!(!status.capabilities.fetch);

    let delivery = futures::executor::block_on(RadrootsTransport::deliver(
        &transport,
        RadrootsTransportDeliveryRequest::new(
            "core-delivery",
            preview_payload(),
            target_set.clone(),
            RadrootsTransportSatisfactionPolicy::any_accepted(),
        ),
    ))
    .expect("delivery receipt");
    assert_eq!(
        delivery.target_receipts[0].status,
        RadrootsTransportDeliveryTargetStatus::PreviewUnavailable
    );

    let fetch = futures::executor::block_on(RadrootsTransport::fetch(
        &transport,
        RadrootsTransportFetchRequest::new("core-fetch", target_set),
    ))
    .expect("fetch receipt");
    assert_eq!(fetch.fetched_count, 0);
    assert_eq!(
        fetch.target_receipts[0].status,
        RadrootsTransportDeliveryTargetStatus::PreviewUnavailable
    );
}

#[test]
fn reject_delivery_attempts_returns_unavailable_without_success_or_nostr_routing() {
    let transport = RadrootsReticulumPreviewTransport::default();
    let request = delivery_request(vec![reticulum_target(
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI,
    )]);
    let receipt = transport.deliver(request).expect("delivery receipt");

    assert_eq!(receipt.target_receipts.len(), 1);
    assert_eq!(
        receipt.satisfied_target_count(RadrootsTransportSatisfactionClass::Accepted),
        0
    );
    for target_receipt in receipt.target_receipts {
        assert_eq!(target_receipt.target.kind, RadrootsTransportKind::Reticulum);
        assert_eq!(
            target_receipt.status,
            RadrootsTransportDeliveryTargetStatus::PreviewUnavailable
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
fn noncanonical_reticulum_preview_targets_are_rejected() {
    for invalid in [
        " reticulum:preview-unavailable",
        "reticulum:preview-unavailable ",
        "RETICULUM:preview-unavailable",
        "reticulum:Preview-Unavailable",
        "reticulum:preview",
        "reticulum:preview-unavailable-alt",
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
    let transport = RadrootsReticulumPreviewTransport::new(
        RadrootsReticulumPreviewProfile::default()
            .with_behavior(RadrootsReticulumPreviewBehavior::DeferDeliveryPlans),
    );
    let request = delivery_request(vec![reticulum_target(
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI,
    )]);
    let receipt = transport.deliver(request).expect("delivery receipt");

    assert_eq!(receipt.target_receipts.len(), 1);
    assert_eq!(
        receipt.satisfied_target_count(RadrootsTransportSatisfactionClass::Accepted),
        0
    );
    assert_eq!(
        receipt.target_receipts[0].status,
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
    );
    assert_eq!(
        receipt.target_receipts[0].outcome.code.as_deref(),
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
    let transport = RadrootsReticulumPreviewTransport::default();
    let err = transport
        .deliver(delivery_request(vec![nostr_target()]))
        .expect_err("non-reticulum target");

    assert_eq!(err, RadrootsReticulumPreviewError::NonReticulumTarget);
}

#[test]
fn malformed_reticulum_target_without_typed_scope_is_rejected() {
    let transport = RadrootsReticulumPreviewTransport::default();
    let mut target = reticulum_target(RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI);
    target.scope = None;
    let err = transport
        .deliver(delivery_request(vec![target]))
        .expect_err("missing typed scope");

    assert_eq!(err, RadrootsReticulumPreviewError::InvalidEndpoint);
}

#[test]
fn fetch_reports_preview_unavailable_without_observed_events() {
    let transport = RadrootsReticulumPreviewTransport::default();
    assert_eq!(
        transport.profile().profile_id(),
        "transport.reticulum.preview"
    );
    assert_eq!(
        transport.status().transport_status.implementation,
        RadrootsTransportImplementationState::PreviewUnavailable
    );
    let receipt = transport
        .fetch(RadrootsReticulumPreviewFetchRequest::new("fetch-1", 10).expect("fetch request"))
        .expect("fetch receipt");

    assert_eq!(receipt.request_id, "fetch-1");
    assert_eq!(
        receipt.endpoint_uri,
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI
    );
    assert_eq!(receipt.observed_event_count, 0);
    assert_eq!(
        receipt.implementation,
        RadrootsTransportImplementationState::PreviewUnavailable
    );
    assert_eq!(receipt.scope.as_str(), RADROOTS_RETICULUM_PREVIEW_SCOPE_ID);
    assert_eq!(receipt.agent_endpoint, None);
    assert_eq!(
        receipt.outcome.status,
        RadrootsTransportDeliveryTargetStatus::PreviewUnavailable
    );
    assert_eq!(
        RadrootsReticulumPreviewFetchRequest::new("fetch-0", 0).expect_err("zero limit"),
        RadrootsReticulumPreviewError::InvalidFetchLimit
    );
    assert_eq!(
        transport
            .fetch(RadrootsReticulumPreviewFetchRequest {
                request_id: "fetch-public-zero".to_owned(),
                max_events: 0,
            })
            .expect_err("zero limit at transport boundary"),
        RadrootsReticulumPreviewError::InvalidFetchLimit
    );
    let deferred_transport = RadrootsReticulumPreviewTransport::new(
        RadrootsReticulumPreviewProfile::default()
            .with_behavior(RadrootsReticulumPreviewBehavior::DeferDeliveryPlans),
    );
    let deferred = deferred_transport
        .fetch(RadrootsReticulumPreviewFetchRequest::new("fetch-deferred", 1).expect("fetch"))
        .expect("fetch receipt");
    assert_eq!(
        deferred.outcome.status,
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
    );
}

#[test]
fn configured_agent_endpoint_is_metadata_only_for_status_delivery_and_fetch() {
    let agent_endpoint =
        RadrootsReticulumPreviewAgentEndpoint::parse("reticulum-agent://localhost:19999")
            .expect("agent endpoint");
    let transport = RadrootsReticulumPreviewTransport::new(
        RadrootsReticulumPreviewProfile::default().with_agent_endpoint(agent_endpoint),
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
        RadrootsTransportImplementationState::PreviewUnavailable
    );
    assert!(!status.transport_status.usable_for_delivery);
    assert!(!status.transport_status.capabilities.deliver);
    assert!(!status.transport_status.capabilities.fetch);

    let receipt = transport
        .deliver(delivery_request(vec![reticulum_target(
            RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI,
        )]))
        .expect("delivery receipt");
    assert_eq!(
        receipt.target_receipts[0].status,
        RadrootsTransportDeliveryTargetStatus::PreviewUnavailable
    );
    let fetch = transport
        .fetch(RadrootsReticulumPreviewFetchRequest::new("fetch-agent", 1).expect("fetch"))
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
        RadrootsTransportImplementationState::PreviewUnavailable
    );
}

#[test]
fn public_models_round_trip_through_serde() {
    let profile = RadrootsReticulumPreviewProfile::default()
        .with_behavior(RadrootsReticulumPreviewBehavior::DeferDeliveryPlans);
    let json = serde_json::to_string(&profile).expect("profile json");
    let decoded: RadrootsReticulumPreviewProfile =
        serde_json::from_str(&json).expect("profile decode");

    assert_eq!(decoded, profile);
}

#[test]
fn preview_source_remains_inert_without_runtime_delivery_hooks() {
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
            "Reticulum preview source contains forbidden runtime hook {forbidden}"
        );
    }
}

#[test]
fn reticulum_preview_errors_and_defaults_are_stable() {
    assert_eq!(
        RadrootsReticulumPreviewBehavior::default(),
        RadrootsReticulumPreviewBehavior::RejectDeliveryAttempts
    );
    let cases = [
        (
            RadrootsReticulumPreviewError::InvalidEndpoint,
            "invalid Reticulum preview endpoint",
        ),
        (
            RadrootsReticulumPreviewError::InvalidAgentEndpoint,
            "invalid Reticulum preview agent endpoint",
        ),
        (
            RadrootsReticulumPreviewError::InvalidProfileId,
            "invalid Reticulum preview profile id",
        ),
        (
            RadrootsReticulumPreviewError::InvalidFetchLimit,
            "Reticulum preview fetch limit must be greater than zero",
        ),
        (
            RadrootsReticulumPreviewError::NonReticulumTarget,
            "Reticulum preview transport received a non-Reticulum target",
        ),
    ];

    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
    }
}
