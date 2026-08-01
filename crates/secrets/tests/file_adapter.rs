#![cfg(feature = "file")]

use futures_executor::block_on;
use radroots_secrets::envelope::Nonce;
use radroots_secrets::file::{FileOpenMode, FileProvider};
use radroots_secrets::id::{BackendKind, KeyVersion};
use radroots_secrets::wrapping::{SecretMaterial, UnwrapRequest, WrapRequest};
use radroots_secrets::{Error, KeyWrapping, SecretId, SecretProvider, SecretRef};
use std::fs;

fn reference(id: &str, version: u32) -> SecretRef {
    SecretRef::new(
        SecretId::parse(id).expect("valid id"),
        BackendKind::File,
        KeyVersion::new(version).expect("valid version"),
    )
}

fn provider(root: &std::path::Path, mode: FileOpenMode) -> FileProvider {
    FileProvider::open(
        root,
        mode,
        SecretMaterial::from_slice(&[0xA5; 32]).expect("master key"),
    )
    .expect("open provider")
}

#[test]
fn file_provider_round_trips_encrypted_material_with_atomic_creation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("vault");
    let provider = provider(&root, FileOpenMode::CreateNew);
    let reference = reference("file-key", 1);
    let material = SecretMaterial::from_slice(&[0x41; 32]).expect("material");
    provider
        .provision(&reference, &material, Nonce::new([0x11; 24]))
        .expect("provision");

    let persisted = fs::read(root.join("66696c652d6b6579.v1.rrk")).expect("read entry");
    assert!(!persisted.windows(32).any(|window| window == [0x41; 32]));
    assert_eq!(fs::read_dir(&root).expect("read root").count(), 1);

    let wrapped = block_on(provider.wrap(WrapRequest::new(&reference, &material))).expect("wrap");
    let opened =
        block_on(provider.unwrap(UnwrapRequest::new(&reference, &wrapped))).expect("unwrap");
    opened.expose_secret(|bytes| assert_eq!(bytes, &[0x41; 32]));
    assert_eq!(provider.backend_kind(), BackendKind::File);
    assert_eq!(format!("{provider:?}"), "FileProvider(<redacted>)");
}

#[test]
fn traversal_existing_replacement_and_truncated_files_fail_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let traversal = temp.path().join("nested/../vault");
    assert!(matches!(
        FileProvider::open(
            &traversal,
            FileOpenMode::CreateNew,
            SecretMaterial::from_slice(&[0xA5; 32]).expect("master key"),
        ),
        Err(Error::UnsafePath)
    ));

    let root = temp.path().join("safe-vault");
    let provider = provider(&root, FileOpenMode::CreateNew);
    let reference = reference("file-key", 1);
    let material = SecretMaterial::from_slice(&[0x41; 32]).expect("material");
    provider
        .provision(&reference, &material, Nonce::new([0x11; 24]))
        .expect("provision");
    assert!(matches!(
        provider.provision(&reference, &material, Nonce::new([0x12; 24])),
        Err(Error::SecretAlreadyExists { .. })
    ));

    fs::write(root.join("66696c652d6b6579.v1.rrk"), b"truncated").expect("truncate");
    assert!(matches!(
        block_on(provider.wrap(WrapRequest::new(&reference, &material))),
        Err(Error::EnvelopeMalformed)
    ));
}

#[test]
fn rotation_resumes_after_new_version_commit_and_removes_old_version() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("vault");
    let provider = provider(&root, FileOpenMode::CreateNew);
    let current = reference("rotating-key", 1);
    let next = reference("rotating-key", 2);
    let current_material = SecretMaterial::from_slice(&[0x11; 32]).expect("material");
    let next_material = SecretMaterial::from_slice(&[0x22; 32]).expect("material");
    provider
        .provision(&current, &current_material, Nonce::new([0x31; 24]))
        .expect("current");

    // Simulate interruption after the new revision commits but before old cleanup.
    provider
        .provision(&next, &next_material, Nonce::new([0x32; 24]))
        .expect("next commit");
    provider
        .rotate(&current, &next, &next_material, Nonce::new([0x32; 24]))
        .expect("resume rotation");
    assert!(!provider.contains(&current).expect("old contains"));
    assert!(provider.contains(&next).expect("new contains"));

    let wrapped = block_on(provider.wrap(WrapRequest::new(&next, &next_material))).expect("wrap");
    assert!(block_on(provider.unwrap(UnwrapRequest::new(&next, &wrapped))).is_ok());
}

#[cfg(unix)]
#[test]
fn symlink_entries_and_insecure_permissions_are_rejected() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let temp = tempfile::tempdir().expect("tempdir");
    let insecure = temp.path().join("insecure");
    fs::create_dir(&insecure).expect("create insecure root");
    fs::set_permissions(&insecure, fs::Permissions::from_mode(0o755)).expect("chmod");
    assert!(matches!(
        FileProvider::open(
            &insecure,
            FileOpenMode::OpenExisting,
            SecretMaterial::from_slice(&[0xA5; 32]).expect("master key"),
        ),
        Err(Error::InsecurePermissions)
    ));

    let root = temp.path().join("vault");
    let provider = provider(&root, FileOpenMode::CreateNew);
    let outside = temp.path().join("outside");
    fs::write(&outside, b"not a secret entry").expect("outside file");
    symlink(&outside, root.join("6c696e6b65642d6b6579.v1.rrk")).expect("symlink");
    let linked = reference("linked-key", 1);
    assert!(matches!(provider.contains(&linked), Err(Error::UnsafePath)));

    let linked_root = temp.path().join("linked-root");
    symlink(&root, &linked_root).expect("root symlink");
    assert!(matches!(
        FileProvider::open(
            &linked_root,
            FileOpenMode::OpenExisting,
            SecretMaterial::from_slice(&[0xA5; 32]).expect("master key"),
        ),
        Err(Error::UnsafePath)
    ));
}
