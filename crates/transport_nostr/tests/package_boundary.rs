use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README.md");
const EXAMPLE: &str = include_str!("../examples/configure_transport.rs");
const PUBLIC_API: &str = include_str!("../../../docs/api/radroots_transport_nostr.txt");
const API_INDEX: &str = include_str!("../../../docs/api/README.md");
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

#[test]
fn documentation_example_and_reviewed_api_baseline_are_complete() {
    for required in [
        "## Configure without connecting",
        "## Public surface",
        "## Relay and network security",
        "## Fetch, delivery, and outcome behavior",
        "## Deadlines, cancellation, and commit points",
        "## Serialization and diagnostics",
        "## Features and runtime requirements",
        "## Intended consumers",
        "radroots_crates_release_v1.md#15-radroots_transport_nostr",
        "examples/configure_transport.rs",
        "docs/api/radroots_transport_nostr.txt",
    ] {
        assert!(README.contains(required), "README is missing `{required}`");
    }
    for required in [
        "Config::new(",
        "RelayUrlPolicy::Public",
        "NostrTransport::new(config)",
        "let source: &dyn EventSource",
        "let sink: &dyn EventSink",
        "drop(source.status())",
        "drop(sink.status())",
    ] {
        assert!(
            EXAMPLE.contains(required),
            "example is missing `{required}`"
        );
    }
    for required in [
        "pub struct radroots_transport_nostr::Config",
        "pub struct radroots_transport_nostr::NostrTransport",
        "pub struct radroots_transport_nostr::RelayUrl(_)",
        "pub enum radroots_transport_nostr::RelayUrlPolicy",
        "pub enum radroots_transport_nostr::Error",
        "impl radroots_transport::sink::EventSink for radroots_transport_nostr::NostrTransport",
        "impl radroots_transport::source::EventSource for radroots_transport_nostr::NostrTransport",
        "NostrTransport::begin_authentication",
        "NostrTransport::complete_authentication",
        "NostrTransport::reject_authentication",
    ] {
        assert!(
            PUBLIC_API.contains(required),
            "public API baseline is missing `{required}`"
        );
    }
    for forbidden in [
        "nostr_sdk",
        "nostr_relay_pool",
        "tokio::",
        "radroots_storage",
        "radroots_outbox",
        "pub trait radroots_transport_nostr",
    ] {
        assert!(
            !PUBLIC_API.contains(forbidden),
            "reviewed public API baseline exposes `{forbidden}`"
        );
    }
    assert!(API_INDEX.contains(
        "| `radroots_transport_nostr` | [`radroots_transport_nostr.txt`](radroots_transport_nostr.txt) |"
    ));
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
