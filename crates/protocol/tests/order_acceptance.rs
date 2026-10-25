#![cfg(feature = "commands")]
use radroots_protocol::command::{
    CapturedCommand, CapturedInventoryCommand, CapturedOrderAcceptance,
};
use serde_json::{Value, json};
fn cases() -> Value {
    serde_json::from_str(include_str!("fixtures/order_acceptance.v1.json")).unwrap()
}
fn parse(
    value: &Value,
) -> Result<CapturedOrderAcceptance, radroots_protocol::command::CommandError> {
    CapturedOrderAcceptance::parse(&serde_json::to_vec(value).unwrap())
}

#[test]
fn acceptance_capture_matches_independent_automatic_and_explicit_hash_vectors() {
    for case in cases()["cases"].as_array().unwrap() {
        let captured = parse(&case["wire"]).unwrap();
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
            CapturedOrderAcceptance::parse(captured.protected_wire())
                .unwrap()
                .digest(),
            captured.digest()
        );
        assert_eq!(
            format!("{captured:?}"),
            "CapturedOrderAcceptance(<redacted>)"
        );
    }
    let data = cases();
    let automatic = parse(&data["cases"][0]["wire"]).unwrap();
    assert!(automatic.input().allocation().is_automatic());
    assert!(
        automatic
            .input()
            .allocation()
            .explicit_allocations()
            .is_none()
    );
    let explicit = parse(&data["cases"][1]["wire"]).unwrap();
    let allocations = explicit
        .input()
        .allocation()
        .explicit_allocations()
        .unwrap();
    assert_eq!(allocations.len(), 2);
    assert_eq!(allocations[0].quantity().amount().to_string(), "250");
    assert_eq!(allocations[1].expected_pool_version().get(), 4);
    // Preparation cannot enter either existing outbox or inventory execution.
    assert!(CapturedCommand::parse(automatic.protected_wire()).is_err());
    assert!(CapturedInventoryCommand::parse(automatic.protected_wire()).is_err());
}

#[test]
fn expected_order_terms_request_and_allocation_are_part_of_the_original_intent() {
    let data = cases();
    let wire = &data["cases"][0]["wire"];
    let original = parse(wire).unwrap();
    for (path, value) in [
        ("/input/request", json!("31".repeat(32))),
        ("/input/terms_digest", json!("32".repeat(32))),
        ("/context/expected/buyer_head", json!("33".repeat(32))),
        ("/context/expected/seller_sequence", json!("1")),
        (
            "/input/allocation",
            data["cases"][1]["wire"]["input"]["allocation"].clone(),
        ),
    ] {
        let mut changed = wire.clone();
        *changed.pointer_mut(path).unwrap() = value;
        let changed = parse(&changed).unwrap();
        assert_ne!(changed.digest(), original.digest());
        assert_ne!(
            changed.approval_intent_digest(),
            original.approval_intent_digest()
        );
    }
    let mut correlated = wire.clone();
    correlated["context"]["correlation_id"] = json!("T0 diagnostic");
    assert_eq!(parse(&correlated).unwrap().digest(), original.digest());
    let mut approved = wire.clone();
    approved["context"]["approval"] = json!({"intent_digest":original.approval_intent_digest(),"scope":"orders.accept","expires_at":"200"});
    let approved = parse(&approved).unwrap();
    assert_ne!(approved.digest(), original.digest());
    assert_eq!(
        approved.approval_intent_digest(),
        original.approval_intent_digest()
    );
}

#[test]
fn capture_requires_closed_seller_order_data_and_distinguishes_allocation_choices() {
    let data = cases();
    let automatic = &data["cases"][0]["wire"];
    for (path, value) in [
        ("/type", json!("orders.decline")),
        ("/context/actor/role", json!("buyer")),
        ("/context/expected", Value::Null),
        ("/input/request", json!("2A".repeat(32))),
        ("/input/terms_digest", json!("2A".repeat(32))),
        ("/input/allocation", json!([])),
        (
            "/input/allocation",
            json!({"type":"automatic","allocations":null}),
        ),
        (
            "/input/allocation",
            json!({"type":"automatic","allocations":[]}),
        ),
        (
            "/input/allocation",
            json!({"type":"explicit","allocations":[]}),
        ),
        ("/input/allocation", json!({"type":"explicit"})),
        ("/input/allocation", json!({"type":"unknown"})),
        (
            "/input/allocation",
            json!({"type":"automatic","stock_free":true}),
        ),
    ] {
        let mut changed = automatic.clone();
        *changed.pointer_mut(path).unwrap() = value;
        assert!(parse(&changed).is_err(), "{path}");
    }
    for field in ["request", "terms_digest", "allocation"] {
        let mut changed = automatic.clone();
        changed["input"].as_object_mut().unwrap().remove(field);
        assert!(parse(&changed).is_err());
    }
    let mut extra = automatic.clone();
    extra["input"]["terms"] = json!({});
    assert!(parse(&extra).is_err());
    let raw = serde_json::to_string(automatic).unwrap();
    let duplicate=raw.replacen("\"request\":","\"request\":\"3131313131313131313131313131313131313131313131313131313131313131\",\"request\":",1);
    assert!(CapturedOrderAcceptance::parse(duplicate.as_bytes()).is_err());
}

#[test]
fn advanced_claims_are_positive_versioned_unique_closed_and_bounded_before_capture() {
    let data = cases();
    let wire = &data["cases"][1]["wire"];
    for (path, value) in [
        (
            "/input/allocation/allocations/0/quantity/amount",
            json!("0"),
        ),
        (
            "/input/allocation/allocations/0/quantity/amount",
            json!(250),
        ),
        (
            "/input/allocation/allocations/0/quantity",
            json!(["250", "g"]),
        ),
        (
            "/input/allocation/allocations/0/expected_pool_version",
            json!("0"),
        ),
        (
            "/input/allocation/allocations/0/expected_pool_version",
            json!(1),
        ),
        ("/input/allocation/allocations/0", json!([])),
    ] {
        let mut changed = wire.clone();
        *changed.pointer_mut(path).unwrap() = value;
        assert!(parse(&changed).is_err(), "{path}");
    }
    let mut duplicate = wire.clone();
    duplicate["input"]["allocation"]["allocations"][1] =
        duplicate["input"]["allocation"]["allocations"][0].clone();
    assert!(parse(&duplicate).is_err());
    let mut extra = wire.clone();
    extra["input"]["allocation"]["allocations"][0]["allow_deficit"] = json!(true);
    assert!(parse(&extra).is_err());
    let mut bounded = wire.clone();
    let values = (1..=64)
        .map(|index| {
            let mut value = wire["input"]["allocation"]["allocations"][0].clone();
            value["pool_id"] = json!(format!("{index:064x}"));
            value
        })
        .collect::<Vec<_>>();
    bounded["input"]["allocation"]["allocations"] = json!(values);
    assert_eq!(
        parse(&bounded)
            .unwrap()
            .input()
            .allocation()
            .explicit_allocations()
            .unwrap()
            .len(),
        64
    );
    bounded["input"]["allocation"]["allocations"].as_array_mut().unwrap().push(json!({"line_id":"25".repeat(32),"pool_id":format!("{:064x}",65),"quantity":{"amount":"1","unit":"g"},"expected_pool_version":"1"}));
    assert!(parse(&bounded).is_err());
}
