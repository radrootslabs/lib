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
const READINESS_SOURCE_RELATIVE: &str = "crates/blossom/src/publication_readiness.rs";
const BLOSSOM_LIB_RELATIVE: &str = "crates/blossom/src/lib.rs";
const BLOSSOM_MANIFEST_RELATIVE: &str = "crates/blossom/Cargo.toml";
const OPERATIONS_RELATIVE: &str = "contracts/operations.toml";
const RELEASE_RELATIVE: &str = "contracts/releases/1.0.0-alpha.1.toml";
const CHANGELOG_RELATIVE: &str = "CHANGELOG.md";
const RAW_PREDECESSOR_GOVERNANCE_RELATIVE: &str = "tools/xtask/src/contract/raw_source_rebuild.rs";
const RELEASE_CHANGE_ID: &str = "blossom-publication-readiness-evidence";
const CHANGELOG_MARKER: &str = "<!-- release-change: blossom-publication-readiness-evidence -->";

const RAW_PREDECESSOR_SUPERSEDED_PATHS: &[&str] = &[
    CHANGELOG_RELATIVE,
    RELEASE_RELATIVE,
    "tools/xtask/src/contract.rs",
    "tools/xtask/src/contract/food_availability_projection.rs",
    "tools/xtask/src/contract/nip09_reconciliation.rs",
    RAW_PREDECESSOR_GOVERNANCE_RELATIVE,
];
const TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS: &[&str] =
    &["crates/blossom/src/error.rs", BLOSSOM_LIB_RELATIVE];

const SOURCE_INVENTORY: &[&str] = &[
    CHANGELOG_RELATIVE,
    BLOSSOM_MANIFEST_RELATIVE,
    "crates/blossom/README",
    "crates/blossom/src/error.rs",
    BLOSSOM_LIB_RELATIVE,
    READINESS_SOURCE_RELATIVE,
    "crates/blossom/tests/publication_readiness.rs",
    VECTOR_MIRROR_RELATIVE,
    "contracts/events/blossom-media.md",
    VECTOR_CANONICAL_RELATIVE,
    OPERATIONS_RELATIVE,
    RELEASE_RELATIVE,
    "tools/xtask/src/contract.rs",
    "tools/xtask/src/contract/blossom_publication_readiness.rs",
    "tools/xtask/src/contract/food_availability_projection.rs",
    "tools/xtask/src/contract/nip09_reconciliation.rs",
    RAW_PREDECESSOR_GOVERNANCE_RELATIVE,
];

const IMMUTABLE_RAW_PREDECESSOR_ARTIFACTS: &[(&str, usize, &str)] = &[
    (
        "crates/event_store/contracts/raw_source_rebuild_v1.manifest.json",
        45_449,
        "b8737a9c5836517114e7df6c2194c46e3c200093e12c4e6297165d2b9dae56a1",
    ),
    (
        "crates/event_store/contracts/raw_source_rebuild_v1.manifest.schema.json",
        17_896,
        "f9d210967e54b66f39c8bb965d97b2001a0ebc0927e7c2c14edb8e474bfda695",
    ),
    (
        "crates/event_store/contracts/raw_source_rebuild_v1.manifest.sha256",
        65,
        "737ee2e4ecd400e1c647e80422c432cd2955d7c7cc04fdf3f9993551480e7957",
    ),
    (
        "crates/event_store/src/generated/raw_source_rebuild_manifest.rs",
        50_735,
        "20ad0d83304bb4ea3aeb0b37fc068891f1f2e5a0c3abc93d1a9932770330307c",
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
        2_088,
        "ab0431ba43619431f4384a474c1e8b7e3e646a802443d66cf992df467fa9c36b",
    ),
    (
        "crates/blossom/src/error.rs",
        30_667,
        "2d30f4e21d71b6978cb3cc564d5ab542d238cf56c2eab89801fce8d1421bccf6",
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
    "RadrootsBlossomRasterDecodeObservation",
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
        Some("publication_raster_frame_count_mismatch"),
    ),
    (
        "decode_format_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "decode_format_mismatch",
        Some("publication_raster_decode_format_mismatch"),
    ),
    (
        "decode_length_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "decode_length_mismatch",
        Some("publication_raster_decode_length_mismatch"),
    ),
    (
        "decode_hash_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "decode_hash_mismatch",
        Some("publication_raster_decode_hash_mismatch"),
    ),
    (
        "decode_container_dimension_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "decode_container_dimension_mismatch",
        Some("publication_raster_container_dimension_mismatch"),
    ),
    (
        "authored_dimension_mismatch",
        "blossom.verify_publication_readiness.invalid",
        "authored_dimension_mismatch",
        Some("publication_authored_raster_dimension_mismatch"),
    ),
    (
        "decode_zero_frames",
        "blossom.verify_publication_readiness.invalid",
        "decode_zero_frames",
        Some("publication_raster_frame_count_mismatch"),
    ),
    (
        "decode_zero_width",
        "blossom.verify_publication_readiness.invalid",
        "decode_zero_width",
        Some("publication_raster_dimensions_out_of_range"),
    ),
    (
        "decode_dimension_over_max",
        "blossom.verify_publication_readiness.invalid",
        "decode_dimension_over_max",
        Some("publication_raster_dimensions_out_of_range"),
    ),
    (
        "decode_pixel_limit",
        "blossom.verify_publication_readiness.invalid",
        "decode_pixel_limit",
        Some("publication_raster_pixel_limit_exceeded"),
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
    validate_readiness_source_text(&source)?;
    let lib = String::from_utf8(read_regular_file(workspace_root, BLOSSOM_LIB_RELATIVE)?)
        .map_err(|error| format!("{BLOSSOM_LIB_RELATIVE} must be UTF-8: {error}"))?;
    if lib.matches("pub mod publication_readiness;").count() != 1
        || !lib.contains("verify_publication_readiness")
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
        "mediatype",
        "serde",
        "sha2",
        "unicode-general-category",
        "url_nostd",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!(
            "{BLOSSOM_MANIFEST_RELATIVE} dependency boundary drifted: expected {expected:?}, found {actual:?}"
        ));
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
        "constBLOSSOM_READINESS_SUCCESSOR_TRANSITIVE_PATHS:&[&str]=&[\"crates/blossom/src/error.rs\",\"crates/blossom/src/lib.rs\"];",
    ] {
        if !compact.contains(required) {
            return Err(format!(
                "{RAW_PREDECESSOR_GOVERNANCE_RELATIVE} lacks validation-only successor route `{required}`"
            ));
        }
    }
    Ok(())
}

fn validate_readiness_source_text(source: &str) -> Result<(), String> {
    for required in [
        "RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION: u16 = 1",
        "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES: u64 = 10_485_760",
        "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION: u32 = 16_384",
        "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_PIXELS: u64 = 20_000_000",
        "pub fn verify_publication_readiness(",
        "b\"radroots.blossom.publication-readiness-evidence.v1\\0\"",
    ] {
        if !source.contains(required) {
            return Err(format!(
                "{READINESS_SOURCE_RELATIVE} is missing governed fragment `{required}`"
            ));
        }
    }
    let lowercase = source.to_ascii_lowercase();
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
    if source.contains("serde::Deserialize") || source.contains("derive(Deserialize") {
        return Err(
            "publication readiness typestates must not gain forgeable Deserialize implementations"
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
        validate_readiness_source_text(&source).unwrap();
        let injected = format!("{source}\nfn injected() {{ let _ = reqwest::get; }}\n");
        assert!(
            validate_readiness_source_text(&injected)
                .unwrap_err()
                .contains("transport-neutral")
        );
        let removed = source.replace(
            "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_PIXELS: u64 = 20_000_000",
            "RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_PIXELS: u64 = 20_000_001",
        );
        assert!(
            validate_readiness_source_text(&removed)
                .unwrap_err()
                .contains("missing governed fragment")
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
