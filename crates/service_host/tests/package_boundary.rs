use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const ADMIN_SOURCE: &str = concat!(
    include_str!("../src/admin/mod.rs"),
    include_str!("../src/admin/client.rs"),
    include_str!("../src/admin/limits.rs"),
    include_str!("../src/admin/model.rs"),
    include_str!("../src/admin/server.rs"),
    include_str!("../src/admin/unix.rs"),
);
const STATUS_SOURCE: &str = concat!(
    include_str!("../src/status/mod.rs"),
    include_str!("../src/status/phase.rs"),
    include_str!("../src/status/reason.rs"),
    include_str!("../src/status/service.rs"),
);
const LIFECYCLE_SOURCE: &str = concat!(
    include_str!("../src/lifecycle/mod.rs"),
    include_str!("../src/lifecycle/cancel.rs"),
    include_str!("../src/lifecycle/shutdown.rs"),
    include_str!("../src/lifecycle/signal.rs"),
    include_str!("../src/lifecycle/supervisor.rs"),
    include_str!("../src/lifecycle/task.rs"),
);

#[test]
fn service_host_is_unpublished_lint_governed_and_dependency_bounded() {
    for required in [
        "name = \"radroots_service_host\"",
        "publish = false",
        "version = \"0.1.0-alpha\"",
        "[lints]\nworkspace = true",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing `{required}`"
        );
    }

    assert_eq!(
        dependency_keys(MANIFEST),
        BTreeSet::from([
            "bytes",
            "fs2",
            "getrandom",
            "http",
            "http-body-util",
            "hyper",
            "hyper-util",
            "radroots_runtime_paths",
            "rustix",
            "serde",
            "serde_json",
            "tokio",
            "tokio-util",
        ])
    );
    assert_eq!(
        public_modules(ROOT),
        BTreeSet::from([
            "admin",
            "build_info",
            "entropy",
            "error",
            "lifecycle",
            "status",
            "time",
        ])
    );
    assert!(!STATUS_SOURCE.contains("serde(untagged)"));
    assert!(!ADMIN_SOURCE.contains("serde(untagged)"));
    for forbidden in ["tokio::signal", "ctrl_c", "signal_hook"] {
        assert!(!LIFECYCLE_SOURCE.contains(forbidden));
    }
}

fn public_modules(root: &str) -> BTreeSet<&str> {
    root.lines()
        .filter_map(|line| line.strip_prefix("pub mod "))
        .filter_map(|module| module.strip_suffix(';'))
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
