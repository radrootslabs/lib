use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const CAPABILITY: &str = include_str!("../src/capability.rs");
const ERROR: &str = include_str!("../src/error.rs");
const EVENT: &str = include_str!("../src/event.rs");
const RADROOTSD: &str = include_str!("../src/radrootsd.rs");
const RUNTIME: &str = include_str!("../src/runtime.rs");

#[test]
fn manifest_has_final_identity_features_and_no_dependencies() {
    assert!(MANIFEST.contains("name = \"radroots_protocol\""));
    assert!(MANIFEST.contains("version = \"0.1.0\""));
    assert!(MANIFEST.contains("publish = false"));
    assert!(MANIFEST.contains("[lib]\nname = \"radroots_protocol\""));
    assert!(MANIFEST.contains("default = [\"std\", \"serde\"]"));
    assert_eq!(
        table_keys(MANIFEST, "[features]"),
        BTreeSet::from(["default", "serde", "std"])
    );
    assert_eq!(table_keys(MANIFEST, "[dependencies]"), BTreeSet::new());
    assert_eq!(table_keys(MANIFEST, "[dev-dependencies]"), BTreeSet::new());
}

#[test]
fn crate_root_exposes_only_the_approved_versioned_skeleton() {
    assert!(ROOT.contains("#![cfg_attr(not(feature = \"std\"), no_std)]"));
    assert_eq!(
        root_declarations("pub mod "),
        BTreeSet::from([
            "capability",
            "error",
            "event",
            "radrootsd",
            "runtime",
            "schema"
        ])
    );
    for source in [CAPABILITY, ERROR, EVENT, RUNTIME] {
        assert!(source.lines().any(|line| line.trim() == "pub mod v1 {}"));
    }
    assert!(RADROOTSD.contains("pub mod transport_publish {"));
    assert!(RADROOTSD.lines().any(|line| line.trim() == "pub mod v5 {}"));
    assert!(
        !ROOT
            .lines()
            .map(str::trim)
            .any(|line| line.starts_with("pub use "))
    );
}

fn table_keys<'a>(manifest: &'a str, heading: &str) -> BTreeSet<&'a str> {
    let Some((_, table)) = manifest.split_once(heading) else {
        return BTreeSet::new();
    };
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
        .filter_map(|line| line.strip_prefix(prefix))
        .filter_map(|name| name.strip_suffix(';'))
        .collect()
}
