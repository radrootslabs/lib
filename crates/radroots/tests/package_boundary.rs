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
fn features_forward_the_exact_user_oriented_graph() {
    for forwarding in [
        "default = [\"client\"]",
        "client = [\"radroots_sdk/default\"]",
        "native = [\"client\", \"radroots_sdk/native\"]",
        "nostr = [\"client\", \"radroots_sdk/nostr\"]",
        "nip46 = [\"nostr\", \"radroots_sdk/nip46\"]",
        "radrootsd = [\"client\", \"radroots_sdk/radrootsd\"]",
        "geonames = [\"client\", \"radroots_sdk/geonames\"]",
        "knowledge = [\"client\", \"radroots_sdk/knowledge\"]",
        "full = [\"radroots_sdk/full\"]",
    ] {
        assert!(MANIFEST.contains(forwarding), "missing `{forwarding}`");
    }
    for forbidden in ["reticulum", "mesh", "simplex", "nostrdb", "replica", "sp1"] {
        assert!(
            !MANIFEST.to_ascii_lowercase().contains(forbidden),
            "forbidden feature vocabulary `{forbidden}`"
        );
    }
}

#[test]
fn root_has_exact_exports_and_no_sdk_namespace() {
    assert!(ROOT.contains("pub use radroots_sdk::{Client, ClientBuilder, Error, Result};"));
    assert!(!ROOT.contains("pub mod sdk"));
    assert!(!ROOT.contains("pub use radroots_sdk::*"));
    assert_eq!(ROOT.matches("pub use ").count(), 1);
}

#[test]
fn modules_use_deliberate_reexports_without_wildcards() {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in std::fs::read_dir(source_root).expect("read facade source") {
        let path = entry.expect("read facade source entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("read facade module");
        assert!(
            !source.contains("::*"),
            "{} contains a wildcard reexport",
            path.display()
        );
        assert!(
            !source.contains("pub mod sdk"),
            "{} exposes the forbidden SDK namespace",
            path.display()
        );
    }
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
