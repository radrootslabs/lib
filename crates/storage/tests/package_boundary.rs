use std::fs;
use std::path::PathBuf;

#[test]
fn manifest_matches_the_release_v1_package_boundary() {
    let manifest = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("storage package manifest must be readable");

    for required in [
        "radroots_event",
        "radroots_protocol",
        "radroots_trade",
        "radroots_transport",
    ] {
        assert!(
            manifest.contains(&format!("{required} = {{ workspace = true")),
            "missing required Radroots dependency {required}"
        );
    }

    for forbidden in ["sqlx", "rusqlite", "reqwest", "nostr-sdk"] {
        assert!(
            !manifest.contains(forbidden),
            "storage SPI must not depend on {forbidden}"
        );
    }

    for feature in [
        "default = [\"memory\", \"serde\"]",
        "memory = []",
        "serde = [",
    ] {
        assert!(manifest.contains(feature), "missing feature law: {feature}");
    }
    for forbidden_feature in ["sqlite =", "tokio =", "nostr =", "runtime ="] {
        assert!(
            !manifest.contains(forbidden_feature),
            "storage SPI declares forbidden feature {forbidden_feature}"
        );
    }
}

#[test]
fn release_v1_public_module_skeleton_is_declared() {
    let root = include_str!("../src/lib.rs");
    for module in [
        "atomic",
        "backup",
        "event",
        "journal",
        "memory",
        "outbox",
        "private_artifact",
        "projection",
        "status",
    ] {
        assert!(
            root.contains(&format!("pub mod {module};")),
            "missing public module {module}"
        );
    }
}

#[test]
fn root_exports_are_exact_and_implementation_types_do_not_leak() {
    let root = include_str!("../src/lib.rs");
    for required in [
        "pub trait Storage:",
        "pub use backup::StorageReliability as BackupSource;",
        "pub use error::Error;",
        "pub use event::EventStore;",
        "pub use journal::Journal;",
        "pub use outbox::Outbox;",
        "pub use projection::ProjectionStore;",
        "pub use status::StorageStatus;",
    ] {
        assert!(root.contains(required), "missing root contract: {required}");
    }
    for forbidden in [
        "pub use atomic::AtomicStorage",
        "pub use backup::StorageReliability;",
        "pub use private_artifact::PrivateArtifactStore",
    ] {
        assert!(
            !root.contains(forbidden),
            "forbidden root export: {forbidden}"
        );
    }

    let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut source = String::new();
    for entry in fs::read_dir(source_root).expect("source directory") {
        let path = entry.expect("source entry").path();
        if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            source.push_str(&fs::read_to_string(path).expect("source file"));
        }
    }
    for forbidden in [
        "sqlx::",
        "rusqlite::",
        "nostr_sdk::",
        "std::path::Path",
        "tokio::runtime",
    ] {
        assert!(
            !source.contains(forbidden),
            "public storage source leaks implementation path {forbidden}"
        );
    }
}
