use std::{
    fs,
    path::{Path, PathBuf},
};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_source(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn active_source_files() -> Vec<PathBuf> {
    fn collect_rust_sources(directory: &Path, files: &mut Vec<PathBuf>) {
        let mut entries = fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
            .map(|entry| entry.expect("source entry").path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                collect_rust_sources(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }

    let mut files = Vec::new();
    collect_rust_sources(&manifest_dir().join("src"), &mut files);
    files
}

#[test]
fn retired_order_runtime_roots_are_not_active() {
    let manifest = manifest_dir();
    let lib = read_source(&manifest.join("src/lib.rs"));

    for forbidden in [
        "mod orders_runtime;",
        "mod market_runtime;",
        "mod trade_storage;",
        "mod order;",
        "pub use crate::orders_runtime",
        "pub use crate::market_runtime",
        "pub use crate::trade_storage",
    ] {
        assert!(
            !lib.contains(forbidden),
            "src/lib.rs must not activate retired runtime root `{forbidden}`"
        );
    }

    for retired in [
        "src/orders_runtime.rs",
        "src/market_runtime.rs",
        "src/trade_storage.rs",
        "src/order.rs",
        "tests/orders_runtime.rs",
        "tests/market_runtime.rs",
        "tests/trade_public_api.rs",
        "tests/trade_product_publish_runtime.rs",
    ] {
        assert!(
            !manifest.join(retired).exists(),
            "retired SDK order runtime file must be removed: {retired}"
        );
    }
}

#[test]
fn active_sources_do_not_import_retired_trade_modules() {
    for path in active_source_files() {
        let source = read_source(&path);
        for forbidden in [
            "radroots_trade::order",
            "radroots_trade::projection",
            "radroots_trade::model::RadrootsTradeProjectionV1",
            "radroots_trade::reducer::RadrootsTradeReducerIssueV1",
            "radroots_trade::reducer::RadrootsTradeReductionInputV1",
            "radroots_trade::workflow::RadrootsTrade",
            "radroots_trade::RadrootsTradeProjectionV1",
            "radroots_trade::RadrootsTradeReducerIssueV1",
            "radroots_trade::RadrootsTradeReductionInputV1",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} must not import retired trade module `{forbidden}`",
                path.display()
            );
        }
    }
}

#[test]
fn active_sources_do_not_import_retired_listing_contracts() {
    for path in active_source_files() {
        let source = read_source(&path);
        for forbidden in [
            "radroots_event::operational_listing",
            "radroots_trade::listing",
            "RadrootsListingAddress",
            "KIND_LISTING",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} must not import retired listing contract `{forbidden}`",
                path.display()
            );
        }
    }
}

#[test]
fn active_sources_use_canonical_transport_type_names() {
    for path in active_source_files() {
        let source = read_source(&path);
        for retired in [
            "RadrootsTransportKind",
            "RadrootsTransportMeshScopeId",
            "RadrootsTransportTarget",
            "RadrootsTransportTargetFingerprint",
            "RadrootsTransportTargetLabel",
            "RadrootsTransportTargetSet",
        ] {
            let restores_identifier = source.match_indices(retired).any(|(index, _)| {
                let before = source[..index].chars().next_back();
                let after = source[index + retired.len()..].chars().next();
                before.is_none_or(|character| !(character.is_alphanumeric() || character == '_'))
                    && after
                        .is_none_or(|character| !(character.is_alphanumeric() || character == '_'))
            });
            assert!(
                !restores_identifier,
                "{} must use the canonical transport type instead of `{retired}`",
                path.display()
            );
        }
    }
}

#[test]
fn temporary_transport_mapping_is_publish_frozen_until_step_235() {
    let manifest = manifest_dir();
    let cargo_manifest = read_source(&manifest.join("Cargo.toml"));
    let transport = read_source(&manifest.join("src/transport.rs"));
    let deviations = read_source(&manifest.join("../../docs/implementation/deviations.toml"));

    assert!(cargo_manifest.contains("publish = false"));
    for required in [
        "pub enum SatisfactionPolicy",
        "pub struct TargetSet",
        "transport_satisfaction_policy",
    ] {
        assert!(
            transport.contains(required),
            "the bounded Step 235 migration inventory is missing `{required}`"
        );
    }
    for required in [
        "id = \"RCRV1-DEV-007\"",
        "affected_steps = [\"122\", \"170\", \"235\", \"305\"]",
        "SDK-local unpublished target-set and satisfaction-policy mapping only until Step 235",
    ] {
        assert!(
            deviations.contains(required),
            "the Step 235 final-removal record is missing `{required}`"
        );
    }
}

#[test]
fn active_sources_do_not_describe_compatibility_paths() {
    for path in active_source_files() {
        let source = read_source(&path).to_lowercase();
        for forbidden in [
            "compatibility",
            "legacy",
            "shim",
            "dual-read",
            "dual-write",
            "fallback adapter",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} must not describe `{forbidden}` behavior",
                path.display()
            );
        }
    }
}

#[test]
fn sdk_does_not_expose_generic_wire_part_signing() {
    let manifest = manifest_dir();
    let lib = read_source(&manifest.join("src/lib.rs"));
    let adapters = read_source(&manifest.join("src/adapters/mod.rs"));

    assert!(!manifest.join("src/adapters/signing.rs").exists());
    assert!(
        !manifest
            .join("tests/unit/adapters_signing_tests.rs")
            .exists()
    );
    assert!(!lib.contains("feature = \"signing\",\n"));
    assert!(!adapters.contains("pub mod signing"));
}

#[test]
fn sdk_consumes_only_the_final_signing_boundary() {
    let manifest = manifest_dir();
    let cargo_manifest = read_source(&manifest.join("Cargo.toml"));
    let workspace = manifest
        .parent()
        .and_then(Path::parent)
        .expect("SDK crate belongs to the workspace root");
    let workspace_manifest = read_source(&workspace.join("Cargo.toml"));
    let cargo_config = read_source(&workspace.join(".cargo/config.toml"));
    assert!(cargo_manifest.contains("radroots_signing = { workspace = true"));
    assert!(workspace_manifest.contains("radroots_signing = { package = \"radroots_signing\""));
    assert!(cargo_config.contains("radroots_signing = { path = \"../lib/crates/signing\" }"));
    for source in [&cargo_manifest, &workspace_manifest, &cargo_config] {
        assert!(!source.contains("radroots_authority"));
    }

    let retired_signing_surface = [
        "radroots_authority",
        "RadrootsActorContext",
        "RadrootsEventSigner",
        "RadrootsLocalEventSigner",
    ];

    for root in ["src", "tests", "examples"] {
        let directory = manifest.join(root);
        let mut files = Vec::new();
        if directory.exists() {
            fn collect(directory: &Path, files: &mut Vec<PathBuf>) {
                for entry in fs::read_dir(directory).expect("read SDK source tree") {
                    let path = entry.expect("SDK source entry").path();
                    if path.is_dir() {
                        collect(&path, files);
                    } else if path.extension().is_some_and(|extension| extension == "rs") {
                        files.push(path);
                    }
                }
            }
            collect(&directory, &mut files);
        }
        for path in files {
            if path.ends_with("source_boundary.rs") {
                continue;
            }
            let source = read_source(&path);
            for retired in retired_signing_surface {
                assert!(
                    !source.contains(retired),
                    "{} must use radroots_signing instead of retired `{retired}`",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn signer_transition_surface_is_private_hidden_and_scheduled_for_removal() {
    let manifest = manifest_dir();
    let package_manifest = read_source(&manifest.join("Cargo.toml"));
    let lib = read_source(&manifest.join("src/lib.rs"));
    let adapters = read_source(&manifest.join("src/adapters/mod.rs"));
    let transition_record =
        read_source(&manifest.join("../../docs/implementation/COMPATIBILITY_SHIMS.md"));

    assert!(package_manifest.contains("publish = false"));
    assert!(lib.contains("#[doc(hidden)]\npub use crate::signer_provider::{"));
    assert!(adapters.contains("#[doc(hidden)]\npub mod signer;"));
    assert!(transition_record.contains("SDK signer provider façade"));
    assert!(transition_record.contains("Step 313"));
    assert!(transition_record.contains("oss/cli"));
    assert!(transition_record.contains("oss/studio_app"));
}

#[test]
fn signer_consumers_use_the_final_nostr_connect_state_machine() {
    let manifest = manifest_dir();
    for relative in [
        "src/adapters/signer.rs",
        "examples/sdk_v1_myc_nip46_signer_setup.rs",
    ] {
        let source = read_source(&manifest.join(relative));
        for retired in [
            "radroots_nostr_connect::prelude",
            "RadrootsNostrConnectClient",
            "RadrootsNostrConnectMethod",
            "RadrootsNostrConnectPermission",
            "RadrootsNostrConnectRequest",
            "RadrootsNostrConnectResponse",
            "RADROOTS_NOSTR_CONNECT_",
            "RadrootsSdkNip46ClientKey",
            "RadrootsSdkNip46Transport",
        ] {
            assert!(
                !source.contains(retired),
                "{relative} retains retired Nostr Connect surface `{retired}`"
            );
        }
    }

    let provider = read_source(&manifest.join("src/signer_provider.rs"));
    assert!(provider.contains("transport: Arc<AsyncMutex<Box<dyn Transport>>>"));
    assert!(provider.contains("impl Transport for RadrootsSdkNip46TimeoutTransport"));
    assert!(provider.contains(".client\n            .execute("));
    assert!(provider.contains("pub fn from_client<T>("));
    assert!(provider.contains("CLI migration Step 271; removed in Step 313"));
    for shim in [
        "pub struct RadrootsSdkNip46ClientKey",
        "pub type RadrootsSdkNip46TransportFuture",
        "pub trait RadrootsSdkNip46Transport",
    ] {
        let position = provider
            .find(shim)
            .unwrap_or_else(|| panic!("missing compatibility shim `{shim}`"));
        assert!(provider[..position].ends_with("#[doc(hidden)]\n"));
    }
}
