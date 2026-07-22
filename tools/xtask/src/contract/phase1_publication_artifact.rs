use super::artifact_bundle::{
    GeneratedArtifact, read_regular_file, with_artifact_bundle_transaction,
};
use radroots_event_codec::wire::publication::{
    RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES,
    RADROOTS_PHASE1_PUBLICATION_ARTIFACT_SCHEMA_VERSION,
    RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT,
    RADROOTS_PHASE1_PUBLICATION_SIGNED_EVENT_MAX_BYTES,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const SCHEMA_VERSION: u32 = 1;
const CONTRACT_ID: &str = "radroots_event_codec.phase1_publication_artifact_v1";
const AUTHORITY_ID: &str = "phase1_publication_artifact_v1";
const HASH_ALGORITHM: &str = "sha256_bytes_v1";
const WRITE_COMMAND: &str = "cargo xtask contract phase1-publication-artifact-manifest --write";

const MANIFEST_RELATIVE: &str =
    "crates/event_codec/contracts/phase1_publication_artifact_v1.manifest.json";
const MANIFEST_SCHEMA_RELATIVE: &str =
    "crates/event_codec/contracts/phase1_publication_artifact_v1.manifest.schema.json";
const MANIFEST_SHA256_RELATIVE: &str =
    "crates/event_codec/contracts/phase1_publication_artifact_v1.manifest.sha256";
const VECTOR_RELATIVE: &str = "contracts/conformance/vectors/publication/phase1_artifact.v1.json";
const VECTOR_MIRROR_RELATIVE: &str =
    "crates/event_codec/tests/fixtures/phase1_publication_artifact.v1.json";
const VECTOR_EXECUTOR_RELATIVE: &str = "crates/event_codec/tests/publication_artifact.rs";
const VECTOR_EXECUTOR_TEST: &str = "publication_artifact_conformance_vector_executes_every_case";
const OPERATIONS_RELATIVE: &str = "contracts/operations.toml";
const RELEASE_RELATIVE: &str = "contracts/releases/1.0.0-alpha.1.toml";
const CHANGELOG_RELATIVE: &str = "CHANGELOG.md";
const RELEASE_CHANGE_ID: &str = "phase1-publication-artifact";
const CHANGELOG_MARKER: &str = "<!-- release-change: phase1-publication-artifact -->";
const REGISTRY_RELATIVE: &str = "contracts/event_store/event_contract_registry_v7.inventory.json";
const REGISTRY_SIDECAR_RELATIVE: &str =
    "contracts/event_store/event_contract_registry_v7.inventory.sha256";

const RAW_MANIFEST_RELATIVE: &str =
    "crates/event_store/contracts/raw_source_rebuild_v1.manifest.json";
const RAW_MANIFEST_SCHEMA_RELATIVE: &str =
    "crates/event_store/contracts/raw_source_rebuild_v1.manifest.schema.json";
const RAW_MANIFEST_SHA256_RELATIVE: &str =
    "crates/event_store/contracts/raw_source_rebuild_v1.manifest.sha256";
const RAW_GENERATED_DESCRIPTOR_RELATIVE: &str =
    "crates/event_store/src/generated/raw_source_rebuild_manifest.rs";
const RAW_VECTOR_RELATIVE: &str =
    "contracts/conformance/vectors/event_store/raw_source_rebuild.v1.json";
const RAW_VECTOR_MIRROR_RELATIVE: &str =
    "crates/event_store/tests/fixtures/raw_source_rebuild.v1.json";
const RAW_VECTOR_EXECUTOR_RELATIVE: &str =
    "crates/event_store/tests/raw_source_rebuild_v1_result_vector.rs";

const GENERATED_ARTIFACT_PATHS: &[&str] = &[
    MANIFEST_RELATIVE,
    MANIFEST_SCHEMA_RELATIVE,
    MANIFEST_SHA256_RELATIVE,
    VECTOR_MIRROR_RELATIVE,
];

const PUBLIC_SOURCE_ROOTS: &[&str] = &[
    "crates/blossom/src",
    "crates/core/src",
    "crates/event/src",
    "crates/event_codec/src",
    "crates/transport/src",
];

const EXPLICIT_SOURCE_SPECS: &[(&str, &str)] = &[
    ("cargo_config_authority", ".cargo/config.toml"),
    ("workspace_manifest_authority", "Cargo.toml"),
    ("workspace_lockfile_authority", "Cargo.lock"),
    ("rust_toolchain_authority", "rust-toolchain.toml"),
    ("blossom_manifest_authority", "crates/blossom/Cargo.toml"),
    ("core_manifest_authority", "crates/core/Cargo.toml"),
    ("event_manifest_authority", "crates/event/Cargo.toml"),
    (
        "event_codec_manifest_authority",
        "crates/event_codec/Cargo.toml",
    ),
    ("event_codec_documentation", "crates/event_codec/README"),
    (
        "event_store_manifest_authority",
        "crates/event_store/Cargo.toml",
    ),
    (
        "transport_manifest_authority",
        "crates/transport/Cargo.toml",
    ),
    ("xtask_manifest_authority", "tools/xtask/Cargo.toml"),
    ("publication_vector_executor", VECTOR_EXECUTOR_RELATIVE),
    ("operations_authority", OPERATIONS_RELATIVE),
    ("release_authority", RELEASE_RELATIVE),
    ("release_notes", CHANGELOG_RELATIVE),
    ("contract_command_authority", "tools/xtask/src/contract.rs"),
    ("contract_dispatch_authority", "tools/xtask/src/main.rs"),
    (
        "superseded_nip09_contract_governance",
        "tools/xtask/src/contract/nip09_reconciliation.rs",
    ),
    (
        "superseded_raw_rebuild_contract_governance",
        "tools/xtask/src/contract/raw_source_rebuild.rs",
    ),
    (
        "publication_contract_governance",
        "tools/xtask/src/contract/phase1_publication_artifact.rs",
    ),
];

const RAW_PREDECESSOR_SUPERSEDED_PATHS: &[&str] = &[
    CHANGELOG_RELATIVE,
    RELEASE_RELATIVE,
    "tools/xtask/src/contract.rs",
    "tools/xtask/src/main.rs",
    "tools/xtask/src/contract/nip09_reconciliation.rs",
    "tools/xtask/src/contract/raw_source_rebuild.rs",
];

const PUBLIC_TYPES: &[&str] = &[
    "RadrootsPhase1PublicationEventVariant",
    "RadrootsPhase1PublicationSemanticVariant",
    "RadrootsPhase1PublicationDraft",
    "RadrootsPhase1PublicationMediaReference",
    "RadrootsPhase1PublicationArtifactDigest",
    "RadrootsPhase1PublicationArtifact",
    "RadrootsPhase1PublicationArtifactError",
];

const VALID_VECTOR_CASES: &[(&str, &str)] = &[
    (
        "profile_round_trip",
        "publication_artifact.build_profile.valid",
    ),
    (
        "update_round_trip",
        "publication_artifact.build_update.valid",
    ),
    (
        "photo_update_round_trip",
        "publication_artifact.build_photo_update.valid",
    ),
    (
        "ask_round_trip_with_fallback",
        "publication_artifact.build_ask.valid",
    ),
    (
        "event_date_round_trip",
        "publication_artifact.build_calendar_date_event.valid",
    ),
    (
        "event_time_round_trip",
        "publication_artifact.build_calendar_time_event.valid",
    ),
    (
        "food_availability_round_trip",
        "publication_artifact.build_food_availability.valid",
    ),
    (
        "canonical_json_serialization",
        "publication_artifact.to_canonical_json.valid",
    ),
    (
        "canonical_json_reload",
        "publication_artifact.from_canonical_json.valid",
    ),
];

const INVALID_VECTOR_KIND: &str = "publication_artifact.from_canonical_json.invalid";

const REQUIRED_INVALID_VECTOR_IDS: &[&str] = &[
    "leading_whitespace_is_noncanonical",
    "artifact_exact_byte_limit_reaches_parser",
    "artifact_one_byte_over_limit_is_rejected",
    "unknown_field_is_rejected",
    "unknown_draft_field_is_rejected",
    "nested_expected_event_id_is_rejected",
    "missing_expected_event_id_is_rejected",
    "malformed_expected_event_id_is_rejected",
    "uppercase_expected_event_id_is_rejected",
    "duplicate_expected_event_id_is_rejected",
    "unknown_media_field_is_rejected",
    "json_field_order_is_noncanonical",
    "unknown_version_is_rejected",
    "cross_variant_is_rejected",
    "operation_mismatch_is_rejected",
    "contract_mismatch_is_rejected",
    "kind_mismatch_is_rejected",
    "author_mismatch_is_rejected",
    "created_at_tamper_is_rejected",
    "draft_tags_tamper_is_rejected",
    "draft_content_tamper_is_rejected",
    "noncanonical_nip05_is_rejected_after_id_rebuild",
    "empty_ask_content_is_rejected_after_id_rebuild",
    "expected_event_id_tamper_is_rejected",
    "digest_tamper_is_rejected",
    "media_order_is_rejected",
    "profile_media_commitment_tamper_is_rejected",
    "media_url_tamper_is_rejected",
    "noncanonical_media_url_casing_is_rejected",
    "media_hash_tamper_is_rejected",
    "media_type_tamper_is_rejected",
    "post_media_commitment_must_match_imeta",
];

const RAW_IMMUTABLE_ARTIFACTS: &[ImmutableArtifactSpec] = &[
    ImmutableArtifactSpec::new(
        RAW_MANIFEST_RELATIVE,
        45_449,
        "cde4346fe1f3fce6ec97c7a6c17c4f7e96800456b1a0fdab2d9c86ad87c08b37",
    ),
    ImmutableArtifactSpec::new(
        RAW_MANIFEST_SCHEMA_RELATIVE,
        17_896,
        "f9d210967e54b66f39c8bb965d97b2001a0ebc0927e7c2c14edb8e474bfda695",
    ),
    ImmutableArtifactSpec::new(
        RAW_MANIFEST_SHA256_RELATIVE,
        65,
        "ac399b4cc9ea589d441c310e0edbef6459d7f4b9c5761fa3055df69c676d8fa9",
    ),
    ImmutableArtifactSpec::new(
        RAW_GENERATED_DESCRIPTOR_RELATIVE,
        50_735,
        "b092c04d7892a441a723ed61958d084f67412da763c887531dbfb79b66973f98",
    ),
    ImmutableArtifactSpec::new(
        RAW_VECTOR_RELATIVE,
        26_833,
        "c37a2bf3714f53ab04fae8c5c9dbe2ad4b3f5310efa51f46bd8b116660f1fe15",
    ),
    ImmutableArtifactSpec::new(
        RAW_VECTOR_MIRROR_RELATIVE,
        26_833,
        "c37a2bf3714f53ab04fae8c5c9dbe2ad4b3f5310efa51f46bd8b116660f1fe15",
    ),
    ImmutableArtifactSpec::new(
        RAW_VECTOR_EXECUTOR_RELATIVE,
        25_542,
        "51647259efdd0d99689ef1db0defb139c8d1f60f2ead69b793ddb2733a28e832",
    ),
];

const PUBLICATION_IMMUTABLE_ARTIFACTS: &[ImmutableArtifactSpec] = &[
    ImmutableArtifactSpec::new(
        MANIFEST_RELATIVE,
        89_464,
        "0776e1d84c9366954047e75cdf12d9acc9a7108260157c3534f22067075a385a",
    ),
    ImmutableArtifactSpec::new(
        MANIFEST_SCHEMA_RELATIVE,
        11_972,
        "1d72cee2754e7ac45105d79b1ecf7d44251991be7a18ba106166e962000e8320",
    ),
    ImmutableArtifactSpec::new(
        MANIFEST_SHA256_RELATIVE,
        65,
        "586c0985b502f22241b4d90f2ecb475d43953fc3ddd66d6fbfa3b3eb9cf34444",
    ),
    ImmutableArtifactSpec::new(
        VECTOR_RELATIVE,
        23_113,
        "ec18c687d5b0710a48624ddb620d89157e6b645dbea8bb91c62e3a111d20c622",
    ),
    ImmutableArtifactSpec::new(
        VECTOR_MIRROR_RELATIVE,
        23_113,
        "ec18c687d5b0710a48624ddb620d89157e6b645dbea8bb91c62e3a111d20c622",
    ),
    ImmutableArtifactSpec::new(
        VECTOR_EXECUTOR_RELATIVE,
        34_582,
        "7a31169eac4217a38cb3ef25eb9213f2f89e11fb17e76ceaf7449b34225e98af",
    ),
];

pub(super) const PUBLICATION_SUCCESSOR_SUPERSEDED_PATHS: &[&str] = &[
    CHANGELOG_RELATIVE,
    OPERATIONS_RELATIVE,
    RELEASE_RELATIVE,
    "crates/event_codec/README",
    "crates/event_codec/src/wire/publication.rs",
    "tools/xtask/src/contract.rs",
    "tools/xtask/src/contract/phase1_publication_artifact.rs",
    "tools/xtask/src/main.rs",
];

const GOVERNED_COMPILER_TABLES: &[(&str, &str, &str)] = &[
    (
        "crates/core/Cargo.toml",
        "radroots_core",
        "3eaa58e5c6a6e1ee857da1d519c65480a15b3432376cb14c4fa781870caef359",
    ),
    (
        "crates/event/Cargo.toml",
        "radroots_event",
        "c6908858696c9b9df86eea1983acf251aa68242ccc24fe5c8ad89c10aafd3cb3",
    ),
    (
        "crates/event_codec/Cargo.toml",
        "radroots_event_codec",
        "ee2e9c60570b1a266a6e813b758c1d556559eaf71acb16ddd8bef93591a26d4b",
    ),
    (
        "crates/blossom/Cargo.toml",
        "radroots_blossom",
        "b91985be4f3da164b434a06ab816321c1cd27a44b0ca7f817881a5ff57ff168d",
    ),
    (
        "crates/event_store/Cargo.toml",
        "radroots_event_store",
        "8afca70838e4d83a7b6409c91cef7184d805086d9fba342fd42ff2c4010db450",
    ),
    (
        "crates/transport/Cargo.toml",
        "radroots_transport",
        "a1391f424322e4851baa881c787f6b824cbc5ff69834db44e4b96e76cb776c15",
    ),
];

const CONSTRUCTORS: &[ConstructorSpec] = &[
    ConstructorSpec {
        semantic_variant: "profile",
        serialized_semantic_variant: "profile",
        event_variant: None,
        strict_input: "RadrootsAuthoredProfile",
        constructor: "from_profile",
        publication_operation_id: "publication_artifact.build_profile",
        authored_operation_id: "profile.build_authored_draft",
        event_contract_id: "radroots.profile.metadata.v1",
        kind: 0,
    },
    ConstructorSpec {
        semantic_variant: "update",
        serialized_semantic_variant: "update",
        event_variant: None,
        strict_input: "RadrootsAuthoredUpdate",
        constructor: "from_update",
        publication_operation_id: "publication_artifact.build_update",
        authored_operation_id: "social.update.build_authored_draft",
        event_contract_id: "radroots.social.update.v1",
        kind: 1,
    },
    ConstructorSpec {
        semantic_variant: "photo_update",
        serialized_semantic_variant: "photo_update",
        event_variant: None,
        strict_input: "RadrootsAuthoredPhotoUpdate",
        constructor: "from_photo_update",
        publication_operation_id: "publication_artifact.build_photo_update",
        authored_operation_id: "social.photo_update.build_authored_draft",
        event_contract_id: "radroots.social.photo_update.v1",
        kind: 1,
    },
    ConstructorSpec {
        semantic_variant: "ask",
        serialized_semantic_variant: "ask",
        event_variant: None,
        strict_input: "RadrootsAuthoredAsk",
        constructor: "from_ask",
        publication_operation_id: "publication_artifact.build_ask",
        authored_operation_id: "social.ask.build_authored_draft",
        event_contract_id: "radroots.social.ask.v1",
        kind: 1,
    },
    ConstructorSpec {
        semantic_variant: "event",
        serialized_semantic_variant: "event_date",
        event_variant: Some("date"),
        strict_input: "RadrootsAuthoredCalendarDateEvent",
        constructor: "from_calendar_date_event",
        publication_operation_id: "publication_artifact.build_calendar_date_event",
        authored_operation_id: "social.calendar_date_event.build_authored_draft",
        event_contract_id: "radroots.calendar.date_event.v1",
        kind: 31_922,
    },
    ConstructorSpec {
        semantic_variant: "event",
        serialized_semantic_variant: "event_time",
        event_variant: Some("time"),
        strict_input: "RadrootsAuthoredCalendarTimeEvent",
        constructor: "from_calendar_time_event",
        publication_operation_id: "publication_artifact.build_calendar_time_event",
        authored_operation_id: "social.calendar_time_event.build_authored_draft",
        event_contract_id: "radroots.calendar.time_event.v1",
        kind: 31_923,
    },
    ConstructorSpec {
        semantic_variant: "food_availability",
        serialized_semantic_variant: "food_availability",
        event_variant: None,
        strict_input: "RadrootsFoodAvailabilityDetails",
        constructor: "from_food_availability",
        publication_operation_id: "publication_artifact.build_food_availability",
        authored_operation_id: "food_availability.build_authored_draft",
        event_contract_id: "radroots.food.availability.v1",
        kind: 30_402,
    },
];

#[derive(Clone, Copy)]
struct ImmutableArtifactSpec {
    relative: &'static str,
    byte_length: usize,
    sha256: &'static str,
}

impl ImmutableArtifactSpec {
    const fn new(relative: &'static str, byte_length: usize, sha256: &'static str) -> Self {
        Self {
            relative,
            byte_length,
            sha256,
        }
    }
}

#[derive(Clone, Copy)]
struct ConstructorSpec {
    semantic_variant: &'static str,
    serialized_semantic_variant: &'static str,
    event_variant: Option<&'static str>,
    strict_input: &'static str,
    constructor: &'static str,
    publication_operation_id: &'static str,
    authored_operation_id: &'static str,
    event_contract_id: &'static str,
    kind: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PublicationManifest {
    schema_version: u32,
    contract_id: String,
    authority_id: String,
    manifest_schema: FileDescriptor,
    predecessor: PredecessorDescriptor,
    event_contract_registry: RegistryDescriptor,
    artifact: ArtifactDescriptor,
    operations: Vec<OperationDescriptor>,
    predecessor_source_supersessions: Vec<String>,
    source_files: Vec<SourceFileDescriptor>,
    result_vector: ResultVectorDescriptor,
    release: ReleaseDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FileDescriptor {
    path: String,
    byte_length: u64,
    sha256: String,
    hash_algorithm: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceFileDescriptor {
    role: String,
    path: String,
    byte_length: u64,
    sha256: String,
    hash_algorithm: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PredecessorDescriptor {
    contract_id: String,
    manifest: FileDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RegistryDescriptor {
    version: u32,
    inventory: FileDescriptor,
    sidecar: FileDescriptor,
    evolution: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ArtifactDescriptor {
    schema_version: u32,
    validator: String,
    semantic_roles: Vec<String>,
    serialized_semantic_variants: Vec<String>,
    event_subvariants: Vec<String>,
    canonical_encoding: String,
    artifact_max_bytes: u64,
    signed_event_wire_max_bytes: u64,
    media_reference_max_count: u64,
    envelope_fields: Vec<String>,
    draft_fields: Vec<String>,
    media_reference_fields: Vec<String>,
    media_reference_identity: String,
    primary_media_url_requirement: String,
    post_fallback_url_requirement: String,
    digest_algorithm: String,
    digest_domain: String,
    digest_domain_terminator: String,
    digest_preimage: String,
    constructors: Vec<ConstructorDescriptor>,
    denied_inputs: Vec<String>,
    reload_capability: String,
    threat_boundary: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ConstructorDescriptor {
    semantic_variant: String,
    serialized_semantic_variant: String,
    event_variant: Option<String>,
    strict_input: String,
    constructor: String,
    publication_operation_id: String,
    authored_operation_id: String,
    event_contract_id: String,
    kind: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct OperationDescriptor {
    id: String,
    strict_input: String,
    output: String,
    error_class: String,
    signing: String,
    transport: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ResultVectorDescriptor {
    canonical_path: String,
    mirror_path: String,
    byte_length: u64,
    sha256: String,
    hash_algorithm: String,
    executor_path: String,
    executor_test: String,
    valid_case_ids: Vec<String>,
    invalid_case_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ReleaseDescriptor {
    record_path: String,
    change_id: String,
    changelog_path: String,
    changelog_marker: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorSuite {
    suite: String,
    contract_version: String,
    vectors: Vec<VectorCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorCase {
    id: String,
    kind: String,
    input: Value,
    expected: Value,
}

struct ValidatedResultVector {
    valid_case_ids: Vec<String>,
    invalid_case_ids: Vec<String>,
    bytes: Vec<u8>,
}

pub(crate) fn write_phase1_publication_artifact_manifest(
    workspace_root: &Path,
) -> Result<(), String> {
    validate_immutable_phase1_publication_artifact_predecessor(workspace_root)?;
    Err(
        "Phase 1 publication artifact v1 is an immutable predecessor and cannot be rewritten; write the active Phase 1 publication allowlist successor instead"
            .to_owned(),
    )
}

pub(crate) fn validate_phase1_publication_artifact_manifest(
    workspace_root: &Path,
) -> Result<(), String> {
    validate_immutable_phase1_publication_artifact_predecessor(workspace_root)
}

pub(crate) fn validate_immutable_phase1_publication_artifact_predecessor(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_immutable_phase1_publication_artifact_predecessor_under_lock(workspace_root)
    })
}

pub(crate) fn validate_immutable_raw_source_rebuild_predecessor(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_immutable_raw_predecessor_under_lock(workspace_root)
    })
}

fn validate_manifest_under_lock(workspace_root: &Path) -> Result<(), String> {
    validate_immutable_raw_predecessor_under_lock(workspace_root)?;
    for artifact in expected_artifacts(workspace_root)? {
        let actual = read_regular_file(workspace_root, artifact.relative)?;
        if actual != artifact.contents {
            return Err(format!(
                "generated Phase 1 publication contract {} is stale; run {WRITE_COMMAND}",
                artifact.relative
            ));
        }
    }

    let bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest: PublicationManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_RELATIVE, &bytes, &manifest)?;
    validate_manifest_shape(&manifest)?;

    let schema_bytes = read_regular_file(workspace_root, MANIFEST_SCHEMA_RELATIVE)?;
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("parse {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_SCHEMA_RELATIVE, &schema_bytes, &schema)?;
    let instance = serde_json::to_value(&manifest)
        .map_err(|error| format!("serialize {MANIFEST_RELATIVE}: {error}"))?;
    validate_json_schema(&schema, &instance)?;

    let sidecar = read_regular_file(workspace_root, MANIFEST_SHA256_RELATIVE)?;
    if sidecar != format!("{}\n", sha256_hex(&bytes)).as_bytes() {
        return Err(format!(
            "{MANIFEST_SHA256_RELATIVE} must authenticate the exact manifest bytes"
        ));
    }
    Ok(())
}

pub(super) fn validate_immutable_phase1_publication_artifact_predecessor_under_lock(
    workspace_root: &Path,
) -> Result<(), String> {
    for spec in PUBLICATION_IMMUTABLE_ARTIFACTS {
        let bytes = read_regular_file(workspace_root, spec.relative)?;
        if bytes.len() != spec.byte_length || sha256_hex(&bytes) != spec.sha256 {
            return Err(format!(
                "immutable Phase 1 publication artifact predecessor {} drifted",
                spec.relative
            ));
        }
    }

    let manifest_bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    if manifest.get("schema_version").and_then(Value::as_u64) != Some(1)
        || manifest.get("contract_id").and_then(Value::as_str)
            != Some("radroots_event_codec.phase1_publication_artifact_v1")
        || manifest.get("authority_id").and_then(Value::as_str)
            != Some("phase1_publication_artifact_v1")
    {
        return Err(format!("{MANIFEST_RELATIVE} identity drifted"));
    }
    let sidecar = read_regular_file(workspace_root, MANIFEST_SHA256_RELATIVE)?;
    if sidecar != format!("{}\n", sha256_hex(&manifest_bytes)).as_bytes() {
        return Err(format!(
            "{MANIFEST_SHA256_RELATIVE} does not authenticate its immutable manifest"
        ));
    }

    let superseded = PUBLICATION_SUCCESSOR_SUPERSEDED_PATHS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if superseded.len() != PUBLICATION_SUCCESSOR_SUPERSEDED_PATHS.len() {
        return Err("publication predecessor supersession paths must be unique".to_owned());
    }
    let sources = manifest
        .get("source_files")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{MANIFEST_RELATIVE} has no source_files array"))?;
    let mut seen = BTreeSet::new();
    for source in sources {
        let path = descriptor_path(source, "publication predecessor source")?;
        if !seen.insert(path) {
            return Err(format!("{MANIFEST_RELATIVE} duplicates source {path}"));
        }
        if !superseded.contains(path) {
            validate_value_descriptor(workspace_root, source, "publication predecessor source")?;
        }
    }
    for path in superseded {
        if !seen.contains(path) {
            return Err(format!(
                "publication predecessor supersession path {path} is not predecessor-bound"
            ));
        }
    }

    validate_value_descriptor(
        workspace_root,
        manifest
            .get("manifest_schema")
            .ok_or_else(|| format!("{MANIFEST_RELATIVE} has no manifest_schema"))?,
        "publication predecessor schema",
    )?;
    for pointer in [
        "/event_contract_registry/inventory",
        "/event_contract_registry/sidecar",
    ] {
        validate_value_descriptor(
            workspace_root,
            manifest
                .pointer(pointer)
                .ok_or_else(|| format!("{MANIFEST_RELATIVE} has no {pointer}"))?,
            "publication predecessor registry",
        )?;
    }
    if read_regular_file(workspace_root, VECTOR_RELATIVE)?
        != read_regular_file(workspace_root, VECTOR_MIRROR_RELATIVE)?
    {
        return Err("immutable Phase 1 publication artifact vector mirror drifted".to_owned());
    }
    Ok(())
}

fn expected_artifacts(workspace_root: &Path) -> Result<Vec<GeneratedArtifact>, String> {
    validate_immutable_raw_predecessor_under_lock(workspace_root)?;
    validate_source_contract(workspace_root)?;
    let schema_bytes = canonical_json_bytes(&manifest_schema())?;
    let manifest = describe_manifest(workspace_root, &schema_bytes)?;
    let manifest_bytes = canonical_json_bytes(&manifest)?;
    let vector_bytes = read_regular_file(workspace_root, VECTOR_RELATIVE)?;
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
            contents: vector_bytes,
        },
    ])
}

fn describe_manifest(
    workspace_root: &Path,
    schema_bytes: &[u8],
) -> Result<PublicationManifest, String> {
    let source_files = source_specs(workspace_root)?
        .into_iter()
        .map(|(role, path)| source_descriptor(workspace_root, &role, &path))
        .collect::<Result<Vec<_>, _>>()?;
    let vector = validate_result_vector(workspace_root)?;
    let vector_byte_length = byte_length(VECTOR_RELATIVE, &vector.bytes)?;
    let vector_sha256 = sha256_hex(&vector.bytes);
    Ok(PublicationManifest {
        schema_version: SCHEMA_VERSION,
        contract_id: CONTRACT_ID.to_owned(),
        authority_id: AUTHORITY_ID.to_owned(),
        manifest_schema: descriptor_for_bytes(MANIFEST_SCHEMA_RELATIVE, schema_bytes)?,
        predecessor: PredecessorDescriptor {
            contract_id: "radroots_event_store.raw_source_rebuild_v1".to_owned(),
            manifest: descriptor_for_file(workspace_root, RAW_MANIFEST_RELATIVE)?,
        },
        event_contract_registry: RegistryDescriptor {
            version: 7,
            inventory: descriptor_for_file(workspace_root, REGISTRY_RELATIVE)?,
            sidecar: descriptor_for_file(workspace_root, REGISTRY_SIDECAR_RELATIVE)?,
            evolution: "immutable_registry_v7_plus_additive_publication_authority_v1".to_owned(),
        },
        artifact: expected_artifact_descriptor(),
        operations: expected_operation_descriptors(),
        predecessor_source_supersessions: RAW_PREDECESSOR_SUPERSEDED_PATHS
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        source_files,
        result_vector: ResultVectorDescriptor {
            canonical_path: VECTOR_RELATIVE.to_owned(),
            mirror_path: VECTOR_MIRROR_RELATIVE.to_owned(),
            byte_length: vector_byte_length,
            sha256: vector_sha256,
            hash_algorithm: HASH_ALGORITHM.to_owned(),
            executor_path: VECTOR_EXECUTOR_RELATIVE.to_owned(),
            executor_test: VECTOR_EXECUTOR_TEST.to_owned(),
            valid_case_ids: vector.valid_case_ids,
            invalid_case_ids: vector.invalid_case_ids,
        },
        release: ReleaseDescriptor {
            record_path: RELEASE_RELATIVE.to_owned(),
            change_id: RELEASE_CHANGE_ID.to_owned(),
            changelog_path: CHANGELOG_RELATIVE.to_owned(),
            changelog_marker: CHANGELOG_MARKER.to_owned(),
        },
    })
}

fn expected_artifact_descriptor() -> ArtifactDescriptor {
    ArtifactDescriptor {
        schema_version: RADROOTS_PHASE1_PUBLICATION_ARTIFACT_SCHEMA_VERSION,
        validator: "validate_phase1_publication_artifact".to_owned(),
        semantic_roles: [
            "profile",
            "update",
            "photo_update",
            "ask",
            "event",
            "food_availability",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        serialized_semantic_variants: [
            "profile",
            "update",
            "photo_update",
            "ask",
            "event_date",
            "event_time",
            "food_availability",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        event_subvariants: ["date", "time"].into_iter().map(str::to_owned).collect(),
        canonical_encoding: "serde_json_compact_struct_field_order_v1".to_owned(),
        artifact_max_bytes: RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES as u64,
        signed_event_wire_max_bytes: RADROOTS_PHASE1_PUBLICATION_SIGNED_EVENT_MAX_BYTES as u64,
        media_reference_max_count: RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT as u64,
        envelope_fields: [
            "schema_version",
            "semantic_variant",
            "authored_operation_id",
            "event_contract_id",
            "expected_author",
            "draft",
            "expected_event_id",
            "media_references",
            "artifact_digest",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        draft_fields: ["created_at", "kind", "tags", "content"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        media_reference_fields: ["url", "sha256", "size", "media_type"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        media_reference_identity:
            "exact_case_preserving_approved_url_with_descriptor_commitment_v1".to_owned(),
        primary_media_url_requirement: "blossom_hash_path_extension_required_v1".to_owned(),
        post_fallback_url_requirement: "approved_blossom_url_extension_optional_v1".to_owned(),
        digest_algorithm: "sha256_domain_nul_canonical_json_v1".to_owned(),
        digest_domain: "radroots.phase1.publication-artifact.v1".to_owned(),
        digest_domain_terminator: "0x00".to_owned(),
        digest_preimage: "ascii_domain_then_single_nul_then_canonical_envelope_without_digest_v1"
            .to_owned(),
        constructors: CONSTRUCTORS
            .iter()
            .map(|spec| ConstructorDescriptor {
                semantic_variant: spec.semantic_variant.to_owned(),
                serialized_semantic_variant: spec.serialized_semantic_variant.to_owned(),
                event_variant: spec.event_variant.map(str::to_owned),
                strict_input: spec.strict_input.to_owned(),
                constructor: format!("RadrootsPhase1PublicationArtifact::{}", spec.constructor),
                publication_operation_id: spec.publication_operation_id.to_owned(),
                authored_operation_id: spec.authored_operation_id.to_owned(),
                event_contract_id: spec.event_contract_id.to_owned(),
                kind: spec.kind,
            })
            .collect(),
        denied_inputs: [
            "arbitrary_event_draft",
            "raw_json",
            "numeric_kind",
            "signed_event",
            "private_key",
            "signer",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        reload_capability: "persisted_artifact_only_no_prior_capability_restoration_v1".to_owned(),
        threat_boundary: [
            "detects_accidental_corruption",
            "detects_payload_only_modification",
            "detects_digest_only_modification",
            "does_not_authenticate_actor_rewriting_payload_and_digest",
            "does_not_survive_validator_binary_or_host_compromise",
            "does_not_restore_byte_verification_or_upload_completion",
            "does_not_replace_nip01_id_and_signature_verification",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    }
}

fn expected_operation_descriptors() -> Vec<OperationDescriptor> {
    CONSTRUCTORS
        .iter()
        .map(|spec| OperationDescriptor {
            id: spec.publication_operation_id.to_owned(),
            strict_input: spec.strict_input.to_owned(),
            output: "RadrootsPhase1PublicationArtifact".to_owned(),
            error_class: "validation_error".to_owned(),
            signing: "none".to_owned(),
            transport: "none".to_owned(),
        })
        .chain([
            OperationDescriptor {
                id: "publication_artifact.to_canonical_json".to_owned(),
                strict_input: "RadrootsPhase1PublicationArtifact".to_owned(),
                output: "Bytes".to_owned(),
                error_class: "none".to_owned(),
                signing: "none".to_owned(),
                transport: "none".to_owned(),
            },
            OperationDescriptor {
                id: "publication_artifact.from_canonical_json".to_owned(),
                strict_input: "Bytes".to_owned(),
                output: "RadrootsPhase1PublicationArtifact".to_owned(),
                error_class: "parse_error".to_owned(),
                signing: "none".to_owned(),
                transport: "none".to_owned(),
            },
        ])
        .collect()
}

fn validate_source_contract(workspace_root: &Path) -> Result<(), String> {
    validate_compiler_authority(workspace_root)?;
    validate_operations_authority(workspace_root)?;
    validate_release_authority(workspace_root)?;
    validate_result_vector(workspace_root)?;
    let paths = source_specs(workspace_root)?
        .into_iter()
        .map(|(_, path)| path)
        .collect::<BTreeSet<_>>();
    for required in RAW_PREDECESSOR_SUPERSEDED_PATHS {
        if !paths.contains(*required) {
            return Err(format!(
                "publication successor does not bind superseded predecessor source {required}"
            ));
        }
    }
    for generated in GENERATED_ARTIFACT_PATHS {
        if paths.contains(*generated) {
            return Err(format!(
                "publication source inventory recursively includes generated artifact {generated}"
            ));
        }
    }
    Ok(())
}

fn validate_compiler_authority(workspace_root: &Path) -> Result<(), String> {
    let toolchain = parse_toml(workspace_root, "rust-toolchain.toml")?;
    let expected_toolchain: toml::Value = toml::from_str(
        r#"
[toolchain]
channel = "1.97.0"
components = ["clippy", "rust-analyzer", "rust-src", "rustfmt"]
targets = ["wasm32-unknown-unknown"]
"#,
    )
    .map_err(|error| format!("parse expected toolchain: {error}"))?;
    if toolchain != expected_toolchain {
        return Err("rust-toolchain.toml drifted from Rust 1.97.0 authority".to_owned());
    }
    let cargo_config = parse_toml(workspace_root, ".cargo/config.toml")?;
    let expected_config: toml::Value = toml::from_str("[alias]\nxtask = \"run -q -p xtask --\"\n")
        .map_err(|error| format!("parse expected Cargo config: {error}"))?;
    if cargo_config != expected_config {
        return Err(".cargo/config.toml must remain the xtask alias only".to_owned());
    }
    for forbidden in ["rust-toolchain", ".cargo/config"] {
        match fs::symlink_metadata(workspace_root.join(forbidden)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(format!(
                    "legacy compiler configuration {forbidden} must be absent"
                ));
            }
            Err(error) => return Err(format!("inspect {forbidden}: {error}")),
        }
    }

    for (relative, package_name, expected_hash) in GOVERNED_COMPILER_TABLES {
        let manifest = parse_toml(workspace_root, relative)?;
        validate_package_shape(workspace_root, relative, package_name, &manifest)?;
        let mut tables = BTreeMap::new();
        for key in ["dependencies", "features"] {
            tables.insert(
                key.to_owned(),
                manifest
                    .get(key)
                    .ok_or_else(|| format!("{relative} must declare [{key}]"))?
                    .clone(),
            );
        }
        if *relative == "crates/event_store/Cargo.toml" {
            tables.insert(
                "dev-dependencies".to_owned(),
                manifest
                    .get("dev-dependencies")
                    .ok_or_else(|| format!("{relative} must declare [dev-dependencies]"))?
                    .clone(),
            );
        }
        let actual = sha256_hex(
            &serde_json::to_vec(&tables)
                .map_err(|error| format!("serialize {relative} compiler tables: {error}"))?,
        );
        if actual != *expected_hash {
            return Err(format!(
                "{relative} compiler tables drifted: expected {expected_hash}, found {actual}"
            ));
        }
    }
    Ok(())
}

fn validate_package_shape(
    workspace_root: &Path,
    relative: &str,
    expected_name: &str,
    manifest: &toml::Value,
) -> Result<(), String> {
    let package = manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{relative} must declare [package]"))?;
    if package.get("name").and_then(toml::Value::as_str) != Some(expected_name)
        || package.get("version").and_then(toml::Value::as_str) != Some("1.0.0-alpha.1")
        || package
            .get("edition")
            .and_then(toml::Value::as_table)
            .and_then(|value| value.get("workspace"))
            .and_then(toml::Value::as_bool)
            != Some(true)
        || package
            .get("rust-version")
            .and_then(toml::Value::as_table)
            .and_then(|value| value.get("workspace"))
            .and_then(toml::Value::as_bool)
            != Some(true)
    {
        return Err(format!("{relative} package/compiler identity drifted"));
    }
    if [
        "build",
        "autolib",
        "autobins",
        "autoexamples",
        "autotests",
        "autobenches",
    ]
    .iter()
    .any(|key| package.contains_key(*key))
        || [
            "build-dependencies",
            "target",
            "lib",
            "bin",
            "example",
            "test",
            "bench",
        ]
        .iter()
        .any(|key| manifest.get(*key).is_some())
    {
        return Err(format!(
            "{relative} introduces unapproved build or target authority"
        ));
    }
    let package_root = workspace_root
        .join(relative)
        .parent()
        .ok_or_else(|| format!("{relative} has no package root"))?
        .to_path_buf();
    for path in [
        package_root.join("build.rs"),
        package_root.join("src/main.rs"),
        package_root.join("src/bin"),
    ] {
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(format!(
                    "{relative} has unapproved auto-discovered target {}",
                    path.display()
                ));
            }
            Err(error) => return Err(format!("inspect {}: {error}", path.display())),
        }
    }
    Ok(())
}

fn validate_operations_authority(workspace_root: &Path) -> Result<(), String> {
    let manifest = parse_toml(workspace_root, OPERATIONS_RELATIVE)?;
    let error_classes = toml_string_array(
        OPERATIONS_RELATIVE,
        manifest
            .get("errors")
            .and_then(|value| value.get("classes")),
    )?;
    if error_classes
        .iter()
        .filter(|value| value.as_str() == "none")
        .count()
        != 1
    {
        return Err("error classes must contain none exactly once".to_owned());
    }
    let domains = toml_string_array(
        OPERATIONS_RELATIVE,
        manifest
            .get("public")
            .and_then(|value| value.get("domains")),
    )?;
    if domains
        .iter()
        .filter(|value| value.as_str() == "publication")
        .count()
        != 1
    {
        return Err("public domains must contain publication exactly once".to_owned());
    }
    let types = toml_string_array(
        OPERATIONS_RELATIVE,
        manifest
            .get("shared_types")
            .and_then(|value| value.get("public")),
    )?;
    for required in PUBLIC_TYPES {
        if types
            .iter()
            .filter(|value| value.as_str() == *required)
            .count()
            != 1
        {
            return Err(format!(
                "shared public types must contain {required} exactly once"
            ));
        }
    }
    let operations = manifest
        .get("operations")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "operations.toml must declare [operations]".to_owned())?;
    for forbidden in [
        "phase1_publication_artifact_build",
        "phase1_publication_artifact_reload",
    ] {
        if operations.contains_key(forbidden) {
            return Err(format!(
                "obsolete publication operation {forbidden} is forbidden"
            ));
        }
    }
    for expected in expected_operation_descriptors() {
        let (key, expected_inputs) =
            if let Some(suffix) = expected.id.strip_prefix("publication_artifact.build_") {
                (
                    format!("phase1_publication_artifact_build_{suffix}"),
                    vec![
                        expected.strict_input.clone(),
                        "u64".to_owned(),
                        "String".to_owned(),
                    ],
                )
            } else {
                match expected.id.as_str() {
                    "publication_artifact.to_canonical_json" => (
                        "phase1_publication_artifact_to_canonical_json".to_owned(),
                        vec![expected.strict_input.clone()],
                    ),
                    "publication_artifact.from_canonical_json" => (
                        "phase1_publication_artifact_from_canonical_json".to_owned(),
                        vec![expected.strict_input.clone()],
                    ),
                    other => return Err(format!("unknown publication operation {other}")),
                }
            };
        let operation = operations
            .get(&key)
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("operations.toml is missing {key}"))?;
        require_toml_string(operation, "domain", "publication", &key)?;
        require_toml_string(operation, "id", &expected.id, &key)?;
        require_toml_string(operation, "stability", "beta", &key)?;
        require_toml_string(operation, "error_class", &expected.error_class, &key)?;
        require_toml_string(operation, "signing", "none", &key)?;
        require_toml_string(operation, "transport", "none", &key)?;
        if operation
            .get("deterministic")
            .and_then(toml::Value::as_bool)
            != Some(true)
            || toml_string_array(&key, operation.get("inputs"))? != expected_inputs
            || toml_string_array(&key, operation.get("outputs"))? != [expected.output.clone()]
        {
            return Err(format!("{key} signature or determinism drifted"));
        }
        let implementation = operation
            .get("implementation")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("{key} implementation is missing"))?;
        let modules = toml_string_array(&key, implementation.get("rust_modules"))?;
        if !modules
            .iter()
            .any(|path| path == "crates/event_codec/src/wire/publication.rs")
        {
            return Err(format!(
                "{key} does not route to the sealed publication module"
            ));
        }
        let conformance = operation
            .get("conformance")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("{key} conformance is missing"))?;
        require_toml_string(conformance, "vector", VECTOR_RELATIVE, &key)?;
        let expected_case_kinds = if expected.id == "publication_artifact.from_canonical_json" {
            vec![
                "publication_artifact.from_canonical_json.valid".to_owned(),
                "publication_artifact.from_canonical_json.invalid".to_owned(),
            ]
        } else {
            vec![format!("{}.valid", expected.id)]
        };
        if toml_string_array(&key, conformance.get("case_kinds"))? != expected_case_kinds {
            return Err(format!("{key} conformance case kinds drifted"));
        }
    }
    Ok(())
}

fn validate_release_authority(workspace_root: &Path) -> Result<(), String> {
    let release = parse_toml(workspace_root, RELEASE_RELATIVE)?;
    let changes = release
        .get("changes")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{RELEASE_RELATIVE} must declare changes"))?;
    let matching = changes
        .iter()
        .filter(|change| change.get("id").and_then(toml::Value::as_str) == Some(RELEASE_CHANGE_ID))
        .collect::<Vec<_>>();
    let [change] = matching.as_slice() else {
        return Err(format!(
            "{RELEASE_RELATIVE} must contain {RELEASE_CHANGE_ID} exactly once"
        ));
    };
    if change.get("classification").and_then(toml::Value::as_str) != Some("feature")
        || toml_string_array(RELEASE_CHANGE_ID, change.get("semver_impacts"))?
            != [
                "add_exported_type",
                "add_exported_function",
                "add_exported_constant",
                "add_conformance_vector",
            ]
        || change
            .get("summary")
            .and_then(toml::Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err(format!("{RELEASE_CHANGE_ID} release authority drifted"));
    }
    let changelog = String::from_utf8(read_regular_file(workspace_root, CHANGELOG_RELATIVE)?)
        .map_err(|error| format!("{CHANGELOG_RELATIVE} must be UTF-8: {error}"))?;
    if changelog.matches(CHANGELOG_MARKER).count() != 1 {
        return Err(format!(
            "{CHANGELOG_RELATIVE} must contain {CHANGELOG_MARKER} exactly once"
        ));
    }
    Ok(())
}

fn validate_result_vector(workspace_root: &Path) -> Result<ValidatedResultVector, String> {
    let bytes = read_regular_file(workspace_root, VECTOR_RELATIVE)?;
    let suite: VectorSuite = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {VECTOR_RELATIVE}: {error}"))?;
    if suite.suite != "phase1_publication_artifact" || suite.contract_version != "1.0.0" {
        return Err(format!("{VECTOR_RELATIVE} identity drifted"));
    }
    let mut ids = BTreeSet::new();
    let mut valid = Vec::new();
    let mut invalid = Vec::new();
    for case in suite.vectors {
        if !ids.insert(case.id.clone()) {
            return Err(format!("{VECTOR_RELATIVE} duplicates case {}", case.id));
        }
        if !case.input.is_object() || !case.expected.is_object() {
            return Err(format!("vector case {} must use object data", case.id));
        }
        if case.kind == INVALID_VECTOR_KIND {
            invalid.push(case.id);
        } else if VALID_VECTOR_CASES
            .iter()
            .any(|(id, kind)| *id == case.id && *kind == case.kind)
        {
            valid.push((case.id, case.kind));
        } else {
            return Err(format!(
                "vector case {} uses unknown or mismatched kind {}",
                case.id, case.kind
            ));
        }
    }
    let expected_valid = VALID_VECTOR_CASES
        .iter()
        .map(|(id, kind)| ((*id).to_owned(), (*kind).to_owned()))
        .collect::<Vec<_>>();
    if valid != expected_valid {
        return Err(format!(
            "{VECTOR_RELATIVE} valid inventory drifted: expected {expected_valid:?}, found {valid:?}"
        ));
    }
    for required in REQUIRED_INVALID_VECTOR_IDS {
        if invalid
            .iter()
            .filter(|value| value.as_str() == *required)
            .count()
            != 1
        {
            return Err(format!(
                "{VECTOR_RELATIVE} must contain invalid case {required} exactly once"
            ));
        }
    }
    Ok(ValidatedResultVector {
        valid_case_ids: valid.into_iter().map(|(id, _)| id).collect(),
        invalid_case_ids: invalid,
        bytes,
    })
}

fn validate_immutable_raw_predecessor_under_lock(workspace_root: &Path) -> Result<(), String> {
    for spec in RAW_IMMUTABLE_ARTIFACTS {
        let bytes = read_regular_file(workspace_root, spec.relative)?;
        if bytes.len() != spec.byte_length || sha256_hex(&bytes) != spec.sha256 {
            return Err(format!(
                "immutable raw-source rebuild predecessor artifact {} drifted",
                spec.relative
            ));
        }
    }
    let manifest_bytes = read_regular_file(workspace_root, RAW_MANIFEST_RELATIVE)?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse {RAW_MANIFEST_RELATIVE}: {error}"))?;
    if manifest.get("schema_version").and_then(Value::as_u64) != Some(1)
        || manifest.get("contract_id").and_then(Value::as_str)
            != Some("radroots_event_store.raw_source_rebuild_v1")
    {
        return Err(format!("{RAW_MANIFEST_RELATIVE} identity drifted"));
    }
    let sidecar = read_regular_file(workspace_root, RAW_MANIFEST_SHA256_RELATIVE)?;
    if sidecar != format!("{}\n", sha256_hex(&manifest_bytes)).as_bytes() {
        return Err(format!(
            "{RAW_MANIFEST_SHA256_RELATIVE} does not authenticate its manifest"
        ));
    }

    let superseded = RAW_PREDECESSOR_SUPERSEDED_PATHS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if superseded.len() != RAW_PREDECESSOR_SUPERSEDED_PATHS.len() {
        return Err("raw predecessor supersession paths must be unique".to_owned());
    }
    let sources = manifest
        .get("source_files")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{RAW_MANIFEST_RELATIVE} has no source_files array"))?;
    let mut seen = BTreeSet::new();
    for source in sources {
        let path = descriptor_path(source, "raw predecessor source")?;
        if !seen.insert(path) {
            return Err(format!("{RAW_MANIFEST_RELATIVE} duplicates source {path}"));
        }
        if !superseded.contains(path) {
            validate_value_descriptor(workspace_root, source, "raw predecessor source")?;
        }
    }
    for path in superseded {
        if !seen.contains(path) {
            return Err(format!(
                "raw predecessor supersession path {path} is not predecessor-bound"
            ));
        }
    }
    for descriptor in manifest
        .get("migration_inventory")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{RAW_MANIFEST_RELATIVE} has no migration inventory"))?
    {
        validate_value_descriptor(workspace_root, descriptor, "raw migration")?;
    }
    validate_value_descriptor(
        workspace_root,
        manifest
            .get("manifest_schema")
            .ok_or_else(|| format!("{RAW_MANIFEST_RELATIVE} has no manifest_schema"))?,
        "raw manifest schema",
    )?;
    validate_value_descriptor(
        workspace_root,
        manifest
            .pointer("/predecessor/manifest")
            .ok_or_else(|| format!("{RAW_MANIFEST_RELATIVE} has no predecessor manifest"))?,
        "raw predecessor manifest",
    )?;
    let executor = json!({
        "path": RAW_VECTOR_EXECUTOR_RELATIVE,
        "byte_length": manifest.pointer("/result_vector/executor_byte_length"),
        "sha256": manifest.pointer("/result_vector/executor_sha256"),
        "hash_algorithm": manifest.pointer("/result_vector/executor_hash_algorithm"),
    });
    validate_value_descriptor(workspace_root, &executor, "raw vector executor")?;
    if read_regular_file(workspace_root, RAW_VECTOR_RELATIVE)?
        != read_regular_file(workspace_root, RAW_VECTOR_MIRROR_RELATIVE)?
    {
        return Err("immutable raw-source rebuild vector mirror drifted".to_owned());
    }
    Ok(())
}

fn validate_value_descriptor(
    workspace_root: &Path,
    descriptor: &Value,
    label: &str,
) -> Result<(), String> {
    let path = descriptor_path(descriptor, label)?;
    let expected_len = descriptor
        .get("byte_length")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{label} {path} has no byte_length"))?;
    let expected_sha = descriptor
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label} {path} has no sha256"))?;
    if descriptor.get("hash_algorithm").and_then(Value::as_str) != Some(HASH_ALGORITHM) {
        return Err(format!("{label} {path} has an unsupported hash algorithm"));
    }
    let bytes = read_regular_file(workspace_root, path)?;
    if bytes.len() as u64 != expected_len || sha256_hex(&bytes) != expected_sha {
        return Err(format!(
            "{label} {path} drifted from its predecessor descriptor"
        ));
    }
    Ok(())
}

fn descriptor_path<'a>(descriptor: &'a Value, label: &str) -> Result<&'a str, String> {
    descriptor
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label} has no path"))
}

pub(super) fn source_specs(workspace_root: &Path) -> Result<Vec<(String, String)>, String> {
    let mut specs = Vec::new();
    for root in PUBLIC_SOURCE_ROOTS {
        for path in regular_file_inventory(workspace_root, root)? {
            if !path.ends_with(".rs") {
                return Err(format!("{root} contains non-Rust production source {path}"));
            }
            specs.push(("public_production_source".to_owned(), path));
        }
    }
    specs.extend(
        EXPLICIT_SOURCE_SPECS
            .iter()
            .map(|(role, path)| ((*role).to_owned(), (*path).to_owned())),
    );
    specs.sort_by(|left, right| left.1.cmp(&right.1));
    let mut seen = BTreeSet::new();
    for (_, path) in &specs {
        if !seen.insert(path.as_str()) {
            return Err(format!("publication source inventory duplicates {path}"));
        }
    }
    Ok(specs)
}

fn regular_file_inventory(workspace_root: &Path, relative: &str) -> Result<Vec<String>, String> {
    let mut pending = vec![workspace_root.join(relative)];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| format!("read {}: {error}", directory.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("read {} entry: {error}", directory.display()))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let file_type = entry
                .file_type()
                .map_err(|error| format!("inspect {}: {error}", entry.path().display()))?;
            if file_type.is_symlink() {
                return Err(format!(
                    "governed source inventory forbids symlink {}",
                    entry.path().display()
                ));
            }
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                files.push(
                    entry
                        .path()
                        .strip_prefix(workspace_root)
                        .map_err(|error| format!("relativize {}: {error}", entry.path().display()))?
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            } else {
                return Err(format!(
                    "governed inventory requires regular files: {}",
                    entry.path().display()
                ));
            }
        }
    }
    files.sort();
    Ok(files)
}

fn source_descriptor(
    workspace_root: &Path,
    role: &str,
    path: &str,
) -> Result<SourceFileDescriptor, String> {
    let bytes = read_regular_file(workspace_root, path)?;
    Ok(SourceFileDescriptor {
        role: role.to_owned(),
        path: path.to_owned(),
        byte_length: byte_length(path, &bytes)?,
        sha256: sha256_hex(&bytes),
        hash_algorithm: HASH_ALGORITHM.to_owned(),
    })
}

fn descriptor_for_file(workspace_root: &Path, path: &str) -> Result<FileDescriptor, String> {
    descriptor_for_bytes(path, &read_regular_file(workspace_root, path)?)
}

fn descriptor_for_bytes(path: &str, bytes: &[u8]) -> Result<FileDescriptor, String> {
    Ok(FileDescriptor {
        path: path.to_owned(),
        byte_length: byte_length(path, bytes)?,
        sha256: sha256_hex(bytes),
        hash_algorithm: HASH_ALGORITHM.to_owned(),
    })
}

fn byte_length(path: &str, bytes: &[u8]) -> Result<u64, String> {
    u64::try_from(bytes.len()).map_err(|_| format!("{path} byte length exceeds u64"))
}

fn parse_toml(workspace_root: &Path, relative: &str) -> Result<toml::Value, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{relative} must be UTF-8: {error}"))?;
    toml::from_str(source).map_err(|error| format!("parse {relative}: {error}"))
}

fn toml_string_array(label: &str, value: Option<&toml::Value>) -> Result<Vec<String>, String> {
    value
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{label} must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{label} values must be strings"))
        })
        .collect()
}

fn require_toml_string(
    table: &toml::map::Map<String, toml::Value>,
    field: &str,
    expected: &str,
    label: &str,
) -> Result<(), String> {
    if table.get(field).and_then(toml::Value::as_str) != Some(expected) {
        return Err(format!("{label}.{field} must be {expected}"));
    }
    Ok(())
}

fn validate_manifest_shape(manifest: &PublicationManifest) -> Result<(), String> {
    if manifest.schema_version != SCHEMA_VERSION
        || manifest.contract_id != CONTRACT_ID
        || manifest.authority_id != AUTHORITY_ID
        || manifest.artifact != expected_artifact_descriptor()
        || manifest.operations != expected_operation_descriptors()
        || manifest.predecessor_source_supersessions
            != RAW_PREDECESSOR_SUPERSEDED_PATHS
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        || manifest.release.change_id != RELEASE_CHANGE_ID
        || manifest.result_vector.canonical_path != VECTOR_RELATIVE
        || manifest.result_vector.mirror_path != VECTOR_MIRROR_RELATIVE
    {
        return Err(format!("{MANIFEST_RELATIVE} shape drifted"));
    }
    let mut paths = BTreeSet::new();
    for source in &manifest.source_files {
        if source.hash_algorithm != HASH_ALGORITHM || !paths.insert(source.path.as_str()) {
            return Err(format!("{MANIFEST_RELATIVE} source inventory is invalid"));
        }
    }
    Ok(())
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|error| format!("serialize JSON: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_canonical_json<T: Serialize>(
    relative: &str,
    bytes: &[u8],
    value: &T,
) -> Result<(), String> {
    if canonical_json_bytes(value)? != bytes {
        return Err(format!("{relative} is not canonical pretty JSON"));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn validate_json_schema(schema: &Value, instance: &Value) -> Result<(), String> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|error| format!("compile {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    let errors = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{MANIFEST_RELATIVE} violates its schema: {}",
            errors.join("; ")
        ))
    }
}

fn manifest_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/contracts/event-codec/phase1-publication-artifact-v1.schema.json",
        "title": "Radroots Phase 1 Publication Artifact Contract",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schema_version",
            "contract_id",
            "authority_id",
            "manifest_schema",
            "predecessor",
            "event_contract_registry",
            "artifact",
            "operations",
            "predecessor_source_supersessions",
            "source_files",
            "result_vector",
            "release"
        ],
        "properties": {
            "schema_version": {"const": SCHEMA_VERSION},
            "contract_id": {"const": CONTRACT_ID},
            "authority_id": {"const": AUTHORITY_ID},
            "manifest_schema": {"$ref": "#/$defs/file"},
            "predecessor": {
                "type": "object",
                "additionalProperties": false,
                "required": ["contract_id", "manifest"],
                "properties": {
                    "contract_id": {"const": "radroots_event_store.raw_source_rebuild_v1"},
                    "manifest": {"$ref": "#/$defs/file"}
                }
            },
            "event_contract_registry": {
                "type": "object",
                "additionalProperties": false,
                "required": ["version", "inventory", "sidecar", "evolution"],
                "properties": {
                    "version": {"const": 7},
                    "inventory": {"$ref": "#/$defs/file"},
                    "sidecar": {"$ref": "#/$defs/file"},
                    "evolution": {"type": "string", "minLength": 1}
                }
            },
            "artifact": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "schema_version",
                    "validator",
                    "semantic_roles",
                    "serialized_semantic_variants",
                    "event_subvariants",
                    "canonical_encoding",
                    "artifact_max_bytes",
                    "signed_event_wire_max_bytes",
                    "media_reference_max_count",
                    "envelope_fields",
                    "draft_fields",
                    "media_reference_fields",
                    "media_reference_identity",
                    "primary_media_url_requirement",
                    "post_fallback_url_requirement",
                    "digest_algorithm",
                    "digest_domain",
                    "digest_domain_terminator",
                    "digest_preimage",
                    "constructors",
                    "denied_inputs",
                    "reload_capability",
                    "threat_boundary"
                ],
                "properties": {
                    "schema_version": {"const": 1},
                    "validator": {"const": "validate_phase1_publication_artifact"},
                    "semantic_roles": {"const": ["profile", "update", "photo_update", "ask", "event", "food_availability"]},
                    "serialized_semantic_variants": {"const": ["profile", "update", "photo_update", "ask", "event_date", "event_time", "food_availability"]},
                    "event_subvariants": {"const": ["date", "time"]},
                    "canonical_encoding": {"const": "serde_json_compact_struct_field_order_v1"},
                    "artifact_max_bytes": {"const": 2097152},
                    "signed_event_wire_max_bytes": {"const": 262144},
                    "media_reference_max_count": {"const": 4096},
                    "envelope_fields": {"const": ["schema_version", "semantic_variant", "authored_operation_id", "event_contract_id", "expected_author", "draft", "expected_event_id", "media_references", "artifact_digest"]},
                    "draft_fields": {"const": ["created_at", "kind", "tags", "content"]},
                    "media_reference_fields": {"const": ["url", "sha256", "size", "media_type"]},
                    "media_reference_identity": {"const": "exact_case_preserving_approved_url_with_descriptor_commitment_v1"},
                    "primary_media_url_requirement": {"const": "blossom_hash_path_extension_required_v1"},
                    "post_fallback_url_requirement": {"const": "approved_blossom_url_extension_optional_v1"},
                    "digest_algorithm": {"const": "sha256_domain_nul_canonical_json_v1"},
                    "digest_domain": {"const": "radroots.phase1.publication-artifact.v1"},
                    "digest_domain_terminator": {"const": "0x00"},
                    "digest_preimage": {"const": "ascii_domain_then_single_nul_then_canonical_envelope_without_digest_v1"},
                    "constructors": {
                        "type": "array",
                        "minItems": 7,
                        "maxItems": 7,
                        "items": {"$ref": "#/$defs/constructor"}
                    },
                    "denied_inputs": {"const": ["arbitrary_event_draft", "raw_json", "numeric_kind", "signed_event", "private_key", "signer"]},
                    "reload_capability": {"const": "persisted_artifact_only_no_prior_capability_restoration_v1"},
                    "threat_boundary": {"const": ["detects_accidental_corruption", "detects_payload_only_modification", "detects_digest_only_modification", "does_not_authenticate_actor_rewriting_payload_and_digest", "does_not_survive_validator_binary_or_host_compromise", "does_not_restore_byte_verification_or_upload_completion", "does_not_replace_nip01_id_and_signature_verification"]}
                }
            },
            "operations": {
                "type": "array",
                "minItems": 9,
                "maxItems": 9,
                "items": {"$ref": "#/$defs/operation"}
            },
            "predecessor_source_supersessions": {
                "type": "array",
                "minItems": 6,
                "maxItems": 6,
                "items": {"type": "string", "minLength": 1}
            },
            "source_files": {
                "type": "array",
                "minItems": 1,
                "items": {"$ref": "#/$defs/source"}
            },
            "result_vector": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "canonical_path",
                    "mirror_path",
                    "byte_length",
                    "sha256",
                    "hash_algorithm",
                    "executor_path",
                    "executor_test",
                    "valid_case_ids",
                    "invalid_case_ids"
                ],
                "properties": {
                    "canonical_path": {"const": VECTOR_RELATIVE},
                    "mirror_path": {"const": VECTOR_MIRROR_RELATIVE},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "hash_algorithm": {"const": HASH_ALGORITHM},
                    "executor_path": {"const": VECTOR_EXECUTOR_RELATIVE},
                    "executor_test": {"const": VECTOR_EXECUTOR_TEST},
                    "valid_case_ids": {"type": "array", "minItems": 7, "items": {"type": "string"}},
                    "invalid_case_ids": {"type": "array", "minItems": 24, "items": {"type": "string"}}
                }
            },
            "release": {
                "type": "object",
                "additionalProperties": false,
                "required": ["record_path", "change_id", "changelog_path", "changelog_marker"],
                "properties": {
                    "record_path": {"const": RELEASE_RELATIVE},
                    "change_id": {"const": RELEASE_CHANGE_ID},
                    "changelog_path": {"const": CHANGELOG_RELATIVE},
                    "changelog_marker": {"const": CHANGELOG_MARKER}
                }
            }
        },
        "$defs": {
            "file": {
                "type": "object",
                "additionalProperties": false,
                "required": ["path", "byte_length", "sha256", "hash_algorithm"],
                "properties": {
                    "path": {"type": "string", "minLength": 1},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "hash_algorithm": {"const": HASH_ALGORITHM}
                }
            },
            "source": {
                "type": "object",
                "additionalProperties": false,
                "required": ["role", "path", "byte_length", "sha256", "hash_algorithm"],
                "properties": {
                    "role": {"type": "string", "minLength": 1},
                    "path": {"type": "string", "minLength": 1},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "hash_algorithm": {"const": HASH_ALGORITHM}
                }
            },
            "constructor": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "semantic_variant",
                    "serialized_semantic_variant",
                    "event_variant",
                    "strict_input",
                    "constructor",
                    "publication_operation_id",
                    "authored_operation_id",
                    "event_contract_id",
                    "kind"
                ],
                "properties": {
                    "semantic_variant": {"type": "string", "minLength": 1},
                    "serialized_semantic_variant": {"type": "string", "minLength": 1},
                    "event_variant": {"type": ["string", "null"]},
                    "strict_input": {"type": "string", "minLength": 1},
                    "constructor": {"type": "string", "minLength": 1},
                    "publication_operation_id": {"type": "string", "minLength": 1},
                    "authored_operation_id": {"type": "string", "minLength": 1},
                    "event_contract_id": {"type": "string", "minLength": 1},
                    "kind": {"type": "integer", "minimum": 0}
                }
            },
            "operation": {
                "type": "object",
                "additionalProperties": false,
                "required": ["id", "strict_input", "output", "error_class", "signing", "transport"],
                "properties": {
                    "id": {"type": "string", "minLength": 1},
                    "strict_input": {"type": "string", "minLength": 1},
                    "output": {"enum": ["RadrootsPhase1PublicationArtifact", "Bytes"]},
                    "error_class": {"enum": ["none", "validation_error", "parse_error"]},
                    "signing": {"const": "none"},
                    "transport": {"const": "none"}
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("xtask workspace root")
            .to_path_buf()
    }

    #[test]
    fn publication_source_inventory_is_closed_and_unique() {
        let root = workspace_root();
        let specs = source_specs(&root).expect("publication source inventory");
        let paths = specs
            .iter()
            .map(|(_, path)| path.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(paths.len(), specs.len());
        assert!(paths.contains("crates/event_codec/src/wire/publication.rs"));
        assert!(paths.contains("tools/xtask/src/contract/phase1_publication_artifact.rs"));
        for generated in GENERATED_ARTIFACT_PATHS {
            assert!(!paths.contains(generated));
        }
    }

    #[test]
    fn publication_compiler_operations_and_vector_authority_are_current() {
        let root = workspace_root();
        validate_compiler_authority(&root).expect("compiler authority");
        validate_operations_authority(&root).expect("operations authority");
        validate_result_vector(&root).expect("result vector");
    }

    #[test]
    fn publication_manifest_schema_rejects_unknown_top_level_fields() {
        let root = workspace_root();
        let schema_bytes = canonical_json_bytes(&manifest_schema()).expect("schema");
        let manifest = describe_manifest(&root, &schema_bytes).expect("manifest");
        let mut value = serde_json::to_value(manifest).expect("manifest value");
        validate_json_schema(&manifest_schema(), &value).expect("valid manifest");
        value["unexpected"] = Value::Bool(true);
        validate_json_schema(&manifest_schema(), &value).expect_err("unknown field must fail");
    }
}
