use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn sync_depends_only_on_final_orchestration_boundaries() {
    assert_eq!(
        dependency_keys(MANIFEST),
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
