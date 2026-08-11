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
    assert!(!ROOT.contains("pub mod namespace;"));
    for source in SOURCES {
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        assert!(!production.contains("std::env"));
    }
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
        "pub fn radroots_runtime_paths::context::RuntimeContext::resolve",
    ] {
        assert!(
            PUBLIC_API.contains(required),
            "reviewed API baseline is missing `{required}`"
        );
    }

    for required in [
        "sole canonical",
        "sealed `RuntimeContext`",
        "process-environment constructor",
        "intentionally removed",
    ] {
        assert!(README.contains(required), "README is missing `{required}`");
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
