use super::{
    artifact_bundle::{GeneratedArtifact, read_regular_file, with_artifact_bundle_transaction},
    phase1_publication_media_readiness::{
        BLOSSOM_PREDECESSOR_ARTIFACTS, BLOSSOM_PREDECESSOR_SUPERSEDED_ARTIFACTS, FileDescriptor,
        PredecessorDescriptor, ProtocolSourcePin, descriptor_for_bytes, descriptor_for_file,
        predecessor_descriptor, validate_immutable_predecessor_with_superseded_artifacts,
        validate_source_supersessions,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use syn::{BinOp, Expr, FnArg, Item, Lit, Pat, Stmt, Type, UseTree, Visibility, visit::Visit};

const SCHEMA_VERSION: u32 = 1;
const CONTRACT_ID: &str = "radroots_blossom.raster_decoder_security_v1";
const AUTHORITY_ID: &str = "blossom_raster_decoder_security_v1";
const HASH_ALGORITHM: &str = "sha256_bytes_v1";
const LIB_CRATE_VERSION: &str = "0.1.0-alpha";
const WRITE_COMMAND: &str = "cargo xtask contract blossom-raster-decoder-security-manifest --write";

const MANIFEST_RELATIVE: &str = "crates/blossom/contracts/raster_decoder_security_v1.manifest.json";
const MANIFEST_SCHEMA_RELATIVE: &str =
    "crates/blossom/contracts/raster_decoder_security_v1.manifest.schema.json";
const MANIFEST_SHA256_RELATIVE: &str =
    "crates/blossom/contracts/raster_decoder_security_v1.manifest.sha256";
const GENERATED_DESCRIPTOR_RELATIVE: &str =
    "crates/blossom/contracts/raster_decoder_security_v1.descriptor.json";
const VECTOR_RELATIVE: &str =
    "contracts/conformance/vectors/blossom/raster_decoder_security.v1.json";
const VECTOR_MIRROR_RELATIVE: &str =
    "crates/blossom/tests/fixtures/raster_decoder_security.v1.json";
const VECTOR_EXECUTOR_RELATIVE: &str = "crates/blossom/tests/decoder_security.rs";
const REGRESSION_TEST: &str = "decoder_regression_corpus_executes_every_case";
const DIFFERENTIAL_TEST: &str = "decoder_differential_matches_independent_backend";
const RESOURCE_TEST: &str = "maximum_resource_probe";
const READINESS_SOURCE_RELATIVE: &str = "crates/blossom/src/publication_readiness.rs";
const JPEG_SOURCE_RELATIVE: &str = "crates/blossom/src/publication_readiness/sequential_jpeg.rs";
const ERROR_SOURCE_RELATIVE: &str = "crates/blossom/src/error.rs";
const WORKSPACE_MANIFEST_RELATIVE: &str = "Cargo.toml";
const BLOSSOM_MANIFEST_RELATIVE: &str = "crates/blossom/Cargo.toml";
const FUZZ_MANIFEST_RELATIVE: &str = "fuzz/Cargo.toml";
const FUZZ_LOCKFILE_RELATIVE: &str = "fuzz/Cargo.lock";
const FUZZ_TOOLCHAIN_RELATIVE: &str = "rust-toolchain-fuzz.toml";
const FUZZ_COMMON_RELATIVE: &str = "fuzz/fuzz_targets/common.rs";
const FUZZ_CORPUS_RELATIVE: &str = "fuzz/corpus";
const SECURITY_DOCUMENT_RELATIVE: &str = "docs/blossom-raster-decoder-security.md";
const OPERATIONS_RELATIVE: &str = "contracts/operations.toml";
const RELEASE_RELATIVE: &str = "contracts/releases/1.0.0-alpha.1.toml";
const CHANGELOG_RELATIVE: &str = "CHANGELOG.md";
const RELEASE_CHANGE_ID: &str = "blossom-raster-decoder-security";
const CHANGELOG_MARKER: &str = "<!-- release-change: blossom-raster-decoder-security -->";

const OPERATION_KEY: &str = "blossom_verify_publication_readiness";
const OPERATION_ID: &str = "blossom.verify_publication_readiness";
const PHANTOM_OPERATION_KEY: &str = "blossom_verify_raster_decoder_security";
const PHANTOM_OPERATION_ID: &str = "blossom.verify_raster_decoder_security";
const PRIMARY_OPERATION_VECTOR: &str =
    "contracts/conformance/vectors/blossom/publication_readiness.v1.json";
const PRIMARY_OPERATION_CASE_KINDS: &[&str] = &[
    "blossom.verify_publication_readiness.valid",
    "blossom.verify_publication_readiness.invalid",
];
const ACCEPTED_KIND: &str = "blossom.verify_publication_readiness.decoder_security.accepted";
const REJECTED_KIND: &str = "blossom.verify_publication_readiness.decoder_security.rejected";

const MAX_RASTER_BYTES: u64 = 10_485_760;
const MAX_DECODED_BYTES: u64 = 80_000_000;
const MAX_DIMENSION: u64 = 16_384;
const MAX_PIXELS: u64 = 20_000_000;
const MAX_CONTAINER_RECORDS: u64 = 65_536;
const JPEG_MAX_SCANS: u64 = 4;
const JPEG_MAX_BLOCKS: u64 = 3_200_000;
const JPEG_MAX_COEFFICIENT_STEPS: u64 = 204_800_000;
const JPEG_ENTROPY_BIT_READS_PER_BYTE: u64 = 8;
const PEAK_RSS_KIB_LIMIT: u64 = 131_072;

const FUZZ_TOOLCHAIN_CHANNEL: &str = "nightly-2026-07-15";
const FUZZ_SMOKE_RUNS: u64 = 256;
const FUZZ_SMOKE_SEED: u64 = 424242;
const FUZZ_SMOKE_MAX_INPUT_BYTES: u64 = 65_536;
const FUZZ_SMOKE_TIMEOUT_SECONDS: u64 = 5;
const FUZZ_ENGINE_RSS_LIMIT_MB: u64 = 2048;

const FUZZ_TARGETS: &[&str] = &["publication_jpeg", "publication_png", "publication_webp"];
const FUZZ_TARGET_SPECS: &[(&str, &str, &str, &str)] = &[
    ("publication_jpeg", "jpeg", "image/jpeg", "jpg"),
    ("publication_png", "png", "image/png", "png"),
    ("publication_webp", "webp", "image/webp", "webp"),
];

const READINESS_LIMIT_CONSTANTS: &[(&str, u64)] = &[
    (
        "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES",
        MAX_RASTER_BYTES,
    ),
    (
        "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES",
        MAX_DECODED_BYTES,
    ),
    (
        "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION",
        MAX_DIMENSION,
    ),
    ("RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_PIXELS", MAX_PIXELS),
    (
        "PUBLICATION_RASTER_MAX_CONTAINER_RECORDS",
        MAX_CONTAINER_RECORDS,
    ),
];
const JPEG_LIMIT_CONSTANTS: &[(&str, u64)] = &[
    ("MAX_SEQUENTIAL_JPEG_SCANS", JPEG_MAX_SCANS),
    ("MAX_SEQUENTIAL_JPEG_BLOCKS", JPEG_MAX_BLOCKS),
    (
        "MAX_SEQUENTIAL_JPEG_COEFFICIENT_STEPS",
        JPEG_MAX_COEFFICIENT_STEPS,
    ),
];

const NORMATIVE_ERROR_CODES: &[&str] = &[
    "publication_raster_empty",
    "publication_raster_byte_limit_exceeded",
    "unsupported_publication_raster_media_type",
    "invalid_publication_raster",
    "publication_jpeg_process_forbidden",
    "publication_raster_animation_forbidden",
    "publication_raster_container_dimension_mismatch",
    "publication_raster_decode_allocation_failed",
    "publication_raster_decoded_byte_limit_exceeded",
    "publication_raster_decode_failed",
    "publication_raster_dimensions_out_of_range",
    "publication_raster_pixel_limit_exceeded",
    "publication_raster_process_forbidden",
];
const NORMATIVE_ERROR_VARIANTS: &[&str] = &[
    "PublicationRasterEmpty",
    "PublicationRasterByteLimitExceeded",
    "UnsupportedPublicationRasterMediaType",
    "InvalidPublicationRaster",
    "PublicationJpegProcessForbidden",
    "PublicationRasterAnimationForbidden",
    "PublicationRasterContainerDimensionMismatch",
    "PublicationRasterDecodeAllocationFailed",
    "PublicationRasterDecodedByteLimitExceeded",
    "PublicationRasterDecodeFailed",
    "PublicationRasterDimensionsOutOfRange",
    "PublicationRasterPixelLimitExceeded",
    "PublicationRasterProcessForbidden",
];
const OBSERVED_REJECTION_ERROR_CODES: &[&str] = &[
    "invalid_publication_raster",
    "publication_jpeg_process_forbidden",
    "publication_raster_animation_forbidden",
    "publication_raster_decode_failed",
    "publication_raster_dimensions_out_of_range",
    "publication_raster_pixel_limit_exceeded",
    "publication_raster_process_forbidden",
];
const REQUIRED_ACCEPTED_MUTATIONS: &[&str] = &["jpeg_sof1", "none"];
const REQUIRED_REJECTED_MUTATIONS: &[&str] = &[
    "jpeg_entropy_truncated",
    "jpeg_huffman_overfull",
    "jpeg_precision_12",
    "jpeg_progressive",
    "jpeg_wrong_restart",
    "png_16_bit",
    "png_animation",
    "png_corrupt_crc",
    "png_corrupt_deflate",
    "png_dimension_over",
    "png_pixel_over",
    "truncate_half",
    "webp_animation",
    "webp_duplicate_primary",
];
const ALL_PROVENANCE_MUTATIONS: &[&str] = &[
    "jpeg_sof1",
    "jpeg_entropy_truncated",
    "jpeg_huffman_overfull",
    "jpeg_precision_12",
    "jpeg_progressive",
    "jpeg_wrong_restart",
    "none",
    "png_16_bit",
    "png_animation",
    "png_corrupt_crc",
    "png_corrupt_deflate",
    "png_dimension_over",
    "png_pixel_over",
    "truncate_half",
    "webp_animation",
    "webp_duplicate_primary",
];
const VECTOR_CASE_IDS: &[&str] = &[
    "jpeg_baseline_gray_8bit",
    "jpeg_baseline_rgb_8bit",
    "jpeg_baseline_cmyk_8bit",
    "jpeg_extended_sequential_sof1",
    "png_gray_8bit",
    "png_rgb_8bit",
    "png_indexed_8bit",
    "png_gray_alpha_8bit",
    "png_rgba_8bit",
    "png_adam7_rgba_8bit",
    "webp_lossless_8bit",
    "webp_lossy_8bit",
    "webp_lossless_alpha_8bit",
    "webp_lossy_alpha_8bit",
    "jpeg_progressive_forbidden",
    "jpeg_twelve_bit_forbidden",
    "jpeg_entropy_truncated",
    "jpeg_huffman_overfull",
    "jpeg_wrong_restart_marker",
    "png_sixteen_bit_forbidden",
    "png_animation_forbidden",
    "png_corrupt_crc",
    "png_corrupt_deflate",
    "png_dimension_over_limit",
    "png_pixel_limit",
    "png_truncated",
    "webp_animation_forbidden",
    "webp_duplicate_primary",
    "webp_truncated",
    "jpeg_truncated",
];
const CORPUS_SEED_SPECS: &[(&str, &str, &str)] = &[
    (
        "jpeg_baseline_gray_8bit",
        "publication_jpeg",
        "fuzz/corpus/publication_jpeg/jpeg_baseline_gray_8bit.bin",
    ),
    (
        "jpeg_baseline_rgb_8bit",
        "publication_jpeg",
        "fuzz/corpus/publication_jpeg/jpeg_baseline_rgb_8bit.bin",
    ),
    (
        "jpeg_baseline_cmyk_8bit",
        "publication_jpeg",
        "fuzz/corpus/publication_jpeg/jpeg_baseline_cmyk_8bit.bin",
    ),
    (
        "jpeg_extended_sequential_sof1",
        "publication_jpeg",
        "fuzz/corpus/publication_jpeg/jpeg_extended_sequential_sof1.bin",
    ),
    (
        "png_gray_8bit",
        "publication_png",
        "fuzz/corpus/publication_png/png_gray_8bit.bin",
    ),
    (
        "png_rgb_8bit",
        "publication_png",
        "fuzz/corpus/publication_png/png_rgb_8bit.bin",
    ),
    (
        "png_indexed_8bit",
        "publication_png",
        "fuzz/corpus/publication_png/png_indexed_8bit.bin",
    ),
    (
        "png_gray_alpha_8bit",
        "publication_png",
        "fuzz/corpus/publication_png/png_gray_alpha_8bit.bin",
    ),
    (
        "png_rgba_8bit",
        "publication_png",
        "fuzz/corpus/publication_png/png_rgba_8bit.bin",
    ),
    (
        "png_adam7_rgba_8bit",
        "publication_png",
        "fuzz/corpus/publication_png/png_adam7_rgba_8bit.bin",
    ),
    (
        "webp_lossless_8bit",
        "publication_webp",
        "fuzz/corpus/publication_webp/webp_lossless_8bit.bin",
    ),
    (
        "webp_lossy_8bit",
        "publication_webp",
        "fuzz/corpus/publication_webp/webp_lossy_8bit.bin",
    ),
    (
        "webp_lossless_alpha_8bit",
        "publication_webp",
        "fuzz/corpus/publication_webp/webp_lossless_alpha_8bit.bin",
    ),
    (
        "webp_lossy_alpha_8bit",
        "publication_webp",
        "fuzz/corpus/publication_webp/webp_lossy_alpha_8bit.bin",
    ),
    (
        "jpeg_progressive_forbidden",
        "publication_jpeg",
        "fuzz/corpus/publication_jpeg/jpeg_progressive_forbidden.bin",
    ),
    (
        "jpeg_twelve_bit_forbidden",
        "publication_jpeg",
        "fuzz/corpus/publication_jpeg/jpeg_twelve_bit_forbidden.bin",
    ),
    (
        "jpeg_entropy_truncated",
        "publication_jpeg",
        "fuzz/corpus/publication_jpeg/jpeg_entropy_truncated.bin",
    ),
    (
        "jpeg_huffman_overfull",
        "publication_jpeg",
        "fuzz/corpus/publication_jpeg/jpeg_huffman_overfull.bin",
    ),
    (
        "jpeg_wrong_restart_marker",
        "publication_jpeg",
        "fuzz/corpus/publication_jpeg/jpeg_wrong_restart_marker.bin",
    ),
    (
        "png_sixteen_bit_forbidden",
        "publication_png",
        "fuzz/corpus/publication_png/png_sixteen_bit_forbidden.bin",
    ),
    (
        "png_animation_forbidden",
        "publication_png",
        "fuzz/corpus/publication_png/png_animation_forbidden.bin",
    ),
    (
        "png_corrupt_crc",
        "publication_png",
        "fuzz/corpus/publication_png/png_corrupt_crc.bin",
    ),
    (
        "png_corrupt_deflate",
        "publication_png",
        "fuzz/corpus/publication_png/png_corrupt_deflate.bin",
    ),
    (
        "png_dimension_over_limit",
        "publication_png",
        "fuzz/corpus/publication_png/png_dimension_over_limit.bin",
    ),
    (
        "png_pixel_limit",
        "publication_png",
        "fuzz/corpus/publication_png/png_pixel_limit.bin",
    ),
    (
        "png_truncated",
        "publication_png",
        "fuzz/corpus/publication_png/png_truncated.bin",
    ),
    (
        "webp_animation_forbidden",
        "publication_webp",
        "fuzz/corpus/publication_webp/webp_animation_forbidden.bin",
    ),
    (
        "webp_duplicate_primary",
        "publication_webp",
        "fuzz/corpus/publication_webp/webp_duplicate_primary.bin",
    ),
    (
        "webp_truncated",
        "publication_webp",
        "fuzz/corpus/publication_webp/webp_truncated.bin",
    ),
    (
        "jpeg_truncated",
        "publication_jpeg",
        "fuzz/corpus/publication_jpeg/jpeg_truncated.bin",
    ),
];
const RESOURCE_PROBE_SPECS: &[(&str, &str, &str, u64, &str)] = &[
    (
        "jpeg_grayscale",
        "jpeg",
        "sof0_grayscale_8bit",
        60_000_000,
        "imagemagick_grayscale",
    ),
    (
        "jpeg_rgb",
        "jpeg",
        "sof0_rgb_8bit",
        60_000_000,
        "imagemagick_rgb",
    ),
    (
        "jpeg_cmyk",
        "jpeg",
        "sof0_cmyk_8bit",
        60_000_000,
        "imagemagick_cmyk",
    ),
    (
        "jpeg_sof1",
        "jpeg",
        "sof1_extended_sequential_8bit",
        60_000_000,
        "imagemagick_rgb_sof1_marker",
    ),
    (
        "png_rgb",
        "png",
        "rgb_8bit",
        60_000_000,
        "imagemagick_png24",
    ),
    (
        "png_palette",
        "png",
        "palette_8bit",
        60_000_000,
        "imagemagick_palette",
    ),
    (
        "png_rgba",
        "png",
        "rgba_8bit",
        80_000_000,
        "imagemagick_png32",
    ),
    (
        "png_adam7",
        "png",
        "adam7_rgba_8bit",
        80_000_000,
        "imagemagick_adam7",
    ),
    (
        "webp_vp8_rgb",
        "webp",
        "vp8_rgb",
        80_000_000,
        "imagemagick_webp_lossy_rgb",
    ),
    (
        "webp_vp8_alpha",
        "webp",
        "vp8_alpha",
        80_000_000,
        "imagemagick_webp_lossy_alpha",
    ),
    (
        "webp_vp8l_rgb",
        "webp",
        "vp8l_rgb",
        80_000_000,
        "imagemagick_webp_lossless_rgb",
    ),
    (
        "webp_vp8l_alpha",
        "webp",
        "vp8l_alpha",
        80_000_000,
        "imagemagick_webp_lossless_alpha",
    ),
];
const RELEASE_SEMVER_IMPACTS: &[&str] = &[
    "add_conformance_vector",
    "add_enum_variant",
    "change_exported_algorithm_behavior",
    "change_exported_constant_value",
];
const OPERATION_INPUTS: &[&str] = &[
    "ByteVerifiedDescriptor",
    "Bytes",
    "AuthoredRasterDimensions",
    "Bud02UploadObservation",
    "Bud01HeadObservation",
    "Bud01GetObservation",
];
const OPERATION_OUTPUTS: &[&str] = &["PublicationReadinessEvidence"];
const OPERATION_MODULES: &[&str] = &[READINESS_SOURCE_RELATIVE];
const OPERATION_RUST_TYPES: &[&str] = &[
    "radroots_blossom::AuthoredRasterDimensions",
    "radroots_blossom::Bud01GetCollector",
    "radroots_blossom::Bud01GetObservation",
    "radroots_blossom::Bud01HeadObservation",
    "radroots_blossom::Bud02UploadObservation",
    "radroots_blossom::PublicationReadinessEvidence",
];
const SOURCE_SUPERSESSIONS: &[&str] = &[
    "CHANGELOG.md",
    "contracts/releases/1.0.0-alpha.1.toml",
    "crates/blossom/Cargo.toml",
    "crates/blossom/src/error.rs",
    "crates/blossom/src/publication_readiness.rs",
    "crates/blossom/src/publication_readiness/sequential_jpeg.rs",
    "crates/blossom/tests/publication_readiness.rs",
    "crates/blossom/tests/publication_readiness_persistence.rs",
    "tools/xtask/src/contract.rs",
    "tools/xtask/src/main.rs",
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ResourceLimitsDescriptor {
    max_raster_bytes: u64,
    max_decoded_bytes: u64,
    max_dimension: u64,
    max_pixels: u64,
    max_container_records: u64,
    jpeg_max_scans: u64,
    jpeg_max_blocks: u64,
    jpeg_max_coefficient_steps: u64,
    jpeg_entropy_bit_reads_per_byte: u64,
    peak_rss_kib_limit: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DecoderProfileDescriptor {
    accepted_formats: Vec<String>,
    accepted_jpeg_processes: Vec<String>,
    accepted_png_color_types: Vec<u8>,
    normative_error_codes: Vec<String>,
    observed_rejection_error_codes: Vec<String>,
    required_accepted_mutations: Vec<String>,
    required_rejected_mutations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ResourceProbeDescriptor {
    id: String,
    format: String,
    process: String,
    width: u32,
    height: u32,
    max_encoded_bytes: u64,
    max_decoded_bytes: u64,
    executor_test: String,
    fixture_authority: String,
    repetitions: u64,
    peak_rss_kib_limit: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FuzzTargetDescriptor {
    name: String,
    format: String,
    media_type: String,
    extension: String,
    binary: FileDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CorpusSeedDescriptor {
    vector_id: String,
    target: String,
    format: String,
    provenance_mutation: String,
    expected_accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_error: Option<String>,
    file: FileDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FuzzCampaignDescriptor {
    project_manifest: FileDescriptor,
    project_lockfile: FileDescriptor,
    toolchain: FileDescriptor,
    toolchain_channel: String,
    toolchain_components: Vec<String>,
    toolchain_profile: String,
    engine: String,
    sanitizer: String,
    common_harness: FileDescriptor,
    targets: Vec<FuzzTargetDescriptor>,
    corpus_seeds: Vec<CorpusSeedDescriptor>,
    smoke_runs: u64,
    smoke_seed: u64,
    smoke_max_input_bytes: u64,
    smoke_timeout_seconds: u64,
    engine_rss_limit_mb: u64,
    extended_campaign_document: FileDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct NixLanesDescriptor {
    app: String,
    devshell: String,
    stable_check: String,
    fuzz_check: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct OperationDescriptor {
    key: String,
    id: String,
    primary_vector: String,
    primary_case_kinds: Vec<String>,
    security_case_kinds: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ResultVectorDescriptor {
    canonical: FileDescriptor,
    mirror: FileDescriptor,
    executor: FileDescriptor,
    executor_tests: Vec<String>,
    case_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RasterDecoderSecurityManifest {
    schema_version: u32,
    contract_id: String,
    authority_id: String,
    manifest_schema: FileDescriptor,
    predecessors: Vec<PredecessorDescriptor>,
    protocol_sources: Vec<ProtocolSourcePin>,
    resource_limits: ResourceLimitsDescriptor,
    decoder_profile: DecoderProfileDescriptor,
    resource_probes: Vec<ResourceProbeDescriptor>,
    fuzz_campaign: FuzzCampaignDescriptor,
    nix_lanes: NixLanesDescriptor,
    operation: OperationDescriptor,
    release_change_id: String,
    result_vector: ResultVectorDescriptor,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorSuite {
    suite: String,
    contract_version: String,
    vectors: Vec<VectorCase>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorCase {
    id: String,
    kind: String,
    input: VectorInput,
    expected: VectorExpected,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorInput {
    format: String,
    bytes_hex: String,
    mutation: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorExpected {
    accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

struct ValidatedVector {
    bytes: Vec<u8>,
    case_ids: Vec<String>,
    cases: Vec<VectorCase>,
}

pub(crate) fn write_blossom_raster_decoder_security_manifest(
    workspace_root: &Path,
) -> Result<(), String> {
    validate_predecessors(workspace_root)?;
    with_artifact_bundle_transaction(workspace_root, |transaction| {
        transaction.write(expected_artifacts(workspace_root)?)?;
        validate_manifest_under_lock(workspace_root)
    })
}

pub(crate) fn validate_blossom_raster_decoder_security_manifest(
    workspace_root: &Path,
) -> Result<(), String> {
    validate_predecessors(workspace_root)?;
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_manifest_under_lock(workspace_root)
    })
}

fn validate_predecessors(workspace_root: &Path) -> Result<(), String> {
    validate_source_supersessions(workspace_root, SOURCE_SUPERSESSIONS)?;
    validate_immutable_predecessor_with_superseded_artifacts(
        workspace_root,
        "Blossom publication readiness",
        BLOSSOM_PREDECESSOR_ARTIFACTS,
        BLOSSOM_PREDECESSOR_SUPERSEDED_ARTIFACTS,
    )
}

fn validate_manifest_under_lock(workspace_root: &Path) -> Result<(), String> {
    for artifact in expected_artifacts(workspace_root)? {
        let actual = read_regular_file(workspace_root, artifact.relative)?;
        if actual != artifact.contents {
            return Err(format!(
                "generated Blossom raster decoder security contract {} is stale; run {WRITE_COMMAND}",
                artifact.relative
            ));
        }
    }
    let bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest: RasterDecoderSecurityManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_RELATIVE, &bytes, &manifest)?;
    let schema_bytes = read_regular_file(workspace_root, MANIFEST_SCHEMA_RELATIVE)?;
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("parse {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_SCHEMA_RELATIVE, &schema_bytes, &schema)?;
    validate_json_schema(
        &schema,
        &serde_json::to_value(&manifest)
            .map_err(|error| format!("serialize {MANIFEST_RELATIVE}: {error}"))?,
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
    validate_source_contract(workspace_root)?;
    let vector = validate_vector(workspace_root)?;
    let manifest_schema_bytes = canonical_json_bytes(&manifest_schema())?;
    let manifest = describe_manifest(workspace_root, &manifest_schema_bytes)?;
    let manifest_bytes = canonical_json_bytes(&manifest)?;
    let sidecar_bytes = format!("{}\n", sha256_hex(&manifest_bytes)).into_bytes();
    let descriptor_bytes = canonical_json_bytes(&json!({
        "schema_version": SCHEMA_VERSION,
        "contract_id": CONTRACT_ID,
        "manifest": descriptor_for_bytes(MANIFEST_RELATIVE, &manifest_bytes),
        "manifest_schema": descriptor_for_bytes(MANIFEST_SCHEMA_RELATIVE, &manifest_schema_bytes),
        "manifest_sidecar": descriptor_for_bytes(MANIFEST_SHA256_RELATIVE, &sidecar_bytes),
        "predecessor_contract_ids": ["radroots_blossom.publication_readiness_v1"]
    }))?;
    let mut artifacts = vec![
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
            relative: VECTOR_MIRROR_RELATIVE,
            contents: vector.bytes.clone(),
        },
    ];
    artifacts.extend(
        expected_seeds(&vector)?
            .into_iter()
            .map(|seed| GeneratedArtifact {
                relative: seed.relative,
                contents: seed.bytes,
            }),
    );
    Ok(artifacts)
}

fn describe_manifest(
    workspace_root: &Path,
    manifest_schema_bytes: &[u8],
) -> Result<RasterDecoderSecurityManifest, String> {
    let vector = validate_vector(workspace_root)?;
    Ok(RasterDecoderSecurityManifest {
        schema_version: SCHEMA_VERSION,
        contract_id: CONTRACT_ID.to_owned(),
        authority_id: AUTHORITY_ID.to_owned(),
        manifest_schema: descriptor_for_bytes(MANIFEST_SCHEMA_RELATIVE, manifest_schema_bytes),
        predecessors: vec![predecessor_descriptor(
            workspace_root,
            "radroots_blossom.publication_readiness_v1",
            BLOSSOM_PREDECESSOR_ARTIFACTS,
            BLOSSOM_PREDECESSOR_SUPERSEDED_ARTIFACTS,
            SOURCE_SUPERSESSIONS,
        )?],
        protocol_sources: vec![
            ProtocolSourcePin {
                id: "nostr_nips".to_owned(),
                repository: "https://github.com/nostr-protocol/nips".to_owned(),
                revision: "bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91".to_owned(),
            },
            ProtocolSourcePin {
                id: "blossom".to_owned(),
                repository: "https://github.com/hzrd149/blossom".to_owned(),
                revision: "b5bd2801d1763aa635fc8fea7a76597e0eb18990".to_owned(),
            },
        ],
        resource_limits: ResourceLimitsDescriptor {
            max_raster_bytes: MAX_RASTER_BYTES,
            max_decoded_bytes: MAX_DECODED_BYTES,
            max_dimension: MAX_DIMENSION,
            max_pixels: MAX_PIXELS,
            max_container_records: MAX_CONTAINER_RECORDS,
            jpeg_max_scans: JPEG_MAX_SCANS,
            jpeg_max_blocks: JPEG_MAX_BLOCKS,
            jpeg_max_coefficient_steps: JPEG_MAX_COEFFICIENT_STEPS,
            jpeg_entropy_bit_reads_per_byte: JPEG_ENTROPY_BIT_READS_PER_BYTE,
            peak_rss_kib_limit: PEAK_RSS_KIB_LIMIT,
        },
        decoder_profile: DecoderProfileDescriptor {
            accepted_formats: owned(&["image/jpeg", "image/png", "image/webp"]),
            accepted_jpeg_processes: owned(&[
                "sof0_baseline_8bit",
                "sof1_extended_sequential_8bit",
            ]),
            accepted_png_color_types: vec![0, 2, 3, 4, 6],
            normative_error_codes: owned(NORMATIVE_ERROR_CODES),
            observed_rejection_error_codes: owned(OBSERVED_REJECTION_ERROR_CODES),
            required_accepted_mutations: owned(REQUIRED_ACCEPTED_MUTATIONS),
            required_rejected_mutations: owned(REQUIRED_REJECTED_MUTATIONS),
        },
        resource_probes: RESOURCE_PROBE_SPECS
            .iter()
            .map(|(id, format, process, decoded_bytes, fixture_authority)| {
                ResourceProbeDescriptor {
                    id: (*id).to_owned(),
                    format: (*format).to_owned(),
                    process: (*process).to_owned(),
                    width: 5_000,
                    height: 4_000,
                    max_encoded_bytes: MAX_RASTER_BYTES,
                    max_decoded_bytes: *decoded_bytes,
                    executor_test: RESOURCE_TEST.to_owned(),
                    fixture_authority: (*fixture_authority).to_owned(),
                    repetitions: 3,
                    peak_rss_kib_limit: PEAK_RSS_KIB_LIMIT,
                }
            })
            .collect(),
        fuzz_campaign: FuzzCampaignDescriptor {
            project_manifest: descriptor_for_file(workspace_root, FUZZ_MANIFEST_RELATIVE)?,
            project_lockfile: descriptor_for_file(workspace_root, FUZZ_LOCKFILE_RELATIVE)?,
            toolchain: descriptor_for_file(workspace_root, FUZZ_TOOLCHAIN_RELATIVE)?,
            toolchain_channel: FUZZ_TOOLCHAIN_CHANNEL.to_owned(),
            toolchain_components: owned(&["rust-src"]),
            toolchain_profile: "minimal".to_owned(),
            engine: "libfuzzer".to_owned(),
            sanitizer: "address".to_owned(),
            common_harness: descriptor_for_file(workspace_root, FUZZ_COMMON_RELATIVE)?,
            targets: FUZZ_TARGET_SPECS
                .iter()
                .map(|(name, format, media_type, extension)| {
                    Ok(FuzzTargetDescriptor {
                        name: (*name).to_owned(),
                        format: (*format).to_owned(),
                        media_type: (*media_type).to_owned(),
                        extension: (*extension).to_owned(),
                        binary: descriptor_for_file(
                            workspace_root,
                            &format!("fuzz/fuzz_targets/{name}.rs"),
                        )?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
            corpus_seeds: expected_seeds(&vector)?
                .into_iter()
                .map(|seed| CorpusSeedDescriptor {
                    vector_id: seed.case.id.clone(),
                    target: seed.target.to_owned(),
                    format: seed.case.input.format.clone(),
                    provenance_mutation: seed.case.input.mutation.clone(),
                    expected_accepted: seed.case.expected.accepted,
                    expected_error: seed.case.expected.error.clone(),
                    file: descriptor_for_bytes(seed.relative, &seed.bytes),
                })
                .collect(),
            smoke_runs: FUZZ_SMOKE_RUNS,
            smoke_seed: FUZZ_SMOKE_SEED,
            smoke_max_input_bytes: FUZZ_SMOKE_MAX_INPUT_BYTES,
            smoke_timeout_seconds: FUZZ_SMOKE_TIMEOUT_SECONDS,
            engine_rss_limit_mb: FUZZ_ENGINE_RSS_LIMIT_MB,
            extended_campaign_document: descriptor_for_file(
                workspace_root,
                SECURITY_DOCUMENT_RELATIVE,
            )?,
        },
        nix_lanes: NixLanesDescriptor {
            app: "decoder-security".to_owned(),
            devshell: "decoder-security".to_owned(),
            stable_check: "blossom-raster-decode-test".to_owned(),
            fuzz_check: "blossom-decoder-fuzz-smoke".to_owned(),
        },
        operation: OperationDescriptor {
            key: OPERATION_KEY.to_owned(),
            id: OPERATION_ID.to_owned(),
            primary_vector: PRIMARY_OPERATION_VECTOR.to_owned(),
            primary_case_kinds: owned(PRIMARY_OPERATION_CASE_KINDS),
            security_case_kinds: owned(&[ACCEPTED_KIND, REJECTED_KIND]),
        },
        release_change_id: RELEASE_CHANGE_ID.to_owned(),
        result_vector: ResultVectorDescriptor {
            canonical: descriptor_for_bytes(VECTOR_RELATIVE, &vector.bytes),
            mirror: descriptor_for_bytes(VECTOR_MIRROR_RELATIVE, &vector.bytes),
            executor: descriptor_for_file(workspace_root, VECTOR_EXECUTOR_RELATIVE)?,
            executor_tests: owned(&[REGRESSION_TEST, DIFFERENTIAL_TEST, RESOURCE_TEST]),
            case_ids: vector.case_ids,
        },
    })
}

struct ExpectedSeed<'a> {
    relative: &'static str,
    target: &'static str,
    case: &'a VectorCase,
    bytes: Vec<u8>,
}

fn expected_seeds(vector: &ValidatedVector) -> Result<Vec<ExpectedSeed<'_>>, String> {
    if CORPUS_SEED_SPECS.len() != vector.cases.len() {
        return Err("fuzz seed specification count must match the vector count".to_owned());
    }
    CORPUS_SEED_SPECS
        .iter()
        .zip(&vector.cases)
        .map(|((case_id, target, relative), case)| {
            let expected_target = FUZZ_TARGET_SPECS
                .iter()
                .find_map(|(candidate, format, _, _)| {
                    (*format == case.input.format).then_some(*candidate)
                })
                .ok_or_else(|| {
                    format!(
                        "{} has unsupported fuzz seed format {}",
                        case.id, case.input.format
                    )
                })?;
            if *case_id != case.id || *target != expected_target {
                return Err(format!(
                    "fuzz seed specification for {} has the wrong identity or target",
                    case.id
                ));
            }
            let bytes = hex::decode(&case.input.bytes_hex)
                .map_err(|error| format!("decode vector {} bytes_hex: {error}", case.id))?;
            if bytes.is_empty() {
                return Err(format!(
                    "vector {} cannot produce an empty fuzz seed",
                    case.id
                ));
            }
            Ok(ExpectedSeed {
                relative,
                target,
                case,
                bytes,
            })
        })
        .collect()
}

fn pat_ident_and_type(argument: &FnArg) -> Option<(String, &Type)> {
    let FnArg::Typed(argument) = argument else {
        return None;
    };
    let Pat::Ident(ident) = argument.pat.as_ref() else {
        return None;
    };
    Some((ident.ident.to_string(), argument.ty.as_ref()))
}

fn type_is_reference_to_str(ty: &Type) -> bool {
    let Type::Reference(reference) = ty else {
        return false;
    };
    matches!(
        reference.elem.as_ref(),
        Type::Path(path) if path.path.is_ident("str")
    )
}

fn type_is_reference_to_u8_slice(ty: &Type) -> bool {
    let Type::Reference(reference) = ty else {
        return false;
    };
    let Type::Slice(slice) = reference.elem.as_ref() else {
        return false;
    };
    matches!(
        slice.elem.as_ref(),
        Type::Path(path) if path.path.is_ident("u8")
    )
}

fn expression_is_ident(expression: &Expr, expected: &str) -> bool {
    matches!(
        expression,
        Expr::Path(path) if path.path.is_ident(expected)
    )
}

fn expression_string_literal(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Lit(literal) => match &literal.lit {
            Lit::Str(value) => Some(value.value()),
            _ => None,
        },
        _ => None,
    }
}

fn use_tree_imports_name(tree: &UseTree, expected: &str) -> bool {
    match tree {
        UseTree::Name(name) => name.ident == expected,
        UseTree::Path(path) => use_tree_imports_name(&path.tree, expected),
        UseTree::Group(group) => group
            .items
            .iter()
            .any(|item| use_tree_imports_name(item, expected)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn validate_fuzz_common_harness(workspace_root: &Path) -> Result<(), String> {
    let bytes = read_regular_file(workspace_root, FUZZ_COMMON_RELATIVE)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{FUZZ_COMMON_RELATIVE} must be UTF-8: {error}"))?;
    validate_fuzz_common_source(source)
}

fn validate_fuzz_common_source(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|error| format!("parse {FUZZ_COMMON_RELATIVE}: {error}"))?;

    let operation_imports = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Use(item) => Some(item),
            _ => None,
        })
        .filter(|item| {
            matches!(
                &item.tree,
                UseTree::Path(path)
                    if path.ident == "radroots_blossom"
                        && use_tree_imports_name(&path.tree, "verify_publication_readiness")
            )
        })
        .count();
    if operation_imports != 1 {
        return Err(format!(
            "{FUZZ_COMMON_RELATIVE} must import verify_publication_readiness from radroots_blossom exactly once"
        ));
    }
    if file.items.iter().any(|item| {
        matches!(
            item,
            Item::Fn(function) if function.sig.ident == "verify_publication_readiness"
        )
    }) {
        return Err(format!(
            "{FUZZ_COMMON_RELATIVE} must not shadow verify_publication_readiness"
        ));
    }

    let exercises = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) if function.sig.ident == "exercise" => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    if exercises.len() != 1 {
        return Err(format!(
            "{FUZZ_COMMON_RELATIVE} must define the common exercise harness exactly once"
        ));
    }
    let exercise = exercises[0];
    let arguments = exercise.sig.inputs.iter().collect::<Vec<_>>();
    let signature_matches = arguments.len() == 3
        && pat_ident_and_type(arguments[0])
            .is_some_and(|(name, ty)| name == "input" && type_is_reference_to_u8_slice(ty))
        && pat_ident_and_type(arguments[1])
            .is_some_and(|(name, ty)| name == "media_type" && type_is_reference_to_str(ty))
        && pat_ident_and_type(arguments[2])
            .is_some_and(|(name, ty)| name == "extension" && type_is_reference_to_str(ty))
        && exercise.sig.constness.is_none()
        && exercise.sig.asyncness.is_none()
        && exercise.sig.unsafety.is_none()
        && exercise.sig.abi.is_none()
        && exercise.sig.generics.params.is_empty()
        && matches!(exercise.sig.output, syn::ReturnType::Default);
    if !signature_matches {
        return Err(format!(
            "{FUZZ_COMMON_RELATIVE}::exercise must retain the governed raw-byte harness signature"
        ));
    }

    #[derive(Default)]
    struct ReadinessCalls {
        total: usize,
        passes_input: bool,
    }

    impl<'ast> Visit<'ast> for ReadinessCalls {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if let Expr::Path(path) = call.func.as_ref()
                && path.path.is_ident("verify_publication_readiness")
            {
                self.total += 1;
                self.passes_input = call
                    .args
                    .iter()
                    .nth(1)
                    .is_some_and(|argument| expression_is_ident(argument, "input"));
            }
            syn::visit::visit_expr_call(self, call);
        }
    }

    let mut readiness_calls = ReadinessCalls::default();
    readiness_calls.visit_block(&exercise.block);
    if readiness_calls.total != 1 || !readiness_calls.passes_input {
        return Err(format!(
            "{FUZZ_COMMON_RELATIVE}::exercise must pass its raw input to exactly one verify_publication_readiness call"
        ));
    }
    Ok(())
}

fn validate_fuzz_target(
    workspace_root: &Path,
    target: &str,
    expected_media_type: &str,
    expected_extension: &str,
) -> Result<(), String> {
    let relative = format!("fuzz/fuzz_targets/{target}.rs");
    let bytes = read_regular_file(workspace_root, &relative)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{relative} must be UTF-8: {error}"))?;
    validate_fuzz_target_source(&relative, source, expected_media_type, expected_extension)
}

fn validate_fuzz_target_source(
    relative: &str,
    source: &str,
    expected_media_type: &str,
    expected_extension: &str,
) -> Result<(), String> {
    let file = syn::parse_file(source).map_err(|error| format!("parse {relative}: {error}"))?;

    struct FuzzTargetMacros<'ast>(Vec<&'ast syn::Macro>);

    impl<'ast> Visit<'ast> for FuzzTargetMacros<'ast> {
        fn visit_macro(&mut self, item: &'ast syn::Macro) {
            if item.path.is_ident("fuzz_target") {
                self.0.push(item);
            }
            syn::visit::visit_macro(self, item);
        }
    }

    let mut macros = FuzzTargetMacros(Vec::new());
    macros.visit_file(&file);
    if macros.0.len() != 1 {
        return Err(format!(
            "{relative} must declare exactly one libFuzzer target"
        ));
    }
    let closure: syn::ExprClosure = syn::parse2(macros.0[0].tokens.clone())
        .map_err(|error| format!("parse {relative} fuzz_target closure: {error}"))?;
    if closure.inputs.len() != 1
        || closure.asyncness.is_some()
        || closure.movability.is_some()
        || closure.capture.is_some()
        || !matches!(closure.output, syn::ReturnType::Default)
    {
        return Err(format!(
            "{relative} must accept exactly one borrowed libFuzzer byte slice"
        ));
    }
    let input = closure.inputs.first().expect("length checked");
    let Pat::Type(input) = input else {
        return Err(format!("{relative} fuzz input must be explicitly typed"));
    };
    let Pat::Ident(input_ident) = input.pat.as_ref() else {
        return Err(format!("{relative} fuzz input must be a simple identifier"));
    };
    if !type_is_reference_to_u8_slice(&input.ty) {
        return Err(format!("{relative} fuzz input must have type &[u8]"));
    }

    let Expr::Block(body) = closure.body.as_ref() else {
        return Err(format!(
            "{relative} fuzz body must call the governed common harness"
        ));
    };
    let [Stmt::Expr(Expr::Call(call), _)] = body.block.stmts.as_slice() else {
        return Err(format!(
            "{relative} fuzz body must contain exactly one common harness call"
        ));
    };
    let Expr::Path(function) = call.func.as_ref() else {
        return Err(format!("{relative} must call common::exercise directly"));
    };
    let function_segments = function
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let arguments = call.args.iter().collect::<Vec<_>>();
    if function_segments != ["common", "exercise"]
        || arguments.len() != 3
        || !expression_is_ident(arguments[0], &input_ident.ident.to_string())
        || expression_string_literal(arguments[1]).as_deref() != Some(expected_media_type)
        || expression_string_literal(arguments[2]).as_deref() != Some(expected_extension)
    {
        return Err(format!(
            "{relative} must pass its libFuzzer bytes and the governed MIME/extension to common::exercise"
        ));
    }
    Ok(())
}

fn validate_fuzz_corpus(workspace_root: &Path) -> Result<(), String> {
    let mut expected = BTreeMap::new();
    let vector = validate_vector(workspace_root)?;
    for seed in expected_seeds(&vector)? {
        if expected
            .insert(seed.relative.to_owned(), seed.bytes)
            .is_some()
        {
            return Err(format!(
                "duplicate fuzz seed authority for {}",
                seed.relative
            ));
        }
    }
    if expected.len() != 30 {
        return Err(format!(
            "fuzz corpus must contain exactly 30 governed seeds, found {}",
            expected.len()
        ));
    }

    let corpus_root = workspace_root.join(FUZZ_CORPUS_RELATIVE);
    let root_entries = fs::read_dir(&corpus_root)
        .map_err(|error| format!("read {FUZZ_CORPUS_RELATIVE}: {error}"))?;
    let expected_targets = FUZZ_TARGETS
        .iter()
        .map(|target| (*target).to_owned())
        .collect::<BTreeSet<_>>();
    let mut actual_targets = BTreeSet::new();
    let mut actual_files = BTreeSet::new();
    for target_entry in root_entries {
        let target_entry = target_entry
            .map_err(|error| format!("read entry under {FUZZ_CORPUS_RELATIVE}: {error}"))?;
        let target_name = target_entry
            .file_name()
            .into_string()
            .map_err(|_| "fuzz corpus target directories must be UTF-8".to_owned())?;
        let target_type = target_entry
            .file_type()
            .map_err(|error| format!("inspect fuzz corpus target {target_name}: {error}"))?;
        if !target_type.is_dir() || !expected_targets.contains(target_name.as_str()) {
            return Err(format!(
                "unexpected fuzz corpus target entry {FUZZ_CORPUS_RELATIVE}/{target_name}"
            ));
        }
        if !actual_targets.insert(target_name.clone()) {
            return Err(format!(
                "duplicate fuzz corpus target directory {target_name}"
            ));
        }
        for seed_entry in fs::read_dir(target_entry.path())
            .map_err(|error| format!("read fuzz corpus target {target_name}: {error}"))?
        {
            let seed_entry = seed_entry
                .map_err(|error| format!("read fuzz corpus seed under {target_name}: {error}"))?;
            let seed_name = seed_entry
                .file_name()
                .into_string()
                .map_err(|_| "fuzz corpus seed names must be UTF-8".to_owned())?;
            let seed_type = seed_entry.file_type().map_err(|error| {
                format!("inspect fuzz corpus seed {target_name}/{seed_name}: {error}")
            })?;
            if !seed_type.is_file() {
                return Err(format!(
                    "fuzz corpus seed {target_name}/{seed_name} must be a regular file"
                ));
            }
            let relative = format!("{FUZZ_CORPUS_RELATIVE}/{target_name}/{seed_name}");
            if !actual_files.insert(relative.clone()) {
                return Err(format!("duplicate fuzz corpus seed {relative}"));
            }
            let expected_bytes = expected
                .get(&relative)
                .ok_or_else(|| format!("unexpected or retargeted fuzz corpus seed {relative}"))?;
            let actual_bytes = read_regular_file(workspace_root, &relative)?;
            if &actual_bytes != expected_bytes {
                return Err(format!(
                    "fuzz corpus seed {relative} differs from its exact vector bytes"
                ));
            }
        }
    }
    if actual_targets != expected_targets || actual_files != expected.keys().cloned().collect() {
        return Err("fuzz corpus is missing a governed target or exact vector seed".to_owned());
    }
    Ok(())
}

fn validate_source_contract(workspace_root: &Path) -> Result<(), String> {
    validate_resource_limits(workspace_root)?;
    validate_error_authority(workspace_root)?;
    validate_vector_executor(workspace_root)?;
    validate_fuzz_authority(workspace_root, false)?;
    validate_operations_authority(workspace_root)?;
    validate_release_authority(workspace_root)?;
    validate_vector(workspace_root)?;
    Ok(())
}

fn collect_const_values(
    source_relative: &str,
    source: &str,
) -> Result<BTreeMap<String, u64>, String> {
    let file =
        syn::parse_file(source).map_err(|error| format!("parse {source_relative}: {error}"))?;
    let mut values = BTreeMap::new();
    for _ in 0..4 {
        for item in &file.items {
            if let Item::Const(item) = item {
                let name = item.ident.to_string();
                if values.contains_key(&name) {
                    continue;
                }
                if let Some(value) = evaluate_u64_expr(&item.expr, &values) {
                    values.insert(name, value);
                }
            }
        }
    }
    Ok(values)
}

fn evaluate_u64_expr(expr: &Expr, known: &BTreeMap<String, u64>) -> Option<u64> {
    match expr {
        Expr::Lit(lit) => match &lit.lit {
            Lit::Int(value) => value.base10_digits().parse().ok(),
            _ => None,
        },
        Expr::Path(path) => path
            .path
            .get_ident()
            .and_then(|ident| known.get(&ident.to_string()).copied()),
        Expr::Binary(binary) => match binary.op {
            BinOp::Mul(_) => evaluate_u64_expr(&binary.left, known)?
                .checked_mul(evaluate_u64_expr(&binary.right, known)?),
            _ => None,
        },
        Expr::Paren(paren) => evaluate_u64_expr(&paren.expr, known),
        Expr::Group(group) => evaluate_u64_expr(&group.expr, known),
        _ => None,
    }
}

fn validate_limit_constants(
    workspace_root: &Path,
    source_relative: &str,
    expected: &[(&str, u64)],
) -> Result<(), String> {
    let bytes = read_regular_file(workspace_root, source_relative)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{source_relative} must be UTF-8: {error}"))?;
    let values = collect_const_values(source_relative, source)?;
    for (name, expected_value) in expected {
        match values.get(*name) {
            Some(actual) if actual == expected_value => {}
            Some(actual) => {
                return Err(format!(
                    "{source_relative} constant {name} drifted: expected {expected_value}, found {actual}"
                ));
            }
            None => {
                return Err(format!(
                    "{source_relative} must define an evaluatable u64 constant {name}"
                ));
            }
        }
    }
    Ok(())
}

fn validate_resource_limits(workspace_root: &Path) -> Result<(), String> {
    validate_limit_constants(
        workspace_root,
        READINESS_SOURCE_RELATIVE,
        READINESS_LIMIT_CONSTANTS,
    )?;
    validate_limit_constants(workspace_root, JPEG_SOURCE_RELATIVE, JPEG_LIMIT_CONSTANTS)
}

fn validate_error_authority(workspace_root: &Path) -> Result<(), String> {
    let bytes = read_regular_file(workspace_root, ERROR_SOURCE_RELATIVE)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{ERROR_SOURCE_RELATIVE} must be UTF-8: {error}"))?;
    let file = syn::parse_file(source)
        .map_err(|error| format!("parse {ERROR_SOURCE_RELATIVE}: {error}"))?;
    let mut variants = BTreeSet::new();
    for item in &file.items {
        if let Item::Enum(item) = item
            && item.ident == "Error"
            && is_public(&item.vis)
        {
            for variant in &item.variants {
                variants.insert(variant.ident.to_string());
            }
        }
    }
    let boundary_variants = variants
        .into_iter()
        .filter(|variant| {
            variant.starts_with("PublicationRaster")
                || variant == "PublicationJpegProcessForbidden"
                || variant == "UnsupportedPublicationRasterMediaType"
                || variant == "InvalidPublicationRaster"
        })
        .collect::<BTreeSet<_>>();
    if boundary_variants
        != NORMATIVE_ERROR_VARIANTS
            .iter()
            .map(|variant| (*variant).to_owned())
            .collect()
    {
        return Err(format!(
            "{ERROR_SOURCE_RELATIVE} raster decoder error variant authority drifted"
        ));
    }

    #[derive(Default)]
    struct StringLiterals(BTreeSet<String>);

    impl<'ast> Visit<'ast> for StringLiterals {
        fn visit_expr(&mut self, expression: &'ast Expr) {
            if let Expr::Lit(lit) = expression
                && let Lit::Str(value) = &lit.lit
            {
                self.0.insert(value.value());
            }
            syn::visit::visit_expr(self, expression);
        }
    }

    let mut literals = StringLiterals::default();
    literals.visit_file(&file);
    let boundary_codes = literals
        .0
        .into_iter()
        .filter(|code| {
            code.starts_with("publication_raster_")
                || code.starts_with("publication_jpeg_")
                || code == "unsupported_publication_raster_media_type"
                || code == "invalid_publication_raster"
        })
        .collect::<BTreeSet<_>>();
    if boundary_codes
        != NORMATIVE_ERROR_CODES
            .iter()
            .map(|code| (*code).to_owned())
            .collect()
    {
        return Err(format!(
            "{ERROR_SOURCE_RELATIVE} raster decoder error code authority drifted"
        ));
    }
    Ok(())
}

fn validate_vector_executor(workspace_root: &Path) -> Result<(), String> {
    let bytes = read_regular_file(workspace_root, VECTOR_EXECUTOR_RELATIVE)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{VECTOR_EXECUTOR_RELATIVE} must be UTF-8: {error}"))?;
    let file = syn::parse_file(source)
        .map_err(|error| format!("parse {VECTOR_EXECUTOR_RELATIVE}: {error}"))?;
    let cfg_gates = file
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg"))
        .count();
    if cfg_gates != 1 {
        return Err(format!(
            "{VECTOR_EXECUTOR_RELATIVE} must be gated by exactly one crate-level cfg attribute"
        ));
    }
    for (test, ignored) in [
        (REGRESSION_TEST, false),
        (DIFFERENTIAL_TEST, true),
        (RESOURCE_TEST, true),
    ] {
        let tests = file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fn(function) if function.sig.ident == test => Some(function),
                _ => None,
            })
            .collect::<Vec<_>>();
        if tests.len() != 1 {
            return Err(format!(
                "{VECTOR_EXECUTOR_RELATIVE} must define {test} exactly once"
            ));
        }
        let function = tests[0];
        let has_test = function
            .attrs
            .iter()
            .any(|attribute| attribute.path().is_ident("test"));
        let has_ignore = function
            .attrs
            .iter()
            .any(|attribute| attribute.path().is_ident("ignore"));
        if !has_test
            || has_ignore != ignored
            || function.sig.constness.is_some()
            || function.sig.asyncness.is_some()
            || function.sig.unsafety.is_some()
            || function.sig.abi.is_some()
            || !function.sig.inputs.is_empty()
            || !matches!(function.sig.output, syn::ReturnType::Default)
        {
            return Err(format!(
                "{VECTOR_EXECUTOR_RELATIVE}::{test} has an invalid test signature or gating"
            ));
        }
    }

    #[derive(Default)]
    struct OperationCalls(BTreeSet<String>);

    impl<'ast> Visit<'ast> for OperationCalls {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if let Expr::Path(path) = call.func.as_ref()
                && let Some(segment) = path.path.segments.last()
            {
                self.0.insert(segment.ident.to_string());
            }
            syn::visit::visit_expr_call(self, call);
        }

        fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
            self.0.insert(call.method.to_string());
            syn::visit::visit_expr_method_call(self, call);
        }

        fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
            self.0.insert(function.sig.ident.to_string());
            syn::visit::visit_item_fn(self, function);
        }
    }

    let mut calls = OperationCalls::default();
    calls.visit_file(&file);
    if !calls.0.contains("verify_publication_readiness") {
        return Err(format!(
            "{VECTOR_EXECUTOR_RELATIVE} must execute the public verify_publication_readiness operation"
        ));
    }
    validate_resource_probe_executor(&file)?;
    Ok(())
}

fn validate_resource_probe_executor(file: &syn::File) -> Result<(), String> {
    let implementations = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item)
                if matches!(
                    item.self_ty.as_ref(),
                    Type::Path(path) if path.path.is_ident("ResourceProbeCase")
                ) =>
            {
                Some(item)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if implementations.len() != 1 {
        return Err(format!(
            "{VECTOR_EXECUTOR_RELATIVE} must define exactly one ResourceProbeCase implementation"
        ));
    }
    let implementation = implementations[0];
    let all = implementation
        .items
        .iter()
        .filter_map(|item| match item {
            syn::ImplItem::Const(item) if item.ident == "ALL" => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let ids = implementation
        .items
        .iter()
        .filter_map(|item| match item {
            syn::ImplItem::Fn(item) if item.sig.ident == "id" => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    if all.len() != 1 || ids.len() != 1 {
        return Err(format!(
            "{VECTOR_EXECUTOR_RELATIVE} must define one closed ResourceProbeCase::ALL and id mapping"
        ));
    }
    let Type::Array(all_type) = &all[0].ty else {
        return Err("ResourceProbeCase::ALL must use an explicit array type".to_owned());
    };
    let Expr::Lit(length) = &all_type.len else {
        return Err("ResourceProbeCase::ALL must use a literal length".to_owned());
    };
    let Lit::Int(length) = &length.lit else {
        return Err("ResourceProbeCase::ALL must use an integer length".to_owned());
    };
    let Expr::Array(elements) = &all[0].expr else {
        return Err("ResourceProbeCase::ALL must use an explicit array value".to_owned());
    };
    if length.base10_parse::<usize>().ok() != Some(RESOURCE_PROBE_SPECS.len())
        || elements.elems.len() != RESOURCE_PROBE_SPECS.len()
    {
        return Err(format!(
            "{VECTOR_EXECUTOR_RELATIVE} ResourceProbeCase::ALL must contain exactly {} cases",
            RESOURCE_PROBE_SPECS.len()
        ));
    }

    #[derive(Default)]
    struct StringLiterals(BTreeSet<String>);

    impl<'ast> Visit<'ast> for StringLiterals {
        fn visit_lit_str(&mut self, value: &'ast syn::LitStr) {
            self.0.insert(value.value());
        }
    }

    let mut literals = StringLiterals::default();
    literals.visit_impl_item_fn(ids[0]);
    let expected = RESOURCE_PROBE_SPECS
        .iter()
        .map(|(id, _, _, _, _)| (*id).to_owned())
        .collect::<BTreeSet<_>>();
    if literals.0 != expected {
        return Err(format!(
            "{VECTOR_EXECUTOR_RELATIVE} ResourceProbeCase::id inventory drifted"
        ));
    }
    Ok(())
}

fn insert_fuzz_bin_name(bin_names: &mut BTreeSet<String>, name: &str) -> Result<(), String> {
    if !bin_names.insert(name.to_owned()) {
        return Err(format!("fuzz bin target {name} is declared more than once"));
    }
    Ok(())
}

fn validate_fuzz_crate_version(label: &str, package: &toml::Table) -> Result<(), String> {
    if package.get("version").and_then(toml::Value::as_str) != Some(LIB_CRATE_VERSION) {
        return Err(format!(
            "{label} version must remain exactly {LIB_CRATE_VERSION}"
        ));
    }
    Ok(())
}

fn validate_fuzz_authority(workspace_root: &Path, validate_corpus: bool) -> Result<(), String> {
    let workspace = parse_toml(workspace_root, WORKSPACE_MANIFEST_RELATIVE)?;
    let excludes = toml_string_array(
        "workspace exclude",
        workspace
            .get("workspace")
            .and_then(|value| value.get("exclude")),
    )?;
    if excludes
        .iter()
        .filter(|value| value.as_str() == "fuzz")
        .count()
        != 1
    {
        return Err("workspace manifest must exclude the fuzz project exactly once".to_owned());
    }

    let fuzz = parse_toml(workspace_root, FUZZ_MANIFEST_RELATIVE)?;
    let package = fuzz
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "fuzz package table is missing".to_owned())?;
    validate_fuzz_crate_version("fuzz package", package)?;
    if package.get("name").and_then(toml::Value::as_str) != Some("radroots-fuzz")
        || package.get("publish").and_then(toml::Value::as_bool) != Some(false)
        || package
            .get("metadata")
            .and_then(|value| value.get("cargo-fuzz"))
            .and_then(toml::Value::as_bool)
            != Some(true)
    {
        return Err(
            "fuzz package must be the non-publishable radroots-fuzz cargo-fuzz project".to_owned(),
        );
    }
    let dependencies = fuzz
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "fuzz dependencies table is missing".to_owned())?;
    if dependencies.contains_key("hex") {
        return Err("fuzz project must not decode textual hex corpus wrappers".to_owned());
    }
    for required in ["libfuzzer-sys"] {
        if !dependencies.contains_key(required) {
            return Err(format!("fuzz dependency {required} is missing"));
        }
    }
    let blossom = dependencies
        .get("radroots_blossom")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "fuzz radroots_blossom dependency is missing".to_owned())?;
    let blossom_features =
        toml_string_array("fuzz radroots_blossom features", blossom.get("features"))?;
    if blossom.get("path").and_then(toml::Value::as_str) != Some("../crates/blossom")
        || blossom
            .get("default-features")
            .and_then(toml::Value::as_bool)
            != Some(false)
        || blossom_features.into_iter().collect::<BTreeSet<_>>()
            != ["raster-decode", "serde"]
                .into_iter()
                .map(str::to_owned)
                .collect()
    {
        return Err("fuzz radroots_blossom dependency profile drifted".to_owned());
    }
    let bins = fuzz
        .get("bin")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "fuzz bin targets are missing".to_owned())?;
    let mut bin_names = BTreeSet::new();
    for bin in bins {
        let bin = bin
            .as_table()
            .ok_or_else(|| "fuzz bin targets must be tables".to_owned())?;
        let name = bin
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| "fuzz bin target names must be strings".to_owned())?;
        let path = bin
            .get("path")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| "fuzz bin target paths must be strings".to_owned())?;
        if path != format!("fuzz_targets/{name}.rs")
            || bin.get("test").and_then(toml::Value::as_bool) != Some(false)
            || bin.get("doc").and_then(toml::Value::as_bool) != Some(false)
            || bin.get("bench").and_then(toml::Value::as_bool) != Some(false)
        {
            return Err(format!("fuzz bin target {name} drifted"));
        }
        insert_fuzz_bin_name(&mut bin_names, name)?;
    }
    if bin_names
        != FUZZ_TARGETS
            .iter()
            .map(|value| (*value).to_owned())
            .collect()
    {
        return Err(format!(
            "fuzz targets drifted: expected {FUZZ_TARGETS:?}, found {bin_names:?}"
        ));
    }

    let toolchain = parse_toml(workspace_root, FUZZ_TOOLCHAIN_RELATIVE)?;
    let toolchain = toolchain
        .get("toolchain")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "fuzz toolchain table is missing".to_owned())?;
    let components = toml_string_array("fuzz toolchain components", toolchain.get("components"))?;
    if toolchain.get("channel").and_then(toml::Value::as_str) != Some(FUZZ_TOOLCHAIN_CHANNEL)
        || toolchain.get("profile").and_then(toml::Value::as_str) != Some("minimal")
        || components != ["rust-src".to_owned()]
    {
        return Err("fuzz toolchain authority drifted".to_owned());
    }

    let lockfile = parse_toml(workspace_root, FUZZ_LOCKFILE_RELATIVE)?;
    let packages = lockfile
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "fuzz lockfile packages are missing".to_owned())?;
    let package_names = packages
        .iter()
        .filter_map(|package| package.get("name").and_then(toml::Value::as_str))
        .collect::<BTreeSet<_>>();
    for required in ["radroots-fuzz", "radroots_blossom", "libfuzzer-sys"] {
        if !package_names.contains(required) {
            return Err(format!("fuzz lockfile is missing package {required}"));
        }
    }
    let fuzz_lock_package = packages
        .iter()
        .find(|package| package.get("name").and_then(toml::Value::as_str) == Some("radroots-fuzz"))
        .ok_or_else(|| "fuzz lockfile is missing package radroots-fuzz".to_owned())?
        .as_table()
        .ok_or_else(|| "fuzz lockfile radroots-fuzz package must be a table".to_owned())?;
    validate_fuzz_crate_version("fuzz lockfile radroots-fuzz package", fuzz_lock_package)?;
    let fuzz_lock_dependencies = toml_string_array(
        "fuzz lockfile radroots-fuzz dependencies",
        fuzz_lock_package.get("dependencies"),
    )?
    .into_iter()
    .collect::<BTreeSet<_>>();
    if fuzz_lock_dependencies
        != ["libfuzzer-sys", "radroots_blossom"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    {
        return Err("fuzz lockfile root dependency authority drifted".to_owned());
    }

    validate_fuzz_common_harness(workspace_root)?;
    for (target, _, media_type, extension) in FUZZ_TARGET_SPECS {
        validate_fuzz_target(workspace_root, target, media_type, extension)?;
    }
    if validate_corpus {
        validate_fuzz_corpus(workspace_root)?;
    }
    read_regular_file(workspace_root, SECURITY_DOCUMENT_RELATIVE)?;
    read_regular_file(workspace_root, BLOSSOM_MANIFEST_RELATIVE)?;
    Ok(())
}

fn validate_operations_authority(workspace_root: &Path) -> Result<(), String> {
    let manifest = parse_toml(workspace_root, OPERATIONS_RELATIVE)?;
    let operations = manifest
        .get("operations")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "operations.toml has no operations table".to_owned())?;
    if operations.contains_key(PHANTOM_OPERATION_KEY)
        || operations.values().any(|operation| {
            operation.get("id").and_then(toml::Value::as_str) == Some(PHANTOM_OPERATION_ID)
        })
    {
        return Err(
            "operations.toml must not reintroduce the synthetic raster operation".to_owned(),
        );
    }
    let operation = operations
        .get(OPERATION_KEY)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("operations.toml is missing {OPERATION_KEY}"))?;
    for (field, expected) in [
        ("domain", "blossom"),
        ("id", OPERATION_ID),
        ("stability", "beta"),
        ("error_class", "validation_error"),
        ("signing", "none"),
        ("transport", "none"),
    ] {
        if operation.get(field).and_then(toml::Value::as_str) != Some(expected) {
            return Err(format!("{OPERATION_KEY} {field} must be {expected}"));
        }
    }
    if operation
        .get("deterministic")
        .and_then(toml::Value::as_bool)
        != Some(true)
        || toml_string_array("operation inputs", operation.get("inputs"))?
            != owned(OPERATION_INPUTS)
        || toml_string_array("operation outputs", operation.get("outputs"))?
            != owned(OPERATION_OUTPUTS)
    {
        return Err(format!("{OPERATION_KEY} signature or determinism drifted"));
    }
    let implementation = operation
        .get("implementation")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{OPERATION_KEY} implementation is missing"))?;
    if toml_string_array("operation modules", implementation.get("rust_modules"))?
        != owned(OPERATION_MODULES)
        || toml_string_array("operation rust types", implementation.get("rust_types"))?
            != owned(OPERATION_RUST_TYPES)
    {
        return Err(format!("{OPERATION_KEY} implementation drifted"));
    }
    let conformance = operation
        .get("conformance")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{OPERATION_KEY} conformance is missing"))?;
    if conformance.get("vector").and_then(toml::Value::as_str) != Some(PRIMARY_OPERATION_VECTOR)
        || toml_string_array("operation case kinds", conformance.get("case_kinds"))?
            != owned(PRIMARY_OPERATION_CASE_KINDS)
    {
        return Err(format!("{OPERATION_KEY} conformance drifted"));
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
            != Some("breaking")
        || matching[0]
            .get("semver_impacts")
            .and_then(toml::Value::as_array)
            .map(|impacts| {
                impacts
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .collect::<BTreeSet<_>>()
            })
            != Some(RELEASE_SEMVER_IMPACTS.iter().copied().collect())
    {
        return Err(format!(
            "{RELEASE_RELATIVE} must contain exactly one breaking change {RELEASE_CHANGE_ID} with its governed semver impacts"
        ));
    }
    let changelog = read_regular_file(workspace_root, CHANGELOG_RELATIVE)?;
    let changelog = std::str::from_utf8(&changelog)
        .map_err(|error| format!("{CHANGELOG_RELATIVE} must be UTF-8: {error}"))?;
    if changelog.matches(CHANGELOG_MARKER).count() != 1 {
        return Err(format!(
            "{CHANGELOG_RELATIVE} must contain exactly one {CHANGELOG_MARKER}"
        ));
    }
    Ok(())
}

fn validate_vector(workspace_root: &Path) -> Result<ValidatedVector, String> {
    let bytes = read_regular_file(workspace_root, VECTOR_RELATIVE)?;
    let suite: VectorSuite = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {VECTOR_RELATIVE}: {error}"))?;
    if suite.suite != "blossom_raster_decoder_security" || suite.contract_version != "1.0.0" {
        return Err("Blossom raster decoder security vector identity drifted".to_owned());
    }
    validate_canonical_json(VECTOR_RELATIVE, &bytes, &suite)?;
    let ordered_ids = suite
        .vectors
        .iter()
        .map(|case| case.id.clone())
        .collect::<Vec<_>>();
    if ordered_ids
        != VECTOR_CASE_IDS
            .iter()
            .map(|id| (*id).to_owned())
            .collect::<Vec<_>>()
    {
        return Err("Blossom raster decoder security vector case inventory drifted".to_owned());
    }
    let mut ids = BTreeSet::new();
    let mut accepted_formats = BTreeSet::new();
    let mut accepted_mutations = BTreeSet::new();
    let mut rejected_mutations = BTreeSet::new();
    let mut rejected_codes = BTreeSet::new();
    for case in &suite.vectors {
        if !ids.insert(case.id.clone())
            || case.input.format.is_empty()
            || case.input.mutation.is_empty()
            || case.input.bytes_hex.is_empty()
            || case.input.bytes_hex.len() % 2 != 0
            || !case
                .input
                .bytes_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(
                "Blossom raster decoder security vector ids and inputs must be unique/nonempty hex"
                    .to_owned(),
            );
        }
        match case.kind.as_str() {
            ACCEPTED_KIND => {
                if !case.expected.accepted
                    || case.expected.error.is_some()
                    || !case.expected.width.is_some_and(|width| width >= 1)
                    || !case.expected.height.is_some_and(|height| height >= 1)
                {
                    return Err(format!("{} has invalid acceptance expectation", case.id));
                }
                accepted_formats.insert(case.input.format.clone());
                accepted_mutations.insert(case.input.mutation.clone());
            }
            REJECTED_KIND => {
                if case.expected.accepted
                    || case.expected.width.is_some()
                    || case.expected.height.is_some()
                    || !case
                        .expected
                        .error
                        .as_deref()
                        .is_some_and(|error| OBSERVED_REJECTION_ERROR_CODES.contains(&error))
                {
                    return Err(format!("{} has invalid rejection expectation", case.id));
                }
                rejected_mutations.insert(case.input.mutation.clone());
                rejected_codes.insert(case.expected.error.clone().unwrap_or_default());
            }
            kind => return Err(format!("{} uses unsupported kind {kind}", case.id)),
        }
    }
    if accepted_formats
        != ["jpeg", "png", "webp"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    {
        return Err(
            "Blossom raster decoder security vector must accept all three raster formats"
                .to_owned(),
        );
    }
    if accepted_mutations
        != REQUIRED_ACCEPTED_MUTATIONS
            .iter()
            .map(|mutation| (*mutation).to_owned())
            .collect()
    {
        return Err("Blossom raster decoder accepted mutation inventory drifted".to_owned());
    }
    if rejected_mutations
        != REQUIRED_REJECTED_MUTATIONS
            .iter()
            .map(|mutation| (*mutation).to_owned())
            .collect()
    {
        return Err("Blossom raster decoder rejected mutation inventory drifted".to_owned());
    }
    if accepted_mutations
        .union(&rejected_mutations)
        .cloned()
        .collect::<BTreeSet<_>>()
        != ALL_PROVENANCE_MUTATIONS
            .iter()
            .map(|mutation| (*mutation).to_owned())
            .collect()
    {
        return Err("Blossom raster decoder provenance mutation inventory drifted".to_owned());
    }
    if rejected_codes
        != OBSERVED_REJECTION_ERROR_CODES
            .iter()
            .map(|code| (*code).to_owned())
            .collect()
    {
        return Err("Blossom raster decoder observed error inventory drifted".to_owned());
    }
    Ok(ValidatedVector {
        bytes,
        case_ids: ordered_ids,
        cases: suite.vectors,
    })
}

fn manifest_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/schemas/blossom/raster-decoder-security-manifest-v1.json",
        "title": "Radroots Blossom raster decoder security semantic contract",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schema_version",
            "contract_id",
            "authority_id",
            "manifest_schema",
            "predecessors",
            "protocol_sources",
            "resource_limits",
            "decoder_profile",
            "resource_probes",
            "fuzz_campaign",
            "nix_lanes",
            "operation",
            "release_change_id",
            "result_vector"
        ],
        "properties": {
            "schema_version": {"const": SCHEMA_VERSION},
            "contract_id": {"const": CONTRACT_ID},
            "authority_id": {"const": AUTHORITY_ID},
            "manifest_schema": {"$ref": "#/$defs/file"},
            "predecessors": {
                "type": "array",
                "minItems": 1,
                "maxItems": 1,
                "items": {"$ref": "#/$defs/predecessor"}
            },
            "protocol_sources": {
                "type": "array",
                "minItems": 2,
                "maxItems": 2,
                "items": {"$ref": "#/$defs/protocol_source"}
            },
            "resource_limits": {"$ref": "#/$defs/resource_limits"},
            "decoder_profile": {"$ref": "#/$defs/decoder_profile"},
            "resource_probes": {
                "type": "array",
                "minItems": 12,
                "maxItems": 12,
                "items": {"$ref": "#/$defs/resource_probe"}
            },
            "fuzz_campaign": {"$ref": "#/$defs/fuzz_campaign"},
            "nix_lanes": {"$ref": "#/$defs/nix_lanes"},
            "operation": {"$ref": "#/$defs/operation"},
            "release_change_id": {"const": RELEASE_CHANGE_ID},
            "result_vector": {"$ref": "#/$defs/result_vector"}
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
            "predecessor": {
                "type": "object",
                "additionalProperties": false,
                "required": ["contract_id", "immutable_artifacts", "artifact_supersessions", "source_supersessions"],
                "properties": {
                    "contract_id": {"const": "radroots_blossom.publication_readiness_v1"},
                    "immutable_artifacts": {
                        "type": "array",
                        "minItems": 1,
                        "items": {"$ref": "#/$defs/file"}
                    },
                    "artifact_supersessions": {
                        "type": "array",
                        "minItems": 1,
                        "items": {"$ref": "#/$defs/file"}
                    },
                    "source_supersessions": {
                        "type": "array",
                        "minItems": 1,
                        "items": {"type": "string", "minLength": 1},
                        "uniqueItems": true
                    }
                }
            },
            "protocol_source": {
                "type": "object",
                "additionalProperties": false,
                "required": ["id", "repository", "revision"],
                "properties": {
                    "id": {"enum": ["nostr_nips", "blossom"]},
                    "repository": {
                        "enum": [
                            "https://github.com/nostr-protocol/nips",
                            "https://github.com/hzrd149/blossom"
                        ]
                    },
                    "revision": {"type": "string", "pattern": "^[0-9a-f]{40}$"}
                }
            },
            "resource_limits": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "max_raster_bytes",
                    "max_decoded_bytes",
                    "max_dimension",
                    "max_pixels",
                    "max_container_records",
                    "jpeg_max_scans",
                    "jpeg_max_blocks",
                    "jpeg_max_coefficient_steps",
                    "jpeg_entropy_bit_reads_per_byte",
                    "peak_rss_kib_limit"
                ],
                "properties": {
                    "max_raster_bytes": {"const": MAX_RASTER_BYTES},
                    "max_decoded_bytes": {"const": MAX_DECODED_BYTES},
                    "max_dimension": {"const": MAX_DIMENSION},
                    "max_pixels": {"const": MAX_PIXELS},
                    "max_container_records": {"const": MAX_CONTAINER_RECORDS},
                    "jpeg_max_scans": {"const": JPEG_MAX_SCANS},
                    "jpeg_max_blocks": {"const": JPEG_MAX_BLOCKS},
                    "jpeg_max_coefficient_steps": {"const": JPEG_MAX_COEFFICIENT_STEPS},
                    "jpeg_entropy_bit_reads_per_byte": {"const": JPEG_ENTROPY_BIT_READS_PER_BYTE},
                    "peak_rss_kib_limit": {"const": PEAK_RSS_KIB_LIMIT}
                }
            },
            "decoder_profile": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "accepted_formats",
                    "accepted_jpeg_processes",
                    "accepted_png_color_types",
                    "normative_error_codes",
                    "observed_rejection_error_codes",
                    "required_accepted_mutations",
                    "required_rejected_mutations"
                ],
                "properties": {
                    "accepted_formats": {
                        "const": ["image/jpeg", "image/png", "image/webp"]
                    },
                    "accepted_jpeg_processes": {
                        "const": ["sof0_baseline_8bit", "sof1_extended_sequential_8bit"]
                    },
                    "accepted_png_color_types": {"const": [0, 2, 3, 4, 6]},
                    "normative_error_codes": {
                        "type": "array",
                        "minItems": 13,
                        "maxItems": 13,
                        "items": {"type": "string", "enum": NORMATIVE_ERROR_CODES},
                        "uniqueItems": true
                    },
                    "observed_rejection_error_codes": {
                        "type": "array",
                        "minItems": 7,
                        "maxItems": 7,
                        "items": {"type": "string", "enum": OBSERVED_REJECTION_ERROR_CODES},
                        "uniqueItems": true
                    },
                    "required_accepted_mutations": {
                        "type": "array",
                        "minItems": 2,
                        "maxItems": 2,
                        "items": {"type": "string", "enum": REQUIRED_ACCEPTED_MUTATIONS},
                        "uniqueItems": true
                    },
                    "required_rejected_mutations": {
                        "type": "array",
                        "minItems": 14,
                        "maxItems": 14,
                        "items": {"type": "string", "enum": REQUIRED_REJECTED_MUTATIONS},
                        "uniqueItems": true
                    }
                }
            },
            "resource_probe": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "id",
                    "format",
                    "process",
                    "width",
                    "height",
                    "max_encoded_bytes",
                    "max_decoded_bytes",
                    "executor_test",
                    "fixture_authority",
                    "repetitions",
                    "peak_rss_kib_limit"
                ],
                "properties": {
                    "id": {
                        "enum": [
                            "jpeg_grayscale", "jpeg_rgb", "jpeg_cmyk", "jpeg_sof1",
                            "png_rgb", "png_palette", "png_rgba", "png_adam7",
                            "webp_vp8_rgb", "webp_vp8_alpha", "webp_vp8l_rgb", "webp_vp8l_alpha"
                        ]
                    },
                    "format": {"enum": ["jpeg", "png", "webp"]},
                    "process": {"type": "string", "minLength": 1},
                    "width": {"const": 5000},
                    "height": {"const": 4000},
                    "max_encoded_bytes": {"const": MAX_RASTER_BYTES},
                    "max_decoded_bytes": {"enum": [60_000_000, 80_000_000]},
                    "executor_test": {"const": RESOURCE_TEST},
                    "fixture_authority": {"type": "string", "minLength": 1},
                    "repetitions": {"const": 3},
                    "peak_rss_kib_limit": {"const": PEAK_RSS_KIB_LIMIT}
                }
            },
            "fuzz_target": {
                "type": "object",
                "additionalProperties": false,
                "required": ["name", "format", "media_type", "extension", "binary"],
                "properties": {
                    "name": {"enum": FUZZ_TARGETS},
                    "format": {"enum": ["jpeg", "png", "webp"]},
                    "media_type": {"enum": ["image/jpeg", "image/png", "image/webp"]},
                    "extension": {"enum": ["jpg", "png", "webp"]},
                    "binary": {"$ref": "#/$defs/file"}
                }
            },
            "corpus_seed": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "vector_id",
                    "target",
                    "format",
                    "provenance_mutation",
                    "expected_accepted",
                    "file"
                ],
                "properties": {
                    "vector_id": {"type": "string", "enum": VECTOR_CASE_IDS},
                    "target": {"type": "string", "enum": FUZZ_TARGETS},
                    "format": {"enum": ["jpeg", "png", "webp"]},
                    "provenance_mutation": {"type": "string", "enum": ALL_PROVENANCE_MUTATIONS},
                    "expected_accepted": {"type": "boolean"},
                    "expected_error": {"type": "string", "enum": OBSERVED_REJECTION_ERROR_CODES},
                    "file": {"$ref": "#/$defs/file"}
                }
            },
            "fuzz_campaign": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "project_manifest",
                    "project_lockfile",
                    "toolchain",
                    "toolchain_channel",
                    "toolchain_components",
                    "toolchain_profile",
                    "engine",
                    "sanitizer",
                    "common_harness",
                    "targets",
                    "corpus_seeds",
                    "smoke_runs",
                    "smoke_seed",
                    "smoke_max_input_bytes",
                    "smoke_timeout_seconds",
                    "engine_rss_limit_mb",
                    "extended_campaign_document"
                ],
                "properties": {
                    "project_manifest": {"$ref": "#/$defs/file"},
                    "project_lockfile": {"$ref": "#/$defs/file"},
                    "toolchain": {"$ref": "#/$defs/file"},
                    "toolchain_channel": {"const": FUZZ_TOOLCHAIN_CHANNEL},
                    "toolchain_components": {"const": ["rust-src"]},
                    "toolchain_profile": {"const": "minimal"},
                    "engine": {"const": "libfuzzer"},
                    "sanitizer": {"const": "address"},
                    "common_harness": {"$ref": "#/$defs/file"},
                    "targets": {
                        "type": "array",
                        "minItems": 3,
                        "maxItems": 3,
                        "items": {"$ref": "#/$defs/fuzz_target"}
                    },
                    "corpus_seeds": {
                        "type": "array",
                        "minItems": 30,
                        "maxItems": 30,
                        "items": {"$ref": "#/$defs/corpus_seed"}
                    },
                    "smoke_runs": {"const": FUZZ_SMOKE_RUNS},
                    "smoke_seed": {"const": FUZZ_SMOKE_SEED},
                    "smoke_max_input_bytes": {"const": FUZZ_SMOKE_MAX_INPUT_BYTES},
                    "smoke_timeout_seconds": {"const": FUZZ_SMOKE_TIMEOUT_SECONDS},
                    "engine_rss_limit_mb": {"const": FUZZ_ENGINE_RSS_LIMIT_MB},
                    "extended_campaign_document": {"$ref": "#/$defs/file"}
                }
            },
            "nix_lanes": {
                "type": "object",
                "additionalProperties": false,
                "required": ["app", "devshell", "stable_check", "fuzz_check"],
                "properties": {
                    "app": {"const": "decoder-security"},
                    "devshell": {"const": "decoder-security"},
                    "stable_check": {"const": "blossom-raster-decode-test"},
                    "fuzz_check": {"const": "blossom-decoder-fuzz-smoke"}
                }
            },
            "operation": {
                "type": "object",
                "additionalProperties": false,
                "required": ["key", "id", "primary_vector", "primary_case_kinds", "security_case_kinds"],
                "properties": {
                    "key": {"const": OPERATION_KEY},
                    "id": {"const": OPERATION_ID},
                    "primary_vector": {"const": PRIMARY_OPERATION_VECTOR},
                    "primary_case_kinds": {"const": PRIMARY_OPERATION_CASE_KINDS},
                    "security_case_kinds": {"const": [ACCEPTED_KIND, REJECTED_KIND]}
                }
            },
            "result_vector": {
                "type": "object",
                "additionalProperties": false,
                "required": ["canonical", "mirror", "executor", "executor_tests", "case_ids"],
                "properties": {
                    "canonical": {"$ref": "#/$defs/file"},
                    "mirror": {"$ref": "#/$defs/file"},
                    "executor": {"$ref": "#/$defs/file"},
                    "executor_tests": {"const": [REGRESSION_TEST, DIFFERENTIAL_TEST, RESOURCE_TEST]},
                    "case_ids": {"const": VECTOR_CASE_IDS}
                }
            }
        }
    })
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
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
    validate_closed_object_schemas(schema, "$")?;
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

fn validate_closed_object_schemas(value: &Value, path: &str) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("object")
                && (object.get("additionalProperties").and_then(Value::as_bool) != Some(false)
                    || !object.get("properties").is_some_and(Value::is_object)
                    || !object.get("required").is_some_and(Value::is_array))
            {
                return Err(format!(
                    "{MANIFEST_SCHEMA_RELATIVE} object schema {path} must close properties and declare its required inventory"
                ));
            }
            for (key, child) in object {
                validate_closed_object_schemas(child, &format!("{path}/{key}"))?;
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                validate_closed_object_schemas(child, &format!("{path}/{index}"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeSet, fs, path::PathBuf};

    const COMMON_SOURCE: &str = include_str!("../../../../fuzz/fuzz_targets/common.rs");
    const JPEG_TARGET_SOURCE: &str =
        include_str!("../../../../fuzz/fuzz_targets/publication_jpeg.rs");

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn write_temp_file(root: &Path, relative: &str, bytes: &[u8]) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("temporary file parent"))
            .expect("create temporary authority parent");
        fs::write(path, bytes).expect("write temporary authority file");
    }

    fn validate_temp_manifest(root: &Path, expected: &Value) -> Result<(), String> {
        let schema_bytes = read_regular_file(root, MANIFEST_SCHEMA_RELATIVE)?;
        let schema: Value = serde_json::from_slice(&schema_bytes)
            .map_err(|error| format!("parse temporary schema: {error}"))?;
        validate_canonical_json(MANIFEST_SCHEMA_RELATIVE, &schema_bytes, &schema)?;
        let manifest_bytes = read_regular_file(root, MANIFEST_RELATIVE)?;
        let manifest: RasterDecoderSecurityManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| format!("parse temporary manifest: {error}"))?;
        validate_canonical_json(MANIFEST_RELATIVE, &manifest_bytes, &manifest)?;
        let actual = serde_json::to_value(manifest)
            .map_err(|error| format!("serialize temporary manifest: {error}"))?;
        validate_json_schema(&schema, &actual)?;
        if &actual != expected {
            return Err("temporary manifest retargeted governed authority".to_owned());
        }
        Ok(())
    }

    fn object_at_mut<'a>(
        value: &'a mut Value,
        pointer: &str,
    ) -> &'a mut serde_json::Map<String, Value> {
        let target = if pointer.is_empty() {
            value
        } else {
            value.pointer_mut(pointer).expect("authority JSON pointer")
        };
        target.as_object_mut().expect("authority object")
    }

    fn expect_manifest_mutation_rejected(expected: &Value, mutated: &Value, label: &str) {
        let temp = tempfile::tempdir().expect("temporary manifest workspace");
        let schema_bytes = canonical_json_bytes(&manifest_schema()).expect("manifest schema bytes");
        write_temp_file(temp.path(), MANIFEST_SCHEMA_RELATIVE, &schema_bytes);
        write_temp_file(
            temp.path(),
            MANIFEST_RELATIVE,
            &canonical_json_bytes(mutated).expect("mutated manifest bytes"),
        );
        validate_temp_manifest(temp.path(), expected).expect_err(label);
    }

    #[test]
    fn checked_in_fuzz_corpus_matches_exact_vector() {
        let workspace_root = workspace_root();
        validate_vector(&workspace_root).expect("exact 30-case vector authority");
        validate_fuzz_authority(&workspace_root, true)
            .expect("raw seed inventory and structural fuzz authority");
    }

    #[test]
    fn fuzz_crate_version_authority_rejects_drift() {
        let root = workspace_root();
        let manifest = parse_toml(&root, FUZZ_MANIFEST_RELATIVE).expect("fuzz manifest");
        let package = manifest
            .get("package")
            .and_then(toml::Value::as_table)
            .expect("fuzz package table");
        validate_fuzz_crate_version("fuzz package", package).expect("frozen fuzz version");

        let mut drifted = package.clone();
        drifted.insert(
            "version".to_owned(),
            toml::Value::String("0.0.0".to_owned()),
        );
        let error = validate_fuzz_crate_version("fuzz package", &drifted)
            .expect_err("fuzz version drift must fail closed");
        assert!(error.contains(LIB_CRATE_VERSION), "{error}");
    }

    #[test]
    fn fuzz_call_chain_rejects_structural_drift() {
        validate_fuzz_common_source(COMMON_SOURCE).expect("governed common fuzz harness");
        validate_fuzz_target_source(
            "fuzz/fuzz_targets/publication_jpeg.rs",
            JPEG_TARGET_SOURCE,
            "image/jpeg",
            "jpg",
        )
        .expect("governed JPEG fuzz target");

        let target_mutations = [
            (
                "no-op target",
                JPEG_TARGET_SOURCE
                    .replace("    common::exercise(data, \"image/jpeg\", \"jpg\");\n", ""),
            ),
            (
                "wrong MIME",
                JPEG_TARGET_SOURCE.replacen("image/jpeg", "image/png", 1),
            ),
            (
                "wrong extension",
                JPEG_TARGET_SOURCE.replacen("\"jpg\"", "\"png\"", 1),
            ),
            (
                "alternate harness",
                JPEG_TARGET_SOURCE.replacen("common::exercise", "alternate::exercise", 1),
            ),
            (
                "bytes not passed",
                JPEG_TARGET_SOURCE.replacen("common::exercise(data", "common::exercise(&[]", 1),
            ),
        ];
        for (label, source) in target_mutations {
            validate_fuzz_target_source(
                "fuzz/fuzz_targets/publication_jpeg.rs",
                &source,
                "image/jpeg",
                "jpg",
            )
            .expect_err(label);
        }

        let omitted_public_call = COMMON_SOURCE.replacen(
            "let _ = verify_publication_readiness(",
            "let _ = omitted_public_operation(",
            1,
        );
        validate_fuzz_common_source(&omitted_public_call)
            .expect_err("omitted public operation must fail closed");

        let mut bin_names = BTreeSet::new();
        insert_fuzz_bin_name(&mut bin_names, "publication_jpeg").expect("first target");
        insert_fuzz_bin_name(&mut bin_names, "publication_jpeg")
            .expect_err("duplicate target must fail closed");
    }

    #[test]
    fn nested_manifest_authority_mutations_fail_closed() {
        struct AuthorityClass {
            label: &'static str,
            pointer: &'static str,
            required_field: &'static str,
            invalid_field: &'static str,
            invalid_value: Value,
            retarget_field: &'static str,
            retarget_value: Value,
        }

        let root = workspace_root();
        let schema = manifest_schema();
        validate_closed_object_schemas(&schema, "$").expect("fully closed manifest schema");
        let schema_bytes = canonical_json_bytes(&schema).expect("schema bytes");
        let expected = serde_json::to_value(
            describe_manifest(&root, &schema_bytes).expect("live raster security manifest"),
        )
        .expect("manifest value");
        let mirror_descriptor = expected["result_vector"]["mirror"].clone();
        let project_descriptor = expected["fuzz_campaign"]["project_manifest"].clone();
        let classes = vec![
            AuthorityClass {
                label: "root",
                pointer: "",
                required_field: "schema_version",
                invalid_field: "contract_id",
                invalid_value: json!("invalid"),
                retarget_field: "authority_id",
                retarget_value: json!("retargeted_authority"),
            },
            AuthorityClass {
                label: "file descriptor",
                pointer: "/manifest_schema",
                required_field: "path",
                invalid_field: "sha256",
                invalid_value: json!("not-a-digest"),
                retarget_field: "path",
                retarget_value: json!("crates/blossom/contracts/retargeted.schema.json"),
            },
            AuthorityClass {
                label: "predecessor",
                pointer: "/predecessors/0",
                required_field: "contract_id",
                invalid_field: "source_supersessions",
                invalid_value: json!([]),
                retarget_field: "contract_id",
                retarget_value: json!("radroots_blossom.retargeted_v1"),
            },
            AuthorityClass {
                label: "protocol source",
                pointer: "/protocol_sources/0",
                required_field: "revision",
                invalid_field: "revision",
                invalid_value: json!("not-a-revision"),
                retarget_field: "repository",
                retarget_value: json!("https://github.com/hzrd149/blossom"),
            },
            AuthorityClass {
                label: "resource limits",
                pointer: "/resource_limits",
                required_field: "max_raster_bytes",
                invalid_field: "max_raster_bytes",
                invalid_value: json!(0),
                retarget_field: "max_decoded_bytes",
                retarget_value: json!(60_000_000),
            },
            AuthorityClass {
                label: "decoder profile",
                pointer: "/decoder_profile",
                required_field: "accepted_formats",
                invalid_field: "normative_error_codes",
                invalid_value: json!([]),
                retarget_field: "accepted_formats",
                retarget_value: json!(["image/png", "image/jpeg", "image/webp"]),
            },
            AuthorityClass {
                label: "resource probe",
                pointer: "/resource_probes/0",
                required_field: "id",
                invalid_field: "id",
                invalid_value: json!("invalid_probe"),
                retarget_field: "id",
                retarget_value: json!("jpeg_rgb"),
            },
            AuthorityClass {
                label: "fuzz campaign",
                pointer: "/fuzz_campaign",
                required_field: "toolchain_channel",
                invalid_field: "smoke_runs",
                invalid_value: json!(0),
                retarget_field: "common_harness",
                retarget_value: project_descriptor,
            },
            AuthorityClass {
                label: "fuzz target",
                pointer: "/fuzz_campaign/targets/0",
                required_field: "name",
                invalid_field: "name",
                invalid_value: json!("invalid_target"),
                retarget_field: "name",
                retarget_value: json!("publication_png"),
            },
            AuthorityClass {
                label: "corpus seed",
                pointer: "/fuzz_campaign/corpus_seeds/0",
                required_field: "vector_id",
                invalid_field: "vector_id",
                invalid_value: json!("invalid_vector"),
                retarget_field: "target",
                retarget_value: json!("publication_png"),
            },
            AuthorityClass {
                label: "Nix lanes",
                pointer: "/nix_lanes",
                required_field: "app",
                invalid_field: "app",
                invalid_value: json!("invalid-app"),
                retarget_field: "stable_check",
                retarget_value: json!("blossom-decoder-fuzz-smoke"),
            },
            AuthorityClass {
                label: "operation",
                pointer: "/operation",
                required_field: "id",
                invalid_field: "primary_case_kinds",
                invalid_value: json!([]),
                retarget_field: "id",
                retarget_value: json!(PHANTOM_OPERATION_ID),
            },
            AuthorityClass {
                label: "result vector",
                pointer: "/result_vector",
                required_field: "canonical",
                invalid_field: "case_ids",
                invalid_value: json!([]),
                retarget_field: "canonical",
                retarget_value: mirror_descriptor,
            },
        ];

        for class in classes {
            let mut missing = expected.clone();
            object_at_mut(&mut missing, class.pointer).remove(class.required_field);
            expect_manifest_mutation_rejected(
                &expected,
                &missing,
                &format!("{} missing field", class.label),
            );

            let mut extra = expected.clone();
            object_at_mut(&mut extra, class.pointer).insert("unexpected".to_owned(), json!(true));
            expect_manifest_mutation_rejected(
                &expected,
                &extra,
                &format!("{} extra field", class.label),
            );

            let mut mistyped = expected.clone();
            object_at_mut(&mut mistyped, class.pointer)
                .insert(class.required_field.to_owned(), Value::Null);
            expect_manifest_mutation_rejected(
                &expected,
                &mistyped,
                &format!("{} mistyped field", class.label),
            );

            let mut invalid = expected.clone();
            object_at_mut(&mut invalid, class.pointer)
                .insert(class.invalid_field.to_owned(), class.invalid_value);
            expect_manifest_mutation_rejected(
                &expected,
                &invalid,
                &format!("{} invalid field", class.label),
            );

            let mut retargeted = expected.clone();
            object_at_mut(&mut retargeted, class.pointer)
                .insert(class.retarget_field.to_owned(), class.retarget_value);
            expect_manifest_mutation_rejected(
                &expected,
                &retargeted,
                &format!("{} retargeted field", class.label),
            );
        }
    }

    #[test]
    fn fuzz_corpus_inventory_mutations_fail_closed() {
        for mutation in ["missing", "extra", "retargeted"] {
            let source = workspace_root();
            let vector = validate_vector(&source).expect("live vector authority");
            let temp = tempfile::tempdir().expect("temporary corpus workspace");
            write_temp_file(
                temp.path(),
                VECTOR_RELATIVE,
                &read_regular_file(&source, VECTOR_RELATIVE).expect("live vector bytes"),
            );
            for seed in expected_seeds(&vector).expect("expected raw seeds") {
                write_temp_file(temp.path(), seed.relative, &seed.bytes);
            }

            match mutation {
                "missing" => fs::remove_file(
                    temp.path()
                        .join("fuzz/corpus/publication_jpeg/jpeg_baseline_gray_8bit.bin"),
                )
                .expect("remove one governed seed"),
                "extra" => write_temp_file(
                    temp.path(),
                    "fuzz/corpus/publication_png/unexpected.bin",
                    b"unexpected seed",
                ),
                "retargeted" => {
                    let original = temp
                        .path()
                        .join("fuzz/corpus/publication_jpeg/jpeg_baseline_gray_8bit.bin");
                    let retargeted = temp
                        .path()
                        .join("fuzz/corpus/publication_png/jpeg_baseline_gray_8bit.bin");
                    fs::rename(original, retargeted).expect("retarget governed seed");
                }
                _ => unreachable!(),
            }
            validate_fuzz_corpus(temp.path())
                .expect_err(&format!("{mutation} corpus mutation must fail closed"));
        }
    }
}
