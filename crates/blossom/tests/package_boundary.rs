use std::collections::BTreeSet;

use radroots_blossom::media_type::RadrootsBlossomMediaType;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn manifest_has_final_identity_features_and_no_radroots_dependencies() {
    assert!(MANIFEST.contains("name = \"radroots_blossom\""));
    assert!(MANIFEST.contains("version = \"0.1.0\""));
    assert!(MANIFEST.contains("publish = false"));
    assert!(MANIFEST.contains("[lib]\nname = \"radroots_blossom\""));
    assert!(MANIFEST.contains("default = [\"std\", \"serde\"]"));
    assert_eq!(
        table_keys(MANIFEST, "[features]"),
        BTreeSet::from(["default", "serde", "std"])
    );

    for heading in ["[dependencies]", "[dev-dependencies]"] {
        assert!(
            table_keys(MANIFEST, heading)
                .iter()
                .all(|dependency| !dependency.starts_with("radroots_")),
            "{heading} must not contain Radroots dependencies"
        );
    }
}

#[test]
fn crate_root_matches_the_approved_module_skeleton() {
    assert!(ROOT.contains("#![cfg_attr(not(feature = \"std\"), no_std)]"));
    assert_eq!(
        root_declarations("pub mod "),
        BTreeSet::from(["authorization", "descriptor", "hash", "media_type", "url"])
    );
    assert_eq!(root_declarations("mod "), BTreeSet::from(["error"]));
}

#[test]
fn final_crate_and_media_type_module_paths_compile() {
    fn assert_public_value<T: Clone + core::fmt::Debug + Eq + Send + Sync>() {}

    assert_public_value::<RadrootsBlossomMediaType>();
}

fn table_keys<'a>(manifest: &'a str, heading: &str) -> BTreeSet<&'a str> {
    let table = manifest
        .split_once(heading)
        .unwrap_or_else(|| panic!("missing manifest table {heading}"))
        .1;
    table
        .lines()
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .filter_map(|line| {
            let line = line.trim();
            (!line.is_empty() && !line.starts_with('#'))
                .then(|| line.split_once('=').map(|(key, _)| key.trim()))
                .flatten()
        })
        .collect()
}

fn root_declarations(prefix: &str) -> BTreeSet<&str> {
    ROOT.lines()
        .map(str::trim)
        .filter_map(|line| {
            line.strip_prefix(prefix)
                .and_then(|name| name.strip_suffix(';'))
        })
        .collect()
}
