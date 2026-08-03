const DEVIATIONS: &str = include_str!("../../../docs/implementation/deviations.toml");
const SHIMS: &str = include_str!("../../../docs/implementation/COMPATIBILITY_SHIMS.md");
const PUBLISH_POLICY: &str = include_str!("../../../contracts/releases/publish_policy.toml");

const PREDECESSORS: &[(&str, &str, &str, &str, &str, &str)] = &[
    (
        "radroots_nostr_runtime",
        "radroots_transport_nostr + radroots_sync",
        "215",
        include_str!("../../nostr_runtime/Cargo.toml"),
        include_str!("../../nostr_runtime/src/lib.rs"),
        include_str!("../../nostr_runtime/README"),
    ),
    (
        "radroots_net",
        "radroots_transport + radroots_sync + radroots_sdk",
        "313",
        include_str!("../../net/Cargo.toml"),
        include_str!("../../net/src/lib.rs"),
        include_str!("../../net/README"),
    ),
];

#[test]
fn superseded_transport_packages_are_fail_closed_compatibility_quarantines() {
    let approved = PUBLISH_POLICY
        .split_once("[workspace_classification]")
        .map(|(publication, _)| publication)
        .expect("workspace classification");
    let private = PUBLISH_POLICY
        .split_once("private = [")
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(entries, _)| entries)
        .expect("private package classification");

    for (package, replacement, removal_step, manifest, root, readme) in PREDECESSORS {
        for required in [
            "publish = false",
            "[package.metadata.radroots.compatibility]",
            "status = \"publish_frozen\"",
            "deviation = \"RCRV1-DEV-010\"",
            "new_consumers_forbidden = true",
        ] {
            assert!(
                manifest.contains(required),
                "{package} manifest is missing `{required}`"
            );
        }
        assert!(manifest.contains(&format!("replacement = \"{replacement}\"")));
        assert!(manifest.contains(&format!("removal_step = {removal_step}")));
        assert!(!manifest.contains("documentation = \"https://docs.rs/"));
        assert!(root.starts_with("#![doc(hidden)]"));
        assert!(readme.contains("## Compatibility quarantine"));
        assert!(readme.contains(&format!("Step {removal_step} removes")));
        assert!(readme.contains("package"));
        assert!(!approved.contains(package));
        assert!(private.contains(&format!("\"{package}\"")));
        assert!(SHIMS.contains(&format!("| `{package}` |")));
    }
}

#[test]
fn quarantine_records_consumers_and_exact_removal_gates() {
    for required in [
        "id = \"RCRV1-DEV-010\"",
        "app_rt resolving radroots_net as radroots_net_core",
        "radroots_nostrdb runtime-adapter feature",
        "Step 215 sync retirement gate",
        "delete it at Step 313",
        "opt-in predecessor feature closures do not compile",
    ] {
        assert!(
            DEVIATIONS.contains(required),
            "transport quarantine is missing `{required}`"
        );
    }
    assert!(SHIMS.contains("| `radroots_nostr_runtime` |"));
    assert!(SHIMS.contains("| `radroots_net` |"));
}
