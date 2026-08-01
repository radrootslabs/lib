use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const DEVIATIONS: &str = include_str!("../../../docs/implementation/deviations.toml");

#[test]
fn legacy_secret_dependencies_are_confined_to_publish_frozen_quarantines() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let crates = workspace.join("crates");
    let mut manifests = Vec::new();
    collect_manifests(&crates, &mut manifests);
    let mut legacy_consumers = BTreeSet::new();

    for path in manifests {
        let manifest = fs::read_to_string(&path).expect("read package manifest");
        if manifest.contains("radroots_secret_vault")
            || manifest.contains("radroots_protected_store")
        {
            assert!(
                manifest.contains("publish = false"),
                "legacy secret consumer must remain publish-frozen: {}",
                path.display()
            );
            let package = package_name(&manifest).expect("package name");
            legacy_consumers.insert(package.to_owned());
        }
    }

    assert_eq!(
        legacy_consumers,
        BTreeSet::from([
            "radroots_nostr_accounts".to_owned(),
            "radroots_protected_store".to_owned(),
            "radroots_runtime".to_owned(),
            "radroots_secret_vault".to_owned(),
            "radroots_simplex_agent_store".to_owned(),
        ])
    );
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

fn collect_manifests(root: &Path, manifests: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("read crates directory") {
        let path = entry.expect("crate entry").path();
        if path.is_dir() {
            let manifest = path.join("Cargo.toml");
            if manifest.is_file() {
                manifests.push(manifest);
            }
        }
    }
}

fn package_name(manifest: &str) -> Option<&str> {
    let package = manifest.split_once("[package]")?.1;
    package
        .lines()
        .skip(1)
        .find_map(|line| line.trim().strip_prefix("name = \"")?.strip_suffix('"'))
}
