use super::{
    ConformanceVectorEntry, OperationsContractManifest, collect_non_empty_set,
    validate_conformance_vector_file, validate_operation_case_kinds,
};
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub(super) const ADMISSION_CONFORMANCE_VECTOR_RELATIVE: &str =
    "contracts/conformance/vectors/event/verified_admission.v1.json";

const REQUIRED_ADMISSION_PUBLIC_TYPES: [&str; 4] = [
    "RadrootsSignatureVerifiedEvent",
    "RadrootsContractValidatedEvent",
    "RadrootsAdmittedEvent",
    "RadrootsEventAdmissionError",
];

const ADMISSION_CASE_KINDS: [&str; 2] =
    ["event.admit_verified.valid", "event.admit_verified.invalid"];

#[derive(Clone, Copy)]
enum ExpectedAdmission {
    Valid {
        variant: &'static str,
        contract_id: &'static str,
    },
    Invalid {
        error_variant: &'static str,
        error_code: &'static str,
    },
}

#[derive(Clone, Copy)]
struct AdmissionVectorExpectation {
    id: &'static str,
    outcome: ExpectedAdmission,
}

const ADMISSION_VECTOR_EXPECTATIONS: [AdmissionVectorExpectation; 13] = [
    AdmissionVectorExpectation {
        id: "event_admit_verified_profile_001",
        outcome: ExpectedAdmission::Valid {
            variant: "profile",
            contract_id: "radroots.profile.metadata.v1",
        },
    },
    AdmissionVectorExpectation {
        id: "event_admit_verified_root_update_002",
        outcome: ExpectedAdmission::Valid {
            variant: "root_post",
            contract_id: "radroots.social.update.v1",
        },
    },
    AdmissionVectorExpectation {
        id: "event_admit_verified_root_photo_update_003",
        outcome: ExpectedAdmission::Valid {
            variant: "root_post",
            contract_id: "radroots.social.photo_update.v1",
        },
    },
    AdmissionVectorExpectation {
        id: "event_admit_verified_root_ask_004",
        outcome: ExpectedAdmission::Valid {
            variant: "root_post",
            contract_id: "radroots.social.ask.v1",
        },
    },
    AdmissionVectorExpectation {
        id: "event_admit_verified_post_to_reply_005",
        outcome: ExpectedAdmission::Valid {
            variant: "reply",
            contract_id: "radroots.social.reply.v1",
        },
    },
    AdmissionVectorExpectation {
        id: "event_admit_verified_comment_006",
        outcome: ExpectedAdmission::Valid {
            variant: "comment",
            contract_id: "radroots.social.comment.v1",
        },
    },
    AdmissionVectorExpectation {
        id: "event_admit_verified_deletion_request_007",
        outcome: ExpectedAdmission::Valid {
            variant: "deletion_request",
            contract_id: "radroots.social.deletion_request.v1",
        },
    },
    AdmissionVectorExpectation {
        id: "event_admit_verified_food_availability_008",
        outcome: ExpectedAdmission::Valid {
            variant: "food_availability",
            contract_id: "radroots.food.availability.v1",
        },
    },
    AdmissionVectorExpectation {
        id: "event_admit_verified_operational_fallback_009",
        outcome: ExpectedAdmission::Valid {
            variant: "contract_validated",
            contract_id: "radroots.operational_listing.published.v1",
        },
    },
    AdmissionVectorExpectation {
        id: "event_admit_verified_unsupported_kind_010",
        outcome: ExpectedAdmission::Invalid {
            error_variant: "contract_match",
            error_code: "unsupported_kind",
        },
    },
    AdmissionVectorExpectation {
        id: "event_admit_verified_generic_nip99_excluded_011",
        outcome: ExpectedAdmission::Invalid {
            error_variant: "contract_match",
            error_code: "unsupported_shape",
        },
    },
    AdmissionVectorExpectation {
        id: "event_admit_verified_operational_invalid_shape_012",
        outcome: ExpectedAdmission::Invalid {
            error_variant: "contract_validation",
            error_code: "missing_tag",
        },
    },
    AdmissionVectorExpectation {
        id: "event_admit_verified_ambiguous_food_markers_013",
        outcome: ExpectedAdmission::Invalid {
            error_variant: "food_availability",
            error_code: "food_profile_ambiguous",
        },
    },
];

pub(super) fn validate_admission_operation_authority(
    manifest: &OperationsContractManifest,
    workspace_root: &Path,
) -> Result<(), String> {
    let vector = validate_conformance_vector_file(
        &workspace_root.join(ADMISSION_CONFORMANCE_VECTOR_RELATIVE),
        &manifest.contract.version,
    )?;
    validate_admission_operation_inventory(manifest, &vector)?;
    validate_source_witnesses(workspace_root)?;
    Ok(())
}

pub(super) fn validate_admission_operation_inventory(
    manifest: &OperationsContractManifest,
    vector: &super::ConformanceVectorFile,
) -> Result<(), String> {
    validate_manifest_authority(manifest, vector)?;
    validate_vector_inventory(&vector.vectors)
}

fn validate_manifest_authority(
    manifest: &OperationsContractManifest,
    vector: &super::ConformanceVectorFile,
) -> Result<(), String> {
    let shared_types = collect_non_empty_set(
        &manifest.shared_types.public,
        "verified admission shared_types.public",
    )?;
    for required in REQUIRED_ADMISSION_PUBLIC_TYPES {
        if !shared_types.contains(required) {
            return Err(format!(
                "verified admission authority requires shared public type {required}"
            ));
        }
    }

    let actual_keys = manifest
        .operations
        .iter()
        .filter(|(key, operation)| {
            key.starts_with("event_admit_verified")
                || operation.id.starts_with("event.admit_verified")
                || operation.conformance.vector == ADMISSION_CONFORMANCE_VECTOR_RELATIVE
        })
        .map(|(key, _)| key.as_str())
        .collect::<BTreeSet<_>>();
    let expected_keys = BTreeSet::from(["event_admit_verified"]);
    if actual_keys != expected_keys {
        return Err(format!(
            "verified admission operation authority drift: expected {expected_keys:?}, got {actual_keys:?}"
        ));
    }

    let operation = manifest
        .operations
        .get("event_admit_verified")
        .ok_or_else(|| {
            "verified admission operation event_admit_verified is required".to_string()
        })?;
    require_scalar("domain", &operation.domain, "event")?;
    require_scalar("id", &operation.id, "event.admit_verified")?;
    require_scalar("stability", &operation.stability, "beta")?;
    require_scalar("error_class", &operation.error_class, "admission_error")?;
    require_scalar("signing", &operation.signing, "none")?;
    require_scalar("transport", &operation.transport, "none")?;
    if !operation.deterministic {
        return Err(
            "verified admission operation deterministic drift: expected true, got false"
                .to_string(),
        );
    }
    require_sequence(
        "inputs",
        &operation.inputs,
        &["RadrootsSignatureVerifiedEvent"],
    )?;
    require_sequence("outputs", &operation.outputs, &["RadrootsAdmittedEvent"])?;
    require_sequence(
        "implementation.rust_modules",
        &operation.implementation.rust_modules,
        &[
            "crates/event_codec/src/admission.rs",
            "crates/event_codec/src/verification.rs",
        ],
    )?;
    require_sequence(
        "implementation.rust_types",
        &operation.implementation.rust_types,
        &[
            "radroots_event_codec::admission::RadrootsAdmittedEvent",
            "radroots_event_codec::admission::RadrootsEventAdmissionError",
            "radroots_event_codec::verification::RadrootsContractValidatedEvent",
            "radroots_event_codec::verification::RadrootsSignatureVerifiedEvent",
        ],
    )?;
    require_scalar(
        "conformance.vector",
        &operation.conformance.vector,
        ADMISSION_CONFORMANCE_VECTOR_RELATIVE,
    )?;
    validate_operation_case_kinds(operation, vector)?;
    require_sequence(
        "conformance.case_kinds",
        &operation.conformance.case_kinds,
        &ADMISSION_CASE_KINDS,
    )
}

fn validate_source_witnesses(workspace_root: &Path) -> Result<(), String> {
    for (relative, fragments) in [
        (
            "crates/event_codec/src/admission.rs",
            &[
                "pub enum RadrootsAdmittedEvent",
                "pub enum RadrootsEventAdmissionError",
                "pub fn admit_verified_event(",
            ][..],
        ),
        (
            "crates/event_codec/src/verification/v1.rs",
            &[
                "pub struct RadrootsSignatureVerifiedEvent",
                "pub struct RadrootsContractValidatedEvent",
            ][..],
        ),
    ] {
        let source = fs::read_to_string(workspace_root.join(relative)).map_err(|error| {
            format!("failed to read verified admission witness {relative}: {error}")
        })?;
        for fragment in fragments {
            if !source.contains(fragment) {
                return Err(format!(
                    "verified admission witness {relative} is missing `{fragment}`"
                ));
            }
        }
    }
    Ok(())
}

fn validate_vector_inventory(vectors: &[ConformanceVectorEntry]) -> Result<(), String> {
    let expected = ADMISSION_VECTOR_EXPECTATIONS
        .iter()
        .map(|entry| (entry.id, entry.outcome))
        .collect::<BTreeMap<_, _>>();
    let mut actual = BTreeMap::new();
    let mut event_ids = BTreeSet::new();

    for vector in vectors {
        let expectation = expected.get(vector.id.as_str()).ok_or_else(|| {
            format!(
                "verified admission conformance vector has unexpected id {}",
                vector.id
            )
        })?;
        if actual
            .insert(vector.id.as_str(), vector.kind.as_str())
            .is_some()
        {
            return Err(format!(
                "verified admission conformance vector has duplicate id {}",
                vector.id
            ));
        }
        validate_vector(vector, *expectation, &mut event_ids)?;
    }

    let expected_inventory = ADMISSION_VECTOR_EXPECTATIONS
        .iter()
        .map(|entry| {
            let kind = match entry.outcome {
                ExpectedAdmission::Valid { .. } => "event.admit_verified.valid",
                ExpectedAdmission::Invalid { .. } => "event.admit_verified.invalid",
            };
            (entry.id, kind)
        })
        .collect::<BTreeMap<_, _>>();
    if actual != expected_inventory {
        return Err(format!(
            "verified admission conformance inventory drift: expected {expected_inventory:?}, got {actual:?}"
        ));
    }
    Ok(())
}

fn validate_vector(
    vector: &ConformanceVectorEntry,
    expectation: ExpectedAdmission,
    event_ids: &mut BTreeSet<String>,
) -> Result<(), String> {
    let input = exact_object(&vector.input, &["event"], &format!("{}.input", vector.id))?;
    let event = exact_object(
        &input["event"],
        &[
            "content",
            "created_at",
            "id",
            "kind",
            "pubkey",
            "sig",
            "tags",
        ],
        &format!("{}.input.event", vector.id),
    )?;
    let event_id = required_string(event, "id", &vector.id)?;
    let pubkey = required_string(event, "pubkey", &vector.id)?;
    let signature = required_string(event, "sig", &vector.id)?;
    require_lower_hex(event_id, 64, "event id", &vector.id)?;
    require_lower_hex(pubkey, 64, "pubkey", &vector.id)?;
    require_lower_hex(signature, 128, "signature", &vector.id)?;
    if !event_ids.insert(event_id.to_string()) {
        return Err(format!(
            "verified admission vector {} reuses event id {event_id}",
            vector.id
        ));
    }
    if !event["created_at"].is_u64()
        || !event["kind"].is_u64()
        || !event["tags"].is_array()
        || !event["content"].is_string()
    {
        return Err(format!(
            "verified admission vector {} must contain a complete typed signed event",
            vector.id
        ));
    }

    match expectation {
        ExpectedAdmission::Valid {
            variant,
            contract_id,
        } => {
            if vector.kind != "event.admit_verified.valid" {
                return Err(format!(
                    "verified admission vector {} kind drift",
                    vector.id
                ));
            }
            let expected = exact_object(
                &vector.expected,
                &["contract_id", "event_id", "variant"],
                &format!("{}.expected", vector.id),
            )?;
            require_expected(expected, "variant", variant, &vector.id)?;
            require_expected(expected, "contract_id", contract_id, &vector.id)?;
            require_expected(expected, "event_id", event_id, &vector.id)?;
        }
        ExpectedAdmission::Invalid {
            error_variant,
            error_code,
        } => {
            if vector.kind != "event.admit_verified.invalid" {
                return Err(format!(
                    "verified admission vector {} kind drift",
                    vector.id
                ));
            }
            let expected = exact_object(
                &vector.expected,
                &["error_code", "error_variant", "event_id"],
                &format!("{}.expected", vector.id),
            )?;
            require_expected(expected, "error_variant", error_variant, &vector.id)?;
            require_expected(expected, "error_code", error_code, &vector.id)?;
            require_expected(expected, "event_id", event_id, &vector.id)?;
        }
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("verified admission {label} must be an object"))?;
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!(
            "verified admission {label} field inventory drift: expected {expected:?}, got {actual:?}"
        ));
    }
    Ok(object)
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    vector_id: &str,
) -> Result<&'a str, String> {
    object[field].as_str().ok_or_else(|| {
        format!("verified admission vector {vector_id} field {field} must be a string")
    })
}

fn require_lower_hex(
    value: &str,
    length: usize,
    label: &str,
    vector_id: &str,
) -> Result<(), String> {
    if value.len() != length
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        || value.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(format!(
            "verified admission vector {vector_id} {label} must be {length} lowercase hex characters"
        ));
    }
    Ok(())
}

fn require_expected(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
    vector_id: &str,
) -> Result<(), String> {
    let actual = required_string(object, field, vector_id)?;
    if actual != expected {
        return Err(format!(
            "verified admission vector {vector_id} expected.{field} drift: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

fn require_scalar(field: &str, actual: &str, expected: &str) -> Result<(), String> {
    if actual != expected {
        return Err(format!(
            "verified admission operation {field} drift: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

fn require_sequence(field: &str, actual: &[String], expected: &[&str]) -> Result<(), String> {
    if !actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        return Err(format!(
            "verified admission operation {field} drift: expected {expected:?}, got {actual:?}"
        ));
    }
    Ok(())
}
