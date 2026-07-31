use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[allow(unused_imports)]
use radroots_nostr::{
    Error as _,
    event::{Coordinate as _, Event as _, EventId as _, Kind as _, Metadata as _, Timestamp as _},
    filter::Filter as _,
    key as _,
    tag::{Tag as _, TagKind as _, TagStandard as _},
};

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const EVENT_MODULE: &str = include_str!("../src/event.rs");
const FILTER_MODULE: &str = include_str!("../src/filter.rs");
const KEY_MODULE: &str = include_str!("../src/key.rs");
const TAG_MODULE: &str = include_str!("../src/tag.rs");
const TYPES_MODULE: &str = include_str!("../src/types.rs");
const IDENTITY_MANIFEST: &str = include_str!("../../identity/Cargo.toml");
const IDENTITY_KEY_MODULE: &str = include_str!("../../identity/src/key.rs");
const TRANSPORT_MANIFEST: &str = include_str!("../../transport_nostr/Cargo.toml");
const TRANSPORT_ROOT: &str = include_str!("../../transport_nostr/src/lib.rs");

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

#[test]
fn protocol_values_are_exposed_only_at_explicit_adapter_modules() {
    for (module, aliases) in [
        (
            EVENT_MODULE,
            [
                "pub type Coordinate",
                "pub type Event",
                "pub type EventId",
                "pub type Kind",
                "pub type Metadata",
                "pub type Timestamp",
            ]
            .as_slice(),
        ),
        (FILTER_MODULE, ["pub type Filter"].as_slice()),
        (
            TAG_MODULE,
            ["pub type Tag", "pub type TagKind", "pub type TagStandard"].as_slice(),
        ),
    ] {
        for alias in aliases {
            assert!(
                module.contains(alias),
                "explicit adapter module is missing `{alias}`"
            );
        }
    }

    for forbidden in [
        "pub type RadrootsNostrCoordinate",
        "pub type RadrootsNostrEvent",
        "pub type RadrootsNostrEventId",
        "pub type RadrootsNostrFilter",
        "pub type RadrootsNostrKind",
        "pub type RadrootsNostrMetadata",
        "pub type RadrootsNostrTag",
        "pub type RadrootsNostrTagKind",
        "pub type RadrootsNostrTagStandard",
        "pub type RadrootsNostrTimestamp",
    ] {
        assert!(
            !TYPES_MODULE.contains(forbidden),
            "broad predecessor alias remains public in types: `{forbidden}`"
        );
    }

    assert!(ROOT.contains("mod event_convert;"));
    assert!(ROOT.contains("mod tags;"));
    assert!(!ROOT.contains("pub mod event_convert;"));
    assert!(!ROOT.contains("pub mod tags;"));
}

#[test]
fn live_client_and_http_ownership_belongs_to_transport_nostr() {
    for forbidden in [
        "nostr-sdk",
        "reqwest",
        "client =",
        "http =",
        "radroots_nostr/client",
    ] {
        assert!(
            !MANIFEST.contains(forbidden),
            "portable manifest still owns forbidden live-client surface `{forbidden}`"
        );
    }

    for forbidden in ["pub mod client;", "pub mod relays;", "pub mod nip11;"] {
        assert!(
            !ROOT.contains(forbidden),
            "portable crate root still exposes live-client module `{forbidden}`"
        );
    }

    for required in ["nostr-sdk", "reqwest", "nip11 = [\"client\""] {
        assert!(
            TRANSPORT_MANIFEST.contains(required),
            "transport manifest is missing live-client ownership marker `{required}`"
        );
    }
    for required in [
        "mod client;",
        "mod relays;",
        "mod nip11;",
        "pub use client::{",
        "pub use nip11::fetch_nip11;",
    ] {
        assert!(
            TRANSPORT_ROOT.contains(required),
            "transport crate root is missing live-client owner `{required}`"
        );
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert!(!manifest_dir.join("src/client.rs").exists());
    assert!(!manifest_dir.join("src/relays.rs").exists());
    assert!(!manifest_dir.join("src/nip11.rs").exists());
    assert!(
        manifest_dir
            .join("../transport_nostr/src/client.rs")
            .exists()
    );
    assert!(
        manifest_dir
            .join("../transport_nostr/src/relays.rs")
            .exists()
    );
    assert!(
        manifest_dir
            .join("../transport_nostr/src/nip11.rs")
            .exists()
    );

    let forbidden_source = [
        "nostr_sdk",
        "reqwest::",
        "feature = \"client\"",
        "feature = \"http\"",
        "crate::client",
    ];
    for source_path in rust_sources(&manifest_dir.join("src")) {
        let source = fs::read_to_string(&source_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
        for forbidden in forbidden_source {
            assert!(
                !source.contains(forbidden),
                "{} still contains live-client ownership marker `{forbidden}`",
                source_path.display()
            );
        }
    }
}

#[test]
fn nostr_key_conversion_is_explicit_and_identity_remains_public_only() {
    for required in [
        "nostr/nip49",
        "pub fn public_key_to_nostr",
        "pub fn public_key_from_nostr",
        "pub fn public_key_to_npub",
        "pub fn public_key_from_npub",
        "pub fn parse_public_key",
        "pub fn parse_secret_key",
        "pub fn secret_key_to_nsec",
        "pub fn encrypt_secret_key_nip49",
        "pub fn encrypt_secret_key_nip49_with_options",
        "pub fn decrypt_secret_key_nip49",
    ] {
        let authority = if required == "nostr/nip49" {
            MANIFEST
        } else {
            KEY_MODULE
        };
        assert!(
            authority.contains(required),
            "Nostr key authority is missing `{required}`"
        );
    }

    for forbidden in ["nostr =", "nip49", "nsec", "ncryptsec", "SecretKey"] {
        assert!(
            !IDENTITY_MANIFEST.contains(forbidden),
            "identity manifest regained Nostr secret ownership `{forbidden}`"
        );
        assert!(
            !IDENTITY_KEY_MODULE.contains(forbidden),
            "identity key module regained Nostr secret ownership `{forbidden}`"
        );
    }

    for secret_function in [
        "parse_secret_key",
        "secret_key_to_nsec",
        "encrypt_secret_key_nip49",
        "encrypt_secret_key_nip49_with_options",
        "decrypt_secret_key_nip49",
    ] {
        let signature = format!("pub fn {secret_function}");
        let position = KEY_MODULE
            .find(&signature)
            .unwrap_or_else(|| panic!("missing secret adapter `{secret_function}`"));
        let prefix = &KEY_MODULE[..position];
        let nearby = &prefix[prefix.len().saturating_sub(160)..];
        assert!(
            nearby.contains("#[cfg(feature = \"signing\")]"),
            "secret adapter `{secret_function}` is not signing-gated"
        );
    }
}

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut sources = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        {
            let path = entry
                .expect("source directory entry must be readable")
                .path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(path);
            }
        }
    }
    sources
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
