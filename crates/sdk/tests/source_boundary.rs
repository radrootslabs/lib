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
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("farms_runtime.rs")
            .as_path(),
    );

    for forbidden in [
        "RadrootsSdkClient",
        "SdkTransportMode",
        "SdkPublishReceipt",
        "radrootsd",
        "publish_with_identity",
        "publish_parts_via_relay",
    ] {
        assert!(
            !source.contains(forbidden),
            "farms_runtime.rs must not use legacy SDK client or transport concept `{forbidden}`"
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
