#![cfg(all(feature = "serde_json", feature = "nostr"))]

use std::{borrow::Cow, fs, path::Path};

use radroots_event::{
    RadrootsEventEnvelope, RadrootsNip01EventWire, contract::identify_event_contract,
};
use radroots_event_codec::post::{
    admission::verify_and_admit_post_event,
    inbound::{RadrootsInboundPostProjection, RadrootsPostClassification, RadrootsPostDiagnostic},
};
use serde::Deserialize;
use serde_json::{Value, json};

const PACKAGED_VECTORS: &str = include_str!("fixtures/post_verified_profiles.v1.json");
const WORKSPACE_VECTOR_PATH: &str =
    "../../contracts/conformance/vectors/post/verified_profiles.v1.json";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";

#[derive(Debug, Deserialize)]
struct Suite {
    suite: String,
    contract_version: String,
    vectors: Vec<Vector>,
}

#[derive(Debug, Deserialize)]
struct Vector {
    id: String,
    kind: String,
    input: Value,
    expected: Value,
}

#[test]
fn raw_signed_vectors_execute_against_verified_post_admission() {
    let vectors = conformance_vectors();
    let suite: Suite = serde_json::from_str(&vectors).expect("verified post vectors must parse");
    assert_eq!(suite.suite, "post_profiles");
    assert_eq!(suite.contract_version, "1.0.0");
    assert!(!suite.vectors.is_empty());

    for vector in &suite.vectors {
        execute(vector);
    }
}

fn conformance_vectors() -> Cow<'static, str> {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(WORKSPACE_VECTOR_PATH);
    match fs::read_to_string(&workspace_path) {
        Ok(canonical) => {
            assert_eq!(
                canonical,
                PACKAGED_VECTORS,
                "packaged verified post vectors must match {}",
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
            Cow::Borrowed(PACKAGED_VECTORS)
        }
        Err(error) => panic!("failed to read {}: {error}", workspace_path.display()),
    }
}

fn execute(vector: &Vector) {
    let envelope = canonical_envelope(input_str(vector, "event_json"));
    match vector.kind.as_str() {
        "post.verify_and_admit.valid" => {
            let generic = identify_event_contract(
                envelope.kind_u32(),
                &envelope.tags_as_vec(),
                envelope.content(),
            )
            .expect("unsigned post identification remains available");
            assert_eq!(generic.id, "radroots.social.post.v1", "{}", vector.id);

            let admitted = verify_and_admit_post_event(envelope)
                .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
            assert_eq!(
                admitted.contract().id,
                expected_str(vector, "contract_id"),
                "{}",
                vector.id
            );
            assert_eq!(
                projection_value(admitted.projection()),
                vector.expected,
                "{}",
                vector.id
            );
            assert_eq!(
                admitted.projection().classification().is_root_card(),
                admitted.projection().classification() != RadrootsPostClassification::Reply,
                "{}",
                vector.id
            );
            let (verified, projection) = admitted.into_parts();
            assert_eq!(verified.event().kind_u32(), 1);
            assert_eq!(projection_value(&projection), vector.expected);
        }
        "post.verify_and_admit.invalid" => {
            let error = verify_and_admit_post_event(envelope)
                .expect_err("invalid signed post vector must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        kind => panic!("{} uses unsupported vector kind {kind}", vector.id),
    }
}

fn projection_value(projection: &RadrootsInboundPostProjection) -> Value {
    json!({
        "classification": classification_label(projection.classification()),
        "contract_id": projection.classification().contract_id(),
        "ask_marker": projection.ask_marker(),
        "diagnostics": diagnostic_codes(projection.diagnostics()),
        "imeta": projection.imeta().iter().map(|media| json!({
            "raw_fields": media.raw_fields(),
            "fallbacks": media.fallbacks(),
            "unknown_fields": media.unknown_fields(),
            "diagnostics": diagnostic_codes(media.diagnostics()),
            "qualifies_photo": media.qualifies_photo(),
        })).collect::<Vec<_>>(),
    })
}

fn classification_label(classification: RadrootsPostClassification) -> &'static str {
    match classification {
        RadrootsPostClassification::Reply => "reply",
        RadrootsPostClassification::Update => "update",
        RadrootsPostClassification::PhotoUpdate => "photo_update",
        RadrootsPostClassification::Ask => "ask",
        _ => "future",
    }
}

fn diagnostic_codes(diagnostics: &[RadrootsPostDiagnostic]) -> Vec<&'static str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code())
        .collect()
}

fn canonical_envelope(raw_json: &str) -> RadrootsEventEnvelope {
    RadrootsNip01EventWire::parse_json(raw_json)
        .expect("canonical raw event")
        .into_envelope()
        .expect("event envelope")
}

fn input_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector.input[field]
        .as_str()
        .unwrap_or_else(|| panic!("{} input.{field} must be a string", vector.id))
}

fn expected_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector.expected[field]
        .as_str()
        .unwrap_or_else(|| panic!("{} expected.{field} must be a string", vector.id))
}
