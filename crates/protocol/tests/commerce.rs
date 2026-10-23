#![cfg(feature = "commerce")]
use radroots_domain::{CommerceContent, CommercePayload, TermsQuote};
use radroots_protocol::{
    commerce::{EncodedRecord, RECORD_MAX_BYTES, RecordWireError, decode},
    profile::COMMERCE_KIND,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/commerce_wire.v1.json")).unwrap()
}
fn body(row: &Value) -> Value {
    serde_json::from_str(row["wire"].as_str().unwrap()).unwrap()
}
fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}
fn objects(value: &Value, path: &str, result: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            result.push(path.into());
            for (key, value) in fields {
                objects(value, &format!("{path}/{key}"), result);
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                objects(value, &format!("{path}/{index}"), result);
            }
        }
        _ => {}
    }
}

#[test]
fn independent_complete_record_bytes_and_both_identity_domains_match_every_family() {
    let fixture = fixture();
    let rows = fixture["vectors"].as_array().unwrap();
    assert_eq!(rows.len(), 39);
    let mut types = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for row in rows {
        types.insert(row["type"].as_str().unwrap());
        let wire = row["wire"].as_str().unwrap();
        let record = decode(wire.as_bytes()).unwrap();
        assert_eq!(
            record.record_id().to_string(),
            row["record_id"].as_str().unwrap()
        );
        assert!(identities.insert(record.record_id().to_string()));
        let output = EncodedRecord::capture(record.content()).unwrap();
        assert_eq!(output.protected_wire(), wire.as_bytes(), "{}", row["name"]);
        assert_eq!(output.record_id(), record.record_id());
        assert_eq!(
            output.retained_bytes(),
            std::mem::size_of::<EncodedRecord>() + wire.len()
        );
        assert_ne!(
            output.record_id().to_string(),
            row["raw_preimage_sha256"].as_str().unwrap()
        );
        assert_ne!(
            output.record_id().to_string(),
            row["terms_domain_digest"].as_str().unwrap()
        );
        assert_eq!(format!("{output:?}"), "EncodedRecord(<redacted>)");
        assert_eq!(format!("{record:?}"), "DecodedRecord(<redacted>)");
        let mut value: Value = serde_json::from_slice(output.protected_wire()).unwrap();
        value.as_object_mut().unwrap().remove("record_id");
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            row["canonical_without_id"].as_str().unwrap()
        );
    }
    assert_eq!(types.len(), 19);
}

#[test]
fn all_known_objects_require_closed_duplicate_safe_complete_fields() {
    for row in fixture()["vectors"].as_array().unwrap().iter().take(19) {
        let original = body(row);
        let mut paths = Vec::new();
        objects(&original, "", &mut paths);
        for path in paths {
            let fields = original.pointer(&path).unwrap().as_object().unwrap();
            let mut array = original.clone();
            *array.pointer_mut(&path).unwrap() = Value::Array(fields.values().cloned().collect());
            assert!(
                matches!(decode(&bytes(&array)), Err(RecordWireError::InvalidInput)),
                "array {}:{path}",
                row["name"]
            );
            let mut extra = original.clone();
            extra
                .pointer_mut(&path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unexpected".into(), Value::Null);
            let outcome = decode(&bytes(&extra));
            assert!(
                matches!(outcome, Err(RecordWireError::InvalidInput)),
                "unknown {}:{path}: {outcome:?}",
                row["name"]
            );
            for key in fields.keys() {
                let mut missing = original.clone();
                missing
                    .pointer_mut(&path)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                assert!(
                    matches!(decode(&bytes(&missing)), Err(RecordWireError::InvalidInput)),
                    "missing {}:{path}/{key}",
                    row["name"]
                );
                let object = serde_json::to_string(original.pointer(&path).unwrap()).unwrap();
                let duplicate = format!(
                    "{{{}:{},{}",
                    serde_json::to_string(key).unwrap(),
                    serde_json::to_string(&fields[key]).unwrap(),
                    &object[1..]
                );
                let mut template = original.clone();
                *template.pointer_mut(&path).unwrap() = json!("__RECORD_NEGATIVE_TARGET__");
                let wire = serde_json::to_string(&template)
                    .unwrap()
                    .replace("\"__RECORD_NEGATIVE_TARGET__\"", &duplicate);
                assert!(
                    matches!(decode(wire.as_bytes()), Err(RecordWireError::InvalidInput)),
                    "duplicate {}:{path}/{key}",
                    row["name"]
                );
            }
        }
    }
}

#[test]
fn causal_party_role_and_exact_scalar_checks_precede_record_identity_acceptance() {
    for row in fixture()["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["name"] != "exact_private_record_limit")
    {
        let original = body(row);
        for parent in row["required_parents"].as_array().unwrap() {
            let mut missing = original.clone();
            missing["parents"]
                .as_array_mut()
                .unwrap()
                .retain(|p| p != parent);
            assert!(
                matches!(decode(&bytes(&missing)), Err(RecordWireError::InvalidInput)),
                "parent {}:{parent}",
                row["name"]
            );
        }
        for path in ["/author", "/buyer", "/seller"] {
            let mut uppercase = original.clone();
            *uppercase.pointer_mut(path).unwrap() = json!(
                original
                    .pointer(path)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_ascii_uppercase()
            );
            assert!(
                matches!(
                    decode(&bytes(&uppercase)),
                    Err(RecordWireError::InvalidInput)
                ),
                "key {path}"
            );
        }
        for (field, value) in [
            ("buyer", original["seller"].clone()),
            ("author_sequence", json!("0")),
            ("created_at", json!(450)),
            ("author_sequence", json!("01")),
        ] {
            let mut invalid = original.clone();
            invalid[field] = value;
            assert!(
                matches!(decode(&bytes(&invalid)), Err(RecordWireError::InvalidInput)),
                "{field}"
            );
        }
        if row["type"] != "quote" {
            let mut no_order = original.clone();
            no_order["order_id"] = Value::Null;
            assert!(matches!(
                decode(&bytes(&no_order)),
                Err(RecordWireError::InvalidInput)
            ));
        }
        if [
            "quote",
            "request",
            "decline",
            "agreement_committed",
            "withdrawal_requested",
            "withdrawal_acknowledged",
            "residual_released",
            "ready",
            "handover",
            "receipt",
            "resolution_committed",
        ]
        .contains(&row["type"].as_str().unwrap())
        {
            let mut wrong = original.clone();
            wrong["author"] = if original["author"] == original["buyer"] {
                original["seller"].clone()
            } else {
                original["buyer"].clone()
            };
            assert!(
                matches!(decode(&bytes(&wrong)), Err(RecordWireError::InvalidInput)),
                "role {}",
                row["name"]
            );
        }
    }
}

#[test]
fn exact_complete_record_limit_includes_header_id_and_payload_before_private_output() {
    assert_eq!(RECORD_MAX_BYTES, radroots_protocol::terms::TERMS_MAX_BYTES);
    let f = fixture();
    let exact = f["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "exact_private_record_limit")
        .unwrap();
    let record = decode(exact["wire"].as_str().unwrap().as_bytes()).unwrap();
    assert_eq!(
        EncodedRecord::capture(record.content())
            .unwrap()
            .protected_wire()
            .len(),
        RECORD_MAX_BYTES
    );
    for row in f["invalid"].as_array().unwrap() {
        assert_eq!(row["utf8_bytes"], RECORD_MAX_BYTES + 1);
        assert!(matches!(
            decode(row["wire"].as_str().unwrap().as_bytes()),
            Err(RecordWireError::LimitExceeded)
        ));
    }
    let terms_fixture: Value =
        serde_json::from_str(include_str!("fixtures/terms_wire.v1.json")).unwrap();
    let standalone = terms_fixture["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "exact_content_limit")
        .unwrap();
    let terms =
        radroots_protocol::terms::decode(standalone["wire"].as_str().unwrap().as_bytes()).unwrap();
    let content = CommerceContent::new(
        record.content().header().clone(),
        CommercePayload::Quote(TermsQuote::new(
            terms,
            "600".parse().unwrap(),
            Some(radroots_domain::SourceId::from_bytes([6; 32])),
        )),
    )
    .unwrap();
    assert!(matches!(
        EncodedRecord::capture(&content),
        Err(RecordWireError::LimitExceeded)
    ));
    let too_deep = format!("{}0{}", "[".repeat(17), "]".repeat(17));
    assert!(matches!(
        decode(too_deep.as_bytes()),
        Err(RecordWireError::LimitExceeded)
    ));
}

#[test]
fn identity_and_final_rumor_fields_are_distinct_checks_without_authentication() {
    let row = &fixture()["vectors"][2];
    let value = body(row);
    let record = decode(&bytes(&value)).unwrap();
    let output = EncodedRecord::capture(record.content()).unwrap();
    let header = record.content().header();
    assert!(
        record
            .check_rumor_fields(header.author(), header.created_at().get(), COMMERCE_KIND)
            .is_ok()
    );
    assert!(
        output
            .check_rumor_fields(header.author(), header.created_at().get(), COMMERCE_KIND)
            .is_ok()
    );
    for (author, time, kind) in [
        (header.buyer(), header.created_at().get(), COMMERCE_KIND),
        (
            header.author(),
            header.created_at().get() + 1,
            COMMERCE_KIND,
        ),
        (header.author(), header.created_at().get(), 14),
    ] {
        assert_eq!(
            record.check_rumor_fields(author, time, kind),
            Err(RecordWireError::CarrierMismatch)
        );
        assert_eq!(
            output.check_rumor_fields(author, time, kind),
            Err(RecordWireError::CarrierMismatch)
        );
    }
    let mut changed = value.clone();
    changed["payload"]["reason"] = json!("Changed private text");
    assert!(matches!(
        decode(&bytes(&changed)),
        Err(RecordWireError::IdentityMismatch)
    ));
    changed = value.clone();
    changed["record_id"] = json!("00".repeat(32));
    assert!(matches!(
        decode(&bytes(&changed)),
        Err(RecordWireError::IdentityMismatch)
    ));
    for (field, new) in [
        ("profile", json!("other.profile")),
        ("version", json!(2)),
        ("type", json!("future_record")),
    ] {
        let unknown = json!({"profile":value["profile"],"version":value["version"],"type":value["type"],"future_only":{"private":"uninterpreted"}});
        let mut unknown = unknown;
        unknown[field] = new;
        assert!(
            matches!(
                decode(&bytes(&unknown)),
                Err(RecordWireError::UnsupportedProfile)
            ),
            "{field}"
        );
    }
    for raw in [b"{".as_slice(), b"[]", b"null", b"\xff"] {
        assert!(decode(raw).is_err());
    }
}

#[test]
fn nested_future_vocabularies_and_collection_limits_never_become_partial_records() {
    let f = fixture();
    let quote = f["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "quote")
        .unwrap();
    let mut invalid_source = body(quote);
    invalid_source["payload"]["terms"]["lines"][0]["source"]["coordinate"]["kind"] = json!(30403);
    assert!(matches!(
        decode(&bytes(&invalid_source)),
        Err(RecordWireError::InvalidInput)
    ));
    for (kind, path) in [
        ("quote", "/payload/terms/cancellation"),
        ("quote", "/payload/terms/rounding"),
        ("quote", "/payload/terms/supply/type"),
        ("request", "/payload/terms/lines/0/price/measurement_unit"),
        ("terms_proposed", "/payload/terms/lines/0/option/price/type"),
        ("handover", "/payload/measurements/0/quantity/unit"),
        ("issue", "/payload/code"),
    ] {
        let row = f["vectors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == kind)
            .unwrap();
        let mut value = body(row);
        *value.pointer_mut(path).unwrap() = json!("future_variant");
        assert!(
            matches!(
                decode(&bytes(&value)),
                Err(RecordWireError::UnsupportedProfile)
            ),
            "{kind}:{path}"
        );
    }
    let row = f["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "handover")
        .unwrap();
    for path in ["/parents", "/payload/lines", "/payload/measurements"] {
        let mut value = body(row);
        *value.pointer_mut(path).unwrap() =
            Value::Array(vec![Value::Null; if path == "/parents" { 9 } else { 33 }]);
        assert!(
            matches!(decode(&bytes(&value)), Err(RecordWireError::InvalidInput)),
            "{path}"
        );
    }
    let row = f["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "resolution_proposed")
        .unwrap();
    for path in [
        "/payload/resolution/issues",
        "/payload/resolution/accepted_receipts",
        "/payload/resolution/release_residual",
    ] {
        let mut value = body(row);
        *value.pointer_mut(path).unwrap() = Value::Array(vec![Value::Null; 33]);
        assert!(
            matches!(decode(&bytes(&value)), Err(RecordWireError::InvalidInput)),
            "{path}"
        );
    }
}
