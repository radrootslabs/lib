use std::fs;
use std::path::{Path, PathBuf};

const REMOVED_CLIENT_NAMES: [&str; 5] = [
    "RadrootsNostrClient",
    "RadrootsNostrClientKey",
    "RadrootsNostrMonitor",
    "RadrootsNostrMonitorNotification",
    "RadrootsNostrRelayStatus",
];

#[test]
fn ready_workspace_consumers_do_not_import_the_removed_client_surface() {
    let workspace = workspace_root();
    for source in rust_sources(workspace.join("crates")) {
        let contents = fs::read_to_string(&source).expect("read workspace source");
        for removed in REMOVED_CLIENT_NAMES {
            assert!(
                !contents.contains(removed),
                "ready consumer {} imports removed `{removed}`",
                source.display()
            );
        }
    }
}

#[test]
fn superseded_transport_packages_and_nostrdb_adapter_are_removed() {
    let workspace = workspace_root();
    assert!(!workspace.join("crates/nostr_runtime/Cargo.toml").exists());
    assert!(!workspace.join("crates/net/Cargo.toml").exists());
    let workspace_manifest =
        fs::read_to_string(workspace.join("Cargo.toml")).expect("workspace manifest");
    let nostrdb_manifest =
        fs::read_to_string(workspace.join("crates/nostrdb/Cargo.toml")).expect("nostrdb manifest");
    let deviations = fs::read_to_string(workspace.join("contracts/architecture/deviations.toml"))
        .expect("deviation authority");

    assert!(!workspace_manifest.contains("radroots_nostr_runtime"));
    assert!(!workspace_manifest.contains("radroots_net"));
    assert!(!nostrdb_manifest.contains("runtime-adapter"));
    assert!(deviations.contains(
        "Delete radroots_nostr_runtime, its NostrDB runtime adapter, and radroots_net during Step 301 qualification"
    ));
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn rust_sources(root: PathBuf) -> Vec<PathBuf> {
    let mut pending = vec![root];
    let mut sources = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).expect("read source directory") {
            let path = entry.expect("source entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs")
                && path
                    .components()
                    .any(|component| component.as_os_str() == "src")
            {
                sources.push(path);
            }
        }
    }
    sources.sort();
    sources
}
