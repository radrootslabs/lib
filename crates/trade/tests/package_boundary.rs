use std::collections::BTreeSet;

#[allow(unused_imports)]
use radroots_trade::{evidence as _, model as _, reducer as _, validation as _, workflow as _};

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const PACKAGE_TIERS: &str = include_str!("../../../contracts/releases/package_tiers.toml");

#[test]
fn manifest_has_final_identity_and_required_radroots_dependencies() {
    assert!(MANIFEST.contains("name = \"radroots_trade\""));
    assert!(MANIFEST.contains("version = \"0.1.0-alpha\""));
    assert!(MANIFEST.contains("publish = false"));
    assert!(MANIFEST.contains("[lib]\nname = \"radroots_trade\""));

    let dependencies = table_keys(MANIFEST, "[dependencies]");
    for dependency in ["radroots_core", "radroots_event", "radroots_identity"] {
        assert!(
            dependencies.contains(dependency),
            "missing required Radroots dependency {dependency}"
        );
    }
}

#[test]
fn crate_root_declares_every_approved_module() {
    let declared = root_declarations("pub mod ");
    for module in ["evidence", "model", "reducer", "validation", "workflow"] {
        assert!(
            declared.contains(module),
            "missing approved module {module}"
        );
    }
}

#[test]
fn expired_upward_development_dependencies_are_absent() {
    let dev_dependencies = table_keys(MANIFEST, "[dev-dependencies]");
    for dependency in ["radroots_nostr", "radroots_transport"] {
        assert!(
            !dev_dependencies.contains(dependency),
            "expired development dependency remains: {dependency}"
        );
        assert!(
            !PACKAGE_TIERS.contains(&format!(
                "owner = \"radroots_trade\"\ndependency = \"{dependency}\""
            )),
            "expired tier exception remains: {dependency}"
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
