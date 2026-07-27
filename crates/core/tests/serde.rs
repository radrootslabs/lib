#![cfg(feature = "serde")]

mod common;

use core::str::FromStr;

use radroots_core::{Currency, Decimal, Money, Percent, Quantity, Unit};
use serde_json::Value;

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
    let q = Quantity::new(common::dec("1.2300"), Unit::MassKg);
    let value = serde_json::to_value(&q).unwrap();

    assert_eq!(value["amount"], Value::String("1.23".to_string()));
    assert_eq!(value["unit"], Value::String("kg".to_string()));
    assert!(value.get("label").is_none());
}

#[test]
fn quantity_deserializes_decimal_str_via_serde_ext() {
    let raw = r#"{"amount":"1.2300","unit":"kg","label":"bag"}"#;
    let q: Quantity = serde_json::from_str(raw).unwrap();
    assert_eq!(q.amount, common::dec("1.23"));
    assert_eq!(q.unit, Unit::MassKg);
    assert_eq!(q.label.as_deref(), Some("bag"));
}

#[test]
fn quantity_rejects_non_string_decimal_amount() {
    let raw = r#"{"amount":1.23,"unit":"kg"}"#;
    let err = serde_json::from_str::<Quantity>(raw).unwrap_err();
    assert!(err.to_string().contains("invalid type"));
}

#[test]
fn money_and_percent_roundtrip_with_strings() {
    let money = Money::new(common::dec("2.50"), Currency::USD);
    let value = serde_json::to_value(&money).unwrap();
    assert_eq!(value["amount"], Value::String("2.5".to_string()));
    assert_eq!(value["currency"], Value::String("USD".to_string()));

    let pct = Percent::new(common::dec("12.5"));
    let value = serde_json::to_value(&pct).unwrap();
    assert_eq!(value["value"], Value::String("12.5".to_string()));
}

#[test]
fn currency_serializes_as_code() {
    let c = Currency::from_str("usd").unwrap();
    let json = serde_json::to_string(&c).unwrap();
    assert_eq!(json, "\"USD\"");
}
