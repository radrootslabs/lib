use std::collections::BTreeSet;

#[allow(unused_imports)]
use radroots_event_codec::{canonical as _, decode as _, encode as _, verify as _};

const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README.md");
const ROOT: &str = include_str!("../src/lib.rs");
const VERIFICATION: &str = include_str!("../src/verification/v1.rs");
const EXAMPLE: &str = include_str!("../examples/verify_profile.rs");
const FUZZ_LOCK: &str = include_str!("../../../fuzz/event_codec/Cargo.lock");
const FUZZ_MANIFEST: &str = include_str!("../../../fuzz/event_codec/Cargo.toml");
const PUBLIC_API: &str = include_str!("../../../docs/api/radroots_event_codec.txt");

#[test]
fn manifest_has_final_identity_and_required_radroots_dependencies() {
    assert!(MANIFEST.contains("name = \"radroots_event_codec\""));
    assert!(MANIFEST.contains("version = \"0.1.0-alpha\""));
    assert!(MANIFEST.contains("publish = [\"crates-io\"]"));
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
fn consolidated_parser_fuzz_package_uses_the_repository_version_contract() {
    let package = "name = \"radroots_parser_fuzz\"\npublish = false\nversion = \"0.1.0-alpha\"";
    let locked_package = "name = \"radroots_parser_fuzz\"\nversion = \"0.1.0-alpha\"";

    assert!(FUZZ_MANIFEST.contains(package));
    assert!(FUZZ_LOCK.contains(locked_package));
    assert!(!FUZZ_MANIFEST.contains("version = \"0.0.0\""));
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

#[test]
fn canonical_root_exports_are_explicit_and_host_types_do_not_leak() {
    for export in [
        "pub use codec::Codec;",
        "pub use decode::DecodeError;",
        "pub use encode::EncodeError;",
        "pub use verify::VerificationError;",
    ] {
        assert!(
            ROOT.contains(export),
            "missing canonical root export {export}"
        );
    }

    assert!(ROOT.contains("#![cfg_attr(not(feature = \"std\"), no_std)]"));
    assert!(!ROOT.contains("pub trait "));
    assert!(!ROOT.contains("TEMPORARY COMPATIBILITY QUARANTINE"));
    for forbidden in [
        "nostr::",
        "nostr_sdk::",
        "reqwest::",
        "sqlx::",
        "tokio::",
        "std::os::",
    ] {
        assert!(
            !ROOT.contains(forbidden),
            "crate root must not expose host path {forbidden}"
        );
    }
}

#[test]
fn compatibility_surface_is_removed() {
    assert!(MANIFEST.contains("publish = [\"crates-io\"]"));
    for module in [
        "comment",
        "deletion",
        "error",
        "job",
        "knowledge",
        "profile",
        "verification",
        "wire",
    ] {
        assert!(
            !ROOT.contains(&format!("pub mod {module};")),
            "compatibility module {module} remains public"
        );
    }

    for retired_root_export in [
        "pub use encode::tag_builders::RadrootsEventTagBuilder;",
        "pub use verify::{",
        "pub use manifest::registry_v7::{",
        "pub use manifest::{",
    ] {
        assert!(
            !ROOT.contains(retired_root_export),
            "retired prefixed root export remains: {retired_root_export}"
        );
    }
}

#[test]
fn codec_runtime_is_protocol_neutral_and_host_free() {
    let features = table_keys(MANIFEST, "[features]");
    let dependencies = table_keys(MANIFEST, "[dependencies]");

    assert!(!features.contains("nostr"));
    assert!(dependencies.contains("secp256k1"));
    for forbidden in [
        "keyring",
        "nostr",
        "nostr-sdk",
        "reqwest",
        "sqlx",
        "tokio",
        "wasm-bindgen",
        "web-sys",
    ] {
        assert!(
            !dependencies.contains(forbidden),
            "codec runtime must not depend on {forbidden}"
        );
    }
    assert!(!VERIFICATION.contains("nostr::"));
    assert!(!VERIFICATION.contains("feature = \"nostr\""));
}

#[test]
fn serialization_features_are_explicit_additive_and_final() {
    let features = table_keys(MANIFEST, "[features]");
    assert_eq!(
        features,
        BTreeSet::from(["default", "json", "knowledge", "manifests", "serde", "std"])
    );
    assert!(MANIFEST.contains("default = [\"std\", \"json\"]"));
    assert!(MANIFEST.contains("json = [\"serde\", \"dep:serde_json\"]"));
    assert!(MANIFEST.contains("knowledge = [\"json\", \"radroots_event/knowledge\"]"));
    assert!(MANIFEST.contains("manifests = [\"knowledge\", \"dep:hex\", \"dep:sha2\"]"));

    for forbidden in [
        "serde_json",
        "contract-manifest",
        "knowledge-nip54",
        "dto-bindgen",
        "codegen",
    ] {
        assert!(
            !features.contains(forbidden),
            "retired public feature {forbidden} must remain absent"
        );
    }
}

#[test]
fn package_documentation_and_reviewed_api_baseline_are_complete() {
    for section in [
        "## Canonical surface",
        "## Verification pipeline",
        "## Features",
        "## Serialization and canonicalization",
        "## Security and trust boundaries",
        "## Side effects, cancellation, and commit points",
        "## Intended consumers",
        "## Package charter",
    ] {
        assert!(README.contains(section), "README is missing {section}");
    }
    assert!(ROOT.contains("#![doc = include_str!(\"../README.md\")]"));
    assert!(EXAMPLE.contains("use radroots_event_codec::{admission, decode, verify};"));

    assert!(PUBLIC_API.starts_with("pub mod radroots_event_codec\n"));
    for item in [
        "pub mod radroots_event_codec::admission",
        "pub mod radroots_event_codec::canonical",
        "pub mod radroots_event_codec::decode",
        "pub mod radroots_event_codec::encode",
        "pub mod radroots_event_codec::manifest",
        "pub mod radroots_event_codec::verify",
        "pub use radroots_event_codec::VerificationError",
    ] {
        assert!(PUBLIC_API.contains(item), "API baseline is missing {item}");
    }
    for forbidden in ["nostr_sdk::", "reqwest::", "sqlx::", "tokio::", "keyring::"] {
        assert!(
            !PUBLIC_API.contains(forbidden),
            "API baseline exposes forbidden host path {forbidden}"
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
