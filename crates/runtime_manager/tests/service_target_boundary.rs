const MANAGEMENT_FIXTURE: &str = include_str!("fixtures/hardened_service_management.v1.toml");
const MANAGER_ROOT_SOURCE: &str = include_str!("../src/lib.rs");
const MANAGER_SOURCE: &str = include_str!("../src/managed.rs");
const CLI_SOURCE: &str = include_str!("../src/cli.rs");
const MODEL_SOURCE: &str = include_str!("../src/model.rs");
#[cfg(any(target_os = "linux", target_os = "macos"))]
const STATUS_SOURCE: &str = include_str!("../src/status.rs");
const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README");

fn production_source(source: &str) -> &str {
    source
        .split("\n#[cfg(test)]")
        .next()
        .expect("production source")
}

#[test]
fn hardened_services_use_only_the_common_context_bound_interfaces() {
    for forbidden in [
        "install_strategy",
        "binary_name",
        "artifact_adapter",
        "qualified",
        "default_instance_id",
        "preferred_cli_binding",
        "typed_instance_registry",
        "instances.toml",
        "explicit_runtime_endpoint_overrides",
        "[paths.",
        "[bootstrap]",
    ] {
        assert!(
            !MANAGEMENT_FIXTURE.contains(forbidden),
            "management fixture contains deferred authority `{forbidden}`"
        );
    }

    assert!(MANAGEMENT_FIXTURE.contains("active = [\"myc\", \"rhi\"]"));
    assert!(MANAGEMENT_FIXTURE.contains("active = [\"cli_v1\", \"unix_admin_v1\"]"));
    assert!(MANAGEMENT_FIXTURE.contains(
        "actions = [\"config_init\", \"config_validate\", \"state_init\", \"run\", \"status\", \"doctor\"]"
    ));
    assert!(MANAGEMENT_FIXTURE.contains("destructive_actions = []"));
    assert!(MANAGEMENT_FIXTURE.contains("runtime_binding = \"typed_runtime_context\""));
    assert!(MANAGEMENT_FIXTURE.contains("admin_endpoint = \"runtime_context_admin_socket\""));
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
fn public_package_contains_only_typed_cli_and_bounded_admin_authority() {
    let sources = [
        production_source(MANAGER_ROOT_SOURCE),
        production_source(MANAGER_SOURCE),
        production_source(CLI_SOURCE),
        production_source(MODEL_SOURCE),
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        production_source(STATUS_SOURCE),
    ];
    let production = sources.join("\n");
    for forbidden in [
        "std::fs",
        "std::process",
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
        "read_secret",
        "credential_path",
        "binary_name",
        "archive_name",
        "install_path",
    ] {
        assert!(
            !production.contains(forbidden),
            "production surface retained `{forbidden}`"
        );
    }

    for forbidden_dependency in ["flate2", "tar ="] {
        assert!(
            !MANIFEST.contains(forbidden_dependency),
            "manifest retained `{forbidden_dependency}`"
        );
    }

    for required in [
        "sole service, instance,\nprofile, and canonical-path authority",
        "exact CLI-v1 argument\nplans",
        "fixed `/v1/status`",
        "never discovers or executes a program",
        "Artifact and\ndistribution resolution remains separately governed by Step220",
    ] {
        assert!(README.contains(required), "README omitted `{required}`");
    }

    for required in ["RuntimeContext", "ManagedCliInvocation"] {
        assert!(
            production.contains(required),
            "production omitted `{required}`"
        );
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    for required in ["AdminClient", "AdminClientTarget", "STATUS_V1_TARGET"] {
        assert!(
            production.contains(required),
            "native production omitted `{required}`"
        );
    }
}
