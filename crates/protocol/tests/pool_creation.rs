#![cfg(feature = "commands")]
use radroots_domain::{
    ActorRef, ActorRole, AuthorityRef, CommandContextData, CorrelationId, Decimal, ExecutorId,
    Generation, GrantId, OperationId, PoolId, PublicKey, Quantity, RouteDigest, UnixSeconds,
    WorkspaceId, listing::MeasurementUnit, stock::PoolUnit,
};
use radroots_protocol::command::{CapturedPoolCreation, CreatePoolInput};
use serde_json::json;
fn context(correlation: Option<&str>) -> CommandContextData {
    let key =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .unwrap();
    CommandContextData::new(
        OperationId::from_bytes([7; 32]),
        ActorRef::new(
            key,
            ActorRole::Seller,
            None,
            Some(GrantId::from_bytes([3; 32])),
        ),
        WorkspaceId::from_bytes([1; 32]),
        AuthorityRef::new(ExecutorId::from_bytes([2; 32]), Generation::new(1).unwrap()),
        RouteDigest::from_bytes([6; 32]),
        None,
        None,
        UnixSeconds::new(200).unwrap(),
        correlation.map(|s| CorrelationId::new(s).unwrap()),
    )
    .unwrap()
}
fn capture(scale: u8, correlation: Option<&str>) -> CapturedPoolCreation {
    let unit = PoolUnit::new(MeasurementUnit::Kilogram, scale).unwrap();
    let quantity = Quantity::new(
        "0.25".parse::<Decimal>().unwrap(),
        MeasurementUnit::Kilogram,
    )
    .unwrap();
    CapturedPoolCreation::new(
        context(correlation),
        CreatePoolInput::new(PoolId::from_bytes([8; 32]), "Carrots", unit, quantity).unwrap(),
    )
    .unwrap()
}
#[test]
fn exact_pool_capture_binds_precision_and_excludes_only_diagnostic_correlation() {
    let original = capture(3, None);
    let parsed = CapturedPoolCreation::parse(original.protected_wire()).unwrap();
    assert_eq!(parsed.digest(), original.digest());
    assert_eq!(parsed.protected_wire(), original.protected_wire());
    assert_eq!(parsed.input().initial_ticks().get(), 250);
    let correlated = capture(3, Some("display-only"));
    assert_eq!(original.digest(), correlated.digest());
    assert_ne!(original.protected_wire(), correlated.protected_wire());
    let finer = capture(4, None);
    assert_ne!(original.digest(), finer.digest());
    assert_eq!(finer.input().initial_ticks().get(), 2500);
    let value: serde_json::Value = serde_json::from_slice(original.protected_wire()).unwrap();
    assert_eq!(value["input"]["tick_scale"], json!("3"));
    assert_eq!(value["input"]["initial_quantity"], json!("0.25"));
}
#[test]
fn malformed_or_implicit_pool_precision_and_unknown_shapes_are_refused() {
    let original = capture(3, None);
    let base: serde_json::Value = serde_json::from_slice(original.protected_wire()).unwrap();
    for (path, bad) in [
        ("/input/tick_scale", json!(3)),
        ("/input/tick_scale", json!("03")),
        ("/input/tick_scale", json!("7")),
        ("/input/tick_scale", json!("1")),
        ("/input/unit", json!("each")),
        ("/input/unit", json!({"kg":null})),
        ("/input/initial_quantity", json!(0.25)),
        ("/input/initial_quantity", json!("0.250")),
        ("/input/id", json!("F".repeat(64))),
    ] {
        let mut v = base.clone();
        *v.pointer_mut(path).unwrap() = bad;
        assert!(
            CapturedPoolCreation::parse(&serde_json::to_vec(&v).unwrap()).is_err(),
            "{path}"
        );
    }
    for field in ["tick_scale", "unit", "label", "initial_quantity", "id"] {
        let mut v = base.clone();
        v["input"].as_object_mut().unwrap().remove(field);
        assert!(
            CapturedPoolCreation::parse(&serde_json::to_vec(&v).unwrap()).is_err(),
            "{field}"
        );
    }
    for path in ["", "/input"] {
        let mut v = base.clone();
        v.pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("ignored".into(), json!(true));
        assert!(CapturedPoolCreation::parse(&serde_json::to_vec(&v).unwrap()).is_err());
    }
    assert!(
        CapturedPoolCreation::parse(
            &serde_json::to_vec(&json!([base["type"], base["context"], base["input"]])).unwrap()
        )
        .is_err()
    );
}
#[test]
fn typed_creation_never_converts_units_rounds_quantities_or_copies_oversized_labels() {
    let unit = PoolUnit::new(MeasurementUnit::Kilogram, 3).unwrap();
    let other = Quantity::new(Decimal::ONE, MeasurementUnit::Gram).unwrap();
    assert!(CreatePoolInput::new(PoolId::from_bytes([1; 32]), "x", unit, other).is_err());
    let quantity = Quantity::new(Decimal::ONE, MeasurementUnit::Kilogram).unwrap();
    assert!(
        CreatePoolInput::new(
            PoolId::from_bytes([1; 32]),
            &"a".repeat(161),
            unit,
            quantity
        )
        .is_err()
    );
    assert!(
        CreatePoolInput::new(PoolId::from_bytes([1; 32]), "hidden\nlabel", unit, quantity).is_err()
    );
}

#[test]
fn complete_pool_command_matches_independent_canonical_hash_vector() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/pool_creation.v1.json")).unwrap();
    let captured =
        CapturedPoolCreation::parse(&serde_json::to_vec(&fixture["wire"]).unwrap()).unwrap();
    assert_eq!(
        captured.digest().to_string(),
        fixture["command_digest"].as_str().unwrap()
    );
    assert_eq!(
        captured.approval_intent_digest().to_string(),
        fixture["approval_intent_digest"].as_str().unwrap()
    );
    for (path, value) in [
        ("/context/authority/generation", json!("2")),
        ("/context/actor/grant_id", json!("09".repeat(32))),
        ("/context/not_after", json!("201")),
        ("/context/route_digest", json!("09".repeat(32))),
        ("/context/workspace_id", json!("09".repeat(32))),
        ("/input/unit", json!("g")),
        ("/input/label", json!("カブ 🥕")),
        ("/input/initial_quantity", json!("0.5")),
        ("/input/tick_scale", json!("4")),
    ] {
        let mut changed = fixture["wire"].clone();
        *changed.pointer_mut(path).unwrap() = value;
        let changed = CapturedPoolCreation::parse(&serde_json::to_vec(&changed).unwrap()).unwrap();
        assert_ne!(changed.digest(), captured.digest(), "{path}");
    }
}
