use radroots_transport::{
    RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI, RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
    RadrootsTransportDeliveryRequest, RadrootsTransportDeliveryTargetStatus,
    RadrootsTransportImplementationState, RadrootsTransportKind, RadrootsTransportReadinessState,
    RadrootsTransportSatisfactionClass, RadrootsTransportSatisfactionPolicy,
    RadrootsTransportTarget, RadrootsTransportTargetSet,
};
use radroots_transport_reticulum::{
    RadrootsReticulumPreviewBehavior, RadrootsReticulumPreviewEndpoint,
    RadrootsReticulumPreviewError, RadrootsReticulumPreviewFetchRequest,
    RadrootsReticulumPreviewProfile, RadrootsReticulumPreviewTransport,
};

fn reticulum_target(uri: &str) -> RadrootsTransportTarget {
    RadrootsTransportTarget::new(RadrootsTransportKind::Reticulum, uri).expect("reticulum target")
}

fn nostr_target() -> RadrootsTransportTarget {
    RadrootsTransportTarget::new(RadrootsTransportKind::Nostr, "wss://relay.example")
        .expect("nostr target")
}

fn delivery_request(targets: Vec<RadrootsTransportTarget>) -> RadrootsTransportDeliveryRequest {
    RadrootsTransportDeliveryRequest::new(
        "reticulum-preview-delivery",
        "sha256:preview-payload",
        RadrootsTransportTargetSet::new(targets).expect("target set"),
        RadrootsTransportSatisfactionPolicy::any_accepted(),
    )
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
        profile.behavior(),
        RadrootsReticulumPreviewBehavior::RejectDeliveryAttempts
    );
    assert_eq!(
        status.transport_status.implementation_state,
        RadrootsTransportImplementationState::PreviewUnavailable
    );
    assert_eq!(
        status.transport_status.readiness,
        RadrootsTransportReadinessState::PreviewUnavailable
    );
    assert_eq!(
        status.transport_status.profile_id.as_deref(),
        Some("transport.reticulum.preview")
    );
    assert_eq!(
        status.transport_status.endpoint_uri.as_deref(),
        Some(RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI)
    );
    assert_eq!(
        status.transport_status.redacted_message.as_deref(),
        Some(RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE)
    );
    assert!(!status.transport_status.publish_usable);
    assert!(!status.transport_status.fetch_usable);
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
    assert_eq!(
        RadrootsReticulumPreviewProfile::new(
            "transport reticulum",
            endpoint,
            RadrootsReticulumPreviewBehavior::RejectDeliveryAttempts,
        )
        .expect_err("profile id whitespace"),
        RadrootsReticulumPreviewError::InvalidProfileId
    );
    assert_eq!(
        RadrootsReticulumPreviewProfile::new(
            "",
            RadrootsReticulumPreviewEndpoint::default(),
            RadrootsReticulumPreviewBehavior::RejectDeliveryAttempts,
        )
        .expect_err("empty profile id"),
        RadrootsReticulumPreviewError::InvalidProfileId
    );
    let profile = RadrootsReticulumPreviewProfile::new(
        "transport.reticulum.custom",
        RadrootsReticulumPreviewEndpoint::default(),
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
fn fetch_reports_preview_unavailable_without_observed_events() {
    let transport = RadrootsReticulumPreviewTransport::default();
    assert_eq!(
        transport.profile().profile_id(),
        "transport.reticulum.preview"
    );
    assert_eq!(
        transport.status().transport_status.implementation_state,
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
        receipt.implementation_state,
        RadrootsTransportImplementationState::PreviewUnavailable
    );
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
fn public_models_round_trip_through_serde() {
    let profile = RadrootsReticulumPreviewProfile::default()
        .with_behavior(RadrootsReticulumPreviewBehavior::DeferDeliveryPlans);
    let json = serde_json::to_string(&profile).expect("profile json");
    let decoded: RadrootsReticulumPreviewProfile =
        serde_json::from_str(&json).expect("profile decode");

    assert_eq!(decoded, profile);
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
