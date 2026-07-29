use std::{collections::BTreeSet, fs, path::Path};

use radroots_identity::{
    AccountId, Error, IdentityId, Profile, PublicIdentity, PublicKey, Username,
    account::{Record, Status},
};

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn manifest_has_exact_features_and_dependencies() {
    assert_eq!(
        table_keys(MANIFEST, "[features]"),
        BTreeSet::from(["default", "serde", "std"])
    );
    assert_eq!(
        table_keys(MANIFEST, "[dependencies]"),
        BTreeSet::from(["k256", "serde", "thiserror"])
    );
    assert_eq!(
        table_keys(MANIFEST, "[dev-dependencies]"),
        BTreeSet::from(["serde_json"])
    );
    assert!(MANIFEST.contains("default = [\"std\", \"serde\"]"));
    assert!(MANIFEST.contains("std = [\"thiserror/std\"]"));
    assert!(MANIFEST.contains("serde = [\"dep:serde\"]"));
    assert!(MANIFEST.contains(
        "k256 = { version = \"0.13\", default-features = false, features = [\"arithmetic\"] }"
    ));
    assert!(MANIFEST.contains("serde = { workspace = true, optional = true }"));
    assert!(MANIFEST.contains("thiserror = { version = \"2\", default-features = false }"));

    for forbidden in [
        "json-file",
        "nostr",
        "radroots_protected_store",
        "radroots_runtime",
        "radroots_runtime_paths",
        "radroots_secret_vault",
        "tracing",
        "tempfile",
    ] {
        assert!(
            !MANIFEST.contains(forbidden),
            "identity manifest must not contain host persistence edge {forbidden}"
        );
    }
}

#[test]
fn crate_root_matches_the_curated_module_and_export_contract() {
    assert!(ROOT.contains("#![cfg_attr(not(feature = \"std\"), no_std)]"));
    assert_eq!(
        root_declarations("pub mod "),
        BTreeSet::from(["account", "key", "profile", "username"])
    );
    assert_eq!(root_declarations("mod "), BTreeSet::from(["error"]));
    assert_eq!(
        ROOT.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("pub use "))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "pub use account::AccountId;",
            "pub use error::Error;",
            "pub use key::{IdentityId, PublicKey};",
            "pub use profile::{Profile, PublicIdentity};",
            "pub use username::Username;",
        ])
    );
}

#[test]
fn intended_public_paths_and_traits_compile() {
    fn assert_public_value<T: Clone + core::fmt::Debug + Eq + Send + Sync>() {}

    assert_public_value::<AccountId>();
    assert_public_value::<IdentityId>();
    assert_public_value::<Profile>();
    assert_public_value::<PublicIdentity>();
    assert_public_value::<PublicKey>();
    assert_public_value::<Record>();
    assert_public_value::<Status>();
    assert_public_value::<Username>();
    let _ = core::mem::size_of::<Error>();
}

#[test]
fn production_sources_have_no_forbidden_owners_or_public_traits() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    collect_rust_sources(&source_root, &mut sources);
    assert!(!sources.is_empty());

    for path in sources {
        let source = fs::read_to_string(&path).expect("read identity source");
        let production = source.split("\n#[cfg(test)]").next().unwrap_or(&source);
        for line in production.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            assert!(
                !trimmed.starts_with("pub trait ") && !trimmed.starts_with("pub unsafe trait "),
                "identity must not publish traits: {}: {trimmed}",
                path.display()
            );
            for forbidden in [
                "RadrootsIdentity",
                "RadrootsSecret",
                "SecretKey",
                "PrivateKey",
                "secret_key",
                "Nsec",
                "nsec::",
                "nip49",
                "Nip49",
                "nostr::",
                "std::fs",
                "std::path",
                "PathBuf",
                "Sqlite",
                "keyring",
            ] {
                assert!(
                    !line.contains(forbidden),
                    "identity production source must not contain {forbidden}: {}: {trimmed}",
                    path.display()
                );
            }
        }
    }
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
            (!line.is_empty() && !line.starts_with('#'))
                .then(|| line.split_once('=').map(|(key, _)| key.trim()))
                .flatten()
        })
        .collect()
}

fn root_declarations(prefix: &str) -> BTreeSet<&str> {
    ROOT.lines()
        .map(str::trim)
        .filter_map(|line| {
            line.strip_prefix(prefix)
                .and_then(|name| name.strip_suffix(';'))
        })
        .collect()
}

fn collect_rust_sources(directory: &Path, paths: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(directory).expect("read identity source directory") {
        let path = entry.expect("source directory entry").path();
        if path.is_dir() {
            collect_rust_sources(&path, paths);
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            paths.push(path);
        }
    }
}
