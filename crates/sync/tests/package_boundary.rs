use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const README: &str = include_str!("../README.md");
const SOURCES: &[(&str, &str)] = &[
    ("engine.rs", include_str!("../src/engine.rs")),
    ("ingest.rs", include_str!("../src/ingest.rs")),
    ("policy.rs", include_str!("../src/policy.rs")),
    ("projection.rs", include_str!("../src/projection.rs")),
    ("pull.rs", include_str!("../src/pull.rs")),
    ("push.rs", include_str!("../src/push.rs")),
    ("status.rs", include_str!("../src/status.rs")),
];

#[test]
fn sync_depends_only_on_final_orchestration_boundaries() {
    for required in [
        "name = \"radroots_sync\"",
        "version = \"0.1.0-alpha\"",
        "publish = false",
        "[lib]\nname = \"radroots_sync\"",
        "default = [\"serde\"]",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing `{required}`"
        );
    }
    assert_eq!(
        dependency_keys(MANIFEST)
            .into_iter()
            .filter(|dependency| dependency.starts_with("radroots_"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "radroots_event",
            "radroots_event_codec",
            "radroots_protocol",
            "radroots_signing",
            "radroots_storage",
            "radroots_trade",
            "radroots_transport",
        ])
    );
    assert!(MANIFEST.contains("serde = { workspace = true, optional = true }"));
    for forbidden in [
        "radroots_event_store",
        "radroots_event_index",
        "radroots_outbox",
        "radroots_runtime_store",
        "radroots_transport_nostr",
    ] {
        assert!(!MANIFEST.contains(forbidden));
        assert!(!ROOT.contains(forbidden));
    }
    assert_eq!(
        declarations(ROOT, "pub mod "),
        BTreeSet::from(["ingest", "policy", "projection", "pull", "push", "status"])
    );
}

#[test]
fn sync_has_no_runtime_scheduler_or_process_lifecycle_authority() {
    for forbidden_dependency in [
        "tokio",
        "async-std",
        "smol",
        "rayon",
        "signal-hook",
        "ctrlc",
    ] {
        assert!(
            !dependency_keys(MANIFEST).contains(forbidden_dependency),
            "sync must not depend on runtime package `{forbidden_dependency}`"
        );
    }
    for (path, source) in SOURCES {
        for forbidden in [
            "tokio::",
            "async_std::",
            "smol::",
            "std::thread",
            "thread::spawn",
            ".spawn(",
            "Runtime::new",
            "new_multi_thread",
            "new_current_thread",
            "tokio::time",
            "SystemTime::now",
            "Instant::now",
            "ctrl_c",
            "signal_hook",
            "process::exit",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} must not own runtime behavior `{forbidden}`"
            );
        }
    }
    let readme = README.split_whitespace().collect::<Vec<_>>().join(" ");
    for explicit_boundary in [
        "does not create an executor",
        "spawn workers",
        "install timers",
        "own process lifecycle",
    ] {
        assert!(readme.contains(explicit_boundary));
    }
}

fn declarations<'a>(source: &'a str, prefix: &str) -> BTreeSet<&'a str> {
    source
        .lines()
        .filter_map(|line| line.trim().strip_prefix(prefix))
        .filter_map(|name| name.strip_suffix(';'))
        .collect()
}

fn dependency_keys(manifest: &str) -> BTreeSet<&str> {
    manifest
        .split_once("[dependencies]")
        .map(|(_, dependencies)| dependencies)
        .unwrap_or_default()
        .lines()
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim()))
        .filter(|key| !key.is_empty())
        .collect()
}
