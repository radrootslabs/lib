use super::artifact_bundle::{
    GeneratedArtifact, read_regular_file, with_artifact_bundle_transaction,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::Path;

const CONTRACT_ID: &str = "radroots_outbox.phase1_publication.v1";
const WRITE_COMMAND: &str = "cargo xtask contract outbox-phase1-publication-manifest --write";
const DESCRIPTOR_RELATIVE: &str = "crates/outbox/contracts/phase1_publication_v1.descriptor.json";
const MANIFEST_RELATIVE: &str = "crates/outbox/contracts/phase1_publication_v1.manifest.json";
const MANIFEST_SCHEMA_RELATIVE: &str =
    "crates/outbox/contracts/phase1_publication_v1.manifest.schema.json";
const MANIFEST_SHA256_RELATIVE: &str =
    "crates/outbox/contracts/phase1_publication_v1.manifest.sha256";
const VECTOR_RELATIVE: &str = "contracts/conformance/vectors/outbox/phase1_publication.v1.json";
const VECTOR_MIRROR_RELATIVE: &str = "crates/outbox/tests/fixtures/phase1_publication.v1.json";
const VECTOR_EXECUTOR_RELATIVE: &str = "crates/outbox/tests/phase1_publication_v1_result_vector.rs";
const VECTOR_EXECUTOR_ID: &str = "radroots_outbox.phase1_publication.v1.result_vector_executor.v1";
const VECTOR_EXECUTOR_TEST: &str = "phase1_publication_v1_result_vector";
const RELEASE_RELATIVE: &str = "contracts/releases/1.0.0-alpha.1.toml";
const CHANGELOG_RELATIVE: &str = "CHANGELOG.md";
const RELEASE_CHANGE_ID: &str = "outbox-phase1-publication-state";
const MIGRATION_REGISTRY_RELATIVE: &str = "crates/outbox/contracts/migration_registry.v1.json";
const MIGRATION_UP_RELATIVE: &str = "crates/outbox/migrations/0002_phase1_publication.up.sql";
const MIGRATION_DOWN_RELATIVE: &str = "crates/outbox/migrations/0002_phase1_publication.down.sql";
const MIGRATION_UP_SHA256: &str =
    "84f0c9897cff8d002961cb6ad9dee53edcf28853d1407483519b00bdbf029308";
const MIGRATION_DOWN_SHA256: &str =
    "57a5a00ca4257973097acf7f5cc64494dd0bc73fcfa00af2e6c8bb1f61823928";
const MIGRATION_SCHEMA_SHA256: &str =
    "a56af9ba400fd51c97d48886fbb3f3733adb97458d7109fa8989c1b7e0c8bcaf";

const SOURCE_FILES: &[(&str, &str)] = &[
    ("outbox_package_manifest", "crates/outbox/Cargo.toml"),
    ("outbox_public_surface", "crates/outbox/src/lib.rs"),
    (
        "phase1_publication_runtime",
        "crates/outbox/src/phase1_publication.rs",
    ),
    ("outbox_schema_runtime", "crates/outbox/src/schema.rs"),
    (
        "migration_registry_runtime",
        "crates/outbox/src/generated/outbox_migration_registry.rs",
    ),
    ("migration_registry_source", MIGRATION_REGISTRY_RELATIVE),
    ("vector_executor", VECTOR_EXECUTOR_RELATIVE),
    (
        "contract_governance",
        "tools/xtask/src/contract/outbox_phase1_publication.rs",
    ),
    ("contract_dispatch", "tools/xtask/src/contract.rs"),
    ("xtask_dispatch", "tools/xtask/src/main.rs"),
    ("release_record", RELEASE_RELATIVE),
    ("release_notes", CHANGELOG_RELATIVE),
];

const EVENT_STATES: &[&str] = &[
    "ready",
    "claimed-for-signing",
    "signed-ready",
    "dispatching",
    "published",
    "failed-retryable",
    "failed-terminal",
    "quarantined",
    "cancelled",
];
const TARGET_STATES: &[&str] = &[
    "pending",
    "in-flight",
    "accepted-observation-pending",
    "accepted-observed",
    "failed-retryable",
    "failed-terminal",
    "uncertain",
    "cancelled",
];
const CASE_IDS: &[&str] = &[
    "duplicate_enqueue",
    "empty_required_policy",
    "expired_lease_reclaim",
    "identity_preimages",
    "migration_rollback_reopen",
    "stale_claim_rejected",
    "target_count_exact",
    "target_count_one_over",
    "target_uri_exact",
    "target_uri_one_over",
    "two_worker_claim_race",
    "typed_enqueue",
];
const ERROR_CODES: &[&str] = &[
    "phase1_publication_artifact_invalid",
    "phase1_publication_claim_invalid",
    "phase1_publication_diagnostic_too_large",
    "phase1_publication_entropy_unavailable",
    "phase1_publication_idempotency_conflict",
    "phase1_publication_integer_range",
    "phase1_publication_lease_invalid",
    "phase1_publication_not_found",
    "phase1_publication_readiness_invalid",
    "phase1_publication_required_target_count",
    "phase1_publication_revision_conflict",
    "phase1_publication_signed_event_invalid",
    "phase1_publication_signed_event_mismatch",
    "phase1_publication_sqlite",
    "phase1_publication_state_conflict",
    "phase1_publication_stored_authority_invalid",
    "phase1_publication_stored_digest_invalid",
    "phase1_publication_stored_state_invalid",
    "phase1_publication_stored_value_too_large",
    "phase1_publication_target_count",
    "phase1_publication_target_duplicate",
    "phase1_publication_target_not_found",
    "phase1_publication_target_uri_invalid",
    "phase1_publication_target_uri_too_large",
    "phase1_publication_time_invalid",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Descriptor {
    schema_version: u32,
    contract_id: String,
    migration: Migration,
    resource_limits: ResourceLimits,
    operation_identity: Identity,
    dispatch_identity: Identity,
    event_states: Vec<String>,
    target_states: Vec<String>,
    transitions: Vec<Transition>,
    stable_errors: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Migration {
    version: u32,
    name: String,
    up_path: String,
    up_sha256: String,
    down_path: String,
    down_sha256: String,
    schema_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceLimits {
    target_count: usize,
    target_uri_bytes: usize,
    diagnostic_bytes: usize,
    claim_lease_millis: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    algorithm: String,
    domain: String,
    domain_terminator_hex: String,
    preimage: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transition {
    id: String,
    scope: String,
    from: String,
    to: String,
    revision_cas: bool,
    lease_predicate: String,
    durable_side_effect: String,
    retry_class: String,
    repair_edge: bool,
    terminal_destination: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Vector {
    schema_version: u32,
    contract_id: String,
    executor: Executor,
    identity_vector: Value,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Executor {
    id: String,
    path: String,
    test: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    execution: String,
    expected_outcome: String,
    expected_error: Option<String>,
}

pub(crate) fn write_outbox_phase1_publication_manifest(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |transaction| {
        transaction.write(expected_artifacts(workspace_root)?)?;
        validate_under_lock(workspace_root)
    })
}

pub(crate) fn validate_outbox_phase1_publication_manifest(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| validate_under_lock(workspace_root))
}

fn validate_under_lock(workspace_root: &Path) -> Result<(), String> {
    for artifact in expected_artifacts(workspace_root)? {
        let actual = read_regular_file(workspace_root, artifact.relative)?;
        if actual != artifact.contents {
            return Err(format!(
                "generated Phase 1 publication artifact {} is stale; run `{WRITE_COMMAND}`",
                artifact.relative
            ));
        }
    }
    let manifest_bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    let schema_bytes = read_regular_file(workspace_root, MANIFEST_SCHEMA_RELATIVE)?;
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("parse {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| format!("compile {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    let errors = validator
        .iter_errors(&manifest)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        return Err(format!(
            "{MANIFEST_RELATIVE} violates its schema: {}",
            errors.join("; ")
        ));
    }
    let sidecar = read_regular_file(workspace_root, MANIFEST_SHA256_RELATIVE)?;
    if sidecar != format!("{}\n", sha256_hex(&manifest_bytes)).as_bytes() {
        return Err(format!(
            "{MANIFEST_SHA256_RELATIVE} must authenticate exact manifest bytes"
        ));
    }
    Ok(())
}

fn expected_artifacts(workspace_root: &Path) -> Result<Vec<GeneratedArtifact>, String> {
    let descriptor = load_descriptor(workspace_root)?;
    validate_vector(workspace_root)?;
    validate_release(workspace_root)?;
    let schema_bytes = canonical_json_bytes(&manifest_schema())?;
    let manifest = expected_manifest(workspace_root, &descriptor, &schema_bytes)?;
    let manifest_bytes = canonical_json_bytes(&manifest)?;
    let vector = read_regular_file(workspace_root, VECTOR_RELATIVE)?;
    Ok(vec![
        GeneratedArtifact {
            relative: MANIFEST_RELATIVE,
            contents: manifest_bytes.clone(),
        },
        GeneratedArtifact {
            relative: MANIFEST_SCHEMA_RELATIVE,
            contents: schema_bytes,
        },
        GeneratedArtifact {
            relative: MANIFEST_SHA256_RELATIVE,
            contents: format!("{}\n", sha256_hex(&manifest_bytes)).into_bytes(),
        },
        GeneratedArtifact {
            relative: VECTOR_MIRROR_RELATIVE,
            contents: vector,
        },
    ])
}

fn load_descriptor(workspace_root: &Path) -> Result<Descriptor, String> {
    let bytes = read_regular_file(workspace_root, DESCRIPTOR_RELATIVE)?;
    let descriptor: Descriptor = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {DESCRIPTOR_RELATIVE}: {error}"))?;
    if descriptor.schema_version != 1 || descriptor.contract_id != CONTRACT_ID {
        return Err("Phase 1 publication descriptor identity is invalid".to_owned());
    }
    if descriptor.migration.version != 2
        || descriptor.migration.name != "phase1_publication"
        || descriptor.migration.up_path != MIGRATION_UP_RELATIVE
        || descriptor.migration.down_path != MIGRATION_DOWN_RELATIVE
        || descriptor.migration.up_sha256 != MIGRATION_UP_SHA256
        || descriptor.migration.down_sha256 != MIGRATION_DOWN_SHA256
        || descriptor.migration.schema_sha256 != MIGRATION_SCHEMA_SHA256
        || sha256_hex(&read_regular_file(workspace_root, MIGRATION_UP_RELATIVE)?)
            != MIGRATION_UP_SHA256
        || sha256_hex(&read_regular_file(workspace_root, MIGRATION_DOWN_RELATIVE)?)
            != MIGRATION_DOWN_SHA256
    {
        return Err("Phase 1 publication migration authority is invalid".to_owned());
    }
    if descriptor.resource_limits.target_count != 16
        || descriptor.resource_limits.target_uri_bytes != 2_048
        || descriptor.resource_limits.diagnostic_bytes != 4_096
        || descriptor.resource_limits.claim_lease_millis != 300_000
    {
        return Err("Phase 1 publication resource limits are invalid".to_owned());
    }
    validate_identity(
        &descriptor.operation_identity,
        "radroots.phase1.publication-operation.v1",
        &[
            "artifact_digest_32",
            "media_readiness_binding_digest_32",
            "expected_author_32",
            "target_policy_digest_32",
        ],
    )?;
    validate_identity(
        &descriptor.dispatch_identity,
        "radroots.phase1.relay-dispatch.v1",
        &[
            "event_id_32",
            "target_policy_digest_32",
            "endpoint_fingerprint_32",
        ],
    )?;
    if descriptor
        .event_states
        .iter()
        .map(String::as_str)
        .ne(EVENT_STATES.iter().copied())
        || descriptor
            .target_states
            .iter()
            .map(String::as_str)
            .ne(TARGET_STATES.iter().copied())
    {
        return Err("Phase 1 publication state inventory is invalid".to_owned());
    }
    validate_transitions(&descriptor)?;
    if descriptor
        .stable_errors
        .iter()
        .map(String::as_str)
        .ne(ERROR_CODES.iter().copied())
    {
        return Err("Phase 1 publication stable-error inventory is invalid".to_owned());
    }
    let registry: Value = serde_json::from_slice(&read_regular_file(
        workspace_root,
        MIGRATION_REGISTRY_RELATIVE,
    )?)
    .map_err(|error| format!("parse {MIGRATION_REGISTRY_RELATIVE}: {error}"))?;
    let migration = registry["migrations"]
        .as_array()
        .and_then(|migrations| migrations.iter().find(|entry| entry["version"] == 2))
        .ok_or_else(|| "migration registry does not contain Phase 1 version 2".to_owned())?;
    if migration["name"] != descriptor.migration.name
        || migration["up_sha256"] != descriptor.migration.up_sha256
        || migration["down_sha256"] != descriptor.migration.down_sha256
        || migration["schema_sha256"] != descriptor.migration.schema_sha256
    {
        return Err("migration registry and Phase 1 descriptor disagree".to_owned());
    }
    Ok(descriptor)
}

fn validate_identity(identity: &Identity, domain: &str, preimage: &[&str]) -> Result<(), String> {
    if identity.algorithm != "sha256_raw_fixed_width_v1"
        || identity.domain != domain
        || identity.domain_terminator_hex != "00"
        || identity
            .preimage
            .iter()
            .map(String::as_str)
            .ne(preimage.iter().copied())
    {
        return Err(format!("Phase 1 identity `{domain}` is invalid"));
    }
    Ok(())
}

fn validate_transitions(descriptor: &Descriptor) -> Result<(), String> {
    if descriptor.transitions.len() != 25 {
        return Err("Phase 1 publication transition inventory must contain 25 entries".to_owned());
    }
    let event_states = descriptor.event_states.iter().collect::<BTreeSet<_>>();
    let target_states = descriptor.target_states.iter().collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    for transition in &descriptor.transitions {
        let states = match transition.scope.as_str() {
            "event" => &event_states,
            "target" => &target_states,
            _ => return Err(format!("invalid transition scope `{}`", transition.scope)),
        };
        if !ids.insert(transition.id.as_str())
            || !states.contains(&transition.from)
            || !states.contains(&transition.to)
            || !transition.revision_cas
            || transition.lease_predicate.is_empty()
            || transition.durable_side_effect.is_empty()
            || !matches!(
                transition.retry_class.as_str(),
                "none" | "retryable" | "repair" | "terminal"
            )
            || transition.repair_edge != (transition.retry_class == "repair")
            || transition.terminal_destination
                != matches!(
                    transition.to.as_str(),
                    "published"
                        | "failed-terminal"
                        | "quarantined"
                        | "cancelled"
                        | "accepted-observed"
                )
        {
            return Err(format!("invalid transition `{}`", transition.id));
        }
    }
    Ok(())
}

fn validate_vector(workspace_root: &Path) -> Result<(), String> {
    let bytes = read_regular_file(workspace_root, VECTOR_RELATIVE)?;
    let vector: Vector = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {VECTOR_RELATIVE}: {error}"))?;
    if vector.schema_version != 1
        || vector.contract_id != CONTRACT_ID
        || vector.executor.id != VECTOR_EXECUTOR_ID
        || vector.executor.path != VECTOR_EXECUTOR_RELATIVE
        || vector.executor.test != VECTOR_EXECUTOR_TEST
        || !vector.identity_vector.is_object()
    {
        return Err("Phase 1 publication vector identity is invalid".to_owned());
    }
    let expected = CASE_IDS.iter().copied().collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    for case in vector.cases {
        if case.execution != "direct_executor"
            || case.expected_outcome.is_empty()
            || case.expected_error.is_some() != (case.expected_outcome == "rejected")
            || !actual.insert(case.id)
        {
            return Err("Phase 1 publication vector case is invalid".to_owned());
        }
    }
    if actual.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected {
        return Err("Phase 1 publication vector case inventory is incomplete".to_owned());
    }
    Ok(())
}

fn validate_release(workspace_root: &Path) -> Result<(), String> {
    let source = read_regular_file(workspace_root, RELEASE_RELATIVE)?;
    let release: toml::Value = toml::from_str(
        std::str::from_utf8(&source)
            .map_err(|error| format!("decode {RELEASE_RELATIVE}: {error}"))?,
    )
    .map_err(|error| format!("parse {RELEASE_RELATIVE}: {error}"))?;
    let changes = release["changes"]
        .as_array()
        .ok_or_else(|| format!("{RELEASE_RELATIVE} has no changes"))?;
    if changes
        .iter()
        .filter(|change| change["id"].as_str() == Some(RELEASE_CHANGE_ID))
        .count()
        != 1
    {
        return Err(format!(
            "{RELEASE_RELATIVE} must declare one `{RELEASE_CHANGE_ID}` change"
        ));
    }
    let changelog = read_regular_file(workspace_root, CHANGELOG_RELATIVE)?;
    let changelog = std::str::from_utf8(&changelog)
        .map_err(|error| format!("decode {CHANGELOG_RELATIVE}: {error}"))?;
    let marker = format!("<!-- release-change: {RELEASE_CHANGE_ID} -->");
    if changelog.matches(&marker).count() != 1 {
        return Err(format!("{CHANGELOG_RELATIVE} must contain one `{marker}`"));
    }
    Ok(())
}

fn expected_manifest(
    workspace_root: &Path,
    descriptor: &Descriptor,
    schema_bytes: &[u8],
) -> Result<Value, String> {
    let sources = SOURCE_FILES
        .iter()
        .map(|(role, relative)| {
            Ok(json!({
                "role": role,
                "file": descriptor_for_file(workspace_root, relative)?,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(json!({
        "schema_version": 1,
        "contract_id": CONTRACT_ID,
        "manifest_schema": descriptor_for_bytes(MANIFEST_SCHEMA_RELATIVE, schema_bytes),
        "descriptor": descriptor_for_file(workspace_root, DESCRIPTOR_RELATIVE)?,
        "migration": {
            "version": descriptor.migration.version,
            "schema_sha256": descriptor.migration.schema_sha256,
            "up": descriptor_for_file(workspace_root, MIGRATION_UP_RELATIVE)?,
            "down": descriptor_for_file(workspace_root, MIGRATION_DOWN_RELATIVE)?,
        },
        "state_machine": {
            "event_state_count": descriptor.event_states.len(),
            "target_state_count": descriptor.target_states.len(),
            "transition_count": descriptor.transitions.len(),
            "stable_error_count": descriptor.stable_errors.len(),
        },
        "result_vector": {
            "canonical": descriptor_for_file(workspace_root, VECTOR_RELATIVE)?,
            "mirror_path": VECTOR_MIRROR_RELATIVE,
            "executor": descriptor_for_file(workspace_root, VECTOR_EXECUTOR_RELATIVE)?,
            "executor_id": VECTOR_EXECUTOR_ID,
            "executor_test": VECTOR_EXECUTOR_TEST,
        },
        "source_files": sources,
        "release": {
            "change_id": RELEASE_CHANGE_ID,
            "record": RELEASE_RELATIVE,
            "changelog": CHANGELOG_RELATIVE,
        },
    }))
}

fn manifest_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/contracts/outbox/phase1_publication_v1.manifest.schema.json",
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "contract_id", "manifest_schema", "descriptor", "migration", "state_machine", "result_vector", "source_files", "release"],
        "properties": {
            "schema_version": { "const": 1 },
            "contract_id": { "const": CONTRACT_ID },
            "manifest_schema": { "$ref": "#/$defs/file" },
            "descriptor": { "$ref": "#/$defs/file" },
            "migration": {
                "type": "object", "additionalProperties": false,
                "required": ["version", "schema_sha256", "up", "down"],
                "properties": {
                    "version": { "const": 2 },
                    "schema_sha256": { "$ref": "#/$defs/sha256" },
                    "up": { "$ref": "#/$defs/file" },
                    "down": { "$ref": "#/$defs/file" }
                }
            },
            "state_machine": {
                "type": "object", "additionalProperties": false,
                "required": ["event_state_count", "target_state_count", "transition_count", "stable_error_count"],
                "properties": {
                    "event_state_count": { "const": 9 },
                    "target_state_count": { "const": 8 },
                    "transition_count": { "const": 25 },
                    "stable_error_count": { "const": 25 }
                }
            },
            "result_vector": {
                "type": "object", "additionalProperties": false,
                "required": ["canonical", "mirror_path", "executor", "executor_id", "executor_test"],
                "properties": {
                    "canonical": { "$ref": "#/$defs/file" },
                    "mirror_path": { "const": VECTOR_MIRROR_RELATIVE },
                    "executor": { "$ref": "#/$defs/file" },
                    "executor_id": { "const": VECTOR_EXECUTOR_ID },
                    "executor_test": { "const": VECTOR_EXECUTOR_TEST }
                }
            },
            "source_files": {
                "type": "array", "minItems": SOURCE_FILES.len(), "maxItems": SOURCE_FILES.len(),
                "items": {
                    "type": "object", "additionalProperties": false,
                    "required": ["role", "file"],
                    "properties": { "role": { "type": "string", "minLength": 1 }, "file": { "$ref": "#/$defs/file" } }
                }
            },
            "release": {
                "type": "object", "additionalProperties": false,
                "required": ["change_id", "record", "changelog"],
                "properties": {
                    "change_id": { "const": RELEASE_CHANGE_ID },
                    "record": { "const": RELEASE_RELATIVE },
                    "changelog": { "const": CHANGELOG_RELATIVE }
                }
            }
        },
        "$defs": {
            "sha256": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
            "file": {
                "type": "object", "additionalProperties": false,
                "required": ["path", "byte_length", "sha256"],
                "properties": {
                    "path": { "type": "string", "minLength": 1 },
                    "byte_length": { "type": "integer", "minimum": 1 },
                    "sha256": { "$ref": "#/$defs/sha256" }
                }
            }
        }
    })
}

fn descriptor_for_file(workspace_root: &Path, relative: &str) -> Result<Value, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    Ok(descriptor_for_bytes(relative, &bytes))
}

fn descriptor_for_bytes(relative: &str, bytes: &[u8]) -> Value {
    json!({
        "path": relative,
        "byte_length": bytes.len(),
        "sha256": sha256_hex(bytes),
    })
}

fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("serialize Phase 1 publication artifact: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
