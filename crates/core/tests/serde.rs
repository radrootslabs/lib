#![cfg(feature = "serde")]

mod common;

use core::str::FromStr;
use std::collections::BTreeSet;

use radroots_core::{Currency, Decimal, Money, Percent, Quantity, QuantityPrice, Unit};
use serde_json::Value;

const SERIALIZATION_VECTORS: &str = include_str!("fixtures/value_serialization.v1.json");

#[test]
fn decimal_serializes_as_string() {
    let d = common::dec("1.2300");
    let json = serde_json::to_string(&d).unwrap();
    assert_eq!(json, "\"1.23\"");

    let back: Decimal = serde_json::from_str(&json).unwrap();
    assert_eq!(back, common::dec("1.23"));
}

#[test]
fn quantity_uses_decimal_str_and_omits_empty_label() {
    let q = Quantity::try_new(common::dec("1.2300"), Unit::MassKg).unwrap();
    let value = serde_json::to_value(&q).unwrap();

    assert_eq!(value["amount"], Value::String("1.23".to_string()));
    assert_eq!(value["unit"], Value::String("kg".to_string()));
    assert!(value.get("label").is_none());
}

#[test]
fn quantity_deserializes_decimal_str_via_serde_ext() {
    let raw = r#"{"amount":"1.2300","unit":"kg","label":"bag"}"#;
    let q: Quantity = serde_json::from_str(raw).unwrap();
    assert_eq!(q.amount(), common::dec("1.23"));
    assert_eq!(q.unit(), Unit::MassKg);
    assert_eq!(q.label(), Some("bag"));
}

#[test]
fn quantity_rejects_non_string_decimal_amount() {
    let raw = r#"{"amount":1.23,"unit":"kg"}"#;
    let err = serde_json::from_str::<Quantity>(raw).unwrap_err();
    assert!(err.to_string().contains("invalid type"));
}

#[test]
fn money_and_percent_roundtrip_with_strings() {
    let money = Money::try_new(common::dec("2.50"), Currency::USD).unwrap();
    let value = serde_json::to_value(&money).unwrap();
    assert_eq!(value["amount"], Value::String("2.5".to_string()));
    assert_eq!(value["currency"], Value::String("USD".to_string()));

    let pct = Percent::new(common::dec("12.5"));
    let value = serde_json::to_value(&pct).unwrap();
    assert_eq!(value["value"], Value::String("12.5".to_string()));
}

#[test]
fn native_value_deserialization_enforces_invariants() {
    let negative_money = r#"{"amount":"-0.01","currency":"USD"}"#;
    assert!(
        serde_json::from_str::<Money>(negative_money)
            .unwrap_err()
            .to_string()
            .contains("money amount")
    );

    let negative_quantity = r#"{"amount":"-1","unit":"each"}"#;
    assert!(
        serde_json::from_str::<Quantity>(negative_quantity)
            .unwrap_err()
            .to_string()
            .contains("quantity amount")
    );

    let zero_price =
        r#"{"amount":{"amount":"1","currency":"USD"},"quantity":{"amount":"0","unit":"each"}}"#;
    assert!(
        serde_json::from_str::<QuantityPrice>(zero_price)
            .unwrap_err()
            .to_string()
            .contains("greater than zero")
    );
}

#[test]
fn currency_serializes_as_code() {
    let c = Currency::from_str("usd").unwrap();
    let json = serde_json::to_string(&c).unwrap();
    assert_eq!(json, "\"USD\"");
}

#[test]
fn canonical_serialization_vectors_roundtrip_through_public_types() {
    let suite: Value = serde_json::from_str(SERIALIZATION_VECTORS).unwrap();
    assert_eq!(suite["suite"], "core_value_serialization");
    assert_eq!(suite["contract_version"], "1.0.0");

    let vectors = suite["vectors"].as_array().unwrap();
    let mut ids = BTreeSet::new();
    for vector in vectors {
        let id = vector["id"].as_str().unwrap();
        assert!(ids.insert(id), "duplicate vector id {id}");
        let input = vector["input"].clone();
        let actual = match vector["kind"].as_str().unwrap() {
            "core.decimal.serde" => {
                serde_json::to_value(serde_json::from_value::<Decimal>(input).unwrap()).unwrap()
            }
            "core.currency.serde" => {
                serde_json::to_value(serde_json::from_value::<Currency>(input).unwrap()).unwrap()
            }
            "core.money.serde" => {
                serde_json::to_value(serde_json::from_value::<Money>(input).unwrap()).unwrap()
            }
            "core.percent.serde" => {
                serde_json::to_value(serde_json::from_value::<Percent>(input).unwrap()).unwrap()
            }
            "core.quantity.serde" => {
                serde_json::to_value(serde_json::from_value::<Quantity>(input).unwrap()).unwrap()
            }
            "core.unit.serde" => {
                serde_json::to_value(serde_json::from_value::<Unit>(input).unwrap()).unwrap()
            }
            "core.quantity_price.serde" => {
                serde_json::to_value(serde_json::from_value::<QuantityPrice>(input).unwrap())
                    .unwrap()
            }
            kind => panic!("unsupported serialization vector kind {kind}"),
        };
        assert_eq!(actual, vector["expected"], "vector {id}");
    }
    assert_eq!(ids.len(), 9);
}
