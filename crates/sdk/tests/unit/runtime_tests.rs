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

fn assert_studio_store_error<T>(result: Result<T, RadrootsSdkError>) {
    match result {
        Err(RadrootsSdkError::StudioStore { .. }) => {}
        Err(other) => panic!("expected studio store error, got {other:?}"),
        Ok(_) => panic!("expected studio store error"),
    }
}

fn assert_unsupported_profile_error<T>(result: Result<T, RadrootsSdkError>) -> PathBuf {
    match result.err().expect("expected unsupported profile error") {
        RadrootsSdkError::UnsupportedProfileSchema { path, .. } => path,
        other => panic!("expected unsupported profile error, got {other:?}"),
    }
}

fn sqlite_status() -> SdkSqliteStoreStatus {
    SdkSqliteStoreStatus {
        schema_version: 1,
        journal_mode: "wal".to_owned(),
        foreign_keys_enabled: true,
        busy_timeout_ms: 5_000,
        wal_status: SdkSqliteWalStatus { wal_enabled: true },
        integrity_ok: true,
        integrity_result: "ok".to_owned(),
    }
}

fn private_sqlite_status() -> SdkSqliteStoreStatus {
    SdkSqliteStoreStatus {
        schema_version: 1,
        ..sqlite_status()
    }
}

fn assert_wal_status_ready(status: &SdkSqliteStoreStatus) {
    assert_eq!(status.journal_mode, "wal");
    assert!(status.wal_status.wal_enabled);
}

fn assert_wal_checkpoint_complete(receipt: &SdkSqliteWalCheckpointReceipt) {
    assert!(receipt.wal_enabled);
    assert_eq!(receipt.busy, 0);
    assert!(receipt.log_frame_count >= 0);
    assert_eq!(receipt.log_frame_count, receipt.checkpointed_frame_count);
    assert!(receipt.checkpoint_complete);
}

fn nostr_profile(
    relays: impl IntoIterator<Item = &'static str>,
    policy: crate::NostrRelayUrlPolicy,
) -> crate::TransportProfile {
    crate::TransportProfile::nostr(crate::NostrProfile::new(relays, policy).expect("Nostr profile"))
}

fn storage_status() -> StorageStatusReceipt {
    StorageStatusReceipt {
        storage: SdkStorageKind::Memory,
        paths: None,
        event_store: SdkEventStoreStorageStatus {
            store: sqlite_status(),
            total_events: 0,
            projection_eligible_events: 0,
            transport_observations: 0,
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
            deferred_until_implemented_events: 0,
            ready_signed_events: 0,
            publishing_events: 0,
            last_attempt_at_ms: None,
            last_error: None,
        },
        private_store: SdkPrivateStoreStorageStatus {
            store: private_sqlite_status(),
            farm_private_locations: 0,
        },
        studio_store: SdkStudioStoreStorageStatus {
            store: sqlite_status(),
            studio_state_records: 0,
        },
    }
}

fn verification(event_store_ok: bool, outbox_ok: bool) -> SdkBackupVerification {
    SdkBackupVerification {
        event_store_ok,
        outbox_ok,
        private_store_ok: true,
        studio_store_ok: true,
        event_store_events: 0,
        outbox_events: 0,
        private_farm_locations: 0,
        studio_state_records: 0,
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
            runtime_path: PathBuf::from(RUNTIME_SQLITE_FILE),
            studio_path: PathBuf::from(STUDIO_SQLITE_FILE),
            private_path: PathBuf::from(PRIVATE_SQLITE_FILE),
        },
        source_status: storage_status(),
        backup_verification: verification(true, true),
    }
}

fn private_farm_location_record() -> crate::private_store::SdkPrivateFarmLocationRecord {
    crate::private_store::SdkPrivateFarmLocationRecord {
        farm_addr: radroots_event::ids::RadrootsAddressableCoordinate::parse(format!(
            "{}:{}:{}",
            radroots_event::kinds::KIND_FARM,
            "a".repeat(64),
            "AAAAAAAAAAAAAAAAAAAAAA"
        ))
        .expect("farm addr"),
        farm_pubkey: "a".repeat(64),
        farm_d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_owned(),
        label: None,
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
    for (latitude, longitude) in [
        (f64::INFINITY, record.longitude),
        (record.latitude, f64::NEG_INFINITY),
        (-90.1, record.longitude),
        (90.1, record.longitude),
        (record.latitude, -180.1),
        (record.latitude, 180.1),
    ] {
        let mut invalid = record.clone();
        invalid.latitude = latitude;
        invalid.longitude = longitude;
        assert!(matches!(
            store.upsert_farm_location(&invalid).await,
            Err(RadrootsSdkError::InvalidRequest { .. })
        ));
    }
    for (latitude, longitude) in [(-90.0, -180.0), (90.0, 180.0)] {
        let mut boundary = record.clone();
        boundary.latitude = latitude;
        boundary.longitude = longitude;
        store
            .upsert_farm_location(&boundary)
            .await
            .expect("boundary coordinates");
    }

    let mut blank_locality = record.clone();
    blank_locality.locality_primary = " ".to_owned();
    assert!(matches!(
        store.upsert_farm_location(&blank_locality).await,
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let mut invalid_geohash = record.clone();
    invalid_geohash.geohash5 = "abcd".to_owned();
    assert!(matches!(
        store.upsert_farm_location(&invalid_geohash).await,
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
    let mut long_geohash = private_farm_location_record();
    long_geohash.geohash5 = "abcdef".to_owned();
    assert!(matches!(
        store.upsert_farm_location(&long_geohash).await,
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    sqlx::query("DROP TABLE private_farm_location")
        .execute(store.pool())
        .await
        .expect("drop private location table");
    assert_private_store_error(store.status_summary().await);
    assert_private_store_error(store.farm_location(&record.farm_addr).await);
    assert_private_store_error(store.upsert_farm_location(&record).await);
}

#[test]
fn transport_profile_defaults_are_explicit() {
    let local = TransportProfile::default();
    assert_eq!(local, TransportProfile::LocalOnly);

    let nostr = nostr_profile(
        ["wss://relay.example.com"],
        crate::NostrRelayUrlPolicy::Public,
    );
    assert_eq!(nostr.transport_profile_id(), "nostr");
}

#[tokio::test]
async fn open_storage_and_storage_kind_cover_memory_directory_and_file_failures() {
    let memory = open_storage(&RadrootsSdkStorageConfig::Memory, 0)
        .await
        .expect("memory storage");
    assert!(memory.paths.is_none());
    let memory_sdk = RadrootsClient {
        _event_store: memory.event_store,
        _outbox: memory.outbox,
        _private_store: memory.private_store,
        _studio_store: memory.studio_store,
        storage_paths: None,
        geonames: None,
        clock: RadrootsSdkClock::Fixed(RadrootsSdkTimestamp::from_unix_seconds(1)),
        transport_profile: TransportProfile::local_only(),
        radrootsd_execution_profile: None,
        #[cfg(feature = "signer-adapters")]
        signer_provider: None,
    };
    assert_eq!(memory_sdk.storage_kind(), SdkStorageKind::Memory);

    let tempdir = tempfile::tempdir().expect("tempdir");
    let directory = tempdir.path().join("sdk");
    let directory_storage = open_storage(&RadrootsSdkStorageConfig::Directory(directory), 0)
        .await
        .expect("directory storage");
    let directory_paths = directory_storage.paths.expect("directory paths");
    assert!(directory_paths.runtime_path.exists());
    assert!(directory_paths.private_path.exists());
    assert!(directory_paths.studio_path.exists());
    let directory_sdk = RadrootsClient {
        _event_store: directory_storage.event_store,
        _outbox: directory_storage.outbox,
        _private_store: directory_storage.private_store,
        _studio_store: directory_storage.studio_store,
        storage_paths: Some(directory_paths),
        geonames: None,
        clock: RadrootsSdkClock::Fixed(RadrootsSdkTimestamp::from_unix_seconds(1)),
        transport_profile: TransportProfile::local_only(),
        radrootsd_execution_profile: None,
        #[cfg(feature = "signer-adapters")]
        signer_provider: None,
    };
    assert_eq!(directory_sdk.storage_kind(), SdkStorageKind::Directory);

    let file_path = tempdir.path().join("not-directory");
    fs::write(&file_path, b"file").expect("file");
    assert!(!io_message(open_directory_storage(&file_path, 0).await).is_empty());

    let event_store_directory = tempdir.path().join("event-store-directory");
    fs::create_dir(&event_store_directory).expect("event store dir");
    fs::create_dir(event_store_directory.join(RUNTIME_SQLITE_FILE))
        .expect("event store file slot dir");
    assert_event_store_error(open_directory_storage(&event_store_directory, 0).await);

    let studio_directory = tempdir.path().join("studio-directory");
    fs::create_dir(&studio_directory).expect("studio dir");
    fs::create_dir(studio_directory.join(STUDIO_SQLITE_FILE)).expect("studio file slot dir");
    assert_studio_store_error(open_directory_storage(&studio_directory, 0).await);

    let private_store_directory = tempdir.path().join("private-store-directory");
    fs::create_dir(&private_store_directory).expect("private store dir");
    fs::create_dir(private_store_directory.join(PRIVATE_SQLITE_FILE))
        .expect("private store file slot dir");
    assert_private_store_error(open_directory_storage(&private_store_directory, 0).await);
}

#[tokio::test]
async fn runtime_schema_refuses_newer_profiles_before_migration() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let profile = tempdir.path().join("sdk");
    fs::create_dir(&profile).expect("profile");
    let runtime_path = profile.join(RUNTIME_SQLITE_FILE);
    let pool = open_runtime_file_pool(&runtime_path)
        .await
        .expect("runtime pool");
    sqlx::query("PRAGMA user_version = 99")
        .execute(&pool)
        .await
        .expect("user version");
    pool.close().await;

    let unsupported = assert_unsupported_profile_error(open_directory_storage(&profile, 10).await);

    assert_eq!(unsupported, runtime_path);
}

#[tokio::test]
async fn runtime_startup_recovery_updates_journal_reservations_projections_and_outbox() {
    let storage = open_storage(&RadrootsSdkStorageConfig::Memory, 0)
        .await
        .expect("memory storage");
    let pool = storage.event_store.pool();
    sqlx::query(
        "INSERT INTO sdk_runtime_operation_journal(contract_version, operation_kind, actor_pubkey, idempotency_key, command_payload_hash, frozen_draft_json, expected_transport_id, state, created_at_ms, updated_at_ms) VALUES ('1', 'trade.proposal.v1', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', '019b0000-0000-7000-8000-000000000001', 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', '{}', 'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc', 'signature_pending', 1, 1)",
    )
    .execute(pool)
    .await
    .expect("operation journal");
    sqlx::query(
        "INSERT INTO sdk_seller_inventory_reservation(reservation_id, farm_id, candidate_id, authority_id, inventory_epoch, assertion_commitment, state, lease_until_ms, created_at_ms, updated_at_ms) VALUES ('reservation-1', 'farm-1', 'candidate-1', 'seller-1', 1, 'commitment-1', 'prepared', 5, 1, 1)",
    )
    .execute(pool)
    .await
    .expect("reservation");
    sqlx::query(
        "INSERT INTO sdk_seller_inventory_reservation_line(reservation_id, farm_id, candidate_id, line_id, bin_id, quantity_mantissa, quantity_scale, unit_code) VALUES ('reservation-1', 'farm-1', 'candidate-1', 'line-1', 'bin-1', '1', 0, 'lb')",
    )
    .execute(pool)
    .await
    .expect("reservation line");
    sqlx::query(
        "INSERT INTO sdk_seller_inventory_reservation(reservation_id, farm_id, candidate_id, authority_id, inventory_epoch, assertion_commitment, state, lease_until_ms, created_at_ms, updated_at_ms) VALUES ('reservation-2', 'farm-1', 'candidate-1', 'seller-1', 1, 'commitment-2', 'prepared', 50, 1, 1)",
    )
    .execute(pool)
    .await
    .expect("second reservation");
    assert!(
        sqlx::query(
            "INSERT INTO sdk_seller_inventory_reservation_line(reservation_id, farm_id, candidate_id, line_id, bin_id, quantity_mantissa, quantity_scale, unit_code) VALUES ('reservation-2', 'farm-1', 'candidate-1', 'line-1', 'bin-1', '1', 0, 'lb')",
        )
        .execute(pool)
        .await
        .is_err()
    );
    sqlx::query(
        "INSERT INTO sdk_runtime_trade_projection_checkpoint(projection_name, reducer_contract_id, reducer_version, last_ingest_seq, source_digest, projection_digest, completeness_state, updated_at_ms) VALUES ('trade_projection', 'radroots.trade.reducer.v1', 1, 7, 'source', 'projection', 'rebuilding', 1)",
    )
    .execute(pool)
    .await
    .expect("projection checkpoint");
    let operation_id = sqlx::query(
        "INSERT INTO outbox_operations(operation_kind, expected_pubkey, semantic_scope, idempotency_key, operation_idempotency_digest, status, created_at_ms, updated_at_ms) VALUES ('listing.publish.v1', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 'generic_event', '019b0000-0000-7000-8000-000000000002', 'digest', 'queued', 1, 1)",
    )
    .execute(pool)
    .await
    .expect("outbox operation")
    .last_insert_rowid();
    sqlx::query(
        "INSERT INTO outbox_event(operation_id, event_id, expected_pubkey, draft_json, state, attempt_count, claim_token, claim_owner, claim_expires_at_ms, next_attempt_after_ms, event_store_ingested, event_store_inserted, created_at_ms, updated_at_ms) VALUES (?, 'dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', '{}', 'publishing', 1, 'claim-1', 'worker-1', 5, 1, 0, 0, 1, 1)",
    )
    .bind(operation_id)
    .execute(pool)
    .await
    .expect("outbox event");

    recover_sdk_runtime_state(pool, 10).await.expect("recover");

    let operation_state: String = sqlx::query_scalar(
        "SELECT state FROM sdk_runtime_operation_journal WHERE operation_kind = 'trade.proposal.v1'",
    )
    .fetch_one(pool)
    .await
    .expect("operation state");
    let reservation_state: String = sqlx::query_scalar(
        "SELECT state FROM sdk_seller_inventory_reservation WHERE reservation_id = 'reservation-1'",
    )
    .fetch_one(pool)
    .await
    .expect("reservation state");
    let projection_state: String = sqlx::query_scalar(
        "SELECT completeness_state FROM sdk_runtime_trade_projection_checkpoint WHERE projection_name = 'trade_projection'",
    )
    .fetch_one(pool)
    .await
    .expect("projection state");
    let outbox_state: String =
        sqlx::query_scalar("SELECT state FROM outbox_event WHERE outbox_event_id = 1")
            .fetch_one(pool)
            .await
            .expect("outbox state");
    let outbox_claim: Option<String> =
        sqlx::query_scalar("SELECT claim_token FROM outbox_event WHERE outbox_event_id = 1")
            .fetch_one(pool)
            .await
            .expect("outbox claim");
    let receipts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sdk_runtime_recovery_receipt WHERE recovery_code IN ('signer_timeout','reservation_expiry','projection_stale','relay_failure')",
    )
    .fetch_one(pool)
    .await
    .expect("recovery receipts");

    assert_eq!(operation_state, "failed_recoverable");
    assert_eq!(reservation_state, "expired");
    assert_eq!(projection_state, "stale");
    assert_eq!(outbox_state, "publish_retryable");
    assert!(outbox_claim.is_none());
    assert_eq!(receipts, 4);
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
        .transport_profile(nostr_profile(
            ["ws://127.0.0.1:7777"],
            crate::NostrRelayUrlPolicy::Localhost,
        ))
        .build()
        .await
        .expect("memory sdk");
    assert_eq!(
        memory_sdk.now().expect("fixed now").unix_seconds(),
        1_700_000_000
    );
    assert_eq!(
        memory_sdk.configured_nostr_relay_urls(),
        ["ws://127.0.0.1:7777"]
    );
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
    assert!(!memory_status.event_store.store.wal_status.wal_enabled);
    assert!(!memory_status.outbox.store.wal_status.wal_enabled);
    assert!(!memory_status.private_store.store.wal_status.wal_enabled);
    let memory_checkpoint = memory_sdk
        .storage_checkpoint(StorageCheckpointRequest::new())
        .await
        .expect("memory checkpoint");
    assert_eq!(memory_checkpoint.storage, SdkStorageKind::Memory);
    assert!(memory_checkpoint.event_store.checkpoint_complete);
    assert!(memory_checkpoint.outbox.checkpoint_complete);
    assert!(memory_checkpoint.private_store.checkpoint_complete);
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
    let directory_status = directory_sdk
        .storage_status(StorageStatusRequest::new())
        .await
        .expect("directory status");
    assert_eq!(directory_status.storage, SdkStorageKind::Directory);
    assert_wal_status_ready(&directory_status.event_store.store);
    assert_wal_status_ready(&directory_status.outbox.store);
    assert_wal_status_ready(&directory_status.private_store.store);
    let directory_checkpoint = directory_sdk
        .storage_checkpoint(StorageCheckpointRequest::new())
        .await
        .expect("directory checkpoint");
    assert_eq!(directory_checkpoint.storage, SdkStorageKind::Directory);
    assert_wal_checkpoint_complete(&directory_checkpoint.event_store);
    assert_wal_checkpoint_complete(&directory_checkpoint.outbox);
    assert_wal_checkpoint_complete(&directory_checkpoint.private_store);

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

#[test]
fn sqlite_wal_checkpoint_receipt_mapping_covers_edge_states() {
    let complete = sqlite_wal_checkpoint_receipt_from_values("wal", 0, 8, 8);
    assert_eq!(
        complete,
        SdkSqliteWalCheckpointReceipt {
            wal_enabled: true,
            busy: 0,
            log_frame_count: 8,
            checkpointed_frame_count: 8,
            checkpoint_complete: true,
        }
    );

    let incomplete = sqlite_wal_checkpoint_receipt_from_values("wal", 0, 8, 7);
    assert!(incomplete.wal_enabled);
    assert!(!incomplete.checkpoint_complete);

    let busy = sqlite_wal_checkpoint_receipt_from_values("wal", 1, 8, 8);
    assert!(busy.wal_enabled);
    assert!(!busy.checkpoint_complete);

    let recovered_or_invalid = sqlite_wal_checkpoint_receipt_from_values("wal", 0, -1, 0);
    assert!(recovered_or_invalid.wal_enabled);
    assert!(!recovered_or_invalid.checkpoint_complete);

    let non_wal_idle = sqlite_wal_checkpoint_receipt_from_values("memory", 0, -1, -1);
    assert!(!non_wal_idle.wal_enabled);
    assert!(non_wal_idle.checkpoint_complete);

    let non_wal_busy = sqlite_wal_checkpoint_receipt_from_values("delete", 1, 0, 0);
    assert!(!non_wal_busy.wal_enabled);
    assert!(!non_wal_busy.checkpoint_complete);
}

#[tokio::test]
async fn storage_status_inspection_is_read_only_and_never_creates_missing_profiles() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let missing = tempdir.path().join("missing-profile");
    assert_event_store_error(
        RadrootsClient::inspect_storage_status(&missing, StorageStatusRequest::new()).await,
    );
    assert!(!missing.exists());

    let pre_v1_profile = tempdir.path().join("pre-v1");
    fs::create_dir(&pre_v1_profile).expect("pre-v1 profile");
    fs::write(pre_v1_profile.join("event_store.sqlite"), b"old runtime").expect("old event store");
    let unsupported = assert_unsupported_profile_error(
        RadrootsClient::inspect_storage_status(&pre_v1_profile, StorageStatusRequest::new()).await,
    );
    assert_eq!(unsupported, pre_v1_profile.join("event_store.sqlite"));
    assert!(!pre_v1_profile.join(RUNTIME_SQLITE_FILE).exists());
    assert!(!pre_v1_profile.join(PRIVATE_SQLITE_FILE).exists());
    assert!(!pre_v1_profile.join(STUDIO_SQLITE_FILE).exists());
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
        Err(RadrootsSdkError::EventStore { .. })
    ));
    assert_outbox_error(outbox_sqlite_status(&outbox_closed._outbox).await);
    assert_outbox_error(outbox_status_summary(&outbox_closed._outbox, 1).await);
    assert!(matches!(
        outbox_closed.integrity(IntegrityRequest::new()).await,
        Err(RadrootsSdkError::EventStore { .. })
    ));

    let private_store_closed = RadrootsClient::builder().build().await.expect("sdk");
    private_store_closed._private_store.pool().close().await;
    assert!(matches!(
        private_store_closed
            .storage_status(StorageStatusRequest::new())
            .await,
        Err(RadrootsSdkError::PrivateStore { .. })
    ));
    assert!(matches!(
        private_store_closed
            .integrity(IntegrityRequest::new())
            .await,
        Err(RadrootsSdkError::PrivateStore { .. })
    ));
    assert_private_store_error(
        private_store_closed
            ._private_store
            .pragma_foreign_keys()
            .await,
    );
    assert_private_store_error(
        private_store_closed
            ._private_store
            .pragma_busy_timeout()
            .await,
    );
    assert_private_store_error(
        private_store_closed
            ._private_store
            .pragma_journal_mode()
            .await,
    );
    assert_private_store_error(
        private_store_sqlite_status(&private_store_closed._private_store).await,
    );
    assert_private_store_error(
        sqlite_store_status(
            private_store_closed._private_store.pool(),
            SDK_PRIVATE_STORE_SCHEMA_VERSION_CURRENT,
            "memory".to_owned(),
            true,
            5_000,
            SqliteStoreRole::PrivateStore,
        )
        .await,
    );
    assert_private_store_error(private_store_closed._private_store.status_summary().await);
    let record = private_farm_location_record();
    assert_private_store_error(
        private_store_closed
            ._private_store
            .upsert_farm_location(&record)
            .await,
    );
    assert_private_store_error(
        private_store_closed
            ._private_store
            .farm_location(&record.farm_addr)
            .await,
    );

    let event_store_summary_error = RadrootsClient::builder().build().await.expect("sdk");
    sqlx::query("DROP TABLE event_envelopes")
        .execute(event_store_summary_error._event_store.pool())
        .await
        .expect("drop event envelopes");
    assert!(matches!(
        event_store_summary_error
            .storage_status(StorageStatusRequest::new())
            .await,
        Err(RadrootsSdkError::EventStore { .. })
    ));
    assert_event_store_error(
        event_store_status_summary(&event_store_summary_error._event_store).await,
    );

    let outbox_summary_error = RadrootsClient::builder().build().await.expect("sdk");
    sqlx::query("DROP TABLE outbox_event")
        .execute(outbox_summary_error._outbox.pool())
        .await
        .expect("drop outbox event");
    assert!(matches!(
        outbox_summary_error
            .storage_status(StorageStatusRequest::new())
            .await,
        Err(RadrootsSdkError::Outbox { .. })
    ));
    assert_outbox_error(outbox_status_summary(&outbox_summary_error._outbox, 1).await);

    let private_summary_error = RadrootsClient::builder().build().await.expect("sdk");
    sqlx::query("DROP TABLE private_farm_location")
        .execute(private_summary_error._private_store.pool())
        .await
        .expect("drop private location");
    assert!(matches!(
        private_summary_error
            .storage_status(StorageStatusRequest::new())
            .await,
        Err(RadrootsSdkError::PrivateStore { .. })
    ));
    assert_private_store_error(private_summary_error._private_store.status_summary().await);

    let event_store = RadrootsEventStore::open_memory()
        .await
        .expect("event store");
    let private_store = SdkPrivateStore::open_memory().await.expect("private store");
    let studio_store = SdkStudioStore::open_memory().await.expect("studio store");
    private_store.pool().close().await;
    let tempdir = tempfile::tempdir().expect("tempdir");
    assert_private_store_error(
        backup_sqlite_stores(
            event_store.pool(),
            private_store.pool(),
            studio_store.pool(),
            &RadrootsSdkStoragePaths {
                runtime_path: tempdir.path().join(RUNTIME_SQLITE_FILE),
                studio_path: tempdir.path().join(STUDIO_SQLITE_FILE),
                private_path: tempdir.path().join(PRIVATE_SQLITE_FILE),
            },
        )
        .await,
    );
}

#[tokio::test]
async fn verify_backup_paths_reports_each_store_member_failure() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let source_sdk = RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .build()
        .await
        .expect("sdk");
    let backup_destination = tempdir.path().join("backup");
    source_sdk
        .backup(BackupRequest::new(&backup_destination))
        .await
        .expect("backup");

    let bad_runtime_path = backup_destination.join("bad-event-store.sqlite");
    fs::create_dir(&bad_runtime_path).expect("bad event store dir");
    let bad_studio_path = backup_destination.join("bad-studio.sqlite");
    fs::create_dir(&bad_studio_path).expect("bad studio dir");
    let bad_private_path = backup_destination.join("bad-private.sqlite");
    fs::create_dir(&bad_private_path).expect("bad private store dir");

    let mut invalid_member = RadrootsSdkStoragePaths {
        runtime_path: bad_runtime_path,
        studio_path: backup_destination.join(STUDIO_SQLITE_FILE),
        private_path: backup_destination.join(PRIVATE_SQLITE_FILE),
    };
    assert_event_store_error(verify_backup_paths(&invalid_member).await);

    invalid_member.runtime_path = backup_destination.join(RUNTIME_SQLITE_FILE);
    invalid_member.studio_path = bad_studio_path;
    assert_studio_store_error(verify_backup_paths(&invalid_member).await);

    invalid_member.studio_path = backup_destination.join(STUDIO_SQLITE_FILE);
    invalid_member.private_path = bad_private_path;
    assert_private_store_error(verify_backup_paths(&invalid_member).await);
}

#[test]
fn restore_archive_path_validators_cover_missing_outside_and_manifest_edges() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let source = tempdir.path().join("source");
    fs::create_dir(&source).expect("source");
    let source_root = canonical_restore_directory(&source).expect("canonical source");
    let file_member = source.join(RUNTIME_SQLITE_FILE);
    fs::write(&file_member, b"sqlite").expect("file member");
    let outside_file = tempdir.path().join("outside.sqlite");
    fs::write(&outside_file, b"sqlite").expect("outside file");
    let dir_member = source.join("dir-member");
    fs::create_dir(&dir_member).expect("dir member");

    assert!(validate_relative_archive_path(Path::new(RUNTIME_SQLITE_FILE), "event store").is_ok());
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
        restore_archive_member_path(&source_root, Path::new(RUNTIME_SQLITE_FILE), "event store",)
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
        paths.runtime_path,
        new_destination.join(RUNTIME_SQLITE_FILE)
    );
    let nested_new_destination = parent.join("nested").join("new-destination");
    fs::create_dir(nested_new_destination.parent().expect("nested parent")).expect("nested parent");
    let nested_paths = preflight_restore_destination(&source, &nested_new_destination, false)
        .expect("nested new preflight");
    assert_eq!(
        nested_paths.private_path,
        nested_new_destination.join(PRIVATE_SQLITE_FILE)
    );
    let relative_destination = PathBuf::from(format!(
        "relative-restore-{}",
        system_time_nanos_since_unix_epoch(SystemTime::now()).expect("time")
    ));
    let relative_paths = preflight_restore_destination(&source, &relative_destination, false)
        .expect("relative preflight");
    assert_eq!(
        relative_paths.runtime_path,
        relative_destination.join(RUNTIME_SQLITE_FILE)
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
        let symlink_destination = parent.join("symlink-destination");
        std::os::unix::fs::symlink(&empty_directory, &symlink_destination).expect("symlink");
        assert!(
            invalid_request_message(preflight_restore_destination(
                &source,
                &symlink_destination,
                true,
            ))
            .contains("symbolic link")
        );

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
            "runtime store",
        ))
        .contains("restore runtime store copy failed")
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

#[test]
fn restore_staging_helpers_cover_new_and_existing_destination_installs() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let new_staging = tempdir.path().join("new-staging");
    let new_destination = tempdir.path().join("new-destination");
    let new_previous = tempdir.path().join("new-previous");
    fs::create_dir(&new_staging).expect("new staging");

    let sidecar =
        unique_restore_sidecar_path(tempdir.path(), &new_destination, "staging").expect("sidecar");
    assert_eq!(sidecar.parent(), Some(tempdir.path()));
    assert!(
        sidecar
            .file_name()
            .expect("sidecar name")
            .to_string_lossy()
            .contains("new-destination")
    );
    assert!(
        !install_restore_staging(&new_staging, &new_destination, &new_previous)
            .expect("install new staging")
    );
    assert!(new_destination.is_dir());
    assert!(!new_previous.exists());

    let existing_staging = tempdir.path().join("existing-staging");
    let existing_destination = tempdir.path().join("existing-destination");
    let existing_previous = tempdir.path().join("existing-previous");
    fs::create_dir(&existing_staging).expect("existing staging");
    fs::create_dir(&existing_destination).expect("existing destination");
    fs::write(existing_destination.join("old"), b"old").expect("old destination member");

    assert!(
        install_restore_staging(&existing_staging, &existing_destination, &existing_previous,)
            .expect("replace existing staging")
    );
    assert!(existing_destination.is_dir());
    assert!(existing_previous.join("old").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn sqlite_backup_errors_cover_invalid_paths_and_execute_failures() {
    let storage = open_storage(&RadrootsSdkStorageConfig::Memory, 0)
        .await
        .expect("memory storage");

    assert!(
        invalid_request_message(
            sqlite_vacuum_into(
                storage.event_store.pool(),
                &non_utf8_path(),
                SqliteStoreRole::EventStore,
            )
            .await
        )
        .contains("valid UTF-8")
    );

    storage.event_store.pool().close().await;
    let tempdir = tempfile::tempdir().expect("tempdir");
    let closed_pool_destination = tempdir.path().join("closed-pool.sqlite");
    let error = sqlite_vacuum_into(
        storage.event_store.pool(),
        &closed_pool_destination,
        SqliteStoreRole::EventStore,
    )
    .await
    .expect_err("sqlite error");
    assert!(matches!(
        error,
        RadrootsSdkError::EventStore { message } if message.contains("backup failed")
    ));
    let backup_paths = RadrootsSdkStoragePaths {
        runtime_path: tempdir.path().join("closed-event-store-backup.sqlite"),
        studio_path: tempdir.path().join("closed-event-store-outbox.sqlite"),
        private_path: tempdir.path().join("closed-event-store-private.sqlite"),
    };
    assert_event_store_error(
        backup_sqlite_stores(
            storage.event_store.pool(),
            storage.private_store.pool(),
            storage.studio_store.pool(),
            &backup_paths,
        )
        .await,
    );

    let studio_closed_storage = open_storage(&RadrootsSdkStorageConfig::Memory, 0)
        .await
        .expect("studio closed storage");
    studio_closed_storage.studio_store.pool().close().await;
    let studio_closed_paths = RadrootsSdkStoragePaths {
        runtime_path: tempdir.path().join("open-runtime-backup.sqlite"),
        studio_path: tempdir.path().join("closed-studio-backup.sqlite"),
        private_path: tempdir.path().join("studio-closed-private.sqlite"),
    };
    assert_studio_store_error(
        backup_sqlite_stores(
            studio_closed_storage.event_store.pool(),
            studio_closed_storage.private_store.pool(),
            studio_closed_storage.studio_store.pool(),
            &studio_closed_paths,
        )
        .await,
    );
    assert!(
        !io_message(write_backup_receipt(
            tempdir.path().join("receipt-destination"),
            RadrootsSdkStoragePaths {
                runtime_path: tempdir.path().join(RUNTIME_SQLITE_FILE),
                studio_path: tempdir.path().join(STUDIO_SQLITE_FILE),
                private_path: tempdir.path().join(PRIVATE_SQLITE_FILE),
            },
            tempdir.path().to_path_buf(),
            manifest(),
        ))
        .is_empty()
    );

    let integrity_error =
        sqlite_integrity_result(storage.event_store.pool(), SqliteStoreRole::EventStore)
            .await
            .err()
            .expect("integrity error");
    assert!(matches!(
        integrity_error,
        RadrootsSdkError::EventStore { .. }
    ));
    assert_event_store_error(
        sqlite_store_status(
            storage.event_store.pool(),
            1,
            "wal".to_owned(),
            true,
            5_000,
            SqliteStoreRole::EventStore,
        )
        .await,
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
        runtime_path: tempdir.path().join("missing-event-store.sqlite"),
        studio_path: tempdir.path().join("missing-outbox.sqlite"),
        private_path: tempdir.path().join("missing-private.sqlite"),
        manifest_path: tempdir.path().join(BACKUP_MANIFEST_FILE),
        manifest: manifest(),
        verification: verification(true, true),
    };
    let staging_paths = RadrootsSdkStoragePaths {
        runtime_path: tempdir.path().join("staging-event-store.sqlite"),
        studio_path: tempdir.path().join("staging-outbox.sqlite"),
        private_path: tempdir.path().join("staging-private.sqlite"),
    };
    assert!(
        io_message(copy_restore_archive_to_staging(&missing_archive, &staging_paths).await)
            .contains("restore runtime store copy failed")
    );
    let partial_archive = RestoreArchive {
        runtime_path: staging_paths.runtime_path.clone(),
        ..missing_archive.clone()
    };
    fs::write(&partial_archive.runtime_path, b"not sqlite").expect("partial event store");
    assert!(
        io_message(copy_restore_archive_to_staging(&partial_archive, &staging_paths).await)
            .contains("restore studio copy failed")
    );
    let private_partial_archive = RestoreArchive {
        runtime_path: tempdir.path().join("private-partial-event-store.sqlite"),
        studio_path: tempdir.path().join("private-partial-outbox.sqlite"),
        ..missing_archive.clone()
    };
    fs::write(&private_partial_archive.runtime_path, b"runtime").expect("private partial runtime");
    fs::write(&private_partial_archive.studio_path, b"studio").expect("private partial studio");
    let private_staging_paths = RadrootsSdkStoragePaths {
        runtime_path: tempdir.path().join("private-staging-event-store.sqlite"),
        studio_path: tempdir.path().join("private-staging-outbox.sqlite"),
        private_path: tempdir.path().join("private-staging-missing.sqlite"),
    };
    assert!(
        io_message(
            copy_restore_archive_to_staging(&private_partial_archive, &private_staging_paths,)
                .await
        )
        .contains("restore private store copy failed")
    );
    let corrupt_archive = RestoreArchive {
        runtime_path: tempdir.path().join("corrupt-event-store.sqlite"),
        studio_path: tempdir.path().join("corrupt-outbox.sqlite"),
        private_path: tempdir.path().join("corrupt-private.sqlite"),
        ..missing_archive.clone()
    };
    fs::write(&corrupt_archive.runtime_path, b"not sqlite").expect("corrupt runtime");
    fs::write(&corrupt_archive.studio_path, b"not sqlite").expect("corrupt studio");
    fs::write(&corrupt_archive.private_path, b"not sqlite").expect("corrupt private store");
    assert_event_store_error(
        copy_restore_archive_to_staging(&corrupt_archive, &staging_paths).await,
    );
    assert!(
        invalid_request_message(
            restore_archive_to_destination(&missing_archive, Path::new(""), &staging_paths).await,
        )
        .contains("parent is required")
    );

    let invalid_studio_member_source = tempdir.path().join("invalid-studio-member");
    fs::create_dir(&invalid_studio_member_source).expect("invalid studio source");
    fs::write(
        invalid_studio_member_source.join(RUNTIME_SQLITE_FILE),
        b"not sqlite",
    )
    .expect("runtime member");
    let mut invalid_studio_manifest = manifest();
    invalid_studio_manifest.backup_paths.studio_path = PathBuf::from("../outside.sqlite");
    write_backup_manifest(
        &invalid_studio_member_source.join(BACKUP_MANIFEST_FILE),
        &invalid_studio_manifest,
    )
    .expect("invalid studio manifest");
    assert!(
        invalid_request_message(inspect_restore_archive(invalid_studio_member_source).await)
            .contains("studio archive path")
    );
    let invalid_private_member_source = tempdir.path().join("invalid-private-member");
    fs::create_dir(&invalid_private_member_source).expect("invalid private source");
    fs::write(
        invalid_private_member_source.join(RUNTIME_SQLITE_FILE),
        b"not sqlite",
    )
    .expect("invalid private runtime member");
    fs::write(
        invalid_private_member_source.join(STUDIO_SQLITE_FILE),
        b"not sqlite",
    )
    .expect("invalid private studio member");
    let mut invalid_private_manifest = manifest();
    invalid_private_manifest.backup_paths.private_path = PathBuf::from("../outside.sqlite");
    write_backup_manifest(
        &invalid_private_member_source.join(BACKUP_MANIFEST_FILE),
        &invalid_private_manifest,
    )
    .expect("invalid private manifest");
    assert!(
        invalid_request_message(inspect_restore_archive(invalid_private_member_source).await)
            .contains("private store archive path")
    );

    let protected_parent = tempdir.path().join("protected-parent");
    fs::create_dir(&protected_parent).expect("protected parent");
    let protected_destination = protected_parent.join("restore");
    let protected_paths = RadrootsSdkStoragePaths {
        runtime_path: protected_destination.join(RUNTIME_SQLITE_FILE),
        studio_path: protected_destination.join(STUDIO_SQLITE_FILE),
        private_path: protected_destination.join(PRIVATE_SQLITE_FILE),
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
    let missing_studio_paths = RadrootsSdkStoragePaths {
        runtime_path: archive.runtime_path.clone(),
        studio_path: tempdir.path().to_path_buf(),
        private_path: archive.private_path.clone(),
    };
    assert!(verify_backup_paths(&missing_studio_paths).await.is_err());

    let invalid_destination = tempdir.path().join(nul_path());
    let invalid_paths = RadrootsSdkStoragePaths {
        runtime_path: invalid_destination.join(RUNTIME_SQLITE_FILE),
        studio_path: invalid_destination.join(STUDIO_SQLITE_FILE),
        private_path: invalid_destination.join(PRIVATE_SQLITE_FILE),
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
    assert!(existing_destination.join(RUNTIME_SQLITE_FILE).exists());
    assert!(existing_destination.join(STUDIO_SQLITE_FILE).exists());
    assert!(existing_destination.join(PRIVATE_SQLITE_FILE).exists());

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
    let populated_event_pubkey = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let populated_event_tags = Vec::<Vec<String>>::new();
    let populated_event_content = "{}".to_owned();
    let populated_event_id = radroots_event::wire::compute_canonical_nip01_event_id(
        populated_event_pubkey,
        1_700_000_002,
        1,
        &populated_event_tags,
        &populated_event_content,
    )
    .expect("canonical event id");
    let populated_event_wire = radroots_event::wire::RadrootsNip01EventWire {
        id: populated_event_id.into_string(),
        pubkey: populated_event_pubkey.to_owned(),
        created_at: 1_700_000_002,
        kind: 1,
        tags: populated_event_tags,
        content: populated_event_content,
        sig: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_owned(),
        extra: Default::default(),
    };
    let populated_event_raw_json =
        serde_json::to_string(&populated_event_wire).expect("raw event json");
    let populated_event = radroots_event::draft::RadrootsSignedEvent::from_wire_verified_id(
        populated_event_wire,
        populated_event_raw_json,
    )
    .expect("signed event");
    populated_sdk
        ._event_store
        .ingest_event(radroots_event_store::RadrootsEventIngest::new(
            populated_event,
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
        runtime_path: populated_archive.runtime_path.clone(),
        studio_path: populated_archive.studio_path.clone(),
        private_path: populated_archive.private_path.clone(),
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
    mismatched_verify_paths.runtime_path = bad_verify_destination.clone();
    mismatched_verify_paths.studio_path = bad_verify_destination.join(STUDIO_SQLITE_FILE);
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
