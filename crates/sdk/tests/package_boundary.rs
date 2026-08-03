use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const CLIENT: &str = include_str!("../src/client.rs");
const FARM: &str = include_str!("../src/farm.rs");
const SYNC: &str = include_str!("../src/sync.rs");
const TRANSPORT: &str = include_str!("../src/transport.rs");

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

#[test]
fn sdk_source_contains_no_studio_storage_surface() {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pending = vec![source_root];
    while let Some(path) = pending.pop() {
        for entry in std::fs::read_dir(path).expect("read SDK source") {
            let entry = entry.expect("source entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
                let source = std::fs::read_to_string(&path).expect("read source file");
                for forbidden in [
                    "studio.sqlite",
                    "SdkStudioStore",
                    "sdk_studio_state",
                    "studio_store",
                ] {
                    assert!(
                        !source.contains(forbidden),
                        "{} contains retired Studio storage marker {forbidden}",
                        path.display()
                    );
                }
            }
        }
    }
}

#[test]
fn transport_profiles_reuse_canonical_types_and_forbid_fallback() {
    assert!(TRANSPORT.contains("use radroots_transport::{"));
    assert!(TRANSPORT.contains("satisfaction.validate_for(&targets)?"));
    assert!(TRANSPORT.contains("Selection::UnavailablePreview"));
    for duplicate in [
        "pub struct TargetSet",
        "pub enum TargetPolicy",
        "pub enum SatisfactionPolicy",
        "pub struct SourceStatus",
        "pub struct SinkStatus",
        "DefaultProfile",
    ] {
        assert!(
            !TRANSPORT.contains(duplicate),
            "SDK transport source contains forbidden duplicate or fallback `{duplicate}`"
        );
    }
}

#[test]
fn sync_operations_only_delegate_to_the_canonical_engine() {
    for delegation in [
        "self.engine.pull(request, admission).await",
        "self.engine.ingest(observed, admission).await",
        "self.engine.ingest_batch(observed, admission).await",
        "self.engine.refresh_projection(request, reducer).await",
        "self.engine.sign_and_enqueue(request).await",
        "self.engine.deliver_pending(request).await",
        "self.engine.status(projections).await",
        "self.engine.retry_decision(record, now_unix_ms)",
    ] {
        assert!(
            SYNC.contains(delegation),
            "missing sync delegation `{delegation}`"
        );
    }
    for duplicate in [
        "pub struct SyncTransport",
        "pub struct SyncOutbox",
        "pub struct SyncProjection",
        "pub struct SyncStatus",
        "pub struct PushOutbox",
    ] {
        assert!(
            !SYNC.contains(duplicate),
            "SDK sync source contains duplicate lower model `{duplicate}`"
        );
    }
    assert!(
        !std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/sync_runtime.rs")
            .exists()
    );
}

#[test]
fn farm_operations_preserve_pure_planning_commit_and_privacy_boundaries() {
    for required in [
        "encode::farm::to_wire_parts",
        "AddressableCoordinate",
        "EventDraft",
        "radroots_sync::PushRequest::new",
        "self.sync",
        ".sign_and_enqueue(",
        "PrivateArtifactStore",
    ] {
        assert!(
            FARM.contains(required),
            "missing farm boundary `{required}`"
        );
    }
    for forbidden in [
        "SdkExactLocation",
        "SdkPublicLocality",
        "latitude:",
        "longitude:",
        "enqueue_signed_workflow",
        "local_event_seq",
        "outbox_event_id",
    ] {
        assert!(
            !FARM.contains(forbidden),
            "farm source contains retired SDK or private representation `{forbidden}`"
        );
    }
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(!source_root.join("farms_runtime.rs").exists());
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
