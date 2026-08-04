use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README.md");
const EXAMPLE: &str = include_str!("../examples/prepare_query.rs");
const PUBLIC_API: &str = include_str!("../../../docs/api/radroots_geonames.txt");
const API_INDEX: &str = include_str!("../../../docs/api/README.md");
const ROOT: &str = include_str!("../src/lib.rs");
const LEGACY_MANIFEST: &str = include_str!("../../geocoder/Cargo.toml");
const LEGACY_README: &str = include_str!("../../geocoder/README");
const COMPATIBILITY: &str = include_str!("../../../docs/implementation/COMPATIBILITY_SHIMS.md");
const DEVIATIONS: &str = include_str!("../../../docs/implementation/deviations.toml");
const PUBLISH_POLICY: &str = include_str!("../../../contracts/releases/publish_policy.toml");

#[test]
fn manifest_and_root_match_the_governed_provider_boundary() {
    for required in [
        "name = \"radroots_geonames\"",
        "version = \"0.1.0-alpha\"",
        "publish = [\"crates-io\"]",
        "[lib]\nname = \"radroots_geonames\"",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing `{required}`"
        );
    }
    assert!(!MANIFEST.contains("[features]"));
    assert!(radroots_dependency_keys(MANIFEST).is_empty());
    assert_eq!(
        public_modules(ROOT),
        BTreeSet::from(["asset", "database", "download", "model", "query"])
    );
    assert_eq!(private_modules(ROOT), BTreeSet::from(["error"]));
    for export in [
        "pub use asset::{AssetSpec, AssetStatus};",
        "pub use database::Geocoder;",
        "pub use error::Error;",
        "pub use model::{Candidate, Point};",
        "pub use query::Query;",
    ] {
        assert!(ROOT.contains(export), "crate root is missing `{export}`");
    }
}

#[test]
fn documentation_example_and_reviewed_api_baseline_are_complete() {
    for required in [
        "## Prepare a query without I/O",
        "## Asset identity and acquisition",
        "## Database and query behavior",
        "## Errors, serialization, and side effects",
        "## Intended consumers",
        "radroots_crates_release_v1.md#17-radroots_geonames",
        "examples/prepare_query.rs",
        "docs/api/radroots_geonames.txt",
    ] {
        assert!(README.contains(required), "README is missing `{required}`");
    }
    for required in [
        "official_asset_spec()",
        "Query::locality(\"Victoria\")",
        "Query::reverse(Point::new(",
        "without I/O",
    ] {
        assert!(
            EXAMPLE.contains(required),
            "example is missing `{required}`"
        );
    }
    for required in [
        "pub struct radroots_geonames::AssetSpec",
        "pub enum radroots_geonames::AssetStatus",
        "pub struct radroots_geonames::Candidate",
        "pub struct radroots_geonames::Geocoder",
        "pub struct radroots_geonames::Point",
        "pub struct radroots_geonames::Query",
        "pub enum radroots_geonames::Error",
        "Geocoder::open",
        "Geocoder::query",
        "Geocoder::close",
    ] {
        assert!(
            PUBLIC_API.contains(required),
            "public API is missing `{required}`"
        );
    }
    for forbidden in [
        "rusqlite",
        "sqlx",
        "reqwest",
        "tokio",
        "runtime_paths",
        "GeocoderLocality",
        "GeoNamesAsset",
    ] {
        assert!(
            !PUBLIC_API.contains(forbidden),
            "public API exposes `{forbidden}`"
        );
    }
    assert!(
        API_INDEX
            .contains("| `radroots_geonames` | [`radroots_geonames.txt`](radroots_geonames.txt) |")
    );
}

#[test]
fn provider_source_has_no_hidden_runtime_path_or_download_implementation() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let source = fs::read_dir(source_root)
        .expect("source directory")
        .map(|entry| {
            let path = entry.expect("source entry").path();
            fs::read_to_string(path).expect("utf-8 source")
        })
        .collect::<String>();
    for forbidden in [
        "radroots_runtime_paths",
        "reqwest::",
        "tokio::",
        "sqlx::",
        "std::env::",
        "directories::",
        "test-fixture-geonames-asset",
    ] {
        assert!(
            !source.contains(forbidden),
            "provider source contains `{forbidden}`"
        );
    }
}

#[test]
fn superseded_geocoder_is_a_bounded_publish_frozen_sdk_bridge() {
    for required in [
        "name = \"radroots_geocoder\"",
        "publish = false",
        "status = \"publish_frozen\"",
        "replacement = \"radroots_geonames\"",
        "deviation = \"RCRV1-DEV-011\"",
        "removal_step = 248",
        "new_consumers_forbidden = true",
    ] {
        assert!(
            LEGACY_MANIFEST.contains(required),
            "legacy manifest is missing `{required}`"
        );
    }
    for required in [
        "## Compatibility quarantine",
        "RCRV1-DEV-011",
        "Step 226",
        "Step 248",
    ] {
        assert!(
            LEGACY_README.contains(required),
            "legacy README is missing `{required}`"
        );
    }
    for required in [
        "| `radroots_geocoder` | `radroots_geonames` |",
        "SDK manifest cutover Step 226",
        "SDK quarantine removal Step 248",
    ] {
        assert!(
            COMPATIBILITY.contains(required),
            "quarantine ledger is missing `{required}`"
        );
    }
    assert!(DEVIATIONS.contains("id = \"RCRV1-DEV-011\""));
    let private = PUBLISH_POLICY
        .split_once("[workspace_classification]")
        .expect("workspace classification")
        .1
        .split_once("build_codegen")
        .expect("private classification")
        .0;
    assert!(private.contains("\"radroots_geocoder\""));
}

fn radroots_dependency_keys(manifest: &str) -> BTreeSet<&str> {
    dependency_keys(manifest)
        .into_iter()
        .filter(|key| key.starts_with("radroots_"))
        .collect()
}

fn dependency_keys(manifest: &str) -> BTreeSet<&str> {
    manifest
        .split_once("[dependencies]")
        .map(|(_, dependencies)| dependencies)
        .unwrap_or_default()
        .lines()
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim()))
        .filter(|key| !key.is_empty())
        .collect()
}

fn public_modules(root: &str) -> BTreeSet<&str> {
    root.lines()
        .filter_map(|line| line.trim().strip_prefix("pub mod "))
        .filter_map(|module| module.strip_suffix(';'))
        .collect()
}

fn private_modules(root: &str) -> BTreeSet<&str> {
    root.lines()
        .filter_map(|line| line.trim().strip_prefix("mod "))
        .filter_map(|module| module.strip_suffix(';'))
        .collect()
}
