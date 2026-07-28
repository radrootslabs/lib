use std::{collections::BTreeSet, fs, path::Path};

use radroots_blossom::{
    AuthorizationClaim, BlobDescriptor, BlobUrl, ByteVerifiedDescriptor, Error, MediaType, Sha256,
    authorization::{
        AuthoredUploadClaim, AuthorizationAction, AuthorizationContent, AuthorizationTarget,
        AuthorizationValidation, AuthorizationWireParts, ServerDomain, ServerScopeRequirement,
        ValidatedAuthorizationClaim,
    },
    descriptor::{ApprovedDescriptor, ByteCommitment},
    hash::{FileExtension, HashPath},
    url::ApprovedBlobUrl,
};

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn manifest_has_final_identity_features_and_no_radroots_dependencies() {
    assert!(MANIFEST.contains("name = \"radroots_blossom\""));
    assert!(MANIFEST.contains("version = \"0.1.0\""));
    assert!(MANIFEST.contains("publish = false"));
    assert!(MANIFEST.contains("[lib]\nname = \"radroots_blossom\""));
    assert!(MANIFEST.contains("default = [\"std\", \"serde\"]"));
    assert_eq!(
        table_keys(MANIFEST, "[features]"),
        BTreeSet::from(["default", "serde", "std"])
    );

    for heading in ["[dependencies]", "[dev-dependencies]"] {
        assert!(
            table_keys(MANIFEST, heading)
                .iter()
                .all(|dependency| !dependency.starts_with("radroots_")),
            "{heading} must not contain Radroots dependencies"
        );
    }
}

#[test]
fn crate_root_matches_the_approved_module_skeleton() {
    assert!(ROOT.contains("#![cfg_attr(not(feature = \"std\"), no_std)]"));
    assert_eq!(
        root_declarations("pub mod "),
        BTreeSet::from(["authorization", "descriptor", "hash", "media_type", "url"])
    );
    assert_eq!(root_declarations("mod "), BTreeSet::from(["error"]));
    assert_eq!(
        ROOT.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("pub use "))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "pub use authorization::AuthorizationClaim;",
            "pub use descriptor::BlobDescriptor;",
            "pub use descriptor::ByteVerifiedDescriptor;",
            "pub use error::Error;",
            "pub use hash::Sha256;",
            "pub use media_type::MediaType;",
            "pub use url::BlobUrl;",
        ])
    );
}

#[test]
fn final_root_and_module_paths_compile() {
    fn assert_public_value<T: Clone + core::fmt::Debug + Eq + Send + Sync>() {}

    assert_public_value::<ApprovedBlobUrl>();
    assert_public_value::<ApprovedDescriptor>();
    assert_public_value::<AuthoredUploadClaim>();
    assert_public_value::<AuthorizationAction>();
    assert_public_value::<AuthorizationClaim>();
    assert_public_value::<AuthorizationContent>();
    assert_public_value::<AuthorizationTarget>();
    assert_public_value::<AuthorizationValidation>();
    assert_public_value::<AuthorizationWireParts>();
    assert_public_value::<BlobDescriptor>();
    assert_public_value::<BlobUrl>();
    assert_public_value::<ByteCommitment>();
    assert_public_value::<ByteVerifiedDescriptor>();
    assert_public_value::<Error>();
    assert_public_value::<FileExtension>();
    assert_public_value::<HashPath>();
    assert_public_value::<MediaType>();
    assert_public_value::<ServerDomain>();
    assert_public_value::<ServerScopeRequirement>();
    assert_public_value::<Sha256>();
    assert_public_value::<ValidatedAuthorizationClaim>();
}

#[test]
fn production_types_do_not_repeat_the_crate_name() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in fs::read_dir(source_root).expect("read Blossom source directory") {
        let path = entry.expect("source directory entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read Blossom source");
        let production = source.split("\n#[cfg(test)]").next().unwrap_or(&source);
        assert!(
            !production.contains("RadrootsBlossom"),
            "Blossom production type repeats its crate name: {}",
            path.display()
        );
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
