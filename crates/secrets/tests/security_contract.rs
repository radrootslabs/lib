use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_secrets::envelope::{Nonce, SealMaterial, SealRequest};
use radroots_secrets::error::{Operation, SecretIdError};
use radroots_secrets::id::{BackendKind, KeyVersion};
use radroots_secrets::wrapping::{SecretMaterial, WrappedSecret};
use radroots_secrets::{Error, SecretId, SecretRef};
use std::fs;
use std::path::{Path, PathBuf};

const PUBLIC_API: &str = include_str!("../../../contracts/api_baselines/radroots_secrets.txt");

#[test]
fn reviewed_api_forbids_secret_bearing_clone_serialize_and_byte_access() {
    for required in [
        "pub mod radroots_secrets::context",
        "pub mod radroots_secrets::envelope",
        "pub mod radroots_secrets::error",
        "pub mod radroots_secrets::id",
        "pub mod radroots_secrets::provider",
        "pub mod radroots_secrets::wrapping",
        "pub struct radroots_secrets::wrapping::SecretMaterial(_)",
        "pub struct radroots_secrets::id::SecretRef",
        "pub trait radroots_secrets::provider::SecretProvider",
        "pub trait radroots_secrets::wrapping::KeyWrapping",
    ] {
        assert!(
            PUBLIC_API.contains(required),
            "reviewed public API is missing `{required}`"
        );
    }

    for secret_bearing_type in [
        "radroots_secrets::wrapping::SecretMaterial",
        "radroots_secrets::id::SecretRef",
        "radroots_secrets::envelope::SealMaterial",
        "radroots_secrets::envelope::SealRequest",
        "radroots_secrets::envelope::LegacyV1ResealAuthority",
        "radroots_secrets::wrapping::LegacyV1UnwrapRequest",
    ] {
        for forbidden_trait in ["core::clone::Clone", "serde_core::ser::Serialize"] {
            let forbidden = format!("impl {forbidden_trait} for {secret_bearing_type}");
            assert!(
                !PUBLIC_API.contains(&forbidden),
                "secret-bearing public type exposes forbidden trait: {forbidden}"
            );
        }
    }

    for forbidden in [
        "SecretMaterial::as_bytes",
        "SecretMaterial::as_slice",
        "SecretMaterial::into_bytes",
        "SecretMaterial::to_vec",
        "SecretRef::clone",
        "EncryptedEnvelope::open(&self, &dyn radroots_secrets::wrapping::KeyWrapping) ->",
        "SealRequest<'a>::new(radroots_secrets::id::SecretRef, &'a radroots_secrets::wrapping::SecretMaterial",
    ] {
        assert!(
            !PUBLIC_API.contains(forbidden),
            "reviewed API exposes forbidden plaintext or duplication surface `{forbidden}`"
        );
    }

    for forbidden_dependency in [
        "chacha20poly1305",
        "futures_executor",
        "keyring",
        "serde_json",
        "tempfile",
        "zeroize",
    ] {
        assert!(
            !exposes_crate_path(PUBLIC_API, forbidden_dependency),
            "reviewed API leaks implementation dependency `{forbidden_dependency}`"
        );
    }
}

fn exposes_crate_path(public_api: &str, crate_name: &str) -> bool {
    public_api
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || matches!(character, '_' | ':'))
        })
        .any(|token| {
            token
                .strip_prefix(crate_name)
                .is_some_and(|remainder| remainder.starts_with("::"))
        })
}

#[test]
fn diagnostics_snapshot_is_redacted_and_plaintext_free() {
    const SECRET_ID_SENTINEL: &str = "plaintext-secret-id-sentinel";
    const PLAINTEXT_SENTINEL: &[u8] = b"plaintext-material-sentinel";

    let id = SecretId::parse(SECRET_ID_SENTINEL).expect("valid secret id");
    let reference = SecretRef::new(
        id,
        BackendKind::External,
        KeyVersion::new(7).expect("valid key version"),
    );
    let plaintext = SecretMaterial::from_slice(PLAINTEXT_SENTINEL).expect("valid material");
    let wrapped = WrappedSecret::from_bytes(b"wrapped-material-sentinel".to_vec())
        .expect("valid wrapped material");
    let sealing_key = SecretMaterial::from_slice(&[0x42; 32]).expect("valid sealing key");
    let context = EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.security_test").expect("purpose"),
        EnvelopeSubject::parse("security_test", "plaintext-secret-id-sentinel").expect("subject"),
        PayloadSchemaId::parse("radroots.security_test.v1").expect("schema"),
    );
    let request = SealRequest::new(
        reference,
        context,
        &plaintext,
        SealMaterial::new(sealing_key, Nonce::new([0x24; 24])),
    );

    let diagnostics = [
        format!("{plaintext:?}"),
        format!("{wrapped:?}"),
        format!("{request:?}"),
        format!("{:?}", Error::DecryptFailed),
        Error::BackendFailure {
            backend: BackendKind::External,
            operation: Operation::Unwrap,
        }
        .to_string(),
        Error::SecretNotFound {
            backend: BackendKind::External,
            key_version: 7,
        }
        .to_string(),
        Error::InvalidSecretId(SecretIdError::InvalidCharacter { byte_offset: 9 }).to_string(),
    ];

    assert_eq!(diagnostics[0], "SecretMaterial(<redacted>)");
    assert_eq!(diagnostics[1], "WrappedSecret(<redacted>)");
    assert_eq!(diagnostics[2], "SealRequest(<redacted>)");
    for diagnostic in diagnostics {
        assert!(!diagnostic.contains(SECRET_ID_SENTINEL));
        assert!(!diagnostic.contains("plaintext-material-sentinel"));
        assert!(!diagnostic.contains("wrapped-material-sentinel"));
    }
}

#[test]
fn envelope_and_private_artifact_sources_have_no_plaintext_logging_surface() {
    let crates_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("secrets crate has a crates directory parent");
    let mut paths = Vec::new();
    for crate_name in ["secrets", "storage", "storage_sqlite"] {
        collect_rust_sources(&crates_root.join(crate_name).join("src"), &mut paths);
    }
    assert!(!paths.is_empty(), "audited production sources must exist");

    for path in paths {
        let source = fs::read_to_string(&path).expect("read audited source");
        let production = source.split("\n#[cfg(test)]").next().unwrap_or(&source);
        for forbidden in ["tracing::", "log::", "println!(", "eprintln!(", "dbg!("] {
            assert!(
                !production.contains(forbidden),
                "envelope or private-artifact source contains logging surface `{forbidden}`: {}",
                path.display()
            );
        }
    }
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
