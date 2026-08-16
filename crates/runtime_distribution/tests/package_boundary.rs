const SERVICE_SOURCE: &str = include_str!("../src/service.rs");
const RESOLVER_SOURCE: &str = include_str!("../src/resolve.rs");
const ROOT_SOURCE: &str = include_str!("../src/lib.rs");
const SERVICE_FIXTURE: &str = include_str!("fixtures/hardened_service_targets.v1.toml");

#[test]
fn hardened_service_metadata_has_no_artifact_or_runtime_authority() {
    for forbidden in [
        "binary_name",
        "package_name",
        "artifact_adapter",
        "default_channel",
        "[[runtime]]",
        "qualified",
    ] {
        assert!(
            !SERVICE_SOURCE.contains(forbidden) && !SERVICE_FIXTURE.contains(forbidden),
            "hardened service metadata contains deferred authority `{forbidden}`"
        );
    }

    for forbidden in [
        "std::fs",
        "std::process",
        "tokio",
        "hyper",
        "reqwest",
        "UnixListener",
        "TcpListener",
    ] {
        assert!(
            !SERVICE_SOURCE.contains(forbidden),
            "service metadata owns forbidden runtime behavior `{forbidden}`"
        );
    }
}

#[test]
fn contract_parser_is_bounded_before_toml_admission() {
    assert!(
        RESOLVER_SOURCE.contains("if raw.len() > RUNTIME_DISTRIBUTION_CONTRACT_MAX_UTF8_BYTES")
    );
    let bound = RESOLVER_SOURCE
        .find("if raw.len() > RUNTIME_DISTRIBUTION_CONTRACT_MAX_UTF8_BYTES")
        .expect("pre-parser bound");
    let parser = RESOLVER_SOURCE
        .find("toml::from_str::<RadrootsRuntimeDistributionContract>(raw)")
        .expect("TOML parser");
    assert!(
        bound < parser,
        "contract size must be checked before parsing"
    );
}

#[test]
fn distribution_test_rows_use_only_neutral_fixture_identities() {
    for required in [
        "fixture-desktop-bundle",
        "fixture_desktop_bundle",
        "fixture-mobile-package",
        "fixture_mobile_package",
    ] {
        assert!(
            ROOT_SOURCE.contains(required),
            "missing `{required}` fixture"
        );
    }

    let retired_product_name = ["radroots_", "studio_app"].concat();
    assert!(
        !ROOT_SOURCE.contains(&retired_product_name),
        "active distribution source retained a retired product fixture"
    );
}
