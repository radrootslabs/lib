use std::{collections::BTreeSet, fs, path::Path};

#[allow(unused_imports)]
use radroots_transport::{
    DeliveryReceipt as _, DeliveryRequest as _, Error as _, EventSink as _, EventSource as _,
    FetchPage as _, FetchRequest as _, Target as _, TargetSet as _, TransportId as _,
    capability as _, endpoint as _, error as _, outcome as _, policy as _, sink as _, source as _,
    target as _,
};

const MANIFEST: &str = include_str!("../Cargo.toml");
const EXAMPLE: &str = include_str!("../examples/host_transport.rs");
const PUBLIC_API: &str = include_str!("../../../docs/api/radroots_transport.txt");
const README: &str = include_str!("../README.md");
const ROOT: &str = include_str!("../src/lib.rs");
const SOURCE: &str = include_str!("../src/source.rs");
const SINK: &str = include_str!("../src/sink.rs");
const ID: &str = include_str!("../src/id.rs");
const LEGACY_TRANSPORT: &str = include_str!("../src/transport.rs");

#[test]
fn manifest_has_final_identity_features_and_required_radroots_dependencies() {
    for required in [
        "name = \"radroots_transport\"",
        "version = \"0.1.0-alpha\"",
        "publish = false",
        "[lib]\nname = \"radroots_transport\"",
        "default = [\"std\", \"serde\"]",
        "radroots_event = { workspace = true, default-features = false }",
        "radroots_identity = { workspace = true, default-features = false }",
        "radroots_protocol = { workspace = true, default-features = false }",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing {required}"
        );
    }

    assert_eq!(
        table_keys(MANIFEST, "[features]"),
        BTreeSet::from(["default", "serde", "std"])
    );
    assert_eq!(
        table_keys(MANIFEST, "[dependencies]")
            .into_iter()
            .filter(|dependency| dependency.starts_with("radroots_"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["radroots_event", "radroots_identity", "radroots_protocol"])
    );
}

#[test]
fn crate_root_declares_the_approved_public_module_skeleton() {
    assert!(ROOT.contains("#![doc = include_str!(\"../README.md\")]"));
    assert!(ROOT.contains("#![cfg_attr(not(feature = \"std\"), no_std)]"));
    assert_eq!(
        root_declarations("pub mod "),
        BTreeSet::from([
            "capability",
            "endpoint",
            "error",
            "outcome",
            "policy",
            "sink",
            "source",
            "target",
        ])
    );
}

#[test]
fn package_documentation_and_reviewed_api_baseline_are_complete() {
    for required in [
        "## Typical flow",
        "## Host SPI contract",
        "## Targets and extensible identity",
        "## Bounds, deadlines, cancellation, and commit points",
        "## Outcomes, partial success, and retry",
        "## Serialization and provenance",
        "## Security and side effects",
        "## Features",
        "## Intended consumers",
        "radroots_crates_release_v1.md#9-radroots_transport",
        "examples/host_transport.rs",
    ] {
        assert!(README.contains(required), "README is missing {required}");
    }
    for required in [
        "impl EventSource for HostTransport",
        "impl EventSink for HostTransport",
        "fn fetch(&self, _request: FetchRequest) -> BoxFuture",
        "_request: DeliveryRequest",
        "let source: &dyn EventSource",
        "let sink: &dyn EventSink",
        "drop(future)",
    ] {
        assert!(EXAMPLE.contains(required), "example is missing {required}");
    }
    for required in [
        "pub mod radroots_transport::capability",
        "pub mod radroots_transport::endpoint",
        "pub mod radroots_transport::error",
        "pub mod radroots_transport::outcome",
        "pub mod radroots_transport::policy",
        "pub mod radroots_transport::sink",
        "pub mod radroots_transport::source",
        "pub mod radroots_transport::target",
        "pub trait radroots_transport::EventSource",
        "pub trait radroots_transport::EventSink",
    ] {
        assert!(
            PUBLIC_API.contains(required),
            "public API baseline is missing {required}"
        );
    }
}

#[test]
fn every_public_module_has_crate_level_documentation() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for module in [
        "capability",
        "endpoint",
        "error",
        "outcome",
        "policy",
        "sink",
        "source",
        "target",
    ] {
        let path = source_root.join(format!("{module}.rs"));
        let source = fs::read_to_string(&path).expect("read module source");
        assert!(
            source.starts_with("//! "),
            "public module {module} must start with module documentation"
        );
    }
}

#[test]
fn source_and_sink_are_independent_dyn_compatible_host_spis() {
    for required in [
        "pub trait EventSource: Send + Sync",
        "fn status(&self)",
        "fn fetch(",
        "Dropping a returned future requests cancellation.",
        "explicit request deadline",
    ] {
        assert!(
            SOURCE.contains(required),
            "source SPI is missing {required}"
        );
    }
    assert!(!SOURCE.contains("fn deliver("));

    for required in [
        "pub trait EventSink: Send + Sync",
        "fn status(&self)",
        "fn deliver(",
        "Dropping a returned future requests cancellation.",
        "explicit request deadline",
    ] {
        assert!(SINK.contains(required), "sink SPI is missing {required}");
    }
    assert!(!SINK.contains("fn fetch("));

    assert!(LEGACY_TRANSPORT.contains("#[doc(hidden)]\npub trait RadrootsTransport: Send + Sync"));
}

#[test]
fn public_api_excludes_adapter_runtime_storage_and_retry_authority() {
    let dependency_keys = table_keys(MANIFEST, "[dependencies]");
    for forbidden in [
        "radroots_outbox",
        "radroots_storage",
        "radroots_transport_nostr",
        "radroots_transport_reticulum",
        "nostr-sdk",
        "nostr_sdk",
        "reqwest",
        "sqlx",
        "tokio",
    ] {
        assert!(
            !dependency_keys.contains(forbidden),
            "generic transport must not depend on `{forbidden}`"
        );
    }

    assert!(ID.contains("pub struct TransportId("));
    assert!(!ID.contains("pub enum TransportId"));
    for forbidden in [
        "RADROOTS_RETICULUM_",
        "ReticulumDestination",
        "RelayUrl",
        "NostrRelay",
    ] {
        assert!(
            !ROOT.contains(forbidden),
            "generic transport root must not export adapter symbol `{forbidden}`"
        );
    }
    for source in [SOURCE, SINK] {
        for forbidden in [
            "tokio::spawn",
            "std::thread::spawn",
            "retry_loop",
            "fallback_transport",
        ] {
            assert!(
                !source.contains(forbidden),
                "transport SPI must not own runtime behavior `{forbidden}`"
            );
        }
    }
}

fn table_keys<'a>(source: &'a str, heading: &str) -> BTreeSet<&'a str> {
    let mut in_table = false;
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line == heading {
                in_table = true;
                return None;
            }
            if in_table && line.starts_with('[') {
                in_table = false;
            }
            in_table
                .then(|| line.split_once('=').map(|(key, _)| key.trim()))
                .flatten()
        })
        .collect()
}

fn root_declarations(prefix: &str) -> BTreeSet<&str> {
    ROOT.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix(prefix))
        .filter_map(|line| line.strip_suffix(';'))
        .collect()
}
