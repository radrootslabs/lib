use std::fs;
use std::path::Path;

const DEVIATIONS: &str = include_str!("../../../docs/implementation/deviations.toml");
const SHIMS: &str = include_str!("../../../docs/implementation/COMPATIBILITY_SHIMS.md");
const PUBLISH_POLICY: &str = include_str!("../../../contracts/releases/publish_policy.toml");

#[test]
fn superseded_transport_packages_are_removed_from_release_authority() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let workspace_manifest =
        fs::read_to_string(workspace.join("Cargo.toml")).expect("workspace manifest");
    let approved = PUBLISH_POLICY
        .split_once("[workspace_classification]")
        .map(|(publication, _)| publication)
        .expect("workspace classification");
    let private = PUBLISH_POLICY
        .split_once("private = [")
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(entries, _)| entries)
        .expect("private package classification");

    for package in ["radroots_net", "radroots_nostr_runtime"] {
        assert!(
            !workspace
                .join(format!("crates/{package}/Cargo.toml"))
                .exists()
        );
        assert!(!workspace_manifest.contains(package));
        assert!(!approved.contains(package));
        assert!(!private.contains(&format!("\"{package}\"")));
        assert!(!SHIMS.contains(&format!("| `{package}` |")));
    }
}

#[test]
fn closure_record_captures_the_pulled_forward_removal_gates() {
    for required in [
        "id = \"RCRV1-DEV-010\"",
        "status = \"closed\"",
        "radroots_nostr_runtime Step 215 deletion and radroots_net Step 313 deletion had not been applied",
        "Delete radroots_nostr_runtime, its NostrDB runtime adapter, and radroots_net during Step 301 qualification",
        "historical names remain only in governed specifications, migration evidence, and fail-closed regression assertions",
        "closure_evidence = [",
    ] {
        assert!(
            DEVIATIONS.contains(required),
            "transport closure record is missing `{required}`"
        );
    }
}
