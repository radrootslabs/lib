use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README.md");
const PUBLIC_API: &str = include_str!("../../../contracts/api_baselines/radroots_service_host.txt");
const ROOT: &str = include_str!("../src/lib.rs");
const CONFIG_DOCUMENT_SOURCE: &str = include_str!("../src/config/document.rs");
const CONFIG_VALUE_SOURCE: &str = include_str!("../src/config/value.rs");
const ADMIN_SOURCE: &str = concat!(
    include_str!("../src/admin/mod.rs"),
    include_str!("../src/admin/client.rs"),
    include_str!("../src/admin/limits.rs"),
    include_str!("../src/admin/model.rs"),
    include_str!("../src/admin/peer.rs"),
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
const OPERATIONS_PRIMITIVES_SOURCE: &str = concat!(
    include_str!("../src/operations/mod.rs"),
    include_str!("../src/operations/config.rs"),
    include_str!("../src/operations/health.rs"),
    include_str!("../src/operations/metrics.rs"),
);
const OPERATIONS_SOURCE: &str = concat!(
    include_str!("../src/operations/mod.rs"),
    include_str!("../src/operations/server.rs"),
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
            "toml",
        ])
    );
    assert!(public_modules(ROOT).is_empty());
    assert_eq!(
        private_modules(ROOT),
        BTreeSet::from([
            "admin",
            "build_info",
            "config",
            "entropy",
            "error",
            "lifecycle",
            "operations",
            "status",
            "time",
        ])
    );
    assert!(ROOT.contains("pub use radroots_runtime_paths::{InstanceId, ServiceId};"));
    assert!(!STATUS_SOURCE.contains("serde(untagged)"));
    assert!(!ADMIN_SOURCE.contains("serde(untagged)"));
    for required in [
        ".take(REASON_CODES_MAX_ITEMS + 1)",
        "deserialize_seq(ReasonCodesVisitor)",
    ] {
        assert!(STATUS_SOURCE.contains(required));
    }
    for required in [
        "struct NonNullSerializer",
        "struct StrictJsonPayload",
        "encode_bounded(result, self.response_body_limit)",
    ] {
        assert!(ADMIN_SOURCE.contains(required));
    }
    for forbidden in [
        "serde_json::Value",
        "serde_json::to_value",
        "StrictJsonValue",
    ] {
        assert!(!ADMIN_SOURCE.contains(forbidden));
    }
    for forbidden in ["tokio::signal", "ctrl_c", "signal_hook"] {
        assert!(!LIFECYCLE_SOURCE.contains(forbidden));
    }
    for forbidden in ["TcpListener", "TcpStream", "tokio::spawn"] {
        assert!(!OPERATIONS_PRIMITIVES_SOURCE.contains(forbidden));
    }
    assert!(!OPERATIONS_SOURCE.contains("process::exit"));
    for source in [CONFIG_DOCUMENT_SOURCE, CONFIG_VALUE_SOURCE] {
        let production = source.split_once("#[cfg(test)]").unwrap().0;
        for forbidden in [
            "create_dir",
            "read_dir",
            "std::env",
            "TcpStream",
            "UdpSocket",
            "SystemTime",
            "MonotonicClock",
            "tokio::",
            "process::",
        ] {
            assert!(!production.contains(forbidden));
        }
    }
}

#[test]
fn documentation_and_reviewed_public_api_are_complete_and_dependency_safe() {
    for required in [
        "## Strict configuration values",
        "## Cached state and explicit cancellation",
        "## Local administration and operations",
        "## Process and runtime ownership",
        "## Supported targets and publication",
        "services_hardening_host.v1.json",
        "contracts/api_baselines/radroots_service_host.txt",
        "BoundedCount::<64>::new(8)",
        "cached_service_state(CachedServiceState::new",
        "parent.child_token()",
        "streamed directly into the capped response writer",
    ] {
        assert!(README.contains(required), "README is missing `{required}`");
    }

    for required in [
        "pub struct radroots_service_host::BuildInfo",
        "pub struct radroots_service_host::ConfigDocumentExpectation",
        "pub struct radroots_service_host::PositiveDuration",
        "pub struct radroots_service_host::CancellationToken",
        "pub struct radroots_service_host::TaskSupervisor",
        "pub struct radroots_service_host::AdminRouter",
        "pub struct radroots_service_host::OperationsServer",
        "pub struct radroots_service_host::BoundedMetricsSnapshot",
        "pub struct radroots_service_host::ServiceStatus",
        "pub trait radroots_service_host::MonotonicClock",
        "pub trait radroots_service_host::EntropySource",
    ] {
        assert!(
            PUBLIC_API.contains(required),
            "reviewed API baseline is missing `{required}`"
        );
    }

    for forbidden in [
        "pub mod radroots_service_host::admin",
        "pub mod radroots_service_host::config",
        "pub mod radroots_service_host::lifecycle",
        "pub mod radroots_service_host::operations",
        "pub mod radroots_service_host::status",
        "hyper::",
        "rustix::",
        "serde_json::",
        "tokio::",
        "tokio_util::",
    ] {
        assert!(
            !PUBLIC_API.contains(forbidden),
            "reviewed API baseline exposes `{forbidden}`"
        );
    }
}

fn public_modules(root: &str) -> BTreeSet<&str> {
    root.lines()
        .filter_map(|line| line.strip_prefix("pub mod "))
        .filter_map(|module| module.strip_suffix(';'))
        .collect()
}

fn private_modules(root: &str) -> BTreeSet<&str> {
    root.lines()
        .filter_map(|line| line.strip_prefix("mod "))
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
