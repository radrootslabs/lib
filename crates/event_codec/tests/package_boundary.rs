use std::collections::BTreeSet;

#[allow(unused_imports)]
use radroots_event_codec::{canonical as _, decode as _, encode as _, verify as _};

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn manifest_has_final_identity_and_required_radroots_dependencies() {
    assert!(MANIFEST.contains("name = \"radroots_event_codec\""));
    assert!(MANIFEST.contains("version = \"0.1.0-alpha\""));
    assert!(MANIFEST.contains("publish = false"));
    assert!(MANIFEST.contains("[lib]\nname = \"radroots_event_codec\""));

    let dependencies = table_keys(MANIFEST, "[dependencies]");
    for dependency in ["radroots_blossom", "radroots_event", "radroots_protocol"] {
        assert!(
            dependencies.contains(dependency),
            "missing required Radroots dependency {dependency}"
        );
    }
    assert!(
        MANIFEST.contains("radroots_protocol = { workspace = true, default-features = false }")
    );
}

#[test]
fn crate_root_declares_every_approved_module() {
    let declared = root_declarations("pub mod ");
    for module in [
        "admission",
        "canonical",
        "decode",
        "encode",
        "manifest",
        "verify",
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
