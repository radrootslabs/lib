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
