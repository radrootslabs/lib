#![cfg(feature = "commands")]
use radroots_protocol::command::CapturedOptionMapping;
use serde_json::{Value, json};
fn cases() -> Value {
    serde_json::from_str(include_str!("fixtures/option_mapping.v1.json")).unwrap()
}

#[test]
fn captured_mapping_matches_independent_canonical_hashes_and_preserves_original_quantities() {
    for case in cases()["cases"].as_array().unwrap() {
        let captured =
            CapturedOptionMapping::parse(&serde_json::to_vec(&case["wire"]).unwrap()).unwrap();
        assert_eq!(
            captured.digest().to_string(),
            case["command_digest"].as_str().unwrap()
        );
        assert_eq!(
            captured.approval_intent_digest().to_string(),
            case["approval_intent_digest"].as_str().unwrap()
        );
        assert_eq!(
            serde_json::from_slice::<Value>(captured.protected_wire()).unwrap(),
            case["wire"]
        );
        assert_eq!(
            CapturedOptionMapping::parse(captured.protected_wire())
                .unwrap()
                .digest(),
            captured.digest()
        );
        assert_eq!(format!("{captured:?}"), "CapturedOptionMapping(<redacted>)");
    }
    let first = &cases()["cases"][0]["wire"];
    let captured = CapturedOptionMapping::parse(&serde_json::to_vec(first).unwrap()).unwrap();
    assert_eq!(captured.input().per_sale_unit().amount().to_string(), "250");
    assert_eq!(captured.input().expected_mapping_version().get(), 0);
    for (path, value) in [
        ("/input/pool_id", json!("09".repeat(32))),
        ("/input/option_id", json!("0c".repeat(32))),
        ("/input/per_sale_unit/amount", json!("500")),
        ("/input/expected_mapping_version", json!("1")),
        ("/input/listing/d", json!("0c".repeat(16))),
    ] {
        let mut changed = first.clone();
        *changed.pointer_mut(path).unwrap() = value;
        assert_ne!(
            CapturedOptionMapping::parse(&serde_json::to_vec(&changed).unwrap())
                .unwrap()
                .digest(),
            captured.digest()
        );
    }
    let mut correlated = first.clone();
    correlated["context"]["correlation_id"] = json!("mapping-diagnostic");
    assert_eq!(
        CapturedOptionMapping::parse(&serde_json::to_vec(&correlated).unwrap())
            .unwrap()
            .digest(),
        captured.digest()
    );
}

#[test]
fn mapping_capture_refuses_noncanonical_or_ambiguous_closed_values() {
    let wire = &cases()["cases"][0]["wire"];
    let mut uppercase = wire.clone();
    uppercase["input"]["listing"]["author"] = json!(
        wire["input"]["listing"]["author"]
            .as_str()
            .unwrap()
            .to_ascii_uppercase()
    );
    assert!(CapturedOptionMapping::parse(&serde_json::to_vec(&uppercase).unwrap()).is_err());
    for (path, value) in [
        ("/type", json!("stock.map")),
        ("/input/listing/kind", json!(30403)),
        ("/input/listing/kind", json!("30402")),
        ("/input/listing/kind", json!(30402.0)),
        ("/input/listing/d", json!("0A".repeat(16))),
        ("/input/per_sale_unit/amount", json!("0")),
        ("/input/per_sale_unit/amount", json!(250)),
        ("/input/per_sale_unit/amount", json!("250.0")),
        ("/input/per_sale_unit/unit", json!("bag")),
        ("/input/expected_mapping_version", json!(0)),
        ("/input/expected_mapping_version", json!("00")),
        ("/input/per_sale_unit", json!(["250", "g"])),
    ] {
        let mut changed = wire.clone();
        *changed.pointer_mut(path).unwrap() = value;
        assert!(
            CapturedOptionMapping::parse(&serde_json::to_vec(&changed).unwrap()).is_err(),
            "{path}"
        );
    }
    for field in [
        "listing",
        "option_id",
        "pool_id",
        "per_sale_unit",
        "expected_mapping_version",
    ] {
        let mut changed = wire.clone();
        changed["input"].as_object_mut().unwrap().remove(field);
        assert!(
            CapturedOptionMapping::parse(&serde_json::to_vec(&changed).unwrap()).is_err(),
            "{field}"
        );
    }
    for (path, name, value) in [
        ("", "extra", json!(true)),
        ("/input", "unit", json!("kg")),
        ("/input/listing", "revision", json!("0c".repeat(32))),
        ("/input/per_sale_unit", "estimate", json!(true)),
    ] {
        let mut changed = wire.clone();
        changed.pointer_mut(path).unwrap()[name] = value;
        assert!(CapturedOptionMapping::parse(&serde_json::to_vec(&changed).unwrap()).is_err());
    }
    let raw = serde_json::to_string(wire).unwrap();
    let duplicate=raw.replacen("\"pool_id\":","\"pool_id\":\"0909090909090909090909090909090909090909090909090909090909090909\",\"pool_id\":",1);
    assert!(CapturedOptionMapping::parse(duplicate.as_bytes()).is_err());
}
