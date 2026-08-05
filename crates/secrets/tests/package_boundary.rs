use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const README: &str = include_str!("../README.md");
const EXAMPLE: &str = include_str!("../examples/explicit_memory_provider.rs");

#[test]
fn manifest_has_final_identity_features_and_no_radroots_dependencies() {
    for required in [
        "name = \"radroots_secrets\"",
        "version = \"0.1.0-alpha\"",
        "publish = [\"crates-io\"]",
        "documentation = \"https://docs.rs/radroots_secrets\"",
        "[lib]\nname = \"radroots_secrets\"",
        "default = [\"std\", \"serde\"]",
        "memory = [\"std\"]",
        "file = [\"std\", \"dep:tempfile\"]",
        "keyring = [\"std\", \"dep:keyring\"]",
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
    assert_eq!(
        table_keys(MANIFEST, "[dependencies]"),
        BTreeSet::from([
            "chacha20poly1305",
            "keyring",
            "serde",
            "tempfile",
            "zeroize"
        ])
    );
    assert_eq!(
        table_keys(MANIFEST, "[dev-dependencies]"),
        BTreeSet::from(["futures-executor", "hex", "serde_json"])
    );
}

#[test]
fn production_sources_publish_only_the_approved_traits() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut paths = Vec::new();
    collect_rust_sources(&source_root, &mut paths);
    let mut public_traits = BTreeSet::new();

    for path in paths {
        let source = fs::read_to_string(&path).expect("read secrets source");
        let production = source.split("\n#[cfg(test)]").next().unwrap_or(&source);
        for line in production.lines() {
            if let Some(name) = line
                .trim_start()
                .strip_prefix("pub trait ")
                .and_then(|rest| rest.split([':', '<', ' ']).next())
            {
                public_traits.insert(name.to_owned());
            }
        }
    }

    assert_eq!(
        public_traits,
        ["KeyWrapping", "SecretProvider"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
}

#[test]
fn crate_root_contains_only_the_approved_module_skeleton() {
    assert!(ROOT.contains("#![cfg_attr(not(feature = \"std\"), no_std)]"));
    assert_eq!(
        declarations(ROOT, "pub mod "),
        BTreeSet::from([
            "context", "envelope", "error", "file", "id", "keyring", "memory", "provider",
            "wrapping",
        ])
    );
    assert_eq!(
        ROOT.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("pub use "))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "pub use envelope::EncryptedEnvelope;",
            "pub use error::Error;",
            "pub use id::{SecretId, SecretRef};",
            "pub use provider::SecretProvider;",
            "pub use wrapping::KeyWrapping;"
        ])
    );
}

#[test]
fn package_documentation_covers_the_security_and_host_contract() {
    for required in [
        "## Canonical surface",
        "## Explicit provider and envelope flow",
        "## Features and supported targets",
        "## Security and serialization contract",
        "## Side effects, cancellation, and commit points",
        "## Intended consumers",
        "public API baseline",
        "implicit retry or fallback",
    ] {
        assert!(README.contains(required), "README is missing `{required}`");
    }
    assert!(ROOT.contains("#![doc = include_str!(\"../README.md\")]"));
    assert!(MANIFEST.contains("name = \"explicit_memory_provider\""));
    assert!(MANIFEST.contains("required-features = [\"memory\"]"));
    for required in [
        "MemoryProvider::new()",
        "provider.provision(",
        "EncryptedEnvelope::seal",
        "EncryptedEnvelope::decode",
        "decoded.open(&provider)",
    ] {
        assert!(
            EXAMPLE.contains(required),
            "example is missing `{required}`"
        );
    }
}

fn table_keys<'a>(source: &'a str, table: &str) -> BTreeSet<&'a str> {
    let Some((_, body)) = source.split_once(table) else {
        return BTreeSet::new();
    };
    body.lines()
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim()))
        .filter(|key| {
            !key.is_empty()
                && key.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
                })
        })
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

fn collect_rust_sources(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("read source directory") {
        let path = entry.expect("source entry").path();
        if path.is_dir() {
            collect_rust_sources(&path, paths);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            paths.push(path);
        }
    }
}
