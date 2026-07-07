use radroots_transport::{
    RadrootsTransportDeliveryReceipt, RadrootsTransportDeliveryRequest,
    RadrootsTransportDeliveryTargetStatus, RadrootsTransportError, RadrootsTransportKind,
    RadrootsTransportOutcome, RadrootsTransportSatisfactionPolicy, RadrootsTransportTarget,
    RadrootsTransportTargetFingerprint, RadrootsTransportTargetReceipt, RadrootsTransportTargetSet,
    RadrootsTransportTargetUri,
};

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
        "wss://relay.example/Events",
    )
    .expect("reticulum target");

    assert_eq!(nostr_upper.uri.as_str(), "wss://relay.example/Events");
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
        RadrootsTransportKind::Local.canonical_label(),
        "local".to_owned()
    );
    assert_eq!(
        RadrootsTransportKind::parse("fieldbus").expect("custom kind"),
        RadrootsTransportKind::Custom("fieldbus".to_owned())
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
    let all = RadrootsTransportSatisfactionPolicy::AllTargets;
    let any = RadrootsTransportSatisfactionPolicy::AnyTarget;
    let two = RadrootsTransportSatisfactionPolicy::AtLeast(2);

    assert!(all.is_satisfied_by(2, 2).expect("all"));
    assert!(!all.is_satisfied_by(2, 1).expect("all incomplete"));
    assert!(any.is_satisfied_by(3, 1).expect("any"));
    assert!(two.is_satisfied_by(3, 2).expect("two"));
    assert_eq!(
        any.is_satisfied_by(0, 0).expect_err("zero target set"),
        RadrootsTransportError::InvalidSatisfactionPolicy
    );
    assert_eq!(
        RadrootsTransportSatisfactionPolicy::AtLeast(0)
            .is_satisfied_by(3, 0)
            .expect_err("zero required targets"),
        RadrootsTransportError::InvalidSatisfactionPolicy
    );
}

#[test]
fn deferred_transport_outcomes_are_terminal_but_not_satisfied() {
    let target =
        RadrootsTransportTarget::new(RadrootsTransportKind::Reticulum, "reticulum:preview")
            .expect("target");
    let receipt = RadrootsTransportDeliveryReceipt {
        request_id: "reticulum-preview".to_owned(),
        target_receipts: vec![RadrootsTransportTargetReceipt::new(
            target,
            RadrootsTransportOutcome::new(RadrootsTransportDeliveryTargetStatus::Deferred),
        )],
    };

    assert!(RadrootsTransportDeliveryTargetStatus::Deferred.is_terminal());
    assert_eq!(receipt.satisfied_target_count(), 0);
    assert!(
        !RadrootsTransportSatisfactionPolicy::AnyTarget
            .is_satisfied_by(1, receipt.satisfied_target_count())
            .expect("satisfaction check")
    );
}

#[test]
fn request_models_round_trip_with_serde() {
    let target = RadrootsTransportTarget::new(RadrootsTransportKind::Nostr, "wss://relay.example")
        .expect("target");
    let target_set = RadrootsTransportTargetSet::new(vec![target]).expect("target set");
    let request = RadrootsTransportDeliveryRequest::new(
        "req-1",
        "sha256:payload",
        target_set,
        RadrootsTransportSatisfactionPolicy::AnyTarget,
    );

    let json = serde_json::to_string(&request).expect("serialize request");
    let decoded: RadrootsTransportDeliveryRequest =
        serde_json::from_str(&json).expect("decode request");

    assert_eq!(decoded, request);
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
fn satisfaction_and_target_status_cover_all_contract_states() {
    assert_eq!(
        RadrootsTransportSatisfactionPolicy::AllTargets
            .required_target_count(3)
            .expect("all targets"),
        3
    );
    assert_eq!(
        RadrootsTransportSatisfactionPolicy::AnyTarget
            .required_target_count(3)
            .expect("any target"),
        1
    );
    assert_eq!(
        RadrootsTransportSatisfactionPolicy::AtLeast(4)
            .required_target_count(3)
            .expect_err("at least too high"),
        RadrootsTransportError::InvalidSatisfactionPolicy
    );

    let statuses = [
        RadrootsTransportDeliveryTargetStatus::Pending,
        RadrootsTransportDeliveryTargetStatus::Accepted,
        RadrootsTransportDeliveryTargetStatus::Deferred,
        RadrootsTransportDeliveryTargetStatus::Rejected,
        RadrootsTransportDeliveryTargetStatus::Failed,
        RadrootsTransportDeliveryTargetStatus::Unavailable,
    ];
    assert!(!statuses[0].is_terminal());
    assert!(statuses[1..].iter().all(|status| status.is_terminal()));
    assert!(RadrootsTransportDeliveryTargetStatus::Accepted.counts_as_satisfied());
    assert!(
        statuses
            .iter()
            .filter(|status| **status != RadrootsTransportDeliveryTargetStatus::Accepted)
            .all(|status| !status.counts_as_satisfied())
    );
}
