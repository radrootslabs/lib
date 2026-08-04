use std::{fs, path::Path};

const RETIRED: &[&str] = &["event_index", "event_store", "outbox", "runtime_store"];

#[test]
fn predecessor_storage_packages_are_removed() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let manifest = fs::read_to_string(workspace.join("Cargo.toml")).expect("workspace manifest");

    for package in RETIRED {
        assert!(!workspace.join("crates").join(package).exists());
        assert!(!manifest.contains(&format!("\"crates/{package}\"")));
        assert!(!manifest.contains(&format!("radroots_{package} =")));
    }
}

#[test]
fn active_manifests_use_only_final_storage_packages() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates root");
    for entry in fs::read_dir(crates).expect("crates directory") {
        let manifest = entry.expect("crate entry").path().join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let source = fs::read_to_string(&manifest).expect("crate manifest");
        for package in RETIRED {
            let retired = format!("radroots_{package}");
            assert!(
                !source.contains(&retired),
                "{} retains {retired}",
                manifest.display()
            );
        }
    }
}
