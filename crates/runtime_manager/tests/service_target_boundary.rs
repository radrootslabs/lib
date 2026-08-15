const MANAGEMENT_FIXTURE: &str = include_str!("fixtures/hardened_service_management.v1.toml");
const MANAGER_ROOT_SOURCE: &str = include_str!("../src/lib.rs");

#[test]
fn hardened_services_remain_metadata_only_in_management_contract() {
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
