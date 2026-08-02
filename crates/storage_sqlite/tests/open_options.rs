use std::fs;
use std::time::Duration;

use radroots_storage_sqlite::{Error, OpenMode, OpenOptions, Paths};

#[test]
fn paths_derive_only_runtime_and_private_database_ownership() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let paths = Paths::from_directory(directory.path()).expect("owned paths");

    assert_eq!(paths.runtime(), directory.path().join("runtime.sqlite"));
    assert_eq!(paths.private(), directory.path().join("private.sqlite"));
    assert!(!format!("{paths:?}").contains("studio.sqlite"));
}

#[test]
fn paths_reject_relative_traversal_and_wrong_owned_names() {
    assert!(matches!(
        Paths::from_directory("relative"),
        Err(Error::InvalidPath(_))
    ));

    let directory = tempfile::tempdir().expect("temporary directory");
    assert!(matches!(
        Paths::from_files(
            directory.path().join("../runtime.sqlite"),
            directory.path().join("private.sqlite"),
        ),
        Err(Error::InvalidPath(_))
    ));
    assert!(matches!(
        Paths::from_files(
            directory.path().join("studio.sqlite"),
            directory.path().join("private.sqlite"),
        ),
        Err(Error::UnexpectedFileName { .. })
    ));
}

#[test]
fn create_mode_accepts_missing_files_but_existing_modes_do_not() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let paths = Paths::from_directory(directory.path()).expect("owned paths");

    OpenOptions::new(paths.clone(), OpenMode::Create)
        .validate_filesystem()
        .expect("create plan");
    assert!(matches!(
        OpenOptions::new(paths.clone(), OpenMode::ReadOnly).validate_filesystem(),
        Err(Error::MissingFile(_))
    ));
    assert!(matches!(
        OpenOptions::new(paths, OpenMode::ReadWriteExisting).validate_filesystem(),
        Err(Error::MissingFile(_))
    ));
}

#[test]
fn existing_modes_accept_only_regular_owned_files() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let paths = Paths::from_directory(directory.path()).expect("owned paths");
    fs::write(paths.runtime(), []).expect("runtime file");
    fs::write(paths.private(), []).expect("private file");

    for mode in [OpenMode::ReadOnly, OpenMode::ReadWriteExisting] {
        OpenOptions::new(paths.clone(), mode)
            .validate_filesystem()
            .expect("existing plan");
    }
}

#[test]
fn options_fix_connection_invariants_and_bound_busy_timeout() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let paths = Paths::from_directory(directory.path()).expect("owned paths");
    let read_only = OpenOptions::new(paths.clone(), OpenMode::ReadOnly);
    assert_eq!(read_only.busy_timeout(), Duration::from_secs(5));
    assert!(read_only.foreign_keys_enabled());
    assert!(!read_only.wal_enabled());
    assert!(!read_only.mode().is_writable());
    assert!(!read_only.mode().may_create());

    let create = OpenOptions::new(paths, OpenMode::Create)
        .with_busy_timeout(Duration::from_secs(30))
        .expect("bounded timeout");
    assert_eq!(create.busy_timeout(), Duration::from_secs(30));
    assert!(create.foreign_keys_enabled());
    assert!(create.wal_enabled());
    assert!(create.mode().is_writable());
    assert!(create.mode().may_create());

    for invalid in [Duration::ZERO, Duration::from_secs(61)] {
        assert!(matches!(
            OpenOptions::new(create.paths().clone(), OpenMode::Create).with_busy_timeout(invalid),
            Err(Error::InvalidBusyTimeout { .. })
        ));
    }
}
