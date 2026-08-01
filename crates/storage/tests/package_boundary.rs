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
