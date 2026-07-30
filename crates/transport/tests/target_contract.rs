use core::str::FromStr;

use radroots_transport::{
    Error, TARGET_SET_MAX_ITEMS, Target, TargetSet, TransportId,
    endpoint::{
        ENDPOINT_URI_MAX_BYTES, EndpointUri, TARGET_LABEL_MAX_BYTES, TARGET_SCOPE_MAX_BYTES,
        TargetLabel, TargetScope,
    },
    target::TargetFingerprint,
};

#[test]
fn endpoint_scope_and_label_values_are_canonical_and_bounded() {
    let endpoint = EndpointUri::from_str("MESH://Node.Example/path").expect("endpoint");
    assert_eq!(endpoint.as_ref(), "mesh://node.example/path");
    assert_eq!(EndpointUri::try_from(endpoint.as_str()).unwrap(), endpoint);
    assert_eq!(
        EndpointUri::parse("a".repeat(ENDPOINT_URI_MAX_BYTES + 1)).unwrap_err(),
        Error::InvalidTargetUri
    );

    let scope = TargetScope::from_str("farm_1.alpha-beta").expect("scope");
    assert_eq!(scope.as_ref(), "farm_1.alpha-beta");
    assert_eq!(TargetScope::try_from(scope.as_str()).unwrap(), scope);
    assert_eq!(
        TargetScope::parse("a".repeat(TARGET_SCOPE_MAX_BYTES + 1)).unwrap_err(),
        Error::InvalidTargetScope
    );

    let label = TargetLabel::from_str(" Relay One ").expect("label");
    assert_eq!(label.as_ref(), "Relay One");
    assert_eq!(TargetLabel::try_from(label.as_str()).unwrap(), label);
    assert_eq!(
        TargetLabel::parse("a".repeat(TARGET_LABEL_MAX_BYTES + 1)).unwrap_err(),
        Error::InvalidTargetLabel
    );
}

#[test]
fn fingerprints_and_target_sets_are_deterministic_unique_and_bounded() {
    let target = Target::new(TransportId::LOCAL, "local:node-0").expect("target");
    let fingerprint =
        TargetFingerprint::from_str(target.fingerprint().as_str()).expect("target fingerprint");
    assert_eq!(&fingerprint, target.fingerprint());
    assert_eq!(
        TargetFingerprint::try_from(fingerprint.as_str()).unwrap(),
        fingerprint
    );

    let targets = (0..TARGET_SET_MAX_ITEMS)
        .map(|index| {
            Target::new(TransportId::LOCAL, format!("local:node-{index}")).expect("bounded target")
        })
        .collect();
    let target_set = TargetSet::new(targets).expect("maximum target set");
    assert_eq!(target_set.len(), TARGET_SET_MAX_ITEMS);

    let oversized = (0..=TARGET_SET_MAX_ITEMS)
        .map(|index| {
            Target::new(TransportId::LOCAL, format!("local:oversized-{index}"))
                .expect("oversized target member")
        })
        .collect();
    assert_eq!(
        TargetSet::new(oversized).unwrap_err(),
        Error::TargetSetTooLarge
    );

    assert_eq!(
        TargetSet::new(vec![target.clone(), target]).unwrap_err(),
        Error::DuplicateTargetFingerprint
    );
}
