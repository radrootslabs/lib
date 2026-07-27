const MANIFEST: &str = include_str!("../Cargo.toml");

#[test]
fn manifest_has_no_host_persistence_feature_or_dependency() {
    for forbidden in [
        "json-file",
        "radroots_protected_store",
        "radroots_runtime",
        "radroots_runtime_paths",
        "radroots_secret_vault",
        "tracing",
        "tempfile",
    ] {
        assert!(
            !MANIFEST.contains(forbidden),
            "identity manifest must not contain host persistence edge {forbidden}"
        );
    }

    assert!(MANIFEST.contains("default = [\"std\", \"serde\"]"));
    assert!(MANIFEST.contains("std = [\"thiserror/std\"]"));
}
