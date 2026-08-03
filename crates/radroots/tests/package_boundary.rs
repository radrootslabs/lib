use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const WORKSPACE: &str = include_str!("../../../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn package_has_final_identity_and_exact_dependencies() {
    assert!(MANIFEST.contains("name = \"radroots\""));
    assert!(MANIFEST.contains("publish = false"));
    assert_eq!(
        dependency_names(MANIFEST),
        BTreeSet::from([
            "radroots_core",
            "radroots_event",
            "radroots_identity",
            "radroots_sdk",
            "radroots_trade",
            "radroots_transport",
        ])
    );
    assert!(WORKSPACE.contains(
        "radroots_sdk = { path = \"crates/sdk\", version = \"=0.1.0-alpha\", default-features = false }"
    ));
}

#[test]
fn root_has_exact_exports_and_no_sdk_namespace() {
    assert!(ROOT.contains("pub use radroots_sdk::{Client, ClientBuilder, Error, Result};"));
    assert!(!ROOT.contains("pub mod sdk"));
    assert!(!ROOT.contains("pub use radroots_sdk::*"));
    assert_eq!(ROOT.matches("pub use ").count(), 1);
}

fn dependency_names(manifest: &str) -> BTreeSet<&str> {
    manifest
        .split_once("[dependencies]")
        .expect("dependency section")
        .1
        .split_once("[features]")
        .expect("feature section")
        .0
        .lines()
        .filter_map(|line| line.split_once(" = ").map(|(name, _)| name.trim()))
        .collect()
}
