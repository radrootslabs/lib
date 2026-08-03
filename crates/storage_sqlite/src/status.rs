//! Passive SQLite storage status and explicit close lifecycle.

use std::{
    sync::{
        Mutex, RwLock,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};

use radroots_storage::{
    Error,
    outbox::BoxFuture,
    status::{
        IntegrityStatus, ShutdownState, StorageBackend, StorageOpenMode, StorageStatus,
        StorageStatusProvider, WriterPolicy,
    },
};

use crate::{OpenMode, SqliteStorage, integrity, lock::WriterLock};

const OPEN: u8 = 0;
const CLOSING: u8 = 1;
const CLOSED: u8 = 2;
const RESTORING: u8 = 3;

pub(crate) struct StorageLifecycle {
    open_mode: StorageOpenMode,
    writer_policy: WriterPolicy,
    wal_enabled: bool,
    busy_timeout: Duration,
    shutdown: AtomicU8,
    integrity: RwLock<Option<IntegrityStatus>>,
    writer_lock: Mutex<Option<WriterLock>>,
}

impl StorageLifecycle {
    pub(crate) fn new(
        mode: OpenMode,
        busy_timeout: Duration,
        writer_lock: Option<WriterLock>,
    ) -> Self {
        Self {
            open_mode: storage_open_mode(mode),
            writer_policy: if mode.is_writable() {
                WriterPolicy::AdvisoryProcessLock
            } else {
                WriterPolicy::NoWriter
            },
            wal_enabled: mode.is_writable(),
            busy_timeout,
            shutdown: AtomicU8::new(OPEN),
            integrity: RwLock::new(None),
            writer_lock: Mutex::new(writer_lock),
        }
    }

    pub(crate) fn scaffold(mode: radroots_storage::status::EventStoreMode) -> Self {
        let open_mode = match mode {
            radroots_storage::status::EventStoreMode::ReadOnly => OpenMode::ReadOnly,
            radroots_storage::status::EventStoreMode::ReadWrite => OpenMode::Create,
        };
        Self::new(open_mode, Duration::from_secs(5), None)
    }

    pub(crate) fn integrity(&self) -> Result<IntegrityStatus, Error> {
        self.integrity
            .read()
            .map_err(|_| Error::BackendUnavailable)?
            .map_or_else(integrity::unknown, Ok)
    }

    pub(crate) fn require_open(&self) -> Result<(), Error> {
        if self.shutdown.load(Ordering::Acquire) == OPEN {
            Ok(())
        } else {
            Err(Error::BackendUnavailable)
        }
    }

    pub(crate) fn record_integrity(
        &self,
        status: IntegrityStatus,
    ) -> Result<IntegrityStatus, Error> {
        let mut recorded = self
            .integrity
            .write()
            .map_err(|_| Error::BackendUnavailable)?;
        if let Some(previous) = *recorded {
            let previous_time = previous
                .checked_at_unix_ms()
                .ok_or(Error::InvalidIntegrityStatus)?;
            let candidate_time = status
                .checked_at_unix_ms()
                .ok_or(Error::InvalidIntegrityStatus)?;
            if candidate_time < previous_time
                || (candidate_time == previous_time && status != previous)
            {
                return Err(Error::InvalidIntegrityStatus);
            }
        }
        *recorded = Some(status);
        Ok(status)
    }

    fn shutdown(&self) -> ShutdownState {
        match self.shutdown.load(Ordering::Acquire) {
            OPEN => ShutdownState::Open,
            CLOSING | RESTORING => ShutdownState::Closing,
            _ => ShutdownState::Closed,
        }
    }

    pub(crate) fn begin_close(&self) {
        let _ = self
            .shutdown
            .compare_exchange(OPEN, CLOSING, Ordering::AcqRel, Ordering::Acquire);
    }

    pub(crate) fn begin_restore_close(&self) -> Result<(), Error> {
        self.shutdown
            .compare_exchange(OPEN, RESTORING, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| Error::BackendUnavailable)
    }

    pub(crate) fn finish_close(&self) -> Result<(), Error> {
        if self.shutdown.load(Ordering::Acquire) == RESTORING {
            return Ok(());
        }
        self.release_writer_and_close()
    }

    pub(crate) fn finish_restore_close(&self) -> Result<(), Error> {
        if self.shutdown.load(Ordering::Acquire) != RESTORING {
            return Err(Error::BackendUnavailable);
        }
        self.release_writer_and_close()
    }

    fn release_writer_and_close(&self) -> Result<(), Error> {
        let mut writer_lock = self
            .writer_lock
            .lock()
            .map_err(|_| Error::BackendUnavailable)?;
        let release_result = writer_lock
            .take()
            .map(WriterLock::release)
            .transpose()
            .map(|_| ())
            .map_err(|_| Error::BackendUnavailable);
        self.shutdown.store(CLOSED, Ordering::Release);
        release_result
    }

    fn status(&self) -> Result<StorageStatus, Error> {
        StorageStatus::new(
            StorageBackend::Sqlite,
            self.open_mode,
            self.writer_policy,
            self.shutdown(),
            self.integrity()?,
            self.wal_enabled,
            u32::try_from(self.busy_timeout.as_millis())
                .map_err(|_| Error::InvalidStorageStatus)?,
        )
    }
}

impl SqliteStorage {
    /// Returns backend-level status without opening a connection or initiating
    /// integrity checks, checkpoints, migrations, or other maintenance.
    pub async fn storage_status(&self) -> Result<StorageStatus, Error> {
        self.lifecycle.status()
    }

    /// Closes both pools, releases writable authority, and returns final
    /// passive status. Repeated and concurrent calls are idempotent.
    pub async fn close(&self) -> Result<StorageStatus, Error> {
        self.lifecycle.begin_close();
        self.pool.close().await;
        self.private_pool.close().await;
        self.lifecycle.finish_close()?;
        self.lifecycle.status()
    }
}

impl StorageStatusProvider for SqliteStorage {
    fn storage_status(&self) -> BoxFuture<'_, Result<StorageStatus, Error>> {
        Box::pin(async move { SqliteStorage::storage_status(self).await })
    }
}

const fn storage_open_mode(mode: OpenMode) -> StorageOpenMode {
    match mode {
        OpenMode::ReadOnly => StorageOpenMode::ReadOnly,
        OpenMode::ReadWriteExisting => StorageOpenMode::ReadWriteExisting,
        OpenMode::Create => StorageOpenMode::Create,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use radroots_storage::{
        EventStore,
        event::SourceGeneration,
        status::{IntegrityHealth, ShutdownState, StorageBackend, StorageOpenMode, WriterPolicy},
    };

    use crate::{OpenOptions, Paths};

    use super::*;

    fn generation(byte: u8) -> SourceGeneration {
        SourceGeneration::new([byte; 32]).expect("source generation")
    }

    async fn create(directory: &std::path::Path) -> (Paths, SqliteStorage) {
        let paths = Paths::from_directory(directory).expect("owned paths");
        let store = SqliteStorage::open(
            OpenOptions::new(paths.clone(), OpenMode::Create)
                .with_busy_timeout(Duration::from_millis(250))
                .expect("busy timeout")
                .with_source_generation(generation(73), 7_300)
                .expect("source generation"),
        )
        .await
        .expect("create storage");
        (paths, store)
    }

    #[tokio::test]
    async fn status_and_integrity_are_passive_and_report_governed_configuration() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let (paths, store) = create(directory.path()).await;

        let integrity = store.integrity().await.expect("integrity status");
        assert_eq!(integrity.health(), IntegrityHealth::Unknown);
        assert_eq!(integrity.checked_at_unix_ms(), None);
        assert_eq!(integrity.verified_members(), 0);
        assert_eq!(integrity.failed_members(), 0);

        let status = store.storage_status().await.expect("storage status");
        assert_eq!(status.backend(), StorageBackend::Sqlite);
        assert_eq!(status.open_mode(), StorageOpenMode::Create);
        assert_eq!(status.writer_policy(), WriterPolicy::AdvisoryProcessLock);
        assert_eq!(status.shutdown(), ShutdownState::Open);
        assert_eq!(status.integrity(), integrity);
        assert!(status.wal_enabled());
        assert_eq!(status.busy_timeout_ms(), 250);

        let reader = SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadOnly))
            .await
            .expect("read-only storage");
        let reader_status = reader.storage_status().await.expect("reader status");
        assert_eq!(reader_status.open_mode(), StorageOpenMode::ReadOnly);
        assert_eq!(reader_status.writer_policy(), WriterPolicy::NoWriter);
        assert!(!reader_status.wal_enabled());
        assert_eq!(reader_status.busy_timeout_ms(), 5_000);
    }

    #[tokio::test]
    async fn close_is_observable_shared_idempotent_and_releases_writable_authority() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let (paths, store) = create(directory.path()).await;
        let clone = store.clone();
        let held_connection = store.pool.acquire().await.expect("held connection");
        let mut close = Box::pin(clone.close());

        tokio::select! {
            biased;
            result = &mut close => panic!("close completed before the checked-out connection was returned: {result:?}"),
            () = tokio::task::yield_now() => {}
        }
        assert_eq!(
            store
                .storage_status()
                .await
                .expect("closing status")
                .shutdown(),
            ShutdownState::Closing
        );

        drop(held_connection);
        assert_eq!(
            close.await.expect("first close").shutdown(),
            ShutdownState::Closed
        );
        assert_eq!(
            store
                .storage_status()
                .await
                .expect("shared closed status")
                .shutdown(),
            ShutdownState::Closed
        );
        assert_eq!(
            store.close().await.expect("idempotent close").shutdown(),
            ShutdownState::Closed
        );
        assert_eq!(
            EventStore::status(&store).await,
            Err(Error::BackendUnavailable)
        );

        let reopened = SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadWriteExisting))
            .await
            .expect("writer authority released before final clone drop");
        assert_eq!(
            reopened
                .storage_status()
                .await
                .expect("reopened status")
                .shutdown(),
            ShutdownState::Open
        );
        let reopened_clone = reopened.clone();
        let (first, second) = tokio::join!(reopened.close(), reopened_clone.close());
        assert_eq!(
            first.expect("concurrent close one").shutdown(),
            ShutdownState::Closed
        );
        assert_eq!(
            second.expect("concurrent close two").shutdown(),
            ShutdownState::Closed
        );
    }
}
