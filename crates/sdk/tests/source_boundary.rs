use std::{
    fs,
    path::{Path, PathBuf},
};

struct ForbiddenSdkConcept {
    pattern: &'static str,
    reason: &'static str,
}

const FORBIDDEN_SDK_SOURCE_CONCEPTS: &[ForbiddenSdkConcept] = &[
    ForbiddenSdkConcept {
        pattern: "radroots_studio_app",
        reason: "SDK source must not depend on the app crate family",
    },
    ForbiddenSdkConcept {
        pattern: "radroots_cli",
        reason: "SDK source must not depend on the CLI crate family",
    },
    ForbiddenSdkConcept {
        pattern: "RadrootsApp",
        reason: "SDK source must not import app product concepts",
    },
    ForbiddenSdkConcept {
        pattern: "RadrootsCli",
        reason: "SDK source must not import CLI product concepts",
    },
    ForbiddenSdkConcept {
        pattern: "AppSdk",
        reason: "app SDK adapters belong outside the SDK crate",
    },
    ForbiddenSdkConcept {
        pattern: "DesktopApp",
        reason: "desktop app runtime concepts belong outside the SDK crate",
    },
    ForbiddenSdkConcept {
        pattern: "domains/radroots/studio_apps",
        reason: "SDK source must remain standalone and path-agnostic",
    },
];

#[test]
fn sdk_sources_do_not_import_app_or_cli_concepts() {
    for path in rust_source_files(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path()) {
        let source = read_source(path.as_path());
        for concept in FORBIDDEN_SDK_SOURCE_CONCEPTS {
            assert!(
                !source.contains(concept.pattern),
                "{} contains forbidden SDK source concept `{}`: {}",
                path.display(),
                concept.pattern,
                concept.reason
            );
        }
    }
}

#[test]
fn sdk_manifest_does_not_depend_on_app_or_cli_crates() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = read_source(manifest_path.as_path());

    for crate_name in ["radroots_studio_app", "radroots_cli"] {
        assert!(
            !manifest.contains(crate_name),
            "SDK manifest contains forbidden downstream crate dependency `{crate_name}`"
        );
    }
}

#[test]
fn farm_runtime_stays_on_product_runtime_boundary() {
    product_runtime_file_stays_on_boundary("src/farms_runtime.rs");
}

#[test]
fn order_runtime_stays_on_product_runtime_boundary() {
    product_runtime_file_stays_on_boundary("src/orders_runtime.rs");
}

#[test]
fn migrated_runtime_tests_stay_on_product_runtime_boundary() {
    for file in ["tests/farms_runtime.rs", "tests/orders_runtime.rs"] {
        product_runtime_file_stays_on_boundary(file);
    }
}

#[test]
fn legacy_client_and_config_modules_are_removed() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    for relative_path in ["src/client.rs", "src/config.rs"] {
        let path = manifest_dir.join(relative_path);
        assert!(
            !path.exists(),
            "{relative_path} must not exist after SDK runtime surface closure"
        );
    }
}

#[test]
fn legacy_trade_client_root_export_is_removed() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/lib.rs")
            .as_path(),
    );

    assert!(
        !source.contains("TradeClient"),
        "src/lib.rs must not re-export the legacy TradeClient facade"
    );
}

#[test]
fn legacy_client_config_modules_are_not_public() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/lib.rs")
            .as_path(),
    );

    for forbidden in [
        "pub mod client;",
        "pub mod config;",
        "pub use crate::client",
        "pub use crate::config",
        "RadrootsSdkClient",
        "RadrootsSdkConfig",
        "SdkTransportMode",
        "ProfileClient",
        "FarmClient",
        "ListingClient",
        "SdkPublishReceipt",
        "SdkTransportReceipt",
    ] {
        assert!(
            !source.contains(forbidden),
            "src/lib.rs must not expose legacy SDK client/config concept `{forbidden}`"
        );
    }
}

fn product_runtime_file_stays_on_boundary(relative_path: &str) {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(relative_path)
            .as_path(),
    );

    for forbidden in [
        "RadrootsSdkClient",
        "SdkTransportMode",
        "SdkPublishReceipt",
        "radrootsd",
        "publish_with_identity",
        "publish_parts_via_relay",
        "publish_listing_via_radrootsd",
        "publish_order_request_via_radrootsd",
        "publish_farm_via_radrootsd",
    ] {
        assert!(
            !source.contains(forbidden),
            "{relative_path} must not use legacy SDK client or transport concept `{forbidden}`"
        );
    }
}

fn read_source(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read source {}: {error}", path.display()))
}

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_source_files(root, &mut paths);
    paths.sort();
    paths
}

fn collect_rust_source_files(root: &Path, paths: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(root).unwrap_or_else(|error| {
        panic!(
            "failed to read source directory {}: {error}",
            root.display()
        )
    });

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "failed to inspect source directory {}: {error}",
                root.display()
            )
        });
        let path = entry.path();

        if path.is_dir() {
            collect_rust_source_files(path.as_path(), paths);
            continue;
        }

        if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            paths.push(path);
        }
    }
}
