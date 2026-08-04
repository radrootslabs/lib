use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const CLIENT: &str = include_str!("../src/client.rs");
const FARM: &str = include_str!("../src/farm.rs");
const LISTING: &str = include_str!("../src/listing.rs");
const TRADE: &str = include_str!("../src/trade.rs");
const STORAGE: &str = include_str!("../src/storage.rs");
const DIAGNOSTICS: &str = include_str!("../src/diagnostics.rs");
const ADAPTERS: &str = include_str!("../src/adapters/mod.rs");
const RADROOTSD: &str = include_str!("../src/adapters/radrootsd.rs");
const SYNC: &str = include_str!("../src/sync.rs");
const TRANSPORT: &str = include_str!("../src/transport.rs");
const PUBLICATION: &str = include_str!("../../../contracts/releases/publication.toml");

#[test]
fn manifest_has_final_identity_and_dependency_boundary() {
    assert!(MANIFEST.contains("name = \"radroots_sdk\""));
    assert!(MANIFEST.contains("publish = [\"crates-io\"]"));
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
    for forbidden in [
        "radroots_event_store",
        "radroots_nostr_signer",
        "radroots_outbox",
        "radroots_protected_store",
        "radroots_runtime_paths",
        "radroots_secret_vault",
        "radroots_transport_reticulum",
    ] {
        assert!(
            !dependencies.contains(forbidden),
            "SDK depends on private or superseded package `{forbidden}`"
        );
    }
}

#[test]
fn manifest_has_exact_feature_vocabulary_and_explicit_optional_activation() {
    let features = MANIFEST
        .split_once("[features]")
        .expect("feature section")
        .1
        .split_once("[dependencies]")
        .expect("dependency section")
        .0
        .lines()
        .filter_map(|line| line.split_once(" = ").map(|(name, _)| name.trim()))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        features,
        BTreeSet::from([
            "default",
            "full",
            "geonames",
            "knowledge",
            "local-signing",
            "memory",
            "native",
            "nip46",
            "nostr",
            "radrootsd",
            "sqlite",
            "sync",
        ])
    );
    for activation in [
        "dep:radroots_storage_sqlite",
        "dep:radroots_sync",
        "dep:radroots_nostr",
        "dep:radroots_transport_nostr",
        "dep:radroots_nostr_connect",
        "dep:radroots_secrets",
        "dep:reqwest",
        "dep:serde",
        "dep:serde_json",
        "dep:radroots_geonames",
    ] {
        assert!(
            MANIFEST.contains(activation),
            "missing activation `{activation}`"
        );
    }
    for retired in [
        "runtime =",
        "local-runtime",
        "signer-adapters",
        "transport-nostr-runtime",
        "transport-nostr-client",
        "fixtures =",
    ] {
        assert!(!MANIFEST.contains(retired), "retired feature `{retired}`");
    }
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
fn package_contains_only_reachable_sources_and_registered_targets() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        rust_files(&root.join("src")),
        BTreeSet::from([
            "adapters/mod.rs".to_owned(),
            "adapters/radrootsd.rs".to_owned(),
            "capability.rs".to_owned(),
            "client.rs".to_owned(),
            "diagnostics.rs".to_owned(),
            "error.rs".to_owned(),
            "farm.rs".to_owned(),
            "lib.rs".to_owned(),
            "listing.rs".to_owned(),
            "listing/operational_listing/draft.rs".to_owned(),
            "listing/operational_listing/mod.rs".to_owned(),
            "listing/operational_listing/model.rs".to_owned(),
            "listing/operational_listing/mutation.rs".to_owned(),
            "listing/operational_listing/price_ext.rs".to_owned(),
            "listing/operational_listing/validation.rs".to_owned(),
            "signing.rs".to_owned(),
            "storage.rs".to_owned(),
            "sync.rs".to_owned(),
            "trade.rs".to_owned(),
            "transport.rs".to_owned(),
        ])
    );
    assert_eq!(
        rust_files(&root.join("tests")),
        BTreeSet::from([
            "lifecycle.rs".to_owned(),
            "package_boundary.rs".to_owned(),
            "public_api.rs".to_owned(),
            "unit/adapters_radrootsd_tests.rs".to_owned(),
        ])
    );
    assert_eq!(
        rust_files(&root.join("examples")),
        BTreeSet::from([
            "safe_memory_client.rs".to_owned(),
            "transport_profile.rs".to_owned(),
        ])
    );
}

#[test]
fn compatibility_packages_are_removed() {
    assert!(PUBLICATION.contains("retired = []"));
    let approved = PUBLICATION
        .split_once("approved_packages = [")
        .expect("approved package list")
        .1
        .split_once("\n]")
        .expect("approved package list terminator")
        .0;
    assert!(!approved.contains("radroots_runtime_contract_v1"));
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    assert!(!workspace.join("crates/runtime_contract_v1").exists());
    assert!(!workspace.join("crates/event_index_bindings").exists());
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

#[test]
fn listing_operations_reuse_trade_event_sync_and_privacy_boundaries() {
    for required in [
        "canonicalize_operational_listing_edit",
        "RadrootsOperationalListingMutation",
        "RadrootsOperationalListingLifecycleState",
        "build_operational_listing_mutation_draft",
        "radroots_sync::PushRequest::new",
        "PrivateArtifactStore",
    ] {
        assert!(
            LISTING.contains(required),
            "missing listing boundary `{required}`"
        );
    }
    for forbidden in [
        "SdkMutationState",
        "ListingEnqueueReceipt",
        "enqueue_signed_workflow",
        "local_event_seq",
        "outbox_event_id",
        "latitude:",
        "longitude:",
    ] {
        assert!(
            !LISTING.contains(forbidden),
            "listing source contains retired duplicate or private representation `{forbidden}`"
        );
    }
    assert!(
        !std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/listings_runtime.rs")
            .exists()
    );
}

#[test]
fn trade_operations_use_canonical_workflow_storage_sync_and_projection_types() {
    for required in [
        "WorkflowPlan",
        "TradeMutationEnvelopeV1",
        "ReductionInput",
        "Projection",
        "reduce_trade_records",
        "EventQuery",
        "EventPage<StoredVisibleEvent>",
        "PrivateArtifactMetadata",
        "radroots_sync::PushRequest::new",
    ] {
        assert!(
            TRADE.contains(required),
            "missing trade boundary `{required}`"
        );
    }
    for forbidden in [
        "sqlx::",
        "QueryBuilder",
        "TradeStatusView",
        "SdkPrivateTradeArtifact",
        "SdkMutationState",
        "TradeCommandReceipt",
        "struct Page",
        "struct TradeId",
    ] {
        assert!(
            !TRADE.contains(forbidden),
            "trade source contains retired duplicate `{forbidden}`"
        );
    }
    assert!(
        !std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/trade_runtime.rs")
            .exists()
    );
}

#[test]
fn reliability_and_diagnostics_return_canonical_storage_contracts() {
    for required in [
        "StorageReliability::begin_backup",
        "StorageReliability::transition_backup",
        "StorageReliability::begin_restore",
        "StorageReliability::transition_restore",
        "StorageReliability::integrity",
        "StorageReliability::status",
    ] {
        assert!(
            STORAGE.contains(required),
            "missing storage delegation `{required}`"
        );
    }
    for forbidden in [
        "struct BackupManifest",
        "struct IntegrityStatus",
        "Studio",
        "sqlx::",
    ] {
        assert!(
            !STORAGE.contains(forbidden),
            "storage source duplicates `{forbidden}`"
        );
    }
    assert!(DIAGNOSTICS.contains("radroots_storage::StorageStatus"));
    for forbidden in ["path:", "SqlitePool", "private_artifact"] {
        assert!(
            !DIAGNOSTICS.contains(forbidden),
            "diagnostics leaks `{forbidden}`"
        );
    }
}

#[test]
fn radrootsd_adapter_is_private_explicit_versioned_and_redacted() {
    assert!(ROOT.contains("mod adapters;"));
    assert!(!ROOT.contains("pub mod adapters"));
    assert!(ADAPTERS.contains("cfg(feature = \"radrootsd\")"));
    assert!(ADAPTERS.contains("pub(crate) mod radrootsd"));
    assert!(RADROOTSD.contains("transport_publish::v5"));
    assert!(RADROOTSD.contains("reqwest::Client::builder"));
    assert!(RADROOTSD.contains("BearerToken(<redacted>)"));
    assert!(!RADROOTSD.contains("tokio::spawn"));
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

fn rust_files(root: &std::path::Path) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("read package source directory") {
            let path = entry.expect("read package source entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
                files.insert(
                    path.strip_prefix(root)
                        .expect("source remains below root")
                        .to_string_lossy()
                        .replace(std::path::MAIN_SEPARATOR, "/"),
                );
            }
        }
    }
    files
}
