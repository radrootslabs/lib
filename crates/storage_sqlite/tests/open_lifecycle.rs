use std::time::Duration;

use radroots_storage::{EventStore, event::SourceGeneration, status::EventStoreMode};
use radroots_storage_sqlite::{Error, OpenMode, OpenOptions, Paths, SqliteStorage};

fn generation(byte: u8) -> SourceGeneration {
    SourceGeneration::new([byte; 32]).expect("source generation")
}

#[tokio::test]
async fn public_open_creates_both_databases_and_reuses_the_durable_generation() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let paths = Paths::from_directory(directory.path()).expect("owned paths");
    let expected = generation(31);
    let store = SqliteStorage::open(
        OpenOptions::new(paths.clone(), OpenMode::Create)
            .with_busy_timeout(Duration::from_millis(250))
            .expect("busy timeout")
            .with_source_generation(expected, 1_000)
            .expect("source generation"),
    )
    .await
    .expect("create storage");
    assert!(paths.runtime().is_file());
    assert!(paths.private().is_file());
    let status = EventStore::status(&store).await.expect("event status");
    assert_eq!(status.generation(), expected);
    assert_eq!(status.mode(), EventStoreMode::ReadWrite);

    let reader = SqliteStorage::open(OpenOptions::new(paths.clone(), OpenMode::ReadOnly))
        .await
        .expect("concurrent reader");
    let reader_status = EventStore::status(&reader).await.expect("reader status");
    assert_eq!(reader_status.generation(), expected);
    assert_eq!(reader_status.mode(), EventStoreMode::ReadOnly);
    drop(reader);

    assert!(matches!(
        SqliteStorage::open(OpenOptions::new(paths.clone(), OpenMode::ReadWriteExisting,)).await,
        Err(Error::WriterAlreadyActive { .. })
    ));
    let cloned = store.clone();
    drop(store);
    assert!(matches!(
        SqliteStorage::open(OpenOptions::new(paths.clone(), OpenMode::ReadWriteExisting,)).await,
        Err(Error::WriterAlreadyActive { .. })
    ));
    drop(cloned);

    let reopened = SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadWriteExisting))
        .await
        .expect("reopen after guard release");
    assert_eq!(
        EventStore::status(&reopened)
            .await
            .expect("reopened status")
            .generation(),
        expected
    );
}

#[tokio::test]
async fn fresh_store_requires_explicit_generation_and_exact_expectations() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let paths = Paths::from_directory(directory.path()).expect("owned paths");
    assert!(matches!(
        SqliteStorage::open(OpenOptions::new(paths.clone(), OpenMode::Create)).await,
        Err(Error::SourceGenerationRequired)
    ));
    assert!(!paths.runtime().exists());
    assert!(!paths.private().exists());

    let expected = generation(41);
    let store = SqliteStorage::open(
        OpenOptions::new(paths.clone(), OpenMode::Create)
            .with_source_generation(expected, 2_000)
            .expect("source generation"),
    )
    .await
    .expect("complete fresh store");
    drop(store);

    assert!(matches!(
        SqliteStorage::open(
            OpenOptions::new(paths, OpenMode::ReadWriteExisting)
                .with_source_generation(generation(42), 2_000)
                .expect("wrong expectation"),
        )
        .await,
        Err(Error::SourceGenerationMismatch)
    ));
}
