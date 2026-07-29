#![cfg(all(feature = "serde_json", feature = "nostr"))]

use std::{borrow::Cow, fs, path::Path};

use radroots_event::{envelope::RadrootsEventEnvelope, wire::RadrootsNip01EventWire};
use radroots_event_codec::{
    admission::{RadrootsAdmittedEvent, RadrootsEventAdmissionError, admit_verified_event},
    verification::verify_nip01_event,
};
use serde::Deserialize;
use serde_json::Value;

const PACKAGED_VECTORS: &str = include_str!("fixtures/verified_admission.v1.json");
const WORKSPACE_VECTOR_PATH: &str =
    "../../contracts/conformance/vectors/event/verified_admission.v1.json";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";

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
fn fixed_signed_vectors_execute_the_complete_verified_admission_boundary() {
    let vectors = conformance_vectors();
    let suite: Suite =
        serde_json::from_str(&vectors).expect("verified admission vectors must parse");
    assert_eq!(suite.suite, "verified_event_admission");
    assert_eq!(suite.contract_version, "1.0.0");
    assert_eq!(suite.vectors.len(), 13);

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
                "packaged verified admission vectors must match {}",
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
    let envelope = event_envelope(&vector.input["event"]);
    let event_id = envelope.id_hex().to_owned();
    let verified = verify_nip01_event(envelope)
        .unwrap_or_else(|error| panic!("{} fixture is not NIP-01 verified: {error}", vector.id));
    let result = admit_verified_event(verified);

    match vector.kind.as_str() {
        "event.admit_verified.valid" => {
            let admitted = result.unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
            assert_eq!(
                admitted_variant(&admitted),
                expected_str(vector, "variant"),
                "{}",
                vector.id
            );
            assert_eq!(
                admitted.contract_id(),
                expected_str(vector, "contract_id"),
                "{}",
                vector.id
            );
            assert_eq!(
                admitted.event().id_hex(),
                expected_str(vector, "event_id"),
                "{}",
                vector.id
            );
            assert_eq!(admitted.into_verified_event().event().id_hex(), event_id);
        }
        "event.admit_verified.invalid" => {
            let error = match result {
                Err(error) => error,
                Ok(_) => panic!("{} unexpectedly admitted", vector.id),
            };
            assert_eq!(
                admission_error_variant(&error),
                expected_str(vector, "error_variant"),
                "{}",
                vector.id
            );
            assert_eq!(
                error.code(),
                expected_str(vector, "error_code"),
                "{}",
                vector.id
            );
            assert_eq!(event_id, expected_str(vector, "event_id"), "{}", vector.id);
        }
        kind => panic!("{} uses unsupported vector kind {kind}", vector.id),
    }
}

fn admitted_variant(admitted: &RadrootsAdmittedEvent) -> &'static str {
    match admitted {
        RadrootsAdmittedEvent::Profile(_) => "profile",
        RadrootsAdmittedEvent::RootPost(_) => "root_post",
        RadrootsAdmittedEvent::Reply(_) => "reply",
        RadrootsAdmittedEvent::Comment(_) => "comment",
        RadrootsAdmittedEvent::DeletionRequest(_) => "deletion_request",
        RadrootsAdmittedEvent::FoodAvailability(_) => "food_availability",
        RadrootsAdmittedEvent::ContractValidated(_) => "contract_validated",
        _ => panic!("conformance runner must be updated for a new admission variant"),
    }
}

fn admission_error_variant(error: &RadrootsEventAdmissionError) -> &'static str {
    match error {
        RadrootsEventAdmissionError::ContractMatch(_) => "contract_match",
        RadrootsEventAdmissionError::ContractValidation(_) => "contract_validation",
        RadrootsEventAdmissionError::Profile(_) => "profile",
        RadrootsEventAdmissionError::Post(_) => "post",
        RadrootsEventAdmissionError::Reply(_) => "reply",
        RadrootsEventAdmissionError::Comment(_) => "comment",
        RadrootsEventAdmissionError::DeletionRequest(_) => "deletion_request",
        RadrootsEventAdmissionError::FoodAvailability(_) => "food_availability",
        _ => panic!("conformance runner must be updated for a new admission error variant"),
    }
}

fn event_envelope(value: &Value) -> RadrootsEventEnvelope {
    let raw_json = serde_json::to_string(value).expect("serialize fixed event");
    RadrootsNip01EventWire::parse_json(&raw_json)
        .expect("fixed signed event wire")
        .into_envelope()
        .expect("fixed signed event envelope")
}

fn expected_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector.expected[field]
        .as_str()
        .unwrap_or_else(|| panic!("{}.expected.{field} must be a string", vector.id))
}
