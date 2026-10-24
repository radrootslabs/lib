#![cfg(feature = "commands")]
use radroots_domain::{PoolId, PositiveDecimal, Sequence, stock::AvailableStockCorrection};
use radroots_protocol::command::{CapturedPoolCorrection, PoolCorrectionInput};
use serde_json::json;
fn wire(kind: &str) -> serde_json::Value {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/pool_creation.v1.json")).unwrap();
    let mut input = json!({"pool_id":"08".repeat(32),"expected_version":"1","amount":"0.125","reason":"counted during harvest"});
    if kind == "stock.adjust" {
        input["direction"] = json!("add");
    }
    json!({"type":kind,"context":fixture["wire"]["context"],"input":input})
}

#[test]
fn correction_hashes_match_independent_vectors_and_common_dispatch_retains_the_exact_family() {
    use radroots_protocol::command::{CapturedInventoryCommand, InventoryCommandKind};
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/pool_correction.v1.json")).unwrap();
    for (case, kind) in vectors["cases"].as_array().unwrap().iter().zip([
        InventoryCommandKind::AddStock,
        InventoryCommandKind::RemoveStock,
        InventoryCommandKind::SpoilStock,
    ]) {
        let captured =
            CapturedInventoryCommand::parse(&serde_json::to_vec(&case["wire"]).unwrap()).unwrap();
        assert_eq!(
            captured.digest().to_string(),
            case["command_digest"].as_str().unwrap()
        );
        assert_eq!(
            captured.approval_intent_digest().to_string(),
            case["approval_intent_digest"].as_str().unwrap()
        );
        assert_eq!(captured.kind(), kind);
        assert_eq!(captured.pool_id(), PoolId::from_bytes([8; 32]));
        assert!(captured.creation_input().is_none());
        assert_eq!(
            captured.correction_input().unwrap().reason(),
            "counted 🥕\nexactly"
        );
    }
    let original: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/pool_creation.v1.json")).unwrap();
    let created =
        CapturedInventoryCommand::parse(&serde_json::to_vec(&original["wire"]).unwrap()).unwrap();
    assert_eq!(created.kind(), InventoryCommandKind::CreatePool);
    assert_eq!(
        created.digest().to_string(),
        original["command_digest"].as_str().unwrap()
    );
    assert!(created.creation_input().is_some());
    assert!(created.correction_input().is_none());
    assert!(CapturedInventoryCommand::parse(br#"{"type":"inbox.inquire"}"#).is_err());
}
#[test]
fn adjustment_and_spoilage_capture_exact_semantics_without_unit_conversion() {
    for (kind, correction) in [
        ("stock.adjust", AvailableStockCorrection::Add),
        ("stock.spoil", AvailableStockCorrection::Spoil),
    ] {
        let raw = wire(kind);
        let captured = CapturedPoolCorrection::parse(&serde_json::to_vec(&raw).unwrap()).unwrap();
        assert_eq!(captured.input().kind(), correction);
        assert_eq!(
            captured.input().amount(),
            "0.125".parse::<PositiveDecimal>().unwrap()
        );
        assert_eq!(captured.input().expected_version().get(), 1);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(captured.protected_wire()).unwrap(),
            raw
        );
        assert_eq!(
            CapturedPoolCorrection::parse(captured.protected_wire())
                .unwrap()
                .digest(),
            captured.digest()
        );
        for (path, value) in [
            ("/input/amount", json!("0.25")),
            ("/input/expected_version", json!("2")),
            ("/input/pool_id", json!("09".repeat(32))),
            ("/input/reason", json!("counted 🥕\nexactly")),
        ] {
            let mut changed = raw.clone();
            *changed.pointer_mut(path).unwrap() = value;
            assert_ne!(
                CapturedPoolCorrection::parse(&serde_json::to_vec(&changed).unwrap())
                    .unwrap()
                    .digest(),
                captured.digest(),
                "{path}"
            );
        }
    }
    let mut changed = wire("stock.adjust");
    changed["input"]["direction"] = json!("remove");
    let removed = CapturedPoolCorrection::parse(&serde_json::to_vec(&changed).unwrap()).unwrap();
    assert_eq!(removed.input().kind(), AvailableStockCorrection::Remove);
    assert_ne!(
        removed.digest(),
        CapturedPoolCorrection::parse(&serde_json::to_vec(&wire("stock.adjust")).unwrap())
            .unwrap()
            .digest()
    );
}
#[test]
fn closed_correction_inputs_refuse_unknown_fields_numeric_values_and_zero_movements() {
    for kind in ["stock.adjust", "stock.spoil"] {
        let original = wire(kind);
        for (path, value) in [
            ("/input/amount", json!("0")),
            ("/input/amount", json!(0.125)),
            ("/input/amount", json!("0.1250")),
            ("/input/expected_version", json!(1)),
            ("/input/expected_version", json!("0")),
            ("/input/expected_version", json!("01")),
            ("/input/reason", json!("🧺".repeat(1025))),
        ] {
            let mut changed = original.clone();
            *changed.pointer_mut(path).unwrap() = value;
            assert!(
                CapturedPoolCorrection::parse(&serde_json::to_vec(&changed).unwrap()).is_err(),
                "{path}"
            );
        }
        for field in ["pool_id", "expected_version", "amount", "reason"] {
            let mut changed = original.clone();
            changed["input"].as_object_mut().unwrap().remove(field);
            assert!(
                CapturedPoolCorrection::parse(&serde_json::to_vec(&changed).unwrap()).is_err(),
                "{field}"
            );
        }
        let mut changed = original.clone();
        changed["input"]["ignored"] = json!(true);
        assert!(CapturedPoolCorrection::parse(&serde_json::to_vec(&changed).unwrap()).is_err());
        let mut changed = original.clone();
        changed["input"] = json!(["08".repeat(32), "1", "0.125", "reason"]);
        assert!(CapturedPoolCorrection::parse(&serde_json::to_vec(&changed).unwrap()).is_err());
    }
    let mut changed = wire("stock.spoil");
    changed["input"]["direction"] = json!("remove");
    assert!(CapturedPoolCorrection::parse(&serde_json::to_vec(&changed).unwrap()).is_err());
    let mut changed = wire("stock.adjust");
    changed["input"]["direction"] = json!("spoil");
    assert!(CapturedPoolCorrection::parse(&serde_json::to_vec(&changed).unwrap()).is_err());
}
#[test]
fn reasons_are_exact_bounded_private_text_and_diagnostic_correlation_is_excluded() {
    let raw = wire("stock.adjust");
    let base = CapturedPoolCorrection::parse(&serde_json::to_vec(&raw).unwrap()).unwrap();
    let input = PoolCorrectionInput::new(
        PoolId::from_bytes([8; 32]),
        Sequence::new(1).unwrap(),
        AvailableStockCorrection::Add,
        "0.125".parse().unwrap(),
        &"🧺".repeat(1024),
    )
    .unwrap();
    assert_eq!(input.reason().chars().count(), 1024);
    assert!(!format!("{input:?}").contains('🧺'));
    let mut changed = raw.clone();
    changed["context"]["correlation_id"] = json!("display-only");
    let changed = CapturedPoolCorrection::parse(&serde_json::to_vec(&changed).unwrap()).unwrap();
    assert_eq!(changed.digest(), base.digest());
    assert_eq!(
        changed.approval_intent_digest(),
        base.approval_intent_digest()
    );
    assert_ne!(changed.protected_wire(), base.protected_wire());
    assert!(!format!("{base:?}").contains("harvest"));
}
