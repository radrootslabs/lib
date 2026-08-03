use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn sync_depends_only_on_final_orchestration_boundaries() {
    for required in [
        "name = \"radroots_sync\"",
        "version = \"0.1.0-alpha\"",
        "publish = false",
        "[lib]\nname = \"radroots_sync\"",
        "default = [\"serde\"]",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing `{required}`"
        );
    }
    assert_eq!(
        dependency_keys(MANIFEST)
            .into_iter()
            .filter(|dependency| dependency.starts_with("radroots_"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "radroots_event",
            "radroots_event_codec",
            "radroots_protocol",
            "radroots_signing",
            "radroots_storage",
            "radroots_trade",
            "radroots_transport",
        ])
    );
    assert!(MANIFEST.contains("serde = { workspace = true, optional = true }"));
    for forbidden in [
        "radroots_event_store",
        "radroots_event_index",
        "radroots_outbox",
        "radroots_runtime_store",
        "radroots_transport_nostr",
    ] {
        assert!(!MANIFEST.contains(forbidden));
        assert!(!ROOT.contains(forbidden));
    }
    assert_eq!(
        declarations(ROOT, "pub mod "),
        BTreeSet::from(["ingest", "policy", "projection", "pull", "push", "status"])
    );
}

fn declarations<'a>(source: &'a str, prefix: &str) -> BTreeSet<&'a str> {
    source
        .lines()
        .filter_map(|line| line.trim().strip_prefix(prefix))
        .filter_map(|name| name.strip_suffix(';'))
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
