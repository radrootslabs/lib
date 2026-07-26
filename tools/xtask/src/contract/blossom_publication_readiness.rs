use super::{
    artifact_bundle::{GeneratedArtifact, read_regular_file, with_artifact_bundle_transaction},
    raw_source_rebuild::validate_raw_source_rebuild_predecessor_production_sources_under_lock,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};
use syn::{ImplItem, Item, Visibility};

const SCHEMA_VERSION: u32 = 1;
const CONTRACT_ID: &str = "radroots_blossom.publication_readiness_v1";
const AUTHORITY_ID: &str = "blossom_publication_readiness_v1";
const HASH_ALGORITHM: &str = "sha256_bytes_v1";
const WRITE_COMMAND: &str = "cargo xtask contract blossom-publication-readiness-manifest --write";

const MANIFEST_RELATIVE: &str = "crates/blossom/contracts/publication_readiness_v1.manifest.json";
const MANIFEST_SCHEMA_RELATIVE: &str =
    "crates/blossom/contracts/publication_readiness_v1.manifest.schema.json";
const MANIFEST_SHA256_RELATIVE: &str =
    "crates/blossom/contracts/publication_readiness_v1.manifest.sha256";
const GENERATED_DESCRIPTOR_RELATIVE: &str =
    "crates/blossom/contracts/publication_readiness_v1.descriptor.json";
const EVIDENCE_SCHEMA_RELATIVE: &str =
    "crates/blossom/contracts/publication_readiness_evidence_v1.schema.json";
const BEHAVIOR_VECTOR_RELATIVE: &str =
    "contracts/conformance/vectors/blossom/publication_readiness.v1.json";
const BEHAVIOR_VECTOR_MIRROR_RELATIVE: &str =
    "crates/blossom/tests/fixtures/publication_readiness.v1.json";
const BEHAVIOR_VECTOR_EXECUTOR_RELATIVE: &str = "crates/blossom/tests/publication_readiness.rs";
const BEHAVIOR_VECTOR_EXECUTOR_TEST: &str =
    "publication_readiness_conformance_vector_executes_every_case";
const PERSISTENCE_VECTOR_RELATIVE: &str =
    "contracts/conformance/vectors/blossom/publication_readiness_persistence.v1.json";
const PERSISTENCE_VECTOR_MIRROR_RELATIVE: &str =
    "crates/blossom/tests/fixtures/publication_readiness_persistence.v1.json";
const PERSISTENCE_VECTOR_EXECUTOR_RELATIVE: &str =
    "crates/blossom/tests/publication_readiness_persistence.rs";
const PERSISTENCE_VECTOR_EXECUTOR_TEST: &str =
    "publication_readiness_persistence_vector_executes_every_case";
const READINESS_SOURCE_RELATIVE: &str = "crates/blossom/src/publication_readiness.rs";
const BLOSSOM_LIB_RELATIVE: &str = "crates/blossom/src/lib.rs";
const BLOSSOM_MANIFEST_RELATIVE: &str = "crates/blossom/Cargo.toml";
const OPERATIONS_RELATIVE: &str = "contracts/operations.toml";
const RELEASE_RELATIVE: &str = "contracts/releases/1.0.0-alpha.1.toml";
const CHANGELOG_RELATIVE: &str = "CHANGELOG.md";
const RELEASE_CHANGE_ID: &str = "blossom-publication-readiness-evidence";
const CHANGELOG_MARKER: &str = "<!-- release-change: blossom-publication-readiness-evidence -->";

const VERIFY_OPERATION_ID: &str = "blossom.verify_publication_readiness";
const SERIALIZE_OPERATION_ID: &str = "blossom.publication_readiness_evidence.to_canonical_json";
const RELOAD_OPERATION_ID: &str = "blossom.publication_readiness_evidence.from_canonical_json";
const EVIDENCE_DIGEST_DOMAIN: &str = "radroots.blossom.publication-readiness-evidence.v1\0";

const EVIDENCE_SCHEMA_VERSION: u32 = 1;
const READINESS_POLICY_VERSION: u16 = 1;
const EVIDENCE_MAX_BYTES: usize = 8 * 1024;
const URL_MAX_BYTES: usize = 4 * 1024;
const RASTER_MAX_BYTES: u64 = 10_485_760;
const RASTER_MAX_DECODED_BYTES: u64 = 160_000_000;
const RASTER_MAX_DIMENSION: u32 = 16_384;
const RASTER_MAX_PIXELS: u64 = 20_000_000;

const PUBLIC_CONSTANTS: &[&str] = &[
    "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES",
    "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES",
    "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION",
    "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_PIXELS",
    "RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES",
    "RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_SCHEMA_VERSION",
    "RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION",
    "RADROOTS_BLOSSOM_PUBLICATION_READINESS_URL_MAX_BYTES",
];
const PUBLIC_TYPES: &[&str] = &[
    "RadrootsBlossomAuthoredRasterDimensions",
    "RadrootsBlossomBud01GetCollector",
    "RadrootsBlossomBud01GetObservation",
    "RadrootsBlossomBud01HeadObservation",
    "RadrootsBlossomBud02UploadObservation",
    "RadrootsBlossomBud02UploadStatus",
    "RadrootsBlossomPublicationReadinessEvidence",
    "RadrootsBlossomPublicationReadinessEvidenceDigest",
    "RadrootsBlossomRasterDimensions",
    "RadrootsBlossomRasterFormat",
];
const PUBLIC_FUNCTIONS: &[&str] = &["verify_publication_readiness"];
const PUBLIC_METHODS: &[&str] = &[
    "RadrootsBlossomBud01GetCollector::finish",
    "RadrootsBlossomBud01GetCollector::new",
    "RadrootsBlossomBud01GetCollector::push_chunk",
    "RadrootsBlossomBud01GetObservation::bytes",
    "RadrootsBlossomBud01GetObservation::declared_size",
    "RadrootsBlossomBud01GetObservation::from_complete_body",
    "RadrootsBlossomBud01GetObservation::url",
    "RadrootsBlossomBud01HeadObservation::content_length",
    "RadrootsBlossomBud01HeadObservation::media_type",
    "RadrootsBlossomBud01HeadObservation::new",
    "RadrootsBlossomBud01HeadObservation::url",
    "RadrootsBlossomBud02UploadObservation::descriptor",
    "RadrootsBlossomBud02UploadObservation::new",
    "RadrootsBlossomBud02UploadObservation::status",
    "RadrootsBlossomBud02UploadStatus::as_u16",
    "RadrootsBlossomPublicationReadinessEvidence::bud02_status",
    "RadrootsBlossomPublicationReadinessEvidence::dimensions",
    "RadrootsBlossomPublicationReadinessEvidence::evidence_digest",
    "RadrootsBlossomPublicationReadinessEvidence::from_canonical_json",
    "RadrootsBlossomPublicationReadinessEvidence::media_type",
    "RadrootsBlossomPublicationReadinessEvidence::policy_version",
    "RadrootsBlossomPublicationReadinessEvidence::raster_format",
    "RadrootsBlossomPublicationReadinessEvidence::schema_version",
    "RadrootsBlossomPublicationReadinessEvidence::sha256",
    "RadrootsBlossomPublicationReadinessEvidence::size",
    "RadrootsBlossomPublicationReadinessEvidence::to_canonical_json",
    "RadrootsBlossomPublicationReadinessEvidence::uploaded",
    "RadrootsBlossomPublicationReadinessEvidence::url",
    "RadrootsBlossomPublicationReadinessEvidenceDigest::as_sha256",
    "RadrootsBlossomRasterDimensions::height",
    "RadrootsBlossomRasterDimensions::new",
    "RadrootsBlossomRasterDimensions::pixels",
    "RadrootsBlossomRasterDimensions::width",
    "RadrootsBlossomRasterFormat::as_str",
    "RadrootsBlossomRasterFormat::from_media_type",
];
const SEALED_TYPES: &[&str] = &[
    "RadrootsBlossomPublicationReadinessEvidence",
    "RadrootsBlossomPublicationReadinessEvidenceDigest",
];
const PRIVATE_WIRE_TYPES: &[&str] = &[
    "PublicationReadinessDimensionsWire",
    "PublicationReadinessEvidenceWire",
];
const WIRE_FIELD_ORDER: &[&str] = &[
    "schema_version",
    "policy_version",
    "url",
    "sha256",
    "size",
    "media_type",
    "raster_format",
    "dimensions",
    "bud02_status",
    "bud01_head_status",
    "bud01_get_status",
    "uploaded",
    "evidence_digest",
];
const SEMANTIC_INVARIANTS: &[&str] = &[
    "bounded_input_before_json_parse_v1",
    "fixed_compact_json_field_order_v1",
    "canonical_json_round_trip_required_v1",
    "sealed_evidence_without_deserialize_v1",
    "private_deny_unknown_fields_wire_models_v1",
    "canonical_hash_path_url_max_4096_utf8_bytes_v1",
    "closed_jpeg_png_still_webp_mime_and_format_v1",
    "nonzero_size_max_10485760_bytes_v1",
    "nonzero_dimensions_axis_max_16384_pixels_max_20000000_v1",
    "bud02_status_200_or_201_and_bud01_status_200_v1",
    "descriptor_head_get_complete_byte_agreement_v1",
    "private_domain_separated_digest_derivation_v1",
    "no_bud11_credentials_entitlement_or_topology_persistence_v1",
];
const DECODE_ERROR_CODES: &[&str] = &[
    "publication_readiness_evidence_too_large",
    "publication_readiness_evidence_invalid_json",
    "publication_readiness_evidence_schema_version_unsupported",
    "publication_readiness_evidence_policy_version_unsupported",
    "publication_readiness_evidence_field_invalid",
    "publication_readiness_evidence_json_non_canonical",
    "publication_readiness_evidence_digest_mismatch",
];
const PROTOCOL_SOURCE_PINS: &[(&str, &str, &str)] = &[(
    "blossom",
    "https://github.com/hzrd149/blossom",
    "b5bd2801d1763aa635fc8fea7a76597e0eb18990",
)];

const RAW_PREDECESSOR_SUPERSEDED_PATHS: &[&str] = &[
    "Cargo.lock",
    "Cargo.toml",
    "build/nix/common.nix",
    CHANGELOG_RELATIVE,
    RELEASE_RELATIVE,
    "tools/xtask/src/contract.rs",
    "tools/xtask/src/contract/food_availability_projection.rs",
    "tools/xtask/src/contract/nip09_reconciliation.rs",
    "tools/xtask/src/contract/raw_source_rebuild.rs",
    "tools/xtask/src/contract/source_maintenance.rs",
    "tools/xtask/src/main.rs",
];
const TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS: &[&str] = &[
    "Cargo.toml",
    BLOSSOM_MANIFEST_RELATIVE,
    "crates/blossom/src/error.rs",
    BLOSSOM_LIB_RELATIVE,
    "crates/blossom/src/url.rs",
];

const IMMUTABLE_RAW_PREDECESSOR_ARTIFACTS: &[ImmutableArtifactSpec] = &[
    ImmutableArtifactSpec::new(
        "crates/event_store/contracts/raw_source_rebuild_v1.manifest.json",
        45_449,
        "03253ce31dc31d465880a895d2685f5deb1274948e0a4eabe81a2f08f238c483",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_store/contracts/raw_source_rebuild_v1.manifest.schema.json",
        17_896,
        "f9d210967e54b66f39c8bb965d97b2001a0ebc0927e7c2c14edb8e474bfda695",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_store/contracts/raw_source_rebuild_v1.manifest.sha256",
        65,
        "2b8bc07cd479be2281781660efd26fd7a8f480e5f3f62053aeccd2b5e6b2070c",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_store/src/generated/raw_source_rebuild_manifest.rs",
        50_735,
        "3763fbee3ee45621afca990002b9298c791bf0396ebf7bccde0ae1bc9aecb7f2",
    ),
    ImmutableArtifactSpec::new(
        "contracts/conformance/vectors/event_store/raw_source_rebuild.v1.json",
        26_833,
        "c37a2bf3714f53ab04fae8c5c9dbe2ad4b3f5310efa51f46bd8b116660f1fe15",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_store/tests/fixtures/raw_source_rebuild.v1.json",
        26_833,
        "c37a2bf3714f53ab04fae8c5c9dbe2ad4b3f5310efa51f46bd8b116660f1fe15",
    ),
];

const BEHAVIOR_CASE_IDS: &[&str] = &[
    "valid_created",
    "valid_ok_without_authored_dimensions",
    "invalid_upload_status",
    "invalid_head_status",
    "invalid_get_status",
    "declared_size_over_public_max",
    "missing_get_body",
    "short_get_body",
    "trailing_get_body",
    "authored_bytes_short",
    "authored_bytes_wrong_hash",
    "upload_url_mismatch",
    "upload_hash_mismatch",
    "upload_size_mismatch",
    "upload_mime_mismatch",
    "head_url_mismatch",
    "head_size_mismatch",
    "head_mime_mismatch",
    "get_url_mismatch",
    "get_declared_size_mismatch",
    "get_complete_hash_mismatch",
    "unsupported_raster_mime",
    "malformed_raster",
    "animated_png",
    "declared_format_mismatch",
    "corrupt_png_crc",
    "corrupt_png_deflate",
    "invalid_png_color_type",
    "authored_dimension_mismatch",
    "animated_webp",
    "zero_width",
    "dimension_over_max",
    "pixel_limit",
    "progressive_jpeg",
    "jpeg_entropy_stripped",
    "jpeg_entropy_partial",
    "malformed_jpeg_dqt",
];
const PERSISTENCE_CASE_IDS: &[&str] = &[
    "canonical_evidence_reloads",
    "canonical_evidence_serializes",
    "leading_whitespace_is_noncanonical",
    "noncanonical_string_escaping_is_rejected",
    "field_reordering_is_noncanonical",
    "unknown_field_is_rejected",
    "private_bud11_field_is_rejected",
    "missing_field_is_rejected",
    "duplicate_field_is_rejected",
    "wrong_field_type_is_rejected",
    "schema_version_is_strict",
    "policy_version_is_strict",
    "url_must_be_canonical",
    "url_exact_maximum_is_valid",
    "url_one_over_maximum_is_rejected",
    "url_hash_must_match_sha256",
    "sha256_must_be_lower_hex",
    "size_must_be_nonzero",
    "size_must_be_bounded",
    "mime_must_be_supported",
    "format_must_match_mime",
    "dimensions_must_be_nonzero",
    "dimensions_must_respect_axis_limit",
    "dimensions_must_respect_pixel_limit",
    "bud02_status_is_strict",
    "bud01_head_status_is_strict",
    "bud01_get_status_is_strict",
    "uploaded_mutation_breaks_digest",
    "digest_must_be_lower_hex",
    "digest_must_match_all_facts",
    "input_is_bounded_before_parse",
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
struct PredecessorDescriptor {
    contract_id: String,
    immutable_artifacts: Vec<FileDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProtocolSourcePin {
    id: String,
    repository: String,
    revision: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PublicApiDescriptor {
    constants: Vec<String>,
    types: Vec<String>,
    functions: Vec<String>,
    methods: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct EvidenceDescriptor {
    schema_version: u32,
    policy_version: u16,
    schema: FileDescriptor,
    max_canonical_json_bytes: u64,
    max_url_utf8_bytes: u64,
    max_raster_bytes: u64,
    max_decoded_bytes: u64,
    max_dimension: u32,
    max_pixels: u64,
    wire_field_order: Vec<String>,
    digest_domain: String,
    digest_framing: String,
    serialize_operation_id: String,
    reload_operation_id: String,
    invariants: Vec<String>,
    decode_error_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorDescriptor {
    canonical_path: String,
    mirror_path: String,
    byte_length: u64,
    sha256: String,
    hash_algorithm: String,
    executor: FileDescriptor,
    executor_test: String,
    case_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ReadinessDescriptor {
    verify_operation_id: String,
    input_types: Vec<String>,
    output_type: String,
    evidence: EvidenceDescriptor,
    behavior_vector: VectorDescriptor,
    persistence_vector: VectorDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PublicationReadinessManifest {
    schema_version: u32,
    contract_id: String,
    authority_id: String,
    manifest_schema: FileDescriptor,
    predecessor: PredecessorDescriptor,
    protocol_sources: Vec<ProtocolSourcePin>,
    public_api: PublicApiDescriptor,
    readiness: ReadinessDescriptor,
    predecessor_source_supersessions: Vec<String>,
    transitive_predecessor_source_supersessions: Vec<String>,
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

struct ValidatedVector {
    bytes: Vec<u8>,
    case_ids: Vec<String>,
}

pub(crate) fn write_blossom_publication_readiness_manifest(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |transaction| {
        transaction.write(expected_artifacts(workspace_root)?)?;
        validate_manifest_under_lock(workspace_root)
    })
}

pub(crate) fn validate_blossom_publication_readiness_manifest(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_manifest_under_lock(workspace_root)
    })
}

pub(super) fn validate_blossom_publication_readiness(workspace_root: &Path) -> Result<(), String> {
    validate_blossom_publication_readiness_manifest(workspace_root)
}

fn validate_manifest_under_lock(workspace_root: &Path) -> Result<(), String> {
    validate_immutable_predecessor(workspace_root)?;
    validate_raw_source_rebuild_predecessor_production_sources_under_lock(
        workspace_root,
        RAW_PREDECESSOR_SUPERSEDED_PATHS,
        TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS,
    )?;
    for artifact in expected_artifacts(workspace_root)? {
        let actual = read_regular_file(workspace_root, artifact.relative)?;
        if actual != artifact.contents {
            return Err(format!(
                "generated Blossom publication-readiness contract {} is stale; run {WRITE_COMMAND}",
                artifact.relative
            ));
        }
    }

    let bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest: PublicationReadinessManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_RELATIVE, &bytes, &manifest)?;
    validate_manifest_shape(&manifest)?;

    let schema_bytes = read_regular_file(workspace_root, MANIFEST_SCHEMA_RELATIVE)?;
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("parse {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_SCHEMA_RELATIVE, &schema_bytes, &schema)?;
    validate_json_schema(
        &schema,
        &serde_json::to_value(&manifest).map_err(|error| {
            format!("serialize {MANIFEST_RELATIVE} for schema validation: {error}")
        })?,
    )?;

    let evidence_schema_bytes = read_regular_file(workspace_root, EVIDENCE_SCHEMA_RELATIVE)?;
    let evidence_schema: Value = serde_json::from_slice(&evidence_schema_bytes)
        .map_err(|error| format!("parse {EVIDENCE_SCHEMA_RELATIVE}: {error}"))?;
    validate_canonical_json(
        EVIDENCE_SCHEMA_RELATIVE,
        &evidence_schema_bytes,
        &evidence_schema,
    )?;

    let sidecar = read_regular_file(workspace_root, MANIFEST_SHA256_RELATIVE)?;
    if sidecar != format!("{}\n", sha256_hex(&bytes)).as_bytes() {
        return Err(format!(
            "{MANIFEST_SHA256_RELATIVE} must authenticate the exact manifest bytes"
        ));
    }
    Ok(())
}

fn expected_artifacts(workspace_root: &Path) -> Result<Vec<GeneratedArtifact>, String> {
    validate_immutable_predecessor(workspace_root)?;
    validate_raw_source_rebuild_predecessor_production_sources_under_lock(
        workspace_root,
        RAW_PREDECESSOR_SUPERSEDED_PATHS,
        TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS,
    )?;
    validate_source_contract(workspace_root)?;

    let evidence_schema_bytes = canonical_json_bytes(&evidence_schema())?;
    let manifest_schema_bytes = canonical_json_bytes(&manifest_schema())?;
    let manifest = describe_manifest(
        workspace_root,
        &manifest_schema_bytes,
        &evidence_schema_bytes,
    )?;
    let manifest_bytes = canonical_json_bytes(&manifest)?;
    let sidecar_bytes = format!("{}\n", sha256_hex(&manifest_bytes)).into_bytes();
    let behavior_vector_bytes = read_regular_file(workspace_root, BEHAVIOR_VECTOR_RELATIVE)?;
    let persistence_vector_bytes = read_regular_file(workspace_root, PERSISTENCE_VECTOR_RELATIVE)?;
    let descriptor_bytes = canonical_json_bytes(&json!({
        "schema_version": SCHEMA_VERSION,
        "contract_id": CONTRACT_ID,
        "manifest": descriptor_for_bytes(MANIFEST_RELATIVE, &manifest_bytes),
        "manifest_schema": descriptor_for_bytes(MANIFEST_SCHEMA_RELATIVE, &manifest_schema_bytes),
        "manifest_sidecar": descriptor_for_bytes(MANIFEST_SHA256_RELATIVE, &sidecar_bytes),
        "evidence_schema": descriptor_for_bytes(EVIDENCE_SCHEMA_RELATIVE, &evidence_schema_bytes),
        "behavior_vector_sha256": sha256_hex(&behavior_vector_bytes),
        "persistence_vector_sha256": sha256_hex(&persistence_vector_bytes),
        "predecessor_manifest_sha256": IMMUTABLE_RAW_PREDECESSOR_ARTIFACTS[0].sha256,
    }))?;

    Ok(vec![
        GeneratedArtifact {
            relative: MANIFEST_RELATIVE,
            contents: manifest_bytes,
        },
        GeneratedArtifact {
            relative: MANIFEST_SCHEMA_RELATIVE,
            contents: manifest_schema_bytes,
        },
        GeneratedArtifact {
            relative: MANIFEST_SHA256_RELATIVE,
            contents: sidecar_bytes,
        },
        GeneratedArtifact {
            relative: GENERATED_DESCRIPTOR_RELATIVE,
            contents: descriptor_bytes,
        },
        GeneratedArtifact {
            relative: EVIDENCE_SCHEMA_RELATIVE,
            contents: evidence_schema_bytes,
        },
        GeneratedArtifact {
            relative: BEHAVIOR_VECTOR_MIRROR_RELATIVE,
            contents: behavior_vector_bytes,
        },
        GeneratedArtifact {
            relative: PERSISTENCE_VECTOR_MIRROR_RELATIVE,
            contents: persistence_vector_bytes,
        },
    ])
}

fn describe_manifest(
    workspace_root: &Path,
    manifest_schema_bytes: &[u8],
    evidence_schema_bytes: &[u8],
) -> Result<PublicationReadinessManifest, String> {
    let behavior = validate_vector(
        workspace_root,
        BEHAVIOR_VECTOR_RELATIVE,
        "blossom_publication_readiness",
        BEHAVIOR_CASE_IDS,
        &[
            "blossom.verify_publication_readiness.valid",
            "blossom.verify_publication_readiness.invalid",
        ],
    )?;
    let persistence = validate_vector(
        workspace_root,
        PERSISTENCE_VECTOR_RELATIVE,
        "blossom_publication_readiness_persistence",
        PERSISTENCE_CASE_IDS,
        &[
            "blossom.publication_readiness_evidence.from_canonical_json.valid",
            "blossom.publication_readiness_evidence.from_canonical_json.invalid",
            "blossom.publication_readiness_evidence.to_canonical_json.valid",
        ],
    )?;

    Ok(PublicationReadinessManifest {
        schema_version: SCHEMA_VERSION,
        contract_id: CONTRACT_ID.to_owned(),
        authority_id: AUTHORITY_ID.to_owned(),
        manifest_schema: descriptor_for_bytes(MANIFEST_SCHEMA_RELATIVE, manifest_schema_bytes),
        predecessor: PredecessorDescriptor {
            contract_id: "radroots_event_store.raw_source_rebuild_v1".to_owned(),
            immutable_artifacts: IMMUTABLE_RAW_PREDECESSOR_ARTIFACTS
                .iter()
                .map(|artifact| FileDescriptor {
                    path: artifact.relative.to_owned(),
                    byte_length: artifact.byte_length as u64,
                    sha256: artifact.sha256.to_owned(),
                    hash_algorithm: HASH_ALGORITHM.to_owned(),
                })
                .collect(),
        },
        protocol_sources: expected_protocol_sources(),
        public_api: expected_public_api(),
        readiness: ReadinessDescriptor {
            verify_operation_id: VERIFY_OPERATION_ID.to_owned(),
            input_types: owned(&[
                "RadrootsBlossomByteVerifiedDescriptor",
                "Bytes",
                "RadrootsBlossomAuthoredRasterDimensions",
                "RadrootsBlossomBud02UploadObservation",
                "RadrootsBlossomBud01HeadObservation",
                "RadrootsBlossomBud01GetObservation",
            ]),
            output_type: "RadrootsBlossomPublicationReadinessEvidence".to_owned(),
            evidence: EvidenceDescriptor {
                schema_version: EVIDENCE_SCHEMA_VERSION,
                policy_version: READINESS_POLICY_VERSION,
                schema: descriptor_for_bytes(EVIDENCE_SCHEMA_RELATIVE, evidence_schema_bytes),
                max_canonical_json_bytes: EVIDENCE_MAX_BYTES as u64,
                max_url_utf8_bytes: URL_MAX_BYTES as u64,
                max_raster_bytes: RASTER_MAX_BYTES,
                max_decoded_bytes: RASTER_MAX_DECODED_BYTES,
                max_dimension: RASTER_MAX_DIMENSION,
                max_pixels: RASTER_MAX_PIXELS,
                wire_field_order: owned(WIRE_FIELD_ORDER),
                digest_domain: EVIDENCE_DIGEST_DOMAIN.to_owned(),
                digest_framing: "domain_bytes_then_u16be_policy_then_u64be_length_prefixed_url_then_raw_sha256_then_u64be_size_then_u64be_length_prefixed_mime_then_u8_format_then_u32be_width_then_u32be_height_then_u16be_bud02_then_u16be_head_then_u16be_get_then_u64be_uploaded_v1".to_owned(),
                serialize_operation_id: SERIALIZE_OPERATION_ID.to_owned(),
                reload_operation_id: RELOAD_OPERATION_ID.to_owned(),
                invariants: owned(SEMANTIC_INVARIANTS),
                decode_error_codes: owned(DECODE_ERROR_CODES),
            },
            behavior_vector: vector_descriptor(
                behavior,
                BEHAVIOR_VECTOR_RELATIVE,
                BEHAVIOR_VECTOR_MIRROR_RELATIVE,
                workspace_root,
                BEHAVIOR_VECTOR_EXECUTOR_RELATIVE,
                BEHAVIOR_VECTOR_EXECUTOR_TEST,
            )?,
            persistence_vector: vector_descriptor(
                persistence,
                PERSISTENCE_VECTOR_RELATIVE,
                PERSISTENCE_VECTOR_MIRROR_RELATIVE,
                workspace_root,
                PERSISTENCE_VECTOR_EXECUTOR_RELATIVE,
                PERSISTENCE_VECTOR_EXECUTOR_TEST,
            )?,
        },
        predecessor_source_supersessions: owned(RAW_PREDECESSOR_SUPERSEDED_PATHS),
        transitive_predecessor_source_supersessions: owned(
            TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS,
        ),
    })
}

fn vector_descriptor(
    vector: ValidatedVector,
    canonical_path: &str,
    mirror_path: &str,
    workspace_root: &Path,
    executor_path: &str,
    executor_test: &str,
) -> Result<VectorDescriptor, String> {
    Ok(VectorDescriptor {
        canonical_path: canonical_path.to_owned(),
        mirror_path: mirror_path.to_owned(),
        byte_length: vector.bytes.len() as u64,
        sha256: sha256_hex(&vector.bytes),
        hash_algorithm: HASH_ALGORITHM.to_owned(),
        executor: descriptor_for_file(workspace_root, executor_path)?,
        executor_test: executor_test.to_owned(),
        case_ids: vector.case_ids,
    })
}

fn validate_source_contract(workspace_root: &Path) -> Result<(), String> {
    validate_public_api_authority(workspace_root)?;
    validate_manifest_features(workspace_root)?;
    validate_operations_authority(workspace_root)?;
    validate_release_authority(workspace_root)?;
    validate_vector(
        workspace_root,
        BEHAVIOR_VECTOR_RELATIVE,
        "blossom_publication_readiness",
        BEHAVIOR_CASE_IDS,
        &[
            "blossom.verify_publication_readiness.valid",
            "blossom.verify_publication_readiness.invalid",
        ],
    )?;
    validate_vector(
        workspace_root,
        PERSISTENCE_VECTOR_RELATIVE,
        "blossom_publication_readiness_persistence",
        PERSISTENCE_CASE_IDS,
        &[
            "blossom.publication_readiness_evidence.from_canonical_json.valid",
            "blossom.publication_readiness_evidence.from_canonical_json.invalid",
            "blossom.publication_readiness_evidence.to_canonical_json.valid",
        ],
    )?;
    Ok(())
}

fn validate_immutable_predecessor(workspace_root: &Path) -> Result<(), String> {
    for artifact in IMMUTABLE_RAW_PREDECESSOR_ARTIFACTS {
        let bytes = read_regular_file(workspace_root, artifact.relative)?;
        if bytes.len() != artifact.byte_length || sha256_hex(&bytes) != artifact.sha256 {
            return Err(format!(
                "immutable raw-source rebuild predecessor artifact `{}` drifted",
                artifact.relative
            ));
        }
    }
    Ok(())
}

fn validate_public_api_authority(workspace_root: &Path) -> Result<(), String> {
    let source = read_utf8(workspace_root, READINESS_SOURCE_RELATIVE)?;
    validate_public_api_source(&source)?;
    let lib = read_utf8(workspace_root, BLOSSOM_LIB_RELATIVE)?;
    validate_lib_reexports(&lib)
}

fn validate_public_api_source(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|error| format!("parse {READINESS_SOURCE_RELATIVE}: {error}"))?;
    let mut constants = BTreeSet::new();
    let mut types = BTreeSet::new();
    let mut functions = BTreeSet::new();
    let mut methods = BTreeSet::new();
    let mut public_type_fields_private = BTreeSet::new();
    let mut private_wire_types = BTreeSet::new();
    let mut forbidden_deserialize = BTreeSet::new();

    for item in &file.items {
        match item {
            Item::Const(item) if is_public(&item.vis) => {
                constants.insert(item.ident.to_string());
            }
            Item::Enum(item) if is_public(&item.vis) => {
                let name = item.ident.to_string();
                types.insert(name.clone());
                if item
                    .variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter())
                    .all(|field| !is_public(&field.vis))
                {
                    public_type_fields_private.insert(name);
                }
            }
            Item::Struct(item) => {
                let name = item.ident.to_string();
                if is_public(&item.vis) {
                    types.insert(name.clone());
                    if item.fields.iter().all(|field| !is_public(&field.vis)) {
                        public_type_fields_private.insert(name.clone());
                    }
                    if SEALED_TYPES.contains(&name.as_str()) && derives(&item.attrs, "Deserialize")
                    {
                        forbidden_deserialize.insert(name);
                    }
                } else if PRIVATE_WIRE_TYPES.contains(&name.as_str())
                    && derives(&item.attrs, "Serialize")
                    && derives(&item.attrs, "Deserialize")
                    && has_serde_deny_unknown_fields(&item.attrs)
                {
                    private_wire_types.insert(name);
                }
            }
            Item::Fn(item) if is_public(&item.vis) => {
                functions.insert(item.sig.ident.to_string());
            }
            Item::Impl(item) => {
                let syn::Type::Path(self_type) = item.self_ty.as_ref() else {
                    continue;
                };
                let Some(type_name) = self_type.path.segments.last() else {
                    continue;
                };
                let type_name = type_name.ident.to_string();
                if item.trait_.as_ref().is_some_and(|(_, path, _)| {
                    path.segments
                        .last()
                        .is_some_and(|segment| segment.ident == "Deserialize")
                }) && SEALED_TYPES.contains(&type_name.as_str())
                {
                    forbidden_deserialize.insert(type_name.clone());
                }
                if item.trait_.is_none() {
                    for method in &item.items {
                        if let ImplItem::Fn(method) = method
                            && is_public(&method.vis)
                        {
                            methods.insert(format!("{type_name}::{}", method.sig.ident));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    for (label, actual, expected) in [
        ("constants", constants, expected_set(PUBLIC_CONSTANTS)),
        ("types", types.clone(), expected_set(PUBLIC_TYPES)),
        ("functions", functions, expected_set(PUBLIC_FUNCTIONS)),
        ("methods", methods, expected_set(PUBLIC_METHODS)),
    ] {
        if actual != expected {
            return Err(format!(
                "{READINESS_SOURCE_RELATIVE} public {label} drifted: expected {expected:?}, found {actual:?}"
            ));
        }
    }
    if public_type_fields_private != types {
        return Err(format!(
            "{READINESS_SOURCE_RELATIVE} public readiness types must expose no public fields"
        ));
    }
    if private_wire_types != expected_set(PRIVATE_WIRE_TYPES) {
        return Err(format!(
            "{READINESS_SOURCE_RELATIVE} private wire models must derive Serialize/Deserialize with deny_unknown_fields"
        ));
    }
    if !forbidden_deserialize.is_empty() {
        return Err(format!(
            "sealed readiness types must not implement Deserialize: {forbidden_deserialize:?}"
        ));
    }
    Ok(())
}

fn validate_lib_reexports(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|error| format!("parse {BLOSSOM_LIB_RELATIVE}: {error}"))?;
    let mut names = BTreeSet::new();
    for item in &file.items {
        if let Item::Use(item) = item
            && is_public(&item.vis)
        {
            collect_use_names(&item.tree, &mut names);
        }
    }
    let required = PUBLIC_CONSTANTS
        .iter()
        .chain(PUBLIC_TYPES)
        .copied()
        .collect::<BTreeSet<_>>();
    let missing = required
        .into_iter()
        .filter(|name| !names.contains(*name))
        .collect::<Vec<_>>();
    if !missing.is_empty() || !names.contains("verify_publication_readiness") {
        return Err(format!(
            "{BLOSSOM_LIB_RELATIVE} readiness reexports are incomplete: missing {missing:?}"
        ));
    }
    Ok(())
}

fn collect_use_names(tree: &syn::UseTree, names: &mut BTreeSet<String>) {
    match tree {
        syn::UseTree::Name(name) => {
            names.insert(name.ident.to_string());
        }
        syn::UseTree::Rename(rename) => {
            names.insert(rename.rename.to_string());
        }
        syn::UseTree::Path(path) => collect_use_names(&path.tree, names),
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_use_names(item, names);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

fn validate_manifest_features(workspace_root: &Path) -> Result<(), String> {
    let manifest = parse_toml(workspace_root, BLOSSOM_MANIFEST_RELATIVE)?;
    let dependencies = manifest
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{BLOSSOM_MANIFEST_RELATIVE} must declare dependencies"))?;
    let actual_dependencies = dependencies
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected_dependencies = BTreeSet::from([
        "image",
        "mediatype",
        "serde",
        "serde_json",
        "sha2",
        "unicode-general-category",
        "url_nostd",
        "zune-core",
        "zune-jpeg",
    ]);
    if actual_dependencies != expected_dependencies {
        return Err(format!(
            "{BLOSSOM_MANIFEST_RELATIVE} dependency boundary drifted: expected {expected_dependencies:?}, found {actual_dependencies:?}"
        ));
    }
    for dependency in ["serde", "serde_json", "image", "zune-core", "zune-jpeg"] {
        let table = dependencies
            .get(dependency)
            .and_then(toml::Value::as_table)
            .ok_or_else(|| {
                format!("{BLOSSOM_MANIFEST_RELATIVE} must declare {dependency} as a table")
            })?;
        if table.get("workspace").and_then(toml::Value::as_bool) != Some(true)
            || table.get("optional").and_then(toml::Value::as_bool) != Some(true)
        {
            return Err(format!(
                "{BLOSSOM_MANIFEST_RELATIVE} dependency {dependency} must be optional and workspace-governed"
            ));
        }
    }
    let features = manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{BLOSSOM_MANIFEST_RELATIVE} must declare features"))?;
    for (feature, expected) in [
        ("default", BTreeSet::from(["serde"])),
        ("serde", BTreeSet::from(["dep:serde", "dep:serde_json"])),
        (
            "std",
            BTreeSet::from(["serde?/std", "sha2/std", "url_nostd/std"]),
        ),
        (
            "raster-decode",
            BTreeSet::from(["dep:image", "dep:zune-core", "dep:zune-jpeg", "std"]),
        ),
    ] {
        let expected = expected
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let actual = toml_string_set(
            &format!("{BLOSSOM_MANIFEST_RELATIVE} feature {feature}"),
            features.get(feature),
        )?;
        if actual != expected {
            return Err(format!(
                "{BLOSSOM_MANIFEST_RELATIVE} feature {feature} drifted: expected {expected:?}, found {actual:?}"
            ));
        }
    }
    Ok(())
}

fn validate_operations_authority(workspace_root: &Path) -> Result<(), String> {
    let operations = parse_toml(workspace_root, OPERATIONS_RELATIVE)?;
    let table = operations
        .get("operations")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{OPERATIONS_RELATIVE} must declare operations"))?;
    validate_operation(
        table,
        "blossom_verify_publication_readiness",
        VERIFY_OPERATION_ID,
        BEHAVIOR_VECTOR_RELATIVE,
        &[
            "blossom.verify_publication_readiness.valid",
            "blossom.verify_publication_readiness.invalid",
        ],
    )?;
    validate_operation(
        table,
        "blossom_publication_readiness_evidence_to_canonical_json",
        SERIALIZE_OPERATION_ID,
        PERSISTENCE_VECTOR_RELATIVE,
        &["blossom.publication_readiness_evidence.to_canonical_json.valid"],
    )?;
    validate_operation(
        table,
        "blossom_publication_readiness_evidence_from_canonical_json",
        RELOAD_OPERATION_ID,
        PERSISTENCE_VECTOR_RELATIVE,
        &[
            "blossom.publication_readiness_evidence.from_canonical_json.valid",
            "blossom.publication_readiness_evidence.from_canonical_json.invalid",
        ],
    )
}

fn validate_operation(
    operations: &toml::map::Map<String, toml::Value>,
    key: &str,
    id: &str,
    vector: &str,
    case_kinds: &[&str],
) -> Result<(), String> {
    let operation = operations
        .get(key)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{OPERATIONS_RELATIVE} must declare {key}"))?;
    let conformance = operation
        .get("conformance")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{OPERATIONS_RELATIVE} operation {key} needs conformance"))?;
    if operation.get("id").and_then(toml::Value::as_str) != Some(id)
        || operation.get("domain").and_then(toml::Value::as_str) != Some("blossom")
        || operation
            .get("deterministic")
            .and_then(toml::Value::as_bool)
            != Some(true)
        || operation.get("transport").and_then(toml::Value::as_str) != Some("none")
        || operation.get("signing").and_then(toml::Value::as_str) != Some("none")
        || conformance.get("vector").and_then(toml::Value::as_str) != Some(vector)
        || toml_string_set("operation case kinds", conformance.get("case_kinds"))?
            != case_kinds.iter().map(|kind| (*kind).to_owned()).collect()
    {
        return Err(format!(
            "{OPERATIONS_RELATIVE} operation {key} semantic authority drifted"
        ));
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
    if matching.len() != 1
        || matching[0]
            .get("classification")
            .and_then(toml::Value::as_str)
            != Some("feature")
    {
        return Err(format!(
            "{RELEASE_RELATIVE} must contain exactly one feature change {RELEASE_CHANGE_ID}"
        ));
    }
    let changelog = read_utf8(workspace_root, CHANGELOG_RELATIVE)?;
    if changelog.matches(CHANGELOG_MARKER).count() != 1 {
        return Err(format!(
            "{CHANGELOG_RELATIVE} must contain exactly one {CHANGELOG_MARKER}"
        ));
    }
    Ok(())
}

fn validate_vector(
    workspace_root: &Path,
    relative: &str,
    suite_name: &str,
    expected_ids: &[&str],
    allowed_kinds: &[&str],
) -> Result<ValidatedVector, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    let suite: VectorSuite =
        serde_json::from_slice(&bytes).map_err(|error| format!("parse {relative}: {error}"))?;
    let ids = suite
        .vectors
        .iter()
        .map(|case| case.id.as_str())
        .collect::<Vec<_>>();
    if suite.suite != suite_name
        || suite.contract_version != "1.0.0"
        || ids != expected_ids
        || suite.vectors.iter().any(|case| {
            !allowed_kinds.contains(&case.kind.as_str())
                || !case.input.is_object()
                || !case.expected.is_object()
        })
    {
        return Err(format!("{relative} case inventory or shape drifted"));
    }
    let unique = ids.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() != ids.len() {
        return Err(format!("{relative} contains duplicate case ids"));
    }
    Ok(ValidatedVector {
        bytes,
        case_ids: expected_ids.iter().map(|id| (*id).to_owned()).collect(),
    })
}

fn validate_manifest_shape(manifest: &PublicationReadinessManifest) -> Result<(), String> {
    if manifest.schema_version != SCHEMA_VERSION
        || manifest.contract_id != CONTRACT_ID
        || manifest.authority_id != AUTHORITY_ID
        || manifest.predecessor.contract_id != "radroots_event_store.raw_source_rebuild_v1"
        || manifest.protocol_sources != expected_protocol_sources()
        || manifest.public_api != expected_public_api()
        || manifest.readiness.verify_operation_id != VERIFY_OPERATION_ID
        || manifest.readiness.input_types
            != owned(&[
                "RadrootsBlossomByteVerifiedDescriptor",
                "Bytes",
                "RadrootsBlossomAuthoredRasterDimensions",
                "RadrootsBlossomBud02UploadObservation",
                "RadrootsBlossomBud01HeadObservation",
                "RadrootsBlossomBud01GetObservation",
            ])
        || manifest.readiness.output_type != "RadrootsBlossomPublicationReadinessEvidence"
        || manifest.readiness.evidence.schema_version != EVIDENCE_SCHEMA_VERSION
        || manifest.readiness.evidence.policy_version != READINESS_POLICY_VERSION
        || manifest.readiness.evidence.max_canonical_json_bytes != EVIDENCE_MAX_BYTES as u64
        || manifest.readiness.evidence.max_url_utf8_bytes != URL_MAX_BYTES as u64
        || manifest.readiness.evidence.max_raster_bytes != RASTER_MAX_BYTES
        || manifest.readiness.evidence.max_decoded_bytes != RASTER_MAX_DECODED_BYTES
        || manifest.readiness.evidence.max_dimension != RASTER_MAX_DIMENSION
        || manifest.readiness.evidence.max_pixels != RASTER_MAX_PIXELS
        || manifest.readiness.evidence.wire_field_order != owned(WIRE_FIELD_ORDER)
        || manifest.readiness.evidence.digest_domain != EVIDENCE_DIGEST_DOMAIN
        || manifest.readiness.evidence.serialize_operation_id != SERIALIZE_OPERATION_ID
        || manifest.readiness.evidence.reload_operation_id != RELOAD_OPERATION_ID
        || manifest.readiness.evidence.invariants != owned(SEMANTIC_INVARIANTS)
        || manifest.readiness.evidence.decode_error_codes != owned(DECODE_ERROR_CODES)
        || manifest.predecessor_source_supersessions != owned(RAW_PREDECESSOR_SUPERSEDED_PATHS)
        || manifest.transitive_predecessor_source_supersessions
            != owned(TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS)
        || manifest.readiness.behavior_vector.case_ids != owned(BEHAVIOR_CASE_IDS)
        || manifest.readiness.persistence_vector.case_ids != owned(PERSISTENCE_CASE_IDS)
    {
        return Err(format!("{MANIFEST_RELATIVE} semantic shape drifted"));
    }
    let predecessor = manifest
        .predecessor
        .immutable_artifacts
        .iter()
        .map(|artifact| {
            (
                artifact.path.as_str(),
                artifact.byte_length as usize,
                artifact.sha256.as_str(),
            )
        })
        .collect::<Vec<_>>();
    let expected = IMMUTABLE_RAW_PREDECESSOR_ARTIFACTS
        .iter()
        .map(|artifact| (artifact.relative, artifact.byte_length, artifact.sha256))
        .collect::<Vec<_>>();
    if predecessor != expected {
        return Err(format!("{MANIFEST_RELATIVE} predecessor identity drifted"));
    }
    Ok(())
}

fn manifest_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/contracts/blossom/publication-readiness-v1.schema.json",
        "title": "Radroots Blossom Publication Readiness Semantic Contract",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schema_version", "contract_id", "authority_id", "manifest_schema",
            "predecessor", "protocol_sources", "public_api", "readiness",
            "predecessor_source_supersessions", "transitive_predecessor_source_supersessions"
        ],
        "properties": {
            "schema_version": {"const": SCHEMA_VERSION},
            "contract_id": {"const": CONTRACT_ID},
            "authority_id": {"const": AUTHORITY_ID},
            "manifest_schema": {"$ref": "#/$defs/file"},
            "predecessor": {
                "type": "object", "additionalProperties": false,
                "required": ["contract_id", "immutable_artifacts"],
                "properties": {
                    "contract_id": {"const": "radroots_event_store.raw_source_rebuild_v1"},
                    "immutable_artifacts": {"type": "array", "minItems": 6, "maxItems": 6, "items": {"$ref": "#/$defs/file"}}
                }
            },
            "protocol_sources": {"const": expected_protocol_sources()},
            "public_api": {"const": expected_public_api()},
            "readiness": {"$ref": "#/$defs/readiness"},
            "predecessor_source_supersessions": {"const": owned(RAW_PREDECESSOR_SUPERSEDED_PATHS)},
            "transitive_predecessor_source_supersessions": {"const": owned(TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS)}
        },
        "$defs": {
            "file": {
                "type": "object", "additionalProperties": false,
                "required": ["path", "byte_length", "sha256", "hash_algorithm"],
                "properties": {
                    "path": {"type": "string", "minLength": 1},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "hash_algorithm": {"const": HASH_ALGORITHM}
                }
            },
            "vector": {
                "type": "object", "additionalProperties": false,
                "required": ["canonical_path", "mirror_path", "byte_length", "sha256", "hash_algorithm", "executor", "executor_test", "case_ids"],
                "properties": {
                    "canonical_path": {"type": "string", "minLength": 1},
                    "mirror_path": {"type": "string", "minLength": 1},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "hash_algorithm": {"const": HASH_ALGORITHM},
                    "executor": {"$ref": "#/$defs/file"},
                    "executor_test": {"type": "string", "minLength": 1},
                    "case_ids": {"type": "array", "minItems": 1, "uniqueItems": true, "items": {"type": "string", "minLength": 1}}
                }
            },
            "evidence": {
                "type": "object", "additionalProperties": false,
                "required": ["schema_version", "policy_version", "schema", "max_canonical_json_bytes", "max_url_utf8_bytes", "max_raster_bytes", "max_decoded_bytes", "max_dimension", "max_pixels", "wire_field_order", "digest_domain", "digest_framing", "serialize_operation_id", "reload_operation_id", "invariants", "decode_error_codes"],
                "properties": {
                    "schema_version": {"const": EVIDENCE_SCHEMA_VERSION},
                    "policy_version": {"const": READINESS_POLICY_VERSION},
                    "schema": {"$ref": "#/$defs/file"},
                    "max_canonical_json_bytes": {"const": EVIDENCE_MAX_BYTES},
                    "max_url_utf8_bytes": {"const": URL_MAX_BYTES},
                    "max_raster_bytes": {"const": RASTER_MAX_BYTES},
                    "max_decoded_bytes": {"const": RASTER_MAX_DECODED_BYTES},
                    "max_dimension": {"const": RASTER_MAX_DIMENSION},
                    "max_pixels": {"const": RASTER_MAX_PIXELS},
                    "wire_field_order": {"const": owned(WIRE_FIELD_ORDER)},
                    "digest_domain": {"const": EVIDENCE_DIGEST_DOMAIN},
                    "digest_framing": {"type": "string", "minLength": 1},
                    "serialize_operation_id": {"const": SERIALIZE_OPERATION_ID},
                    "reload_operation_id": {"const": RELOAD_OPERATION_ID},
                    "invariants": {"const": owned(SEMANTIC_INVARIANTS)},
                    "decode_error_codes": {"const": owned(DECODE_ERROR_CODES)}
                }
            },
            "readiness": {
                "type": "object", "additionalProperties": false,
                "required": ["verify_operation_id", "input_types", "output_type", "evidence", "behavior_vector", "persistence_vector"],
                "properties": {
                    "verify_operation_id": {"const": VERIFY_OPERATION_ID},
                    "input_types": {"type": "array", "minItems": 6, "maxItems": 6, "items": {"type": "string", "minLength": 1}},
                    "output_type": {"const": "RadrootsBlossomPublicationReadinessEvidence"},
                    "evidence": {"$ref": "#/$defs/evidence"},
                    "behavior_vector": {"$ref": "#/$defs/vector"},
                    "persistence_vector": {"$ref": "#/$defs/vector"}
                }
            }
        }
    })
}

fn evidence_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/contracts/blossom/publication-readiness-evidence-v1.schema.json",
        "title": "Radroots Blossom Publication Readiness Evidence v1",
        "type": "object",
        "additionalProperties": false,
        "required": WIRE_FIELD_ORDER,
        "properties": {
            "schema_version": {"const": EVIDENCE_SCHEMA_VERSION},
            "policy_version": {"const": READINESS_POLICY_VERSION},
            "url": {"type": "string", "minLength": 1, "maxLength": URL_MAX_BYTES},
            "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
            "size": {"type": "integer", "minimum": 1, "maximum": RASTER_MAX_BYTES},
            "media_type": {"enum": ["image/jpeg", "image/png", "image/webp"]},
            "raster_format": {"enum": ["jpeg", "png", "still_webp"]},
            "dimensions": {
                "type": "object", "additionalProperties": false,
                "required": ["width", "height"],
                "properties": {
                    "width": {"type": "integer", "minimum": 1, "maximum": RASTER_MAX_DIMENSION},
                    "height": {"type": "integer", "minimum": 1, "maximum": RASTER_MAX_DIMENSION}
                }
            },
            "bud02_status": {"enum": [200, 201]},
            "bud01_head_status": {"const": 200},
            "bud01_get_status": {"const": 200},
            "uploaded": {"type": "integer", "minimum": 0},
            "evidence_digest": {"type": "string", "pattern": "^[0-9a-f]{64}$"}
        }
    })
}

fn expected_protocol_sources() -> Vec<ProtocolSourcePin> {
    PROTOCOL_SOURCE_PINS
        .iter()
        .map(|(id, repository, revision)| ProtocolSourcePin {
            id: (*id).to_owned(),
            repository: (*repository).to_owned(),
            revision: (*revision).to_owned(),
        })
        .collect()
}

fn expected_public_api() -> PublicApiDescriptor {
    PublicApiDescriptor {
        constants: owned(PUBLIC_CONSTANTS),
        types: owned(PUBLIC_TYPES),
        functions: owned(PUBLIC_FUNCTIONS),
        methods: owned(PUBLIC_METHODS),
    }
}

fn descriptor_for_file(workspace_root: &Path, path: &str) -> Result<FileDescriptor, String> {
    Ok(descriptor_for_bytes(
        path,
        &read_regular_file(workspace_root, path)?,
    ))
}

fn descriptor_for_bytes(path: &str, bytes: &[u8]) -> FileDescriptor {
    FileDescriptor {
        path: path.to_owned(),
        byte_length: bytes.len() as u64,
        sha256: sha256_hex(bytes),
        hash_algorithm: HASH_ALGORITHM.to_owned(),
    }
}

fn read_utf8(workspace_root: &Path, relative: &str) -> Result<String, String> {
    String::from_utf8(read_regular_file(workspace_root, relative)?)
        .map_err(|error| format!("{relative} must be UTF-8: {error}"))
}

fn parse_toml(workspace_root: &Path, relative: &str) -> Result<toml::Value, String> {
    let source = read_utf8(workspace_root, relative)?;
    toml::from_str(&source).map_err(|error| format!("parse {relative}: {error}"))
}

fn toml_string_set(label: &str, value: Option<&toml::Value>) -> Result<BTreeSet<String>, String> {
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

fn derives(attributes: &[syn::Attribute], derive: &str) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("derive")
            && attribute
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                )
                .is_ok_and(|paths| {
                    paths.iter().any(|path| {
                        path.segments
                            .last()
                            .is_some_and(|segment| segment.ident == derive)
                    })
                })
    })
}

fn has_serde_deny_unknown_fields(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        if !attribute.path().is_ident("serde") {
            return false;
        }
        let mut found = false;
        let _ = attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("deny_unknown_fields") {
                found = true;
            }
            Ok(())
        });
        found
    })
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

fn expected_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
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

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("xtask workspace root")
            .to_path_buf()
    }

    #[test]
    fn readiness_public_api_is_ast_governed() {
        let source = read_utf8(&workspace_root(), READINESS_SOURCE_RELATIVE).unwrap();
        validate_public_api_source(&source).unwrap();
        validate_public_api_source(&format!("// unrelated comment\n{source}")).unwrap();

        let exposed = source.replacen(
            "    url: RadrootsBlossomApprovedBlobUrl,",
            "    pub url: RadrootsBlossomApprovedBlobUrl,",
            1,
        );
        assert!(
            validate_public_api_source(&exposed)
                .unwrap_err()
                .contains("public readiness types")
        );

        let deserializable = source.replacen(
            "#[derive(Clone, Debug, PartialEq, Eq)]\npub struct RadrootsBlossomPublicationReadinessEvidence",
            "#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]\npub struct RadrootsBlossomPublicationReadinessEvidence",
            1,
        );
        assert!(
            validate_public_api_source(&deserializable)
                .unwrap_err()
                .contains("must not implement Deserialize")
        );
    }

    #[test]
    fn readiness_vectors_have_exact_semantic_inventories() {
        let root = workspace_root();
        validate_vector(
            &root,
            BEHAVIOR_VECTOR_RELATIVE,
            "blossom_publication_readiness",
            BEHAVIOR_CASE_IDS,
            &[
                "blossom.verify_publication_readiness.valid",
                "blossom.verify_publication_readiness.invalid",
            ],
        )
        .unwrap();
        validate_vector(
            &root,
            PERSISTENCE_VECTOR_RELATIVE,
            "blossom_publication_readiness_persistence",
            PERSISTENCE_CASE_IDS,
            &[
                "blossom.publication_readiness_evidence.from_canonical_json.valid",
                "blossom.publication_readiness_evidence.from_canonical_json.invalid",
                "blossom.publication_readiness_evidence.to_canonical_json.valid",
            ],
        )
        .unwrap();
    }

    #[test]
    fn semantic_manifest_excludes_mutable_source_and_release_identity() {
        let root = workspace_root();
        let schema = canonical_json_bytes(&manifest_schema()).unwrap();
        let evidence_schema = canonical_json_bytes(&evidence_schema()).unwrap();
        let manifest = describe_manifest(&root, &schema, &evidence_schema).unwrap();
        let value = serde_json::to_value(manifest).unwrap();
        assert!(value.get("source_files").is_none());
        assert!(value.get("release").is_none());
        assert!(value.get("workspace_lock").is_none());
    }

    #[test]
    fn generated_readiness_contract_is_current() {
        validate_blossom_publication_readiness_manifest(&workspace_root()).unwrap();
    }
}
