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
        RadrootsTransportKind::parse("fieldbus").expect("custom kind"),
        RadrootsTransportKind::Custom("fieldbus".to_owned())
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
