use std::collections::BTreeSet;

#[allow(unused_imports)]
use radroots_transport::{
    capability as _, endpoint as _, error as _, outcome as _, policy as _, sink as _, source as _,
    target as _,
};

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn manifest_has_final_identity_features_and_required_radroots_dependencies() {
    for required in [
        "name = \"radroots_transport\"",
        "version = \"0.1.0-alpha\"",
        "publish = false",
        "[lib]\nname = \"radroots_transport\"",
        "default = [\"std\", \"serde\"]",
        "radroots_event = { workspace = true, default-features = false }",
        "radroots_identity = { workspace = true, default-features = false }",
        "radroots_protocol = { workspace = true, default-features = false }",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing {required}"
        );
    }

    assert_eq!(
        table_keys(MANIFEST, "[features]"),
        BTreeSet::from(["default", "serde", "std"])
    );
    assert_eq!(
        table_keys(MANIFEST, "[dependencies]")
            .into_iter()
            .filter(|dependency| dependency.starts_with("radroots_"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["radroots_event", "radroots_identity", "radroots_protocol"])
    );
}

#[test]
fn crate_root_declares_the_approved_public_module_skeleton() {
    assert!(ROOT.contains("#![cfg_attr(not(feature = \"std\"), no_std)]"));
    assert_eq!(
        root_declarations("pub mod "),
        BTreeSet::from([
            "capability",
            "endpoint",
            "error",
            "outcome",
            "policy",
            "sink",
            "source",
            "target",
        ])
    );
}

fn table_keys<'a>(source: &'a str, heading: &str) -> BTreeSet<&'a str> {
    let mut in_table = false;
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line == heading {
                in_table = true;
                return None;
            }
            if in_table && line.starts_with('[') {
                in_table = false;
            }
            in_table
                .then(|| line.split_once('=').map(|(key, _)| key.trim()))
                .flatten()
        })
        .collect()
}

fn root_declarations(prefix: &str) -> BTreeSet<&str> {
    ROOT.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix(prefix))
        .filter_map(|line| line.strip_suffix(';'))
        .collect()
}
