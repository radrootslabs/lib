use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const CLIENT: &str = include_str!("../src/client.rs");

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
    assert_eq!(ROOT.matches("pub use crate::").count(), 2);
    assert!(ROOT.contains("pub use crate::client::{Client, ClientBuilder};"));
    assert!(ROOT.contains("pub use crate::error::{Error, Result};"));
    assert!(!ROOT.contains("pub use radroots_"));
}

#[test]
fn root_types_have_the_required_std_contracts() {
    fn assert_client<T: Clone + Send + Sync>() {}
    fn assert_builder<T: Send + Sync>() {}
    fn assert_error<T: std::error::Error + Send + Sync + 'static>() {}

    assert_client::<radroots_sdk::Client>();
    assert_builder::<radroots_sdk::ClientBuilder>();
    assert_error::<radroots_sdk::Error>();
    let result: radroots_sdk::Result<()> = Ok(());
    assert!(result.is_ok());
}

#[cfg(feature = "sqlite")]
#[test]
fn sqlite_backend_implements_the_canonical_storage_capability() {
    fn assert_storage<T: radroots_storage::Storage>() {}
    assert_storage::<radroots_storage_sqlite::SqliteStorage>();
}

#[cfg(feature = "nostr")]
#[test]
fn nostr_adapter_implements_independent_source_and_sink_capabilities() {
    fn assert_source<T: radroots_transport::EventSource>() {}
    fn assert_sink<T: radroots_transport::EventSink>() {}
    assert_source::<radroots_transport_nostr::NostrTransport>();
    assert_sink::<radroots_transport_nostr::NostrTransport>();
}

#[test]
fn lifecycle_source_owns_no_hidden_worker_runtime_or_blocking_drop() {
    for forbidden in [
        "tokio::spawn",
        "std::thread::spawn",
        "Runtime::new",
        "block_on(self.close",
        "impl Drop for Client",
    ] {
        assert!(
            !CLIENT.contains(forbidden),
            "forbidden lifecycle source `{forbidden}`"
        );
    }
    assert!(CLIENT.contains("pub async fn close(&self)"));
    assert!(CLIENT.contains("impl Drop for CloseAttempt"));
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
