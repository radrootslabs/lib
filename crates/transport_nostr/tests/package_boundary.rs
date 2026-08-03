use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn manifest_and_root_match_the_governed_transport_boundary() {
    for required in [
        "name = \"radroots_transport_nostr\"",
        "version = \"0.1.0-alpha\"",
        "publish = false",
        "[lib]\nname = \"radroots_transport_nostr\"",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing `{required}`"
        );
    }
    assert!(!MANIFEST.contains("[features]"));
    assert_eq!(
        radroots_dependency_keys(MANIFEST),
        BTreeSet::from([
            "radroots_event_codec",
            "radroots_nostr",
            "radroots_protocol",
            "radroots_transport",
        ])
    );
    assert_eq!(
        private_modules(ROOT),
        BTreeSet::from([
            "auth", "client", "error", "relay", "sink", "source", "status"
        ])
    );
    for export in [
        "pub use client::{Config, NostrTransport};",
        "pub use error::Error;",
        "pub use relay::{RelayUrl, RelayUrlPolicy};",
    ] {
        assert!(ROOT.contains(export), "crate root is missing `{export}`");
    }
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

fn private_modules(root: &str) -> BTreeSet<&str> {
    root.lines()
        .filter_map(|line| line.trim().strip_prefix("mod "))
        .filter_map(|module| module.strip_suffix(';'))
        .collect()
}

#[test]
fn adapter_owns_no_storage_outbox_or_orchestration_surface() {
    for forbidden in [
        "radroots_event_store",
        "radroots_outbox",
        "radroots_storage",
        "publish_claimed",
        "fetch_and_ingest",
        "projection_refresh",
        "retry_schedule",
    ] {
        assert!(!MANIFEST.contains(forbidden));
        assert!(!ROOT.contains(forbidden));
    }

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let source_files = fs::read_dir(source_root)
        .expect("source directory")
        .map(|entry| {
            entry
                .expect("source entry")
                .file_name()
                .into_string()
                .expect("utf-8 source name")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        source_files,
        BTreeSet::from([
            "auth.rs".to_owned(),
            "client.rs".to_owned(),
            "error.rs".to_owned(),
            "lib.rs".to_owned(),
            "relay.rs".to_owned(),
            "sink.rs".to_owned(),
            "source.rs".to_owned(),
            "status.rs".to_owned(),
        ])
    );
}
