#[cfg(feature = "serde")]
use radroots_transport::RADROOTS_TRANSPORT_TARGET_FINGERPRINT_BYTES;
use radroots_transport::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RADROOTS_RETICULUM_SCOPE_ID,
    RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES, RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES,
    RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES, RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT,
    RADROOTS_TRANSPORT_FETCH_FILTER_MAX_BYTES, RADROOTS_TRANSPORT_FETCH_FILTER_MAX_COUNT,
    RADROOTS_TRANSPORT_FETCH_FILTERS_MAX_BYTES, RADROOTS_TRANSPORT_FETCH_RAW_ITEM_MAX_COUNT,
    RADROOTS_TRANSPORT_FETCH_RAW_JSON_MAX_BYTES, RADROOTS_TRANSPORT_FETCH_REQUEST_ID_MAX_BYTES,
    RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES, RADROOTS_TRANSPORT_OPAQUE_PAYLOAD_MAX_BYTES,
    RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES, RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES,
    RADROOTS_TRANSPORT_RETICULUM_PAYLOAD_MAX_BYTES, RADROOTS_TRANSPORT_SIGNED_EVENT_JSON_MAX_BYTES,
    RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES, RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
    RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES, RADROOTS_TRANSPORT_TOTAL_DEADLINE_MAX_MS,
    RadrootsTransport, RadrootsTransportCapabilities, RadrootsTransportCapabilityAvailability,
    RadrootsTransportCapabilityMaturity, RadrootsTransportDeliveryReceipt,
    RadrootsTransportDeliveryRequest, RadrootsTransportDeliveryTargetStatus,
    RadrootsTransportError, RadrootsTransportFetchReceipt, RadrootsTransportFetchRequest,
    RadrootsTransportFuture, RadrootsTransportImplementationState, RadrootsTransportKind,
    RadrootsTransportMeshScopeId, RadrootsTransportOutcome, RadrootsTransportOutcomeKind,
    RadrootsTransportPayload, RadrootsTransportSatisfactionClass,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportSatisfactionPolicyKind,
    RadrootsTransportStatus, RadrootsTransportTarget, RadrootsTransportTargetFingerprint,
    RadrootsTransportTargetLabel, RadrootsTransportTargetReceipt, RadrootsTransportTargetSet,
    RadrootsTransportTargetUri, ReticulumCapabilityReportV1, ReticulumDestinationV1,
    ReticulumDuplicateFragmentBehaviorV1, ReticulumFragmentIntegrityV1,
    ReticulumFragmentationModeV1, ReticulumGatewaySemanticsV1, ReticulumPrivacySemanticsV1,
};
use serde_json::Value;
use std::borrow::ToOwned;
use std::boxed::Box;
use std::format;
use std::string::{String, ToString};
use std::vec;
use std::vec::Vec;

fn opaque_payload() -> RadrootsTransportPayload {
    RadrootsTransportPayload::opaque_bytes("transport-test-payload", b"transport payload")
        .expect("payload")
}

#[test]
fn target_fingerprints_are_stable_and_transport_scoped() {
    let nostr_upper =
        RadrootsTransportTarget::nostr_relay("WSS://Relay.Example/Events").expect("nostr target");
    let nostr_lower =
        RadrootsTransportTarget::nostr_relay("wss://relay.example/Events").expect("nostr target");
    let reticulum = RadrootsTransportTarget::reticulum().expect("reticulum target");

    assert_eq!(nostr_upper.uri().as_str(), "wss://relay.example/Events");
    assert_eq!(nostr_upper.scope(), None);
    assert_eq!(
        reticulum.scope().map(|scope| scope.as_str()),
        Some(RADROOTS_RETICULUM_SCOPE_ID)
    );
    assert_eq!(nostr_upper.fingerprint(), nostr_lower.fingerprint());
    assert_ne!(nostr_upper.fingerprint(), reticulum.fingerprint());
    assert_eq!(
        nostr_upper.fingerprint().as_str(),
        "d0903c3067150d7b4f7efd92a9be002b97d74e83f8bb6827327fa7ecd869332b"
    );
}

#[test]
fn reticulum_destination_v1_is_canonical_and_stable() {
    let target = RadrootsTransportTarget::reticulum().expect("reticulum target");
    let destination = radroots_transport::ReticulumDestinationV1::from_target(&target)
        .expect("destination from target");
    let local = radroots_transport::ReticulumDestinationV1::local();

    assert_eq!(destination, local);
    assert_eq!(destination.uri().as_str(), RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(
        destination.routing().scope().as_str(),
        RADROOTS_RETICULUM_SCOPE_ID
    );
    assert_eq!(
        destination.routing().gateway(),
        ReticulumGatewaySemanticsV1::NoGatewayForwarding
    );
    assert_eq!(
        destination.routing().privacy(),
        ReticulumPrivacySemanticsV1::CanonicalSignedEventBytesOnly
    );
    assert_eq!(destination.fingerprint(), target.fingerprint());
    assert_eq!(
        destination
            .transport_target()
            .expect("transport target")
            .fingerprint(),
        target.fingerprint()
    );
    assert_eq!(
        destination.fingerprint().as_str(),
        "39142c9a79d6912655e0ad00fb5dbfbe9d2d91b4999e5d68d04a81d89a77f831"
    );
    assert!(
        radroots_transport::ReticulumDestinationV1::new(
            "reticulum:other",
            RadrootsTransportMeshScopeId::local_reticulum(),
            None,
        )
        .is_err()
    );
}

#[test]
#[cfg(feature = "serde")]
fn reticulum_destination_deserialization_revalidates_canonical_identity() {
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
fn reticulum_capability_report_v1_is_explicitly_unavailable_without_fragmentation() {
    let report = ReticulumCapabilityReportV1::unavailable_local();

    assert!(report.is_delivery_required());
    assert!(!report.is_fetch_required());
    assert!(!report.can_deliver());
    assert!(!report.can_fetch());
    assert!(!report.can_discover());
    assert!(!report.can_forward_gateway());
    assert!(!report.can_observe_receipts());
    assert_eq!(
        report.payload_policy().fragment_policy().mode(),
        ReticulumFragmentationModeV1::Unsupported
    );
    assert_eq!(
        report
            .payload_policy()
            .fragment_policy()
            .max_fragment_count(),
        1
    );
    assert_eq!(
        report
            .payload_policy()
            .fragment_policy()
            .max_reassembled_bytes(),
        report.payload_policy().max_payload_bytes()
    );
    assert_eq!(
        report
            .payload_policy()
            .fragment_policy()
            .duplicate_fragment_behavior(),
        ReticulumDuplicateFragmentBehaviorV1::Reject
    );
    assert_eq!(
        report
            .payload_policy()
            .fragment_policy()
            .integrity_verification(),
        ReticulumFragmentIntegrityV1::PayloadDigest
    );
}

#[test]
#[cfg(feature = "serde")]
fn transport_bounds_reticulum_policy_wire_rejects_forged_or_unknown_state() {
    let report = ReticulumCapabilityReportV1::unavailable_local();
    let canonical = serde_json::to_value(&report).expect("serialize capability report");
    assert_eq!(
        serde_json::from_value::<ReticulumCapabilityReportV1>(canonical.clone())
            .expect("reload capability report"),
        report
    );

    for pointer in [
        "/fetch_required",
        "/can_deliver",
        "/can_fetch",
        "/can_discover",
        "/can_forward_gateway",
        "/can_observe_receipts",
    ] {
        let mut forged = canonical.clone();
        *forged.pointer_mut(pointer).expect("capability field") = Value::Bool(true);
        assert!(serde_json::from_value::<ReticulumCapabilityReportV1>(forged).is_err());
    }
    for (pointer, value) in [
        ("/payload_policy/max_payload_bytes", Value::from(65_535)),
        (
            "/payload_policy/fragment_policy/max_fragment_count",
            Value::from(2),
        ),
        (
            "/payload_policy/fragment_policy/max_reassembled_bytes",
            Value::from(65_535),
        ),
    ] {
        let mut forged = canonical.clone();
        *forged.pointer_mut(pointer).expect("policy field") = value;
        assert!(serde_json::from_value::<ReticulumCapabilityReportV1>(forged).is_err());
    }

    let mut unknown = canonical;
    unknown["unexpected"] = Value::Bool(true);
    assert!(serde_json::from_value::<ReticulumCapabilityReportV1>(unknown).is_err());
}

#[test]
fn transport_kind_parser_round_trips_first_wave_canonical_labels() {
    assert_eq!(
        RadrootsTransportKind::parse(" NOSTR ").expect("nostr kind"),
        RadrootsTransportKind::Nostr
    );
    assert_eq!(
        RadrootsTransportKind::parse("reticulum").expect("reticulum kind"),
        RadrootsTransportKind::Reticulum
    );
    assert_eq!(
        RadrootsTransportKind::parse("local").expect("local kind"),
        RadrootsTransportKind::Local
    );
    assert_eq!(
        RadrootsTransportKind::Local.canonical_label(),
        "local".to_owned()
    );
    for retired in [
        "mesh".to_owned(),
        ["pro", "xy"].concat(),
        ["hy", "brid"].concat(),
        ["reticulum", "_preview"].concat(),
        "fieldbus".to_owned(),
    ] {
        assert_eq!(
            RadrootsTransportKind::parse(retired).expect_err("retired or unknown kind"),
            RadrootsTransportError::InvalidTransportKind
        );
    }
}

#[test]
fn canonical_transport_kind_parser_rejects_noncanonical_public_values() {
    assert_eq!(
        RadrootsTransportKind::parse_canonical("nostr").expect("nostr kind"),
        RadrootsTransportKind::Nostr
    );
    assert_eq!(
        RadrootsTransportKind::parse_canonical("NOSTR").expect_err("uppercase kind"),
        RadrootsTransportError::InvalidTransportKind
    );
    assert_eq!(
        RadrootsTransportKind::parse_canonical(" nostr ").expect_err("trimmed kind"),
        RadrootsTransportError::InvalidTransportKind
    );
    assert_eq!(
        RadrootsTransportKind::parse_canonical(removed_radrootsd_execution_transport_kind())
            .expect_err("removed radrootsd execution kind"),
        RadrootsTransportError::InvalidTransportKind
    );
    assert_eq!(
        RadrootsTransportKind::parse_canonical("fieldbus").expect_err("custom kind"),
        RadrootsTransportError::InvalidTransportKind
    );
    assert_eq!(
        RadrootsTransportKind::parse_canonical("").expect_err("empty kind"),
        RadrootsTransportError::EmptyTransportKind
    );
}

fn removed_radrootsd_execution_transport_kind() -> String {
    ["radrootsd", "_", "pro", "xy"].concat()
}

#[test]
fn target_set_rejects_duplicate_fingerprints() {
    let first =
        RadrootsTransportTarget::nostr_relay("wss://relay.example/a").expect("first target");
    let duplicate =
        RadrootsTransportTarget::nostr_relay("WSS://RELAY.EXAMPLE/a").expect("duplicate target");
    let err = RadrootsTransportTargetSet::new(vec![first, duplicate])
        .expect_err("duplicate fingerprints must fail");

    assert_eq!(err, RadrootsTransportError::DuplicateTargetFingerprint);
}

#[test]
fn nostr_relay_targets_use_canonical_endpoint_identity() {
    let root = RadrootsTransportTarget::nostr_relay("wss://relay.example").expect("root target");
    let root_slash =
        RadrootsTransportTarget::nostr_relay("WSS://RELAY.EXAMPLE/").expect("root slash target");
    let root_default_port = RadrootsTransportTarget::nostr_relay("wss://relay.example:443")
        .expect("default-port root target");
    let path =
        RadrootsTransportTarget::nostr_relay("wss://relay.example/path").expect("path target");
    let generic =
        RadrootsTransportTarget::new(RadrootsTransportKind::Nostr, "wss://relay.example/")
            .expect("generic nostr target");

    assert_eq!(root.uri().as_str(), "wss://relay.example");
    assert_eq!(root_slash.uri().as_str(), "wss://relay.example");
    assert_eq!(root_default_port.uri().as_str(), "wss://relay.example");
    assert_eq!(root.fingerprint(), root_slash.fingerprint());
    assert_eq!(root.fingerprint(), root_default_port.fingerprint());
    assert_eq!(root.fingerprint(), generic.fingerprint());
    assert_ne!(root.fingerprint(), path.fingerprint());
    assert_eq!(path.uri().as_str(), "wss://relay.example/path");
    assert_eq!(
        RadrootsTransportTargetSet::new(vec![root, root_slash])
            .expect_err("canonical-equivalent roots collide"),
        RadrootsTransportError::DuplicateTargetFingerprint
    );
}

#[test]
fn nostr_relay_targets_reject_noncanonical_or_unsupported_endpoint_forms() {
    for invalid in [
        "https://relay.example",
        "wss://user@relay.example",
        "wss://user:password@relay.example",
        "wss://relay.example?subscription=1",
        "wss://relay.example#fragment",
        "wss://",
        "wss://relay.example:bad",
        "wss://relay.example:0",
        "wss://relay.example:01",
        "wss://relay.example:65536",
        "wss://:443",
        "wss://[not-ipv6]",
        "wss://relay.example\\path",
        "wss://relay.example/%2f",
        "ws://relay.example",
    ] {
        assert_eq!(
            RadrootsTransportTarget::nostr_relay(invalid).expect_err("invalid Nostr relay target"),
            RadrootsTransportError::InvalidTargetUri
        );
    }

    let local_ws =
        RadrootsTransportTarget::nostr_relay("ws://LOCALHOST:7777/").expect("local ws relay");
    assert_eq!(local_ws.uri().as_str(), "ws://localhost:7777");
    let local_ipv6 =
        RadrootsTransportTarget::nostr_relay("ws://[::1]:7777").expect("local ipv6 relay");
    assert_eq!(local_ipv6.uri().as_str(), "ws://[::1]:7777");
}

#[test]
fn satisfaction_policy_counts_target_statuses() {
    let no_wait = RadrootsTransportSatisfactionPolicy::no_wait();
    let all = RadrootsTransportSatisfactionPolicy::all_accepted();
    let any = RadrootsTransportSatisfactionPolicy::any_accepted();
    let two = RadrootsTransportSatisfactionPolicy::quorum_accepted(2).expect("valid quorum");
    let delivered = RadrootsTransportSatisfactionPolicy::quorum_delivered(2).expect("valid quorum");
    let forwarded = RadrootsTransportSatisfactionPolicy::any_forwarded();
    let stored = RadrootsTransportSatisfactionPolicy::all_stored();
    let seen = RadrootsTransportSatisfactionPolicy::quorum_seen(2).expect("valid quorum");
    let durable_or_observed = RadrootsTransportSatisfactionPolicy::any_durable_or_observed();

    assert_eq!(no_wait.required_target_count(0).expect("no wait"), 0);
    assert_eq!(
        no_wait.kind(),
        RadrootsTransportSatisfactionPolicyKind::NoWait
    );
    assert_eq!(two.kind(), RadrootsTransportSatisfactionPolicyKind::Quorum);
    assert_eq!(two.quorum_threshold(), Some(2));
    assert_eq!(no_wait.required_target_count(3).expect("no wait"), 0);
    assert!(no_wait.is_satisfied_by(0, 0).expect("no wait"));
    assert_ne!(no_wait, all);
    assert!(all.is_satisfied_by(2, 2).expect("all"));
    assert!(!all.is_satisfied_by(2, 1).expect("all incomplete"));
    assert!(any.is_satisfied_by(3, 1).expect("any"));
    assert!(two.is_satisfied_by(3, 2).expect("two"));
    assert_eq!(no_wait.target_satisfaction_class(), None);
    assert_eq!(
        all.target_satisfaction_class(),
        Some(RadrootsTransportSatisfactionClass::Accepted)
    );
    assert_eq!(
        delivered.target_satisfaction_class(),
        Some(RadrootsTransportSatisfactionClass::Delivered)
    );
    assert_eq!(
        forwarded.target_satisfaction_class(),
        Some(RadrootsTransportSatisfactionClass::Forwarded)
    );
    assert_eq!(
        stored.target_satisfaction_class(),
        Some(RadrootsTransportSatisfactionClass::Stored)
    );
    assert_eq!(
        seen.target_satisfaction_class(),
        Some(RadrootsTransportSatisfactionClass::Seen)
    );
    assert_eq!(
        durable_or_observed.target_satisfaction_class(),
        Some(RadrootsTransportSatisfactionClass::DurableOrObserved)
    );
    for (policy, class) in [
        (
            RadrootsTransportSatisfactionPolicy::all_forwarded(),
            RadrootsTransportSatisfactionClass::Forwarded,
        ),
        (
            RadrootsTransportSatisfactionPolicy::quorum_forwarded(2).expect("valid quorum"),
            RadrootsTransportSatisfactionClass::Forwarded,
        ),
        (
            RadrootsTransportSatisfactionPolicy::any_stored(),
            RadrootsTransportSatisfactionClass::Stored,
        ),
        (
            RadrootsTransportSatisfactionPolicy::quorum_stored(2).expect("valid quorum"),
            RadrootsTransportSatisfactionClass::Stored,
        ),
        (
            RadrootsTransportSatisfactionPolicy::any_seen(),
            RadrootsTransportSatisfactionClass::Seen,
        ),
        (
            RadrootsTransportSatisfactionPolicy::all_seen(),
            RadrootsTransportSatisfactionClass::Seen,
        ),
        (
            RadrootsTransportSatisfactionPolicy::all_durable_or_observed(),
            RadrootsTransportSatisfactionClass::DurableOrObserved,
        ),
        (
            RadrootsTransportSatisfactionPolicy::quorum_durable_or_observed(2)
                .expect("valid quorum"),
            RadrootsTransportSatisfactionClass::DurableOrObserved,
        ),
    ] {
        assert_eq!(policy.target_satisfaction_class(), Some(class));
    }
    assert_eq!(
        any.is_satisfied_by(0, 0).expect_err("zero target set"),
        RadrootsTransportError::InvalidSatisfactionPolicy
    );
    assert_eq!(
        RadrootsTransportSatisfactionPolicy::quorum_accepted(0).expect_err("zero required targets"),
        RadrootsTransportError::InvalidSatisfactionPolicy
    );
}

#[test]
#[cfg(feature = "serde")]
fn transport_status_models_canonical_configuration_and_delivery_usability() {
    let status = RadrootsTransportStatus::new(
        RadrootsTransportKind::Nostr,
        true,
        RadrootsTransportImplementationState::Real,
        true,
        "ready",
    )
    .expect("bounded status")
    .try_with_profile_id("transport.nostr.default")
    .expect("bounded profile id")
    .try_with_endpoint_uri("wss://relay.example")
    .expect("bounded endpoint URI");

    assert_eq!(status.kind(), &RadrootsTransportKind::Nostr);
    assert_eq!(status.profile_id(), Some("transport.nostr.default"));
    assert_eq!(status.endpoint_uri(), Some("wss://relay.example"));
    assert!(status.is_configured());
    assert_eq!(
        status.implementation(),
        RadrootsTransportImplementationState::Real
    );
    assert!(status.is_usable_for_delivery());
    assert_eq!(
        status.capabilities(),
        &RadrootsTransportCapabilities::deliver_only()
    );
    assert_eq!(status.message(), "ready");

    let json = serde_json::to_value(&status).expect("status json");
    assert_eq!(json["transport"], "nostr");
    assert_eq!(json["implementation"], "real");
    assert_eq!(json["configured"], true);
    assert_eq!(json["usable_for_delivery"], true);
    assert_eq!(json["capabilities"]["deliver"], true);
    assert_eq!(json["capabilities"]["fetch"], false);
    assert_eq!(json["capabilities"]["discovery"], false);
    assert_eq!(json["capabilities"]["gateway_forwarding"], false);
    assert_eq!(json["capabilities"]["receipt_observation"], false);
    assert_eq!(json["message"], "ready");
    for retired in [
        "kind",
        "implementation_state",
        "readiness",
        "publish_usable",
        "fetch_usable",
        "redacted_message",
    ] {
        assert!(
            json.get(retired).is_none(),
            "retired status field {retired}"
        );
    }
}

#[test]
fn deferred_transport_outcomes_are_terminal_but_not_satisfied() {
    let target = RadrootsTransportTarget::reticulum().expect("target");
    let receipt = RadrootsTransportDeliveryReceipt::new(
        "reticulum",
        RadrootsTransportTargetSet::new(vec![target.clone()]).expect("target set"),
        vec![RadrootsTransportTargetReceipt::new(
            target,
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::DeferredUntilImplemented),
        )],
    )
    .expect("receipt");

    assert!(
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
            .is_deferred_until_implemented()
    );
    assert_eq!(
        receipt.satisfied_target_count(RadrootsTransportSatisfactionClass::Accepted),
        0
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
#[cfg(feature = "serde")]
fn request_models_round_trip_with_serde() {
    let target = RadrootsTransportTarget::nostr_relay("wss://relay.example").expect("target");
    let target_set = RadrootsTransportTargetSet::new(vec![target.clone()]).expect("target set");
    let request = RadrootsTransportDeliveryRequest::new(
        "req-1",
        opaque_payload(),
        target_set.clone(),
        RadrootsTransportSatisfactionPolicy::any_accepted(),
    )
    .expect("request");

    let json = serde_json::to_string(&request).expect("serialize request");
    let decoded: RadrootsTransportDeliveryRequest =
        serde_json::from_str(&json).expect("decode request");

    assert_eq!(decoded, request);

    let fetch_request =
        RadrootsTransportFetchRequest::new("fetch-1", target_set).expect("fetch request");
    let fetch_receipt = RadrootsTransportFetchReceipt::for_request(
        &fetch_request,
        vec![RadrootsTransportTargetReceipt::new(
            target,
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Seen),
        )],
        1,
    )
    .expect("fetch receipt");
    let fetch_request_json =
        serde_json::to_string(&fetch_request).expect("serialize fetch request");
    let fetch_receipt_json =
        serde_json::to_string(&fetch_receipt).expect("serialize fetch receipt");
    assert_eq!(
        serde_json::from_str::<RadrootsTransportFetchRequest>(&fetch_request_json)
            .expect("decode fetch request"),
        fetch_request
    );
    assert_eq!(
        serde_json::from_str::<RadrootsTransportFetchReceipt>(&fetch_receipt_json)
            .expect("decode fetch receipt"),
        fetch_receipt
    );
}

#[test]
fn payload_contract_derives_and_validates_unchecked_signed_event_digests() {
    let event_id = "a".repeat(64);
    let signed = RadrootsTransportPayload::unchecked_signed_event_json(
        event_id.as_str(),
        "{\"id\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}",
    )
    .expect("unchecked signed event payload");
    assert_eq!(signed.payload_kind(), "signed_event_json");
    assert_eq!(signed.digest().len(), 64);
    assert!(
        signed
            .digest()
            .bytes()
            .all(|byte| { byte.is_ascii_digit() || matches!(byte, b'a'..=b'f') })
    );
    assert_eq!(
        RadrootsTransportPayload::unchecked_signed_event_json_with_digest(
            event_id.as_str(),
            "{\"id\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}",
            signed.digest(),
        )
        .expect("unchecked signed payload with digest"),
        signed
    );

    let mesh =
        RadrootsTransportPayload::mesh_frame_cbor("mesh.message-1", [1_u8, 2, 3]).expect("mesh");
    assert_eq!(
        RadrootsTransportPayload::mesh_frame_cbor_with_digest(
            "mesh.message-1",
            [1_u8, 2, 3],
            mesh.digest(),
        )
        .expect("mesh with digest"),
        mesh
    );

    let opaque = RadrootsTransportPayload::opaque_bytes("operator note", b"bytes").expect("opaque");
    assert_eq!(
        RadrootsTransportPayload::opaque_bytes_with_digest(
            "operator note",
            b"bytes",
            opaque.digest(),
        )
        .expect("opaque with digest"),
        opaque
    );
}

#[test]
fn payload_contract_rejects_invalid_unchecked_signed_event_ids_bytes_labels_and_digests() {
    assert_eq!(
        RadrootsTransportPayload::unchecked_signed_event_json("A".repeat(64), "{}")
            .expect_err("uppercase event id"),
        RadrootsTransportError::InvalidPayloadId
    );
    assert_eq!(
        RadrootsTransportPayload::unchecked_signed_event_json("a".repeat(64), " [] ")
            .expect_err("non-object json"),
        RadrootsTransportError::InvalidPayloadBytes
    );
    assert_eq!(
        RadrootsTransportPayload::mesh_frame_cbor("mesh message", [1_u8])
            .expect_err("space in message id"),
        RadrootsTransportError::InvalidPayloadId
    );
    assert_eq!(
        RadrootsTransportPayload::mesh_frame_cbor("mesh-message", [])
            .expect_err("empty mesh bytes"),
        RadrootsTransportError::EmptyPayloadBytes
    );
    assert_eq!(
        RadrootsTransportPayload::opaque_bytes("bad\u{0007}", b"bytes")
            .expect_err("control character label"),
        RadrootsTransportError::InvalidPayloadLabel
    );
    assert_eq!(
        RadrootsTransportPayload::opaque_bytes_with_digest("label", b"bytes", "f".repeat(64))
            .expect_err("digest mismatch"),
        RadrootsTransportError::PayloadDigestMismatch
    );
    assert_eq!(
        RadrootsTransportPayload::opaque_bytes_with_digest("label", b"bytes", "F".repeat(64))
            .expect_err("uppercase digest"),
        RadrootsTransportError::InvalidPayloadDigest
    );
}

#[test]
fn fingerprint_parser_rejects_non_sha256_hex() {
    assert_eq!(
        RadrootsTransportTargetFingerprint::parse("abc").expect_err("short fingerprint"),
        RadrootsTransportError::InvalidTargetFingerprint
    );
    assert_eq!(
        RadrootsTransportTargetUri::parse("wss://relay example").expect_err("space in target uri"),
        RadrootsTransportError::InvalidTargetUri
    );
}

#[test]
fn transport_errors_have_stable_display_strings() {
    let cases = [
        (
            RadrootsTransportError::UnsupportedOperation,
            "transport operation is unsupported",
        ),
        (
            RadrootsTransportError::EmptyTransportKind,
            "transport kind is empty",
        ),
        (
            RadrootsTransportError::InvalidTransportKind,
            "transport kind is invalid",
        ),
        (
            RadrootsTransportError::EmptyTargetUri,
            "transport target URI is empty",
        ),
        (
            RadrootsTransportError::InvalidTargetUri,
            "transport target URI is invalid",
        ),
        (
            RadrootsTransportError::EmptyTargetScope,
            "transport target scope is empty",
        ),
        (
            RadrootsTransportError::InvalidTargetScope,
            "transport target scope is invalid",
        ),
        (
            RadrootsTransportError::EmptyTargetLabel,
            "transport target label is empty",
        ),
        (
            RadrootsTransportError::InvalidTargetLabel,
            "transport target label is invalid",
        ),
        (
            RadrootsTransportError::EmptyTargetSet,
            "transport target set is empty",
        ),
        (
            RadrootsTransportError::DuplicateTargetFingerprint,
            "transport target set contains duplicate fingerprints",
        ),
        (
            RadrootsTransportError::InvalidTargetFingerprint,
            "transport target fingerprint is invalid",
        ),
        (
            RadrootsTransportError::InvalidSatisfactionPolicy,
            "transport satisfaction policy is invalid",
        ),
        (
            RadrootsTransportError::EmptyRequiredTargetSet,
            "transport required target set is empty",
        ),
        (
            RadrootsTransportError::DuplicateRequiredTargetFingerprint,
            "transport required target set contains duplicate fingerprints",
        ),
    ];

    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
    }
}

#[test]
fn transport_kind_and_target_parsers_cover_negative_edges() {
    for invalid in ["bad kind", "bad:kind", "bad/kind", "bad\nkind", "fieldbus"] {
        assert_eq!(
            RadrootsTransportKind::parse(invalid).expect_err("invalid kind"),
            RadrootsTransportError::InvalidTransportKind
        );
    }

    let no_scheme =
        RadrootsTransportTargetUri::parse("transport-target").expect("schemeless target uri");
    assert_eq!(no_scheme.as_str(), "transport-target");
    assert_eq!(no_scheme.to_string(), "transport-target");
    let opaque = RadrootsTransportTargetUri::parse("RNS:PeerA").expect("opaque uri");
    assert_eq!(opaque.as_str(), "rns:PeerA");
    let authority = RadrootsTransportTargetUri::parse("MESH://Node.Example/path?q=1#frag")
        .expect("authority uri");
    assert_eq!(authority.as_str(), "mesh://node.example/path?q=1#frag");

    assert_eq!(
        RadrootsTransportTargetUri::parse(" ").expect_err("empty uri"),
        RadrootsTransportError::EmptyTargetUri
    );
    for invalid in [
        "bad target",
        " transport-target ",
        ":target",
        "1bad:target",
        "bad_scheme://target",
        "bad\target",
    ] {
        assert_eq!(
            RadrootsTransportTargetUri::parse(invalid).expect_err("invalid uri"),
            RadrootsTransportError::InvalidTargetUri
        );
    }
}

#[test]
fn checked_in_transport_target_uri_vectors_match_parser_behavior() {
    let vectors =
        include_str!("../../../contracts/conformance/vectors/transport/target_uri.v1.json");
    let document: Value = serde_json::from_str(vectors).expect("transport target vector json");
    let entries = document
        .get("vectors")
        .and_then(Value::as_array)
        .expect("transport target vectors");

    for entry in entries {
        let kind = entry.get("kind").and_then(Value::as_str).expect("kind");
        let raw_uri = entry
            .get("input")
            .and_then(|input| input.get("uri"))
            .and_then(Value::as_str)
            .expect("input uri");
        let expected = entry.get("expected").expect("expected");
        match kind {
            "transport.target_uri.valid" => {
                let target = RadrootsTransportTargetUri::parse(raw_uri).expect("target URI");
                assert_eq!(
                    target.as_str(),
                    expected
                        .get("canonical_uri")
                        .and_then(Value::as_str)
                        .expect("canonical uri")
                );
            }
            "transport.target_uri.invalid" => {
                assert!(RadrootsTransportTargetUri::parse(raw_uri).is_err());
            }
            "transport.nostr_relay_target.valid" => {
                let target = RadrootsTransportTarget::nostr_relay(raw_uri).expect("relay target");
                assert_eq!(
                    target.uri().as_str(),
                    expected
                        .get("canonical_uri")
                        .and_then(Value::as_str)
                        .expect("canonical uri")
                );
            }
            "transport.nostr_relay_target.invalid" => {
                assert!(RadrootsTransportTarget::nostr_relay(raw_uri).is_err());
            }
            other => panic!("unknown transport target vector kind {other}"),
        }
    }
}

#[test]
fn transport_bounds_checked_in_resource_manifest_matches_exported_constants() {
    let vectors =
        include_str!("../../../contracts/conformance/vectors/transport/resource_limits.v1.json");
    let document: Value = serde_json::from_str(vectors).expect("transport resource manifest json");
    let entries = document
        .get("vectors")
        .and_then(Value::as_array)
        .expect("transport resource vectors");
    let expected = [
        (
            "transport_signed_event_json_max_bytes_001",
            "radroots_transport::RADROOTS_TRANSPORT_SIGNED_EVENT_JSON_MAX_BYTES",
            "resource_authority",
            RADROOTS_TRANSPORT_SIGNED_EVENT_JSON_MAX_BYTES as u64,
            "bytes",
        ),
        (
            "transport_reticulum_payload_max_bytes_002",
            "radroots_transport::RADROOTS_TRANSPORT_RETICULUM_PAYLOAD_MAX_BYTES",
            "resource_authority",
            RADROOTS_TRANSPORT_RETICULUM_PAYLOAD_MAX_BYTES as u64,
            "bytes",
        ),
        (
            "transport_opaque_payload_max_bytes_003",
            "radroots_transport::RADROOTS_TRANSPORT_OPAQUE_PAYLOAD_MAX_BYTES",
            "RADROOTS_TRANSPORT_RETICULUM_PAYLOAD_MAX_BYTES",
            RADROOTS_TRANSPORT_OPAQUE_PAYLOAD_MAX_BYTES as u64,
            "bytes",
        ),
        (
            "transport_endpoint_uri_max_bytes_004",
            "radroots_transport::RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES",
            "resource_authority",
            RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES as u64,
            "utf8_bytes",
        ),
        (
            "transport_identifier_max_bytes_005",
            "radroots_transport::RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES",
            "resource_authority",
            RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES as u64,
            "utf8_bytes",
        ),
        (
            "transport_target_scope_max_bytes_006",
            "radroots_transport::RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES",
            "RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES",
            RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES as u64,
            "utf8_bytes",
        ),
        (
            "transport_target_label_max_bytes_007",
            "radroots_transport::RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES",
            "RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES",
            RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES as u64,
            "utf8_bytes",
        ),
        (
            "transport_unique_target_max_count_008",
            "radroots_transport::RADROOTS_TRANSPORT_TARGET_MAX_COUNT",
            "resource_authority",
            RADROOTS_TRANSPORT_TARGET_MAX_COUNT as u64,
            "items",
        ),
        (
            "transport_fetch_filter_max_count_009",
            "radroots_transport::RADROOTS_TRANSPORT_FETCH_FILTER_MAX_COUNT",
            "resource_authority",
            RADROOTS_TRANSPORT_FETCH_FILTER_MAX_COUNT as u64,
            "items",
        ),
        (
            "transport_fetch_filter_max_bytes_010",
            "radroots_transport::RADROOTS_TRANSPORT_FETCH_FILTER_MAX_BYTES",
            "resource_authority",
            RADROOTS_TRANSPORT_FETCH_FILTER_MAX_BYTES as u64,
            "compact_json_bytes",
        ),
        (
            "transport_fetch_filters_max_bytes_011",
            "radroots_transport::RADROOTS_TRANSPORT_FETCH_FILTERS_MAX_BYTES",
            "RADROOTS_TRANSPORT_FETCH_FILTER_MAX_COUNT * RADROOTS_TRANSPORT_FETCH_FILTER_MAX_BYTES",
            RADROOTS_TRANSPORT_FETCH_FILTERS_MAX_BYTES as u64,
            "compact_json_bytes",
        ),
        (
            "transport_fetch_admitted_event_max_count_012",
            "radroots_transport::RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT",
            "resource_authority",
            RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT as u64,
            "items",
        ),
        (
            "transport_fetch_raw_item_max_count_013",
            "radroots_transport::RADROOTS_TRANSPORT_FETCH_RAW_ITEM_MAX_COUNT",
            "resource_authority",
            RADROOTS_TRANSPORT_FETCH_RAW_ITEM_MAX_COUNT as u64,
            "items",
        ),
        (
            "transport_fetch_raw_json_max_bytes_014",
            "radroots_transport::RADROOTS_TRANSPORT_FETCH_RAW_JSON_MAX_BYTES",
            "resource_authority",
            RADROOTS_TRANSPORT_FETCH_RAW_JSON_MAX_BYTES as u64,
            "bytes",
        ),
        (
            "transport_complete_request_diagnostic_max_bytes_015",
            "radroots_transport::RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES",
            "resource_authority",
            RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES as u64,
            "utf8_bytes",
        ),
        (
            "transport_outcome_code_max_bytes_016",
            "radroots_transport::RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES",
            "RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES",
            RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES as u64,
            "utf8_bytes",
        ),
        (
            "transport_outcome_message_max_bytes_017",
            "radroots_transport::RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES",
            "RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES",
            RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES as u64,
            "utf8_bytes",
        ),
        (
            "transport_total_deadline_max_ms_018",
            "radroots_transport::RADROOTS_TRANSPORT_TOTAL_DEADLINE_MAX_MS",
            "resource_authority",
            RADROOTS_TRANSPORT_TOTAL_DEADLINE_MAX_MS,
            "milliseconds",
        ),
        (
            "transport_delivery_request_id_max_bytes_019",
            "radroots_transport::RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES",
            "RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES",
            RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES as u64,
            "utf8_bytes",
        ),
        (
            "transport_fetch_request_id_max_bytes_020",
            "radroots_transport::RADROOTS_TRANSPORT_FETCH_REQUEST_ID_MAX_BYTES",
            "RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES",
            RADROOTS_TRANSPORT_FETCH_REQUEST_ID_MAX_BYTES as u64,
            "utf8_bytes",
        ),
    ];

    assert_eq!(entries.len(), expected.len());
    for (entry, (id, authority, derivation, maximum, unit)) in entries.iter().zip(expected) {
        assert_eq!(
            entry,
            &serde_json::json!({
                "id": id,
                "kind": "transport.resource_limit.exact",
                "input": {
                    "authority": authority,
                    "derivation": derivation,
                },
                "expected": {
                    "maximum": maximum,
                    "unit": unit,
                },
            })
        );
    }

    assert_eq!(
        RADROOTS_TRANSPORT_FETCH_FILTERS_MAX_BYTES,
        RADROOTS_TRANSPORT_FETCH_FILTER_MAX_COUNT * RADROOTS_TRANSPORT_FETCH_FILTER_MAX_BYTES
    );
}

#[test]
fn reticulum_transport_targets_use_default_destination_and_scope() {
    let target = RadrootsTransportTarget::reticulum().expect("Reticulum target");
    assert_eq!(target.uri().as_str(), RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(
        target.scope().map(|scope| scope.as_str()),
        Some(RADROOTS_RETICULUM_SCOPE_ID)
    );

    let invalid_reticulum_destination = ["reticulum:", "remote"].concat();
    for invalid in [
        " reticulum:local".to_owned(),
        "reticulum:local ".to_owned(),
        "RETICULUM:local".to_owned(),
        invalid_reticulum_destination,
    ] {
        assert_eq!(
            RadrootsTransportTarget::new(RadrootsTransportKind::Reticulum, invalid.as_str())
                .expect_err("invalid Reticulum endpoint"),
            RadrootsTransportError::InvalidTargetUri
        );
    }
}

#[test]
fn target_fingerprints_and_sets_cover_accessors_and_validation() {
    let target = RadrootsTransportTarget::reticulum().expect("Reticulum target");
    let parsed = RadrootsTransportTargetFingerprint::parse(
        target.fingerprint().as_str().to_ascii_uppercase(),
    )
    .expect("uppercase fingerprint parses");
    assert_eq!(parsed.as_str(), target.fingerprint().as_str());
    assert_eq!(parsed.to_string(), target.fingerprint().as_str());
    assert_eq!(
        RadrootsTransportTargetFingerprint::parse("g".repeat(64)).expect_err("non-hex fingerprint"),
        RadrootsTransportError::InvalidTargetFingerprint
    );

    assert_eq!(
        RadrootsTransportTargetSet::new(Vec::new()).expect_err("empty target set"),
        RadrootsTransportError::EmptyTargetSet
    );
    let target_set = RadrootsTransportTargetSet::new(vec![target]).expect("target set");
    assert_eq!(target_set.len(), 1);
    assert!(!target_set.is_empty());
    assert_eq!(target_set.targets().len(), 1);
}

#[test]
fn target_scope_participates_in_identity_and_label_does_not() {
    let local_scope = RadrootsTransportMeshScopeId::parse("local").expect("local scope");
    let remote_scope = RadrootsTransportMeshScopeId::parse("remote").expect("remote scope");
    let local = RadrootsTransportTarget::new_with_metadata(
        RadrootsTransportKind::Reticulum,
        RADROOTS_RETICULUM_ENDPOINT_URI,
        Some(local_scope.clone()),
        Some(RadrootsTransportTargetLabel::parse("Local Reticulum node").expect("label")),
    )
    .expect("local Reticulum target");
    let relabeled = RadrootsTransportTarget::new_with_metadata(
        RadrootsTransportKind::Reticulum,
        RADROOTS_RETICULUM_ENDPOINT_URI,
        Some(local_scope),
        Some(RadrootsTransportTargetLabel::parse("Renamed node").expect("label")),
    )
    .expect("relabeled mesh target");
    let remote = RadrootsTransportTarget::new_with_metadata(
        RadrootsTransportKind::Reticulum,
        RADROOTS_RETICULUM_ENDPOINT_URI,
        Some(remote_scope),
        None,
    )
    .expect("remote Reticulum target");

    assert_eq!(local.fingerprint(), relabeled.fingerprint());
    assert_ne!(local.fingerprint(), remote.fingerprint());
    assert_eq!(local.scope().map(|scope| scope.as_str()), Some("local"));
    assert_eq!(
        local.label().map(|label| label.as_str()),
        Some("Local Reticulum node")
    );
    assert_eq!(
        RadrootsTransportMeshScopeId::parse("").expect_err("empty scope"),
        RadrootsTransportError::EmptyTargetScope
    );
    assert_eq!(
        RadrootsTransportMeshScopeId::parse("bad scope").expect_err("invalid scope"),
        RadrootsTransportError::InvalidTargetScope
    );
    assert_eq!(
        RadrootsTransportTargetLabel::parse(" ").expect_err("empty label"),
        RadrootsTransportError::EmptyTargetLabel
    );
    assert_eq!(
        RadrootsTransportTargetLabel::parse("bad\nlabel").expect_err("invalid label"),
        RadrootsTransportError::InvalidTargetLabel
    );
}

#[test]
fn satisfaction_and_target_status_cover_all_contract_states() {
    assert_eq!(
        RadrootsTransportSatisfactionPolicy::no_wait()
            .required_target_count(0)
            .expect("no wait"),
        0
    );
    assert_eq!(
        RadrootsTransportSatisfactionPolicy::all_accepted()
            .required_target_count(3)
            .expect("all targets"),
        3
    );
    assert_eq!(
        RadrootsTransportSatisfactionPolicy::any_accepted()
            .required_target_count(3)
            .expect("any target"),
        1
    );
    assert_eq!(
        RadrootsTransportSatisfactionPolicy::quorum_accepted(4)
            .expect("bounded quorum")
            .required_target_count(3)
            .expect_err("at least too high"),
        RadrootsTransportError::InvalidSatisfactionPolicy
    );

    let classes = [
        RadrootsTransportSatisfactionClass::Accepted,
        RadrootsTransportSatisfactionClass::Forwarded,
        RadrootsTransportSatisfactionClass::Stored,
        RadrootsTransportSatisfactionClass::Seen,
        RadrootsTransportSatisfactionClass::Delivered,
        RadrootsTransportSatisfactionClass::DurableOrObserved,
    ];
    let cases: &[(
        RadrootsTransportDeliveryTargetStatus,
        &[RadrootsTransportSatisfactionClass],
    )] = &[
        (RadrootsTransportDeliveryTargetStatus::Pending, &[]),
        (
            RadrootsTransportDeliveryTargetStatus::Accepted,
            &[RadrootsTransportSatisfactionClass::Accepted],
        ),
        (
            RadrootsTransportDeliveryTargetStatus::Delivered,
            &[
                RadrootsTransportSatisfactionClass::Accepted,
                RadrootsTransportSatisfactionClass::Forwarded,
                RadrootsTransportSatisfactionClass::Seen,
                RadrootsTransportSatisfactionClass::Delivered,
                RadrootsTransportSatisfactionClass::DurableOrObserved,
            ],
        ),
        (
            RadrootsTransportDeliveryTargetStatus::Forwarded,
            &[
                RadrootsTransportSatisfactionClass::Accepted,
                RadrootsTransportSatisfactionClass::Forwarded,
            ],
        ),
        (
            RadrootsTransportDeliveryTargetStatus::StoredByGateway,
            &[
                RadrootsTransportSatisfactionClass::Accepted,
                RadrootsTransportSatisfactionClass::Stored,
                RadrootsTransportSatisfactionClass::DurableOrObserved,
            ],
        ),
        (
            RadrootsTransportDeliveryTargetStatus::Seen,
            &[
                RadrootsTransportSatisfactionClass::Accepted,
                RadrootsTransportSatisfactionClass::Seen,
                RadrootsTransportSatisfactionClass::DurableOrObserved,
            ],
        ),
        (
            RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented,
            &[],
        ),
        (
            RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented,
            &[],
        ),
        (
            RadrootsTransportDeliveryTargetStatus::SkippedPolicyDenied,
            &[],
        ),
        (RadrootsTransportDeliveryTargetStatus::FailedRetryable, &[]),
        (RadrootsTransportDeliveryTargetStatus::FailedTerminal, &[]),
    ];
    assert!(RadrootsTransportDeliveryTargetStatus::Pending.is_ready_for_attempt());
    assert!(RadrootsTransportDeliveryTargetStatus::FailedRetryable.is_ready_for_attempt());
    for (status, satisfied_classes) in cases {
        for class in classes {
            assert_eq!(
                status.counts_as_satisfied(class),
                satisfied_classes.contains(&class),
                "{status:?} / {class:?}"
            );
        }
    }
    assert!(
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
            .is_deferred_until_implemented()
    );
    assert!(RadrootsTransportDeliveryTargetStatus::FailedRetryable.is_retryable_failure());
    assert!(RadrootsTransportDeliveryTargetStatus::FailedTerminal.is_terminal_failure());
}

#[test]
fn typed_outcome_kinds_drive_status_and_satisfaction_semantics() {
    let classes = [
        RadrootsTransportSatisfactionClass::Accepted,
        RadrootsTransportSatisfactionClass::Forwarded,
        RadrootsTransportSatisfactionClass::Stored,
        RadrootsTransportSatisfactionClass::Seen,
        RadrootsTransportSatisfactionClass::Delivered,
        RadrootsTransportSatisfactionClass::DurableOrObserved,
    ];
    let cases = [
        (
            RadrootsTransportOutcomeKind::Accepted,
            "accepted",
            RadrootsTransportDeliveryTargetStatus::Accepted,
            &[RadrootsTransportSatisfactionClass::Accepted] as &[_],
        ),
        (
            RadrootsTransportOutcomeKind::DuplicateAccepted,
            "duplicate_accepted",
            RadrootsTransportDeliveryTargetStatus::Accepted,
            &[RadrootsTransportSatisfactionClass::Accepted],
        ),
        (
            RadrootsTransportOutcomeKind::Delivered,
            "delivered",
            RadrootsTransportDeliveryTargetStatus::Delivered,
            &[
                RadrootsTransportSatisfactionClass::Accepted,
                RadrootsTransportSatisfactionClass::Forwarded,
                RadrootsTransportSatisfactionClass::Seen,
                RadrootsTransportSatisfactionClass::Delivered,
                RadrootsTransportSatisfactionClass::DurableOrObserved,
            ],
        ),
        (
            RadrootsTransportOutcomeKind::Forwarded,
            "forwarded",
            RadrootsTransportDeliveryTargetStatus::Forwarded,
            &[
                RadrootsTransportSatisfactionClass::Accepted,
                RadrootsTransportSatisfactionClass::Forwarded,
            ],
        ),
        (
            RadrootsTransportOutcomeKind::StoredByGateway,
            "stored_by_gateway",
            RadrootsTransportDeliveryTargetStatus::StoredByGateway,
            &[
                RadrootsTransportSatisfactionClass::Accepted,
                RadrootsTransportSatisfactionClass::Stored,
                RadrootsTransportSatisfactionClass::DurableOrObserved,
            ],
        ),
        (
            RadrootsTransportOutcomeKind::Seen,
            "seen",
            RadrootsTransportDeliveryTargetStatus::Seen,
            &[
                RadrootsTransportSatisfactionClass::Accepted,
                RadrootsTransportSatisfactionClass::Seen,
                RadrootsTransportSatisfactionClass::DurableOrObserved,
            ],
        ),
        (
            RadrootsTransportOutcomeKind::DeferredUntilImplemented,
            "deferred_until_implemented",
            RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented,
            &[],
        ),
        (
            RadrootsTransportOutcomeKind::Rejected,
            "rejected",
            RadrootsTransportDeliveryTargetStatus::FailedTerminal,
            &[],
        ),
        (
            RadrootsTransportOutcomeKind::RouteUnavailable,
            "route_unavailable",
            RadrootsTransportDeliveryTargetStatus::FailedTerminal,
            &[],
        ),
        (
            RadrootsTransportOutcomeKind::PayloadTooLarge,
            "payload_too_large",
            RadrootsTransportDeliveryTargetStatus::FailedTerminal,
            &[],
        ),
        (
            RadrootsTransportOutcomeKind::PolicyDenied,
            "policy_denied",
            RadrootsTransportDeliveryTargetStatus::SkippedPolicyDenied,
            &[],
        ),
        (
            RadrootsTransportOutcomeKind::Timeout,
            "timeout",
            RadrootsTransportDeliveryTargetStatus::FailedRetryable,
            &[],
        ),
        (
            RadrootsTransportOutcomeKind::ConnectionFailed,
            "connection_failed",
            RadrootsTransportDeliveryTargetStatus::FailedRetryable,
            &[],
        ),
        (
            RadrootsTransportOutcomeKind::TransportUnavailable,
            "transport_unavailable",
            RadrootsTransportDeliveryTargetStatus::FailedRetryable,
            &[],
        ),
    ];

    for (kind, label, status, satisfied_classes) in cases {
        let outcome = RadrootsTransportOutcome::new(kind)
            .try_with_message("transport detail")
            .expect("bounded transport detail");
        assert_eq!(kind.as_str(), label);
        assert_eq!(outcome.kind(), kind);
        assert_eq!(outcome.status(), status);
        for class in classes {
            assert_eq!(
                kind.counts_as_satisfied(class),
                satisfied_classes.contains(&class),
                "{kind:?} / {class:?}"
            );
        }
        assert_eq!(outcome.message(), Some("transport detail"));
    }

    let deferred_until_implemented =
        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::DeferredUntilImplemented);
    assert_eq!(
        deferred_until_implemented.status(),
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
    );
    deferred_until_implemented
        .validate()
        .expect("canonical outcome");
}

#[test]
fn required_target_satisfaction_uses_fingerprints_not_target_counts() {
    let required =
        RadrootsTransportTarget::nostr_relay("wss://one.example").expect("required target");
    let optional =
        RadrootsTransportTarget::nostr_relay("wss://two.example").expect("optional target");
    let policy = RadrootsTransportSatisfactionPolicy::required_targets(
        RadrootsTransportSatisfactionClass::Accepted,
        vec![required.fingerprint().clone()],
    )
    .expect("required target policy");
    assert_eq!(policy.required_target_count(2).expect("required count"), 1);
    assert_eq!(
        policy
            .is_satisfied_by(2, 1)
            .expect_err("count-only required targets are invalid"),
        RadrootsTransportError::InvalidSatisfactionPolicy
    );
    assert_eq!(
        policy
            .required_target_count(0)
            .expect_err("required target count exceeds total targets"),
        RadrootsTransportError::InvalidSatisfactionPolicy
    );
    assert_eq!(
        policy.target_satisfaction_class(),
        Some(RadrootsTransportSatisfactionClass::Accepted)
    );
    assert_eq!(
        policy
            .required_target_fingerprints()
            .expect("required targets"),
        std::slice::from_ref(required.fingerprint())
    );
    let unordered_policy = RadrootsTransportSatisfactionPolicy::required_targets(
        RadrootsTransportSatisfactionClass::Accepted,
        vec![
            optional.fingerprint().clone(),
            required.fingerprint().clone(),
        ],
    )
    .expect("unordered required targets");
    let mut expected_required_targets = vec![
        required.fingerprint().clone(),
        optional.fingerprint().clone(),
    ];
    expected_required_targets.sort();
    assert_eq!(
        unordered_policy
            .required_target_fingerprints()
            .expect("canonical required targets"),
        expected_required_targets.as_slice()
    );

    let optional_only = RadrootsTransportDeliveryReceipt::new(
        "required-target",
        RadrootsTransportTargetSet::new(vec![optional.clone()]).expect("optional target set"),
        vec![RadrootsTransportTargetReceipt::new(
            optional.clone(),
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
        )],
    )
    .expect("optional receipt");
    assert_eq!(
        optional_only
            .is_satisfied_by(&policy)
            .expect_err("unrequested required target"),
        RadrootsTransportError::RequiredTargetNotRequested
    );

    let required_delivered = RadrootsTransportDeliveryReceipt::new(
        "required-target",
        RadrootsTransportTargetSet::new(vec![optional.clone(), required.clone()])
            .expect("required target set"),
        vec![
            RadrootsTransportTargetReceipt::new(
                optional,
                RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Rejected),
            ),
            RadrootsTransportTargetReceipt::new(
                required.clone(),
                RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
            ),
        ],
    )
    .expect("required receipt");
    assert!(
        required_delivered
            .is_satisfied_by(&policy)
            .expect("required target accepted")
    );
    assert_eq!(
        RadrootsTransportSatisfactionPolicy::required_targets(
            RadrootsTransportSatisfactionClass::Accepted,
            Vec::new(),
        )
        .expect_err("empty required targets"),
        RadrootsTransportError::EmptyRequiredTargetSet
    );
    assert_eq!(
        RadrootsTransportSatisfactionPolicy::required_targets(
            RadrootsTransportSatisfactionClass::Accepted,
            vec![
                required.fingerprint().clone(),
                required.fingerprint().clone(),
            ],
        )
        .expect_err("duplicate required target"),
        RadrootsTransportError::DuplicateRequiredTargetFingerprint
    );
}

#[test]
fn neutral_transport_trait_covers_status_delivery_and_fetch() {
    struct MemoryTransport {
        target: RadrootsTransportTarget,
    }

    impl RadrootsTransport for MemoryTransport {
        fn transport_kind(&self) -> RadrootsTransportKind {
            RadrootsTransportKind::Local
        }

        fn status<'a>(&'a self) -> RadrootsTransportFuture<'a, RadrootsTransportStatus> {
            Box::pin(async move {
                RadrootsTransportStatus::new(
                    RadrootsTransportKind::Local,
                    true,
                    RadrootsTransportImplementationState::Real,
                    true,
                    "ready",
                )
                .map(|status| {
                    status.with_capabilities(RadrootsTransportCapabilities::deliver_and_fetch())
                })
            })
        }

        fn deliver<'a>(
            &'a self,
            request: RadrootsTransportDeliveryRequest,
        ) -> RadrootsTransportFuture<'a, RadrootsTransportDeliveryReceipt> {
            Box::pin(async move {
                RadrootsTransportDeliveryReceipt::new(
                    request.request_id(),
                    request.target_set().clone(),
                    vec![RadrootsTransportTargetReceipt::new(
                        self.target.clone(),
                        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Delivered),
                    )],
                )
            })
        }

        fn fetch<'a>(
            &'a self,
            request: RadrootsTransportFetchRequest,
        ) -> RadrootsTransportFuture<'a, RadrootsTransportFetchReceipt> {
            Box::pin(async move {
                RadrootsTransportFetchReceipt::for_request(
                    &request,
                    vec![RadrootsTransportTargetReceipt::new(
                        self.target.clone(),
                        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Seen),
                    )],
                    1,
                )
            })
        }
    }

    let target = RadrootsTransportTarget::local("local:memory").expect("local target");
    let target_set = RadrootsTransportTargetSet::new(vec![target.clone()]).expect("target set");
    let transport = MemoryTransport { target };
    assert_eq!(transport.transport_kind(), RadrootsTransportKind::Local);
    let status = futures::executor::block_on(transport.status()).expect("status");
    assert_eq!(status.kind(), &RadrootsTransportKind::Local);
    assert_eq!(
        status.capabilities(),
        &RadrootsTransportCapabilities::deliver_and_fetch()
    );
    let delivery = futures::executor::block_on(
        transport.deliver(
            RadrootsTransportDeliveryRequest::new(
                "deliver-1",
                opaque_payload(),
                target_set.clone(),
                RadrootsTransportSatisfactionPolicy::all_delivered(),
            )
            .expect("delivery request"),
        ),
    )
    .expect("deliver");
    assert_eq!(
        delivery.target_receipts()[0].outcome().kind(),
        RadrootsTransportOutcomeKind::Delivered
    );
    let fetch =
        futures::executor::block_on(transport.fetch(
            RadrootsTransportFetchRequest::new("fetch-1", target_set).expect("fetch request"),
        ))
        .expect("fetch");
    assert_eq!(fetch.fetched_count(), 1);
    assert_eq!(
        fetch.target_receipts()[0].outcome().kind(),
        RadrootsTransportOutcomeKind::Seen
    );
}

#[test]
fn delivery_contract_covers_every_policy_and_receipt_path() {
    let one = RadrootsTransportTarget::nostr_relay("wss://one.example").expect("one");
    let two = RadrootsTransportTarget::nostr_relay("wss://two.example").expect("two");
    let request = RadrootsTransportDeliveryRequest::new(
        "delivery",
        opaque_payload(),
        RadrootsTransportTargetSet::new(vec![one.clone(), two.clone()]).expect("target set"),
        RadrootsTransportSatisfactionPolicy::all_accepted(),
    )
    .expect("delivery request")
    .try_with_now_ms(42)
    .expect("delivery timestamp");
    assert_eq!(request.now_ms(), 42);

    for (policy, class) in [
        (
            RadrootsTransportSatisfactionPolicy::any_delivered(),
            RadrootsTransportSatisfactionClass::Delivered,
        ),
        (
            RadrootsTransportSatisfactionPolicy::quorum_durable_or_observed(2)
                .expect("valid quorum"),
            RadrootsTransportSatisfactionClass::DurableOrObserved,
        ),
    ] {
        assert_eq!(policy.target_satisfaction_class(), Some(class));
    }
    for policy in [
        RadrootsTransportSatisfactionPolicy::no_wait(),
        RadrootsTransportSatisfactionPolicy::any_accepted(),
        RadrootsTransportSatisfactionPolicy::all_accepted(),
        RadrootsTransportSatisfactionPolicy::quorum_accepted(1).expect("valid quorum"),
    ] {
        assert!(policy.required_target_fingerprints().is_none());
    }

    let required = RadrootsTransportSatisfactionPolicy::required_targets(
        RadrootsTransportSatisfactionClass::Accepted,
        vec![one.fingerprint().clone(), two.fingerprint().clone()],
    )
    .expect("required policy");
    assert_eq!(
        required
            .required_target_count(1)
            .expect_err("required set exceeds total"),
        RadrootsTransportError::InvalidSatisfactionPolicy
    );

    let accepted = RadrootsTransportTargetReceipt::new(
        one.clone(),
        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
    );
    let rejected = RadrootsTransportTargetReceipt::new(
        two.clone(),
        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Rejected),
    );
    let receipt = RadrootsTransportDeliveryReceipt::new(
        "delivery",
        RadrootsTransportTargetSet::new(vec![one.clone(), two]).expect("receipt target set"),
        vec![accepted, rejected],
    )
    .expect("delivery receipt");
    assert!(
        receipt
            .is_satisfied_by(&RadrootsTransportSatisfactionPolicy::no_wait())
            .expect("no wait")
    );
    assert!(
        receipt
            .is_satisfied_by(&RadrootsTransportSatisfactionPolicy::any_accepted())
            .expect("any")
    );
    assert!(
        !receipt
            .is_satisfied_by(&RadrootsTransportSatisfactionPolicy::all_accepted())
            .expect("all")
    );
    let quorum = RadrootsTransportSatisfactionPolicy::quorum_accepted(1).expect("valid quorum");
    assert!(receipt.is_satisfied_by(&quorum).expect("quorum"));
    assert!(!receipt.is_satisfied_by(&required).expect("required"));

    assert_eq!(
        RadrootsTransportSatisfactionPolicy::required_targets(
            RadrootsTransportSatisfactionClass::Accepted,
            Vec::new(),
        )
        .expect_err("empty required set"),
        RadrootsTransportError::EmptyRequiredTargetSet
    );

    assert_eq!(
        RadrootsTransportSatisfactionPolicy::required_targets(
            RadrootsTransportSatisfactionClass::Accepted,
            vec![one.fingerprint().clone(), one.fingerprint().clone()],
        )
        .expect_err("duplicate required set"),
        RadrootsTransportError::DuplicateRequiredTargetFingerprint
    );
}

#[test]
fn delivery_requests_and_receipts_reject_forged_identity_and_cardinality() {
    let first = RadrootsTransportTarget::nostr_relay("wss://one.example").expect("first");
    let second = RadrootsTransportTarget::nostr_relay("wss://two.example").expect("second");
    let unexpected =
        RadrootsTransportTarget::nostr_relay("wss://unexpected.example").expect("unexpected");
    let targets =
        RadrootsTransportTargetSet::new(vec![first.clone(), second.clone()]).expect("target set");
    let payload = opaque_payload();

    assert_eq!(
        RadrootsTransportDeliveryRequest::new(
            "",
            payload.clone(),
            targets.clone(),
            RadrootsTransportSatisfactionPolicy::all_accepted(),
        )
        .expect_err("empty request id"),
        RadrootsTransportError::EmptyDeliveryRequestId
    );
    for request_id in [" request".to_owned(), "request\n".to_owned()] {
        assert_eq!(
            RadrootsTransportDeliveryRequest::new(
                request_id,
                payload.clone(),
                targets.clone(),
                RadrootsTransportSatisfactionPolicy::all_accepted(),
            )
            .expect_err("invalid request id"),
            RadrootsTransportError::InvalidDeliveryRequestId
        );
    }

    let unrequested_policy = RadrootsTransportSatisfactionPolicy::required_targets(
        RadrootsTransportSatisfactionClass::Accepted,
        vec![unexpected.fingerprint().clone()],
    )
    .expect("required policy");
    assert_eq!(
        RadrootsTransportDeliveryRequest::new(
            "request",
            payload.clone(),
            targets.clone(),
            unrequested_policy,
        )
        .expect_err("unrequested required target"),
        RadrootsTransportError::RequiredTargetNotRequested
    );

    let request = RadrootsTransportDeliveryRequest::new(
        "request",
        payload,
        targets.clone(),
        RadrootsTransportSatisfactionPolicy::all_accepted(),
    )
    .expect("request");
    assert_eq!(
        request
            .clone()
            .try_with_now_ms(-1)
            .expect_err("negative timestamp"),
        RadrootsTransportError::InvalidDeliveryTimestamp
    );

    let accepted_first = RadrootsTransportTargetReceipt::new(
        first.clone(),
        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
    );
    let accepted_second = RadrootsTransportTargetReceipt::new(
        second.clone(),
        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
    );
    assert_eq!(
        RadrootsTransportDeliveryReceipt::new(
            "request",
            targets.clone(),
            vec![accepted_first.clone()],
        )
        .expect_err("missing receipt"),
        RadrootsTransportError::MissingDeliveryTargetReceipt
    );
    assert_eq!(
        RadrootsTransportDeliveryReceipt::new(
            "request",
            targets.clone(),
            vec![accepted_first.clone(), accepted_first.clone()],
        )
        .expect_err("duplicate receipt"),
        RadrootsTransportError::DuplicateDeliveryTargetReceipt
    );
    assert_eq!(
        RadrootsTransportDeliveryReceipt::new(
            "request",
            targets.clone(),
            vec![
                accepted_first.clone(),
                RadrootsTransportTargetReceipt::new(
                    unexpected.clone(),
                    RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
                ),
            ],
        )
        .expect_err("unexpected receipt"),
        RadrootsTransportError::UnexpectedDeliveryTargetReceipt
    );
    assert_eq!(
        RadrootsTransportTargetReceipt::skipped(
            second.clone(),
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
        )
        .expect_err("skipped accepted outcome"),
        RadrootsTransportError::DeliveryTargetReceiptAttemptMismatch
    );

    let receipt = RadrootsTransportDeliveryReceipt::new(
        "request",
        targets.clone(),
        vec![accepted_second, accepted_first],
    )
    .expect("canonical receipt");
    assert_eq!(receipt.target_receipts()[0].target(), &first);
    assert_eq!(receipt.target_receipts()[1].target(), &second);
    receipt
        .validate_for_request(&request)
        .expect("matching request");

    let wrong_id = RadrootsTransportDeliveryReceipt::new(
        "other-request",
        targets,
        receipt.target_receipts().to_vec(),
    )
    .expect("wrong id receipt");
    assert_eq!(
        wrong_id
            .validate_for_request(&request)
            .expect_err("request id mismatch"),
        RadrootsTransportError::DeliveryReceiptRequestIdMismatch
    );
    let wrong_targets = RadrootsTransportDeliveryReceipt::new(
        "request",
        RadrootsTransportTargetSet::new(vec![unexpected.clone()]).expect("unexpected target set"),
        vec![RadrootsTransportTargetReceipt::new(
            unexpected,
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
        )],
    )
    .expect("wrong target receipt");
    assert_eq!(
        wrong_targets
            .validate_for_request(&request)
            .expect_err("target set mismatch"),
        RadrootsTransportError::DeliveryReceiptTargetSetMismatch
    );
}

#[test]
#[cfg(feature = "serde")]
fn delivery_request_and_receipt_deserialization_revalidates_invariants() {
    let target = RadrootsTransportTarget::nostr_relay("wss://relay.example").expect("target");
    let request = RadrootsTransportDeliveryRequest::new(
        "request",
        opaque_payload(),
        RadrootsTransportTargetSet::new(vec![target.clone()]).expect("target set"),
        RadrootsTransportSatisfactionPolicy::all_accepted(),
    )
    .expect("request");
    let mut request_wire = serde_json::to_value(&request).expect("request wire");
    request_wire["now_ms"] = Value::from(-1);
    assert!(serde_json::from_value::<RadrootsTransportDeliveryRequest>(request_wire).is_err());

    let receipt = RadrootsTransportDeliveryReceipt::for_request(
        &request,
        vec![RadrootsTransportTargetReceipt::new(
            target,
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
        )],
    )
    .expect("receipt");
    let mut receipt_wire = serde_json::to_value(&receipt).expect("receipt wire");
    receipt_wire["target_receipts"][0]["status"] = Value::String("Pending".to_owned());
    assert!(serde_json::from_value::<RadrootsTransportDeliveryReceipt>(receipt_wire).is_err());
    let mut attempt_wire = serde_json::to_value(&receipt).expect("receipt wire");
    attempt_wire["target_receipts"][0]["attempted"] = Value::Bool(false);
    assert!(serde_json::from_value::<RadrootsTransportDeliveryReceipt>(attempt_wire).is_err());
    let mut direct_target_receipt_wire =
        serde_json::to_value(&receipt.target_receipts()[0]).expect("target receipt wire");
    direct_target_receipt_wire["status"] = Value::String("Pending".to_owned());
    assert!(
        serde_json::from_value::<RadrootsTransportTargetReceipt>(direct_target_receipt_wire)
            .is_err()
    );
    let mut direct_attempt_wire =
        serde_json::to_value(&receipt.target_receipts()[0]).expect("target receipt wire");
    direct_attempt_wire["attempted"] = Value::Bool(false);
    assert!(serde_json::from_value::<RadrootsTransportTargetReceipt>(direct_attempt_wire).is_err());

    let exact_outcome_wire = serde_json::json!({
        "kind": "Accepted",
        "status": "Accepted",
        "code": "c".repeat(RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES),
        "message": "m".repeat(RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES),
    });
    let exact_outcome_json =
        serde_json::to_string(&exact_outcome_wire).expect("exact outcome JSON");
    serde_json::from_str::<RadrootsTransportOutcome>(&exact_outcome_json)
        .expect("decode exact outcome wire");
    for (field, value, expected_limit) in [
        (
            "code",
            "c".repeat(RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES + 1),
            "transport_outcome_code",
        ),
        (
            "message",
            "m".repeat(RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES + 1),
            "transport_outcome_message",
        ),
    ] {
        let mut one_over = exact_outcome_wire.clone();
        one_over[field] = Value::String(value);
        let encoded = serde_json::to_string(&one_over).expect("one-over outcome JSON");
        assert!(
            serde_json::from_str::<RadrootsTransportOutcome>(&encoded)
                .expect_err("reject one-over outcome wire")
                .to_string()
                .contains(expected_limit)
        );
    }
    let mut unknown_outcome = exact_outcome_wire;
    unknown_outcome["unknown"] = Value::Bool(true);
    assert!(serde_json::from_value::<RadrootsTransportOutcome>(unknown_outcome).is_err());

    let mut exact_request_id_wire = serde_json::to_value(&receipt).expect("receipt wire");
    exact_request_id_wire["request_id"] =
        Value::String("r".repeat(RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES));
    serde_json::from_value::<RadrootsTransportDeliveryReceipt>(exact_request_id_wire)
        .expect("decode exact receipt request id");
    let mut one_over_request_id_wire = serde_json::to_value(&receipt).expect("receipt wire");
    one_over_request_id_wire["request_id"] =
        Value::String("r".repeat(RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES + 1));
    assert!(
        serde_json::from_value::<RadrootsTransportDeliveryReceipt>(one_over_request_id_wire)
            .expect_err("reject one-over receipt request id")
            .to_string()
            .contains("delivery_request_id")
    );

    let mut one_over_receipts_wire = serde_json::to_value(&receipt).expect("receipt wire");
    let receipt_item = one_over_receipts_wire["target_receipts"][0].clone();
    one_over_receipts_wire["target_receipts"] =
        Value::Array(vec![receipt_item; RADROOTS_TRANSPORT_TARGET_MAX_COUNT + 1]);
    assert!(
        serde_json::from_value::<RadrootsTransportDeliveryReceipt>(one_over_receipts_wire)
            .expect_err("reject one-over receipt collection before identity validation")
            .to_string()
            .contains("delivery_target_receipt_count")
    );

    let mut payload_wire = serde_json::to_value(opaque_payload()).expect("payload wire");
    payload_wire["OpaqueBytes"]["digest"] = Value::String("0".repeat(64));
    assert!(serde_json::from_value::<RadrootsTransportPayload>(payload_wire).is_err());

    let first = RadrootsTransportTarget::nostr_relay("wss://one.example").expect("first target");
    let second = RadrootsTransportTarget::nostr_relay("wss://two.example").expect("second target");
    let mut reversed = vec![first.fingerprint().clone(), second.fingerprint().clone()];
    reversed.sort();
    reversed.reverse();
    let required_wire = serde_json::json!({
        "RequiredTargets": {
            "class": "Accepted",
            "targets": reversed,
        }
    });
    let decoded_required =
        serde_json::from_value::<RadrootsTransportSatisfactionPolicy>(required_wire)
            .expect("decode required policy");
    let required_targets = decoded_required
        .required_target_fingerprints()
        .expect("required fingerprints");
    assert!(required_targets.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(
        serde_json::from_value::<RadrootsTransportSatisfactionPolicy>(serde_json::json!({
            "RequiredTargets": {
                "class": "Accepted",
                "targets": [],
            }
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<RadrootsTransportSatisfactionPolicy>(serde_json::json!({
            "Quorum": {
                "class": "Accepted",
                "threshold": 0,
            }
        }))
        .is_err()
    );
    let exact_required = (0..RADROOTS_TRANSPORT_TARGET_MAX_COUNT)
        .map(|index| format!("{index:064x}"))
        .collect::<Vec<_>>();
    let exact_required_wire = serde_json::json!({
        "RequiredTargets": {
            "class": "Accepted",
            "targets": exact_required,
        }
    });
    assert_eq!(
        serde_json::from_value::<RadrootsTransportSatisfactionPolicy>(exact_required_wire)
            .expect("decode exact required-target policy")
            .required_target_fingerprints()
            .expect("required targets")
            .len(),
        RADROOTS_TRANSPORT_TARGET_MAX_COUNT
    );
    let one_over_required = (0..=RADROOTS_TRANSPORT_TARGET_MAX_COUNT)
        .map(|index| format!("{index:064x}"))
        .collect::<Vec<_>>();
    let one_over_required_wire = serde_json::json!({
        "RequiredTargets": {
            "class": "Accepted",
            "targets": one_over_required,
        }
    });
    assert!(
        serde_json::from_value::<RadrootsTransportSatisfactionPolicy>(one_over_required_wire)
            .expect_err("reject one-over required-target wire")
            .to_string()
            .contains("required_target_count")
    );
    assert!(
        serde_json::from_value::<RadrootsTransportSatisfactionPolicy>(serde_json::json!({
            "Any": {
                "class": "Accepted",
                "unknown": true,
            }
        }))
        .is_err()
    );
}

#[test]
fn payload_contract_covers_all_validation_boundaries() {
    let signed = RadrootsTransportPayload::unchecked_signed_event_json("a".repeat(64), "{}")
        .expect("signed");
    let mesh = RadrootsTransportPayload::mesh_frame_cbor("mesh", [1]).expect("mesh");
    let opaque = RadrootsTransportPayload::opaque_bytes("label", [2]).expect("opaque");
    assert_eq!(signed.payload_kind(), "signed_event_json");
    assert_eq!(mesh.payload_kind(), "mesh_frame_cbor");
    assert_eq!(opaque.payload_kind(), "opaque_bytes");

    for invalid_id in ["a".repeat(63), "g".repeat(64)] {
        assert_eq!(
            RadrootsTransportPayload::unchecked_signed_event_json(invalid_id, "{}")
                .expect_err("invalid event id"),
            RadrootsTransportError::InvalidPayloadId
        );
    }
    assert_eq!(
        RadrootsTransportPayload::mesh_frame_cbor("", [1]).expect_err("empty message id"),
        RadrootsTransportError::EmptyPayloadId
    );
    for invalid_id in [" mesh", "mesh/one", "mesh\n"] {
        assert_eq!(
            RadrootsTransportPayload::mesh_frame_cbor(invalid_id, [1])
                .expect_err("invalid token id"),
            RadrootsTransportError::InvalidPayloadId
        );
    }
    assert_eq!(
        RadrootsTransportPayload::opaque_bytes(" ", [1]).expect_err("empty label"),
        RadrootsTransportError::EmptyPayloadLabel
    );
    assert_eq!(
        RadrootsTransportPayload::opaque_bytes("label", []).expect_err("empty opaque bytes"),
        RadrootsTransportError::EmptyPayloadBytes
    );
    assert_eq!(
        RadrootsTransportPayload::unchecked_signed_event_json("a".repeat(64), "")
            .expect_err("empty raw json"),
        RadrootsTransportError::EmptyPayloadBytes
    );
    for invalid_json in [" {}", "{} ", "{\n}", "[]", "{", "}"] {
        assert_eq!(
            RadrootsTransportPayload::unchecked_signed_event_json("a".repeat(64), invalid_json)
                .expect_err("invalid raw json"),
            RadrootsTransportError::InvalidPayloadBytes
        );
    }
    for invalid_digest in ["f".repeat(63), "g".repeat(64)] {
        assert_eq!(
            RadrootsTransportPayload::opaque_bytes_with_digest("label", [1], invalid_digest)
                .expect_err("invalid digest"),
            RadrootsTransportError::InvalidPayloadDigest
        );
    }

    assert_eq!(
        RadrootsTransportPayload::unchecked_signed_event_json_with_digest(
            "bad",
            "{}",
            "f".repeat(64),
        )
        .expect_err("invalid signed event before digest"),
        RadrootsTransportError::InvalidPayloadId
    );
    assert_eq!(
        RadrootsTransportPayload::unchecked_signed_event_json_with_digest(
            "a".repeat(64),
            "{}",
            "bad",
        )
        .expect_err("invalid signed digest"),
        RadrootsTransportError::InvalidPayloadDigest
    );
    assert_eq!(
        RadrootsTransportPayload::mesh_frame_cbor_with_digest("", [1], "f".repeat(64))
            .expect_err("invalid mesh before digest"),
        RadrootsTransportError::EmptyPayloadId
    );
    assert_eq!(
        RadrootsTransportPayload::mesh_frame_cbor_with_digest("mesh", [1], "bad")
            .expect_err("invalid mesh digest"),
        RadrootsTransportError::InvalidPayloadDigest
    );
    assert_eq!(
        RadrootsTransportPayload::opaque_bytes_with_digest("", [1], "f".repeat(64))
            .expect_err("invalid opaque payload before digest"),
        RadrootsTransportError::EmptyPayloadLabel
    );
}

#[test]
fn status_contract_covers_builders_and_availability_defaults() {
    assert_eq!(
        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted)
            .try_with_code("accepted")
            .expect("bounded code")
            .try_with_message("accepted by transport")
            .expect("bounded message")
            .code(),
        Some("accepted")
    );

    assert!(!RadrootsTransportCapabilities::none().can_deliver());
    assert!(RadrootsTransportCapabilities::fetch_only().can_fetch());
    assert_eq!(
        RadrootsTransportCapabilities::reticulum_unavailable(),
        RadrootsTransportCapabilities::none()
    );
    let capabilities = RadrootsTransportCapabilities::deliver_and_fetch()
        .with_discovery(true)
        .with_gateway_forwarding(true)
        .with_receipt_observation(true);
    assert!(capabilities.can_deliver());
    assert!(capabilities.can_fetch());
    assert!(capabilities.can_discover());
    assert!(capabilities.can_forward_gateway());
    assert!(capabilities.can_observe_receipts());

    let unavailable = RadrootsTransportStatus::new(
        RadrootsTransportKind::Reticulum,
        true,
        RadrootsTransportImplementationState::Mock,
        false,
        "unavailable",
    )
    .expect("bounded status")
    .with_capabilities(capabilities.clone())
    .with_maturity(RadrootsTransportCapabilityMaturity::Preview)
    .with_availability(RadrootsTransportCapabilityAvailability::Degraded)
    .try_with_profile_id("reticulum.local")
    .expect("bounded profile id")
    .try_with_endpoint_uri(RADROOTS_RETICULUM_ENDPOINT_URI)
    .expect("bounded endpoint URI");
    assert_eq!(
        unavailable.availability(),
        RadrootsTransportCapabilityAvailability::Degraded
    );
    assert_eq!(
        unavailable.maturity(),
        RadrootsTransportCapabilityMaturity::Preview
    );
    assert_eq!(unavailable.capabilities(), &capabilities);
    assert!(!unavailable.is_usable_for_delivery());

    assert!(!RadrootsTransportDeliveryTargetStatus::Accepted.is_ready_for_attempt());
    assert!(!RadrootsTransportDeliveryTargetStatus::Accepted.is_retryable_failure());
    assert!(RadrootsTransportDeliveryTargetStatus::SkippedPolicyDenied.is_terminal_failure());
    assert!(!RadrootsTransportDeliveryTargetStatus::Accepted.is_terminal_failure());
    assert!(!RadrootsTransportDeliveryTargetStatus::Accepted.is_deferred_until_implemented());
}

#[test]
#[cfg(feature = "serde")]
fn transport_bounds_status_construction_and_wire_are_strict() {
    let exact_message = "m".repeat(RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES);
    let exact_profile = "p".repeat(RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES);
    let exact_endpoint = "e".repeat(RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES);
    let status = RadrootsTransportStatus::new(
        RadrootsTransportKind::Local,
        true,
        RadrootsTransportImplementationState::Real,
        true,
        exact_message,
    )
    .expect("exact status message")
    .try_with_profile_id(exact_profile)
    .expect("exact profile id")
    .try_with_endpoint_uri(exact_endpoint)
    .expect("exact endpoint URI");
    let wire = serde_json::to_value(&status).expect("serialize bounded status");
    assert_eq!(
        serde_json::from_value::<RadrootsTransportStatus>(wire.clone())
            .expect("reload bounded status"),
        status
    );

    assert_eq!(
        RadrootsTransportStatus::new(
            RadrootsTransportKind::Local,
            true,
            RadrootsTransportImplementationState::Real,
            true,
            "m".repeat(RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES + 1),
        )
        .expect_err("one-over status message"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "transport_status_message",
            max: RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES + 1,
        }
    );
    assert_eq!(
        RadrootsTransportStatus::new(
            RadrootsTransportKind::Local,
            true,
            RadrootsTransportImplementationState::Real,
            true,
            "ready",
        )
        .expect("status")
        .try_with_profile_id("p".repeat(RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES + 1))
        .expect_err("one-over profile id"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "transport_status_profile_id",
            max: RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES + 1,
        }
    );
    assert_eq!(
        RadrootsTransportStatus::new(
            RadrootsTransportKind::Local,
            true,
            RadrootsTransportImplementationState::Real,
            true,
            "ready",
        )
        .expect("status")
        .try_with_endpoint_uri("e".repeat(RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES + 1))
        .expect_err("one-over endpoint URI"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "transport_status_endpoint_uri",
            max: RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES + 1,
        }
    );

    for field in ["message", "profile_id", "endpoint_uri"] {
        let mut oversized = wire.clone();
        let max = match field {
            "message" => RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES,
            "profile_id" => RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES,
            "endpoint_uri" => RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES,
            _ => unreachable!(),
        };
        oversized[field] = Value::String("x".repeat(max + 1));
        assert!(serde_json::from_value::<RadrootsTransportStatus>(oversized).is_err());
    }
    let mut unknown = wire;
    unknown["unexpected"] = Value::Bool(true);
    assert!(serde_json::from_value::<RadrootsTransportStatus>(unknown).is_err());

    let capabilities = serde_json::to_value(RadrootsTransportCapabilities::none())
        .expect("serialize capabilities");
    let mut unknown_capability = capabilities;
    unknown_capability["unexpected"] = Value::Bool(true);
    assert!(serde_json::from_value::<RadrootsTransportCapabilities>(unknown_capability).is_err());
}

#[test]
#[cfg(feature = "serde")]
fn transport_kind_deserializer_rejects_non_string_values() {
    assert!(serde_json::from_str::<RadrootsTransportKind>("1").is_err());
    assert!(serde_json::from_str::<RadrootsTransportKind>("\"NOSTR\"").is_err());
}

#[test]
fn reticulum_destination_rejects_wrong_kind() {
    let local = RadrootsTransportTarget::local("local:memory").expect("local target");
    assert_eq!(
        ReticulumDestinationV1::from_target(&local).expect_err("wrong kind"),
        RadrootsTransportError::InvalidTargetUri
    );
}

#[test]
#[cfg(feature = "serde")]
fn transport_target_deserialization_rejects_forged_and_noncanonical_identity() {
    let target = RadrootsTransportTarget::reticulum().expect("Reticulum target");
    let canonical = serde_json::to_value(&target).expect("serialize target");
    assert_eq!(
        serde_json::from_value::<RadrootsTransportTarget>(canonical.clone())
            .expect("deserialize canonical target"),
        target
    );

    for (field, forged) in [
        ("uri", Value::String("reticulum:other".to_owned())),
        ("scope", Value::Null),
        ("fingerprint", Value::String("0".repeat(64))),
        (
            "fingerprint",
            Value::String(target.fingerprint().as_str().to_ascii_uppercase()),
        ),
    ] {
        let mut wire = canonical.clone();
        wire.as_object_mut()
            .expect("target object")
            .insert(field.to_owned(), forged);
        assert!(
            serde_json::from_value::<RadrootsTransportTarget>(wire).is_err(),
            "forged {field} must fail"
        );
    }

    let labeled = RadrootsTransportTarget::nostr_relay_with_metadata(
        "wss://relay.example",
        None,
        Some(RadrootsTransportTargetLabel::parse("Relay").expect("label")),
    )
    .expect("labeled target");
    let mut noncanonical_label = serde_json::to_value(&labeled).expect("serialize labeled target");
    noncanonical_label
        .as_object_mut()
        .expect("target object")
        .insert("label".to_owned(), Value::String(" Relay ".to_owned()));
    assert!(serde_json::from_value::<RadrootsTransportTarget>(noncanonical_label).is_err());

    let mut unknown_field = canonical;
    unknown_field
        .as_object_mut()
        .expect("target object")
        .insert("unexpected".to_owned(), Value::Bool(true));
    assert!(serde_json::from_value::<RadrootsTransportTarget>(unknown_field).is_err());
}

#[test]
#[cfg(feature = "serde")]
fn transport_target_set_deserialization_revalidates_nonempty_unique_targets() {
    let target = RadrootsTransportTarget::reticulum().expect("Reticulum target");
    let set = RadrootsTransportTargetSet::new(vec![target.clone()]).expect("target set");
    let canonical = serde_json::to_value(&set).expect("serialize target set");
    assert_eq!(
        serde_json::from_value::<RadrootsTransportTargetSet>(canonical)
            .expect("deserialize canonical target set"),
        set
    );
    assert!(
        serde_json::from_value::<RadrootsTransportTargetSet>(serde_json::json!({ "targets": [] }))
            .is_err()
    );
    assert!(
        serde_json::from_value::<RadrootsTransportTargetSet>(
            serde_json::json!({ "targets": [target.clone(), target.clone()] })
        )
        .is_err()
    );
    let one_over = vec![target; RADROOTS_TRANSPORT_TARGET_MAX_COUNT + 1];
    assert!(
        serde_json::from_value::<RadrootsTransportTargetSet>(
            serde_json::json!({ "targets": one_over })
        )
        .expect_err("reject one-over target set before duplicate validation")
        .to_string()
        .contains("target_count")
    );
}

#[test]
#[cfg(feature = "serde")]
fn transport_bounds_target_wire_strings_fail_before_canonical_validation() {
    let target = RadrootsTransportTarget::nostr_relay_with_metadata(
        "wss://relay.example",
        Some(RadrootsTransportMeshScopeId::parse("scope").expect("scope")),
        Some(RadrootsTransportTargetLabel::parse("label").expect("label")),
    )
    .expect("target");
    let canonical = serde_json::to_value(&target).expect("target wire");
    for (field, value, expected_limit) in [
        (
            "uri",
            "u".repeat(RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES + 1),
            "target_uri",
        ),
        (
            "scope",
            "s".repeat(RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES + 1),
            "target_scope",
        ),
        (
            "label",
            "l".repeat(RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES + 1),
            "target_label",
        ),
        (
            "fingerprint",
            "f".repeat(RADROOTS_TRANSPORT_TARGET_FINGERPRINT_BYTES + 1),
            "target_fingerprint",
        ),
    ] {
        let mut one_over = canonical.clone();
        one_over[field] = Value::String(value);
        let encoded = serde_json::to_string(&one_over).expect("one-over target JSON");
        assert!(
            serde_json::from_str::<RadrootsTransportTarget>(&encoded)
                .expect_err("reject one-over target wire")
                .to_string()
                .contains(expected_limit),
            "{field}"
        );
    }

    for (encoded, expected_limit) in [
        (
            serde_json::to_string(&"u".repeat(RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES + 1))
                .expect("URI JSON"),
            "target_uri",
        ),
        (
            serde_json::to_string(&"s".repeat(RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES + 1))
                .expect("scope JSON"),
            "target_scope",
        ),
        (
            serde_json::to_string(&"l".repeat(RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES + 1))
                .expect("label JSON"),
            "target_label",
        ),
        (
            serde_json::to_string(&"f".repeat(RADROOTS_TRANSPORT_TARGET_FINGERPRINT_BYTES + 1))
                .expect("fingerprint JSON"),
            "target_fingerprint",
        ),
    ] {
        let error = match expected_limit {
            "target_uri" => serde_json::from_str::<RadrootsTransportTargetUri>(&encoded)
                .expect_err("reject URI")
                .to_string(),
            "target_scope" => serde_json::from_str::<RadrootsTransportMeshScopeId>(&encoded)
                .expect_err("reject scope")
                .to_string(),
            "target_label" => serde_json::from_str::<RadrootsTransportTargetLabel>(&encoded)
                .expect_err("reject label")
                .to_string(),
            "target_fingerprint" => {
                serde_json::from_str::<RadrootsTransportTargetFingerprint>(&encoded)
                    .expect_err("reject fingerprint")
                    .to_string()
            }
            _ => unreachable!("closed target wire field"),
        };
        assert!(error.contains(expected_limit));
    }
}

#[test]
fn target_contract_covers_parser_and_authority_boundaries() {
    let scope = RadrootsTransportMeshScopeId::parse("farm_1.alpha-beta").expect("scope");
    assert_eq!(scope.as_str(), "farm_1.alpha-beta");
    assert_eq!(scope.to_string(), "farm_1.alpha-beta");
    for invalid_scope in [" scope", "scope ", "scope/path", "scope\n"] {
        assert_eq!(
            RadrootsTransportMeshScopeId::parse(invalid_scope).expect_err("invalid scope"),
            RadrootsTransportError::InvalidTargetScope
        );
    }

    let label = RadrootsTransportTargetLabel::parse("Relay One").expect("label");
    assert_eq!(label.as_str(), "Relay One");
    assert_eq!(label.to_string(), "Relay One");
    assert_eq!(
        RadrootsTransportTargetLabel::parse(" Relay One ").expect_err("noncanonical label"),
        RadrootsTransportError::InvalidTargetLabel
    );
    assert_eq!(
        RadrootsTransportTargetLabel::parse("\u{7f}").expect_err("control label"),
        RadrootsTransportError::InvalidTargetLabel
    );

    for invalid in [
        "",
        " wss://relay.example",
        "wss://relay example",
        "wss://relay\u{7f}.example",
        "relay.example",
        "ftp://relay.example",
        "wss://[::1",
        "wss://[]",
        "wss://[::1]suffix",
        "wss://[[::1]]",
        "wss://relay[.example",
        "wss://.relay.example",
        "wss://relay..example",
        "wss://relay.example.",
        "wss://relay_example",
        "wss://-relay.example",
        "wss://relay-.example",
        "wss://xn--fa-hia.example",
        "wss://example.999",
        "wss://example.0x1",
        "wss://%65xample.com",
        "wss://127.1",
        "wss://2130706433",
        "wss://0x7f.1",
        "wss://01.2.3.4",
        "wss://256.1.1.1",
        "wss://:443",
        "wss://relay.example:",
        "wss://relay.example:0",
        "wss://relay.example:01",
        "wss://[::1]:",
        "wss://[::1]:bad",
        "wss://[::1]:42949672960",
        "wss://[not-ipv6]",
        "wss://relay.example\\path",
        "wss://relay.example/[raw]",
        "wss://relay.example/%",
        "wss://relay.example/%2",
        "wss://relay.example/%2f",
        "wss://relay.example/%GG",
        "wss://relay.example/a/./b",
        "wss://relay.example/a/../b",
        "wss://relay.example/a/%2E/b",
        "wss://relay.example/a/.%2E/b",
        "ws://[2001:db8::1]",
    ] {
        assert_eq!(
            RadrootsTransportTarget::nostr_relay(invalid).expect_err("invalid relay URI"),
            if invalid.is_empty() {
                RadrootsTransportError::EmptyTargetUri
            } else {
                RadrootsTransportError::InvalidTargetUri
            },
            "{invalid}"
        );
    }

    for (raw, canonical) in [
        ("WSS://[2001:0DB8:0:0:0:0:0:1]", "wss://[2001:db8::1]"),
        ("wss://relay.example:443/", "wss://relay.example"),
        (
            "wss://relay.example/nostr/%2Ffeed",
            "wss://relay.example/nostr/%2Ffeed",
        ),
        ("ws://127.0.0.1", "ws://127.0.0.1"),
    ] {
        assert_eq!(
            RadrootsTransportTarget::nostr_relay(raw)
                .expect("relay URI")
                .uri()
                .as_str(),
            canonical
        );
    }
}

#[test]
fn transport_bounds_targets_enforce_exact_and_one_over_before_set_work() {
    let exact_uri = "a".repeat(RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES);
    assert_eq!(
        RadrootsTransportTargetUri::parse(exact_uri)
            .expect("exact URI")
            .as_str()
            .len(),
        RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES
    );
    assert_eq!(
        RadrootsTransportTargetUri::parse(
            "a".repeat(RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES + 1)
        )
        .expect_err("one-over URI"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "target_uri",
            max: RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES + 1,
        }
    );

    assert_eq!(
        RadrootsTransportMeshScopeId::parse("a".repeat(RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES))
            .expect("exact scope")
            .as_str()
            .len(),
        RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES
    );
    assert_eq!(
        RadrootsTransportMeshScopeId::parse(
            "a".repeat(RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES + 1)
        )
        .expect_err("one-over scope"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "target_scope",
            max: RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES + 1,
        }
    );

    assert_eq!(
        RadrootsTransportTargetLabel::parse("a".repeat(RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES))
            .expect("exact label")
            .as_str()
            .len(),
        RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES
    );
    assert_eq!(
        RadrootsTransportTargetLabel::parse(
            "a".repeat(RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES + 1)
        )
        .expect_err("one-over label"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "target_label",
            max: RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES + 1,
        }
    );

    let exact_targets = (0..RADROOTS_TRANSPORT_TARGET_MAX_COUNT)
        .map(|index| {
            RadrootsTransportTarget::local(format!("local:target-{index}")).expect("bounded target")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        RadrootsTransportTargetSet::new(exact_targets.clone())
            .expect("exact target set")
            .len(),
        RADROOTS_TRANSPORT_TARGET_MAX_COUNT
    );
    let mut one_over_targets = exact_targets;
    one_over_targets
        .push(RadrootsTransportTarget::local("local:target-one-over").expect("one-over target"));
    assert_eq!(
        RadrootsTransportTargetSet::new(one_over_targets).expect_err("one-over target set"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "target_count",
            max: RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
            actual: RADROOTS_TRANSPORT_TARGET_MAX_COUNT + 1,
        }
    );

    let required_targets = (0..RADROOTS_TRANSPORT_TARGET_MAX_COUNT)
        .map(|index| {
            RadrootsTransportTargetFingerprint::parse(format!("{index:064x}"))
                .expect("bounded fingerprint")
        })
        .collect::<Vec<_>>();
    RadrootsTransportSatisfactionPolicy::required_targets(
        RadrootsTransportSatisfactionClass::Accepted,
        required_targets.clone(),
    )
    .expect("exact required-target set");
    let mut one_over_required_targets = required_targets;
    one_over_required_targets.push(
        RadrootsTransportTargetFingerprint::parse(format!(
            "{:064x}",
            RADROOTS_TRANSPORT_TARGET_MAX_COUNT
        ))
        .expect("one-over fingerprint"),
    );
    assert_eq!(
        RadrootsTransportSatisfactionPolicy::required_targets(
            RadrootsTransportSatisfactionClass::Accepted,
            one_over_required_targets,
        )
        .expect_err("one-over required-target set"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "required_target_count",
            max: RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
            actual: RADROOTS_TRANSPORT_TARGET_MAX_COUNT + 1,
        }
    );
}

#[test]
fn transport_bounds_outcomes_and_receipts_enforce_exact_and_one_over() {
    let exact_code = "c".repeat(RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES);
    let exact_message = "m".repeat(RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES);
    let exact_outcome = RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted)
        .try_with_code(exact_code)
        .expect("exact outcome code")
        .try_with_message(exact_message)
        .expect("exact outcome message");
    assert_eq!(
        exact_outcome.code().map(str::len),
        Some(RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES)
    );
    assert_eq!(
        exact_outcome.message().map(str::len),
        Some(RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES)
    );
    assert_eq!(
        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted)
            .try_with_code("c".repeat(RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES + 1))
            .expect_err("one-over outcome code"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "transport_outcome_code",
            max: RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES + 1,
        }
    );
    assert_eq!(
        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted)
            .try_with_message("m".repeat(RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES + 1))
            .expect_err("one-over outcome message"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "transport_outcome_message",
            max: RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES + 1,
        }
    );

    let first = RadrootsTransportTarget::local("local:diagnostic-first").expect("first target");
    let second = RadrootsTransportTarget::local("local:diagnostic-second").expect("second target");
    let target_set = RadrootsTransportTargetSet::new(vec![first.clone(), second.clone()])
        .expect("diagnostic target set");
    let half = RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES / 2;
    let exact_receipts = vec![
        RadrootsTransportTargetReceipt::new(
            first.clone(),
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted)
                .try_with_message("a".repeat(half))
                .expect("first diagnostic"),
        ),
        RadrootsTransportTargetReceipt::new(
            second.clone(),
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted)
                .try_with_message("b".repeat(half))
                .expect("second diagnostic"),
        ),
    ];
    RadrootsTransportDeliveryReceipt::new("request", target_set.clone(), exact_receipts)
        .expect("exact aggregate diagnostic budget");
    let one_over_receipts = vec![
        RadrootsTransportTargetReceipt::new(
            first,
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted)
                .try_with_message("a".repeat(half))
                .expect("first diagnostic"),
        ),
        RadrootsTransportTargetReceipt::new(
            second,
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted)
                .try_with_message("b".repeat(half + 1))
                .expect("second diagnostic"),
        ),
    ];
    assert_eq!(
        RadrootsTransportDeliveryReceipt::new("request", target_set, one_over_receipts)
            .expect_err("one-over aggregate diagnostic budget"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "delivery_diagnostic_bytes",
            max: RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES + 1,
        }
    );
}

#[test]
fn transport_bounds_fetch_requests_and_receipts_bind_limits_and_targets() {
    let targets = (0..RADROOTS_TRANSPORT_TARGET_MAX_COUNT)
        .map(|index| {
            RadrootsTransportTarget::local(format!("local:fetch-{index}"))
                .expect("bounded fetch target")
        })
        .collect::<Vec<_>>();
    let target_set = RadrootsTransportTargetSet::new(targets.clone()).expect("target set");
    let request = RadrootsTransportFetchRequest::new(
        "r".repeat(RADROOTS_TRANSPORT_FETCH_REQUEST_ID_MAX_BYTES),
        target_set.clone(),
    )
    .expect("exact fetch request id");
    assert_eq!(
        request.request_id().len(),
        RADROOTS_TRANSPORT_FETCH_REQUEST_ID_MAX_BYTES
    );
    assert_eq!(request.target_set(), &target_set);
    for (request_id, expected) in [
        (String::new(), RadrootsTransportError::EmptyFetchRequestId),
        (
            " fetch".to_owned(),
            RadrootsTransportError::InvalidFetchRequestId,
        ),
        (
            "r".repeat(RADROOTS_TRANSPORT_FETCH_REQUEST_ID_MAX_BYTES + 1),
            RadrootsTransportError::ResourceLimitExceeded {
                field: "fetch_request_id",
                max: RADROOTS_TRANSPORT_FETCH_REQUEST_ID_MAX_BYTES,
                actual: RADROOTS_TRANSPORT_FETCH_REQUEST_ID_MAX_BYTES + 1,
            },
        ),
    ] {
        assert_eq!(
            RadrootsTransportFetchRequest::new(request_id, target_set.clone())
                .expect_err("invalid fetch request id"),
            expected
        );
    }

    let reversed_receipts = targets
        .iter()
        .rev()
        .cloned()
        .map(|target| {
            RadrootsTransportTargetReceipt::new(
                target,
                RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Seen),
            )
        })
        .collect::<Vec<_>>();
    let receipt = RadrootsTransportFetchReceipt::for_request(
        &request,
        reversed_receipts,
        RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT,
    )
    .expect("exact fetch receipt bounds");
    assert_eq!(receipt.request_id(), request.request_id());
    assert_eq!(receipt.target_set(), request.target_set());
    assert_eq!(
        receipt.fetched_count(),
        RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT
    );
    assert!(
        receipt
            .target_receipts()
            .iter()
            .zip(targets.iter())
            .all(|(receipt, target)| receipt.target() == target)
    );
    receipt
        .validate_for_request(&request)
        .expect("bound fetch request");
    let wrong_id_receipt = RadrootsTransportFetchReceipt::new(
        "other-request",
        request.target_set().clone(),
        receipt.target_receipts().to_vec(),
        0,
    )
    .expect("wrong-id receipt");
    assert_eq!(
        wrong_id_receipt
            .validate_for_request(&request)
            .expect_err("fetch request id mismatch"),
        RadrootsTransportError::FetchReceiptRequestIdMismatch
    );
    let other_target = RadrootsTransportTarget::local("local:other-fetch").expect("other target");
    let other_set = RadrootsTransportTargetSet::new(vec![other_target.clone()]).expect("other set");
    let wrong_target_receipt = RadrootsTransportFetchReceipt::new(
        request.request_id(),
        other_set,
        vec![RadrootsTransportTargetReceipt::new(
            other_target.clone(),
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Seen),
        )],
        0,
    )
    .expect("wrong-target receipt");
    assert_eq!(
        wrong_target_receipt
            .validate_for_request(&request)
            .expect_err("fetch target set mismatch"),
        RadrootsTransportError::FetchReceiptTargetSetMismatch
    );

    assert_eq!(
        RadrootsTransportFetchReceipt::for_request(
            &request,
            receipt.target_receipts().to_vec(),
            RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT + 1,
        )
        .expect_err("one-over admitted fetch count"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "fetch_admitted_event_count",
            max: RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT,
            actual: RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT + 1,
        }
    );
    assert_eq!(
        RadrootsTransportFetchReceipt::for_request(
            &request,
            receipt.target_receipts()[..RADROOTS_TRANSPORT_TARGET_MAX_COUNT - 1].to_vec(),
            0,
        )
        .expect_err("missing fetch target receipt"),
        RadrootsTransportError::MissingFetchTargetReceipt
    );
    let mut duplicate_receipts = receipt.target_receipts().to_vec();
    duplicate_receipts[RADROOTS_TRANSPORT_TARGET_MAX_COUNT - 1] = duplicate_receipts[0].clone();
    assert_eq!(
        RadrootsTransportFetchReceipt::for_request(&request, duplicate_receipts, 0)
            .expect_err("duplicate fetch target receipt"),
        RadrootsTransportError::DuplicateFetchTargetReceipt
    );

    assert_eq!(
        RadrootsTransportFetchReceipt::new(
            "fetch",
            RadrootsTransportTargetSet::new(vec![targets[0].clone()]).expect("one target"),
            vec![RadrootsTransportTargetReceipt::new(
                other_target,
                RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Seen),
            )],
            0,
        )
        .expect_err("unexpected fetch target receipt"),
        RadrootsTransportError::UnexpectedFetchTargetReceipt
    );

    let first = RadrootsTransportTarget::local("local:fetch-diagnostic-one").expect("first");
    let second = RadrootsTransportTarget::local("local:fetch-diagnostic-two").expect("second");
    let diagnostic_set =
        RadrootsTransportTargetSet::new(vec![first.clone(), second.clone()]).expect("target set");
    let diagnostic_request =
        RadrootsTransportFetchRequest::new("fetch-diagnostics", diagnostic_set)
            .expect("diagnostic request");
    let half = RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES / 2;
    let diagnostic_receipts = |second_len| {
        vec![
            RadrootsTransportTargetReceipt::new(
                first.clone(),
                RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Seen)
                    .try_with_message("a".repeat(half))
                    .expect("first diagnostic"),
            ),
            RadrootsTransportTargetReceipt::new(
                second.clone(),
                RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Seen)
                    .try_with_message("b".repeat(second_len))
                    .expect("second diagnostic"),
            ),
        ]
    };
    RadrootsTransportFetchReceipt::for_request(&diagnostic_request, diagnostic_receipts(half), 0)
        .expect("exact fetch diagnostic budget");
    assert_eq!(
        RadrootsTransportFetchReceipt::for_request(
            &diagnostic_request,
            diagnostic_receipts(half + 1),
            0,
        )
        .expect_err("one-over fetch diagnostic budget"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "fetch_diagnostic_bytes",
            max: RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES + 1,
        }
    );
}

#[test]
#[cfg(feature = "serde")]
fn transport_bounds_fetch_wire_is_strict_bounded_and_request_bound() {
    let targets = (0..RADROOTS_TRANSPORT_TARGET_MAX_COUNT)
        .map(|index| {
            RadrootsTransportTarget::local(format!("local:fetch-wire-{index}"))
                .expect("fetch target")
        })
        .collect::<Vec<_>>();
    let request = RadrootsTransportFetchRequest::new(
        "r".repeat(RADROOTS_TRANSPORT_FETCH_REQUEST_ID_MAX_BYTES),
        RadrootsTransportTargetSet::new(targets.clone()).expect("target set"),
    )
    .expect("fetch request");
    let receipt = RadrootsTransportFetchReceipt::for_request(
        &request,
        targets
            .into_iter()
            .map(|target| {
                RadrootsTransportTargetReceipt::new(
                    target,
                    RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Seen),
                )
            })
            .collect(),
        RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT,
    )
    .expect("fetch receipt");
    let request_wire = serde_json::to_value(&request).expect("request wire");
    let receipt_wire = serde_json::to_value(&receipt).expect("receipt wire");
    serde_json::from_value::<RadrootsTransportFetchRequest>(request_wire.clone())
        .expect("exact request wire");
    serde_json::from_value::<RadrootsTransportFetchReceipt>(receipt_wire.clone())
        .expect("exact receipt wire");

    let mut one_over_request_id = request_wire.clone();
    one_over_request_id["request_id"] =
        Value::String("r".repeat(RADROOTS_TRANSPORT_FETCH_REQUEST_ID_MAX_BYTES + 1));
    assert!(
        serde_json::from_value::<RadrootsTransportFetchRequest>(one_over_request_id)
            .expect_err("one-over fetch request id")
            .to_string()
            .contains("fetch_request_id")
    );
    let mut unknown_request = request_wire;
    unknown_request["unknown"] = Value::Bool(true);
    assert!(serde_json::from_value::<RadrootsTransportFetchRequest>(unknown_request).is_err());

    let mut one_over_receipts = receipt_wire.clone();
    let extra = one_over_receipts["target_receipts"][0].clone();
    one_over_receipts["target_receipts"]
        .as_array_mut()
        .expect("receipt array")
        .push(extra);
    assert!(
        serde_json::from_value::<RadrootsTransportFetchReceipt>(one_over_receipts)
            .expect_err("one-over fetch target receipt count")
            .to_string()
            .contains("fetch_target_receipt_count")
    );
    let mut one_over_events = receipt_wire.clone();
    one_over_events["fetched_count"] =
        Value::from(RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT + 1);
    assert!(
        serde_json::from_value::<RadrootsTransportFetchReceipt>(one_over_events)
            .expect_err("one-over admitted fetch count")
            .to_string()
            .contains("fetch_admitted_event_count")
    );
    let mut mismatched_target_set = receipt_wire.clone();
    mismatched_target_set["target_set"] = serde_json::json!({
        "targets": [RadrootsTransportTarget::local("local:mismatch").expect("target")],
    });
    assert!(
        serde_json::from_value::<RadrootsTransportFetchReceipt>(mismatched_target_set).is_err()
    );
    let mut unknown_receipt = receipt_wire;
    unknown_receipt["unknown"] = Value::Bool(true);
    assert!(serde_json::from_value::<RadrootsTransportFetchReceipt>(unknown_receipt).is_err());
}

#[test]
fn transport_bounds_payloads_enforce_exact_and_one_over_before_copying() {
    let exact_json = format!(
        "{{{}}}",
        "a".repeat(RADROOTS_TRANSPORT_SIGNED_EVENT_JSON_MAX_BYTES - 2)
    );
    RadrootsTransportPayload::unchecked_signed_event_json("a".repeat(64), &exact_json)
        .expect("exact signed event JSON");
    let one_over_json = format!(
        "{{{}}}",
        "a".repeat(RADROOTS_TRANSPORT_SIGNED_EVENT_JSON_MAX_BYTES - 1)
    );
    assert_eq!(
        RadrootsTransportPayload::unchecked_signed_event_json("a".repeat(64), &one_over_json)
            .expect_err("one-over signed event JSON"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "signed_event_json_bytes",
            max: RADROOTS_TRANSPORT_SIGNED_EVENT_JSON_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_SIGNED_EVENT_JSON_MAX_BYTES + 1,
        }
    );

    RadrootsTransportPayload::mesh_frame_cbor(
        "a".repeat(RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES),
        vec![0; RADROOTS_TRANSPORT_RETICULUM_PAYLOAD_MAX_BYTES],
    )
    .expect("exact mesh payload");
    assert_eq!(
        RadrootsTransportPayload::mesh_frame_cbor(
            "mesh",
            vec![0; RADROOTS_TRANSPORT_RETICULUM_PAYLOAD_MAX_BYTES + 1],
        )
        .expect_err("one-over mesh payload"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "mesh_frame_cbor_bytes",
            max: RADROOTS_TRANSPORT_RETICULUM_PAYLOAD_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_RETICULUM_PAYLOAD_MAX_BYTES + 1,
        }
    );
    assert_eq!(
        RadrootsTransportPayload::mesh_frame_cbor(
            "a".repeat(RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES + 1),
            [1],
        )
        .expect_err("one-over payload id"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "payload_id",
            max: RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES + 1,
        }
    );

    RadrootsTransportPayload::opaque_bytes(
        "a".repeat(RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES),
        vec![0; RADROOTS_TRANSPORT_OPAQUE_PAYLOAD_MAX_BYTES],
    )
    .expect("exact opaque payload");
    assert_eq!(
        RadrootsTransportPayload::opaque_bytes(
            "a".repeat(RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES + 1),
            [1],
        )
        .expect_err("one-over payload label"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "payload_label",
            max: RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES + 1,
        }
    );
    assert_eq!(
        RadrootsTransportPayload::opaque_bytes(
            "label",
            vec![0; RADROOTS_TRANSPORT_OPAQUE_PAYLOAD_MAX_BYTES + 1],
        )
        .expect_err("one-over opaque payload"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "opaque_payload_bytes",
            max: RADROOTS_TRANSPORT_OPAQUE_PAYLOAD_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_OPAQUE_PAYLOAD_MAX_BYTES + 1,
        }
    );

    let target_set = RadrootsTransportTargetSet::new(vec![
        RadrootsTransportTarget::local("local:bounded-request").expect("request target"),
    ])
    .expect("request target set");
    let payload = RadrootsTransportPayload::opaque_bytes("bounded", [1]).expect("request payload");
    RadrootsTransportDeliveryRequest::new(
        "a".repeat(RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES),
        payload.clone(),
        target_set.clone(),
        RadrootsTransportSatisfactionPolicy::no_wait(),
    )
    .expect("exact request id");
    assert_eq!(
        RadrootsTransportDeliveryRequest::new(
            "a".repeat(RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES + 1),
            payload,
            target_set,
            RadrootsTransportSatisfactionPolicy::no_wait(),
        )
        .expect_err("one-over request id"),
        RadrootsTransportError::ResourceLimitExceeded {
            field: "delivery_request_id",
            max: RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES,
            actual: RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES + 1,
        }
    );
}

#[cfg(feature = "serde")]
#[test]
fn transport_bounds_payload_wire_is_strict_and_bounded_for_every_variant() {
    let exact_signed_json = format!(
        "{{{}}}",
        "a".repeat(RADROOTS_TRANSPORT_SIGNED_EVENT_JSON_MAX_BYTES - 2)
    );
    let exact_signed =
        RadrootsTransportPayload::unchecked_signed_event_json("a".repeat(64), &exact_signed_json)
            .expect("exact signed payload");
    assert_eq!(
        serde_json::from_value::<RadrootsTransportPayload>(
            serde_json::to_value(&exact_signed).expect("serialize signed payload")
        )
        .expect("decode exact signed payload"),
        exact_signed
    );

    let one_over_signed_json = format!(
        "{{{}}}",
        "a".repeat(RADROOTS_TRANSPORT_SIGNED_EVENT_JSON_MAX_BYTES - 1)
    );
    let one_over_signed_wire = serde_json::json!({
        "SignedEventJson": {
            "event_id": "a".repeat(64),
            "raw_json": one_over_signed_json,
            "digest": "0".repeat(64),
        }
    });
    assert!(
        serde_json::from_value::<RadrootsTransportPayload>(one_over_signed_wire)
            .expect_err("reject one-over signed wire")
            .to_string()
            .contains("signed_event_json_bytes")
    );

    for exact in [
        RadrootsTransportPayload::mesh_frame_cbor(
            "mesh",
            vec![0; RADROOTS_TRANSPORT_RETICULUM_PAYLOAD_MAX_BYTES],
        )
        .expect("exact mesh payload"),
        RadrootsTransportPayload::opaque_bytes(
            "opaque",
            vec![0; RADROOTS_TRANSPORT_OPAQUE_PAYLOAD_MAX_BYTES],
        )
        .expect("exact opaque payload"),
    ] {
        assert_eq!(
            serde_json::from_value::<RadrootsTransportPayload>(
                serde_json::to_value(&exact).expect("serialize byte payload")
            )
            .expect("decode exact byte payload"),
            exact
        );
    }

    for (variant, field) in [
        ("MeshFrameCbor", "mesh_frame_cbor_bytes"),
        ("OpaqueBytes", "opaque_payload_bytes"),
    ] {
        let id_field = if variant == "MeshFrameCbor" {
            "message_id"
        } else {
            "label"
        };
        let mut body = serde_json::Map::new();
        body.insert(id_field.to_owned(), Value::String("bounded".to_owned()));
        body.insert(
            "bytes".to_owned(),
            Value::Array(
                core::iter::repeat_n(
                    Value::from(0),
                    RADROOTS_TRANSPORT_RETICULUM_PAYLOAD_MAX_BYTES + 1,
                )
                .collect(),
            ),
        );
        body.insert("digest".to_owned(), Value::String("0".repeat(64)));
        let mut wire = serde_json::Map::new();
        wire.insert(variant.to_owned(), Value::Object(body));
        assert!(
            serde_json::from_value::<RadrootsTransportPayload>(Value::Object(wire))
                .expect_err("reject one-over byte wire")
                .to_string()
                .contains(field)
        );
    }

    let mut unknown = serde_json::to_value(opaque_payload()).expect("payload wire");
    unknown["OpaqueBytes"]["unknown"] = Value::Bool(true);
    assert!(serde_json::from_value::<RadrootsTransportPayload>(unknown).is_err());
}

#[test]
fn every_transport_error_has_a_stable_display_message() {
    let remaining = [
        RadrootsTransportError::RequiredTargetNotRequested,
        RadrootsTransportError::EmptyDeliveryRequestId,
        RadrootsTransportError::InvalidDeliveryRequestId,
        RadrootsTransportError::InvalidDeliveryTimestamp,
        RadrootsTransportError::UnexpectedDeliveryTargetReceipt,
        RadrootsTransportError::DuplicateDeliveryTargetReceipt,
        RadrootsTransportError::MissingDeliveryTargetReceipt,
        RadrootsTransportError::DeliveryTargetReceiptStatusMismatch,
        RadrootsTransportError::DeliveryTargetReceiptAttemptMismatch,
        RadrootsTransportError::TransportOutcomeStatusMismatch,
        RadrootsTransportError::DeliveryReceiptRequestIdMismatch,
        RadrootsTransportError::DeliveryReceiptTargetSetMismatch,
        RadrootsTransportError::EmptyFetchRequestId,
        RadrootsTransportError::InvalidFetchRequestId,
        RadrootsTransportError::UnexpectedFetchTargetReceipt,
        RadrootsTransportError::DuplicateFetchTargetReceipt,
        RadrootsTransportError::MissingFetchTargetReceipt,
        RadrootsTransportError::FetchReceiptRequestIdMismatch,
        RadrootsTransportError::FetchReceiptTargetSetMismatch,
        RadrootsTransportError::EmptyPayloadId,
        RadrootsTransportError::InvalidPayloadId,
        RadrootsTransportError::EmptyPayloadLabel,
        RadrootsTransportError::InvalidPayloadLabel,
        RadrootsTransportError::EmptyPayloadBytes,
        RadrootsTransportError::InvalidPayloadBytes,
        RadrootsTransportError::InvalidPayloadDigest,
        RadrootsTransportError::PayloadDigestMismatch,
        RadrootsTransportError::ResourceLimitExceeded {
            field: "fixture",
            max: 1,
            actual: 2,
        },
    ];
    for error in remaining {
        assert!(!error.to_string().is_empty());
    }
}
