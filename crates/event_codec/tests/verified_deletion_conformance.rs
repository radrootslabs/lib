#![cfg(feature = "json")]

mod support;

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use radroots_event::{
    envelope::EventEnvelope,
    envelope::EventEnvelopeLimits,
    envelope::EventEnvelopeParts,
    post::deletion::{
        AuthoredNip09DeletionRequest, Nip09DeletionAddressTarget, Nip09DeletionError,
        Nip09DeletionEventTarget, RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES,
        RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES,
        RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES, RADROOTS_NIP09_DELETION_TAG_MAX_COUNT,
        RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT,
        RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES,
    },
};
use radroots_event_codec::{
    deletion::{
        admission::verify_and_admit_nip09_deletion_request_event,
        authored::authored_nip09_deletion_request_to_wire_parts,
        inbound::{
            RadrootsInboundNip09DeletionProjection, project_verified_nip09_deletion_request_event,
        },
    },
    verification::verify_nip01_event,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const PACKAGED_VECTORS: &str = include_str!("fixtures/deletion_verified_profile.v1.json");
const WORKSPACE_VECTOR_PATH: &str =
    "../../contracts/conformance/vectors/deletion/verified_profile.v1.json";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";

const AUTHORED_VALID: &str = "social.deletion_request.build_authored_draft.valid";
const AUTHORED_INVALID: &str = "social.deletion_request.build_authored_draft.invalid";
const PROJECT_VALID: &str = "social.deletion_request.project_verified_event.valid";
const PROJECT_INVALID: &str = "social.deletion_request.project_verified_event.invalid";
const ADMIT_VALID: &str = "social.deletion_request.verify_and_admit_event.valid";
const ADMIT_INVALID: &str = "social.deletion_request.verify_and_admit_event.invalid";

const VECTOR_EXPECTATIONS: [(&str, &str); 80] = [
    (
        "nip09_authored_event_target_min_kind_empty_content",
        AUTHORED_VALID,
    ),
    (
        "nip09_authored_event_target_max_kind_unicode_content",
        AUTHORED_VALID,
    ),
    (
        "nip09_authored_coordinate_kind_0_empty_identifier",
        AUTHORED_VALID,
    ),
    (
        "nip09_authored_coordinate_kind_3_empty_identifier",
        AUTHORED_VALID,
    ),
    (
        "nip09_authored_coordinate_kind_10000_empty_identifier",
        AUTHORED_VALID,
    ),
    (
        "nip09_authored_coordinate_kind_19999_empty_identifier",
        AUTHORED_VALID,
    ),
    (
        "nip09_authored_coordinate_kind_30000_empty_identifier",
        AUTHORED_VALID,
    ),
    (
        "nip09_authored_coordinate_kind_39999_opaque_identifier",
        AUTHORED_VALID,
    ),
    (
        "nip09_authored_mixed_targets_canonical_order",
        AUTHORED_VALID,
    ),
    ("nip09_authored_content_bytes_exact", AUTHORED_VALID),
    ("nip09_authored_tag_count_exact", AUTHORED_VALID),
    ("nip09_authored_tag_element_bytes_exact", AUTHORED_VALID),
    ("nip09_authored_tag_bytes_exact", AUTHORED_VALID),
    ("nip09_authored_event_wire_bytes_exact", AUTHORED_VALID),
    ("nip09_authored_event_target_invalid", AUTHORED_INVALID),
    (
        "nip09_authored_event_target_kind_out_of_range",
        AUTHORED_INVALID,
    ),
    (
        "nip09_authored_address_target_invalid_format",
        AUTHORED_INVALID,
    ),
    (
        "nip09_authored_address_target_invalid_pubkey",
        AUTHORED_INVALID,
    ),
    (
        "nip09_authored_address_target_unsupported_kind",
        AUTHORED_INVALID,
    ),
    (
        "nip09_authored_replaceable_identifier_nonempty",
        AUTHORED_INVALID,
    ),
    (
        "nip09_authored_event_target_duplicate_normalized",
        AUTHORED_INVALID,
    ),
    (
        "nip09_authored_address_target_duplicate_normalized",
        AUTHORED_INVALID,
    ),
    ("nip09_authored_target_missing", AUTHORED_INVALID),
    ("nip09_authored_content_bytes_overflow", AUTHORED_INVALID),
    (
        "nip09_authored_tag_count_overflow_precedes_duplicate",
        AUTHORED_INVALID,
    ),
    (
        "nip09_authored_tag_element_bytes_overflow",
        AUTHORED_INVALID,
    ),
    (
        "nip09_authored_tag_bytes_overflow_precedes_duplicate",
        AUTHORED_INVALID,
    ),
    (
        "nip09_authored_event_wire_bytes_overflow_precedes_duplicate",
        AUTHORED_INVALID,
    ),
    ("nip09_project_signed_event_target_without_k", PROJECT_VALID),
    (
        "nip09_project_signed_address_replaceable_boundaries",
        PROJECT_VALID,
    ),
    ("nip09_project_signed_addressable_boundaries", PROJECT_VALID),
    ("nip09_project_signed_mixed_raw_retention", PROJECT_VALID),
    (
        "nip09_project_signed_duplicate_targets_first_provenance",
        PROJECT_VALID,
    ),
    (
        "nip09_project_signed_canonical_effect_sorting",
        PROJECT_VALID,
    ),
    (
        "nip09_project_signed_kind_advisory_diagnostics",
        PROJECT_VALID,
    ),
    (
        "nip09_project_signed_event_target_conflict_unprovable",
        PROJECT_VALID,
    ),
    (
        "nip09_project_signed_trailing_kind_and_unknown_tags",
        PROJECT_VALID,
    ),
    (
        "nip09_project_signed_unicode_whitespace_control_content",
        PROJECT_VALID,
    ),
    ("nip09_project_signed_content_bytes_exact", PROJECT_VALID),
    ("nip09_project_signed_tag_count_exact", PROJECT_VALID),
    (
        "nip09_project_signed_tag_element_count_exact",
        PROJECT_VALID,
    ),
    (
        "nip09_project_signed_tag_element_bytes_exact_multibyte",
        PROJECT_VALID,
    ),
    ("nip09_project_signed_tag_bytes_exact", PROJECT_VALID),
    (
        "nip09_project_signed_event_wire_bytes_exact_max_created_at",
        PROJECT_VALID,
    ),
    (
        "nip09_project_signed_event_wire_short_created_at_width",
        PROJECT_VALID,
    ),
    ("nip09_project_signed_kind_advisory_min_max", PROJECT_VALID),
    ("nip09_project_signed_wrong_kind", PROJECT_INVALID),
    (
        "nip09_project_signed_content_bytes_overflow",
        PROJECT_INVALID,
    ),
    ("nip09_project_signed_tag_count_overflow", PROJECT_INVALID),
    (
        "nip09_project_signed_tag_element_count_overflow",
        PROJECT_INVALID,
    ),
    (
        "nip09_project_signed_tag_element_bytes_overflow",
        PROJECT_INVALID,
    ),
    ("nip09_project_signed_tag_bytes_overflow", PROJECT_INVALID),
    (
        "nip09_project_signed_event_wire_bytes_overflow",
        PROJECT_INVALID,
    ),
    ("nip09_project_signed_event_target_shape", PROJECT_INVALID),
    ("nip09_project_signed_event_target_empty", PROJECT_INVALID),
    ("nip09_project_signed_event_target_invalid", PROJECT_INVALID),
    ("nip09_project_signed_address_target_shape", PROJECT_INVALID),
    (
        "nip09_project_signed_address_target_missing_colon",
        PROJECT_INVALID,
    ),
    (
        "nip09_project_signed_address_target_invalid_pubkey",
        PROJECT_INVALID,
    ),
    (
        "nip09_project_signed_address_target_unsupported_kind",
        PROJECT_INVALID,
    ),
    (
        "nip09_project_signed_address_target_identifier_forbidden",
        PROJECT_INVALID,
    ),
    ("nip09_project_signed_target_missing", PROJECT_INVALID),
    (
        "nip09_project_signed_first_malformed_event_target",
        PROJECT_INVALID,
    ),
    (
        "nip09_project_signed_first_malformed_address_target",
        PROJECT_INVALID,
    ),
    (
        "nip09_project_signed_kind_precedes_content",
        PROJECT_INVALID,
    ),
    (
        "nip09_project_signed_content_precedes_tag_count",
        PROJECT_INVALID,
    ),
    (
        "nip09_project_signed_tag_count_precedes_element_count",
        PROJECT_INVALID,
    ),
    (
        "nip09_project_signed_element_count_precedes_element_size",
        PROJECT_INVALID,
    ),
    (
        "nip09_project_signed_element_size_precedes_tag_bytes",
        PROJECT_INVALID,
    ),
    (
        "nip09_project_signed_tag_bytes_precedes_wire",
        PROJECT_INVALID,
    ),
    (
        "nip09_project_signed_wire_precedes_target_parse",
        PROJECT_INVALID,
    ),
    (
        "nip09_project_signed_target_parse_precedes_missing_union",
        PROJECT_INVALID,
    ),
    ("nip09_admit_signed_event_target", ADMIT_VALID),
    ("nip09_admit_signed_address_target", ADMIT_VALID),
    ("nip09_admit_signed_mixed_tolerant_projection", ADMIT_VALID),
    ("nip09_admit_invalid_signature", ADMIT_INVALID),
    ("nip09_admit_id_mismatch", ADMIT_INVALID),
    ("nip09_admit_wrong_kind", ADMIT_INVALID),
    ("nip09_admit_invalid_target", ADMIT_INVALID),
    ("nip09_admit_target_missing", ADMIT_INVALID),
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
fn deletion_operation_vectors_execute_the_declared_public_functions() {
    let vectors = conformance_vectors();
    let raw_suite: Value = serde_json::from_str(&vectors).expect("NIP-09 JSON must parse");
    validate_no_forbidden_corpus_metadata(&raw_suite, "$")
        .unwrap_or_else(|error| panic!("{error}"));
    let suite: Suite = serde_json::from_str(&vectors).expect("NIP-09 vectors must parse");
    assert_eq!(suite.suite, "nip09_deletion_request_profile");
    assert_eq!(suite.contract_version, "1.0.0");
    assert_vector_inventory(&suite.vectors);

    for vector in &suite.vectors {
        execute(vector);
        assert_boundary_case(vector);
    }
}

fn validate_no_forbidden_corpus_metadata(value: &Value, path: &str) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if is_forbidden_metadata_key(key) {
                    return Err(format!(
                        "frozen NIP-09 corpus contains forbidden key {path}.{key}"
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
            if contains_nsec_material(string) {
                return Err(format!(
                    "frozen NIP-09 corpus contains an nsec value at {path}"
                ));
            }
            if contains_approved_fixture_secret(string) {
                return Err(format!(
                    "frozen NIP-09 corpus contains an approved fixture secret at {path}"
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
        || normalized.contains("seed")
        || normalized.contains("generator")
        || normalized.contains("recipe")
        || normalized.contains("secret_key")
        || normalized.contains("private_key")
        || normalized.contains("signing_key")
        || normalized.contains("boundary")
        || normalized.contains("authorization")
        || normalized.contains("authorized")
        || normalized.contains("cutoff")
        || normalized.contains("evaluator")
        || normalized.contains("store_mutation")
        || normalized.contains("suppression")
        || normalized.contains("suppressed")
        || normalized == "effect"
        || normalized.ends_with("_effect")
        || normalized == "effects"
}

fn contains_nsec_material(value: &str) -> bool {
    value.to_ascii_lowercase().contains("nsec1")
}

fn contains_approved_fixture_secret(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    support::approved_fixture_identities()
        .iter()
        .any(|identity| normalized.contains(identity.secret_key_hex))
}

#[test]
fn deletion_corpus_hygiene_recognizes_every_approved_fixture_secret() {
    assert!(contains_nsec_material("NSEC1FORBIDDEN"));
    for key in [
        "SeEd",
        "GENERATOR",
        "AuThOrIzAtIoN",
        "CuToFf",
        "Evaluator",
        "SUPPRESSION",
        "effects",
    ] {
        let value = Value::Object([(key.to_string(), Value::Null)].into_iter().collect());
        let error = validate_no_forbidden_corpus_metadata(&value, "$")
            .expect_err("mixed-case generation and effect metadata must fail");
        assert!(error.contains(key), "{error}");
    }

    for identity in support::approved_fixture_identities() {
        assert!(contains_approved_fixture_secret(identity.secret_key_hex));
        assert!(contains_approved_fixture_secret(
            format!("prefix{}suffix", identity.secret_key_hex).as_str()
        ));
    }
}

#[test]
fn deletion_corpus_shape_rejects_unknown_suite_and_vector_fields() {
    let suite = json!({
        "suite": "nip09_deletion_request_profile",
        "contract_version": "1.0.0",
        "vectors": [],
        "unexpected": null
    });
    serde_json::from_value::<Suite>(suite).expect_err("unknown suite fields must fail");

    let vector = json!({
        "id": "nip09_unknown_shape",
        "kind": AUTHORED_VALID,
        "input": {},
        "expected": {},
        "unexpected": null
    });
    serde_json::from_value::<Vector>(vector).expect_err("unknown vector fields must fail");
}

fn conformance_vectors() -> Cow<'static, str> {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(WORKSPACE_VECTOR_PATH);
    match fs::read_to_string(&workspace_path) {
        Ok(canonical) => {
            assert_eq!(
                canonical,
                PACKAGED_VECTORS,
                "packaged NIP-09 vectors must match {}",
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

fn assert_vector_inventory(vectors: &[Vector]) {
    let expected = VECTOR_EXPECTATIONS.into_iter().collect::<BTreeMap<_, _>>();
    let actual = vectors
        .iter()
        .map(|vector| (vector.id.as_str(), vector.kind.as_str()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(vectors.len(), 80);
    assert_eq!(actual, expected);
    assert!(vectors.iter().all(|vector| vector.id.starts_with("nip09_")));

    let kinds = vectors.iter().fold(BTreeMap::new(), |mut counts, vector| {
        *counts.entry(vector.kind.as_str()).or_insert(0usize) += 1;
        counts
    });
    assert_eq!(
        kinds,
        BTreeMap::from([
            (AUTHORED_INVALID, 14),
            (AUTHORED_VALID, 14),
            (PROJECT_INVALID, 26),
            (PROJECT_VALID, 18),
            (ADMIT_INVALID, 5),
            (ADMIT_VALID, 3),
        ])
    );

    assert_error_set(
        vectors,
        AUTHORED_INVALID,
        &[
            "deletion_address_target_duplicate",
            "deletion_address_target_invalid",
            "deletion_content_too_large",
            "deletion_event_target_duplicate",
            "deletion_event_target_invalid",
            "deletion_event_wire_too_large",
            "deletion_tag_bytes_exceeded",
            "deletion_tag_count_exceeded",
            "deletion_tag_element_too_large",
            "deletion_target_missing",
        ],
    );
    assert_error_set(
        vectors,
        PROJECT_INVALID,
        &[
            "deletion_address_target_invalid",
            "deletion_address_target_shape",
            "deletion_content_too_large",
            "deletion_event_target_invalid",
            "deletion_event_target_shape",
            "deletion_event_wire_too_large",
            "deletion_tag_bytes_exceeded",
            "deletion_tag_count_exceeded",
            "deletion_tag_element_count_exceeded",
            "deletion_tag_element_too_large",
            "deletion_target_missing",
            "unsupported_kind",
        ],
    );
    assert_error_set(
        vectors,
        ADMIT_INVALID,
        &[
            "deletion_event_target_invalid",
            "deletion_target_missing",
            "id_mismatch",
            "signature_invalid",
            "unsupported_kind",
        ],
    );

    let diagnostics = vectors
        .iter()
        .filter(|vector| vector.kind == PROJECT_VALID || vector.kind == ADMIT_VALID)
        .flat_map(|vector| {
            vector.expected["diagnostics"]
                .as_array()
                .unwrap_or_else(|| panic!("{} expected.diagnostics must be an array", vector.id))
        })
        .map(|diagnostic| {
            diagnostic["code"]
                .as_str()
                .expect("diagnostic.code must be a string")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        diagnostics,
        BTreeSet::from([
            "deletion_kind_advisory_conflict_ignored",
            "deletion_kind_advisory_duplicate_ignored",
            "deletion_kind_advisory_invalid_ignored",
            "deletion_kind_advisory_shape_ignored",
        ])
    );

    for vector in vectors {
        let input = vector
            .input
            .as_object()
            .unwrap_or_else(|| panic!("{} input must be an object", vector.id));
        if vector.kind == AUTHORED_VALID || vector.kind == AUTHORED_INVALID {
            assert_eq!(
                input.keys().map(String::as_str).collect::<Vec<_>>(),
                ["address_targets", "content", "event_targets"],
                "{} authored input fields drifted",
                vector.id
            );
        } else {
            assert_eq!(
                input.keys().map(String::as_str).collect::<Vec<_>>(),
                ["event_json"],
                "{} signed input must contain only fixed event_json",
                vector.id
            );
        }

        if vector.kind == PROJECT_VALID || vector.kind == ADMIT_VALID {
            assert_eq!(
                vector
                    .expected
                    .as_object()
                    .expect("valid expected object")
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                [
                    "address_targets",
                    "contract_id",
                    "diagnostics",
                    "event_targets",
                    "kind_advisories",
                    "raw_tags",
                ],
                "{} projection output shape drifted",
                vector.id
            );
        }
    }
}

fn assert_error_set(vectors: &[Vector], kind: &str, expected: &[&str]) {
    let actual = vectors
        .iter()
        .filter(|vector| vector.kind == kind)
        .map(|vector| expected_str(vector, "error"))
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected.iter().copied().collect(), "{kind}");
}

fn execute(vector: &Vector) {
    match vector.kind.as_str() {
        AUTHORED_VALID => {
            let request =
                authored_request(vector).unwrap_or_else(|error| panic!("{}: {error}", vector.id));
            let first = authored_nip09_deletion_request_to_wire_parts(&request);
            let second = authored_nip09_deletion_request_to_wire_parts(&request);
            assert_eq!(first, second, "{} repeat encoding drifted", vector.id);
            assert_eq!(
                serde_json::to_value(first).expect("authored deletion result must serialize"),
                vector.expected,
                "{}",
                vector.id
            );
        }
        AUTHORED_INVALID => {
            let error = authored_request(vector).expect_err("invalid authored deletion must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        PROJECT_VALID => {
            let verified = verify_nip01_event(fixture_envelope(vector))
                .unwrap_or_else(|error| panic!("{} verification failed: {error}", vector.id));
            let projection = project_verified_nip09_deletion_request_event(&verified)
                .unwrap_or_else(|error| panic!("{} projection failed: {error}", vector.id));
            assert_eq!(
                projection_snapshot(&projection),
                vector.expected,
                "{}",
                vector.id
            );
        }
        PROJECT_INVALID => {
            let verified = verify_nip01_event(fixture_envelope(vector))
                .unwrap_or_else(|error| panic!("{} verification failed: {error}", vector.id));
            let error = project_verified_nip09_deletion_request_event(&verified)
                .expect_err("invalid verified deletion projection must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        ADMIT_VALID => {
            let envelope = fixture_envelope(vector);
            let expected_event = envelope.clone();
            let admitted = verify_and_admit_nip09_deletion_request_event(envelope)
                .unwrap_or_else(|error| panic!("{} admission failed: {error}", vector.id));
            assert_eq!(admitted.event(), &expected_event, "{}", vector.id);
            assert_eq!(
                admitted.contract().id,
                "radroots.social.deletion_request.v1"
            );
            assert_eq!(
                projection_snapshot(admitted.projection()),
                vector.expected,
                "{}",
                vector.id
            );
            let (verified, projection) = admitted.into_parts();
            assert_eq!(verified.event(), &expected_event, "{}", vector.id);
            assert_eq!(
                projection.contract_id(),
                "radroots.social.deletion_request.v1"
            );
        }
        ADMIT_INVALID => {
            let error = verify_and_admit_nip09_deletion_request_event(fixture_envelope(vector))
                .expect_err("invalid signed deletion must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        other => panic!("{} has unknown vector kind {other}", vector.id),
    }
}

fn authored_request(vector: &Vector) -> Result<AuthoredNip09DeletionRequest, Nip09DeletionError> {
    let event_targets = input_array(vector, "event_targets")
        .iter()
        .map(|target| {
            let target = target
                .as_object()
                .unwrap_or_else(|| panic!("{} event target must be an object", vector.id));
            Nip09DeletionEventTarget::parse(
                object_str(target, "event_id", &vector.id),
                object_u32(target, "kind", &vector.id),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let address_targets = input_array(vector, "address_targets")
        .iter()
        .map(|target| {
            Nip09DeletionAddressTarget::parse(
                target
                    .as_str()
                    .unwrap_or_else(|| panic!("{} address target must be a string", vector.id)),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    AuthoredNip09DeletionRequest::new(input_str(vector, "content"), event_targets, address_targets)
}

fn fixture_raw_event(vector: &Vector) -> RawEvent {
    let event_json = input_str(vector, "event_json");
    let raw: RawEvent = serde_json::from_str(event_json)
        .unwrap_or_else(|error| panic!("{} event_json failed to parse: {error}", vector.id));
    assert_eq!(
        serde_json::to_string(&raw).expect("raw event must serialize"),
        event_json,
        "{} event_json must be compact and canonical",
        vector.id
    );
    raw
}

fn fixture_envelope(vector: &Vector) -> EventEnvelope {
    let raw = fixture_raw_event(vector);
    let mut limits = EventEnvelopeLimits::default();
    limits.max_content_bytes = limits.max_content_bytes.max(raw.content.len());
    limits.max_tag_count = limits.max_tag_count.max(raw.tags.len());
    limits.max_total_tag_elements = limits
        .max_total_tag_elements
        .max(raw.tags.iter().map(Vec::len).sum());
    limits.max_tag_element_bytes = limits.max_tag_element_bytes.max(
        raw.tags
            .iter()
            .flat_map(|tag| tag.iter())
            .map(String::len)
            .max()
            .unwrap_or_default(),
    );
    limits.max_total_tag_bytes = limits.max_total_tag_bytes.max(tag_bytes(&raw.tags));
    EventEnvelope::new_with_limits(
        EventEnvelopeParts {
            id: raw.id,
            author: raw.pubkey,
            created_at: raw.created_at,
            kind: raw.kind,
            tags: raw.tags,
            content: raw.content,
            sig: raw.sig,
        },
        limits,
    )
    .unwrap_or_else(|error| panic!("{} envelope failed: {error}", vector.id))
}

fn projection_snapshot(projection: &RadrootsInboundNip09DeletionProjection) -> Value {
    json!({
        "contract_id": projection.contract_id(),
        "event_targets": projection.event_targets().iter().map(|target| json!({
            "tag_index": target.tag_index(),
            "event_id": target.event_id().to_hex(),
            "raw_tag": target.raw_tag(),
        })).collect::<Vec<_>>(),
        "address_targets": projection.address_targets().iter().map(|target| json!({
            "tag_index": target.tag_index(),
            "coordinate": target.coordinate().as_str(),
            "kind": target.coordinate().kind(),
            "pubkey": target.coordinate().pubkey().to_hex(),
            "identifier": target.coordinate().identifier(),
            "raw_tag": target.raw_tag(),
        })).collect::<Vec<_>>(),
        "kind_advisories": projection.kind_advisories().iter().map(|advisory| json!({
            "tag_index": advisory.tag_index(),
            "kind": advisory.kind(),
            "raw_tag": advisory.raw_tag(),
        })).collect::<Vec<_>>(),
        "diagnostics": projection.diagnostics().iter().map(|diagnostic| json!({
            "code": diagnostic.code(),
            "tag_index": diagnostic.tag_index(),
            "raw_tag": diagnostic.raw_tag(),
        })).collect::<Vec<_>>(),
        "raw_tags": projection.raw_tags(),
    })
}

fn assert_boundary_case(vector: &Vector) {
    match vector.id.as_str() {
        "nip09_authored_content_bytes_exact" => {
            assert_eq!(
                input_str(vector, "content").len(),
                RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES
            );
        }
        "nip09_authored_tag_count_exact" => {
            assert_eq!(
                vector.expected["tags"].as_array().expect("tags").len(),
                RADROOTS_NIP09_DELETION_TAG_MAX_COUNT
            );
        }
        "nip09_authored_tag_element_bytes_exact" => {
            assert_eq!(
                input_array(vector, "address_targets")[0]
                    .as_str()
                    .expect("coordinate")
                    .len(),
                RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES
            );
        }
        "nip09_authored_tag_bytes_exact" => {
            let tags = vector.expected["tags"].as_array().expect("tags");
            assert_eq!(
                value_tag_bytes(tags),
                RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES
            );
        }
        "nip09_authored_event_wire_bytes_exact" => {
            let request = authored_request(vector).expect("exact authored request");
            assert_eq!(
                request.maximum_signed_event_wire_bytes(),
                RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES
            );
        }
        "nip09_project_signed_content_bytes_exact" => {
            assert_eq!(
                fixture_raw_event(vector).content.len(),
                RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES
            );
        }
        "nip09_project_signed_tag_count_exact" => {
            assert_eq!(
                fixture_raw_event(vector).tags.len(),
                RADROOTS_NIP09_DELETION_TAG_MAX_COUNT
            );
        }
        "nip09_project_signed_tag_element_count_exact" => {
            assert_eq!(
                fixture_raw_event(vector)
                    .tags
                    .iter()
                    .map(Vec::len)
                    .sum::<usize>(),
                RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT
            );
        }
        "nip09_project_signed_tag_element_bytes_exact_multibyte" => {
            assert_eq!(
                fixture_raw_event(vector)
                    .tags
                    .iter()
                    .flat_map(|tag| tag.iter())
                    .map(String::len)
                    .max(),
                Some(RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES)
            );
        }
        "nip09_project_signed_tag_bytes_exact" => {
            assert_eq!(
                tag_bytes(&fixture_raw_event(vector).tags),
                RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES
            );
        }
        "nip09_project_signed_event_wire_bytes_exact_max_created_at" => {
            let raw = fixture_raw_event(vector);
            assert_eq!(raw.created_at, u64::MAX);
            assert_eq!(
                input_str(vector, "event_json").len(),
                RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES
            );
        }
        "nip09_project_signed_event_wire_short_created_at_width" => {
            let raw = fixture_raw_event(vector);
            assert_eq!(raw.created_at, 1);
            assert_eq!(
                input_str(vector, "event_json").len(),
                RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES
            );
        }
        "nip09_project_signed_content_bytes_overflow" => {
            assert_eq!(
                fixture_raw_event(vector).content.len(),
                RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES + 1
            );
        }
        "nip09_project_signed_tag_count_overflow" => {
            assert_eq!(
                fixture_raw_event(vector).tags.len(),
                RADROOTS_NIP09_DELETION_TAG_MAX_COUNT + 1
            );
        }
        "nip09_project_signed_tag_element_count_overflow" => {
            assert_eq!(
                fixture_raw_event(vector)
                    .tags
                    .iter()
                    .map(Vec::len)
                    .sum::<usize>(),
                RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT + 1
            );
        }
        "nip09_project_signed_tag_element_bytes_overflow" => {
            assert_eq!(
                fixture_raw_event(vector)
                    .tags
                    .iter()
                    .flat_map(|tag| tag.iter())
                    .map(String::len)
                    .max(),
                Some(RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES + 1)
            );
        }
        "nip09_project_signed_tag_bytes_overflow" => {
            assert_eq!(
                tag_bytes(&fixture_raw_event(vector).tags),
                RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES + 1
            );
        }
        "nip09_project_signed_event_wire_bytes_overflow"
        | "nip09_project_signed_wire_precedes_target_parse" => {
            assert_eq!(
                input_str(vector, "event_json").len(),
                RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES + 1
            );
        }
        _ => {}
    }
}

fn input_array<'a>(vector: &'a Vector, field: &str) -> &'a [Value] {
    vector.input[field]
        .as_array()
        .unwrap_or_else(|| panic!("{} input.{field} must be an array", vector.id))
}

fn input_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector.input[field]
        .as_str()
        .unwrap_or_else(|| panic!("{} input.{field} must be a string", vector.id))
}

fn object_str<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
    vector_id: &str,
) -> &'a str {
    object[field]
        .as_str()
        .unwrap_or_else(|| panic!("{vector_id} {field} must be a string"))
}

fn object_u32(object: &serde_json::Map<String, Value>, field: &str, vector_id: &str) -> u32 {
    u32::try_from(
        object[field]
            .as_u64()
            .unwrap_or_else(|| panic!("{vector_id} {field} must be an integer")),
    )
    .unwrap_or_else(|_| panic!("{vector_id} {field} must fit u32"))
}

fn expected_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector.expected[field]
        .as_str()
        .unwrap_or_else(|| panic!("{} expected.{field} must be a string", vector.id))
}

fn tag_bytes(tags: &[Vec<String>]) -> usize {
    tags.iter()
        .flat_map(|tag| tag.iter())
        .map(String::len)
        .sum()
}

fn value_tag_bytes(tags: &[Value]) -> usize {
    tags.iter()
        .flat_map(|tag| tag.as_array().expect("tag array"))
        .map(|element| element.as_str().expect("tag element").len())
        .sum()
}
