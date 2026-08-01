use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn sqlite_storage_declares_the_final_secret_boundary() {
    assert_eq!(
        dependency_keys(MANIFEST),
        BTreeSet::from(["radroots_secrets"])
    );
    for forbidden in [
        "radroots_protected_store",
        "radroots_secret_vault",
        "radroots_nostr_accounts",
    ] {
        assert!(!MANIFEST.contains(forbidden));
        assert!(!ROOT.contains(forbidden));
    }
}

fn dependency_keys(manifest: &str) -> BTreeSet<&str> {
    manifest
        .split_once("[dependencies]")
        .map(|(_, dependencies)| dependencies)
        .unwrap_or_default()
        .lines()
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim()))
        .filter(|key| !key.is_empty())
        .collect()
}
