#![cfg(feature = "terms")]
use radroots_protocol::terms::{EncodedTerms, TermsWireError, decode};
use serde_json::{Value, json};

fn fixtures() -> Value {
    serde_json::from_str(include_str!("fixtures/terms_wire.v1.json")).unwrap()
}
fn example(index: usize) -> Value {
    serde_json::from_str(fixtures()["vectors"][index]["wire"].as_str().unwrap()).unwrap()
}
fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

#[test]
fn independently_authored_complete_terms_decode_and_reencode_without_changed_bytes_or_digests() {
    for vector in fixtures()["vectors"].as_array().unwrap() {
        let wire = vector["wire"].as_str().unwrap();
        let terms = decode(wire.as_bytes()).unwrap();
        let encoded = EncodedTerms::capture(&terms).unwrap();
        assert_eq!(encoded.protected_wire(), wire.as_bytes());
        assert_eq!(
            encoded.digest().to_string(),
            vector["terms_digest"].as_str().unwrap()
        );
        let pretty =
            serde_json::to_string_pretty(&serde_json::from_str::<Value>(wire).unwrap()).unwrap();
        if pretty.len() <= 16_384 {
            assert_eq!(
                EncodedTerms::capture(&decode(pretty.as_bytes()).unwrap())
                    .unwrap()
                    .digest(),
                encoded.digest()
            );
        }
    }
}

#[test]
fn every_terms_object_boundary_is_closed_required_and_object_only() {
    for (index, paths) in [
        (
            0,
            vec![
                "",
                "/lines/0",
                "/lines/0/source",
                "/lines/0/source/coordinate",
                "/lines/0/product",
                "/lines/0/option",
                "/lines/0/option/unit",
                "/lines/0/option/unit/contents",
                "/lines/0/option/unit/contents/quantity",
                "/lines/0/option/price",
                "/lines/0/option/price/rate",
                "/lines/0/option/price/tiers/0",
                "/lines/0/option/price/tiers/0/rate",
                "/lines/0/option/supply",
                "/lines/0/option/supply/window",
                "/lines/0/price",
                "/lines/0/price/rate",
                "/lines/0/maximum_line_total",
                "/fulfillment/0",
                "/fulfillment/0/terms",
                "/fulfillment/0/terms/window",
                "/fulfillment_charge",
                "/maximum_total",
                "/supply",
            ],
        ),
        (
            1,
            vec![
                "/lines/0/option/price",
                "/lines/0/option/price/rate",
                "/lines/0/option/price/maximum_per_sale_unit_total",
                "/lines/0/price",
                "/lines/0/price/rate",
                "/lines/0/price/maximum_total",
                "/supply",
                "/supply/expected_window",
            ],
        ),
    ] {
        let original = example(index);
        for path in paths {
            let fields = original.pointer(path).unwrap().as_object().unwrap();
            let mut positional = original.clone();
            *positional.pointer_mut(path).unwrap() =
                Value::Array(fields.values().cloned().collect());
            assert!(
                decode(&bytes(&positional)).is_err(),
                "positional {index}:{path}"
            );
            let mut extra = original.clone();
            extra
                .pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unexpected".into(), json!(null));
            assert!(decode(&bytes(&extra)).is_err(), "unknown {index}:{path}");
            for key in fields.keys() {
                let mut missing = original.clone();
                missing
                    .pointer_mut(path)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                assert!(
                    decode(&bytes(&missing)).is_err(),
                    "missing {index}:{path}/{key}"
                );
                // Duplicate the field in its exact object before inserting it
                // into the surrounding fixture; no Value parser may erase it.
                let object = if path.is_empty() {
                    serde_json::to_string(&original).unwrap()
                } else {
                    serde_json::to_string(original.pointer(path).unwrap()).unwrap()
                };
                let duplicate = format!(
                    "{{{}:{},{}",
                    serde_json::to_string(key).unwrap(),
                    serde_json::to_string(&fields[key]).unwrap(),
                    &object[1..]
                );
                let mut template = original.clone();
                *template.pointer_mut(path).unwrap() = json!("__TERMS_NEGATIVE_TARGET__");
                let altered = serde_json::to_string(&template)
                    .unwrap()
                    .replace("\"__TERMS_NEGATIVE_TARGET__\"", &duplicate);
                assert!(
                    decode(altered.as_bytes()).is_err(),
                    "duplicate {index}:{path}/{key}"
                );
            }
        }
    }
}

#[test]
fn private_text_decodes_escaped_unicode_without_normalizing_it_or_omitting_explicit_nulls() {
    let original = example(0);
    let wire = serde_json::to_string(&original).unwrap();
    let escaped = wire.replace("🥬", r"\uD83E\uDD6C").replace("Å", r"\u00c5");
    assert_ne!(wire, escaped);
    assert_eq!(
        EncodedTerms::capture(&decode(wire.as_bytes()).unwrap())
            .unwrap()
            .digest(),
        EncodedTerms::capture(&decode(escaped.as_bytes()).unwrap())
            .unwrap()
            .digest()
    );
    for (path, value) in [
        ("/fulfillment/0/terms/address", json!(true)),
        ("/fulfillment/0/terms/contact", json!(12)),
        ("/lines/0/source_lot_label", json!(false)),
        ("/fulfillment/0/terms/instructions", json!("a".repeat(2049))),
    ] {
        let mut invalid = original.clone();
        *invalid.pointer_mut(path).unwrap() = value;
        assert!(decode(&bytes(&invalid)).is_err(), "{path}");
    }
}

#[test]
fn domain_invariants_and_unsupported_profiles_never_become_silent_fallbacks() {
    let original = example(0);
    for (path, value) in [
        ("/buyer", original["seller"].clone()),
        ("/lines/0/amount", json!("0")),
        ("/lines/0/option_id", json!("00".repeat(32))),
        ("/lines/0/source/coordinate/kind", json!(95938)),
        (
            "/lines/0/source/coordinate/author",
            original["buyer"].clone(),
        ),
        ("/lines/0/price/discount_bps", json!(10001)),
        ("/lines/0/maximum_line_total/minor", json!("438")),
        ("/maximum_total/minor", json!("465")),
        ("/fulfillment_charge/exponent", json!(0)),
        ("/fulfillment/0/terms/window/end", json!("300")),
        (
            "/fulfillment/0/terms/window/timezone",
            json!("invalid/zone"),
        ),
        ("/lines/0/price/rate/minor", json!(150)),
        ("/lines/0/price/basis", json!(1)),
    ] {
        let mut invalid = original.clone();
        *invalid.pointer_mut(path).unwrap() = value;
        assert!(decode(&bytes(&invalid)).is_err(), "{path}");
    }
    for path in [
        "/cancellation",
        "/rounding",
        "/supply/type",
        "/lines/0/price/type",
        "/lines/0/option/price/type",
        "/lines/0/option/supply/type",
        "/lines/0/option/unit/type",
        "/lines/0/option/unit/contents/type",
        "/lines/0/option/unit/contents/quantity/unit",
        "/fulfillment/0/terms/mode",
    ] {
        let mut unknown = original.clone();
        *unknown.pointer_mut(path).unwrap() = json!("future_profile");
        assert!(
            matches!(
                decode(&bytes(&unknown)),
                Err(TermsWireError::UnsupportedProfile)
            ),
            "{path}"
        );
    }
    let mut variable = example(1);
    variable["lines"][0]["price"]["measurement_unit"] = json!("future_unit");
    assert!(matches!(
        decode(&bytes(&variable)),
        Err(TermsWireError::UnsupportedProfile)
    ));
}

#[test]
fn public_snapshot_alternatives_preserve_explicit_values_without_invented_defaults() {
    for unit in ["g", "kg", "mL", "L", "each"] {
        let mut value = example(0);
        value["lines"][0]["product"]["producer"] = Value::Null;
        value["lines"][0]["product"]["origin_summary"] = Value::Null;
        value["lines"][0]["option"]["unit"] = json!({"type":"measured","unit":unit});
        value["lines"][0]["option"]["supply"] = json!({"type":"available","amount":"0"});
        let terms = decode(&bytes(&value)).unwrap();
        let output: Value =
            serde_json::from_slice(EncodedTerms::capture(&terms).unwrap().protected_wire())
                .unwrap();
        assert_eq!(output, value);
        value["lines"][0]["option"]["unit"]["unit"] = json!("future_unit");
        assert!(matches!(
            decode(&bytes(&value)),
            Err(TermsWireError::UnsupportedProfile)
        ));
    }
    for unit in ["g", "kg", "mL", "L"] {
        let mut value = example(1);
        value["lines"][0]["price"]["measurement_unit"] = json!(unit);
        let terms = decode(&bytes(&value)).unwrap();
        let output: Value =
            serde_json::from_slice(EncodedTerms::capture(&terms).unwrap().protected_wire())
                .unwrap();
        assert_eq!(output, value);
    }
}

#[test]
fn byte_depth_and_collection_budgets_bound_terms_before_domain_materialization() {
    let fixture = fixtures();
    let exact = fixture["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "exact_content_limit")
        .unwrap()["wire"]
        .as_str()
        .unwrap();
    assert!(decode(exact.as_bytes()).is_ok());
    assert!(matches!(
        decode(format!("{exact} ").as_bytes()),
        Err(TermsWireError::LimitExceeded)
    ));
    let original = example(0);
    for (field, count) in [("lines", 33), ("fulfillment", 9)] {
        let mut invalid = original.clone();
        invalid[field] = Value::Array(vec![Value::Null; count]);
        assert!(decode(&bytes(&invalid)).is_err());
    }
    for field in ["lines", "fulfillment"] {
        let mut invalid = original.clone();
        let duplicate = invalid[field][0].clone();
        invalid[field].as_array_mut().unwrap().push(duplicate);
        assert!(decode(&bytes(&invalid)).is_err());
    }
    let too_deep = format!("{}0{}", "[".repeat(17), "]".repeat(17));
    assert!(matches!(
        decode(too_deep.as_bytes()),
        Err(TermsWireError::LimitExceeded)
    ));
    for raw in [b"{".as_slice(), b"[]", b"null", b"\xff"] {
        assert!(decode(raw).is_err());
    }
}
