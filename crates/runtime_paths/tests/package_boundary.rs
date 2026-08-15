use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README");
const PUBLIC_API: &str =
    include_str!("../../../contracts/api_baselines/radroots_runtime_paths.txt");
const ROOT: &str = include_str!("../src/lib.rs");
const SOURCES: &[&str] = &[
    include_str!("../src/context.rs"),
    include_str!("../src/conventions.rs"),
    include_str!("../src/error.rs"),
    include_str!("../src/identifier.rs"),
    include_str!("../src/platform.rs"),
    include_str!("../src/roots.rs"),
    include_str!("../src/service.rs"),
];

#[test]
fn runtime_paths_is_unpublished_lint_governed_and_dependency_bounded() {
    for required in [
        "name = \"radroots_runtime_paths\"",
        "publish = false",
        "version = \"0.1.0-alpha\"",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing `{required}`"
        );
    }
    assert_eq!(
        dependency_keys(MANIFEST),
        BTreeSet::from(["serde", "thiserror"])
    );
    assert_eq!(
        ROOT.lines()
            .filter_map(|line| line.strip_prefix("mod "))
            .filter_map(|line| line.strip_suffix(';'))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "context",
            "conventions",
            "error",
            "identifier",
            "platform",
            "roots",
            "service",
        ])
    );
    assert!(!ROOT.contains("pub mod "));
    assert!(ROOT.contains("#![doc = include_str!(\"../README\")]"));
    for source in SOURCES {
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        assert!(!production.contains("std::env"));
        for forbidden in [
            "impl Into<String>",
            "let value = value.into();",
            "String::deserialize",
        ] {
            assert!(
                !production.contains(forbidden),
                "bounded runtime-path text boundary still contains `{forbidden}`"
            );
        }
    }
    assert!(
        SOURCES
            .iter()
            .any(|source| source.contains("deserializer.deserialize_str(Visitor)"))
    );
}

#[test]
fn removed_namespace_selection_and_raw_root_surfaces_cannot_return() {
    for forbidden in [
        "RadrootsRuntimeNamespace",
        "RadrootsRuntimeNamespaceKind",
        "RadrootsServiceInstanceNamespace",
        "RadrootsRuntimePathSelection",
        "RadrootsRuntimePathConfigEntry",
        "RadrootsRuntimePathPolicyContract",
        "RadrootsRuntimeSelectionContract",
        "RadrootsRuntimeSelectionOverrideContract",
        "RadrootsPathOverrides",
        "RadrootsBootstrapPaths",
        "default_service_instance_paths",
        "default_namespaced_bootstrap_paths",
        "default_shared_identity_path",
        "default_shared_runtime_logs_dir",
        "from_current_process",
        "MissingMobileRoots",
        "InvalidNamespaceComponent",
    ] {
        assert!(
            SOURCES.iter().all(|source| !source.contains(forbidden)) && !ROOT.contains(forbidden),
            "removed runtime-path surface `{forbidden}` returned"
        );
    }

    for forbidden in [
        "pub struct radroots_runtime_paths::RadrootsPaths",
        "pub struct radroots_runtime_paths::RadrootsPathOverrides",
        "pub struct radroots_runtime_paths::RadrootsRuntimeNamespace",
        "pub struct radroots_runtime_paths::RadrootsServiceInstanceNamespace",
        "pub struct radroots_runtime_paths::RadrootsRuntimePathSelection",
        "pub fn radroots_runtime_paths::RadrootsPathResolver::current",
        "pub fn radroots_runtime_paths::RadrootsPathResolver::resolve",
    ] {
        assert!(
            !PUBLIC_API.contains(forbidden),
            "reviewed API baseline exposes `{forbidden}`"
        );
    }
}

#[test]
fn reviewed_api_requires_the_typed_runtime_context_boundary() {
    for required in [
        "pub struct radroots_runtime_paths::RuntimeContext",
        "pub struct radroots_runtime_paths::RuntimeContextBootstrap",
        "pub enum radroots_runtime_paths::RuntimeContextSource",
        "pub struct radroots_runtime_paths::RadrootsPathResolver",
        "pub struct radroots_runtime_paths::RadrootsServiceInstancePaths",
        "pub struct radroots_runtime_paths::RadrootsServiceInstanceArtifacts",
        "pub struct radroots_runtime_paths::ServiceId",
        "pub struct radroots_runtime_paths::InstanceId",
        "pub fn radroots_runtime_paths::RuntimeContext::resolve",
        "pub fn radroots_runtime_paths::RadrootsPlatform::current",
        "pub fn radroots_runtime_paths::default_service_instance_artifacts",
        "pub fn radroots_runtime_paths::service_credential_artifact_path",
        "pub fn radroots_runtime_paths::default_shared_geonames_database_path_from_cache_root",
        "pub fn radroots_runtime_paths::default_shared_runtime_store_database_path_from_data_root",
    ] {
        assert!(
            PUBLIC_API.contains(required),
            "reviewed API baseline is missing `{required}`"
        );
    }

    for required in [
        "## Example",
        "## Root Profiles",
        "## Common Artifacts",
        "## Support Caveats",
        "## Public API Baseline",
        "The final reviewed root-only API",
        "validated as\n  borrowed UTF-8 before the crate creates their retained strings",
        "```rust",
        "| Linux `ServiceHost` | `/etc/radroots` | `/var/lib/radroots` | `/var/cache/radroots` | `/var/log/radroots` | `/run/radroots` | `/etc/radroots/secrets` |",
        "| Linux `InteractiveUser` | `$XDG_CONFIG_HOME/radroots` | `$XDG_DATA_HOME/radroots` | `$XDG_CACHE_HOME/radroots` | `$XDG_STATE_HOME/radroots/logs` | `$XDG_RUNTIME_DIR/radroots` | `$XDG_CONFIG_HOME/radroots/secrets` |",
        "| macOS `InteractiveUser` | `$HOME/Library/Application Support/Radroots/config` | `$HOME/Library/Application Support/Radroots/data` | `$HOME/Library/Caches/Radroots` | `$HOME/Library/Logs/Radroots` | `$HOME/Library/Application Support/Radroots/run` | `$HOME/Library/Application Support/Radroots/secrets` |",
        r"| Windows `InteractiveUser` | `%APPDATA%\Radroots\config` | `%LOCALAPPDATA%\Radroots\data` | `%LOCALAPPDATA%\Radroots\cache` | `%LOCALAPPDATA%\Radroots\logs` | `%LOCALAPPDATA%\Radroots\run` | `%APPDATA%\Radroots\secrets` |",
        "| `RepoLocal` | `<base>/config` | `<base>/data` | `<base>/cache` | `<base>/logs` | `<base>/run` | `<base>/secrets` |",
        "| Configuration | `<config>/config.toml` |",
        "| SQLite state | `<state>/state.sqlite` |",
        "| SQLite writer lock | `<state>/state.lock` |",
        "| Local admin socket | `<run>/admin.sock` |",
        "| Credential artifact | `<secrets>/<validated-credential-name>` |",
        "It performs\nno filesystem I/O, creates no directories, and never reads the ambient process",
        "An absolute\n`XDG_RUNTIME_DIR` is mandatory and has no fallback.",
        "Linux service-host\non x86_64 and aarch64 is eligible for Tier 1 only after all release gates pass.",
        "Linux and macOS interactive and explicit repo-local profiles on x86_64 and\naarch64 are developer-target behavior.",
        "Linux rootless OCI on x86_64 and\naarch64 is also only a target",
        "Windows interactive and repo-local derivation is\nimplemented but unsupported and carries no v1 support claim.",
        "Non-Linux `ServiceHost`, `MobileNative`, Android/iOS/Other interactive,",
        "Repo-local never",
        "Successful compilation or path derivation does not change",
        "[runtime-paths API baseline](../../contracts/api_baselines/radroots_runtime_paths.txt)",
    ] {
        assert!(README.contains(required), "README is missing `{required}`");
    }

    for forbidden in [
        "pub mod radroots_runtime_paths::context",
        "pub mod radroots_runtime_paths::conventions",
        "pub mod radroots_runtime_paths::error",
        "pub mod radroots_runtime_paths::identifier",
        "pub mod radroots_runtime_paths::platform",
        "pub mod radroots_runtime_paths::roots",
        "pub mod radroots_runtime_paths::service",
        "radroots_runtime_paths::context::",
        "radroots_runtime_paths::conventions::",
        "radroots_runtime_paths::error::",
        "radroots_runtime_paths::identifier::",
        "radroots_runtime_paths::platform::",
        "radroots_runtime_paths::roots::",
        "radroots_runtime_paths::service::",
        "thiserror::",
        "serde_json::",
    ] {
        assert!(
            !PUBLIC_API.contains(forbidden),
            "reviewed API baseline exposes `{forbidden}`"
        );
    }
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
