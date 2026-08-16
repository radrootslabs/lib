const MANAGEMENT_FIXTURE: &str = include_str!("fixtures/hardened_service_management.v1.toml");
const MANAGER_ROOT_SOURCE: &str = include_str!("../src/lib.rs");
const MANAGER_SOURCE: &str = include_str!("../src/managed.rs");
const MODEL_SOURCE: &str = include_str!("../src/model.rs");
const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README");

fn production_source(source: &str) -> &str {
    source
        .split("\n#[cfg(test)]")
        .next()
        .expect("production source")
}

#[test]
fn hardened_services_remain_exact_metadata_only_targets() {
    for forbidden in [
        "active = [\"myc",
        "active = [\"rhi",
        "install_strategy",
        "binary_name",
        "artifact_adapter",
        "qualified",
        "default_instance_id",
        "preferred_cli_binding",
    ] {
        assert!(
            !MANAGEMENT_FIXTURE.contains(forbidden),
            "management fixture contains deferred authority `{forbidden}`"
        );
    }

    assert!(MANAGEMENT_FIXTURE.contains("defined = [\"myc\", \"rhi\"]"));
    assert!(MANAGEMENT_FIXTURE.contains("actions = []"));
    assert!(MANAGEMENT_FIXTURE.contains("destructive_actions = []"));
    assert!(MANAGEMENT_FIXTURE.contains("[bootstrap]"));
}

#[test]
fn management_contract_is_bounded_before_toml_admission() {
    assert!(
        MANAGER_ROOT_SOURCE.contains("if raw.len() > RUNTIME_MANAGEMENT_CONTRACT_MAX_UTF8_BYTES")
    );
    let bound = MANAGER_ROOT_SOURCE
        .find("if raw.len() > RUNTIME_MANAGEMENT_CONTRACT_MAX_UTF8_BYTES")
        .expect("pre-parser bound");
    let parser = MANAGER_ROOT_SOURCE
        .find("toml::from_str::<RadrootsRuntimeManagementContract>(raw)")
        .expect("TOML parser");
    assert!(
        bound < parser,
        "contract size must be checked before parsing"
    );
}

#[test]
fn public_package_contains_only_metadata_resolution_authority() {
    let production = [
        production_source(MANAGER_ROOT_SOURCE),
        production_source(MANAGER_SOURCE),
        production_source(MODEL_SOURCE),
    ]
    .join("\n");
    for forbidden in [
        "std::fs",
        "std::process",
        "std::path",
        "radroots_runtime_paths::RuntimeContext",
        "ManagedRuntimeArtifactName",
        "ManagedRuntimeInstancePaths",
        "ManagedRuntimeSharedPaths",
        "ManagedRuntimeInstanceRegistry",
        "ManagedRuntimeInstanceRecord",
        "load_registry",
        "save_registry",
        "register_instance",
        "remove_instance",
        "start_process",
        "stop_process",
        "process_running",
        "install_binary",
        "extract_binary_archive",
        "remove_instance_artifacts",
        "write_instance_config",
        "inspect_runtime_",
    ] {
        assert!(
            !production.contains(forbidden),
            "production surface retained `{forbidden}`"
        );
    }

    for forbidden_dependency in ["flate2", "tar =", "tempfile"] {
        assert!(
            !MANIFEST.contains(forbidden_dependency),
            "manifest retained `{forbidden_dependency}`"
        );
    }

    for required in [
        "performs no filesystem, registry, process, archive, artifact",
        "runtime paths or raw persistence helpers",
        "Steps 219 and 220",
    ] {
        assert!(README.contains(required), "README omitted `{required}`");
    }
}
