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
        for forbidden in ["radroots_trade::order", "radroots_trade::projection"] {
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
            "radroots_event::listing",
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
