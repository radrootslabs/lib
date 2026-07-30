use radroots_transport::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RADROOTS_RETICULUM_SCOPE_ID,
    RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES, RadrootsTransport,
    RadrootsTransportCapabilities, RadrootsTransportCapabilityAvailability,
    RadrootsTransportCapabilityMaturity, RadrootsTransportDeliveryReceipt,
    RadrootsTransportDeliveryRequest, RadrootsTransportDeliveryTargetStatus,
    RadrootsTransportError, RadrootsTransportFetchReceipt, RadrootsTransportFetchRequest,
    RadrootsTransportFuture, RadrootsTransportImplementationState, RadrootsTransportKind,
    RadrootsTransportMeshScopeId, RadrootsTransportOutcome, RadrootsTransportOutcomeKind,
    RadrootsTransportPayload, RadrootsTransportSatisfactionClass,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportStatus, RadrootsTransportTarget,
    RadrootsTransportTargetFingerprint, RadrootsTransportTargetLabel,
    RadrootsTransportTargetReceipt, RadrootsTransportTargetSet, RadrootsTransportTargetUri,
    ReticulumCapabilityReportV1, ReticulumDestinationV1, ReticulumDuplicateFragmentBehaviorV1,
    ReticulumFragmentIntegrityV1, ReticulumFragmentationModeV1, ReticulumGatewaySemanticsV1,
    ReticulumPrivacySemanticsV1, TRANSPORT_ID_MAX_BYTES, TransportId,
};
use serde_json::Value;
use std::borrow::ToOwned;
use std::boxed::Box;
use std::string::ToString;
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
        destination.routing().scope.as_str(),
        RADROOTS_RETICULUM_SCOPE_ID
    );
    assert_eq!(
        destination.routing().gateway,
        ReticulumGatewaySemanticsV1::NoGatewayForwarding
    );
    assert_eq!(
        destination.routing().privacy,
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

    assert!(report.delivery_required);
    assert!(!report.fetch_required);
    assert!(!report.can_deliver);
    assert!(!report.can_fetch);
    assert!(!report.can_discover);
    assert!(!report.can_forward_gateway);
    assert!(!report.can_observe_receipts);
    assert_eq!(
        report.payload_policy.fragment_policy.mode,
        ReticulumFragmentationModeV1::Unsupported
    );
    assert_eq!(report.payload_policy.fragment_policy.max_fragment_count, 1);
    assert_eq!(
        report.payload_policy.fragment_policy.max_reassembled_bytes,
        report.payload_policy.max_payload_bytes
    );
    assert_eq!(
        report
            .payload_policy
            .fragment_policy
            .duplicate_fragment_behavior,
        ReticulumDuplicateFragmentBehaviorV1::Reject
    );
    assert_eq!(
        report.payload_policy.fragment_policy.integrity_verification,
        ReticulumFragmentIntegrityV1::PayloadDigest
    );
}

#[test]
fn transport_id_round_trips_built_ins_and_custom_values() {
    for (raw, expected) in [
        ("local", TransportId::LOCAL),
        ("nostr", TransportId::NOSTR),
        ("reticulum", TransportId::RETICULUM),
        ("radrootsd", TransportId::RADROOTSD),
    ] {
        let parsed = TransportId::parse(raw).expect("built-in transport id");
        assert_eq!(parsed, expected);
        assert_eq!(parsed.as_str(), raw);
        assert_eq!(parsed.to_string(), raw);
    }

    let custom = TransportId::parse("fieldbus-v2").expect("custom transport id");
    assert_eq!(custom.as_str(), "fieldbus-v2");
    let custom_target =
        RadrootsTransportTarget::new(custom, "fieldbus:node-7").expect("custom target");
    assert_eq!(custom_target.kind(), &custom);
    assert_eq!(
        RadrootsTransportKind::Local.canonical_label(),
        "local".to_owned()
    );
}

#[test]
fn transport_id_parser_enforces_canonical_syntax_and_bound() {
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
        RadrootsTransportKind::parse_canonical("radrootsd_proxy")
            .expect_err("underscore separator"),
        RadrootsTransportError::InvalidTransportKind
    );
    assert_eq!(
        RadrootsTransportKind::parse_canonical("fieldbus").expect("custom kind"),
        TransportId::parse("fieldbus").expect("same custom kind")
    );
    assert_eq!(
        RadrootsTransportKind::parse_canonical("").expect_err("empty kind"),
        RadrootsTransportError::EmptyTransportKind
    );
    assert_eq!(
        TransportId::parse("a".repeat(TRANSPORT_ID_MAX_BYTES + 1)).expect_err("overlong kind"),
        RadrootsTransportError::InvalidTransportKind
    );
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
    let two = RadrootsTransportSatisfactionPolicy::quorum_accepted(2);
    let delivered = RadrootsTransportSatisfactionPolicy::quorum_delivered(2);
    let forwarded = RadrootsTransportSatisfactionPolicy::any_forwarded();
    let stored = RadrootsTransportSatisfactionPolicy::all_stored();
    let seen = RadrootsTransportSatisfactionPolicy::quorum_seen(2);
    let durable_or_observed = RadrootsTransportSatisfactionPolicy::any_durable_or_observed();

    assert_eq!(no_wait.required_target_count(0).expect("no wait"), 0);
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
            RadrootsTransportSatisfactionPolicy::quorum_forwarded(2),
            RadrootsTransportSatisfactionClass::Forwarded,
        ),
        (
            RadrootsTransportSatisfactionPolicy::any_stored(),
            RadrootsTransportSatisfactionClass::Stored,
        ),
        (
            RadrootsTransportSatisfactionPolicy::quorum_stored(2),
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
            RadrootsTransportSatisfactionPolicy::quorum_durable_or_observed(2),
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
        RadrootsTransportSatisfactionPolicy::quorum_accepted(0)
            .is_satisfied_by(3, 0)
            .expect_err("zero required targets"),
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
    .with_profile_id("transport.nostr.default")
    .with_endpoint_uri("wss://relay.example");

    assert_eq!(status.kind, RadrootsTransportKind::Nostr);
    assert_eq!(
        status.profile_id.as_deref(),
        Some("transport.nostr.default")
    );
    assert_eq!(status.endpoint_uri.as_deref(), Some("wss://relay.example"));
    assert!(status.configured);
    assert_eq!(
        status.implementation,
        RadrootsTransportImplementationState::Real
    );
    assert!(status.usable_for_delivery);
    assert_eq!(
        status.capabilities,
        RadrootsTransportCapabilities::deliver_only()
    );
    assert_eq!(status.message, "ready");

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
    let target_set = RadrootsTransportTargetSet::new(vec![target]).expect("target set");
    let request = RadrootsTransportDeliveryRequest::new(
        "req-1",
        opaque_payload(),
        target_set,
        RadrootsTransportSatisfactionPolicy::any_accepted(),
    )
    .expect("request");

    let json = serde_json::to_string(&request).expect("serialize request");
    let decoded: RadrootsTransportDeliveryRequest =
        serde_json::from_str(&json).expect("decode request");

    assert_eq!(decoded, request);
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
            RadrootsTransportError::TargetSetTooLarge,
            "transport target set exceeds its item limit",
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
    for invalid in [
        "bad kind",
        "bad:kind",
        "bad/kind",
        "bad\nkind",
        "-fieldbus",
        "fieldbus-",
        "fieldbus--v2",
    ] {
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
        let outcome = RadrootsTransportOutcome::new(kind).with_message("transport detail");
        assert_eq!(kind.as_str(), label);
        assert_eq!(outcome.kind, kind);
        assert_eq!(outcome.status, status);
        for class in classes {
            assert_eq!(
                kind.counts_as_satisfied(class),
                satisfied_classes.contains(&class),
                "{kind:?} / {class:?}"
            );
        }
        assert_eq!(outcome.message.as_deref(), Some("transport detail"));
    }

    let deferred_until_implemented =
        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::DeferredUntilImplemented);
    assert_eq!(
        deferred_until_implemented.status,
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
                Ok(RadrootsTransportStatus::new(
                    RadrootsTransportKind::Local,
                    true,
                    RadrootsTransportImplementationState::Real,
                    true,
                    "ready",
                )
                .with_capabilities(RadrootsTransportCapabilities::deliver_and_fetch()))
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
                Ok(RadrootsTransportFetchReceipt::new(
                    request.request_id,
                    vec![RadrootsTransportTargetReceipt::new(
                        self.target.clone(),
                        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Seen),
                    )],
                    1,
                ))
            })
        }
    }

    let target = RadrootsTransportTarget::local("local:memory").expect("local target");
    let target_set = RadrootsTransportTargetSet::new(vec![target.clone()]).expect("target set");
    let transport = MemoryTransport { target };
    assert_eq!(transport.transport_kind(), RadrootsTransportKind::Local);
    let status = futures::executor::block_on(transport.status()).expect("status");
    assert_eq!(status.kind, RadrootsTransportKind::Local);
    assert_eq!(
        status.capabilities,
        RadrootsTransportCapabilities::deliver_and_fetch()
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
        delivery.target_receipts()[0].outcome.kind,
        RadrootsTransportOutcomeKind::Delivered
    );
    let fetch = futures::executor::block_on(
        transport.fetch(RadrootsTransportFetchRequest::new("fetch-1", target_set)),
    )
    .expect("fetch");
    assert_eq!(fetch.fetched_count, 1);
    assert_eq!(
        fetch.target_receipts[0].outcome.kind,
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
            RadrootsTransportSatisfactionPolicy::quorum_durable_or_observed(2),
            RadrootsTransportSatisfactionClass::DurableOrObserved,
        ),
    ] {
        assert_eq!(policy.target_satisfaction_class(), Some(class));
    }
    for policy in [
        RadrootsTransportSatisfactionPolicy::no_wait(),
        RadrootsTransportSatisfactionPolicy::any_accepted(),
        RadrootsTransportSatisfactionPolicy::all_accepted(),
        RadrootsTransportSatisfactionPolicy::quorum_accepted(1),
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
    assert!(
        receipt
            .is_satisfied_by(&RadrootsTransportSatisfactionPolicy::quorum_accepted(1))
            .expect("quorum")
    );
    assert!(!receipt.is_satisfied_by(&required).expect("required"));

    let invalid_empty = RadrootsTransportSatisfactionPolicy::RequiredTargets {
        class: RadrootsTransportSatisfactionClass::Accepted,
        targets: Vec::new(),
    };
    assert_eq!(
        invalid_empty
            .required_target_count(2)
            .expect_err("empty required set"),
        RadrootsTransportError::EmptyRequiredTargetSet
    );
    assert_eq!(
        receipt
            .is_satisfied_by(&invalid_empty)
            .expect_err("empty required receipt policy"),
        RadrootsTransportError::EmptyRequiredTargetSet
    );

    let duplicate = RadrootsTransportSatisfactionPolicy::RequiredTargets {
        class: RadrootsTransportSatisfactionClass::Accepted,
        targets: vec![one.fingerprint().clone(), one.fingerprint().clone()],
    };
    assert_eq!(
        duplicate
            .required_target_count(2)
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
    for request_id in [
        " request".to_owned(),
        "request\n".to_owned(),
        "x".repeat(RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES + 1),
    ] {
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

    let forged_payload = RadrootsTransportPayload::OpaqueBytes {
        label: "opaque".to_owned(),
        bytes: vec![1],
        digest: "0".repeat(64),
    };
    assert_eq!(
        RadrootsTransportDeliveryRequest::new(
            "request",
            forged_payload,
            targets.clone(),
            RadrootsTransportSatisfactionPolicy::all_accepted(),
        )
        .expect_err("forged payload"),
        RadrootsTransportError::PayloadDigestMismatch
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
    let mut mismatched_status = accepted_second.clone();
    mismatched_status.status = RadrootsTransportDeliveryTargetStatus::Pending;
    assert_eq!(
        RadrootsTransportDeliveryReceipt::new(
            "request",
            targets.clone(),
            vec![accepted_first.clone(), mismatched_status],
        )
        .expect_err("status mismatch"),
        RadrootsTransportError::DeliveryTargetReceiptStatusMismatch
    );
    let mut forged_outcome = RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted);
    forged_outcome.status = RadrootsTransportDeliveryTargetStatus::Pending;
    assert_eq!(
        RadrootsTransportDeliveryReceipt::new(
            "request",
            targets.clone(),
            vec![
                accepted_first.clone(),
                RadrootsTransportTargetReceipt {
                    target: second.clone(),
                    attempted: true,
                    status: RadrootsTransportDeliveryTargetStatus::Pending,
                    outcome: forged_outcome,
                },
            ],
        )
        .expect_err("outcome status mismatch"),
        RadrootsTransportError::TransportOutcomeStatusMismatch
    );
    assert_eq!(
        RadrootsTransportDeliveryReceipt::new(
            "request",
            targets.clone(),
            vec![
                accepted_first.clone(),
                RadrootsTransportTargetReceipt::skipped(
                    second.clone(),
                    RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
                ),
            ],
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
    assert_eq!(receipt.target_receipts()[0].target, first);
    assert_eq!(receipt.target_receipts()[1].target, second);
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
}

#[test]
fn payload_contract_covers_all_validation_boundaries() {
    let signed = RadrootsTransportPayload::unchecked_signed_event_json("a".repeat(64), "{}")
        .expect("signed");
    let mesh = RadrootsTransportPayload::mesh_frame_cbor("mesh", [1]).expect("mesh");
    let opaque = RadrootsTransportPayload::opaque_bytes(" label ", [2]).expect("opaque");
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
            .with_code("accepted")
            .with_message("accepted by transport")
            .code
            .as_deref(),
        Some("accepted")
    );

    assert!(!RadrootsTransportCapabilities::none().deliver);
    assert!(RadrootsTransportCapabilities::fetch_only().fetch);
    assert_eq!(
        RadrootsTransportCapabilities::reticulum_unavailable(),
        RadrootsTransportCapabilities::none()
    );
    let capabilities = RadrootsTransportCapabilities::deliver_and_fetch()
        .with_discovery(true)
        .with_gateway_forwarding(true)
        .with_receipt_observation(true);
    assert!(capabilities.deliver);
    assert!(capabilities.fetch);
    assert!(capabilities.discovery);
    assert!(capabilities.gateway_forwarding);
    assert!(capabilities.receipt_observation);

    let unavailable = RadrootsTransportStatus::new(
        RadrootsTransportKind::Reticulum,
        true,
        RadrootsTransportImplementationState::Mock,
        false,
        "unavailable",
    )
    .with_capabilities(capabilities.clone())
    .with_maturity(RadrootsTransportCapabilityMaturity::Preview)
    .with_availability(RadrootsTransportCapabilityAvailability::Degraded)
    .with_profile_id("reticulum.local")
    .with_endpoint_uri(RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(
        unavailable.availability,
        RadrootsTransportCapabilityAvailability::Degraded
    );
    assert_eq!(
        unavailable.maturity,
        RadrootsTransportCapabilityMaturity::Preview
    );
    assert_eq!(unavailable.capabilities, capabilities);
    assert!(!unavailable.usable_for_delivery);

    assert!(!RadrootsTransportDeliveryTargetStatus::Accepted.is_ready_for_attempt());
    assert!(!RadrootsTransportDeliveryTargetStatus::Accepted.is_retryable_failure());
    assert!(RadrootsTransportDeliveryTargetStatus::SkippedPolicyDenied.is_terminal_failure());
    assert!(!RadrootsTransportDeliveryTargetStatus::Accepted.is_terminal_failure());
    assert!(!RadrootsTransportDeliveryTargetStatus::Accepted.is_deferred_until_implemented());
}

#[test]
#[cfg(feature = "serde")]
fn transport_kind_deserializer_rejects_non_string_values() {
    assert!(serde_json::from_str::<RadrootsTransportKind>("1").is_err());
    assert!(serde_json::from_str::<RadrootsTransportKind>("\"NOSTR\"").is_err());
}

#[test]
#[cfg(feature = "serde")]
fn transport_id_serde_uses_the_protocol_wire_contract() {
    let id = TransportId::parse("future-mesh-v3").expect("future transport id");
    let encoded = serde_json::to_string(&id).expect("serialize transport id");
    assert_eq!(encoded, "\"future-mesh-v3\"");
    assert_eq!(serde_json::from_str::<TransportId>(&encoded).unwrap(), id);

    let protocol: radroots_protocol::capability::v1::TransportKind = id.into();
    assert_eq!(protocol.as_str(), id.as_str());
    assert_eq!(TransportId::from(protocol), id);
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
            serde_json::json!({ "targets": [target, target] })
        )
        .is_err()
    );
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

    let label = RadrootsTransportTargetLabel::parse(" Relay One ").expect("label");
    assert_eq!(label.as_str(), "Relay One");
    assert_eq!(label.to_string(), "Relay One");
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
fn every_transport_error_has_a_stable_display_message() {
    let remaining = [
        RadrootsTransportError::RequiredTargetNotRequested,
        RadrootsTransportError::EmptyFetchRequestId,
        RadrootsTransportError::InvalidFetchRequestId,
        RadrootsTransportError::InvalidFetchLimit,
        RadrootsTransportError::InvalidFetchDeadline,
        RadrootsTransportError::EmptyFetchCursor,
        RadrootsTransportError::InvalidFetchCursor,
        RadrootsTransportError::InvalidObservedAt,
        RadrootsTransportError::UnexpectedFetchProvenance,
        RadrootsTransportError::UnexpectedFetchTargetOutcome,
        RadrootsTransportError::DuplicateFetchTargetOutcome,
        RadrootsTransportError::FetchPageLimitExceeded,
        RadrootsTransportError::FetchPageRequestMismatch,
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
        RadrootsTransportError::EmptyPayloadId,
        RadrootsTransportError::InvalidPayloadId,
        RadrootsTransportError::EmptyPayloadLabel,
        RadrootsTransportError::InvalidPayloadLabel,
        RadrootsTransportError::EmptyPayloadBytes,
        RadrootsTransportError::InvalidPayloadBytes,
        RadrootsTransportError::InvalidPayloadDigest,
        RadrootsTransportError::PayloadDigestMismatch,
    ];
    for error in remaining {
        assert!(!error.to_string().is_empty());
    }
}
