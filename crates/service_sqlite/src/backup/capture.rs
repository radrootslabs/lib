//! Cancellation-aware online capture into a caller-owned staging directory.

use core::fmt;
use std::{
    error::Error,
    ffi::{OsStr, OsString},
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    os::unix::ffi::OsStrExt,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

#[cfg(test)]
use core::sync::atomic::AtomicU8;

use rusqlite::{Connection, OpenFlags, OptionalExtension, backup::StepResult, types::ValueRef};
use rustix::{
    fs::{AtFlags, FileType, Mode, OFlags, fchmod, fstat, mkdirat, open, openat, statat, unlinkat},
    process::geteuid,
};
use sha2::{Digest, Sha256};
use sqlx::{Sqlite, pool::PoolConnection};

use crate::{
    BackupCreatedAtUnixMs, BackupMemberSha256, OpenMode, ServiceBackupManifest,
    ServiceDatabaseMetadata, ServiceSqliteError, ServiceSqliteErrorKind,
    open::{BackupSourceValidator, PrivateConnectionPool},
};

const MAX_STAGING_PATH_BYTES: usize = 4_096;
const BACKUP_PAGES_PER_STEP: i32 = 64;
const HASH_BUFFER_BYTES: usize = 16 * 1_024;
const MAX_ID_UTF8_BYTES: i64 = 128;
const MAX_INTEGRITY_RESULT_UTF8_BYTES: usize = 64;
const STATE_FILE_NAME: &str = radroots_runtime_paths::SERVICE_STATE_DATABASE_FILE_NAME;
const KNOWN_SIDECARS: [&str; 3] = [
    "state.sqlite-wal",
    "state.sqlite-shm",
    "state.sqlite-journal",
];

pub(crate) const TEST_CAPTURE_PHASE_BEFORE_CREATE: u8 = 1;
pub(crate) const TEST_CAPTURE_PHASE_STAGING_CREATED: u8 = 2;
pub(crate) const TEST_CAPTURE_PHASE_BACKUP_STEPPED: u8 = 3;
pub(crate) const TEST_CAPTURE_PHASE_POST_COPY: u8 = 4;
pub(crate) const TEST_CAPTURE_PHASE_PRE_FINAL_SYNC: u8 = 5;
pub(crate) const TEST_CAPTURE_PHASE_METADATA_AWAITED: u8 = 10;
pub(crate) const TEST_CAPTURE_PHASE_JOIN_AWAITED: u8 = 11;
#[cfg(test)]
static TEST_CAPTURE_PHASE: AtomicU8 = AtomicU8::new(0);
#[cfg(test)]
static TEST_CAPTURE_BLOCK_PHASE: AtomicU8 = AtomicU8::new(0);
#[cfg(test)]
static TEST_CAPTURE_INJECT_METADATA_FAILURE: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static TEST_CAPTURE_PANIC_WORKER: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
pub(crate) fn test_capture_phase() -> u8 {
    TEST_CAPTURE_PHASE.load(Ordering::Acquire)
}

#[cfg(test)]
pub(crate) fn test_capture_block_phase(phase: u8) {
    TEST_CAPTURE_BLOCK_PHASE.store(phase, Ordering::Release);
}

#[cfg(test)]
pub(crate) fn test_capture_reset() {
    TEST_CAPTURE_BLOCK_PHASE.store(0, Ordering::Release);
    TEST_CAPTURE_PHASE.store(0, Ordering::Release);
    TEST_CAPTURE_INJECT_METADATA_FAILURE.store(false, Ordering::Release);
    TEST_CAPTURE_PANIC_WORKER.store(false, Ordering::Release);
}

#[cfg(test)]
pub(crate) fn test_capture_inject_metadata_failure(enabled: bool) {
    TEST_CAPTURE_INJECT_METADATA_FAILURE.store(enabled, Ordering::Release);
}

#[cfg(test)]
pub(crate) fn test_capture_panic_worker(enabled: bool) {
    TEST_CAPTURE_PANIC_WORKER.store(enabled, Ordering::Release);
}

async fn test_async_phase(phase: u8) {
    #[cfg(test)]
    {
        TEST_CAPTURE_PHASE.store(phase, Ordering::Release);
        while TEST_CAPTURE_BLOCK_PHASE.load(Ordering::Acquire) == phase {
            tokio::task::yield_now().await;
        }
    }
    #[cfg(not(test))]
    let _ = phase;
}

trait CaptureOperations: Send + Sync {
    fn sync_state(&self, state: &File) -> io::Result<()>;
    fn sync_staging(&self, staging: &File) -> io::Result<()>;
    fn sync_parent(&self, parent: &File) -> io::Result<()>;
}

struct SystemCaptureOperations;

impl CaptureOperations for SystemCaptureOperations {
    fn sync_state(&self, state: &File) -> io::Result<()> {
        state.sync_all()
    }

    fn sync_staging(&self, staging: &File) -> io::Result<()> {
        staging.sync_all()
    }

    fn sync_parent(&self, parent: &File) -> io::Result<()> {
        parent.sync_all()
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TestCaptureSyncFailure {
    State,
    Staging,
    FinalParent,
}

#[cfg(test)]
struct FailingCaptureOperations {
    failure: TestCaptureSyncFailure,
    parent_syncs: core::sync::atomic::AtomicU8,
}

#[cfg(test)]
impl CaptureOperations for FailingCaptureOperations {
    fn sync_state(&self, state: &File) -> io::Result<()> {
        if self.failure == TestCaptureSyncFailure::State {
            Err(io::Error::other("injected state sync failure"))
        } else {
            state.sync_all()
        }
    }

    fn sync_staging(&self, staging: &File) -> io::Result<()> {
        if self.failure == TestCaptureSyncFailure::Staging {
            Err(io::Error::other("injected staging sync failure"))
        } else {
            staging.sync_all()
        }
    }

    fn sync_parent(&self, parent: &File) -> io::Result<()> {
        let occurrence = self.parent_syncs.fetch_add(1, Ordering::AcqRel);
        if self.failure == TestCaptureSyncFailure::FinalParent && occurrence == 1 {
            Err(io::Error::other("injected parent sync failure"))
        } else {
            parent.sync_all()
        }
    }
}

pub(crate) async fn capture_online_backup(
    pool: &PrivateConnectionPool,
    closing: &AtomicBool,
    active: &Arc<AtomicBool>,
    staging_directory: &Path,
    created_at_unix_ms: BackupCreatedAtUnixMs,
) -> Result<ServiceBackupManifest, ServiceSqliteError> {
    capture_online_backup_with_operations(
        pool,
        closing,
        active,
        staging_directory,
        created_at_unix_ms,
        Arc::new(SystemCaptureOperations),
    )
    .await
}

#[cfg(test)]
pub(crate) async fn test_capture_online_backup_with_sync_failure(
    pool: &PrivateConnectionPool,
    closing: &AtomicBool,
    active: &Arc<AtomicBool>,
    staging_directory: &Path,
    created_at_unix_ms: BackupCreatedAtUnixMs,
    failure: TestCaptureSyncFailure,
) -> Result<ServiceBackupManifest, ServiceSqliteError> {
    capture_online_backup_with_operations(
        pool,
        closing,
        active,
        staging_directory,
        created_at_unix_ms,
        Arc::new(FailingCaptureOperations {
            failure,
            parent_syncs: core::sync::atomic::AtomicU8::new(0),
        }),
    )
    .await
}

async fn capture_online_backup_with_operations(
    pool: &PrivateConnectionPool,
    closing: &AtomicBool,
    active: &Arc<AtomicBool>,
    staging_directory: &Path,
    created_at_unix_ms: BackupCreatedAtUnixMs,
    operations: Arc<dyn CaptureOperations>,
) -> Result<ServiceBackupManifest, ServiceSqliteError> {
    if !matches!(
        pool.mode(),
        OpenMode::Initialize | OpenMode::ReadWriteExisting
    ) || closing.load(Ordering::Acquire)
    {
        return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Open));
    }
    let staging = StagingPath::new(staging_directory)?;
    let permit = CapturePermit::acquire(Arc::clone(active))?;
    pool.validate()?;
    let mut admission = pool.acquire().await?;
    pool.validate()?;
    if closing.load(Ordering::Acquire) {
        return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Open));
    }
    let metadata = crate::metadata::verify_database_metadata(&mut admission, pool.identity()).await;
    #[cfg(test)]
    let metadata = if TEST_CAPTURE_INJECT_METADATA_FAILURE.load(Ordering::Acquire) {
        Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Metadata))
    } else {
        metadata
    };
    test_async_phase(TEST_CAPTURE_PHASE_METADATA_AWAITED).await;
    pool.validate()?;
    let metadata = metadata?;
    let validator = pool.backup_source_validator();
    validator.validate()?;

    let cancellation = Arc::new(AtomicBool::new(false));
    let cancellation_guard = CaptureCancellation::new(Arc::clone(&cancellation));
    let worker = CaptureWorker {
        _admission: admission,
        _permit: permit,
        validator,
        metadata,
        staging,
        created_at_unix_ms,
        cancellation,
        operations,
    };
    let joined = tokio::task::spawn_blocking(move || worker.run()).await;
    test_async_phase(TEST_CAPTURE_PHASE_JOIN_AWAITED).await;
    cancellation_guard.complete();
    pool.validate()?;
    let result = joined.map_err(|source| backup_source(BackupFailureKind::Join, source))?;
    result.map(PendingCapture::commit)
}

struct CapturePermit {
    active: Arc<AtomicBool>,
}

impl CapturePermit {
    fn acquire(active: Arc<AtomicBool>) -> Result<Self, ServiceSqliteError> {
        active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| backup_error(BackupFailureKind::AlreadyActive))?;
        Ok(Self { active })
    }
}

impl Drop for CapturePermit {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

struct CaptureCancellation {
    cancelled: Arc<AtomicBool>,
    complete: AtomicBool,
}

impl CaptureCancellation {
    fn new(cancelled: Arc<AtomicBool>) -> Self {
        Self {
            cancelled,
            complete: AtomicBool::new(false),
        }
    }

    fn complete(&self) {
        self.complete.store(true, Ordering::Release);
    }
}

impl Drop for CaptureCancellation {
    fn drop(&mut self) {
        if !self.complete.load(Ordering::Acquire) {
            self.cancelled.store(true, Ordering::Release);
        }
    }
}

struct CaptureWorker {
    _admission: PoolConnection<Sqlite>,
    _permit: CapturePermit,
    validator: BackupSourceValidator,
    metadata: ServiceDatabaseMetadata,
    staging: StagingPath,
    created_at_unix_ms: BackupCreatedAtUnixMs,
    cancellation: Arc<AtomicBool>,
    operations: Arc<dyn CaptureOperations>,
}

impl CaptureWorker {
    fn run(self) -> Result<PendingCapture, ServiceSqliteError> {
        self.check_cancelled()?;
        self.validator.validate()?;
        self.test_phase(TEST_CAPTURE_PHASE_BEFORE_CREATE);
        self.check_cancelled()?;
        let mut staging = StagingGuard::create(&self.staging, self.operations.as_ref())?;
        self.test_phase(TEST_CAPTURE_PHASE_STAGING_CREATED);
        #[cfg(test)]
        if TEST_CAPTURE_PANIC_WORKER.load(Ordering::Acquire) {
            panic!("injected backup worker failure");
        }
        self.check_cancelled()?;
        self.validator.validate()?;
        staging.validate()?;

        let source = self.open_source()?;
        let mut destination = self.open_destination(&staging)?;
        staging.record_sidecars();
        verify_database_inventory(&source)?;
        verify_database_metadata(&source, &self.metadata)?;
        self.validator.validate()?;
        staging.validate()?;

        {
            let backup = match rusqlite::backup::Backup::new(&source, &mut destination) {
                Ok(backup) => backup,
                Err(source) => {
                    staging.record_sidecars();
                    return Err(backup_source(BackupFailureKind::Capture, source));
                }
            };
            loop {
                self.check_cancelled()?;
                self.validator.validate()?;
                staging.validate()?;
                let step = backup.step(BACKUP_PAGES_PER_STEP);
                staging.record_sidecars();
                self.test_phase(TEST_CAPTURE_PHASE_BACKUP_STEPPED);
                let step =
                    step.map_err(|source| backup_source(BackupFailureKind::Capture, source))?;
                self.validator.validate()?;
                staging.validate()?;
                match step {
                    StepResult::Done => break,
                    StepResult::More => {}
                    StepResult::Busy | StepResult::Locked => thread::yield_now(),
                    _ => return Err(backup_error(BackupFailureKind::Capture)),
                }
            }
        }
        staging.record_sidecars();

        self.test_phase(TEST_CAPTURE_PHASE_POST_COPY);
        self.check_cancelled()?;
        self.validator.validate()?;
        staging.validate()?;
        verify_database_inventory(&destination)?;
        verify_database_metadata(&destination, &self.metadata)?;
        verify_integrity(&destination)?;
        staging.record_sidecars();
        self.check_cancelled()?;
        self.validator.validate()?;
        staging.validate()?;

        destination
            .close()
            .map_err(|(_, source)| backup_source(BackupFailureKind::Capture, source))?;
        staging.record_sidecars();
        source
            .close()
            .map_err(|(_, source)| backup_source(BackupFailureKind::Capture, source))?;
        self.validator.validate()?;
        staging.validate()?;
        staging.validate_inventory()?;
        self.check_cancelled()?;

        staging.sync_state(self.operations.as_ref())?;
        let (byte_length, digest) = staging.hash_state(&self.cancellation)?;
        self.test_phase(TEST_CAPTURE_PHASE_PRE_FINAL_SYNC);
        self.check_cancelled()?;
        staging.sync_directories(self.operations.as_ref())?;
        self.validator.validate()?;
        staging.validate()?;
        staging.validate_inventory()?;
        self.check_cancelled()?;

        let manifest = ServiceBackupManifest::from_capture(
            &self.metadata,
            self.created_at_unix_ms,
            byte_length,
            BackupMemberSha256::from_bytes(digest),
        )
        .map_err(|source| backup_source(BackupFailureKind::Manifest, source))?;
        Ok(PendingCapture {
            staging,
            _admission: self._admission,
            _permit: self._permit,
            manifest,
        })
    }

    fn open_source(&self) -> Result<Connection, ServiceSqliteError> {
        self.validator.validate()?;
        let result = Connection::open_with_flags(
            self.validator.database_path(),
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        );
        self.validator.validate()?;
        let connection =
            result.map_err(|source| backup_source(BackupFailureKind::Capture, source))?;
        connection
            .pragma_update(None, "query_only", true)
            .map_err(|source| backup_source(BackupFailureKind::Capture, source))?;
        Ok(connection)
    }

    fn open_destination(&self, staging: &StagingGuard) -> Result<Connection, ServiceSqliteError> {
        staging.validate()?;
        let result = Connection::open_with_flags(
            staging.state_path(),
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        );
        staging.validate()?;
        result.map_err(|source| backup_source(BackupFailureKind::Capture, source))
    }

    fn check_cancelled(&self) -> Result<(), ServiceSqliteError> {
        if self.cancellation.load(Ordering::Acquire) {
            Err(backup_error(BackupFailureKind::Cancelled))
        } else {
            Ok(())
        }
    }

    fn test_phase(&self, phase: u8) {
        #[cfg(test)]
        {
            TEST_CAPTURE_PHASE.store(phase, Ordering::Release);
            while TEST_CAPTURE_BLOCK_PHASE.load(Ordering::Acquire) == phase
                && !self.cancellation.load(Ordering::Acquire)
            {
                thread::yield_now();
            }
        }
        #[cfg(not(test))]
        let _ = phase;
    }
}

struct PendingCapture {
    staging: StagingGuard,
    _admission: PoolConnection<Sqlite>,
    _permit: CapturePermit,
    manifest: ServiceBackupManifest,
}

impl PendingCapture {
    fn commit(mut self) -> ServiceBackupManifest {
        self.staging.commit();
        self.manifest
    }
}

#[derive(Clone)]
struct StagingPath {
    full: PathBuf,
    parent: PathBuf,
    name: OsString,
}

impl StagingPath {
    fn new(path: &Path) -> Result<Self, ServiceSqliteError> {
        if path.as_os_str().as_bytes().len() > MAX_STAGING_PATH_BYTES || !path.is_absolute() {
            return Err(backup_error(BackupFailureKind::InvalidStagingPath));
        }
        if path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(backup_error(BackupFailureKind::InvalidStagingPath));
        }
        let name = match path.components().next_back() {
            Some(Component::Normal(name)) if !name.as_bytes().is_empty() => name.to_os_string(),
            _ => return Err(backup_error(BackupFailureKind::InvalidStagingPath)),
        };
        let parent = path
            .parent()
            .filter(|parent| parent.is_absolute())
            .ok_or_else(|| backup_error(BackupFailureKind::InvalidStagingPath))?
            .to_path_buf();
        Ok(Self {
            full: path.to_path_buf(),
            parent,
            name,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

struct StagingGuard {
    path: StagingPath,
    parent: File,
    parent_identity: FileIdentity,
    directory: File,
    directory_identity: FileIdentity,
    state: File,
    state_identity: FileIdentity,
    sidecar_identities: [Option<FileIdentity>; KNOWN_SIDECARS.len()],
    committed: bool,
}

impl StagingGuard {
    fn create(
        path: &StagingPath,
        operations: &dyn CaptureOperations,
    ) -> Result<Self, ServiceSqliteError> {
        let parent = File::from(
            open(
                &path.parent,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|source| backup_source(BackupFailureKind::InvalidStagingParent, source))?,
        );
        let parent_identity = validate_directory_descriptor(&parent, false)?;
        mkdirat(&parent, &path.name, Mode::RUSR | Mode::WUSR | Mode::XUSR).map_err(|source| {
            backup_source(
                if source == rustix::io::Errno::EXIST {
                    BackupFailureKind::StagingCollision
                } else {
                    BackupFailureKind::CreateStaging
                },
                source,
            )
        })?;
        let created_directory_identity = match created_directory_identity(&parent, &path.name) {
            Ok(identity) => identity,
            Err(error) => {
                // Without a proven identity, the current entry must be preserved.
                let _ = parent.sync_all();
                return Err(error);
            }
        };
        let directory = match openat(
            &parent,
            &path.name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(directory) => File::from(directory),
            Err(source) => {
                cleanup_partial_staging(
                    &parent,
                    &path.name,
                    Some(created_directory_identity),
                    None,
                    None,
                );
                return Err(backup_source(BackupFailureKind::CreateStaging, source));
            }
        };
        let opened_directory_identity = match descriptor_identity(&directory) {
            Ok(identity) => identity,
            Err(error) => {
                cleanup_partial_staging(
                    &parent,
                    &path.name,
                    Some(created_directory_identity),
                    Some(&directory),
                    None,
                );
                return Err(error);
            }
        };
        if opened_directory_identity != created_directory_identity {
            cleanup_partial_staging(
                &parent,
                &path.name,
                Some(created_directory_identity),
                Some(&directory),
                None,
            );
            return Err(backup_error(BackupFailureKind::StagingReplaced));
        }
        if let Err(source) = fchmod(&directory, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
            cleanup_partial_staging(
                &parent,
                &path.name,
                Some(created_directory_identity),
                Some(&directory),
                None,
            );
            return Err(backup_source(BackupFailureKind::CreateStaging, source));
        }
        let directory_identity = match validate_directory_descriptor(&directory, true) {
            Ok(identity) => identity,
            Err(error) => {
                cleanup_partial_staging(
                    &parent,
                    &path.name,
                    Some(created_directory_identity),
                    Some(&directory),
                    None,
                );
                return Err(error);
            }
        };
        let state = match openat(
            &directory,
            STATE_FILE_NAME,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        ) {
            Ok(state) => File::from(state),
            Err(source) => {
                cleanup_partial_staging(
                    &parent,
                    &path.name,
                    Some(created_directory_identity),
                    Some(&directory),
                    None,
                );
                return Err(backup_source(BackupFailureKind::CreateState, source));
            }
        };
        let created_state_identity = match descriptor_identity(&state) {
            Ok(identity) => identity,
            Err(error) => {
                cleanup_partial_staging(
                    &parent,
                    &path.name,
                    Some(created_directory_identity),
                    Some(&directory),
                    None,
                );
                return Err(error);
            }
        };
        if let Err(source) = fchmod(&state, Mode::RUSR | Mode::WUSR) {
            cleanup_partial_staging(
                &parent,
                &path.name,
                Some(created_directory_identity),
                Some(&directory),
                Some(created_state_identity),
            );
            return Err(backup_source(BackupFailureKind::CreateState, source));
        }
        let state_identity = match validate_file_descriptor(&state) {
            Ok(identity) => identity,
            Err(error) => {
                cleanup_partial_staging(
                    &parent,
                    &path.name,
                    Some(created_directory_identity),
                    Some(&directory),
                    Some(created_state_identity),
                );
                return Err(error);
            }
        };
        let staging = Self {
            path: path.clone(),
            parent,
            parent_identity,
            directory,
            directory_identity,
            state,
            state_identity,
            sidecar_identities: [None; KNOWN_SIDECARS.len()],
            committed: false,
        };
        staging.validate()?;
        operations
            .sync_parent(&staging.parent)
            .map_err(|source| backup_source(BackupFailureKind::SyncParent, source))?;
        Ok(staging)
    }

    fn state_path(&self) -> PathBuf {
        self.path.full.join(STATE_FILE_NAME)
    }

    fn validate(&self) -> Result<(), ServiceSqliteError> {
        validate_reopened_directory(&self.path.parent, &self.parent, self.parent_identity, false)?;
        validate_directory_entry(
            &self.parent,
            &self.path.name,
            &self.directory,
            self.directory_identity,
        )?;
        validate_file_entry(
            &self.directory,
            OsStr::new(STATE_FILE_NAME),
            &self.state,
            self.state_identity,
        )
    }

    fn validate_inventory(&self) -> Result<(), ServiceSqliteError> {
        self.validate()?;
        let mut entries = std::fs::read_dir(&self.path.full)
            .map_err(|source| backup_source(BackupFailureKind::InvalidStagingInventory, source))?;
        let first = entries
            .next()
            .transpose()
            .map_err(|source| backup_source(BackupFailureKind::InvalidStagingInventory, source))?
            .ok_or_else(|| backup_error(BackupFailureKind::InvalidStagingInventory))?;
        if first.file_name() != OsStr::new(STATE_FILE_NAME) || entries.next().is_some() {
            return Err(backup_error(BackupFailureKind::InvalidStagingInventory));
        }
        Ok(())
    }

    fn sync_state(&self, operations: &dyn CaptureOperations) -> Result<(), ServiceSqliteError> {
        self.validate()?;
        operations
            .sync_state(&self.state)
            .map_err(|source| backup_source(BackupFailureKind::SyncState, source))?;
        self.validate()
    }

    fn hash_state(&self, cancellation: &AtomicBool) -> Result<(u64, [u8; 32]), ServiceSqliteError> {
        self.validate()?;
        let mut state = self
            .state
            .try_clone()
            .map_err(|source| backup_source(BackupFailureKind::HashState, source))?;
        state
            .seek(SeekFrom::Start(0))
            .map_err(|source| backup_source(BackupFailureKind::HashState, source))?;
        let mut hasher = Sha256::new();
        let mut length = 0_u64;
        let mut buffer = [0_u8; HASH_BUFFER_BYTES];
        loop {
            if cancellation.load(Ordering::Acquire) {
                return Err(backup_error(BackupFailureKind::Cancelled));
            }
            let count = state
                .read(&mut buffer)
                .map_err(|source| backup_source(BackupFailureKind::HashState, source))?;
            if count == 0 {
                break;
            }
            length = length
                .checked_add(
                    u64::try_from(count).map_err(|_| backup_error(BackupFailureKind::HashState))?,
                )
                .ok_or_else(|| backup_error(BackupFailureKind::HashState))?;
            if length > i64::MAX as u64 {
                return Err(backup_error(BackupFailureKind::HashState));
            }
            hasher.update(&buffer[..count]);
        }
        if length == 0 {
            return Err(backup_error(BackupFailureKind::HashState));
        }
        self.validate()?;
        Ok((length, hasher.finalize().into()))
    }

    fn sync_directories(
        &self,
        operations: &dyn CaptureOperations,
    ) -> Result<(), ServiceSqliteError> {
        self.validate()?;
        operations
            .sync_staging(&self.directory)
            .map_err(|source| backup_source(BackupFailureKind::SyncStaging, source))?;
        self.validate()?;
        operations
            .sync_parent(&self.parent)
            .map_err(|source| backup_source(BackupFailureKind::SyncParent, source))?;
        self.validate()
    }

    fn record_sidecars(&mut self) {
        for (index, name) in KNOWN_SIDECARS.iter().enumerate() {
            if self.sidecar_identities[index].is_none() {
                self.sidecar_identities[index] = safe_sidecar_identity(&self.directory, name);
            }
        }
    }

    fn commit(&mut self) {
        self.committed = true;
    }

    fn cleanup(&mut self) {
        if self.committed {
            return;
        }
        if validate_directory_entry(
            &self.parent,
            &self.path.name,
            &self.directory,
            self.directory_identity,
        )
        .is_err()
        {
            return;
        }
        if current_entry_identity(&self.directory, OsStr::new(STATE_FILE_NAME))
            == Some(self.state_identity)
        {
            let _ = unlinkat(&self.directory, STATE_FILE_NAME, AtFlags::empty());
        }
        for (sidecar, identity) in KNOWN_SIDECARS.iter().zip(self.sidecar_identities) {
            if identity.is_some() && safe_sidecar_identity(&self.directory, sidecar) == identity {
                let _ = unlinkat(&self.directory, *sidecar, AtFlags::empty());
            }
        }
        if current_entry_identity(&self.parent, &self.path.name) == Some(self.directory_identity) {
            let _ = unlinkat(&self.parent, &self.path.name, AtFlags::REMOVEDIR);
        }
        let _ = self.parent.sync_all();
        self.committed = true;
    }
}

fn cleanup_partial_staging(
    parent: &File,
    name: &OsStr,
    directory_identity: Option<FileIdentity>,
    directory: Option<&File>,
    state_identity: Option<FileIdentity>,
) {
    if let (Some(directory), Some(state_identity)) = (directory, state_identity)
        && current_entry_identity(directory, OsStr::new(STATE_FILE_NAME)) == Some(state_identity)
    {
        let _ = unlinkat(directory, STATE_FILE_NAME, AtFlags::empty());
    }
    if directory_identity.is_some() && current_entry_identity(parent, name) == directory_identity {
        let _ = unlinkat(parent, name, AtFlags::REMOVEDIR);
    }
    let _ = parent.sync_all();
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        self.cleanup();
    }
}

fn created_directory_identity(
    parent: &File,
    name: &OsStr,
) -> Result<FileIdentity, ServiceSqliteError> {
    let status = statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|source| backup_source(BackupFailureKind::StagingReplaced, source))?;
    if !FileType::from_raw_mode(status.st_mode).is_dir()
        || status.st_uid != geteuid().as_raw()
        || u32::from(status.st_mode) & 0o022 != 0
    {
        return Err(backup_error(BackupFailureKind::StagingReplaced));
    }
    identity(&status)
}

fn descriptor_identity(descriptor: &File) -> Result<FileIdentity, ServiceSqliteError> {
    let status = fstat(descriptor)
        .map_err(|source| backup_source(BackupFailureKind::StagingReplaced, source))?;
    identity(&status)
}

fn validate_directory_descriptor(
    directory: &File,
    exact_owner_mode: bool,
) -> Result<FileIdentity, ServiceSqliteError> {
    let status = fstat(directory)
        .map_err(|source| backup_source(BackupFailureKind::InvalidStagingParent, source))?;
    let mode = u32::from(status.st_mode) & 0o777;
    if !FileType::from_raw_mode(status.st_mode).is_dir()
        || status.st_uid != geteuid().as_raw()
        || if exact_owner_mode {
            mode != 0o700
        } else {
            mode & 0o022 != 0
        }
    {
        return Err(backup_error(BackupFailureKind::InvalidStagingParent));
    }
    identity(&status)
}

fn validate_file_descriptor(file: &File) -> Result<FileIdentity, ServiceSqliteError> {
    let status = fstat(file)
        .map_err(|source| backup_source(BackupFailureKind::InvalidStagingInventory, source))?;
    if !FileType::from_raw_mode(status.st_mode).is_file()
        || u64::from(status.st_nlink) != 1
        || status.st_uid != geteuid().as_raw()
        || u32::from(status.st_mode) & 0o777 != 0o600
    {
        return Err(backup_error(BackupFailureKind::InvalidStagingInventory));
    }
    identity(&status)
}

fn validate_reopened_directory(
    path: &Path,
    held: &File,
    expected: FileIdentity,
    exact_owner_mode: bool,
) -> Result<(), ServiceSqliteError> {
    let current = File::from(
        open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|source| backup_source(BackupFailureKind::StagingReplaced, source))?,
    );
    if validate_directory_descriptor(&current, exact_owner_mode)? != expected
        || validate_directory_descriptor(held, exact_owner_mode)? != expected
    {
        return Err(backup_error(BackupFailureKind::StagingReplaced));
    }
    Ok(())
}

fn validate_directory_entry(
    parent: &File,
    name: &OsStr,
    held: &File,
    expected: FileIdentity,
) -> Result<(), ServiceSqliteError> {
    let current = File::from(
        openat(
            parent,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|source| backup_source(BackupFailureKind::StagingReplaced, source))?,
    );
    if validate_directory_descriptor(&current, true)? != expected
        || validate_directory_descriptor(held, true)? != expected
    {
        return Err(backup_error(BackupFailureKind::StagingReplaced));
    }
    Ok(())
}

fn validate_file_entry(
    directory: &File,
    name: &OsStr,
    held: &File,
    expected: FileIdentity,
) -> Result<(), ServiceSqliteError> {
    let current = File::from(
        openat(
            directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|source| backup_source(BackupFailureKind::StagingReplaced, source))?,
    );
    if validate_file_descriptor(&current)? != expected
        || validate_file_descriptor(held)? != expected
    {
        return Err(backup_error(BackupFailureKind::StagingReplaced));
    }
    Ok(())
}

fn current_entry_identity(directory: &File, name: &OsStr) -> Option<FileIdentity> {
    let status = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW).ok()?;
    identity(&status).ok()
}

fn safe_sidecar_identity(directory: &File, name: &str) -> Option<FileIdentity> {
    let status = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW).ok()?;
    if !FileType::from_raw_mode(status.st_mode).is_file()
        || u64::from(status.st_nlink) != 1
        || status.st_uid != geteuid().as_raw()
    {
        return None;
    }
    identity(&status).ok()
}

fn identity(status: &rustix::fs::Stat) -> Result<FileIdentity, ServiceSqliteError> {
    Ok(FileIdentity {
        device: u64::try_from(status.st_dev)
            .map_err(|_| backup_error(BackupFailureKind::StagingReplaced))?,
        inode: status.st_ino,
    })
}

fn verify_database_inventory(connection: &Connection) -> Result<(), ServiceSqliteError> {
    let mut statement = connection
        .prepare("PRAGMA database_list")
        .map_err(|source| backup_source(BackupFailureKind::Capture, source))?;
    let mut rows = statement
        .query([])
        .map_err(|source| backup_source(BackupFailureKind::Capture, source))?;
    let first = rows
        .next()
        .map_err(|source| backup_source(BackupFailureKind::Capture, source))?
        .ok_or_else(|| backup_error(BackupFailureKind::Capture))?;
    let sequence: i64 = first
        .get(0)
        .map_err(|source| backup_source(BackupFailureKind::Capture, source))?;
    let name: String = first
        .get(1)
        .map_err(|source| backup_source(BackupFailureKind::Capture, source))?;
    if sequence != 0 || name != "main" {
        return Err(backup_error(BackupFailureKind::Capture));
    }
    if rows
        .next()
        .map_err(|source| backup_source(BackupFailureKind::Capture, source))?
        .is_some()
    {
        return Err(backup_error(BackupFailureKind::Capture));
    }
    Ok(())
}

fn verify_database_metadata(
    connection: &Connection,
    expected: &ServiceDatabaseMetadata,
) -> Result<(), ServiceSqliteError> {
    let application_id: i64 = connection
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .map_err(metadata_source)?;
    let row_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM (SELECT 1 FROM radroots_service_metadata LIMIT 2)",
            [],
            |row| row.get(0),
        )
        .map_err(metadata_source)?;
    if row_count != 1 || application_id != i64::from(expected.application_id().get()) {
        return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Metadata));
    }
    let row = connection
        .query_row(
            "SELECT
                CASE WHEN typeof(service_id) = 'text'
                          AND length(CAST(service_id AS BLOB)) BETWEEN 1 AND ?1
                     THEN service_id END,
                CASE WHEN typeof(instance_id) = 'text'
                          AND length(CAST(instance_id AS BLOB)) BETWEEN 1 AND ?1
                     THEN instance_id END,
                CASE WHEN typeof(source_generation) = 'blob'
                          AND length(source_generation) = 32
                     THEN source_generation END,
                CASE WHEN typeof(state_schema_version) = 'integer'
                     THEN state_schema_version END,
                CASE WHEN typeof(created_at_unix_ms) = 'integer'
                     THEN created_at_unix_ms END
             FROM radroots_service_metadata
             WHERE singleton = 1
             LIMIT 1",
            [MAX_ID_UTF8_BYTES],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<Vec<u8>>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        )
        .optional()
        .map_err(metadata_source)?;
    let Some((Some(service), Some(instance), Some(generation), Some(schema), Some(created_at))) =
        row
    else {
        return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Metadata));
    };
    if service != expected.service().as_str()
        || instance != expected.instance().as_str()
        || generation.as_slice() != expected.source_generation().as_bytes()
        || schema != i64::from(expected.state_schema_version().get())
        || created_at != i64::try_from(expected.created_at_unix_ms()).unwrap_or(-1)
    {
        return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Metadata));
    }
    Ok(())
}

fn verify_integrity(connection: &Connection) -> Result<(), ServiceSqliteError> {
    let mut statement = connection
        .prepare("PRAGMA integrity_check(1)")
        .map_err(integrity_source)?;
    let mut rows = statement.query([]).map_err(integrity_source)?;
    let row = rows
        .next()
        .map_err(integrity_source)?
        .ok_or_else(|| ServiceSqliteError::new(ServiceSqliteErrorKind::Integrity))?;
    let result = row.get_ref(0).map_err(integrity_source)?;
    if !integrity_projection_is_ok(result) || rows.next().map_err(integrity_source)?.is_some() {
        return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Integrity));
    }
    let mut statement = connection
        .prepare("PRAGMA foreign_key_check")
        .map_err(integrity_source)?;
    if statement
        .query([])
        .map_err(integrity_source)?
        .next()
        .map_err(integrity_source)?
        .is_some()
    {
        return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Integrity));
    }
    Ok(())
}

fn integrity_projection_is_ok(value: ValueRef<'_>) -> bool {
    matches!(
        value,
        ValueRef::Text(bytes)
            if !bytes.is_empty()
                && bytes.len() <= MAX_INTEGRITY_RESULT_UTF8_BYTES
                && bytes == b"ok"
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackupFailureKind {
    InvalidStagingPath,
    InvalidStagingParent,
    StagingCollision,
    CreateStaging,
    CreateState,
    StagingReplaced,
    InvalidStagingInventory,
    AlreadyActive,
    Capture,
    Cancelled,
    HashState,
    SyncState,
    SyncStaging,
    SyncParent,
    Manifest,
    Join,
}

struct BackupFailure {
    kind: BackupFailureKind,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl fmt::Debug for BackupFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupFailure")
            .field("kind", &self.kind)
            .field("source", &self.source.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

impl fmt::Display for BackupFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            BackupFailureKind::InvalidStagingPath => "backup staging path is invalid",
            BackupFailureKind::InvalidStagingParent => "backup staging parent is invalid",
            BackupFailureKind::StagingCollision => "backup staging destination already exists",
            BackupFailureKind::CreateStaging => "backup staging directory could not be created",
            BackupFailureKind::CreateState => "backup state member could not be created",
            BackupFailureKind::StagingReplaced => "backup staging identity changed",
            BackupFailureKind::InvalidStagingInventory => "backup staging inventory is invalid",
            BackupFailureKind::AlreadyActive => "another backup capture is active",
            BackupFailureKind::Capture => "online backup capture failed",
            BackupFailureKind::Cancelled => "online backup capture was cancelled",
            BackupFailureKind::HashState => "backup state member could not be hashed",
            BackupFailureKind::SyncState => "backup state member could not be synchronized",
            BackupFailureKind::SyncStaging => "backup staging directory could not be synchronized",
            BackupFailureKind::SyncParent => "backup staging parent could not be synchronized",
            BackupFailureKind::Manifest => "backup manifest could not be constructed",
            BackupFailureKind::Join => "backup worker could not be joined",
        })
    }
}

impl Error for BackupFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

fn backup_error(kind: BackupFailureKind) -> ServiceSqliteError {
    ServiceSqliteError::with_source(
        ServiceSqliteErrorKind::Backup,
        BackupFailure { kind, source: None },
    )
}

fn backup_source(
    kind: BackupFailureKind,
    source: impl Error + Send + Sync + 'static,
) -> ServiceSqliteError {
    ServiceSqliteError::with_source(
        ServiceSqliteErrorKind::Backup,
        BackupFailure {
            kind,
            source: Some(Box::new(source)),
        },
    )
}

fn integrity_source(source: rusqlite::Error) -> ServiceSqliteError {
    ServiceSqliteError::with_source(ServiceSqliteErrorKind::Integrity, source)
}

fn metadata_source(source: rusqlite::Error) -> ServiceSqliteError {
    ServiceSqliteError::with_source(ServiceSqliteErrorKind::Metadata, source)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{PermissionsExt, symlink};

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn staging_path_rejects_relative_parent_and_oversize_inputs() {
        assert!(StagingPath::new(Path::new("relative/stage")).is_err());
        assert!(StagingPath::new(Path::new("/tmp/../stage")).is_err());
        assert!(StagingPath::new(Path::new("/")).is_err());
        let large = format!("/tmp/{}", "x".repeat(MAX_STAGING_PATH_BYTES));
        assert!(StagingPath::new(Path::new(&large)).is_err());
    }

    #[test]
    fn staging_guard_creates_exact_modes_and_cleans_uncommitted_state() {
        let root = tempdir().expect("root");
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
            .expect("parent mode");
        let path = StagingPath::new(&root.path().join("backup-stage")).expect("path");
        {
            let staging = StagingGuard::create(&path, &SystemCaptureOperations).expect("staging");
            staging.validate().expect("validate");
            assert_eq!(
                std::fs::metadata(&path.full)
                    .expect("directory metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(staging.state_path())
                    .expect("state metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        assert!(!path.full.exists());
    }

    #[test]
    fn staging_guard_rejects_collision_and_preserves_replacement() {
        let root = tempdir().expect("root");
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
            .expect("parent mode");
        let full = root.path().join("backup-stage");
        std::fs::create_dir(&full).expect("collision");
        assert!(
            StagingGuard::create(
                &StagingPath::new(&full).expect("path"),
                &SystemCaptureOperations,
            )
            .is_err()
        );
        std::fs::remove_dir(&full).expect("remove collision");

        let path = StagingPath::new(&full).expect("path");
        let staging = StagingGuard::create(&path, &SystemCaptureOperations).expect("staging");
        let original = staging.state_path();
        let replacement = full.join("replacement");
        std::fs::write(&replacement, b"foreign").expect("replacement");
        std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o600))
            .expect("replacement mode");
        std::fs::rename(&replacement, &original).expect("replace state");
        drop(staging);
        assert_eq!(std::fs::read(&original).expect("preserved"), b"foreign");
        assert!(full.exists());
    }

    #[test]
    fn staging_guard_rejects_insecure_or_symlinked_parent_without_mutation() {
        let root = tempdir().expect("root");
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
            .expect("root mode");
        let insecure = root.path().join("insecure");
        std::fs::create_dir(&insecure).expect("insecure parent");
        std::fs::set_permissions(&insecure, std::fs::Permissions::from_mode(0o770))
            .expect("insecure mode");
        let insecure_stage = insecure.join("stage");
        let insecure_error = match StagingGuard::create(
            &StagingPath::new(&insecure_stage).expect("insecure staging path"),
            &SystemCaptureOperations,
        ) {
            Ok(_) => panic!("group-writable parent must be rejected"),
            Err(error) => error,
        };
        assert_eq!(insecure_error.kind(), ServiceSqliteErrorKind::Backup);
        assert!(!insecure_stage.exists());

        let parent = root.path().join("real-parent");
        std::fs::create_dir(&parent).expect("real parent");
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700))
            .expect("real parent mode");
        let alias = root.path().join("parent-alias");
        symlink(&parent, &alias).expect("parent symlink");
        let symlink_stage = alias.join("stage");
        assert!(
            StagingGuard::create(
                &StagingPath::new(&symlink_stage).expect("symlink staging path"),
                &SystemCaptureOperations,
            )
            .is_err()
        );
        assert!(!parent.join("stage").exists());
    }

    #[test]
    fn staging_guard_rejects_hardlinked_state_and_preserves_other_link() {
        let root = tempdir().expect("root");
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
            .expect("parent mode");
        let path = StagingPath::new(&root.path().join("backup-stage")).expect("path");
        let staging = StagingGuard::create(&path, &SystemCaptureOperations).expect("staging");
        std::fs::write(staging.state_path(), b"captured").expect("state bytes");
        let outside = root.path().join("outside-link");
        std::fs::hard_link(staging.state_path(), &outside).expect("hard link");
        assert!(staging.validate().is_err());
        drop(staging);
        assert_eq!(
            std::fs::read(&outside).expect("other link survives"),
            b"captured"
        );
        assert!(!path.full.exists());
    }

    #[test]
    fn partial_cleanup_preserves_replaced_directory_and_state_entries() {
        let root = tempdir().expect("root");
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
            .expect("root mode");
        let parent = File::from(
            open(
                root.path(),
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .expect("open parent"),
        );
        mkdirat(&parent, "stage", Mode::RUSR | Mode::WUSR | Mode::XUSR)
            .expect("create original stage");
        let original_identity =
            created_directory_identity(&parent, OsStr::new("stage")).expect("original identity");
        let original = File::from(
            openat(
                &parent,
                "stage",
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .expect("open original stage"),
        );
        std::fs::rename(root.path().join("stage"), root.path().join("retired"))
            .expect("retire original stage");
        std::fs::create_dir(root.path().join("stage")).expect("replacement stage");
        std::fs::set_permissions(
            root.path().join("stage"),
            std::fs::Permissions::from_mode(0o700),
        )
        .expect("replacement mode");
        std::fs::write(root.path().join("stage/foreign"), b"foreign").expect("replacement member");
        cleanup_partial_staging(
            &parent,
            OsStr::new("stage"),
            Some(original_identity),
            Some(&original),
            None,
        );
        assert_eq!(
            std::fs::read(root.path().join("stage/foreign")).expect("foreign survives"),
            b"foreign"
        );

        let path = StagingPath::new(&root.path().join("state-stage")).expect("path");
        let staging = StagingGuard::create(&path, &SystemCaptureOperations).expect("staging");
        let retired_state = path.full.join("retired-state");
        std::fs::rename(staging.state_path(), &retired_state).expect("retire state");
        std::fs::write(staging.state_path(), b"foreign-state").expect("foreign state");
        std::fs::set_permissions(staging.state_path(), std::fs::Permissions::from_mode(0o600))
            .expect("foreign state mode");
        cleanup_partial_staging(
            &staging.parent,
            &staging.path.name,
            Some(staging.directory_identity),
            Some(&staging.directory),
            Some(staging.state_identity),
        );
        assert_eq!(
            std::fs::read(staging.state_path()).expect("replacement state survives"),
            b"foreign-state"
        );
        drop(staging);
        assert!(path.full.exists());
    }

    #[test]
    fn integrity_projection_bounds_corrupt_text_before_semantic_acceptance() {
        assert!(integrity_projection_is_ok(ValueRef::Text(b"ok")));
        let maximum = vec![b'x'; MAX_INTEGRITY_RESULT_UTF8_BYTES];
        assert!(!integrity_projection_is_ok(ValueRef::Text(&maximum)));
        let over_maximum = vec![b'x'; MAX_INTEGRITY_RESULT_UTF8_BYTES + 1];
        assert!(!integrity_projection_is_ok(ValueRef::Text(&over_maximum)));
        assert!(!integrity_projection_is_ok(ValueRef::Null));
    }

    #[test]
    fn active_capture_permit_is_exclusive_and_recoverable() {
        let active = Arc::new(AtomicBool::new(false));
        let first = CapturePermit::acquire(Arc::clone(&active)).expect("first permit");
        assert!(CapturePermit::acquire(Arc::clone(&active)).is_err());
        drop(first);
        assert!(CapturePermit::acquire(active).is_ok());
    }
}
