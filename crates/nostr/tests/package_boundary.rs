use std::collections::BTreeSet;

#[allow(unused_imports)]
use radroots_nostr::{Error as _, event as _, filter as _, key as _, tag as _};

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn manifest_has_final_identity_features_and_radroots_dependencies() {
    for required in [
        "name = \"radroots_nostr\"",
        "version = \"0.1.0-alpha\"",
        "publish = false",
        "[lib]\nname = \"radroots_nostr\"",
        "default = [\"std\", \"events\"]",
        "radroots_identity = { workspace = true, default-features = false }",
        "nostr = { workspace = true, default-features = false, features = [",
        "\"nostr/std\"",
        "\"radroots_event/std\"",
        "\"radroots_event_codec/std\"",
        "\"radroots_identity/std\"",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing `{required}`"
        );
    }

    let dependencies = table_keys(MANIFEST, "[dependencies]");
    for required in [
        "radroots_identity",
        "radroots_event",
        "radroots_event_codec",
    ] {
        let declaration = table_value(MANIFEST, "[dependencies]", required)
            .unwrap_or_else(|| panic!("missing required dependency `{required}`"));
        assert!(
            !declaration.contains("optional = true"),
            "`{required}` must be required"
        );
    }
    for optional in ["radroots_signing", "radroots_blossom"] {
        let declaration = table_value(MANIFEST, "[dependencies]", optional)
            .unwrap_or_else(|| panic!("missing optional dependency `{optional}`"));
        assert!(
            declaration.contains("optional = true"),
            "`{optional}` must be optional"
        );
    }
    assert_eq!(
        dependencies
            .into_iter()
            .filter(|dependency| dependency.starts_with("radroots_"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "radroots_blossom",
            "radroots_event",
            "radroots_event_codec",
            "radroots_identity",
            "radroots_signing",
        ])
    );
}

#[test]
fn crate_root_establishes_the_final_public_module_skeleton() {
    for module in [
        "blossom", "event", "filter", "key", "nip17", "signing", "tag",
    ] {
        assert!(
            ROOT.contains(&format!("pub mod {module};")),
            "crate root is missing final module `{module}`"
        );
    }
    assert!(ROOT.contains("pub use error::RadrootsNostrError as Error;"));
}

fn table_keys<'a>(source: &'a str, header: &str) -> BTreeSet<&'a str> {
    table_lines(source, header)
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim()))
        .collect()
}

fn table_value<'a>(source: &'a str, header: &str, key: &str) -> Option<&'a str> {
    table_lines(source, header).find_map(|line| {
        let (candidate, value) = line.split_once('=')?;
        (candidate.trim() == key).then_some(value.trim())
    })
}

fn table_lines<'a>(source: &'a str, header: &str) -> impl Iterator<Item = &'a str> {
    let start = source
        .find(header)
        .unwrap_or_else(|| panic!("missing table `{header}`"));
    source[start + header.len()..]
        .lines()
        .skip_while(|line| line.trim().is_empty())
        .take_while(|line| !line.trim_start().starts_with('['))
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
}
