#![cfg(all(feature = "std", feature = "serde", feature = "serde_json"))]

use nostr::secp256k1::Message;
use nostr::{Event as NostrEvent, JsonUtil, Keys, SECP256K1};
use radroots_event::{RadrootsNip01EventWire, ids::RadrootsIdParseError};
use radroots_event_codec::verification::{RadrootsSignatureVerifiedEvent, verify_nip01_event};
use radroots_nostr::prelude::radroots_event_from_nostr;
use radroots_trade::operational_listing::{
    parse_classified_listing_address, validation::validate_operational_listing_event,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{borrow::Cow, collections::BTreeSet, fs, path::Path};

const SIGNING_SECRET_KEY: &str = "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
const PACKAGED_ADDRESS_VECTORS: &str =
    include_str!("fixtures/parse_classified_listing_address.v1.json");
const PACKAGED_VALIDATION_VECTORS: &str =
    include_str!("fixtures/validate_operational_listing_event.v1.json");
const WORKSPACE_ADDRESS_VECTOR_PATH: &str =
    "../../contracts/conformance/vectors/trade/parse_classified_listing_address.v1.json";
const WORKSPACE_VALIDATION_VECTOR_PATH: &str = "../../contracts/conformance/vectors/trade_validation/validate_operational_listing_event.v1.json";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";

const ADDRESS_VECTOR_IDS: [&str; 7] = [
    "trade_parse_classified_listing_address_canonical_001",
    "trade_parse_classified_listing_address_normalized_002",
    "trade_parse_classified_listing_address_wrong_kind_003",
    "trade_parse_classified_listing_address_malformed_format_004",
    "trade_parse_classified_listing_address_short_pubkey_005",
    "trade_parse_classified_listing_address_non_hex_pubkey_006",
    "trade_parse_classified_listing_address_invalid_d_tag_007",
];

const VALIDATION_VECTOR_IDS: [&str; 6] = [
    "trade_validation_validate_operational_listing_event_valid_001",
    "trade_validation_validate_operational_listing_event_invalid_seller_002",
    "trade_validation_validate_operational_listing_event_missing_inventory_003",
    "trade_validation_validate_operational_listing_event_focused_profile_004",
    "trade_validation_validate_operational_listing_event_generic_nip99_005",
    "trade_validation_validate_operational_listing_event_ambiguous_profile_006",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Suite {
    suite: String,
    contract_version: String,
    vectors: Vec<Vector>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Vector {
    id: String,
    kind: String,
    input: Value,
    expected: Value,
}

#[test]
fn classified_listing_address_vectors_execute_against_public_api() {
    let raw = conformance_vectors(
        PACKAGED_ADDRESS_VECTORS,
        WORKSPACE_ADDRESS_VECTOR_PATH,
        "Classified Listing address",
    );
    let suite: Suite = serde_json::from_str(&raw).expect("address vectors must parse");
    assert_suite(&suite, "trade", &ADDRESS_VECTOR_IDS);

    for vector in &suite.vectors {
        match vector.kind.as_str() {
            "trade.parse_classified_listing_address.valid" => address_valid(vector),
            "trade.parse_classified_listing_address.invalid" => address_invalid(vector),
            kind => panic!("{} uses unsupported vector kind {kind}", vector.id),
        }
    }
}

#[test]
fn operational_listing_validation_vectors_execute_verified_nip01_events() {
    let raw = conformance_vectors(
        PACKAGED_VALIDATION_VECTORS,
        WORKSPACE_VALIDATION_VECTOR_PATH,
        "Operational Listing validation",
    );
    let suite: Suite = serde_json::from_str(&raw).expect("validation vectors must parse");
    assert_suite(&suite, "trade_validation", &VALIDATION_VECTOR_IDS);
    let keys = Keys::parse(SIGNING_SECRET_KEY).expect("fixed signing key must parse");

    for vector in &suite.vectors {
        let event = verified_event(vector, &keys);
        match vector.kind.as_str() {
            "trade_validation.validate_operational_listing_event.valid" => {
                validation_valid(vector, &event)
            }
            "trade_validation.validate_operational_listing_event.invalid" => {
                validation_invalid(vector, &event)
            }
            kind => panic!("{} uses unsupported vector kind {kind}", vector.id),
        }
    }
}

fn conformance_vectors(
    packaged: &'static str,
    workspace_relative: &str,
    label: &str,
) -> Cow<'static, str> {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(workspace_relative);
    match fs::read_to_string(&workspace_path) {
        Ok(canonical) => {
            assert_eq!(
                canonical,
                packaged,
                "packaged {label} vectors must match {}",
                workspace_path.display()
            );
            Cow::Owned(canonical)
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && !Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(WORKSPACE_CONTRACT_MARKER_PATH)
                    .is_file() =>
        {
            Cow::Borrowed(packaged)
        }
        Err(error) => panic!("failed to read {}: {error}", workspace_path.display()),
    }
}

fn assert_suite(suite: &Suite, expected_suite: &str, expected_ids: &[&str]) {
    assert_eq!(suite.suite, expected_suite);
    assert_eq!(suite.contract_version, "1.0.0");
    assert_eq!(suite.vectors.len(), expected_ids.len());

    let actual: BTreeSet<&str> = suite
        .vectors
        .iter()
        .map(|vector| vector.id.as_str())
        .collect();
    let expected: BTreeSet<&str> = expected_ids.iter().copied().collect();
    assert_eq!(actual, expected, "conformance vector inventory drift");
}

fn address_valid(vector: &Vector) {
    assert_object_keys(&vector.input, &["listing_addr"], "input", &vector.id);
    assert_object_keys(
        &vector.expected,
        &["address", "kind", "seller_pubkey", "listing_id"],
        "expected",
        &vector.id,
    );
    let parsed = parse_classified_listing_address(input_str(vector, "listing_addr"))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    let actual = json!({
        "address": parsed.address.as_str(),
        "kind": parsed.kind,
        "seller_pubkey": parsed.seller_pubkey.to_hex(),
        "listing_id": parsed.listing_id.as_str(),
    });
    assert_eq!(actual, vector.expected, "{}", vector.id);
}

fn address_invalid(vector: &Vector) {
    assert_object_keys(&vector.input, &["listing_addr"], "input", &vector.id);
    assert_object_keys(
        &vector.expected,
        &["error", "message"],
        "expected",
        &vector.id,
    );
    let error = parse_classified_listing_address(input_str(vector, "listing_addr"))
        .expect_err("invalid Classified Listing address vector must fail");
    let actual = json!({
        "error": address_error_value(&error),
        "message": error.to_string(),
    });
    assert_eq!(actual, vector.expected, "{}", vector.id);
}

fn address_error_value(error: &RadrootsIdParseError) -> Value {
    match error {
        RadrootsIdParseError::Empty => json!({ "kind": "empty" }),
        RadrootsIdParseError::InvalidFormat => json!({ "kind": "invalid_format" }),
        RadrootsIdParseError::InvalidLength { expected, actual } => json!({
            "kind": "invalid_length",
            "expected": expected,
            "actual": actual,
        }),
        RadrootsIdParseError::InvalidCharacter => json!({ "kind": "invalid_character" }),
        RadrootsIdParseError::InvalidPublicKey => json!({ "kind": "invalid_public_key" }),
        RadrootsIdParseError::UnexpectedKind { expected, actual } => json!({
            "kind": "unexpected_kind",
            "expected": expected,
            "actual": actual,
        }),
        RadrootsIdParseError::TooLong { max, actual } => json!({
            "kind": "too_long",
            "max": max,
            "actual": actual,
        }),
    }
}

fn verified_event(vector: &Vector, keys: &Keys) -> RadrootsSignatureVerifiedEvent {
    assert_object_keys(&vector.input, &["event"], "input", &vector.id);
    let event_value = vector
        .input
        .get("event")
        .unwrap_or_else(|| panic!("{} is missing input.event", vector.id));
    assert_object_keys(
        event_value,
        &[
            "id",
            "pubkey",
            "created_at",
            "kind",
            "tags",
            "content",
            "sig",
        ],
        "input.event",
        &vector.id,
    );
    let raw = serde_json::to_string(event_value).expect("event vector must serialize");
    let event = NostrEvent::from_json(&raw)
        .unwrap_or_else(|error| panic!("{} is not a NIP-01 event: {error}", vector.id));
    let expected_id = value_str(event_value, "id", &vector.id);
    let expected_signature = value_str(event_value, "sig", &vector.id);

    assert_eq!(event.id.to_string(), expected_id, "{}", vector.id);
    assert_eq!(event.sig.to_string(), expected_signature, "{}", vector.id);
    assert_eq!(
        event.pubkey,
        keys.public_key(),
        "{} signer drift",
        vector.id
    );
    let message = Message::from_digest(event.id.to_bytes());
    let deterministic_signature =
        SECP256K1.sign_schnorr_no_aux_rand(&message, keys.key_pair(SECP256K1));
    assert_eq!(
        event.sig, deterministic_signature,
        "{} deterministic signature drift",
        vector.id
    );
    event
        .verify()
        .unwrap_or_else(|error| panic!("{} failed NIP-01 verification: {error}", vector.id));

    let wire = RadrootsNip01EventWire::parse_json(&raw)
        .unwrap_or_else(|error| panic!("{} failed Radroots wire parsing: {error}", vector.id));
    assert!(
        wire.extra.is_empty(),
        "{} has extra NIP-01 fields",
        vector.id
    );
    assert_eq!(wire.id, expected_id, "{}", vector.id);
    assert_eq!(wire.sig, expected_signature, "{}", vector.id);
    wire.verify_id()
        .unwrap_or_else(|error| panic!("{} failed Radroots ID verification: {error}", vector.id));
    let envelope = wire
        .into_envelope()
        .unwrap_or_else(|error| panic!("{} failed envelope conversion: {error}", vector.id));
    let adapted = radroots_event_from_nostr(&event)
        .unwrap_or_else(|error| panic!("{} failed Nostr adapter conversion: {error}", vector.id));
    assert_eq!(envelope, adapted, "{} conversion drift", vector.id);
    verify_nip01_event(envelope)
        .unwrap_or_else(|error| panic!("{} failed typed verification: {error}", vector.id))
}

fn validation_valid(vector: &Vector, event: &RadrootsSignatureVerifiedEvent) {
    assert_object_keys(&vector.expected, &["projection"], "expected", &vector.id);
    let projection = validate_operational_listing_event(event)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(
        projection.listing.farm.pubkey,
        event.event().author().to_hex(),
        "{} decoded farm author drift",
        vector.id
    );
    let actual = serde_json::to_value(projection).expect("projection must serialize");
    assert_eq!(actual, vector.expected["projection"], "{}", vector.id);
}

fn validation_invalid(vector: &Vector, event: &RadrootsSignatureVerifiedEvent) {
    assert_object_keys(
        &vector.expected,
        &["error", "message"],
        "expected",
        &vector.id,
    );
    let error = validate_operational_listing_event(event)
        .expect_err("invalid Operational Listing validation vector must fail");
    let actual = json!({
        "error": serde_json::to_value(&error).expect("validation error must serialize"),
        "message": error.to_string(),
    });
    assert_eq!(actual, vector.expected, "{}", vector.id);
}

fn input_str<'a>(vector: &'a Vector, key: &str) -> &'a str {
    value_str(&vector.input, key, &vector.id)
}

fn value_str<'a>(value: &'a Value, key: &str, vector_id: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{vector_id} is missing string field {key}"))
}

fn assert_object_keys(value: &Value, expected: &[&str], path: &str, vector_id: &str) {
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("{vector_id} {path} must be an object"));
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "{vector_id} {path} key drift");
}
