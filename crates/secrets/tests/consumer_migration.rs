use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const DEVIATIONS: &str = include_str!("../../../docs/implementation/deviations.toml");
const PUBLISH_POLICY: &str = include_str!("../../../contracts/releases/publish_policy.toml");
const SECRET_VAULT_MANIFEST: &str = include_str!("../../secret_vault/Cargo.toml");
const SECRET_VAULT_ROOT: &str = include_str!("../../secret_vault/src/lib.rs");
const SECRET_VAULT_README: &str = include_str!("../../secret_vault/README");
const PROTECTED_STORE_MANIFEST: &str = include_str!("../../protected_store/Cargo.toml");
const PROTECTED_STORE_ROOT: &str = include_str!("../../protected_store/src/lib.rs");
const PROTECTED_STORE_README: &str = include_str!("../../protected_store/README");

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

#[test]
fn superseded_packages_are_fail_closed_compatibility_quarantines() {
    for (package, manifest, root, readme) in [
        (
            "radroots_secret_vault",
            SECRET_VAULT_MANIFEST,
            SECRET_VAULT_ROOT,
            SECRET_VAULT_README,
        ),
        (
            "radroots_protected_store",
            PROTECTED_STORE_MANIFEST,
            PROTECTED_STORE_ROOT,
            PROTECTED_STORE_README,
        ),
    ] {
        for required in [
            "publish = false",
            "[package.metadata.radroots.compatibility]",
            "status = \"publish_frozen\"",
            "replacement = \"radroots_secrets\"",
            "deviation = \"RCRV1-DEV-008\"",
            "removal_step = 313",
            "new_consumers_forbidden = true",
        ] {
            assert!(
                manifest.contains(required),
                "{package} manifest is missing `{required}`"
            );
        }
        assert!(root.contains("#![doc(hidden)]"));
        assert!(root.contains("use `radroots_secrets` for new integrations"));
        assert!(readme.contains("Compatibility quarantine"));
        assert!(readme.contains("Step 313 removes this package"));
    }

    let approved = PUBLISH_POLICY
        .split_once("[workspace_classification]")
        .map(|(publication, _)| publication)
        .expect("workspace classification");
    for package in ["radroots_secret_vault", "radroots_protected_store"] {
        assert!(
            !approved.contains(package),
            "compatibility package cannot enter the approved release inventory: {package}"
        );
    }

    let private = PUBLISH_POLICY
        .split_once("private = [")
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(entries, _)| entries)
        .expect("private package classification");
    for package in ["radroots_secret_vault", "radroots_protected_store"] {
        assert!(
            private.contains(&format!("\"{package}\"")),
            "compatibility package must remain private: {package}"
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
