use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn manifest_has_final_identity_features_and_no_radroots_dependencies() {
    for required in [
        "name = \"radroots_secrets\"",
        "version = \"0.1.0-alpha\"",
        "publish = false",
        "documentation = \"https://docs.rs/radroots_secrets\"",
        "[lib]\nname = \"radroots_secrets\"",
        "default = [\"std\", \"serde\"]",
        "memory = [\"std\"]",
        "file = [\"std\"]",
        "keyring = [\"std\"]",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing `{required}`"
        );
    }

    assert_eq!(
        table_keys(MANIFEST, "[features]"),
        BTreeSet::from(["default", "file", "keyring", "memory", "serde", "std"])
    );
    assert!(
        table_keys(MANIFEST, "[dependencies]")
            .into_iter()
            .all(|dependency| !dependency.starts_with("radroots_")),
        "security SPI must not depend on another Radroots package"
    );
}

#[test]
fn crate_root_contains_only_the_approved_module_skeleton() {
    assert!(ROOT.contains("#![cfg_attr(not(feature = \"std\"), no_std)]"));
    assert_eq!(
        declarations(ROOT, "pub mod "),
        BTreeSet::from([
            "envelope", "error", "file", "id", "keyring", "memory", "provider", "wrapping",
        ])
    );
    assert_eq!(
        ROOT.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("pub use "))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "pub use error::Error;",
            "pub use id::{SecretId, SecretRef};",
            "pub use provider::SecretProvider;",
            "pub use wrapping::KeyWrapping;"
        ])
    );
}

fn table_keys<'a>(source: &'a str, table: &str) -> BTreeSet<&'a str> {
    let Some((_, body)) = source.split_once(table) else {
        return BTreeSet::new();
    };
    body.lines()
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim()))
        .filter(|key| !key.is_empty())
        .collect()
}

fn declarations<'a>(source: &'a str, prefix: &str) -> BTreeSet<&'a str> {
    source
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix(prefix))
        .filter_map(|line| line.strip_suffix(';'))
        .collect()
}
