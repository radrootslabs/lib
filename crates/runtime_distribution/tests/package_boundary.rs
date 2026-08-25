const SERVICE_SOURCE: &str = include_str!("../src/service.rs");
const SERVICE_ARTIFACT_SOURCE: &str = include_str!("../src/service_artifact.rs");
const RESOLVER_SOURCE: &str = include_str!("../src/resolve.rs");
const ROOT_SOURCE: &str = include_str!("../src/lib.rs");
const SERVICE_FIXTURE: &str = include_str!("fixtures/hardened_service_targets.v1.toml");

#[test]
fn hardened_service_artifacts_are_closed_metadata_without_runtime_authority() {
    for forbidden in [
        "artifact_adapter",
        "default_channel",
        "[[runtime]]",
        "qualified",
    ] {
        assert!(
            !SERVICE_ARTIFACT_SOURCE.contains(forbidden) && !SERVICE_FIXTURE.contains(forbidden),
            "hardened service artifact metadata contains forbidden authority `{forbidden}`"
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
            !SERVICE_SOURCE.contains(forbidden) && !SERVICE_ARTIFACT_SOURCE.contains(forbidden),
            "service metadata owns forbidden runtime behavior `{forbidden}`"
        );
    }

    for required in [
        "service_artifacts.myc",
        "service_artifacts.rhi",
        "binary.tar.gz",
        "artifact-manifest.v1.json",
        "SHA256SUMS",
        "service-source.tar.gz",
        "sbom.cdx.json",
        "provenance-input.v1.json",
    ] {
        assert!(SERVICE_FIXTURE.contains(required), "missing `{required}`");
    }
    assert!(ROOT_SOURCE.contains("mod service_artifact;"));
    assert!(!ROOT_SOURCE.contains("pub mod service_artifact;"));
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
