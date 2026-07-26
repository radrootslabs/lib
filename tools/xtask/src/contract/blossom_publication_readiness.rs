use super::artifact_bundle::{read_regular_file, with_artifact_bundle_transaction};
use super::raw_source_rebuild::validate_raw_source_rebuild_predecessor_production_sources_under_lock;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const VECTOR_CANONICAL_RELATIVE: &str =
    "contracts/conformance/vectors/blossom/publication_readiness.v1.json";
const VECTOR_MIRROR_RELATIVE: &str = "crates/blossom/tests/fixtures/publication_readiness.v1.json";
const WORKSPACE_MANIFEST_RELATIVE: &str = "Cargo.toml";
const WORKSPACE_LOCK_RELATIVE: &str = "Cargo.lock";
const READINESS_SOURCE_RELATIVE: &str = "crates/blossom/src/publication_readiness.rs";
const SEQUENTIAL_JPEG_SOURCE_RELATIVE: &str =
    "crates/blossom/src/publication_readiness/sequential_jpeg.rs";
const BLOSSOM_LIB_RELATIVE: &str = "crates/blossom/src/lib.rs";
const BLOSSOM_URL_RELATIVE: &str = "crates/blossom/src/url.rs";
const BLOSSOM_MANIFEST_RELATIVE: &str = "crates/blossom/Cargo.toml";
const COVERAGE_PROFILES_RELATIVE: &str = "contracts/coverage-profiles.toml";
const NIX_COMMON_RELATIVE: &str = "build/nix/common.nix";
const NIX_CHECKS_RELATIVE: &str = "build/nix/checks.nix";
const OPERATIONS_RELATIVE: &str = "contracts/operations.toml";
const RELEASE_RELATIVE: &str = "contracts/releases/1.0.0-alpha.1.toml";
const CHANGELOG_RELATIVE: &str = "CHANGELOG.md";
const RAW_PREDECESSOR_GOVERNANCE_RELATIVE: &str = "tools/xtask/src/contract/raw_source_rebuild.rs";
const RELEASE_CHANGE_ID: &str = "blossom-publication-readiness-evidence";
const CHANGELOG_MARKER: &str = "<!-- release-change: blossom-publication-readiness-evidence -->";

const RAW_PREDECESSOR_SUPERSEDED_PATHS: &[&str] = &[
    WORKSPACE_LOCK_RELATIVE,
    WORKSPACE_MANIFEST_RELATIVE,
    NIX_COMMON_RELATIVE,
    CHANGELOG_RELATIVE,
    RELEASE_RELATIVE,
    "tools/xtask/src/contract.rs",
    "tools/xtask/src/contract/food_availability_projection.rs",
    "tools/xtask/src/contract/nip09_reconciliation.rs",
    RAW_PREDECESSOR_GOVERNANCE_RELATIVE,
    "tools/xtask/src/contract/source_maintenance.rs",
    "tools/xtask/src/main.rs",
];
const TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS: &[&str] = &[
    WORKSPACE_MANIFEST_RELATIVE,
    BLOSSOM_MANIFEST_RELATIVE,
    "crates/blossom/src/error.rs",
    BLOSSOM_LIB_RELATIVE,
    BLOSSOM_URL_RELATIVE,
];

const SOURCE_INVENTORY: &[&str] = &[
    WORKSPACE_LOCK_RELATIVE,
    WORKSPACE_MANIFEST_RELATIVE,
    NIX_CHECKS_RELATIVE,
    NIX_COMMON_RELATIVE,
    CHANGELOG_RELATIVE,
    BLOSSOM_MANIFEST_RELATIVE,
    "crates/blossom/README",
    "crates/blossom/src/error.rs",
    BLOSSOM_LIB_RELATIVE,
    READINESS_SOURCE_RELATIVE,
    SEQUENTIAL_JPEG_SOURCE_RELATIVE,
    BLOSSOM_URL_RELATIVE,
    "crates/blossom/tests/publication_readiness.rs",
    VECTOR_MIRROR_RELATIVE,
    "contracts/events/blossom-media.md",
    COVERAGE_PROFILES_RELATIVE,
    VECTOR_CANONICAL_RELATIVE,
    OPERATIONS_RELATIVE,
    RELEASE_RELATIVE,
    "tools/xtask/src/contract.rs",
    "tools/xtask/src/main.rs",
    "tools/xtask/src/contract/blossom_publication_readiness.rs",
    "tools/xtask/src/contract/food_availability_projection.rs",
    "tools/xtask/src/contract/nip09_reconciliation.rs",
    RAW_PREDECESSOR_GOVERNANCE_RELATIVE,
    "tools/xtask/src/contract/source_maintenance.rs",
];

const IMMUTABLE_RAW_PREDECESSOR_ARTIFACTS: &[(&str, usize, &str)] = &[
    (
        "crates/event_store/contracts/raw_source_rebuild_v1.manifest.json",
        45_449,
        "03253ce31dc31d465880a895d2685f5deb1274948e0a4eabe81a2f08f238c483",
    ),
    (
        "crates/event_store/contracts/raw_source_rebuild_v1.manifest.schema.json",
        17_896,
        "f9d210967e54b66f39c8bb965d97b2001a0ebc0927e7c2c14edb8e474bfda695",
    ),
    (
        "crates/event_store/contracts/raw_source_rebuild_v1.manifest.sha256",
        65,
        "2b8bc07cd479be2281781660efd26fd7a8f480e5f3f62053aeccd2b5e6b2070c",
    ),
    (
        "crates/event_store/src/generated/raw_source_rebuild_manifest.rs",
        50_735,
        "3763fbee3ee45621afca990002b9298c791bf0396ebf7bccde0ae1bc9aecb7f2",
    ),
    (
        "contracts/conformance/vectors/event_store/raw_source_rebuild.v1.json",
        26_833,
        "c37a2bf3714f53ab04fae8c5c9dbe2ad4b3f5310efa51f46bd8b116660f1fe15",
    ),
    (
        "crates/event_store/tests/fixtures/raw_source_rebuild.v1.json",
        26_833,
        "c37a2bf3714f53ab04fae8c5c9dbe2ad4b3f5310efa51f46bd8b116660f1fe15",
    ),
];

const CURRENT_BYTE_BOUND_BLOSSOM_SOURCES: &[(&str, usize, &str)] = &[
    (
        BLOSSOM_LIB_RELATIVE,
        2_172,
        "97cae38f693795445cc17671649f3d88c0c492fc8d71d2c98a4ca02502e5d43a",
    ),
    (
        "crates/blossom/src/error.rs",
        30_918,
        "bd77810306b3556434d93057ca5ee62db474e9b0af94176ba4aa7a2ef25be7d8",
    ),
    (
        BLOSSOM_URL_RELATIVE,
        15_794,
        "e9673f074ba6328a121aa3008fc11cd4d9d22cae3b09f26602ea6fea3f964c80",
    ),
];

const REQUIRED_PUBLIC_TYPES: &[&str] = &[
    "RadrootsBlossomBud02UploadStatus",
    "RadrootsBlossomBud02UploadObservation",
    "RadrootsBlossomBud01HeadObservation",
    "RadrootsBlossomBud01GetCollector",
    "RadrootsBlossomBud01GetObservation",
    "RadrootsBlossomRasterFormat",
    "RadrootsBlossomRasterDimensions",
    "RadrootsBlossomAuthoredRasterDimensions",
    "RadrootsBlossomPublicationReadinessEvidenceDigest",
    "RadrootsBlossomPublicationReadinessEvidence",
];

const VECTOR_EXPECTATIONS: &[(&str, &str, &str, Option<&str>)] = &[
    (
        "valid_created",
        "blossom.verify_publication_readiness.valid",
        "none",
        None,
    ),
    (
        "valid_ok_without_authored_dimensions",
        "blossom.verify_publication_readiness.valid",
        "upload_status_200",
        None,
    ),
    (
        "invalid_upload_status",
        "blossom.verify_publication_readiness.invalid",
        "upload_status_202",
        Some("invalid_bud02_upload_status"),
    ),
    (
        "invalid_head_status",
        "blossom.verify_publication_readiness.invalid",
        "head_status_204",
        Some("invalid_bud01_head_status"),
    ),
    (
        "invalid_get_status",
        "blossom.verify_publication_readiness.invalid",
        "get_status_206",
        Some("invalid_bud01_get_status"),
    ),
    (
        "declared_size_over_public_max",
        "blossom.verify_publication_readiness.invalid",
        "get_size_over_max",
        Some("publication_raster_byte_limit_exceeded"),
    ),
    (
        "missing_get_body",
        "blossom.verify_publication_readiness.invalid",
        "get_body_missing",
        Some("publication_get_body_missing"),
    ),
    (
        "short_get_body",
        "blossom.verify_publication_readiness.invalid",
        "get_body_short",
        Some("publication_get_body_short"),
    ),
    (
        "trailing_get_body",
        "blossom.verify_publication_readiness.invalid",
        "get_body_trailing",
        Some("publication_get_body_trailing"),
    ),
    (
        "authored_bytes_short",
        "blossom.verify_publication_readiness.invalid",
        "authored_bytes_short",
        Some("publication_authored_bytes_size_mismatch"),
    ),
    (
        "authored_bytes_wrong_hash",
        "blossom.verify_publication_readiness.invalid",
        "authored_bytes_wrong_hash",
        Some("publication_authored_bytes_hash_mismatch"),
    ),
    (
        "upload_url_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "upload_url_mismatch",
        Some("publication_upload_url_mismatch"),
    ),
    (
        "upload_hash_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "upload_hash_mismatch",
        Some("publication_upload_hash_mismatch"),
    ),
    (
        "upload_size_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "upload_size_mismatch",
        Some("publication_upload_size_mismatch"),
    ),
    (
        "upload_mime_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "upload_mime_mismatch",
        Some("publication_upload_media_type_mismatch"),
    ),
    (
        "head_url_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "head_url_mismatch",
        Some("publication_head_url_mismatch"),
    ),
    (
        "head_size_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "head_size_mismatch",
        Some("publication_head_size_mismatch"),
    ),
    (
        "head_mime_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "head_mime_mismatch",
        Some("publication_head_media_type_mismatch"),
    ),
    (
        "get_url_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "get_url_mismatch",
        Some("publication_get_url_mismatch"),
    ),
    (
        "get_declared_size_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "get_declared_size_mismatch",
        Some("publication_get_declared_size_mismatch"),
    ),
    (
        "get_complete_hash_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "get_bytes_wrong_hash",
        Some("publication_retrieved_bytes_hash_mismatch"),
    ),
    (
        "unsupported_raster_mime",
        "blossom.verify_publication_readiness.invalid",
        "unsupported_mime",
        Some("unsupported_publication_raster_media_type"),
    ),
    (
        "malformed_raster",
        "blossom.verify_publication_readiness.invalid",
        "malformed_container",
        Some("invalid_publication_raster"),
    ),
    (
        "animated_png",
        "blossom.verify_publication_readiness.invalid",
        "animated_png",
        Some("publication_raster_animation_forbidden"),
    ),
    (
        "declared_format_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "declared_mime_jpeg",
        Some("invalid_publication_raster"),
    ),
    (
        "corrupt_png_crc",
        "blossom.verify_publication_readiness.invalid",
        "corrupt_png_crc",
        Some("publication_raster_decode_failed"),
    ),
    (
        "corrupt_png_deflate",
        "blossom.verify_publication_readiness.invalid",
        "corrupt_png_deflate",
        Some("publication_raster_decode_failed"),
    ),
    (
        "invalid_png_color_type",
        "blossom.verify_publication_readiness.invalid",
        "invalid_png_color_type",
        Some("publication_raster_decode_failed"),
    ),
    (
        "authored_dimension_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "authored_dimension_mismatch",
        Some("publication_authored_raster_dimension_mismatch"),
    ),
    (
        "animated_webp",
        "blossom.verify_publication_readiness.invalid",
        "animated_webp",
        Some("publication_raster_animation_forbidden"),
    ),
    (
        "zero_width",
        "blossom.verify_publication_readiness.invalid",
        "zero_width",
        Some("publication_raster_dimensions_out_of_range"),
    ),
    (
        "dimension_over_max",
        "blossom.verify_publication_readiness.invalid",
        "dimension_over_max",
        Some("publication_raster_dimensions_out_of_range"),
    ),
    (
        "pixel_limit",
        "blossom.verify_publication_readiness.invalid",
        "pixel_limit",
        Some("publication_raster_pixel_limit_exceeded"),
    ),
    (
        "progressive_jpeg",
        "blossom.verify_publication_readiness.invalid",
        "progressive_jpeg",
        Some("publication_jpeg_process_forbidden"),
    ),
    (
        "jpeg_entropy_stripped",
        "blossom.verify_publication_readiness.invalid",
        "jpeg_entropy_stripped",
        Some("publication_raster_decode_failed"),
    ),
    (
        "jpeg_entropy_partial",
        "blossom.verify_publication_readiness.invalid",
        "jpeg_entropy_partial",
        Some("publication_raster_decode_failed"),
    ),
    (
        "malformed_jpeg_dqt",
        "blossom.verify_publication_readiness.invalid",
        "malformed_jpeg_dqt",
        Some("invalid_publication_raster"),
    ),
];

pub(super) fn validate_blossom_publication_readiness(workspace_root: &Path) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_blossom_publication_readiness_under_lock(workspace_root)
    })
}

fn validate_blossom_publication_readiness_under_lock(workspace_root: &Path) -> Result<(), String> {
    validate_immutable_predecessor(workspace_root)?;
    validate_raw_source_rebuild_predecessor_production_sources_under_lock(
        workspace_root,
        RAW_PREDECESSOR_SUPERSEDED_PATHS,
        TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS,
    )?;
    validate_source_inventory(workspace_root)?;
    validate_source_boundary(workspace_root)?;
    validate_vector(workspace_root)?;
    validate_operation(workspace_root)?;
    validate_release(workspace_root)
}

fn validate_immutable_predecessor(workspace_root: &Path) -> Result<(), String> {
    for (relative, expected_length, expected_sha256) in IMMUTABLE_RAW_PREDECESSOR_ARTIFACTS {
        let bytes = read_regular_file(workspace_root, relative)?;
        if bytes.len() != *expected_length || sha256_hex(&bytes) != *expected_sha256 {
            return Err(format!(
                "immutable raw-source rebuild predecessor artifact `{relative}` drifted"
            ));
        }
    }
    Ok(())
}

fn validate_source_inventory(workspace_root: &Path) -> Result<(), String> {
    let unique = SOURCE_INVENTORY.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() != SOURCE_INVENTORY.len() {
        return Err("publication-readiness source inventory contains duplicates".to_owned());
    }
    for relative in SOURCE_INVENTORY {
        let metadata = fs::symlink_metadata(workspace_root.join(relative)).map_err(|error| {
            format!("inspect publication-readiness source `{relative}`: {error}")
        })?;
        if !metadata.file_type().is_file() {
            return Err(format!(
                "publication-readiness source `{relative}` must be a regular file"
            ));
        }
    }
    for superseded in RAW_PREDECESSOR_SUPERSEDED_PATHS
        .iter()
        .chain(TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS)
    {
        if !unique.contains(*superseded) {
            return Err(format!(
                "superseded predecessor source `{superseded}` is not current-byte governed"
            ));
        }
    }
    for (relative, expected_length, expected_sha256) in CURRENT_BYTE_BOUND_BLOSSOM_SOURCES {
        validate_current_byte_bound_blossom_source(
            relative,
            &read_regular_file(workspace_root, relative)?,
            *expected_length,
            expected_sha256,
        )?;
    }
    Ok(())
}

fn validate_current_byte_bound_blossom_source(
    relative: &str,
    bytes: &[u8],
    expected_length: usize,
    expected_sha256: &str,
) -> Result<(), String> {
    if bytes.len() != expected_length || sha256_hex(bytes) != expected_sha256 {
        return Err(format!(
            "publication-readiness current-byte source `{relative}` drifted"
        ));
    }
    Ok(())
}

fn validate_source_boundary(workspace_root: &Path) -> Result<(), String> {
    let source = String::from_utf8(read_regular_file(
        workspace_root,
        READINESS_SOURCE_RELATIVE,
    )?)
    .map_err(|error| format!("{READINESS_SOURCE_RELATIVE} must be UTF-8: {error}"))?;
    let sequential_jpeg_source = String::from_utf8(read_regular_file(
        workspace_root,
        SEQUENTIAL_JPEG_SOURCE_RELATIVE,
    )?)
    .map_err(|error| format!("{SEQUENTIAL_JPEG_SOURCE_RELATIVE} must be UTF-8: {error}"))?;
    validate_readiness_source_text(&source, &sequential_jpeg_source)?;
    let lib = String::from_utf8(read_regular_file(workspace_root, BLOSSOM_LIB_RELATIVE)?)
        .map_err(|error| format!("{BLOSSOM_LIB_RELATIVE} must be UTF-8: {error}"))?;
    if lib.matches("pub mod publication_readiness;").count() != 1
        || !lib.contains(
            "#[cfg(feature = \"raster-decode\")]\npub use publication_readiness::verify_publication_readiness;",
        )
        || !lib.contains("RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES")
        || !lib.contains("RadrootsBlossomPublicationReadinessEvidence")
    {
        return Err(
            "Blossom crate root must route the readiness module and public API exactly".to_owned(),
        );
    }

    let manifest = parse_toml(workspace_root, BLOSSOM_MANIFEST_RELATIVE)?;
    let dependencies = manifest
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{BLOSSOM_MANIFEST_RELATIVE} must declare dependencies"))?;
    let actual = dependencies
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected = [
        "image",
        "mediatype",
        "serde",
        "sha2",
        "unicode-general-category",
        "url_nostd",
        "zune-core",
        "zune-jpeg",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!(
            "{BLOSSOM_MANIFEST_RELATIVE} dependency boundary drifted: expected {expected:?}, found {actual:?}"
        ));
    }
    let image_dependency = dependencies
        .get("image")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{BLOSSOM_MANIFEST_RELATIVE} must declare optional image"))?;
    if image_dependency
        .get("workspace")
        .and_then(toml::Value::as_bool)
        != Some(true)
        || image_dependency
            .get("optional")
            .and_then(toml::Value::as_bool)
            != Some(true)
    {
        return Err(format!(
            "{BLOSSOM_MANIFEST_RELATIVE} image dependency must be optional and workspace-governed"
        ));
    }
    let zune_core_dependency = dependencies
        .get("zune-core")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{BLOSSOM_MANIFEST_RELATIVE} must declare optional zune-core"))?;
    if zune_core_dependency
        .get("workspace")
        .and_then(toml::Value::as_bool)
        != Some(true)
        || zune_core_dependency
            .get("optional")
            .and_then(toml::Value::as_bool)
            != Some(true)
    {
        return Err(format!(
            "{BLOSSOM_MANIFEST_RELATIVE} zune-core dependency must be optional and workspace-governed"
        ));
    }
    let zune_jpeg_dependency = dependencies
        .get("zune-jpeg")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{BLOSSOM_MANIFEST_RELATIVE} must declare optional zune-jpeg"))?;
    if zune_jpeg_dependency
        .get("workspace")
        .and_then(toml::Value::as_bool)
        != Some(true)
        || zune_jpeg_dependency
            .get("optional")
            .and_then(toml::Value::as_bool)
            != Some(true)
    {
        return Err(format!(
            "{BLOSSOM_MANIFEST_RELATIVE} zune-jpeg dependency must be optional and workspace-governed"
        ));
    }
    let workspace_manifest = parse_toml(workspace_root, WORKSPACE_MANIFEST_RELATIVE)?;
    let workspace_image = workspace_manifest
        .get("workspace")
        .and_then(|value| value.get("dependencies"))
        .and_then(|value| value.get("image"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{WORKSPACE_MANIFEST_RELATIVE} must govern image"))?;
    let workspace_image_features = workspace_image
        .get("features")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .collect::<BTreeSet<_>>();
    if workspace_image.get("version").and_then(toml::Value::as_str) != Some("=0.25.10")
        || workspace_image
            .get("default-features")
            .and_then(toml::Value::as_bool)
            != Some(false)
        || workspace_image_features != BTreeSet::from(["png", "webp"])
    {
        return Err(format!(
            "{WORKSPACE_MANIFEST_RELATIVE} image decoder dependency must be exactly pinned to the PNG/WebP set"
        ));
    }
    let workspace_zune_core = workspace_manifest
        .get("workspace")
        .and_then(|value| value.get("dependencies"))
        .and_then(|value| value.get("zune-core"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{WORKSPACE_MANIFEST_RELATIVE} must govern zune-core"))?;
    let workspace_zune_core_features = workspace_zune_core
        .get("features")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .collect::<BTreeSet<_>>();
    if workspace_zune_core
        .get("version")
        .and_then(toml::Value::as_str)
        != Some("=0.5.1")
        || workspace_zune_core
            .get("default-features")
            .and_then(toml::Value::as_bool)
            != Some(false)
        || workspace_zune_core_features != BTreeSet::from(["std"])
    {
        return Err(format!(
            "{WORKSPACE_MANIFEST_RELATIVE} JPEG decoder core must be exactly pinned to zune-core 0.5.1 with std only"
        ));
    }
    let workspace_zune_jpeg = workspace_manifest
        .get("workspace")
        .and_then(|value| value.get("dependencies"))
        .and_then(|value| value.get("zune-jpeg"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{WORKSPACE_MANIFEST_RELATIVE} must govern zune-jpeg"))?;
    let workspace_zune_jpeg_features = workspace_zune_jpeg
        .get("features")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .collect::<BTreeSet<_>>();
    if workspace_zune_jpeg
        .get("version")
        .and_then(toml::Value::as_str)
        != Some("=0.5.15")
        || workspace_zune_jpeg
            .get("default-features")
            .and_then(toml::Value::as_bool)
            != Some(false)
        || workspace_zune_jpeg_features != BTreeSet::from(["std"])
    {
        return Err(format!(
            "{WORKSPACE_MANIFEST_RELATIVE} strict JPEG authority must be exactly pinned to zune-jpeg 0.5.15 with std only"
        ));
    }
    let features = manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{BLOSSOM_MANIFEST_RELATIVE} must declare features"))?;
    let raster_decode = features
        .get("raster-decode")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{BLOSSOM_MANIFEST_RELATIVE} must declare raster-decode feature"))?
        .iter()
        .filter_map(toml::Value::as_str)
        .collect::<BTreeSet<_>>();
    if raster_decode != BTreeSet::from(["dep:image", "dep:zune-core", "dep:zune-jpeg", "std"]) {
        return Err(format!(
            "{BLOSSOM_MANIFEST_RELATIVE} raster-decode feature must select only std, image, zune-core, and zune-jpeg"
        ));
    }
    let coverage = parse_toml(workspace_root, COVERAGE_PROFILES_RELATIVE)?;
    let blossom_coverage = coverage
        .get("profiles")
        .and_then(|value| value.get("crates"))
        .and_then(|value| value.get("radroots_blossom"))
        .ok_or_else(|| {
            format!("{COVERAGE_PROFILES_RELATIVE} must declare radroots_blossom coverage")
        })?;
    let coverage_features = blossom_coverage
        .get("features")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .collect::<BTreeSet<_>>();
    if blossom_coverage
        .get("no_default_features")
        .and_then(toml::Value::as_bool)
        != Some(true)
        || coverage_features != BTreeSet::from(["raster-decode", "serde"])
    {
        return Err(format!(
            "{COVERAGE_PROFILES_RELATIVE} must measure the explicit Blossom raster-decode surface"
        ));
    }
    let nix_common = String::from_utf8(read_regular_file(workspace_root, NIX_COMMON_RELATIVE)?)
        .map_err(|error| format!("{NIX_COMMON_RELATIVE} must be UTF-8: {error}"))?;
    if !nix_common.contains("radroots_blossom/raster-decode") {
        return Err(format!(
            "{NIX_COMMON_RELATIVE} core contract lane must enable raster-decode"
        ));
    }
    let nix_checks = String::from_utf8(read_regular_file(workspace_root, NIX_CHECKS_RELATIVE)?)
        .map_err(|error| format!("{NIX_CHECKS_RELATIVE} must be UTF-8: {error}"))?;
    let nix_check_commands = nix_checks.lines().map(str::trim).collect::<BTreeSet<_>>();
    for required in [
        "cargo check -p radroots_blossom --lib --no-default-features",
        "cargo check -p radroots_blossom --lib --no-default-features --features raster-decode",
        "cargo test -p radroots_blossom --no-default-features --features raster-decode,serde",
    ] {
        if !nix_check_commands.contains(required) {
            return Err(format!(
                "{NIX_CHECKS_RELATIVE} lacks governed Blossom verification `{required}`"
            ));
        }
    }
    let raw_predecessor = String::from_utf8(read_regular_file(
        workspace_root,
        RAW_PREDECESSOR_GOVERNANCE_RELATIVE,
    )?)
    .map_err(|error| format!("{RAW_PREDECESSOR_GOVERNANCE_RELATIVE} must be UTF-8: {error}"))?;
    validate_raw_predecessor_successor_routing(&raw_predecessor)?;
    Ok(())
}

fn validate_raw_predecessor_successor_routing(source: &str) -> Result<(), String> {
    let compact = source.split_whitespace().collect::<String>();
    for required in [
        "pub(crate)fnwrite_raw_source_rebuild_manifest(workspace_root:&Path)->Result<(),String>{validate_raw_source_rebuild_manifest(workspace_root)}",
        "super::blossom_publication_readiness::validate_blossom_publication_readiness(workspace_root)",
        "constBLOSSOM_READINESS_SUCCESSOR_TRANSITIVE_PATHS:&[&str]=&[\"Cargo.toml\",\"crates/blossom/Cargo.toml\",\"crates/blossom/src/error.rs\",\"crates/blossom/src/lib.rs\",\"crates/blossom/src/url.rs\",];",
    ] {
        if !compact.contains(required) {
            return Err(format!(
                "{RAW_PREDECESSOR_GOVERNANCE_RELATIVE} lacks validation-only successor route `{required}`"
            ));
        }
    }
    Ok(())
}

fn validate_readiness_source_text(
    source: &str,
    sequential_jpeg_source: &str,
) -> Result<(), String> {
    for required in [
        "RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION: u16 = 1",
        "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES: u64 = 10_485_760",
        "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES: u64 =",
        "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION: u32 = 16_384",
        "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_PIXELS: u64 = 20_000_000",
        "#[cfg(feature = \"raster-decode\")]\npub fn verify_publication_readiness(",
        "use zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions};",
        "use zune_jpeg::JpegDecoder as StrictJpegDecoder;",
        "mod sequential_jpeg;",
        "sequential_jpeg::validate(bytes, container)?;",
        "StrictJpegDecoder::new_with_options(ZCursor::new(bytes), strict_jpeg_decoder_options())",
        ".set_strict_mode(true)",
        ".set_use_unsafe(false)",
        ".jpeg_set_out_colorspace(ColorSpace::RGB)",
        ".decode_headers()",
        ".output_buffer_size()",
        ".decode_into(&mut decoded)",
        "if !matches!(marker, 0xc0 | 0xc1) || data[0] != 8",
        "PublicationJpegProcessForbidden",
        "PngDecoder::with_limits(Cursor::new(bytes), raster_decode_limits())",
        "WebPDecoder::new(Cursor::new(bytes))",
        ".read_image(&mut decoded)",
        "b\"radroots.blossom.publication-readiness-evidence.v1\\0\"",
    ] {
        if !source.contains(required) {
            return Err(format!(
                "{READINESS_SOURCE_RELATIVE} is missing governed fragment `{required}`"
            ));
        }
    }
    for required in [
        "struct SequentialJpegHuffmanTable",
        "struct SequentialJpegEntropyReader",
        "sampling_product_sum > 10",
        "value_count > 256",
        "seen_values[value_index]",
        "unused_codes == 0",
        "payload[payload.len() - 3..] != [0, 63, 0]",
        "reader.finish_restart(expected_restart)?",
        "seen_components[component.frame_index] = true",
        "checked_mcu_grid_count",
    ] {
        if !sequential_jpeg_source.contains(required) {
            return Err(format!(
                "{SEQUENTIAL_JPEG_SOURCE_RELATIVE} is missing governed fragment `{required}`"
            ));
        }
    }
    let combined = format!("{source}\n{sequential_jpeg_source}");
    let lowercase = combined.to_ascii_lowercase();
    for forbidden in [
        "reqwest",
        "hyper::",
        "tokio::",
        "axum::",
        "std::net",
        "std::fs",
        "authorization: nostr",
        "bearer ",
        "cookie",
    ] {
        if lowercase.contains(forbidden) {
            return Err(format!(
                "{READINESS_SOURCE_RELATIVE} crosses the transport-neutral boundary with `{forbidden}`"
            ));
        }
    }
    if combined.contains("serde::Deserialize") || combined.contains("derive(Deserialize") {
        return Err(
            "publication readiness typestates must not gain forgeable Deserialize implementations"
                .to_owned(),
        );
    }
    if combined.contains("pub struct RadrootsBlossomRasterDecodeObservation")
        || combined.contains("decode: &RadrootsBlossom")
        || combined.contains("ImageReader")
        || combined.contains("load_from_memory")
        || combined.contains("JpegDecoder::new(Cursor::new(bytes))")
        || combined.contains("push_backend(")
        || combined.contains("gamut_core")
        || combined.contains("gamut_jpeg")
        || combined.contains("jpeg_decoder::")
        || combined.contains("use jpeg_decoder")
        || combined.contains("extern crate jpeg_decoder")
        || combined.contains("zenjpeg")
    {
        return Err(
            "publication readiness must force declared-format decode authority internally from exact bytes"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_vector(workspace_root: &Path) -> Result<(), String> {
    let canonical = read_regular_file(workspace_root, VECTOR_CANONICAL_RELATIVE)?;
    let mirror = read_regular_file(workspace_root, VECTOR_MIRROR_RELATIVE)?;
    if canonical != mirror {
        return Err(format!(
            "{VECTOR_MIRROR_RELATIVE} must byte-match {VECTOR_CANONICAL_RELATIVE}"
        ));
    }
    let vector: Value = serde_json::from_slice(&canonical)
        .map_err(|error| format!("parse {VECTOR_CANONICAL_RELATIVE}: {error}"))?;
    validate_vector_value(&vector)
}

fn validate_vector_value(vector: &Value) -> Result<(), String> {
    if vector.get("suite").and_then(Value::as_str) != Some("blossom_publication_readiness")
        || vector.get("contract_version").and_then(Value::as_str) != Some("1.0.0")
    {
        return Err("publication-readiness vector identity drifted".to_owned());
    }
    let cases = vector
        .get("vectors")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication-readiness vectors must be an array".to_owned())?;
    if cases.len() != VECTOR_EXPECTATIONS.len() {
        return Err(format!(
            "publication-readiness vector count drifted: expected {}, found {}",
            VECTOR_EXPECTATIONS.len(),
            cases.len()
        ));
    }
    for (case, (id, kind, mutation, expected_error)) in cases.iter().zip(VECTOR_EXPECTATIONS) {
        let actual_id = case.get("id").and_then(Value::as_str);
        let actual_kind = case.get("kind").and_then(Value::as_str);
        let actual_mutation = case
            .get("input")
            .and_then(|input| input.get("mutation"))
            .and_then(Value::as_str);
        let actual_error = case
            .get("expected")
            .and_then(|expected| expected.get("error"))
            .and_then(Value::as_str);
        if actual_id != Some(*id)
            || actual_kind != Some(*kind)
            || actual_mutation != Some(*mutation)
            || actual_error != *expected_error
        {
            return Err(format!(
                "publication-readiness vector `{id}` identity or expected error drifted"
            ));
        }
        let bytes_hex = case
            .get("input")
            .and_then(|input| input.get("bytes_hex"))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("publication-readiness vector `{id}` lacks bytes_hex"))?;
        if bytes_hex.len() != 140
            || !bytes_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!(
                "publication-readiness vector `{id}` must bind the exact 70-byte lowercase-hex raster"
            ));
        }
    }
    Ok(())
}

fn validate_operation(workspace_root: &Path) -> Result<(), String> {
    let manifest = parse_toml(workspace_root, OPERATIONS_RELATIVE)?;
    let operation = manifest
        .get("operations")
        .and_then(|value| value.get("blossom_verify_publication_readiness"))
        .ok_or_else(|| "operations contract lacks Blossom publication readiness".to_owned())?;
    if operation.get("domain").and_then(toml::Value::as_str) != Some("blossom")
        || operation.get("id").and_then(toml::Value::as_str)
            != Some("blossom.verify_publication_readiness")
        || operation.get("transport").and_then(toml::Value::as_str) != Some("none")
        || operation.get("signing").and_then(toml::Value::as_str) != Some("none")
        || operation
            .get("deterministic")
            .and_then(toml::Value::as_bool)
            != Some(true)
    {
        return Err("Blossom publication-readiness operation authority drifted".to_owned());
    }
    let shared = manifest
        .get("shared_types")
        .and_then(|value| value.get("public"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "operations contract shared public types are missing".to_owned())?
        .iter()
        .filter_map(toml::Value::as_str)
        .collect::<BTreeSet<_>>();
    if let Some(missing) = REQUIRED_PUBLIC_TYPES
        .iter()
        .find(|required| !shared.contains(**required))
    {
        return Err(format!(
            "operations contract lacks readiness public type `{missing}`"
        ));
    }
    if shared.contains("RadrootsBlossomRasterDecodeObservation") {
        return Err(
            "operations contract must not expose caller-constructible raster decode authority"
                .to_owned(),
        );
    }
    let inputs = operation
        .get("inputs")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .collect::<BTreeSet<_>>();
    let expected_inputs = BTreeSet::from([
        "Bytes",
        "RadrootsBlossomAuthoredRasterDimensions",
        "RadrootsBlossomBud01GetObservation",
        "RadrootsBlossomBud01HeadObservation",
        "RadrootsBlossomBud02UploadObservation",
        "RadrootsBlossomByteVerifiedDescriptor",
    ]);
    if inputs != expected_inputs {
        return Err(
            "Blossom publication-readiness inputs must contain transport evidence and exact bytes only"
                .to_owned(),
        );
    }
    let rust_types = operation
        .get("implementation")
        .and_then(|value| value.get("rust_types"))
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .collect::<BTreeSet<_>>();
    if rust_types.contains("radroots_blossom::RadrootsBlossomRasterDecodeObservation") {
        return Err(
            "Blossom publication-readiness implementation must derive decode facts internally"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_release(workspace_root: &Path) -> Result<(), String> {
    let release = parse_toml(workspace_root, RELEASE_RELATIVE)?;
    let changes = release
        .get("changes")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{RELEASE_RELATIVE} must declare changes"))?;
    let matches = changes
        .iter()
        .filter(|change| change.get("id").and_then(toml::Value::as_str) == Some(RELEASE_CHANGE_ID))
        .collect::<Vec<_>>();
    if matches.len() != 1
        || matches[0]
            .get("classification")
            .and_then(toml::Value::as_str)
            != Some("feature")
    {
        return Err(format!(
            "{RELEASE_RELATIVE} must contain one feature change `{RELEASE_CHANGE_ID}`"
        ));
    }
    let changelog = String::from_utf8(read_regular_file(workspace_root, CHANGELOG_RELATIVE)?)
        .map_err(|error| format!("{CHANGELOG_RELATIVE} must be UTF-8: {error}"))?;
    if changelog.matches(CHANGELOG_MARKER).count() != 1 {
        return Err(format!(
            "{CHANGELOG_RELATIVE} must contain exactly one readiness release marker"
        ));
    }
    Ok(())
}

fn parse_toml(workspace_root: &Path, relative: &str) -> Result<toml::Value, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{relative} must be UTF-8 TOML: {error}"))?;
    toml::from_str(source).map_err(|error| format!("parse {relative}: {error}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("String writes cannot fail");
    }
    output
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
    fn publication_readiness_vector_inventory_is_exact_and_mutation_sensitive() {
        let bytes = read_regular_file(&workspace_root(), VECTOR_CANONICAL_RELATIVE).unwrap();
        let mut vector: Value = serde_json::from_slice(&bytes).unwrap();
        validate_vector_value(&vector).unwrap();
        vector["vectors"][0]["input"]["mutation"] = Value::String("renamed".to_owned());
        assert!(
            validate_vector_value(&vector)
                .unwrap_err()
                .contains("valid_created")
        );
    }

    #[test]
    fn publication_readiness_source_boundary_rejects_transport_and_contract_drift() {
        let source = String::from_utf8(
            read_regular_file(&workspace_root(), READINESS_SOURCE_RELATIVE).unwrap(),
        )
        .unwrap();
        let sequential_jpeg_source = String::from_utf8(
            read_regular_file(&workspace_root(), SEQUENTIAL_JPEG_SOURCE_RELATIVE).unwrap(),
        )
        .unwrap();
        validate_readiness_source_text(&source, &sequential_jpeg_source).unwrap();
        let injected = format!("{source}\nfn injected() {{ let _ = reqwest::get; }}\n");
        assert!(
            validate_readiness_source_text(&injected, &sequential_jpeg_source)
                .unwrap_err()
                .contains("transport-neutral")
        );
        let removed = source.replace(
            "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_PIXELS: u64 = 20_000_000",
            "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_PIXELS: u64 = 20_000_001",
        );
        assert!(
            validate_readiness_source_text(&removed, &sequential_jpeg_source)
                .unwrap_err()
                .contains("missing governed fragment")
        );
        let weakened_jpeg = sequential_jpeg_source
            .replace("sampling_product_sum > 10", "sampling_product_sum > 16");
        assert!(
            validate_readiness_source_text(&source, &weakened_jpeg)
                .unwrap_err()
                .contains("missing governed fragment")
        );
        let legacy_decoder = format!("{source}\nuse jpeg_decoder::Decoder;\n");
        assert!(
            validate_readiness_source_text(&legacy_decoder, &sequential_jpeg_source)
                .unwrap_err()
                .contains("decode authority")
        );

        for (relative, expected_length, expected_sha256) in CURRENT_BYTE_BOUND_BLOSSOM_SOURCES {
            let mut bytes = read_regular_file(&workspace_root(), relative).unwrap();
            validate_current_byte_bound_blossom_source(
                relative,
                &bytes,
                *expected_length,
                expected_sha256,
            )
            .unwrap();
            bytes.push(b' ');
            assert!(
                validate_current_byte_bound_blossom_source(
                    relative,
                    &bytes,
                    *expected_length,
                    expected_sha256,
                )
                .unwrap_err()
                .contains("current-byte source")
            );
        }
    }

    #[test]
    fn raw_predecessor_supersession_rejects_unknown_paths() {
        let error = validate_raw_source_rebuild_predecessor_production_sources_under_lock(
            &workspace_root(),
            &["not/a/predecessor.rs"],
            &[],
        )
        .unwrap_err();
        assert!(error.contains("not predecessor-bound"), "{error}");
    }

    #[test]
    fn retired_raw_predecessor_write_route_is_validation_only() {
        let root = workspace_root();
        let before = IMMUTABLE_RAW_PREDECESSOR_ARTIFACTS
            .iter()
            .map(|(relative, _, _)| {
                (
                    *relative,
                    read_regular_file(&root, relative).expect("immutable raw predecessor artifact"),
                )
            })
            .collect::<Vec<_>>();
        super::super::raw_source_rebuild::write_raw_source_rebuild_manifest(&root)
            .expect("retired raw predecessor write route validates its active successor");
        for (relative, expected) in before {
            assert_eq!(
                read_regular_file(&root, relative).expect("raw predecessor after validation"),
                expected,
                "retired raw predecessor writer mutated {relative}"
            );
        }

        let source = String::from_utf8(
            read_regular_file(&root, RAW_PREDECESSOR_GOVERNANCE_RELATIVE).unwrap(),
        )
        .unwrap();
        validate_raw_predecessor_successor_routing(&source).unwrap();
        let bypass = source.replacen(
            "validate_raw_source_rebuild_manifest(workspace_root)",
            "Ok(())",
            1,
        );
        assert!(
            validate_raw_predecessor_successor_routing(&bypass)
                .unwrap_err()
                .contains("validation-only successor route")
        );
    }
}
