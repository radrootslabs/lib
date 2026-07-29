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
fn manifest_has_phase1_extension_features_and_no_radroots_dependencies() {
    assert!(MANIFEST.contains("name = \"radroots_blossom\""));
    assert!(MANIFEST.contains("version = \"0.1.0-alpha\""));
    assert!(MANIFEST.contains("publish = false"));
    assert!(MANIFEST.contains("[lib]\nname = \"radroots_blossom\""));
    assert!(MANIFEST.contains("default = [\"std\", \"serde\"]"));
    assert_eq!(
        table_keys(MANIFEST, "[features]"),
        BTreeSet::from(["default", "raster-decode", "serde", "std"])
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
fn manifest_dependency_surface_is_protocol_plus_opt_in_raster() {
    assert_eq!(
        table_keys(MANIFEST, "[dependencies]"),
        BTreeSet::from([
            "image",
            "libwebp",
            "mediatype",
            "serde",
            "serde_json",
            "sha2",
            "unicode-general-category",
            "url_nostd",
            "zune-core",
            "zune-jpeg",
        ])
    );
    assert_eq!(
        table_keys(MANIFEST, "[dev-dependencies]"),
        BTreeSet::from(["hex", "image", "serde_json", "tempfile"])
    );
}

#[test]
fn crate_root_matches_the_approved_module_skeleton() {
    assert!(ROOT.contains("#![cfg_attr(not(feature = \"std\"), no_std)]"));
    assert_eq!(
        root_declarations("pub mod "),
        BTreeSet::from([
            "authorization",
            "descriptor",
            "hash",
            "media_type",
            "publication_readiness",
            "url",
        ])
    );
    assert_eq!(root_declarations("mod "), BTreeSet::from(["error"]));
    assert_eq!(
        ROOT.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("pub use "))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "pub use authorization::{",
            "pub use descriptor::{ApprovedDescriptor, BlobDescriptor, ByteCommitment, ByteVerifiedDescriptor};",
            "pub use error::Error;",
            "pub use hash::{FileExtension, HashPath, Sha256};",
            "pub use media_type::MediaType;",
            "pub use publication_readiness::verify_publication_readiness;",
            "pub use publication_readiness::{",
            "pub use url::{ApprovedBlobUrl, BlobUrl};",
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
    for (path, production) in production_sources() {
        assert!(
            !production.contains("RadrootsBlossom"),
            "Blossom production type repeats its crate name: {}",
            path.display()
        );
    }
}

#[test]
fn production_surface_exposes_no_public_traits() {
    for (path, production) in production_sources() {
        assert!(
            !production
                .lines()
                .map(str::trim_start)
                .any(|line| line.starts_with("pub trait ")),
            "Blossom must expose concrete protocol values, not public traits: {}",
            path.display()
        );
    }
}

#[test]
fn production_surface_owns_no_external_io_or_application_media_policy() {
    const FORBIDDEN_MARKERS: &[&str] = &[
        "std::fs",
        "std::net",
        "std::path",
        "std::process",
        "std::thread",
        "std::time",
        "tokio::",
        "reqwest::",
        "hyper::",
        "ureq::",
        "tower::",
        "HttpClient",
        "BlobClient",
        "UploadClient",
        "DownloadClient",
        "UploadQueue",
        "DownloadQueue",
        "CachePolicy",
        "FileCache",
        "RetryPolicy",
        "BackoffPolicy",
        "MediaPolicy",
        "Filesystem",
        "FileSystem",
    ];

    for (path, production) in production_sources() {
        for marker in FORBIDDEN_MARKERS {
            assert!(
                !production.contains(marker),
                "Blossom production source owns forbidden I/O or application policy marker `{marker}`: {}",
                path.display()
            );
        }
    }
}

fn production_sources() -> Vec<(std::path::PathBuf, String)> {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    fs::read_dir(source_root)
        .expect("read Blossom source directory")
        .map(|entry| entry.expect("source directory entry").path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("rs"))
        .map(|path| {
            let source = fs::read_to_string(&path).expect("read Blossom source");
            let production = source
                .split("\n#[cfg(test)]")
                .next()
                .unwrap_or(&source)
                .to_owned();
            (path, production)
        })
        .collect()
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
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, _) = line.split_once('=')?;
            let key = key.trim();
            key.chars()
                .all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
                })
                .then_some(key)
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
