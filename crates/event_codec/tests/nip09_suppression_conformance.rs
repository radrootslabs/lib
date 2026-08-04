#![cfg(feature = "json")]

mod support;

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use radroots_event::{envelope::EventEnvelope, envelope::EventEnvelopeParts};
use radroots_event_codec::{
    admission::deletion::{
        RadrootsAdmittedNip09DeletionRequestEvent, RadrootsNip09SuppressionDecision,
        admit_verified_nip09_deletion_request_event, evaluate_nip09_suppression,
    },
    verify::{RadrootsSignatureVerifiedEvent, verify_nip01_event},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const PACKAGED_VECTORS: &str = include_str!("fixtures/deletion_suppression.v1.json");
const WORKSPACE_VECTOR_PATH: &str =
    "../../contracts/conformance/vectors/deletion/suppression.v1.json";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";
const OPERATION: &str = "social.deletion_request.evaluate_suppression.valid";

const VECTOR_EXPECTATIONS: [(&str, &str, &str); 32] = [
    (
        "nip09_suppress_no_requests_visible",
        "visible",
        "deletion_no_authorized_reference",
    ),
    (
        "nip09_suppress_same_author_event_reference",
        "suppressed",
        "deletion_event_id_reference",
    ),
    (
        "nip09_suppress_same_author_nonmatching_reference",
        "visible",
        "deletion_no_authorized_reference",
    ),
    (
        "nip09_suppress_wrong_author_exact_event_reference",
        "visible",
        "deletion_request_author_mismatch",
    ),
    (
        "nip09_suppress_event_reference_predates_target",
        "suppressed",
        "deletion_event_id_reference",
    ),
    (
        "nip09_suppress_deletion_request_immune_event_reference",
        "visible",
        "deletion_request_immune",
    ),
    (
        "nip09_suppress_deletion_request_immune_mixed_references",
        "visible",
        "deletion_request_immune",
    ),
    (
        "nip09_suppress_address_cutoff_before_target",
        "visible",
        "deletion_address_cutoff_precedes_target",
    ),
    (
        "nip09_suppress_address_cutoff_equal_target",
        "suppressed",
        "deletion_address_reference",
    ),
    (
        "nip09_suppress_address_cutoff_after_target",
        "suppressed",
        "deletion_address_reference",
    ),
    (
        "nip09_suppress_address_wrong_kind",
        "visible",
        "deletion_no_authorized_reference",
    ),
    (
        "nip09_suppress_address_wrong_identifier",
        "visible",
        "deletion_no_authorized_reference",
    ),
    (
        "nip09_suppress_address_wrong_pubkey",
        "visible",
        "deletion_no_authorized_reference",
    ),
    (
        "nip09_suppress_wrong_author_address_reference",
        "visible",
        "deletion_request_author_mismatch",
    ),
    (
        "nip09_suppress_replaceable_kind_0",
        "suppressed",
        "deletion_address_reference",
    ),
    (
        "nip09_suppress_replaceable_kind_3",
        "suppressed",
        "deletion_address_reference",
    ),
    (
        "nip09_suppress_replaceable_kind_10000",
        "suppressed",
        "deletion_address_reference",
    ),
    (
        "nip09_suppress_replaceable_kind_19999",
        "suppressed",
        "deletion_address_reference",
    ),
    (
        "nip09_suppress_addressable_kind_30000_empty_identifier",
        "suppressed",
        "deletion_address_reference",
    ),
    (
        "nip09_suppress_addressable_kind_39999_opaque_identifier",
        "suppressed",
        "deletion_address_reference",
    ),
    (
        "nip09_suppress_combined_event_and_address_references",
        "suppressed",
        "deletion_event_id_and_address_reference",
    ),
    (
        "nip09_suppress_event_reference_with_stale_address",
        "suppressed",
        "deletion_event_id_reference",
    ),
    (
        "nip09_suppress_kind_advisory_diagnostics_ignored",
        "suppressed",
        "deletion_event_id_reference",
    ),
    (
        "nip09_suppress_duplicate_raw_targets_deduplicated",
        "suppressed",
        "deletion_event_id_and_address_reference",
    ),
    (
        "nip09_suppress_max_address_cutoff_forward_order",
        "suppressed",
        "deletion_address_reference",
    ),
    (
        "nip09_suppress_max_address_cutoff_reverse_order",
        "suppressed",
        "deletion_address_reference",
    ),
    (
        "nip09_suppress_later_revision_survives_address_cutoff",
        "visible",
        "deletion_address_cutoff_precedes_target",
    ),
    (
        "nip09_suppress_equal_max_cutoff_uses_lowest_request_id",
        "suppressed",
        "deletion_address_reference",
    ),
    (
        "nip09_suppress_multiple_event_references_use_lowest_request_id",
        "suppressed",
        "deletion_event_id_reference",
    ),
    (
        "nip09_suppress_repeated_request_is_idempotent",
        "suppressed",
        "deletion_event_id_reference",
    ),
    (
        "nip09_suppress_unauthorized_event_plus_stale_authorized_address",
        "visible",
        "deletion_address_cutoff_precedes_target",
    ),
    (
        "nip09_suppress_exact_event_deletes_later_replacement",
        "suppressed",
        "deletion_event_id_reference",
    ),
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
    input: Input,
    expected: ExpectedDecision,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    request_event_jsons: Vec<String>,
    target_event_json: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ExpectedDecision {
    outcome: String,
    reason: String,
    event_reference: Option<ExpectedEventReference>,
    address_reference: Option<ExpectedAddressReference>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ExpectedEventReference {
    request_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ExpectedAddressReference {
    coordinate: String,
    inclusive_cutoff: u64,
    request_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
fn suppression_vectors_execute_the_public_evaluator() {
    let vectors = conformance_vectors();
    let raw_suite: Value =
        serde_json::from_str(&vectors).expect("NIP-09 suppression JSON must parse");
    validate_no_forbidden_corpus_metadata(&raw_suite, "$")
        .unwrap_or_else(|error| panic!("{error}"));
    assert_exact_nullable_evidence_shape(&raw_suite);

    let suite: Suite =
        serde_json::from_str(&vectors).expect("NIP-09 suppression vectors must parse");
    assert_eq!(suite.suite, "nip09_suppression_evaluator");
    assert_eq!(suite.contract_version, "1.0.0");
    assert_vector_inventory(&suite.vectors);
    assert_signed_event_inventory(&suite.vectors);

    for vector in &suite.vectors {
        execute(vector);
    }
}

fn assert_vector_inventory(vectors: &[Vector]) {
    let expected = VECTOR_EXPECTATIONS
        .into_iter()
        .map(|(id, outcome, reason)| (id, (OPERATION, outcome, reason)))
        .collect::<BTreeMap<_, _>>();
    let actual = vectors
        .iter()
        .map(|vector| {
            (
                vector.id.as_str(),
                (
                    vector.kind.as_str(),
                    vector.expected.outcome.as_str(),
                    vector.expected.reason.as_str(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(vectors.len(), VECTOR_EXPECTATIONS.len());
    assert_eq!(actual, expected);
    assert!(vectors.iter().all(|vector| vector.id.starts_with("nip09_")));

    let reasons = vectors
        .iter()
        .map(|vector| vector.expected.reason.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        reasons,
        BTreeSet::from([
            "deletion_address_cutoff_precedes_target",
            "deletion_address_reference",
            "deletion_event_id_and_address_reference",
            "deletion_event_id_reference",
            "deletion_no_authorized_reference",
            "deletion_request_author_mismatch",
            "deletion_request_immune",
        ])
    );
}

fn assert_signed_event_inventory(vectors: &[Vector]) {
    let event_jsons = vectors
        .iter()
        .flat_map(|vector| {
            std::iter::once(vector.input.target_event_json.as_str())
                .chain(vector.input.request_event_jsons.iter().map(String::as_str))
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(event_jsons.len(), 38, "signed event inventory drifted");

    let mut deletion_request_count = 0usize;
    for event_json in event_jsons {
        let verified = verified_event(event_json, "signed event inventory");
        if verified.event().kind_u32() == 5 {
            admit_verified_nip09_deletion_request_event(verified)
                .expect("every fixed kind-5 corpus event must admit");
            deletion_request_count += 1;
        }
    }
    assert_eq!(deletion_request_count, 30);
}

fn execute(vector: &Vector) {
    let target = verified_event(&vector.input.target_event_json, &vector.id);
    let requests = vector
        .input
        .request_event_jsons
        .iter()
        .map(|event_json| admitted_request(event_json, &vector.id))
        .collect::<Vec<_>>();
    let target_before = target.clone();
    let requests_before = requests.clone();

    let decision = evaluate_nip09_suppression(&target, &requests);
    assert_eq!(actual_decision(&decision), vector.expected, "{}", vector.id);
    assert_eq!(target, target_before, "{} target changed", vector.id);
    assert_eq!(requests, requests_before, "{} requests changed", vector.id);

    let repeated = evaluate_nip09_suppression(&target, &requests);
    assert_eq!(
        repeated, decision,
        "{} repeat evaluation drifted",
        vector.id
    );
    let mut reversed = requests.clone();
    reversed.reverse();
    assert_eq!(
        evaluate_nip09_suppression(&target, &reversed),
        decision,
        "{} request order changed the decision",
        vector.id
    );
}

fn actual_decision(decision: &RadrootsNip09SuppressionDecision) -> ExpectedDecision {
    ExpectedDecision {
        outcome: decision.outcome().code().to_owned(),
        reason: decision.reason().code().to_owned(),
        event_reference: decision
            .event_reference()
            .map(|evidence| ExpectedEventReference {
                request_id: evidence.request_id().to_hex(),
            }),
        address_reference: decision
            .address_reference()
            .map(|evidence| ExpectedAddressReference {
                coordinate: evidence.coordinate().as_str().to_owned(),
                inclusive_cutoff: evidence.inclusive_cutoff(),
                request_id: evidence.request_id().to_hex(),
            }),
    }
}

fn admitted_request(
    event_json: &str,
    vector_id: &str,
) -> RadrootsAdmittedNip09DeletionRequestEvent {
    let verified = verified_event(event_json, vector_id);
    admit_verified_nip09_deletion_request_event(verified)
        .unwrap_or_else(|error| panic!("{vector_id} request admission failed: {error}"))
}

fn verified_event(event_json: &str, vector_id: &str) -> RadrootsSignatureVerifiedEvent {
    verify_nip01_event(event_envelope(event_json, vector_id))
        .unwrap_or_else(|error| panic!("{vector_id} signature verification failed: {error}"))
}

fn event_envelope(event_json: &str, vector_id: &str) -> EventEnvelope {
    let raw: RawEvent = serde_json::from_str(event_json)
        .unwrap_or_else(|error| panic!("{vector_id} signed event JSON is invalid: {error}"));
    assert_eq!(
        serde_json::to_string(&raw).expect("raw event serialization"),
        event_json,
        "{vector_id} signed event JSON must be canonical and compact"
    );
    EventEnvelope::new(EventEnvelopeParts {
        id: raw.id,
        author: raw.pubkey,
        created_at: raw.created_at,
        kind: raw.kind,
        tags: raw.tags,
        content: raw.content,
        sig: raw.sig,
    })
    .unwrap_or_else(|error| panic!("{vector_id} event envelope failed: {error}"))
}

fn assert_exact_nullable_evidence_shape(suite: &Value) {
    let suite_object = suite.as_object().expect("suite object");
    assert_eq!(
        suite_object
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["contract_version", "suite", "vectors"])
    );
    for vector in suite["vectors"].as_array().expect("vectors array") {
        assert_eq!(
            vector
                .as_object()
                .expect("vector object")
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["expected", "id", "input", "kind"])
        );
        assert_eq!(
            vector["input"]
                .as_object()
                .expect("input object")
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["request_event_jsons", "target_event_json"])
        );
        assert_eq!(
            vector["expected"]
                .as_object()
                .expect("expected object")
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["address_reference", "event_reference", "outcome", "reason",])
        );
    }
}

fn conformance_vectors() -> Cow<'static, str> {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(WORKSPACE_VECTOR_PATH);
    match fs::read_to_string(&workspace_path) {
        Ok(canonical) => {
            assert_eq!(
                canonical,
                PACKAGED_VECTORS,
                "packaged suppression vectors must match {}",
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

fn validate_no_forbidden_corpus_metadata(value: &Value, path: &str) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if is_forbidden_metadata_key(key) {
                    return Err(format!(
                        "frozen NIP-09 suppression corpus contains forbidden key {path}.{key}"
                    ));
                }
                validate_no_forbidden_corpus_metadata(child, &format!("{path}.{key}"))?;
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                validate_no_forbidden_corpus_metadata(child, &format!("{path}[{index}]"))?;
            }
        }
        Value::String(string) => {
            if string.to_ascii_lowercase().contains("nsec1") {
                return Err(format!(
                    "frozen NIP-09 suppression corpus contains nsec material at {path}"
                ));
            }
            if contains_approved_fixture_secret(string) {
                return Err(format!(
                    "frozen NIP-09 suppression corpus contains a fixture secret at {path}"
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_forbidden_metadata_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    matches!(normalized.as_str(), "base" | "mutation")
        || normalized.contains("source")
        || normalized.contains("seed")
        || normalized.contains("generator")
        || normalized.contains("recipe")
        || normalized.contains("secret_key")
        || normalized.contains("private_key")
        || normalized.contains("signing_key")
        || normalized.contains("authorization")
        || normalized.contains("authorized")
        || normalized.contains("store_mutation")
        || normalized == "effect"
        || normalized.ends_with("_effect")
        || normalized == "effects"
}

fn contains_approved_fixture_secret(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    support::approved_fixture_identities()
        .iter()
        .any(|identity| normalized.contains(identity.secret_key_hex))
}

#[test]
fn suppression_corpus_hygiene_rejects_generation_and_effect_authority_metadata() {
    for key in [
        "SOURCE",
        "SeEd",
        "GENERATOR",
        "MuTaTiOn",
        "AuThOrIzAtIoN",
        "store_effect",
    ] {
        let value = Value::Object([(key.to_owned(), Value::Null)].into_iter().collect());
        let error = validate_no_forbidden_corpus_metadata(&value, "$")
            .expect_err("forbidden corpus metadata must fail");
        assert!(error.contains(key), "{error}");
    }
    assert!(validate_no_forbidden_corpus_metadata(&json!("NSEC1FORBIDDEN"), "$").is_err());
    for identity in support::approved_fixture_identities() {
        assert!(
            validate_no_forbidden_corpus_metadata(&json!(identity.secret_key_hex), "$").is_err()
        );
    }
}

#[test]
fn suppression_corpus_shapes_reject_unknown_fields() {
    let input = json!({
        "request_event_jsons": [],
        "target_event_json": "{}",
        "unexpected": null
    });
    serde_json::from_value::<Input>(input).expect_err("unknown input fields must fail");

    let expected = json!({
        "outcome": "visible",
        "reason": "deletion_no_authorized_reference",
        "event_reference": null,
        "address_reference": null,
        "unexpected": null
    });
    serde_json::from_value::<ExpectedDecision>(expected)
        .expect_err("unknown expected fields must fail");

    let evidence = json!({"request_id": "a", "unexpected": null});
    serde_json::from_value::<ExpectedEventReference>(evidence)
        .expect_err("unknown evidence fields must fail");
}
