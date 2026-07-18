#![cfg(all(feature = "serde_json", feature = "nostr"))]

use std::{borrow::Cow, fs, path::Path};

use radroots_event::event_head::{
    RadrootsCurrentEventHead, RadrootsEventHeadCandidate, RadrootsEventHeadCandidateResult,
    RadrootsEventHeadDecision, event_head_candidate_for_event, select_event_head,
};
use radroots_event::{RadrootsEventEnvelope, RadrootsEventEnvelopeParts, RadrootsNip01EventWire};
use radroots_event_codec::profile::admission::{
    RadrootsAdmittedProfileEvent, RadrootsProfileAdmissionError, verify_and_admit_profile_event,
};
use radroots_event_codec::profile::inbound::RadrootsProfileMetadataParseError;
use radroots_event_codec::verification::{RadrootsNip01VerificationError, verify_nip01_event};
use serde::Deserialize;
use serde_json::{Map, Value};

const PACKAGED_VECTORS: &str = include_str!("fixtures/profile_verified_event.v1.json");
const WORKSPACE_VECTOR_PATH: &str =
    "../../contracts/conformance/vectors/profile/verified_event.v1.json";
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEvent {
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
}

#[test]
fn raw_signed_vectors_execute_against_verified_event_and_profile_boundaries() {
    let vectors = conformance_vectors();
    let suite: Suite = serde_json::from_str(&vectors).expect("verified Profile vectors must parse");
    assert_eq!(suite.suite, "verified_profile_event");
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
                "packaged verified Profile vectors must match {}",
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
    match vector.kind.as_str() {
        "event.verify_nip01.valid" => verify_valid(vector),
        "event.verify_nip01.invalid_id" => verify_invalid_id(vector),
        "event.verify_nip01.invalid_signature" => verify_invalid_signature(vector),
        "event.verify_nip01.malformed_envelope" => verify_malformed_envelope(vector),
        "event.verify_nip01.kind_overflow" => verify_kind_overflow(vector),
        "profile.verify_and_admit.valid" => profile_admit_valid(vector),
        "profile.verify_and_admit.invalid_kind" => profile_admit_invalid_kind(vector),
        "profile.verify_and_admit.invalid_metadata" => profile_admit_invalid_metadata(vector),
        "profile.select_equal_time_head" => profile_select_equal_time_head(vector),
        kind => panic!("{} uses unsupported vector kind {kind}", vector.id),
    }
}

fn verify_valid(vector: &Vector) {
    let verified = verify_nip01_event(canonical_envelope(input_str(vector, "event_json")))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(
        verified.event().id_str(),
        expected_str(vector, "event_id"),
        "{}",
        vector.id
    );
    assert_eq!(
        u64::from(verified.event().kind_u32()),
        vector.expected["kind"].as_u64().expect("expected.kind"),
        "{}",
        vector.id
    );
}

fn verify_invalid_id(vector: &Vector) {
    let error = verify_nip01_event(unchecked_id_envelope(input_str(vector, "event_json")))
        .expect_err("mismatched event id must fail");
    assert_eq!(error.code(), expected_str(vector, "error"));
    assert!(matches!(
        error,
        RadrootsNip01VerificationError::IdMismatch { .. }
    ));
}

fn verify_invalid_signature(vector: &Vector) {
    let error = verify_nip01_event(canonical_envelope(input_str(vector, "event_json")))
        .expect_err("invalid event signature must fail");
    assert_eq!(error.code(), expected_str(vector, "error"));
    assert_eq!(error, RadrootsNip01VerificationError::SignatureInvalid);
}

fn verify_malformed_envelope(vector: &Vector) {
    let error = verify_nip01_event(canonical_envelope(input_str(vector, "event_json")))
        .expect_err("invalid secp256k1 public key must fail envelope conversion");
    assert_eq!(error.code(), expected_str(vector, "error"));
    assert_eq!(error, RadrootsNip01VerificationError::MalformedEnvelope);
}

fn verify_kind_overflow(vector: &Vector) {
    let error = verify_nip01_event(canonical_envelope(input_str(vector, "event_json")))
        .expect_err("out-of-range event kind must fail");
    assert_eq!(error.code(), expected_str(vector, "error"));
    assert!(matches!(
        error,
        RadrootsNip01VerificationError::KindOutOfRange { kind }
            if u64::from(kind) == vector.expected["kind"].as_u64().expect("expected.kind")
    ));
}

fn profile_admit_valid(vector: &Vector) {
    let admitted = admitted_event(input_str(vector, "event_json"), &vector.id);
    assert_eq!(admitted.event().id_str(), expected_str(vector, "event_id"));
    assert_eq!(
        admitted.verified_event().event().id_str(),
        expected_str(vector, "event_id")
    );
    assert_eq!(
        serde_json::to_value(admitted.event().tags_as_vec()).expect("tags"),
        vector.expected["tags"]
    );
    assert_eq!(projected_metadata(&admitted), vector.expected["projected"]);
    assert_eq!(
        serde_json::to_value(admitted.metadata().residual_fields()).expect("residual fields"),
        vector.expected["residual_fields"]
    );
    let (verified, metadata) = admitted.into_parts();
    assert_eq!(verified.event().id_str(), expected_str(vector, "event_id"));
    assert!(metadata.name().is_none());
}

fn profile_admit_invalid_kind(vector: &Vector) {
    let error = verify_and_admit_profile_event(canonical_envelope(input_str(vector, "event_json")))
        .expect_err("verified non-Profile kind must fail admission");
    assert_eq!(error.code(), expected_str(vector, "error"));
    assert!(matches!(
        error,
        RadrootsProfileAdmissionError::InvalidKind { expected: 0, actual }
            if u64::from(actual) == vector.expected["actual"].as_u64().expect("expected.actual")
    ));
}

fn profile_admit_invalid_metadata(vector: &Vector) {
    let error = verify_and_admit_profile_event(canonical_envelope(input_str(vector, "event_json")))
        .expect_err("verified Profile with non-object content must fail admission");
    assert_eq!(error.code(), expected_str(vector, "error"));
    assert_eq!(
        error,
        RadrootsProfileAdmissionError::Metadata(RadrootsProfileMetadataParseError::RootNotObject)
    );
}

fn profile_select_equal_time_head(vector: &Vector) {
    let first = admitted_event(input_str(vector, "first_event_json"), &vector.id);
    let second = admitted_event(input_str(vector, "second_event_json"), &vector.id);
    assert_eq!(
        first.event().created_at_u64(),
        second.event().created_at_u64()
    );
    assert_eq!(
        selected_event_id(&first, &second),
        expected_str(vector, "event_id")
    );
    assert_eq!(
        selected_event_id(&second, &first),
        expected_str(vector, "event_id")
    );
    assert_eq!(
        first.event().created_at_u64(),
        vector.expected["created_at"]
            .as_u64()
            .expect("expected.created_at")
    );
}

fn selected_event_id(
    current: &RadrootsAdmittedProfileEvent,
    candidate: &RadrootsAdmittedProfileEvent,
) -> String {
    let current: RadrootsCurrentEventHead = head_candidate(current).into();
    match select_event_head(head_candidate(candidate), Some(&current)) {
        RadrootsEventHeadDecision::Applied(head) => head.event_id.into_string(),
        RadrootsEventHeadDecision::SkippedDuplicate
        | RadrootsEventHeadDecision::SkippedOlder
        | RadrootsEventHeadDecision::SkippedSameTimestampHigherEventId => {
            current.event_id.into_string()
        }
        RadrootsEventHeadDecision::CoordinateMismatch => {
            panic!("admitted Profile events from one author must share a coordinate")
        }
    }
}

fn head_candidate(admitted: &RadrootsAdmittedProfileEvent) -> RadrootsEventHeadCandidate {
    match event_head_candidate_for_event(admitted.event()).expect("Profile contract") {
        RadrootsEventHeadCandidateResult::Candidate(candidate) => candidate,
        other => panic!("admitted Profile must be a replaceable head candidate: {other:?}"),
    }
}

fn admitted_event(raw_json: &str, vector_id: &str) -> RadrootsAdmittedProfileEvent {
    verify_and_admit_profile_event(canonical_envelope(raw_json))
        .unwrap_or_else(|error| panic!("{vector_id} failed: {error}"))
}

fn canonical_envelope(raw_json: &str) -> RadrootsEventEnvelope {
    RadrootsNip01EventWire::parse_json(raw_json)
        .expect("canonical raw event")
        .into_envelope()
        .expect("event envelope")
}

fn unchecked_id_envelope(raw_json: &str) -> RadrootsEventEnvelope {
    let raw: RawEvent = serde_json::from_str(raw_json).expect("raw event");
    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: raw.id,
        author: raw.pubkey,
        created_at: raw.created_at,
        kind: raw.kind,
        tags: raw.tags,
        content: raw.content,
        sig: raw.sig,
    })
    .expect("unchecked-id envelope")
}

fn projected_metadata(admitted: &RadrootsAdmittedProfileEvent) -> Value {
    let metadata = admitted.metadata();
    let mut projected = Map::new();
    if let Some(value) = metadata.name() {
        projected.insert("name".to_string(), Value::String(value.to_string()));
    }
    if let Some(value) = metadata.display_name() {
        projected.insert("display_name".to_string(), Value::String(value.to_string()));
    }
    if let Some(value) = metadata.bot() {
        projected.insert("bot".to_string(), Value::Bool(value));
    }
    Value::Object(projected)
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
