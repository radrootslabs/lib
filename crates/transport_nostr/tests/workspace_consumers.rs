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
    let allowed = [
        workspace.join("crates/net/src"),
        workspace.join("crates/nostr_runtime/src"),
    ];

    for source in rust_sources(workspace.join("crates")) {
        if allowed.iter().any(|root| source.starts_with(root)) {
            continue;
        }
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
fn predecessor_runtime_is_publish_frozen_and_not_default_reachable() {
    let workspace = workspace_root();
    let manifest = fs::read_to_string(workspace.join("crates/nostr_runtime/Cargo.toml"))
        .expect("runtime manifest");
    let nostrdb_manifest =
        fs::read_to_string(workspace.join("crates/nostrdb/Cargo.toml")).expect("nostrdb manifest");
    let readme =
        fs::read_to_string(workspace.join("crates/nostr_runtime/README")).expect("runtime readme");
    let deviations = fs::read_to_string(workspace.join("docs/implementation/deviations.toml"))
        .expect("deviation authority");

    assert!(manifest.contains("publish = false"));
    assert!(manifest.contains("default = [\"std\"]"));
    assert!(!manifest.contains("default = [\"std\", \"nostr-client\""));
    assert!(
        !nostrdb_manifest.contains("default = [\"std\", \"rt\", \"nostrdb\", \"runtime-adapter\"]")
    );
    assert!(readme.contains("RCRV1-DEV-007"));
    assert!(readme.contains("radroots_sync"));
    assert!(deviations.contains("id = \"RCRV1-DEV-007\""));
    assert!(deviations.contains("affected_steps = [\"122\", \"170\", \"215\", \"235\", \"305\"]"));
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
