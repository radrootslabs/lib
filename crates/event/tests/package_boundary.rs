use std::collections::BTreeSet;

#[cfg(feature = "knowledge")]
#[allow(unused_imports)]
use radroots_event::knowledge as _;
#[allow(unused_imports)]
use radroots_event::{
    admission as _, calendar as _, contract as _, draft as _, envelope as _, farm as _, food as _,
    id as _, listing as _, media as _, post as _, profile as _, social as _, tag as _, trade as _,
    wire as _,
};

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn manifest_has_final_identity_and_required_radroots_dependencies() {
    assert!(MANIFEST.contains("name = \"radroots_event\""));
    assert!(MANIFEST.contains("version = \"0.1.0-alpha\""));
    assert!(MANIFEST.contains("publish = false"));
    assert!(MANIFEST.contains("[lib]\nname = \"radroots_event\""));
    assert!(MANIFEST.contains("default = [\"std\", \"serde\"]"));
    assert_eq!(
        table_keys(MANIFEST, "[dependencies]")
            .into_iter()
            .filter(|dependency| dependency.starts_with("radroots_"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "radroots_blossom",
            "radroots_core",
            "radroots_identity",
            "radroots_protocol",
        ])
    );
    assert!(
        MANIFEST.contains("radroots_protocol = { workspace = true, default-features = false }")
    );
}

#[test]
fn crate_root_declares_every_approved_module() {
    let declared = root_declarations("pub mod ");
    for module in [
        "admission",
        "calendar",
        "contract",
        "draft",
        "envelope",
        "farm",
        "food",
        "id",
        "knowledge",
        "listing",
        "media",
        "post",
        "profile",
        "social",
        "tag",
        "trade",
        "wire",
    ] {
        assert!(
            declared.contains(module),
            "missing approved module {module}"
        );
    }
}

fn table_keys<'a>(manifest: &'a str, heading: &str) -> BTreeSet<&'a str> {
    let Some((_, table)) = manifest.split_once(heading) else {
        return BTreeSet::new();
    };
    table
        .lines()
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .filter_map(|line| {
            let line = line.trim();
            (line
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'_')
                && !line.starts_with('#'))
            .then(|| line.split_once('=').map(|(key, _)| key.trim()))
            .flatten()
        })
        .collect()
}

fn root_declarations(prefix: &str) -> BTreeSet<&str> {
    ROOT.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix(prefix))
        .filter_map(|name| name.strip_suffix(';'))
        .collect()
}
