#[cfg(feature = "memory")]
use radroots_sdk::{ClientBuilder, capability::CapabilityId, error::ErrorKind};

const CLIENT_SOURCE: &str = include_str!("../src/client.rs");
const MANIFEST: &str = include_str!("../Cargo.toml");

#[test]
fn clean_default_path_contains_no_implicit_resource_or_worker_authority() {
    let production = CLIENT_SOURCE
        .split_once("#[cfg(all(test, feature = \"memory\"))]")
        .expect("production client boundary")
        .0;
    for forbidden in [
        "keyring",
        "reqwest",
        "radrootsd",
        "std::fs",
        "tokio::fs",
        "tokio::spawn",
        "std::thread::spawn",
        "Runtime::new",
        "File::create",
        "impl Drop for Client",
    ] {
        assert!(
            !production.contains(forbidden),
            "clean client path contains implicit authority `{forbidden}`"
        );
    }
    assert!(MANIFEST.contains("default = [\"memory\"]"));
    assert!(!MANIFEST.contains("default = [\"native\"]"));
    assert!(!MANIFEST.contains("default = [\"full\"]"));
}

#[cfg(feature = "memory")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_client_is_passive_clone_shared_and_explicitly_closed() {
    let generation =
        radroots_storage::event::SourceGeneration::new([41; 32]).expect("non-zero generation");
    let client = ClientBuilder::memory(generation).build().expect("client");

    let first_report = client.capabilities();
    let second_report = client.capabilities();
    assert_eq!(first_report, second_report);
    let storage = first_report
        .get(CapabilityId::CANONICAL_STORAGE)
        .expect("canonical storage status");
    assert!(storage.is_compiled());
    assert!(storage.is_configured());

    let unpolled_close = client.close();
    drop(unpolled_close);
    assert!(client.storage().is_ok());

    let first = client.clone();
    let second = client.clone();
    let first_close = tokio::spawn(async move { first.close().await });
    let second_close = tokio::spawn(async move { second.close().await });
    let outcomes = [
        first_close.await.expect("first close task"),
        second_close.await.expect("second close task"),
    ];
    assert!(outcomes.iter().all(|outcome| {
        outcome.is_ok()
            || outcome
                .as_ref()
                .is_err_and(|error| error.kind() == ErrorKind::CloseInProgress)
    }));
    if !client.is_closed() {
        client.close().await.expect("complete close");
    }

    assert!(client.is_closed());
    assert_eq!(
        client
            .storage()
            .err()
            .expect("closed client rejects storage")
            .kind(),
        ErrorKind::ClientClosed
    );
    assert_eq!(
        client
            .capabilities()
            .get(CapabilityId::CANONICAL_STORAGE)
            .expect("closed storage status")
            .availability(),
        radroots_sdk::capability::Availability::Unavailable
    );
}

#[cfg(all(feature = "memory", feature = "sqlite"))]
#[tokio::test]
async fn sqlite_resources_exist_only_after_explicit_open_and_close_across_clones() {
    use radroots_sdk::storage::{SqliteOpenMode, SqliteOptions, SqlitePaths};
    use radroots_storage::status::{ShutdownState, StorageBackend};

    let directory = tempfile::tempdir().expect("temporary directory");
    let empty_builder = ClientBuilder::new();
    assert!(
        directory
            .path()
            .read_dir()
            .expect("read tempdir")
            .next()
            .is_none()
    );
    drop(empty_builder);

    let paths = SqlitePaths::from_directory(directory.path()).expect("SQLite paths");
    let generation =
        radroots_storage::event::SourceGeneration::new([42; 32]).expect("non-zero generation");
    let options = SqliteOptions::new(paths, SqliteOpenMode::Create)
        .with_source_generation(generation, 1)
        .expect("source generation");
    assert!(
        directory
            .path()
            .read_dir()
            .expect("read tempdir")
            .next()
            .is_none()
    );

    let client = ClientBuilder::sqlite(options)
        .await
        .expect("explicit SQLite open")
        .build()
        .expect("client");
    assert!(
        directory
            .path()
            .read_dir()
            .expect("read tempdir")
            .next()
            .is_some()
    );
    let status = client.storage_status().await.expect("storage status");
    assert_eq!(status.backend(), StorageBackend::Sqlite);
    assert_eq!(status.shutdown(), ShutdownState::Open);

    let clone = client.clone();
    let unpolled_close = clone.close();
    drop(unpolled_close);
    assert!(client.storage().is_ok());
    client.close().await.expect("explicit close");
    assert!(clone.is_closed());
    assert_eq!(
        clone
            .storage_status()
            .await
            .expect_err("closed clone rejects inspection")
            .kind(),
        ErrorKind::ClientClosed
    );
}
