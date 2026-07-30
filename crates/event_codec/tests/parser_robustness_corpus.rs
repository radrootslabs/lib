#![cfg(feature = "json")]

use radroots_event::wire::{
    DEFAULT_EXTRA_MAX_FIELDS, DEFAULT_RAW_JSON_MAX_BYTES, DEFAULT_TAG_MAX_COUNT, EventWireError,
};
use radroots_event_codec::{DecodeError, decode, verify};
use serde_json::{Map, Value, json};

const VALID_ID: &str = "762bee187e9e645b81ec26ade05a69b5e8398caf527be8de0d9a45311ed0c7a0";
const VALID_PUBKEY: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const VALID_SIGNATURE: &str = "4290da0bb6422986647bc8cd5f63bd52d49f41e7b665d3b47105b8109183e8d596f322c531d4061df53e1d2b70fda12d5d1c14f3720d7a56d9d0a03746af5109";

fn valid_event() -> Value {
    json!({
        "id": VALID_ID,
        "pubkey": VALID_PUBKEY,
        "created_at": 1_800_000_100u64,
        "kind": 0u32,
        "tags": [],
        "content": "{\"display_name\":\"Moss Street Farm\",\"bot\":false,\"website\":\"https://mossstreet.example\",\"picture\":42}",
        "sig": VALID_SIGNATURE
    })
}

fn encode(value: &Value) -> String {
    serde_json::to_string(value).expect("corpus value must serialize")
}

fn object(value: &mut Value) -> &mut Map<String, Value> {
    value.as_object_mut().expect("event corpus object")
}

#[test]
fn corpus_rejects_oversized_input_before_json_allocation() {
    let raw = " ".repeat(DEFAULT_RAW_JSON_MAX_BYTES + 1);
    let error = decode::event(raw.as_str()).expect_err("oversized event");

    assert_eq!(error.code(), "input_too_large");
    assert_eq!(
        error,
        DecodeError::InputTooLarge {
            max: DEFAULT_RAW_JSON_MAX_BYTES,
            actual: DEFAULT_RAW_JSON_MAX_BYTES + 1,
        }
    );
}

#[test]
fn corpus_rejects_tag_count_overflow_before_id_verification() {
    let mut value = valid_event();
    object(&mut value).insert(
        "tags".to_owned(),
        Value::Array(vec![json!(["t"]); DEFAULT_TAG_MAX_COUNT + 1]),
    );
    let error = decode::event(encode(&value).as_str()).expect_err("tag count overflow");

    assert!(matches!(
        error,
        DecodeError::InvalidWire(EventWireError::TooManyTags {
            max: DEFAULT_TAG_MAX_COUNT,
            actual,
        }) if actual == DEFAULT_TAG_MAX_COUNT + 1
    ));
}

#[test]
fn corpus_bounds_unknown_fields_and_nested_json() {
    let mut accepted = valid_event();
    object(&mut accepted).insert("client".to_owned(), json!({"nested": [1, 2, 3]}));
    decode::event(encode(&accepted).as_str()).expect("bounded extension field");

    let mut too_many = valid_event();
    for index in 0..=DEFAULT_EXTRA_MAX_FIELDS {
        object(&mut too_many).insert(format!("extension_{index}"), Value::Null);
    }
    assert!(matches!(
        decode::event(encode(&too_many).as_str()),
        Err(DecodeError::InvalidWire(
            EventWireError::TooManyExtraFields {
                max: DEFAULT_EXTRA_MAX_FIELDS,
                actual,
            }
        )) if actual == DEFAULT_EXTRA_MAX_FIELDS + 1
    ));

    let nested = format!(
        "{{\"id\":\"{VALID_ID}\",\"pubkey\":\"{VALID_PUBKEY}\",\"created_at\":1800000100,\"kind\":0,\"tags\":[],\"content\":\"\",\"sig\":\"{VALID_SIGNATURE}\",\"extension\":{} }}",
        "[".repeat(160) + "0" + "]".repeat(160).as_str()
    );
    assert_eq!(
        decode::event(nested.as_str())
            .expect_err("excessive nesting")
            .code(),
        "invalid_json"
    );
}

#[test]
fn corpus_rejects_malformed_signature_and_identifier_shapes() {
    let mut malformed_signature = valid_event();
    object(&mut malformed_signature).insert("sig".to_owned(), json!("00"));
    assert!(matches!(
        decode::event(encode(&malformed_signature).as_str()),
        Err(DecodeError::InvalidWire(
            EventWireError::InvalidIdentifier { field: "sig", .. }
        ))
    ));

    let mut malformed_id = valid_event();
    object(&mut malformed_id).insert("id".to_owned(), json!("00"));
    assert!(matches!(
        decode::event(encode(&malformed_id).as_str()),
        Err(DecodeError::InvalidWire(
            EventWireError::InvalidIdentifier { field: "id", .. }
        ))
    ));
}

#[test]
fn corpus_keeps_identifier_mismatch_in_the_verifier_stage() {
    let mut mismatched = valid_event();
    object(&mut mismatched).insert("id".to_owned(), json!("a".repeat(64)));

    let raw = decode::event(encode(&mismatched).as_str()).expect("structurally valid event");
    let error = verify::id(raw).expect_err("identifier mismatch");

    assert_eq!(error.code(), "id_mismatch");
}
