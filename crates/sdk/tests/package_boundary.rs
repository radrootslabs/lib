use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn manifest_has_final_identity_and_dependency_boundary() {
    assert!(MANIFEST.contains("name = \"radroots_sdk\""));
    assert!(MANIFEST.contains("publish = false"));
    assert!(MANIFEST.contains("version.workspace = true"));
    assert!(MANIFEST.contains("name = \"package_boundary\""));

    let dependencies = dependency_names(MANIFEST);
    let expected = BTreeSet::from([
        "radroots_core",
        "radroots_event",
        "radroots_event_codec",
        "radroots_geonames",
        "radroots_identity",
        "radroots_nostr",
        "radroots_nostr_connect",
        "radroots_protocol",
        "radroots_secrets",
        "radroots_signing",
        "radroots_storage",
        "radroots_storage_sqlite",
        "radroots_sync",
        "radroots_trade",
        "radroots_transport",
        "radroots_transport_nostr",
    ]);
    assert_eq!(dependencies, expected);
}

#[test]
fn root_declares_exact_final_module_skeleton() {
    let actual = ROOT
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub mod "))
        .map(|module| module.trim_end_matches(';'))
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        "capability",
        "client",
        "diagnostics",
        "error",
        "farm",
        "listing",
        "signing",
        "storage",
        "sync",
        "trade",
        "transport",
    ]);
    assert_eq!(actual, expected);
    assert!(!ROOT.contains("no_std"));
}

fn dependency_names(manifest: &str) -> BTreeSet<&str> {
    let dependencies = manifest
        .split_once("[dependencies]")
        .expect("dependency section")
        .1
        .split_once("[[test]]")
        .expect("test section")
        .0;
    dependencies
        .lines()
        .filter_map(|line| line.split_once(" = ").map(|(name, _)| name.trim()))
        .filter(|name| name.starts_with("radroots_"))
        .collect()
}
