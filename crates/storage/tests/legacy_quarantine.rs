const DEVIATIONS: &str = include_str!("../../../docs/implementation/deviations.toml");
const PUBLISH_POLICY: &str = include_str!("../../../contracts/releases/publish_policy.toml");

const LEGACY_PACKAGES: &[(&str, &str, &str, &str)] = &[
    (
        "radroots_event_index",
        include_str!("../../event_index/Cargo.toml"),
        include_str!("../../event_index/src/lib.rs"),
        include_str!("../../event_index/README"),
    ),
    (
        "radroots_event_store",
        include_str!("../../event_store/Cargo.toml"),
        include_str!("../../event_store/src/lib.rs"),
        include_str!("../../event_store/README"),
    ),
    (
        "radroots_outbox",
        include_str!("../../outbox/Cargo.toml"),
        include_str!("../../outbox/src/lib.rs"),
        include_str!("../../outbox/README"),
    ),
    (
        "radroots_runtime_store",
        include_str!("../../runtime_store/Cargo.toml"),
        include_str!("../../runtime_store/src/lib.rs"),
        include_str!("../../runtime_store/README"),
    ),
];

#[test]
fn superseded_storage_packages_are_fail_closed_compatibility_quarantines() {
    let approved = PUBLISH_POLICY
        .split_once("[workspace_classification]")
        .map(|(publication, _)| publication)
        .expect("workspace classification");
    let private = PUBLISH_POLICY
        .split_once("private = [")
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(entries, _)| entries)
        .expect("private package classification");

    for (package, manifest, root, readme) in LEGACY_PACKAGES {
        for required in [
            "publish = false",
            "[package.metadata.radroots.compatibility]",
            "status = \"publish_frozen\"",
            "deviation = \"RCRV1-DEV-009\"",
            "removal_step = 313",
            "new_consumers_forbidden = true",
        ] {
            assert!(
                manifest.contains(required),
                "{package} manifest is missing `{required}`"
            );
        }
        assert!(
            !manifest.contains("documentation = \"https://docs.rs/"),
            "{package} must not advertise a public docs.rs surface"
        );
        assert!(root.contains("#![doc(hidden)]"));
        assert!(readme.contains("## Compatibility quarantine"));
        assert!(readme.contains("Step 313 removes this package"));
        assert!(
            !approved.contains(package),
            "compatibility package cannot enter the approved release inventory: {package}"
        );
        assert!(
            private.contains(&format!("\"{package}\"")),
            "compatibility package must remain private: {package}"
        );
    }
}

#[test]
fn quarantine_records_external_consumers_and_exact_removal_gate() {
    for required in [
        "id = \"RCRV1-DEV-009\"",
        "The Step 170 first-party census found radroots_event_index consumers",
        "radroots_event_store and radroots_outbox consumers",
        "radroots_runtime_store consumed by the standalone CLI",
        "delete every remaining package at Step 313",
    ] {
        assert!(
            DEVIATIONS.contains(required),
            "storage quarantine is missing `{required}`"
        );
    }
}
