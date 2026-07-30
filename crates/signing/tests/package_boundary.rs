use std::{collections::BTreeSet, fs, path::Path};

#[allow(unused_imports)]
use radroots_signing::{
    Actor, Error, SignReceipt, SignRequest, Signer, SignerStatus, actor as _, capability as _,
    error as _, receipt as _, request as _, signer as _, status as _,
};

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn manifest_has_final_identity_features_and_dependencies() {
    for required in [
        "name = \"radroots_signing\"",
        "version = \"0.1.0-alpha\"",
        "publish = false",
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
        table_keys(MANIFEST, "[dependencies]"),
        BTreeSet::from([
            "radroots_event",
            "radroots_identity",
            "radroots_protocol",
            "serde",
        ])
    );
    assert_eq!(
        table_keys(MANIFEST, "[dev-dependencies]"),
        BTreeSet::from(["serde_json"])
    );
    for forbidden in [
        "async-trait",
        "keyring",
        "nostr",
        "nostr-sdk",
        "reqwest",
        "sqlx",
        "tokio",
    ] {
        assert!(
            !table_keys(MANIFEST, "[dependencies]").contains(forbidden),
            "signing runtime must not depend on {forbidden}"
        );
    }
}

#[test]
fn crate_root_declares_the_approved_module_skeleton() {
    assert!(ROOT.contains("#![cfg_attr(not(feature = \"std\"), no_std)]"));
    for module in [
        "actor",
        "capability",
        "error",
        "request",
        "receipt",
        "signer",
        "status",
    ] {
        let declaration = format!("pub mod {module};");
        assert!(
            ROOT.contains(&declaration),
            "crate root is missing {module}"
        );
    }
    assert_eq!(
        root_declarations("pub mod "),
        BTreeSet::from([
            "actor",
            "capability",
            "error",
            "receipt",
            "request",
            "signer",
            "status",
        ])
    );
    let _ = core::mem::size_of::<Actor>();
    let _ = core::mem::size_of::<SignRequest>();
    let _ = core::mem::size_of::<SignReceipt>();
    let _ = core::mem::size_of::<SignerStatus>();
    let _ = core::mem::size_of::<Error>();
    fn assert_object_safe(_: &dyn Signer) {}
    let _ = assert_object_safe;
    for root_export in [
        "pub use actor::Actor;",
        "pub use error::Error;",
        "pub use receipt::SignReceipt;",
        "pub use request::SignRequest;",
        "pub use signer::Signer;",
        "pub use status::SignerStatus;",
    ] {
        assert!(ROOT.contains(root_export), "missing {root_export}");
    }
    assert_eq!(
        ROOT.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("pub use "))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "pub use actor::Actor;",
            "pub use error::Error;",
            "pub use receipt::SignReceipt;",
            "pub use request::SignRequest;",
            "pub use signer::Signer;",
            "pub use status::SignerStatus;",
        ])
    );
    assert!(!ROOT.contains("prelude"));
}

#[test]
fn production_sources_publish_only_the_approved_traits_and_no_host_stack() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    collect_rust_sources(&source_root, &mut sources);
    assert!(!sources.is_empty());
    let mut public_traits = BTreeSet::new();

    for path in sources {
        let source = fs::read_to_string(&path).expect("read signing source");
        let production = source.split("\n#[cfg(test)]").next().unwrap_or(&source);
        for line in production.lines() {
            let trimmed = line.trim_start();
            if let Some(name) = trimmed
                .strip_prefix("pub trait ")
                .and_then(|rest| rest.split([':', '<', ' ']).next())
            {
                public_traits.insert(name.to_owned());
            }
            for forbidden in [
                "nostr::",
                "nostr_sdk::",
                "reqwest::",
                "sqlx::",
                "tokio::",
                "keyring::",
                "std::fs",
                "std::path",
                "SecretKey",
                "PrivateKey",
            ] {
                assert!(
                    !line.contains(forbidden),
                    "signing production source must not contain {forbidden}: {}: {trimmed}",
                    path.display()
                );
            }
        }
    }

    assert_eq!(
        public_traits,
        ["ProgressObserver", "Signer"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
}

fn table_keys<'a>(manifest: &'a str, heading: &str) -> BTreeSet<&'a str> {
    let table = manifest
        .split_once(heading)
        .unwrap_or_else(|| panic!("missing manifest table {heading}"))
        .1;
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

fn collect_rust_sources(directory: &Path, paths: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(directory).expect("read signing source directory") {
        let path = entry.expect("source directory entry").path();
        if path.is_dir() {
            collect_rust_sources(&path, paths);
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            paths.push(path);
        }
    }
}
