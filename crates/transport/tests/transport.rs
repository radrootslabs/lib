use radroots_transport::{
    RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI, RADROOTS_RETICULUM_PREVIEW_SCOPE_ID,
    RadrootsTransport, RadrootsTransportCapabilities, RadrootsTransportDeliveryReceipt,
    RadrootsTransportDeliveryRequest, RadrootsTransportDeliveryTargetStatus,
    RadrootsTransportError, RadrootsTransportFetchReceipt, RadrootsTransportFetchRequest,
    RadrootsTransportFuture, RadrootsTransportImplementationState, RadrootsTransportKind,
    RadrootsTransportMeshScopeId, RadrootsTransportOutcome, RadrootsTransportOutcomeKind,
    RadrootsTransportPayload, RadrootsTransportSatisfactionClass,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportStatus, RadrootsTransportTarget,
    RadrootsTransportTargetFingerprint, RadrootsTransportTargetLabel,
    RadrootsTransportTargetReceipt, RadrootsTransportTargetSet, RadrootsTransportTargetUri,
};

fn opaque_payload() -> RadrootsTransportPayload {
    RadrootsTransportPayload::opaque_bytes("transport-test-payload", b"transport payload")
        .expect("payload")
}

#[test]
fn target_fingerprints_are_stable_and_transport_scoped() {
    let nostr_upper =
        RadrootsTransportTarget::new(RadrootsTransportKind::Nostr, " WSS://Relay.Example/Events ")
            .expect("nostr target");
    let nostr_lower =
        RadrootsTransportTarget::new(RadrootsTransportKind::Nostr, "wss://relay.example/Events")
            .expect("nostr target");
    let reticulum = RadrootsTransportTarget::new(
        RadrootsTransportKind::Reticulum,
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI,
    )
    .expect("reticulum target");

    assert_eq!(nostr_upper.uri.as_str(), "wss://relay.example/Events");
    assert_eq!(nostr_upper.scope, None);
    assert_eq!(
        reticulum.scope.as_ref().map(|scope| scope.as_str()),
        Some(RADROOTS_RETICULUM_PREVIEW_SCOPE_ID)
    );
    assert_eq!(nostr_upper.fingerprint, nostr_lower.fingerprint);
    assert_ne!(nostr_upper.fingerprint, reticulum.fingerprint);
    assert_eq!(
        nostr_upper.fingerprint.as_str(),
        "d0903c3067150d7b4f7efd92a9be002b97d74e83f8bb6827327fa7ecd869332b"
    );
}

#[test]
fn transport_kind_parser_round_trips_canonical_labels_and_custom_values() {
    assert_eq!(
        RadrootsTransportKind::parse(" NOSTR ").expect("nostr kind"),
        RadrootsTransportKind::Nostr
    );
    assert_eq!(
        RadrootsTransportKind::parse("reticulum").expect("reticulum kind"),
        RadrootsTransportKind::Reticulum
    );
    assert_eq!(
        RadrootsTransportKind::parse("mesh").expect("mesh kind"),
        RadrootsTransportKind::Mesh
    );
    assert_eq!(
        RadrootsTransportKind::parse("local").expect("local kind"),
        RadrootsTransportKind::Local
    );
    assert_eq!(
        RadrootsTransportKind::parse("PROXY").expect("proxy kind"),
        RadrootsTransportKind::Proxy
    );
    assert_eq!(
        RadrootsTransportKind::Local.canonical_label(),
        "local".to_owned()
    );
    assert_eq!(
        RadrootsTransportKind::Proxy.canonical_label(),
        "proxy".to_owned()
    );
    assert_eq!(
        RadrootsTransportKind::parse("fieldbus").expect("custom kind"),
        RadrootsTransportKind::Custom("fieldbus".to_owned())
    );
    assert_eq!(
        RadrootsTransportKind::parse(removed_proxy_kind()).expect_err("removed proxy kind"),
        RadrootsTransportError::InvalidTransportKind
    );
    assert_eq!(
        RadrootsTransportKind::custom(removed_proxy_kind().to_ascii_uppercase())
            .expect_err("removed proxy custom kind"),
        RadrootsTransportError::InvalidTransportKind
    );
    assert_eq!(
        RadrootsTransportKind::Custom("fieldbus".to_owned()).canonical_label(),
        "fieldbus".to_owned()
    );
    assert_eq!(
        RadrootsTransportKind::parse("bad kind").expect_err("invalid kind"),
        RadrootsTransportError::InvalidTransportKind
    );
}

#[test]
fn canonical_transport_kind_parser_rejects_noncanonical_public_values() {
    assert_eq!(
        RadrootsTransportKind::parse_canonical("nostr").expect("nostr kind"),
        RadrootsTransportKind::Nostr
    );
    assert_eq!(
        RadrootsTransportKind::parse_canonical("fieldbus").expect("custom kind"),
        RadrootsTransportKind::Custom("fieldbus".to_owned())
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
        RadrootsTransportKind::parse_canonical(removed_proxy_kind())
            .expect_err("removed proxy kind"),
        RadrootsTransportError::InvalidTransportKind
    );
    assert_eq!(
        RadrootsTransportKind::parse_canonical("").expect_err("empty kind"),
        RadrootsTransportError::EmptyTransportKind
    );
}

fn removed_proxy_kind() -> String {
    ["radrootsd", "_proxy"].concat()
}

#[test]
fn target_set_rejects_duplicate_fingerprints() {
    let first = RadrootsTransportTarget::new(RadrootsTransportKind::Nostr, "wss://relay.example/a")
        .expect("first target");
    let duplicate =
        RadrootsTransportTarget::new(RadrootsTransportKind::Nostr, "WSS://RELAY.EXAMPLE/a")
            .expect("duplicate target");
    let err = RadrootsTransportTargetSet::new(vec![first, duplicate])
        .expect_err("duplicate fingerprints must fail");

    assert_eq!(err, RadrootsTransportError::DuplicateTargetFingerprint);
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
    let target = RadrootsTransportTarget::new(
        RadrootsTransportKind::Reticulum,
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI,
    )
    .expect("target");
    let receipt = RadrootsTransportDeliveryReceipt {
        request_id: "reticulum-preview".to_owned(),
        target_receipts: vec![RadrootsTransportTargetReceipt::new(
            target,
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::DeferredUntilImplemented),
        )],
    };

    assert!(RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented.is_deferred_preview());
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
    let target = RadrootsTransportTarget::new(RadrootsTransportKind::Nostr, "wss://relay.example")
        .expect("target");
    let target_set = RadrootsTransportTargetSet::new(vec![target]).expect("target set");
    let request = RadrootsTransportDeliveryRequest::new(
        "req-1",
        opaque_payload(),
        target_set,
        RadrootsTransportSatisfactionPolicy::any_accepted(),
    );

    let json = serde_json::to_string(&request).expect("serialize request");
    let decoded: RadrootsTransportDeliveryRequest =
        serde_json::from_str(&json).expect("decode request");

    assert_eq!(decoded, request);
}

#[test]
fn payload_contract_derives_and_validates_digests() {
    let event_id = "a".repeat(64);
    let signed = RadrootsTransportPayload::signed_event_json(
        event_id.as_str(),
        "{\"id\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}",
    )
    .expect("signed event payload");
    assert_eq!(signed.payload_kind(), "signed_event_json");
    assert_eq!(signed.digest().len(), 64);
    assert!(
        signed
            .digest()
            .bytes()
            .all(|byte| { byte.is_ascii_digit() || matches!(byte, b'a'..=b'f') })
    );
    assert_eq!(
        RadrootsTransportPayload::signed_event_json_with_digest(
            event_id.as_str(),
            "{\"id\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}",
            signed.digest(),
        )
        .expect("signed payload with digest"),
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
fn payload_contract_rejects_invalid_ids_bytes_labels_and_digests() {
    assert_eq!(
        RadrootsTransportPayload::signed_event_json("A".repeat(64), "{}")
            .expect_err("uppercase event id"),
        RadrootsTransportError::InvalidPayloadId
    );
    assert_eq!(
        RadrootsTransportPayload::signed_event_json("a".repeat(64), " [] ")
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
    assert_eq!(
        RadrootsTransportKind::custom(" ").expect_err("empty kind"),
        RadrootsTransportError::EmptyTransportKind
    );
    for invalid in ["bad kind", "bad:kind", "bad/kind", "bad\nkind"] {
        assert_eq!(
            RadrootsTransportKind::custom(invalid).expect_err("invalid kind"),
            RadrootsTransportError::InvalidTransportKind
        );
    }
    assert_eq!(
        RadrootsTransportKind::custom(" FieldBus ").expect("custom kind"),
        RadrootsTransportKind::Custom("fieldbus".to_owned())
    );

    let no_scheme =
        RadrootsTransportTargetUri::parse(" transport-target ").expect("schemeless target uri");
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
fn reticulum_transport_targets_require_exact_preview_endpoint() {
    let target = RadrootsTransportTarget::new(
        RadrootsTransportKind::Reticulum,
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI,
    )
    .expect("exact Reticulum preview endpoint");
    assert_eq!(target.uri.as_str(), RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI);
    assert_eq!(
        target.scope.as_ref().map(|scope| scope.as_str()),
        Some(RADROOTS_RETICULUM_PREVIEW_SCOPE_ID)
    );

    for invalid in [
        " reticulum:preview-unavailable",
        "reticulum:preview-unavailable ",
        "RETICULUM:preview-unavailable",
        "reticulum:Preview-Unavailable",
        "reticulum:preview",
        "reticulum:preview-unavailable-alt",
        "reticulum:custom",
        "wss://relay.example/Events",
    ] {
        assert_eq!(
            RadrootsTransportTarget::new(RadrootsTransportKind::Reticulum, invalid)
                .expect_err("invalid Reticulum preview endpoint"),
            RadrootsTransportError::InvalidTargetUri
        );
    }
}

#[test]
fn target_fingerprints_and_sets_cover_accessors_and_validation() {
    let target = RadrootsTransportTarget::new(RadrootsTransportKind::Mesh, "mesh://node.example")
        .expect("mesh target");
    let parsed =
        RadrootsTransportTargetFingerprint::parse(target.fingerprint.as_str().to_ascii_uppercase())
            .expect("uppercase fingerprint parses");
    assert_eq!(parsed.as_str(), target.fingerprint.as_str());
    assert_eq!(parsed.to_string(), target.fingerprint.as_str());
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
    let local_scope = RadrootsTransportMeshScopeId::parse("local_preview").expect("local scope");
    let remote_scope = RadrootsTransportMeshScopeId::parse("remote_preview").expect("remote scope");
    let local = RadrootsTransportTarget::new_with_metadata(
        RadrootsTransportKind::Mesh,
        "mesh://node.example",
        Some(local_scope.clone()),
        Some(RadrootsTransportTargetLabel::parse("Local mesh node").expect("label")),
    )
    .expect("local mesh target");
    let relabeled = RadrootsTransportTarget::new_with_metadata(
        RadrootsTransportKind::Mesh,
        "mesh://node.example",
        Some(local_scope),
        Some(RadrootsTransportTargetLabel::parse("Renamed node").expect("label")),
    )
    .expect("relabeled mesh target");
    let remote = RadrootsTransportTarget::new_with_metadata(
        RadrootsTransportKind::Mesh,
        "mesh://node.example",
        Some(remote_scope),
        None,
    )
    .expect("remote mesh target");

    assert_eq!(local.fingerprint, relabeled.fingerprint);
    assert_ne!(local.fingerprint, remote.fingerprint);
    assert_eq!(
        local.scope.as_ref().map(|scope| scope.as_str()),
        Some("local_preview")
    );
    assert_eq!(
        local.label.as_ref().map(|label| label.as_str()),
        Some("Local mesh node")
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
            RadrootsTransportDeliveryTargetStatus::PreviewUnavailable,
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
    assert!(RadrootsTransportDeliveryTargetStatus::PreviewUnavailable.is_deferred_preview());
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

    let preview_unavailable =
        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::TransportUnavailable)
            .with_target_status(RadrootsTransportDeliveryTargetStatus::PreviewUnavailable);
    assert_eq!(
        preview_unavailable.status,
        RadrootsTransportDeliveryTargetStatus::PreviewUnavailable
    );
}

#[test]
fn required_target_satisfaction_uses_fingerprints_not_target_counts() {
    let required = RadrootsTransportTarget::new(RadrootsTransportKind::Nostr, "wss://one.example")
        .expect("required target");
    let optional = RadrootsTransportTarget::new(RadrootsTransportKind::Nostr, "wss://two.example")
        .expect("optional target");
    let policy = RadrootsTransportSatisfactionPolicy::required_targets(
        RadrootsTransportSatisfactionClass::Accepted,
        vec![required.fingerprint.clone()],
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
        &[required.fingerprint.clone()]
    );
    let unordered_policy = RadrootsTransportSatisfactionPolicy::required_targets(
        RadrootsTransportSatisfactionClass::Accepted,
        vec![optional.fingerprint.clone(), required.fingerprint.clone()],
    )
    .expect("unordered required targets");
    let mut expected_required_targets =
        vec![required.fingerprint.clone(), optional.fingerprint.clone()];
    expected_required_targets.sort();
    assert_eq!(
        unordered_policy
            .required_target_fingerprints()
            .expect("canonical required targets"),
        expected_required_targets.as_slice()
    );

    let optional_only = RadrootsTransportDeliveryReceipt {
        request_id: "required-target".to_owned(),
        target_receipts: vec![RadrootsTransportTargetReceipt::new(
            optional.clone(),
            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
        )],
    };
    assert!(
        !optional_only
            .is_satisfied_by(&policy)
            .expect("missing required target")
    );

    let required_delivered = RadrootsTransportDeliveryReceipt {
        request_id: "required-target".to_owned(),
        target_receipts: vec![
            RadrootsTransportTargetReceipt::new(
                optional,
                RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Rejected),
            ),
            RadrootsTransportTargetReceipt::new(
                required.clone(),
                RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
            ),
        ],
    };
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
            vec![required.fingerprint.clone(), required.fingerprint],
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
                Ok(RadrootsTransportDeliveryReceipt {
                    request_id: request.request_id,
                    target_receipts: vec![RadrootsTransportTargetReceipt::new(
                        self.target.clone(),
                        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Delivered),
                    )],
                })
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

    let target = RadrootsTransportTarget::new(RadrootsTransportKind::Local, "local:memory")
        .expect("local target");
    let target_set = RadrootsTransportTargetSet::new(vec![target.clone()]).expect("target set");
    let transport = MemoryTransport { target };
    assert_eq!(transport.transport_kind(), RadrootsTransportKind::Local);
    let status = futures::executor::block_on(transport.status()).expect("status");
    assert_eq!(status.kind, RadrootsTransportKind::Local);
    assert_eq!(
        status.capabilities,
        RadrootsTransportCapabilities::deliver_and_fetch()
    );
    let delivery =
        futures::executor::block_on(transport.deliver(RadrootsTransportDeliveryRequest::new(
            "deliver-1",
            opaque_payload(),
            target_set.clone(),
            RadrootsTransportSatisfactionPolicy::all_delivered(),
        )))
        .expect("deliver");
    assert_eq!(
        delivery.target_receipts[0].outcome.kind,
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
