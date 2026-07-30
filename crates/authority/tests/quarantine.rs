use std::{fs, path::PathBuf};

fn package_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn transition_package_remains_private_and_has_an_exact_removal_step() {
    let root = package_root();
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("read package manifest");
    let readme = fs::read_to_string(root.join("README.md")).expect("read transition record");
    let release_policy =
        fs::read_to_string(root.join("../../contracts/releases/publish_policy.toml"))
            .expect("read release policy");

    assert!(manifest.contains("publish = false"));
    assert!(manifest.contains("readme = \"README.md\""));
    assert!(readme.contains("`radroots_signing` for actor context"));
    assert!(readme.contains("removed at Step 313"));
    assert!(release_policy.contains("private = [\n  \"radroots_authority\""));
    assert!(
        !release_policy
            .split("approved_packages = [")
            .nth(1)
            .and_then(|tail| tail.split(']').next())
            .expect("approved package list")
            .contains("radroots_authority")
    );
}
