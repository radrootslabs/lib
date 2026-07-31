use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const SERVER: &str = include_str!("../src/server.rs");

#[test]
fn manifest_has_final_identity_feature_vocabulary_and_radroots_dependencies() {
    for required in [
        "name = \"radroots_nostr_connect\"",
        "version = \"0.1.0-alpha\"",
        "publish = false",
        "[lib]\nname = \"radroots_nostr_connect\"",
        "default = [\"serde\"]",
        "radroots_event = { workspace = true, default-features = false }",
        "radroots_identity = { workspace = true, default-features = false }",
        "radroots_nostr = { workspace = true, default-features = false }",
        "radroots_protocol = { workspace = true, default-features = false }",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing `{required}`"
        );
    }

    assert_eq!(
        table_keys(MANIFEST, "[features]"),
        BTreeSet::from(["default", "serde"])
    );
    assert_eq!(
        table_keys(MANIFEST, "[dependencies]")
            .into_iter()
            .filter(|dependency| dependency.starts_with("radroots_"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "radroots_event",
            "radroots_identity",
            "radroots_nostr",
            "radroots_protocol",
        ])
    );
}

#[test]
fn crate_root_contains_the_approved_module_skeleton() {
    for module in [
        "client",
        "error",
        "message",
        "method",
        "permission",
        "server",
        "uri",
    ] {
        assert!(
            ROOT.contains(&format!("pub mod {module};")),
            "crate root is missing `{module}`"
        );
    }

    assert!(SERVER.starts_with("//! NIP-46 server-side protocol state."));
    assert!(ROOT.contains("#[doc(hidden)]\npub mod prelude"));
    assert!(
        ROOT.contains("Step 143 removes this module"),
        "the temporary prelude must carry an exact removal checkpoint"
    );
}

fn table_keys<'a>(source: &'a str, header: &str) -> BTreeSet<&'a str> {
    let Some((_, tail)) = source.split_once(header) else {
        panic!("manifest is missing {header}");
    };

    tail.lines()
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('"') {
                return None;
            }
            line.split_once('=').map(|(key, _)| key.trim())
        })
        .collect()
}
