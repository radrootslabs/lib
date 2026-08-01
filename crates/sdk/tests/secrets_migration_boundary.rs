use std::fs;
use std::path::Path;

const ROOT_MANIFEST: &str = include_str!("../../../Cargo.toml");
const PACKAGE_MANIFEST: &str = include_str!("../Cargo.toml");
const CARGO_CONFIG: &str = include_str!("../../../.cargo/config.toml");
const PRIVATE_STORE: &str = include_str!("../src/private_store.rs");
const DEVIATIONS: &str = include_str!("../../../docs/implementation/deviations.toml");

#[test]
fn final_secret_dependency_is_activated_at_the_sdk_boundary() {
    assert!(ROOT_MANIFEST.contains(
        "radroots_secrets = { package = \"radroots_secrets\", version = \"=0.1.0-alpha\", default-features = false }"
    ));
    assert!(PACKAGE_MANIFEST.contains(
        "radroots_secrets = { workspace = true, optional = true, default-features = false }"
    ));
    assert!(PACKAGE_MANIFEST.contains("\"dep:radroots_secrets\""));
    assert!(CARGO_CONFIG.contains("radroots_secrets = { path = \"../lib/crates/secrets\" }"));
}

#[test]
fn predecessor_secret_imports_are_confined_to_the_private_store_quarantine() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut legacy_sources = Vec::new();
    collect_rust_sources(&source_root, &mut legacy_sources);
    legacy_sources.retain(|path| {
        let source = fs::read_to_string(path).expect("read SDK source");
        source.contains("radroots_protected_store") || source.contains("radroots_secret_vault")
    });

    assert_eq!(legacy_sources, vec![source_root.join("private_store.rs")]);
    assert!(PRIVATE_STORE.contains("RCRV1-DEV-008"));
    assert!(PRIVATE_STORE.contains("Step 179 transfers the private store"));
}

#[test]
fn quarantine_has_exact_future_removal_gates() {
    for required in [
        "id = \"RCRV1-DEV-008\"",
        "affected_steps = [\"153\", \"155\", \"171\", \"179\", \"226\", \"288\", \"293\", \"313\"]",
        "Step 179 transfers canonical private storage",
        "Step 313 removes every remaining compatibility package and legacy name",
    ] {
        assert!(
            DEVIATIONS.contains(required),
            "secret consumer quarantine is missing `{required}`"
        );
    }
}

fn collect_rust_sources(root: &Path, sources: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(root).expect("read SDK source directory") {
        let path = entry.expect("SDK source entry").path();
        if path.is_dir() {
            collect_rust_sources(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
    sources.sort();
}
