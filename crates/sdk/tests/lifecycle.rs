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

#[cfg(all(feature = "memory", feature = "nostr"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_terminates_a_pending_owned_connection_and_closes_retained_slot_clones() {
    use radroots_sdk::transport::{
        NostrSlot, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind, RelayUrlPolicy,
    };
    use radroots_transport::{FetchRequest, source::FetchBounds};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use tokio::{io::AsyncReadExt, net::TcpListener, sync::oneshot};

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let (connected, ready) = oneshot::channel();
    let mut server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = [0; 4096];
        assert!(stream.read(&mut bytes).await.unwrap() > 0);
        connected.send(()).unwrap();
        // Deliberately withhold the upgrade. The configured connection timeout
        // is ten seconds; owner shutdown must close this socket before that.
        loop {
            match stream.read(&mut bytes).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });
    let profile = RelayProfile::explicit(
        RelayProfileKind::Simulator,
        [RelayEndpoint::new(&url, RelayUrlPolicy::Local, RelayAccess::ReadOnly).unwrap()],
    )
    .unwrap();
    let slot = NostrSlot::new();
    slot.configure(profile.clone()).await.unwrap();
    let targets = slot.read_targets().unwrap();
    let request = FetchRequest::new(
        "close-pending-connect",
        targets,
        FetchBounds::new(
            1,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                + 5000,
        )
        .unwrap(),
    )
    .unwrap();
    let client = ClientBuilder::memory_default()
        .nostr(slot.clone())
        .build()
        .unwrap();
    let clone = client.clone();
    let mut fetch =
        tokio::spawn(async move { clone.source().unwrap().unwrap().fetch(request).await });
    tokio::time::timeout(Duration::from_secs(2), ready)
        .await
        .unwrap()
        .unwrap();
    drop(client.close());
    assert!(!client.is_closed());
    client.close().await.unwrap();
    assert!(client.is_closed());
    let disconnected = tokio::time::timeout(Duration::from_secs(2), &mut server).await;
    if disconnected.is_err() {
        server.abort();
        let _ = server.await;
        fetch.abort();
        let _ = fetch.await;
        panic!("client close left a pending owned socket alive");
    }
    disconnected.unwrap().unwrap();
    let finished = tokio::time::timeout(Duration::from_secs(5), &mut fetch).await;
    if finished.is_err() {
        fetch.abort();
        let _ = fetch.await;
        panic!("started fetch did not terminate after shutdown");
    }
    let _obtained_fetch_outcome = finished.unwrap().unwrap();
    assert_eq!(
        slot.configure(profile).await.unwrap_err().kind(),
        ErrorKind::ClientClosed
    );
    assert!(slot.read_targets().is_none());
    client.close().await.unwrap();
}
