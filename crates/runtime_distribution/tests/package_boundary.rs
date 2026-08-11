const SERVICE_SOURCE: &str = include_str!("../src/service.rs");
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
