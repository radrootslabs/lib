use std::fs;
use std::time::Duration;

use radroots_storage::event::SourceGeneration;
use radroots_storage::status::WriterPolicy;
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
    assert!(matches!(
        Paths::from_files("/", directory.path().join("private.sqlite")),
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
    assert_eq!(read_only.writer_policy(), WriterPolicy::NoWriter);
    assert!(!read_only.mode().is_writable());
    assert!(!read_only.mode().may_create());

    let create = OpenOptions::new(paths, OpenMode::Create)
        .with_busy_timeout(Duration::from_secs(30))
        .expect("bounded timeout");
    assert_eq!(create.busy_timeout(), Duration::from_secs(30));
    assert!(create.foreign_keys_enabled());
    assert!(create.wal_enabled());
    assert_eq!(create.writer_policy(), WriterPolicy::AdvisoryProcessLock);
    assert!(create.mode().is_writable());
    assert!(create.mode().may_create());

    for invalid in [Duration::ZERO, Duration::from_secs(61)] {
        assert!(matches!(
            OpenOptions::new(create.paths().clone(), OpenMode::Create).with_busy_timeout(invalid),
            Err(Error::InvalidBusyTimeout { .. })
        ));
    }

    let generation = SourceGeneration::new([9; 32]).expect("source generation");
    let bootstrapped = OpenOptions::new(create.paths().clone(), OpenMode::Create)
        .with_source_generation(generation, 42)
        .expect("source generation bootstrap");
    assert_eq!(bootstrapped.source_generation(), Some(generation));
    assert_eq!(
        bootstrapped.source_generation_created_at_unix_ms(),
        Some(42)
    );
    assert!(matches!(
        OpenOptions::new(create.paths().clone(), OpenMode::Create)
            .with_source_generation(generation, 0),
        Err(Error::InvalidSourceGenerationTimestamp { actual: 0 })
    ));
    let beyond_sqlite_integer = u64::try_from(i64::MAX).expect("positive i64 maximum") + 1;
    assert!(matches!(
        OpenOptions::new(create.paths().clone(), OpenMode::Create)
            .with_source_generation(generation, beyond_sqlite_integer),
        Err(Error::InvalidSourceGenerationTimestamp { actual })
            if actual == beyond_sqlite_integer
    ));
}
