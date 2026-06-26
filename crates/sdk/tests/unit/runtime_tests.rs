use super::*;
use std::time::{Duration, SystemTime};

fn invalid_request_message<T>(result: Result<T, RadrootsSdkError>) -> String {
    match result.err().expect("expected invalid request error") {
        RadrootsSdkError::InvalidRequest { message } => message,
        other => panic!("expected invalid request error, got {other:?}"),
    }
}

fn io_message<T>(result: Result<T, RadrootsSdkError>) -> String {
    match result.err().expect("expected io error") {
        RadrootsSdkError::Io { message, .. } => message,
        other => panic!("expected io error, got {other:?}"),
    }
}

fn assert_event_store_error<T>(result: Result<T, RadrootsSdkError>) {
    match result {
        Err(RadrootsSdkError::EventStore { .. }) => {}
        Err(other) => panic!("expected event store error, got {other:?}"),
        Ok(_) => panic!("expected event store error"),
    }
}

fn assert_outbox_error<T>(result: Result<T, RadrootsSdkError>) {
    match result {
        Err(RadrootsSdkError::Outbox { .. }) => {}
        Err(other) => panic!("expected outbox error, got {other:?}"),
        Ok(_) => panic!("expected outbox error"),
    }
}

fn assert_private_store_error<T>(result: Result<T, RadrootsSdkError>) {
    match result {
        Err(RadrootsSdkError::PrivateStore { .. }) => {}
        Err(other) => panic!("expected private store error, got {other:?}"),
        Ok(_) => panic!("expected private store error"),
    }
}

fn sqlite_status() -> SdkSqliteStoreStatus {
    SdkSqliteStoreStatus {
        schema_version: 1,
        journal_mode: "wal".to_owned(),
        foreign_keys_enabled: true,
        busy_timeout_ms: 5_000,
        integrity_ok: true,
        integrity_result: "ok".to_owned(),
    }
}

fn storage_status() -> StorageStatusReceipt {
    StorageStatusReceipt {
        storage: SdkStorageKind::Memory,
        paths: None,
        event_store: SdkEventStoreStorageStatus {
            store: sqlite_status(),
            total_events: 0,
            projection_eligible_events: 0,
            relay_observations: 0,
            last_event_seq: None,
            last_event_updated_at_ms: None,
        },
        outbox: SdkOutboxStorageStatus {
            store: sqlite_status(),
            total_events: 0,
            pending_events: 0,
            retryable_events: 0,
            terminal_events: 0,
            failed_terminal_events: 0,
            ready_signed_events: 0,
            publishing_events: 0,
            last_attempt_at_ms: None,
            last_error: None,
        },
        private_store: SdkPrivateStoreStorageStatus {
            store: sqlite_status(),
            farm_private_locations: 0,
        },
    }
}

fn verification(event_store_ok: bool, outbox_ok: bool) -> SdkBackupVerification {
    SdkBackupVerification {
        event_store_ok,
        outbox_ok,
        private_store_ok: true,
        event_store_events: 0,
        outbox_events: 0,
        private_farm_locations: 0,
    }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions).expect("permissions");
}

#[cfg(unix)]
fn non_utf8_path() -> PathBuf {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    PathBuf::from(OsString::from_vec(b"invalid-\xFF.sqlite".to_vec()))
}

#[cfg(unix)]
fn nul_path() -> PathBuf {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    PathBuf::from(OsString::from_vec(b"invalid-\0path".to_vec()))
}

fn manifest() -> SdkBackupManifest {
    SdkBackupManifest {
        manifest_kind: SDK_STORAGE_MANIFEST_KIND,
        manifest_version: SDK_STORAGE_MANIFEST_VERSION,
        sdk_version: "0.1.0".to_owned(),
        created_at_ms: 1_700_000_000_000,
        source_storage: SdkStorageKind::Memory,
        source_paths: None,
        backup_paths: RadrootsSdkStoragePaths {
            event_store_path: PathBuf::from(EVENT_STORE_BACKUP_FILE),
            outbox_path: PathBuf::from(OUTBOX_BACKUP_FILE),
            private_store_path: PathBuf::from(PRIVATE_STORE_BACKUP_FILE),
        },
        source_status: storage_status(),
        backup_verification: verification(true, true),
    }
}

fn private_farm_location_record() -> crate::private_store::SdkPrivateFarmLocationRecord {
    crate::private_store::SdkPrivateFarmLocationRecord {
        farm_addr: radroots_events::ids::RadrootsAddressableCoordinate::parse(format!(
            "{}:{}:{}",
            radroots_events::kinds::KIND_FARM,
            "a".repeat(64),
            "AAAAAAAAAAAAAAAAAAAAAA"
        ))
        .expect("farm addr"),
        farm_pubkey: "a".repeat(64),
        farm_d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_owned(),
        latitude: 12.26,
        longitude: -34.51,
        locality_primary: "Fixture Town".to_owned(),
        locality_city: Some("Fixture Town".to_owned()),
        locality_region: Some("Fixture Region".to_owned()),
        locality_country: Some("Fixture Country".to_owned()),
        geohash5: "e4pmw".to_owned(),
        geonames_feature_id: Some(1),
        geonames_country_id: Some("FX".to_owned()),
        updated_at_ms: 1_700_000_123_000,
    }
}

#[tokio::test]
async fn private_store_validates_location_rows_and_round_trips_valid_records() {
    let store = SdkPrivateStore::open_memory().await.expect("private store");
    let record = private_farm_location_record();
    store
        .upsert_farm_location(&record)
        .await
        .expect("valid private farm location");
    assert_eq!(
        store
            .farm_location(&record.farm_addr)
            .await
            .expect("lookup"),
        Some(record.clone())
    );

    let mut invalid_coordinates = record.clone();
    invalid_coordinates.latitude = f64::NAN;
    assert!(matches!(
        store.upsert_farm_location(&invalid_coordinates).await,
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let mut blank_locality = record.clone();
    blank_locality.locality_primary = " ".to_owned();
    assert!(matches!(
        store.upsert_farm_location(&blank_locality).await,
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let mut invalid_geohash = record;
    invalid_geohash.geohash5 = "abcd".to_owned();
    assert!(matches!(
        store.upsert_farm_location(&invalid_geohash).await,
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
}

#[tokio::test]
async fn open_storage_and_storage_kind_cover_memory_directory_and_file_failures() {
    let memory = open_storage(&RadrootsSdkStorageConfig::Memory)
        .await
        .expect("memory storage");
    assert!(memory.paths.is_none());
    let memory_sdk = RadrootsClient {
        _event_store: memory.event_store,
        _outbox: memory.outbox,
        _private_store: memory.private_store,
        storage_paths: None,
        geonames: None,
        clock: RadrootsSdkClock::Fixed(RadrootsSdkTimestamp::from_unix_seconds(1)),
        relay_urls: Vec::new(),
        publish_transport: SdkPublishTransport::DirectNostrRelay,
        #[cfg(feature = "signer-adapters")]
        signer_provider: None,
    };
    assert_eq!(memory_sdk.storage_kind(), SdkStorageKind::Memory);

    let tempdir = tempfile::tempdir().expect("tempdir");
    let directory = tempdir.path().join("sdk");
    let directory_storage = open_storage(&RadrootsSdkStorageConfig::Directory(directory))
        .await
        .expect("directory storage");
    let directory_paths = directory_storage.paths.expect("directory paths");
    assert!(directory_paths.event_store_path.exists());
    assert!(directory_paths.outbox_path.exists());
    assert!(directory_paths.private_store_path.exists());
    let directory_sdk = RadrootsClient {
        _event_store: directory_storage.event_store,
        _outbox: directory_storage.outbox,
        _private_store: directory_storage.private_store,
        storage_paths: Some(directory_paths),
        geonames: None,
        clock: RadrootsSdkClock::Fixed(RadrootsSdkTimestamp::from_unix_seconds(1)),
        relay_urls: Vec::new(),
        publish_transport: SdkPublishTransport::DirectNostrRelay,
        #[cfg(feature = "signer-adapters")]
        signer_provider: None,
    };
    assert_eq!(directory_sdk.storage_kind(), SdkStorageKind::Directory);

    let file_path = tempdir.path().join("not-directory");
    fs::write(&file_path, b"file").expect("file");
    assert!(!io_message(open_directory_storage(&file_path).await).is_empty());

    let event_store_directory = tempdir.path().join("event-store-directory");
    fs::create_dir(&event_store_directory).expect("event store dir");
    fs::create_dir(event_store_directory.join(EVENT_STORE_BACKUP_FILE))
        .expect("event store file slot dir");
    assert_event_store_error(open_directory_storage(&event_store_directory).await);

    let outbox_directory = tempdir.path().join("outbox-directory");
    fs::create_dir(&outbox_directory).expect("outbox dir");
    fs::create_dir(outbox_directory.join(OUTBOX_BACKUP_FILE)).expect("outbox file slot dir");
    assert_outbox_error(open_directory_storage(&outbox_directory).await);

    let private_store_directory = tempdir.path().join("private-store-directory");
    fs::create_dir(&private_store_directory).expect("private store dir");
    fs::create_dir(private_store_directory.join(PRIVATE_STORE_BACKUP_FILE))
        .expect("private store file slot dir");
    assert_private_store_error(open_directory_storage(&private_store_directory).await);
}

#[tokio::test]
async fn runtime_public_surface_covers_builders_status_integrity_backup_and_restore() {
    assert_eq!(
        RadrootsSdkStorageConfig::default(),
        RadrootsSdkStorageConfig::Memory
    );
    assert_eq!(RadrootsSdkClock::default(), RadrootsSdkClock::System);
    assert!(
        RadrootsSdkTimestamp::from_unix_seconds(u64::from(u32::MAX) + 1)
            .try_into_nostr_created_at()
            .is_err()
    );

    let memory_sdk = RadrootsClient::builder()
        .storage(RadrootsSdkStorageConfig::Memory)
        .clock(RadrootsSdkClock::Fixed(
            RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000),
        ))
        .relay_url_policy(SdkRelayUrlPolicy::Localhost)
        .relay_url("ws://127.0.0.1:7777")
        .build()
        .await
        .expect("memory sdk");
    assert_eq!(
        memory_sdk.now().expect("fixed now").unix_seconds(),
        1_700_000_000
    );
    assert_eq!(memory_sdk.relay_urls(), ["ws://127.0.0.1:7777"]);
    assert!(memory_sdk.storage_paths().is_none());
    let _ = memory_sdk.farms();
    let _ = memory_sdk.listings();
    let _ = memory_sdk.trades();
    let _ = memory_sdk.sync();
    let memory_status = memory_sdk
        .storage_status(StorageStatusRequest::new())
        .await
        .expect("memory status");
    assert_eq!(memory_status.storage, SdkStorageKind::Memory);
    let memory_integrity = memory_sdk
        .integrity(IntegrityRequest::new())
        .await
        .expect("memory integrity");
    assert!(memory_integrity.event_store_ok);
    assert!(memory_integrity.outbox_ok);

    let tempdir = tempfile::tempdir().expect("tempdir");
    let directory = tempdir.path().join("sdk");
    let directory_sdk = RadrootsClient::builder()
        .directory_storage(&directory)
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_001))
        .build()
        .await
        .expect("directory sdk");
    assert!(directory_sdk.storage_paths().is_some());

    let backup_destination = tempdir.path().join("backup");
    let backup = directory_sdk
        .backup(BackupRequest::new(&backup_destination).with_overwrite(false))
        .await
        .expect("backup");
    assert_eq!(backup.state, SdkBackupState::Completed);
    assert!(
        backup
            .manifest_path
            .as_ref()
            .is_some_and(|path| path.exists())
    );

    let archive = RadrootsClient::inspect_restore_archive(&backup_destination)
        .await
        .expect("restore archive");
    assert_eq!(archive.manifest, backup.manifest);

    let restore_destination = tempdir.path().join("restore");
    let dry_run = RadrootsClient::restore(
        RestoreRequest::new(&backup_destination)
            .with_destination(&restore_destination)
            .with_overwrite(false)
            .with_dry_run(true),
    )
    .await
    .expect("dry-run restore");
    assert_eq!(dry_run.state, SdkRestoreState::DryRun);
    assert!(dry_run.restored_paths.is_none());

    let restore = RadrootsClient::restore(
        RestoreRequest::new(&backup_destination)
            .with_destination(&restore_destination)
            .with_overwrite(true),
    )
    .await
    .expect("restore");
    assert_eq!(restore.state, SdkRestoreState::Completed);
    assert!(restore.restored_paths.is_some());

    let dry_request = RestoreRequest::new(&backup_destination)
        .with_destination(tempdir.path().join("restore-dry-helper"))
        .dry_run();
    assert!(dry_request.dry_run);
}

#[tokio::test]
async fn runtime_clock_errors_cover_sdk_now_callers() {
    assert!(matches!(
        RadrootsSdkClock::BeforeUnixEpoch.now(),
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
    let sdk = RadrootsClient::builder()
        .clock(RadrootsSdkClock::BeforeUnixEpoch)
        .build()
        .await
        .expect("sdk");
    assert!(matches!(
        sdk_now_ms(&sdk),
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
    assert!(matches!(
        sdk.storage_status(StorageStatusRequest::new()).await,
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
    let tempdir = tempfile::tempdir().expect("tempdir");
    assert!(matches!(
        sdk.backup(BackupRequest::new(tempdir.path().join("backup")))
            .await,
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
}

#[test]
fn system_time_converters_cover_epoch_success_and_failure_edges() {
    assert_eq!(
        sdk_timestamp_from_system_time(UNIX_EPOCH + Duration::from_secs(42))
            .expect("timestamp")
            .unix_seconds(),
        42
    );
    assert!(matches!(
        sdk_timestamp_from_system_time(UNIX_EPOCH - Duration::from_secs(1)),
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
    assert_eq!(
        system_time_nanos_since_unix_epoch(UNIX_EPOCH + Duration::from_nanos(7)).expect("nanos"),
        7
    );
    assert!(matches!(
        system_time_nanos_since_unix_epoch(UNIX_EPOCH - Duration::from_nanos(1)),
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
}

#[tokio::test]
async fn storage_status_integrity_and_backup_map_closed_pool_errors() {
    let event_store_closed = RadrootsClient::builder().build().await.expect("sdk");
    event_store_closed._event_store.pool().close().await;
    assert!(matches!(
        event_store_closed
            .storage_status(StorageStatusRequest::new())
            .await,
        Err(RadrootsSdkError::EventStore { .. })
    ));
    assert_event_store_error(event_store_sqlite_status(&event_store_closed._event_store).await);
    assert_event_store_error(event_store_status_summary(&event_store_closed._event_store).await);
    assert!(matches!(
        event_store_closed.integrity(IntegrityRequest::new()).await,
        Err(RadrootsSdkError::EventStore { .. })
    ));
    let tempdir = tempfile::tempdir().expect("tempdir");
    let backup_destination = tempdir.path().join("backup");
    assert!(matches!(
        event_store_closed
            .backup(BackupRequest::new(backup_destination))
            .await,
        Err(RadrootsSdkError::EventStore { .. })
    ));

    let outbox_closed = RadrootsClient::builder().build().await.expect("sdk");
    outbox_closed._outbox.pool().close().await;
    assert!(matches!(
        outbox_closed
            .storage_status(StorageStatusRequest::new())
            .await,
        Err(RadrootsSdkError::Outbox { .. })
    ));
    assert_outbox_error(outbox_sqlite_status(&outbox_closed._outbox).await);
    assert_outbox_error(outbox_status_summary(&outbox_closed._outbox, 1).await);
    assert!(matches!(
        outbox_closed.integrity(IntegrityRequest::new()).await,
        Err(RadrootsSdkError::EventStore { .. })
    ));
}

#[test]
fn restore_archive_path_validators_cover_missing_outside_and_manifest_edges() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let source = tempdir.path().join("source");
    fs::create_dir(&source).expect("source");
    let source_root = canonical_restore_directory(&source).expect("canonical source");
    let file_member = source.join(EVENT_STORE_BACKUP_FILE);
    fs::write(&file_member, b"sqlite").expect("file member");
    let outside_file = tempdir.path().join("outside.sqlite");
    fs::write(&outside_file, b"sqlite").expect("outside file");
    let dir_member = source.join("dir-member");
    fs::create_dir(&dir_member).expect("dir member");

    assert!(
        validate_relative_archive_path(Path::new(EVENT_STORE_BACKUP_FILE), "event store").is_ok()
    );
    assert!(
        invalid_request_message(validate_relative_archive_path(Path::new(""), "event store"))
            .contains("must not be empty")
    );
    assert!(
        invalid_request_message(validate_relative_archive_path(
            Path::new("../outside.sqlite"),
            "event store",
        ))
        .contains("relative and contained")
    );
    assert!(validate_restore_member_path(&source_root, &file_member, "event store").is_ok());
    assert!(
        invalid_request_message(validate_restore_member_path(
            &source_root,
            &dir_member,
            "event store",
        ))
        .contains("regular file")
    );
    assert!(
        invalid_request_message(validate_restore_member_path(
            &source_root,
            &outside_file,
            "event store",
        ))
        .contains("inside the backup directory")
    );
    assert!(
        io_message(validate_restore_member_path(
            &source_root,
            &source.join("missing.sqlite"),
            "event store",
        ))
        .contains("No such")
    );
    assert!(
        restore_archive_member_path(
            &source_root,
            Path::new(EVENT_STORE_BACKUP_FILE),
            "event store",
        )
        .is_ok()
    );
    assert!(
        invalid_request_message(restore_archive_member_path(
            &source_root,
            Path::new("../outside.sqlite"),
            "event store",
        ))
        .contains("relative and contained")
    );

    assert!(write_backup_manifest(&source.join(BACKUP_MANIFEST_FILE), &manifest()).is_ok());
    assert!(!io_message(write_backup_manifest(tempdir.path(), &manifest())).is_empty());
    assert!(
        !io_message(canonicalize_restore_path(
            &tempdir.path().join("missing-canonical-path"),
        ))
        .is_empty()
    );

    let mut unsupported_version = manifest();
    unsupported_version.manifest_version = SDK_STORAGE_MANIFEST_VERSION + 1;
    assert!(
        invalid_request_message(validate_restore_manifest(&unsupported_version))
            .contains("version")
    );

    let ok = verification(true, true);
    assert!(validate_restore_verification(&ok, &ok).is_ok());
    assert!(
        invalid_request_message(validate_restore_verification(
            &verification(false, true),
            &ok,
        ))
        .contains("integrity")
    );
    let mismatch = SdkBackupVerification {
        event_store_events: 1,
        ..ok.clone()
    };
    assert!(
        invalid_request_message(validate_restore_verification(&mismatch, &ok))
            .contains("does not match manifest")
    );
}

#[test]
fn restore_destination_preflight_covers_empty_existing_new_and_overlap_paths() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let source = tempdir.path().join("source");
    let parent = tempdir.path().join("parent");
    fs::create_dir(&source).expect("source");
    fs::create_dir(&parent).expect("parent");

    assert!(
        invalid_request_message(preflight_restore_destination(&source, Path::new(""), false))
            .contains("destination must not be empty")
    );

    let new_destination = parent.join("new-destination");
    let paths =
        preflight_restore_destination(&source, &new_destination, false).expect("new preflight");
    assert_eq!(
        paths.event_store_path,
        new_destination.join(EVENT_STORE_BACKUP_FILE)
    );
    let relative_destination = PathBuf::from(format!(
        "relative-restore-{}",
        system_time_nanos_since_unix_epoch(SystemTime::now()).expect("time")
    ));
    let relative_paths = preflight_restore_destination(&source, &relative_destination, false)
        .expect("relative preflight");
    assert_eq!(
        relative_paths.event_store_path,
        relative_destination.join(EVENT_STORE_BACKUP_FILE)
    );

    let file_source = tempdir.path().join("file-source");
    fs::write(&file_source, b"source file").expect("file source");
    assert!(
        invalid_request_message(preflight_restore_destination(
            &file_source,
            &parent.join("file-source-restore"),
            false,
        ))
        .contains("source must be a directory")
    );

    let empty_directory = parent.join("empty");
    fs::create_dir(&empty_directory).expect("empty dir");
    assert!(preflight_restore_destination(&source, &empty_directory, false).is_ok());

    let nonempty_directory = parent.join("nonempty");
    fs::create_dir(&nonempty_directory).expect("nonempty dir");
    fs::write(nonempty_directory.join("entry"), b"entry").expect("entry");
    assert!(
        invalid_request_message(preflight_restore_destination(
            &source,
            &nonempty_directory,
            false,
        ))
        .contains("overwrite is false")
    );
    assert!(preflight_restore_destination(&source, &nonempty_directory, true).is_ok());

    let file_destination = parent.join("file-destination");
    fs::write(&file_destination, b"file").expect("file");
    assert!(
        invalid_request_message(preflight_restore_destination(
            &source,
            &file_destination,
            false,
        ))
        .contains("overwrite is false")
    );
    assert!(preflight_restore_destination(&source, &file_destination, true).is_ok());

    let nested_file_destination = source.join("nested-file-destination");
    fs::write(&nested_file_destination, b"nested").expect("nested destination");
    assert!(
        invalid_request_message(preflight_restore_destination(
            &source,
            &nested_file_destination,
            true,
        ))
        .contains("must not overlap")
    );

    #[cfg(unix)]
    {
        let socket_parent = tempfile::Builder::new()
            .prefix("rrsdk")
            .tempdir_in("/tmp")
            .expect("short socket tempdir");
        let socket_destination = socket_parent.path().join("socket-destination");
        let _listener =
            std::os::unix::net::UnixListener::bind(&socket_destination).expect("socket");
        assert!(
            invalid_request_message(preflight_restore_destination(
                &source,
                &socket_destination,
                true,
            ))
            .contains("directory path")
        );
    }

    assert!(
        invalid_request_message(reject_restore_destination_overlap(
            &source,
            &source.join("nested"),
        ))
        .contains("must not overlap")
    );
    assert!(
        invalid_request_message(reject_restore_destination_overlap(tempdir.path(), &source))
            .contains("must not overlap")
    );
    assert!(
        invalid_request_message(preflight_restore_destination(&source, &source, true))
            .contains("must not overlap")
    );

    let file_parent = tempdir.path().join("file-parent");
    fs::write(&file_parent, b"file").expect("file parent");
    assert!(
        !io_message(preflight_restore_destination(
            &source,
            &file_parent.join("restore"),
            false,
        ))
        .is_empty()
    );
}

#[test]
fn backup_destination_and_restore_file_helpers_cover_cleanup_and_io_edges() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let new_backup = tempdir.path().join("backup-new");
    prepare_backup_destination(&new_backup, false).expect("new backup destination");
    assert!(new_backup.is_dir());
    assert!(
        invalid_request_message(prepare_backup_destination(&new_backup, false))
            .contains("already exists")
    );

    let file_backup = tempdir.path().join("backup-file");
    fs::write(&file_backup, b"file").expect("backup file");
    prepare_backup_destination(&file_backup, true).expect("overwrite file backup");
    assert!(file_backup.is_dir());

    let directory_backup = tempdir.path().join("backup-directory");
    fs::create_dir(&directory_backup).expect("backup dir");
    fs::write(directory_backup.join("entry"), b"entry").expect("backup dir entry");
    prepare_backup_destination(&directory_backup, true).expect("overwrite dir backup");
    assert!(directory_backup.is_dir());
    assert!(
        directory_backup
            .join("entry")
            .try_exists()
            .is_ok_and(|exists| !exists)
    );

    let missing = tempdir.path().join("missing");
    assert!(remove_existing_restore_path(&missing).is_ok());
    assert!(!io_message(remove_existing_restore_path(&nul_path())).is_empty());

    let restore_file = tempdir.path().join("restore-file");
    fs::write(&restore_file, b"file").expect("restore file");
    remove_existing_restore_path(&restore_file).expect("remove restore file");
    assert!(!restore_file.exists());

    let restore_dir = tempdir.path().join("restore-dir");
    fs::create_dir(&restore_dir).expect("restore dir");
    fs::write(restore_dir.join("entry"), b"entry").expect("restore dir entry");
    remove_existing_restore_path(&restore_dir).expect("remove restore dir");
    assert!(!restore_dir.exists());

    assert!(
        io_message(copy_restore_file(
            &tempdir.path().join("missing-source"),
            &tempdir.path().join("copy-destination"),
            "event store",
        ))
        .contains("restore event store copy failed")
    );
    assert!(
        io_message(rename_restore_path(
            &tempdir.path().join("missing-source"),
            &tempdir.path().join("rename-destination"),
            "previous destination",
        ))
        .contains("restore previous destination rename failed")
    );
    assert!(
        invalid_request_message(unique_restore_sidecar_path(
            tempdir.path(),
            Path::new(""),
            "staging",
        ))
        .contains("directory name")
    );

    let destination = tempdir.path().join("destination");
    let previous = tempdir.path().join("previous");
    fs::write(&destination, b"current").expect("current destination");
    fs::write(&previous, b"previous").expect("previous destination");
    rollback_restore_destination(&destination, &previous, true);
    assert_eq!(
        fs::read(&destination).expect("rolled back destination"),
        b"previous"
    );
    rollback_restore_destination(&destination, &previous, false);
}

#[test]
fn unique_restore_sidecar_path_reserves_after_collisions_and_reports_exhaustion() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let collision = tempdir.path().join(".restore.radroots-restore-staging-7-0");
    fs::write(&collision, b"taken").expect("collision");
    let reserved = unique_restore_sidecar_path_with_nanos(tempdir.path(), "restore", "staging", 7)
        .expect("reserved after collision");
    assert!(reserved.ends_with(".restore.radroots-restore-staging-7-1"));

    for attempt in 1..100u8 {
        fs::write(
            tempdir
                .path()
                .join(format!(".restore.radroots-restore-staging-7-{attempt}")),
            b"taken",
        )
        .expect("attempt collision");
    }
    assert!(
        invalid_request_message(unique_restore_sidecar_path_with_nanos(
            tempdir.path(),
            "restore",
            "staging",
            7,
        ))
        .contains("could not reserve")
    );
    assert!(
        !io_message(unique_restore_sidecar_path_with_nanos(
            nul_path().as_path(),
            "restore",
            "staging",
            8,
        ))
        .is_empty()
    );

    let missing_staging = tempdir.path().join("missing-staging");
    let missing_destination = tempdir.path().join("missing-destination");
    let missing_previous = tempdir.path().join("missing-previous");
    assert!(
        !io_message(install_restore_staging(
            &missing_staging,
            &missing_destination,
            &missing_previous,
        ))
        .is_empty()
    );
    assert!(!missing_destination.exists());

    let rollback_destination = tempdir.path().join("rollback-destination");
    fs::create_dir(&rollback_destination).expect("rollback destination");
    fs::write(rollback_destination.join("old"), b"old").expect("old entry");
    let rollback_previous = tempdir.path().join("rollback-previous");
    assert!(
        !io_message(install_restore_staging(
            &missing_staging,
            &rollback_destination,
            &rollback_previous,
        ))
        .is_empty()
    );
    assert!(rollback_destination.join("old").exists());
    assert!(!rollback_previous.exists());
}

#[cfg(unix)]
#[test]
fn permission_denied_paths_cover_backup_restore_io_edges() {
    let tempdir = tempfile::tempdir().expect("tempdir");

    let protected_create_parent = tempdir.path().join("protected-create");
    fs::create_dir(&protected_create_parent).expect("protected create parent");
    set_mode(&protected_create_parent, 0o500);
    let create_result = prepare_backup_destination(&protected_create_parent.join("backup"), false);
    set_mode(&protected_create_parent, 0o700);
    assert!(!io_message(create_result).is_empty());

    let hidden_backup_parent = tempdir.path().join("hidden-backup-parent");
    fs::create_dir(&hidden_backup_parent).expect("hidden backup parent");
    let hidden_backup = hidden_backup_parent.join("backup");
    set_mode(&hidden_backup_parent, 0o000);
    let metadata_result = prepare_backup_destination(&hidden_backup, false);
    set_mode(&hidden_backup_parent, 0o700);
    assert!(!io_message(metadata_result).is_empty());

    let protected_backup_parent = tempdir.path().join("protected-backup");
    fs::create_dir(&protected_backup_parent).expect("protected backup parent");
    let protected_backup_dir = protected_backup_parent.join("backup-dir");
    fs::create_dir(&protected_backup_dir).expect("protected backup dir");
    set_mode(&protected_backup_parent, 0o500);
    let remove_dir_result = prepare_backup_destination(&protected_backup_dir, true);
    set_mode(&protected_backup_parent, 0o700);
    assert!(!io_message(remove_dir_result).is_empty());

    let protected_backup_file = protected_backup_parent.join("backup-file");
    fs::write(&protected_backup_file, b"backup").expect("protected backup file");
    set_mode(&protected_backup_parent, 0o500);
    let remove_file_result = prepare_backup_destination(&protected_backup_file, true);
    set_mode(&protected_backup_parent, 0o700);
    assert!(!io_message(remove_file_result).is_empty());

    let protected_restore_parent = tempdir.path().join("protected-restore");
    fs::create_dir(&protected_restore_parent).expect("protected restore parent");
    let protected_restore_dir = protected_restore_parent.join("restore-dir");
    fs::create_dir(&protected_restore_dir).expect("protected restore dir");
    set_mode(&protected_restore_parent, 0o500);
    let remove_restore_dir_result = remove_existing_restore_path(&protected_restore_dir);
    set_mode(&protected_restore_parent, 0o700);
    assert!(!io_message(remove_restore_dir_result).is_empty());

    let protected_restore_file = protected_restore_parent.join("restore-file");
    fs::write(&protected_restore_file, b"restore").expect("protected restore file");
    set_mode(&protected_restore_parent, 0o500);
    let remove_restore_file_result = remove_existing_restore_path(&protected_restore_file);
    set_mode(&protected_restore_parent, 0o700);
    assert!(!io_message(remove_restore_file_result).is_empty());

    let source = tempdir.path().join("source");
    let destination = tempdir.path().join("destination");
    fs::create_dir(&source).expect("source");
    fs::create_dir(&destination).expect("destination");

    set_mode(&destination, 0o300);
    let read_dir_result = preflight_restore_destination(&source, &destination, false);
    set_mode(&destination, 0o700);
    assert!(!io_message(read_dir_result).is_empty());

    let no_execute_destination = tempdir.path().join("no-execute-destination");
    fs::create_dir(&no_execute_destination).expect("no execute destination");
    set_mode(&no_execute_destination, 0o200);
    let canonicalize_result =
        preflight_restore_destination(&source, &no_execute_destination, false);
    set_mode(&no_execute_destination, 0o700);
    assert!(!io_message(canonicalize_result).is_empty());

    let hidden_parent = tempdir.path().join("hidden-parent");
    fs::create_dir(&hidden_parent).expect("hidden parent");
    let hidden_destination = hidden_parent.join("destination");
    set_mode(&hidden_parent, 0o000);
    let metadata_result = preflight_restore_destination(&source, &hidden_destination, false);
    set_mode(&hidden_parent, 0o700);
    assert!(!io_message(metadata_result).is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn sqlite_backup_errors_cover_invalid_paths_and_execute_failures() {
    let storage = open_storage(&RadrootsSdkStorageConfig::Memory)
        .await
        .expect("memory storage");

    assert!(
        invalid_request_message(
            sqlite_vacuum_into(storage.event_store.pool(), &non_utf8_path(), "event store",).await
        )
        .contains("valid UTF-8")
    );

    storage.event_store.pool().close().await;
    let tempdir = tempfile::tempdir().expect("tempdir");
    let closed_pool_destination = tempdir.path().join("closed-pool.sqlite");
    let error = sqlite_vacuum_into(
        storage.event_store.pool(),
        &closed_pool_destination,
        "event store",
    )
    .await
    .err()
    .expect("sqlite error");
    assert!(matches!(
        error,
        RadrootsSdkError::EventStore { message } if message.contains("backup failed")
    ));
    let backup_paths = RadrootsSdkStoragePaths {
        event_store_path: tempdir.path().join("closed-event-store-backup.sqlite"),
        outbox_path: tempdir.path().join("closed-event-store-outbox.sqlite"),
        private_store_path: tempdir.path().join("closed-event-store-private.sqlite"),
    };
    assert_event_store_error(
        backup_sqlite_stores(
            storage.event_store.pool(),
            storage.outbox.pool(),
            storage.private_store.pool(),
            &backup_paths,
        )
        .await,
    );

    let outbox_closed_storage = open_storage(&RadrootsSdkStorageConfig::Memory)
        .await
        .expect("outbox closed storage");
    outbox_closed_storage.outbox.pool().close().await;
    let outbox_closed_paths = RadrootsSdkStoragePaths {
        event_store_path: tempdir.path().join("open-event-store-backup.sqlite"),
        outbox_path: tempdir.path().join("closed-outbox-backup.sqlite"),
        private_store_path: tempdir.path().join("outbox-closed-private.sqlite"),
    };
    assert_event_store_error(
        backup_sqlite_stores(
            outbox_closed_storage.event_store.pool(),
            outbox_closed_storage.outbox.pool(),
            outbox_closed_storage.private_store.pool(),
            &outbox_closed_paths,
        )
        .await,
    );
    assert!(
        !io_message(write_backup_receipt(
            tempdir.path().join("receipt-destination"),
            RadrootsSdkStoragePaths {
                event_store_path: tempdir.path().join(EVENT_STORE_BACKUP_FILE),
                outbox_path: tempdir.path().join(OUTBOX_BACKUP_FILE),
                private_store_path: tempdir.path().join(PRIVATE_STORE_BACKUP_FILE),
            },
            tempdir.path().to_path_buf(),
            manifest(),
        ))
        .is_empty()
    );

    let integrity_error = sqlite_integrity_result(storage.event_store.pool())
        .await
        .err()
        .expect("integrity error");
    assert!(matches!(
        integrity_error,
        RadrootsSdkError::EventStore { .. }
    ));
    assert_event_store_error(
        sqlite_store_status(storage.event_store.pool(), 1, "wal".to_owned(), true, 5_000).await,
    );
}

#[cfg(unix)]
#[tokio::test]
async fn restore_archive_private_failures_cover_staging_and_verification_edges() {
    let tempdir = tempfile::tempdir().expect("tempdir");

    let unreadable_source = tempdir.path().join("unreadable-source");
    fs::create_dir(&unreadable_source).expect("unreadable source");
    let unreadable_manifest = unreadable_source.join(BACKUP_MANIFEST_FILE);
    fs::write(&unreadable_manifest, b"{}").expect("unreadable manifest");
    set_mode(&unreadable_manifest, 0o000);
    let inspect_result = inspect_restore_archive(unreadable_source).await;
    set_mode(&unreadable_manifest, 0o600);
    assert!(!io_message(inspect_result).is_empty());

    let missing_archive = RestoreArchive {
        source: tempdir.path().join("missing-archive"),
        event_store_path: tempdir.path().join("missing-event-store.sqlite"),
        outbox_path: tempdir.path().join("missing-outbox.sqlite"),
        private_store_path: tempdir.path().join("missing-private.sqlite"),
        manifest_path: tempdir.path().join(BACKUP_MANIFEST_FILE),
        manifest: manifest(),
        verification: verification(true, true),
    };
    let staging_paths = RadrootsSdkStoragePaths {
        event_store_path: tempdir.path().join("staging-event-store.sqlite"),
        outbox_path: tempdir.path().join("staging-outbox.sqlite"),
        private_store_path: tempdir.path().join("staging-private.sqlite"),
    };
    assert!(
        io_message(copy_restore_archive_to_staging(&missing_archive, &staging_paths).await)
            .contains("restore event store copy failed")
    );
    let partial_archive = RestoreArchive {
        event_store_path: staging_paths.event_store_path.clone(),
        ..missing_archive.clone()
    };
    fs::write(&partial_archive.event_store_path, b"not sqlite").expect("partial event store");
    assert!(
        io_message(copy_restore_archive_to_staging(&partial_archive, &staging_paths).await)
            .contains("restore outbox copy failed")
    );
    let corrupt_archive = RestoreArchive {
        event_store_path: tempdir.path().join("corrupt-event-store.sqlite"),
        outbox_path: tempdir.path().join("corrupt-outbox.sqlite"),
        private_store_path: tempdir.path().join("corrupt-private.sqlite"),
        ..missing_archive.clone()
    };
    fs::write(&corrupt_archive.event_store_path, b"not sqlite").expect("corrupt event store");
    fs::write(&corrupt_archive.outbox_path, b"not sqlite").expect("corrupt outbox");
    fs::write(&corrupt_archive.private_store_path, b"not sqlite").expect("corrupt private store");
    assert_event_store_error(
        copy_restore_archive_to_staging(&corrupt_archive, &staging_paths).await,
    );
    assert!(
        invalid_request_message(
            restore_archive_to_destination(&missing_archive, Path::new(""), &staging_paths).await,
        )
        .contains("parent is required")
    );

    let invalid_outbox_member_source = tempdir.path().join("invalid-outbox-member");
    fs::create_dir(&invalid_outbox_member_source).expect("invalid outbox source");
    fs::write(
        invalid_outbox_member_source.join(EVENT_STORE_BACKUP_FILE),
        b"not sqlite",
    )
    .expect("event store member");
    let mut invalid_outbox_manifest = manifest();
    invalid_outbox_manifest.backup_paths.outbox_path = PathBuf::from("../outside.sqlite");
    write_backup_manifest(
        &invalid_outbox_member_source.join(BACKUP_MANIFEST_FILE),
        &invalid_outbox_manifest,
    )
    .expect("invalid outbox manifest");
    assert!(
        invalid_request_message(inspect_restore_archive(invalid_outbox_member_source).await)
            .contains("outbox archive path")
    );

    let protected_parent = tempdir.path().join("protected-parent");
    fs::create_dir(&protected_parent).expect("protected parent");
    let protected_destination = protected_parent.join("restore");
    let protected_paths = RadrootsSdkStoragePaths {
        event_store_path: protected_destination.join(EVENT_STORE_BACKUP_FILE),
        outbox_path: protected_destination.join(OUTBOX_BACKUP_FILE),
        private_store_path: protected_destination.join(PRIVATE_STORE_BACKUP_FILE),
    };
    set_mode(&protected_parent, 0o500);
    let protected_result =
        restore_archive_to_destination(&missing_archive, &protected_destination, &protected_paths)
            .await;
    set_mode(&protected_parent, 0o700);
    assert!(!io_message(protected_result).is_empty());

    let sdk = RadrootsClient::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_001))
        .build()
        .await
        .expect("directory sdk");
    let backup_destination = tempdir.path().join("backup");
    sdk.backup(BackupRequest::new(&backup_destination))
        .await
        .expect("backup");
    let archive = inspect_restore_archive(backup_destination.clone())
        .await
        .expect("archive");
    let public_protected_parent = tempdir.path().join("public-protected-parent");
    fs::create_dir(&public_protected_parent).expect("public protected parent");
    let public_protected_destination = public_protected_parent.join("restore");
    set_mode(&public_protected_parent, 0o500);
    let public_protected_result = RadrootsClient::restore(
        RestoreRequest::new(&backup_destination).with_destination(&public_protected_destination),
    )
    .await;
    set_mode(&public_protected_parent, 0o700);
    assert!(!io_message(public_protected_result).is_empty());
    let missing_outbox_paths = RadrootsSdkStoragePaths {
        event_store_path: archive.event_store_path.clone(),
        outbox_path: tempdir.path().to_path_buf(),
        private_store_path: archive.private_store_path.clone(),
    };
    assert!(verify_backup_paths(&missing_outbox_paths).await.is_err());

    let invalid_destination = tempdir.path().join(nul_path());
    let invalid_paths = RadrootsSdkStoragePaths {
        event_store_path: invalid_destination.join(EVENT_STORE_BACKUP_FILE),
        outbox_path: invalid_destination.join(OUTBOX_BACKUP_FILE),
        private_store_path: invalid_destination.join(PRIVATE_STORE_BACKUP_FILE),
    };
    let invalid_restore_message = io_message(
        restore_archive_to_destination(&archive, &invalid_destination, &invalid_paths).await,
    );
    assert!(!invalid_restore_message.is_empty());

    let existing_destination = tempdir.path().join("existing-restore");
    fs::create_dir(&existing_destination).expect("existing restore");
    fs::write(existing_destination.join("old-file"), b"old").expect("old restore file");
    let existing_paths =
        preflight_restore_destination(&archive.source, &existing_destination, true)
            .expect("existing preflight");
    restore_archive_to_destination(&archive, &existing_destination, &existing_paths)
        .await
        .expect("overwrite existing restore");
    assert!(existing_destination.join(EVENT_STORE_BACKUP_FILE).exists());
    assert!(existing_destination.join(OUTBOX_BACKUP_FILE).exists());

    let mut mismatch_archive = archive.clone();
    mismatch_archive.verification.event_store_events += 1;
    let mismatch_destination = tempdir.path().join("mismatch-restore");
    let mismatch_paths =
        preflight_restore_destination(&mismatch_archive.source, &mismatch_destination, false)
            .expect("mismatch preflight");
    assert!(
        invalid_request_message(
            restore_archive_to_destination(
                &mismatch_archive,
                &mismatch_destination,
                &mismatch_paths,
            )
            .await,
        )
        .contains("does not match manifest")
    );

    let populated_sdk = RadrootsClient::builder()
        .directory_storage(tempdir.path().join("populated-sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_002))
        .build()
        .await
        .expect("populated sdk");
    populated_sdk
        ._event_store
        .ingest_event(radroots_event_store::RadrootsEventIngest::new(
            radroots_events::RadrootsNostrEvent {
                id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                author: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_owned(),
                created_at: 1_700_000_002,
                kind: 1,
                tags: Vec::new(),
                content: "{}".to_owned(),
                sig: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_owned(),
            },
            1_700_000_002_000,
        ))
        .await
        .expect("populated event");
    let populated_backup_destination = tempdir.path().join("populated-backup");
    populated_sdk
        .backup(BackupRequest::new(&populated_backup_destination))
        .await
        .expect("populated backup");
    let populated_archive = inspect_restore_archive(populated_backup_destination)
        .await
        .expect("populated archive");
    assert_ne!(archive.verification, populated_archive.verification);
    let verification_mismatch_destination = tempdir.path().join("verification-mismatch-restore");
    let wrong_destination_paths = RadrootsSdkStoragePaths {
        event_store_path: populated_archive.event_store_path.clone(),
        outbox_path: populated_archive.outbox_path.clone(),
        private_store_path: populated_archive.private_store_path.clone(),
    };
    assert!(
        invalid_request_message(
            restore_archive_to_destination(
                &archive,
                &verification_mismatch_destination,
                &wrong_destination_paths,
            )
            .await,
        )
        .contains("does not match manifest")
    );
    assert!(!verification_mismatch_destination.exists());

    let bad_verify_destination = tempdir.path().join("bad-verify-restore");
    let bad_verify_paths =
        preflight_restore_destination(&archive.source, &bad_verify_destination, false)
            .expect("bad verify preflight");
    let mut mismatched_verify_paths = bad_verify_paths.clone();
    mismatched_verify_paths.event_store_path = bad_verify_destination.clone();
    mismatched_verify_paths.outbox_path = bad_verify_destination.join(OUTBOX_BACKUP_FILE);
    assert!(
            restore_archive_to_destination(
                &archive,
                &bad_verify_destination,
                &mismatched_verify_paths,
            )
            .await
            .is_err()
        );
}
