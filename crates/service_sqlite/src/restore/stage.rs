//! Offline staging of one retained verified backup.

use core::fmt;

use crate::{
    MigrationCatalog, SchemaCatalog, ServiceDatabaseIdentity, ServiceSqliteError,
    ServiceSqliteErrorKind, ServiceSqlitePaths, VerifiedServiceBackup,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use {
    super::{
        RestoreArtifactExpectation, RestoreRecoveryLayout,
        marker::{BACKUP_FILE_NAME, MARKER_FILE_NAME, MARKER_NEXT_FILE_NAME, STAGED_FILE_NAME},
    },
    crate::{OpenMode, ServiceDatabaseMetadata, WriterAuthority},
    rustix::{
        fs::{AtFlags, FileType, Mode, OFlags, fchmod, fstat, openat, statat, unlinkat},
        io::Errno,
        process::geteuid,
    },
    sha2::{Digest, Sha256},
    sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions},
    std::{
        error::Error,
        fs::File,
        io::{Read, Seek, SeekFrom, Write},
        os::fd::AsRawFd,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    },
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
const COPY_BUFFER_BYTES: usize = 64 * 1_024;

#[cfg(any(target_os = "linux", target_os = "macos"))]
const TEST_PHASE_BEFORE_CREATE: u8 = 1;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const TEST_PHASE_MID_COPY: u8 = 2;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const TEST_PHASE_POST_COPY: u8 = 3;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const TEST_PHASE_POLICY: u8 = 4;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const TEST_PHASE_METADATA: u8 = 5;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const TEST_PHASE_HISTORY: u8 = 6;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const TEST_PHASE_INTEGRITY: u8 = 7;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const TEST_PHASE_PRE_FINAL_SYNC: u8 = 8;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const TEST_PHASE_CREATED: u8 = 9;
#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
use std::sync::atomic::AtomicU8;
#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
static TEST_STAGE_PHASE: AtomicU8 = AtomicU8::new(0);
#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
static TEST_STAGE_BLOCK_PHASE: AtomicU8 = AtomicU8::new(0);
#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
static TEST_STAGE_FAIL_SYNC: AtomicU8 = AtomicU8::new(0);
#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
static TEST_STAGE_PANIC_WORKER: AtomicBool = AtomicBool::new(false);

/// Sealed capability for one completely reverified adjacent restore stage.
///
/// The capability owns exclusive writer authority and exact retained file
/// identities. Dropping it before finalization attempts exact-inode cleanup
/// before releasing authority; cleanup failure leaves evidence that later
/// admission rejects. It exposes no path or raw descriptor.
///
/// ```compile_fail
/// use radroots_service_sqlite::StagedServiceRestore;
/// let _forged = StagedServiceRestore {};
/// ```
///
/// ```compile_fail
/// fn require_clone<T: Clone>() {}
/// require_clone::<radroots_service_sqlite::StagedServiceRestore>();
/// ```
#[allow(dead_code)] // Step 068 consumes the retained native capability.
pub struct StagedServiceRestore {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    inner: Option<NativeStagedServiceRestore>,
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    _private: (),
}

impl fmt::Debug for StagedServiceRestore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("StagedServiceRestore([redacted])")
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl StagedServiceRestore {
    #[allow(dead_code)] // Step 068 consumes the retained native capability.
    pub(crate) fn into_native(mut self) -> NativeStagedServiceRestore {
        self.inner
            .take()
            .expect("staged restore capability is consumed only once")
    }
}

/// Copies and completely reverifies a retained backup beside closed live state.
///
/// This operation never renames or replaces live state and never creates a
/// recovery marker. It acquires exclusive writer authority before staging, so
/// an open writable or inspection host is rejected. Caller cancellation
/// requests bounded worker cancellation; any detached work continues to own
/// authority and exact cleanup until it terminates.
pub async fn stage_verified_restore(
    paths: &ServiceSqlitePaths,
    expected: &ServiceDatabaseIdentity,
    migrations: &MigrationCatalog,
    schema: &SchemaCatalog,
    verified: VerifiedServiceBackup,
) -> Result<StagedServiceRestore, ServiceSqliteError> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        validate_intent(paths, expected, migrations, schema, &verified)?;
        let authority = WriterAuthority::acquire(paths, OpenMode::ReadWriteExisting)?
            .ok_or_else(|| ServiceSqliteError::new(ServiceSqliteErrorKind::Authority))?;
        authority.validate_for(paths)?;

        let cancellation = Arc::new(AtomicBool::new(false));
        let cancellation_on_drop = CancellationOnDrop::new(Arc::clone(&cancellation));
        let task_paths = paths.clone();
        let task_expected = expected.clone();
        let task_migrations = migrations.clone();
        let task_schema = schema.clone();
        let task = tokio::spawn(async move {
            run_stage(
                task_paths,
                task_expected,
                task_migrations,
                task_schema,
                verified,
                authority,
                cancellation,
            )
            .await
        });
        let result = task
            .await
            .map_err(|source| restore_source(RestoreFailureKind::Join, source))?;
        cancellation_on_drop.disarm();
        result.map(|inner| StagedServiceRestore { inner: Some(inner) })
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (paths, expected, migrations, schema, verified);
        Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Restore))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_intent(
    paths: &ServiceSqlitePaths,
    expected: &ServiceDatabaseIdentity,
    migrations: &MigrationCatalog,
    schema: &SchemaCatalog,
    verified: &VerifiedServiceBackup,
) -> Result<(), ServiceSqliteError> {
    let metadata = verified.database_metadata();
    let manifest = verified.manifest();
    crate::require_condition(
        stage_intent_matches(paths, expected, migrations, schema, metadata, manifest),
        ServiceSqliteErrorKind::Metadata,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn stage_intent_matches(
    paths: &ServiceSqlitePaths,
    expected: &ServiceDatabaseIdentity,
    migrations: &MigrationCatalog,
    schema: &SchemaCatalog,
    metadata: &ServiceDatabaseMetadata,
    manifest: &crate::ServiceBackupManifest,
) -> bool {
    crate::all_constraints([
        expected.matches_paths(paths),
        expected.supported_state_schema_version().get() == migrations.current_version(),
        schema.matches_migrations(migrations),
        metadata.service() == expected.service(),
        metadata.instance() == expected.instance(),
        metadata.source_generation() == expected.source_generation(),
        metadata.application_id() == expected.application_id(),
        metadata.state_schema_version() <= expected.supported_state_schema_version(),
        manifest.service() == metadata.service(),
        manifest.instance() == metadata.instance(),
        manifest.source_generation() == metadata.source_generation(),
        manifest.state_schema_version() == metadata.state_schema_version(),
    ])
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn run_stage(
    paths: ServiceSqlitePaths,
    expected: ServiceDatabaseIdentity,
    migrations: MigrationCatalog,
    schema: SchemaCatalog,
    verified: VerifiedServiceBackup,
    authority: WriterAuthority,
    cancellation: Arc<AtomicBool>,
) -> Result<NativeStagedServiceRestore, ServiceSqliteError> {
    let copy_cancellation = Arc::clone(&cancellation);
    let copied = tokio::task::spawn_blocking(move || {
        #[cfg(test)]
        if TEST_STAGE_PANIC_WORKER.load(Ordering::Acquire) {
            panic!("injected restore staging worker failure");
        }
        NativeStagedServiceRestore::copy_from_verified(
            paths,
            verified,
            authority,
            &copy_cancellation,
        )
    })
    .await
    .map_err(|source| restore_source(RestoreFailureKind::Join, source))?;
    let mut staged = copied?;
    staged.validate()?;
    require_restore_condition(
        !cancellation.load(Ordering::Acquire),
        RestoreFailureKind::Cancelled,
    )?;

    let connect_options = staged.connect_options()?;
    staged.validate()?;
    let connected = SqliteConnection::connect_with(&connect_options).await;
    staged.validate()?;
    let mut connection =
        connected.map_err(|source| restore_source(RestoreFailureKind::OpenStaged, source))?;
    if let Err(error) = require_restore_condition(
        !cancellation.load(Ordering::Acquire),
        RestoreFailureKind::Cancelled,
    ) {
        close_after(&mut staged, connection).await?;
        return Err(error);
    }

    let verification = verify_staged_connection(
        &mut staged,
        &mut connection,
        &expected,
        &migrations,
        &schema,
        &cancellation,
    )
    .await;
    let close = connection.close().await;
    staged.validate()?;
    verification?;
    close.map_err(|source| restore_source(RestoreFailureKind::CloseStaged, source))?;
    require_restore_condition(
        !cancellation.load(Ordering::Acquire),
        RestoreFailureKind::Cancelled,
    )?;

    let final_cancellation = Arc::clone(&cancellation);
    tokio::task::spawn_blocking(move || staged.finalize(&final_cancellation))
        .await
        .map_err(|source| restore_source(RestoreFailureKind::Join, source))?
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn close_after(
    staged: &mut NativeStagedServiceRestore,
    connection: SqliteConnection,
) -> Result<(), ServiceSqliteError> {
    let result = connection.close().await;
    staged.validate()?;
    result.map_err(|source| restore_source(RestoreFailureKind::CloseStaged, source))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn verify_staged_connection(
    staged: &mut NativeStagedServiceRestore,
    connection: &mut SqliteConnection,
    expected: &ServiceDatabaseIdentity,
    migrations: &MigrationCatalog,
    schema: &SchemaCatalog,
    cancellation: &AtomicBool,
) -> Result<(), ServiceSqliteError> {
    let policy = verify_read_only_policy(connection).await;
    staged.validate()?;
    policy?;
    let phase = test_async_phase(TEST_PHASE_POLICY, cancellation).await;
    staged.validate()?;
    phase?;
    check_cancel(cancellation)?;

    let metadata = crate::metadata::verify_database_metadata(connection, expected).await;
    staged.validate()?;
    let metadata = metadata?;
    crate::require_condition(
        metadata == staged.metadata,
        ServiceSqliteErrorKind::Metadata,
    )?;
    let phase = test_async_phase(TEST_PHASE_METADATA, cancellation).await;
    staged.validate()?;
    phase?;
    check_cancel(cancellation)?;

    let history =
        crate::migration::verify_migration_history(connection, migrations, schema, false).await;
    staged.validate()?;
    let version = history?;
    crate::require_condition(
        version == staged.metadata.state_schema_version().get(),
        ServiceSqliteErrorKind::Migration,
    )?;
    let phase = test_async_phase(TEST_PHASE_HISTORY, cancellation).await;
    staged.validate()?;
    phase?;
    check_cancel(cancellation)?;

    let integrity = crate::integrity::verify_database_integrity(connection).await;
    staged.validate()?;
    integrity?;
    let phase = test_async_phase(TEST_PHASE_INTEGRITY, cancellation).await;
    staged.validate()?;
    phase?;
    check_cancel(cancellation)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn test_async_phase(phase: u8, cancellation: &AtomicBool) -> Result<(), ServiceSqliteError> {
    #[cfg(test)]
    {
        TEST_STAGE_PHASE.store(phase, Ordering::Release);
        while TEST_STAGE_BLOCK_PHASE.load(Ordering::Acquire) == phase {
            if cancellation.load(Ordering::Acquire) {
                return Err(restore_error(RestoreFailureKind::Cancelled));
            }
            tokio::task::yield_now().await;
        }
    }
    #[cfg(not(test))]
    let _ = (phase, cancellation);
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn verify_read_only_policy(
    connection: &mut SqliteConnection,
) -> Result<(), ServiceSqliteError> {
    sqlx::query("PRAGMA query_only = ON")
        .execute(&mut *connection)
        .await
        .map_err(|source| restore_source(RestoreFailureKind::Policy, source))?;
    sqlx::query("PRAGMA trusted_schema = OFF")
        .execute(&mut *connection)
        .await
        .map_err(|source| restore_source(RestoreFailureKind::Policy, source))?;
    let query_only = sqlx::query_scalar::<_, i64>("PRAGMA query_only")
        .fetch_one(&mut *connection)
        .await
        .map_err(|source| restore_source(RestoreFailureKind::Policy, source))?;
    let trusted_schema = sqlx::query_scalar::<_, i64>("PRAGMA trusted_schema")
        .fetch_one(&mut *connection)
        .await
        .map_err(|source| restore_source(RestoreFailureKind::Policy, source))?;
    let databases = sqlx::query(
        "SELECT
            seq,
            typeof(name) = 'text' AS name_type_ok,
            length(CAST(name AS BLOB)) AS name_length,
            substr(CAST(name AS BLOB), 1, 5) AS name_prefix
         FROM pragma_database_list
         LIMIT 2",
    )
    .fetch_all(connection)
    .await
    .map_err(|source| restore_source(RestoreFailureKind::Policy, source))?;
    let first_sequence = databases
        .first()
        .and_then(|row| row.try_get::<i64, _>(0).ok());
    let first_name = databases.first().and_then(|row| {
        crate::persisted_value::bounded_utf8(
            row,
            "name_type_ok",
            "name_length",
            "name_prefix",
            1,
            4,
        )
    });
    require_restore_condition(
        read_only_policy_matches(
            query_only,
            trusted_schema,
            databases.len(),
            first_sequence,
            first_name,
        ),
        RestoreFailureKind::Policy,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn read_only_policy_matches(
    query_only: i64,
    trusted_schema: i64,
    database_count: usize,
    first_sequence: Option<i64>,
    first_name: Option<&str>,
) -> bool {
    crate::all_constraints([
        query_only == 1,
        trusted_schema == 0,
        database_count == 1,
        first_sequence == Some(0),
        first_name == Some("main"),
    ])
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_restore_condition(
    condition: bool,
    kind: RestoreFailureKind,
) -> Result<(), ServiceSqliteError> {
    if condition {
        Ok(())
    } else {
        Err(restore_error(kind))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn check_cancel(cancellation: &AtomicBool) -> Result<(), ServiceSqliteError> {
    if cancellation.load(Ordering::Acquire) {
        Err(restore_error(RestoreFailureKind::Cancelled))
    } else {
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(dead_code)] // Step 068 consumes the retained marker/finalization inputs.
pub(crate) struct NativeStagedServiceRestore {
    paths: ServiceSqlitePaths,
    authority: Option<WriterAuthority>,
    directory: File,
    directory_identity: FileIdentity,
    staged: File,
    staged_identity: FileIdentity,
    live_artifact: RestoreArtifactExpectation,
    metadata: ServiceDatabaseMetadata,
    manifest_digest: crate::BackupManifestSha256,
    artifact: RestoreArtifactExpectation,
    armed: AtomicBool,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl NativeStagedServiceRestore {
    fn copy_from_verified(
        paths: ServiceSqlitePaths,
        verified: VerifiedServiceBackup,
        authority: WriterAuthority,
        cancellation: &AtomicBool,
    ) -> Result<Self, ServiceSqliteError> {
        authority.validate_for(&paths)?;
        check_cancel(cancellation)?;
        let source_binding = verified.validate_binding();
        authority.validate_for(&paths)?;
        source_binding.map_err(|_| restore_error(RestoreFailureKind::SourceChanged))?;
        let layout = RestoreRecoveryLayout::for_paths(&paths)
            .map_err(|_| restore_error(RestoreFailureKind::Layout))?;
        let directory_result = authority
            .directory()
            .try_clone()
            .map_err(|source| restore_source(RestoreFailureKind::Layout, source));
        authority.validate_for(&paths)?;
        let directory = directory_result?;
        let directory_identity_result = directory_identity(&directory);
        authority.validate_for(&paths)?;
        let directory_identity = directory_identity_result?;
        let closed_live = validate_closed_live(&authority, &paths, &directory);
        authority.validate_for(&paths)?;
        let live_artifact = closed_live?;
        let before_create = test_blocking_phase(TEST_PHASE_BEFORE_CREATE, cancellation);
        authority.validate_for(&paths)?;
        before_create?;
        let staged_result = create_stage(&directory);
        authority.validate_for(&paths)?;
        let pending_stage = staged_result?;
        let created = test_blocking_phase(TEST_PHASE_CREATED, cancellation);
        authority.validate_for(&paths)?;
        created?;
        let staged_identity = pending_stage.identity();
        let metadata = verified.database_metadata().clone();
        let manifest_digest = verified.manifest().digest();
        let member = verified
            .manifest()
            .members()
            .first()
            .ok_or_else(|| restore_error(RestoreFailureKind::SourceChanged))?;
        let expected_length = member.byte_length();
        let expected_digest = *member.sha256().as_bytes();
        let artifact = RestoreArtifactExpectation::new(
            staged_identity.device,
            staged_identity.inode,
            expected_length,
            expected_digest,
        )
        .map_err(|_| restore_error(RestoreFailureKind::Copy))?;
        let staged = pending_stage.into_file();
        let result = Self {
            paths,
            authority: Some(authority),
            directory,
            directory_identity,
            staged,
            staged_identity,
            live_artifact,
            metadata,
            manifest_digest,
            artifact,
            armed: AtomicBool::new(true),
        };
        result.validate_created()?;
        let copy = copy_exact(
            verified.state_file(),
            &result.staged,
            expected_length,
            expected_digest,
            cancellation,
        );
        result.validate()?;
        copy?;
        let sync = sync_staged_file(&result.staged, 1);
        result.validate()?;
        sync?;
        let header = validate_sqlite_header(&result.staged);
        result.validate()?;
        header?;
        let source_binding = verified.validate_binding();
        result.validate()?;
        source_binding.map_err(|_| restore_error(RestoreFailureKind::SourceChanged))?;
        let post_copy = test_blocking_phase(TEST_PHASE_POST_COPY, cancellation);
        result.validate()?;
        post_copy?;
        require_restore_condition(
            layout
                .staged()
                .file_name()
                .is_some_and(|name| name == STAGED_FILE_NAME)
                && Some(layout.state_directory().as_path())
                    == result.paths.state_database().parent(),
            RestoreFailureKind::Layout,
        )?;
        Ok(result)
    }

    fn connect_options(&self) -> Result<SqliteConnectOptions, ServiceSqliteError> {
        self.validate()?;
        let descriptor = self.staged.as_raw_fd();
        #[cfg(target_os = "linux")]
        let descriptor_path = format!("/proc/self/fd/{descriptor}");
        #[cfg(target_os = "macos")]
        let descriptor_path = format!("/dev/fd/{descriptor}");
        Ok(SqliteConnectOptions::new()
            .filename(descriptor_path)
            .read_only(true)
            .immutable(true)
            .create_if_missing(false))
    }

    fn finalize(self, cancellation: &AtomicBool) -> Result<Self, ServiceSqliteError> {
        self.validate()?;
        check_cancel(cancellation)?;
        let digest = hash_exact(&self.staged, self.artifact.byte_length());
        self.validate()?;
        let digest = digest?;
        require_restore_condition(
            digest == self.artifact.sha256(),
            RestoreFailureKind::StagedChanged,
        )?;
        let header = validate_sqlite_header(&self.staged);
        self.validate()?;
        header?;
        let sync = sync_staged_file(&self.staged, 2);
        self.validate()?;
        sync?;
        check_cancel(cancellation)?;
        let pre_final_sync = test_blocking_phase(TEST_PHASE_PRE_FINAL_SYNC, cancellation);
        self.validate()?;
        pre_final_sync?;
        let sync = sync_stage_directory(&self.directory);
        self.validate()?;
        sync?;
        check_cancel(cancellation)?;
        Ok(self)
    }

    pub(crate) fn validate(&self) -> Result<(), ServiceSqliteError> {
        let authority = self
            .authority
            .as_ref()
            .ok_or_else(|| ServiceSqliteError::new(ServiceSqliteErrorKind::Authority))?;
        authority.validate_for(&self.paths)?;
        let result = (|| {
            require_restore_condition(
                directory_identity(&self.directory)? == self.directory_identity,
                RestoreFailureKind::StagedChanged,
            )?;
            validate_stage_binding(
                &self.directory,
                &self.staged,
                self.staged_identity,
                self.artifact.byte_length(),
            )?;
            validate_live_binding(&self.directory, self.live_artifact)?;
            require_absent(&self.directory, BACKUP_FILE_NAME)?;
            require_absent(&self.directory, MARKER_FILE_NAME)?;
            require_absent(&self.directory, MARKER_NEXT_FILE_NAME)?;
            require_absent(&self.directory, "state.sqlite-wal")?;
            require_absent(&self.directory, "state.sqlite-shm")?;
            require_absent(&self.directory, "state.sqlite-journal")?;
            Ok(())
        })();
        authority.validate_for(&self.paths)?;
        result
    }

    pub(super) fn validate_finalization_authority(&self) -> Result<(), ServiceSqliteError> {
        let authority = self
            .authority
            .as_ref()
            .ok_or_else(|| ServiceSqliteError::new(ServiceSqliteErrorKind::Authority))?;
        authority.validate_for(&self.paths)?;
        let valid_directory = directory_identity(&self.directory)
            .is_ok_and(|identity| identity == self.directory_identity);
        authority.validate_for(&self.paths)?;
        crate::require_condition(valid_directory, ServiceSqliteErrorKind::Authority)?;
        Ok(())
    }

    fn validate_created(&self) -> Result<(), ServiceSqliteError> {
        let authority = self
            .authority
            .as_ref()
            .ok_or_else(|| ServiceSqliteError::new(ServiceSqliteErrorKind::Authority))?;
        authority.validate_for(&self.paths)?;
        let result = (|| {
            require_restore_condition(
                directory_identity(&self.directory)? == self.directory_identity,
                RestoreFailureKind::StagedChanged,
            )?;
            validate_stage_binding(&self.directory, &self.staged, self.staged_identity, 0)
        })();
        authority.validate_for(&self.paths)?;
        result
    }

    #[allow(dead_code)]
    pub(crate) fn paths(&self) -> &ServiceSqlitePaths {
        &self.paths
    }

    #[allow(dead_code)]
    pub(crate) fn metadata(&self) -> &ServiceDatabaseMetadata {
        &self.metadata
    }

    #[allow(dead_code)]
    pub(crate) const fn manifest_digest(&self) -> crate::BackupManifestSha256 {
        self.manifest_digest
    }

    #[allow(dead_code)]
    pub(crate) const fn artifact(&self) -> RestoreArtifactExpectation {
        self.artifact
    }

    pub(super) const fn live_artifact(&self) -> RestoreArtifactExpectation {
        self.live_artifact
    }

    #[allow(dead_code)]
    pub(crate) fn authority(&self) -> &WriterAuthority {
        self.authority
            .as_ref()
            .expect("staged restore retains authority until consumed")
    }

    pub(super) fn directory(&self) -> &File {
        &self.directory
    }

    pub(super) fn staged_file(&self) -> &File {
        &self.staged
    }

    #[allow(dead_code)]
    pub(crate) fn disarm_cleanup(&self) {
        self.armed.store(false, Ordering::Release);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Debug for NativeStagedServiceRestore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("NativeStagedServiceRestore([redacted])")
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for NativeStagedServiceRestore {
    fn drop(&mut self) {
        if self.armed.load(Ordering::Acquire) {
            let _ = cleanup_exact_stage(&self.directory, &self.staged, self.staged_identity);
        }
        if let Some(authority) = self.authority.as_mut() {
            let _ = authority.release();
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn directory_identity(directory: &File) -> Result<FileIdentity, ServiceSqliteError> {
    let status =
        fstat(directory).map_err(|source| restore_source(RestoreFailureKind::Layout, source))?;
    require_restore_condition(
        crate::native_metadata::secure_directory(
            FileType::from_raw_mode(status.st_mode).is_dir(),
            status.st_uid,
            geteuid().as_raw(),
            crate::native_metadata::mode(status.st_mode),
        ),
        RestoreFailureKind::Layout,
    )?;
    status_identity(&status)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn stage_identity(staged: &File) -> Result<FileIdentity, ServiceSqliteError> {
    let status =
        fstat(staged).map_err(|source| restore_source(RestoreFailureKind::CreateStage, source))?;
    validate_stage_status(&status, None)?;
    status_identity(&status)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn status_identity(status: &rustix::fs::Stat) -> Result<FileIdentity, ServiceSqliteError> {
    Ok(FileIdentity {
        device: crate::native_metadata::device(status.st_dev)
            .map_err(|_| restore_error(RestoreFailureKind::StagedChanged))?,
        inode: status.st_ino,
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn create_stage(directory: &File) -> Result<PendingStage, ServiceSqliteError> {
    let cleanup_directory = directory
        .try_clone()
        .map_err(|source| restore_source(RestoreFailureKind::CreateStage, source))?;
    let descriptor = openat(
        directory,
        STAGED_FILE_NAME,
        OFlags::RDWR
            | OFlags::CREATE
            | OFlags::EXCL
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | OFlags::NONBLOCK,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|source| restore_source(RestoreFailureKind::StageCollision, source))?;
    let file = File::from(descriptor);
    let mut pending = PendingStage {
        directory: cleanup_directory,
        staged: Some(file),
        identity: None,
        armed: true,
    };
    let status = fstat(
        pending
            .staged
            .as_ref()
            .expect("pending stage retains its file"),
    )
    .map_err(|source| restore_source(RestoreFailureKind::CreateStage, source))?;
    let identity = status_identity(&status)?;
    pending.identity = Some(identity);
    fchmod(
        pending
            .staged
            .as_ref()
            .expect("pending stage retains its file"),
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|source| restore_source(RestoreFailureKind::CreateStage, source))?;
    let confirmed = stage_identity(
        pending
            .staged
            .as_ref()
            .expect("pending stage retains its file"),
    )?;
    require_restore_condition(confirmed == identity, RestoreFailureKind::StagedChanged)?;
    Ok(pending)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct PendingStage {
    directory: File,
    staged: Option<File>,
    identity: Option<FileIdentity>,
    armed: bool,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl PendingStage {
    fn identity(&self) -> FileIdentity {
        self.identity
            .expect("successful stage creation records an exact identity")
    }

    fn into_file(mut self) -> File {
        self.armed = false;
        self.staged
            .take()
            .expect("successful stage creation retains its file")
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for PendingStage {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Some(staged) = self.staged.as_ref() else {
            return;
        };
        let identity = self.identity.or_else(|| {
            fstat(staged)
                .ok()
                .and_then(|status| status_identity(&status).ok())
        });
        if let Some(identity) = identity {
            let _ = cleanup_exact_stage(&self.directory, staged, identity);
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_closed_live(
    authority: &WriterAuthority,
    paths: &ServiceSqlitePaths,
    directory: &File,
) -> Result<RestoreArtifactExpectation, ServiceSqliteError> {
    authority.validate_for(paths)?;
    let live = openat(
        directory,
        radroots_runtime_paths::SERVICE_STATE_DATABASE_FILE_NAME,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|source| restore_source(RestoreFailureKind::LiveState, source))?;
    let status =
        fstat(&live).map_err(|source| restore_source(RestoreFailureKind::LiveState, source))?;
    require_restore_condition(
        crate::native_metadata::exact_regular_file(
            FileType::from_raw_mode(status.st_mode).is_file(),
            crate::native_metadata::link_count(status.st_nlink),
            status.st_uid,
            geteuid().as_raw(),
            crate::native_metadata::mode(status.st_mode),
        ),
        RestoreFailureKind::LiveState,
    )?;
    let length =
        u64::try_from(status.st_size).map_err(|_| restore_error(RestoreFailureKind::LiveState))?;
    require_restore_condition(
        crate::native_metadata::valid_artifact_length(length, None),
        RestoreFailureKind::LiveState,
    )?;
    let live = File::from(live);
    let digest = hash_exact(&live, length)?;
    let artifact = RestoreArtifactExpectation::new(
        crate::native_metadata::device(status.st_dev)
            .map_err(|_| restore_error(RestoreFailureKind::LiveState))?,
        status.st_ino,
        length,
        digest,
    )
    .map_err(|_| restore_error(RestoreFailureKind::LiveState))?;
    for name in [
        "state.sqlite-wal",
        "state.sqlite-shm",
        "state.sqlite-journal",
        BACKUP_FILE_NAME,
        MARKER_FILE_NAME,
        MARKER_NEXT_FILE_NAME,
    ] {
        require_absent(directory, name)?;
    }
    Ok(artifact)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_live_binding(
    directory: &File,
    expected: RestoreArtifactExpectation,
) -> Result<(), ServiceSqliteError> {
    let current = openat(
        directory,
        radroots_runtime_paths::SERVICE_STATE_DATABASE_FILE_NAME,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|source| restore_source(RestoreFailureKind::LiveState, source))?;
    let status =
        fstat(&current).map_err(|source| restore_source(RestoreFailureKind::LiveState, source))?;
    let device = crate::native_metadata::device(status.st_dev)
        .map_err(|_| restore_error(RestoreFailureKind::LiveState))?;
    let length =
        u64::try_from(status.st_size).map_err(|_| restore_error(RestoreFailureKind::LiveState))?;
    require_restore_condition(
        crate::all_constraints([
            crate::native_metadata::exact_regular_file(
                FileType::from_raw_mode(status.st_mode).is_file(),
                crate::native_metadata::link_count(status.st_nlink),
                status.st_uid,
                geteuid().as_raw(),
                crate::native_metadata::mode(status.st_mode),
            ),
            (device, status.st_ino) == (expected.device(), expected.inode()),
            crate::native_metadata::valid_artifact_length(length, Some(expected.byte_length())),
        ]),
        RestoreFailureKind::LiveState,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_stage_binding(
    directory: &File,
    held: &File,
    identity: FileIdentity,
    expected_length: u64,
) -> Result<(), ServiceSqliteError> {
    let current = openat(
        directory,
        STAGED_FILE_NAME,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|source| restore_source(RestoreFailureKind::StagedChanged, source))?;
    let held_status =
        fstat(held).map_err(|source| restore_source(RestoreFailureKind::StagedChanged, source))?;
    let current_status = fstat(&current)
        .map_err(|source| restore_source(RestoreFailureKind::StagedChanged, source))?;
    validate_stage_status(&held_status, Some(expected_length))?;
    validate_stage_status(&current_status, Some(expected_length))?;
    let held_identity = status_identity(&held_status)?;
    let current_identity = status_identity(&current_status)?;
    require_restore_condition(
        crate::native_metadata::identity_pair_matches(
            held_identity.device,
            held_identity.inode,
            current_identity.device,
            current_identity.inode,
            identity.device,
            identity.inode,
        ),
        RestoreFailureKind::StagedChanged,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_stage_status(
    status: &rustix::fs::Stat,
    expected_length: Option<u64>,
) -> Result<(), ServiceSqliteError> {
    let length = u64::try_from(status.st_size)
        .map_err(|_| restore_error(RestoreFailureKind::StagedChanged))?;
    let length_matches = match expected_length {
        Some(expected) => length == expected,
        None => true,
    };
    require_restore_condition(
        crate::all_constraints([
            crate::native_metadata::exact_regular_file(
                FileType::from_raw_mode(status.st_mode).is_file(),
                crate::native_metadata::link_count(status.st_nlink),
                status.st_uid,
                geteuid().as_raw(),
                crate::native_metadata::mode(status.st_mode),
            ),
            length_matches,
        ]),
        RestoreFailureKind::StagedChanged,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_absent(directory: &File, name: &str) -> Result<(), ServiceSqliteError> {
    match statat(directory, name, AtFlags::SYMLINK_NOFOLLOW) {
        Err(Errno::NOENT) => Ok(()),
        Ok(_) => Err(restore_error(RestoreFailureKind::RecoveryEvidence)),
        Err(source) => Err(restore_source(RestoreFailureKind::RecoveryEvidence, source)),
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn copy_exact(
    source: &File,
    destination: &File,
    expected_length: u64,
    expected_digest: [u8; 32],
    cancellation: &AtomicBool,
) -> Result<(), ServiceSqliteError> {
    let mut source = source
        .try_clone()
        .map_err(|source| restore_source(RestoreFailureKind::Copy, source))?;
    let mut destination = destination;
    source
        .seek(SeekFrom::Start(0))
        .map_err(|source| restore_source(RestoreFailureKind::Copy, source))?;
    destination
        .seek(SeekFrom::Start(0))
        .map_err(|source| restore_source(RestoreFailureKind::Copy, source))?;
    let mut remaining = expected_length;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    let mut hasher = Sha256::new();
    while remaining != 0 {
        check_cancel(cancellation)?;
        let requested = usize::try_from(remaining.min(COPY_BUFFER_BYTES as u64))
            .map_err(|_| restore_error(RestoreFailureKind::Copy))?;
        let read = source
            .read(&mut buffer[..requested])
            .map_err(|source| restore_source(RestoreFailureKind::Copy, source))?;
        if read == 0 {
            return Err(restore_error(RestoreFailureKind::Copy));
        }
        destination
            .write_all(&buffer[..read])
            .map_err(|source| restore_source(RestoreFailureKind::Copy, source))?;
        hasher.update(&buffer[..read]);
        remaining -= u64::try_from(read).map_err(|_| restore_error(RestoreFailureKind::Copy))?;
        test_blocking_phase(TEST_PHASE_MID_COPY, cancellation)?;
    }
    let mut extra = [0_u8; 1];
    if source
        .read(&mut extra)
        .map_err(|source| restore_source(RestoreFailureKind::Copy, source))?
        != 0
        || <[u8; 32]>::from(hasher.finalize()) != expected_digest
    {
        return Err(restore_error(RestoreFailureKind::Copy));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn test_blocking_phase(phase: u8, cancellation: &AtomicBool) -> Result<(), ServiceSqliteError> {
    #[cfg(test)]
    {
        TEST_STAGE_PHASE.store(phase, Ordering::Release);
        while TEST_STAGE_BLOCK_PHASE.load(Ordering::Acquire) == phase {
            if cancellation.load(Ordering::Acquire) {
                return Err(restore_error(RestoreFailureKind::Cancelled));
            }
            std::thread::yield_now();
        }
    }
    #[cfg(not(test))]
    let _ = (phase, cancellation);
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn sync_staged_file(file: &File, occurrence: u8) -> Result<(), ServiceSqliteError> {
    #[cfg(test)]
    if TEST_STAGE_FAIL_SYNC
        .compare_exchange(occurrence, 0, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        return Err(restore_source(
            RestoreFailureKind::SyncStaged,
            crate::failpoint::storage_full_error(),
        ));
    }
    #[cfg(not(test))]
    let _ = occurrence;
    file.sync_all()
        .map_err(|source| restore_source(RestoreFailureKind::SyncStaged, source))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn sync_stage_directory(directory: &File) -> Result<(), ServiceSqliteError> {
    #[cfg(test)]
    if TEST_STAGE_FAIL_SYNC
        .compare_exchange(3, 0, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        return Err(restore_source(
            RestoreFailureKind::SyncDirectory,
            crate::failpoint::storage_full_error(),
        ));
    }
    directory
        .sync_all()
        .map_err(|source| restore_source(RestoreFailureKind::SyncDirectory, source))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn hash_exact(file: &File, expected_length: u64) -> Result<[u8; 32], ServiceSqliteError> {
    let mut file = file
        .try_clone()
        .map_err(|source| restore_source(RestoreFailureKind::HashStaged, source))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|source| restore_source(RestoreFailureKind::HashStaged, source))?;
    let mut remaining = expected_length;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    let mut hasher = Sha256::new();
    while remaining != 0 {
        let requested = usize::try_from(remaining.min(COPY_BUFFER_BYTES as u64))
            .map_err(|_| restore_error(RestoreFailureKind::HashStaged))?;
        let read = file
            .read(&mut buffer[..requested])
            .map_err(|source| restore_source(RestoreFailureKind::HashStaged, source))?;
        if read == 0 {
            return Err(restore_error(RestoreFailureKind::HashStaged));
        }
        hasher.update(&buffer[..read]);
        remaining -=
            u64::try_from(read).map_err(|_| restore_error(RestoreFailureKind::HashStaged))?;
    }
    let mut extra = [0_u8; 1];
    if file
        .read(&mut extra)
        .map_err(|source| restore_source(RestoreFailureKind::HashStaged, source))?
        != 0
    {
        return Err(restore_error(RestoreFailureKind::HashStaged));
    }
    Ok(hasher.finalize().into())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_sqlite_header(file: &File) -> Result<(), ServiceSqliteError> {
    let mut file = file
        .try_clone()
        .map_err(|source| restore_source(RestoreFailureKind::Policy, source))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|source| restore_source(RestoreFailureKind::Policy, source))?;
    let mut header = [0_u8; 20];
    file.read_exact(&mut header)
        .map_err(|source| restore_source(RestoreFailureKind::Policy, source))?;
    require_restore_condition(
        crate::native_metadata::sqlite_header(&header),
        RestoreFailureKind::Policy,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn cleanup_exact_stage(
    directory: &File,
    held: &File,
    identity: FileIdentity,
) -> Result<(), ServiceSqliteError> {
    let current = match openat(
        directory,
        STAGED_FILE_NAME,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    ) {
        Ok(file) => file,
        Err(Errno::NOENT) => return Ok(()),
        Err(source) => return Err(restore_source(RestoreFailureKind::Cleanup, source)),
    };
    let held_status =
        fstat(held).map_err(|source| restore_source(RestoreFailureKind::Cleanup, source))?;
    let current_status =
        fstat(&current).map_err(|source| restore_source(RestoreFailureKind::Cleanup, source))?;
    if require_restore_condition(
        (
            status_identity(&held_status)?,
            status_identity(&current_status)?,
        ) == (identity, identity),
        RestoreFailureKind::Cleanup,
    )
    .is_err()
    {
        return Ok(());
    }
    unlinkat(directory, STAGED_FILE_NAME, AtFlags::empty())
        .map_err(|source| restore_source(RestoreFailureKind::Cleanup, source))?;
    directory
        .sync_all()
        .map_err(|source| restore_source(RestoreFailureKind::Cleanup, source))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct CancellationOnDrop {
    cancellation: Arc<AtomicBool>,
    armed: AtomicBool,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl CancellationOnDrop {
    fn new(cancellation: Arc<AtomicBool>) -> Self {
        Self {
            cancellation,
            armed: AtomicBool::new(true),
        }
    }

    fn disarm(&self) {
        self.armed.store(false, Ordering::Release);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for CancellationOnDrop {
    fn drop(&mut self) {
        if self.armed.load(Ordering::Acquire) {
            self.cancellation.store(true, Ordering::Release);
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RestoreFailureKind {
    Layout,
    LiveState,
    RecoveryEvidence,
    StageCollision,
    CreateStage,
    SourceChanged,
    Copy,
    HashStaged,
    StagedChanged,
    OpenStaged,
    Policy,
    SyncStaged,
    SyncDirectory,
    CloseStaged,
    Cleanup,
    Cancelled,
    Join,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct RestoreFailure {
    kind: RestoreFailureKind,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Debug for RestoreFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RestoreFailure")
            .field("kind", &self.kind)
            .field("source", &self.source.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Display for RestoreFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            RestoreFailureKind::Layout => "restore layout is invalid",
            RestoreFailureKind::LiveState => "live state is not closed and canonical",
            RestoreFailureKind::RecoveryEvidence => "prior restore evidence is present",
            RestoreFailureKind::StageCollision => "restore staging destination exists",
            RestoreFailureKind::CreateStage => "restore staging could not be created",
            RestoreFailureKind::SourceChanged => "verified backup binding changed",
            RestoreFailureKind::Copy => "verified backup copy failed",
            RestoreFailureKind::HashStaged => "restore staging hash failed",
            RestoreFailureKind::StagedChanged => "restore staging binding changed",
            RestoreFailureKind::OpenStaged => "restore staging could not be opened",
            RestoreFailureKind::Policy => "restore verification policy failed",
            RestoreFailureKind::SyncStaged => "restore staging sync failed",
            RestoreFailureKind::SyncDirectory => "restore directory sync failed",
            RestoreFailureKind::CloseStaged => "restore verification close failed",
            RestoreFailureKind::Cleanup => "restore staging cleanup failed",
            RestoreFailureKind::Cancelled => "restore staging was cancelled",
            RestoreFailureKind::Join => "restore staging worker failed",
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Error for RestoreFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn restore_error(kind: RestoreFailureKind) -> ServiceSqliteError {
    ServiceSqliteError::with_source(
        ServiceSqliteErrorKind::Restore,
        RestoreFailure { kind, source: None },
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn restore_source(
    kind: RestoreFailureKind,
    source: impl Error + Send + Sync + 'static,
) -> ServiceSqliteError {
    ServiceSqliteError::with_source(
        ServiceSqliteErrorKind::Restore,
        RestoreFailure {
            kind,
            source: Some(Box::new(source)),
        },
    )
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use core::num::{NonZeroU32, NonZeroU64};
    use std::{
        fs,
        os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt, symlink},
        path::{Path, PathBuf},
        process::Command,
    };

    use radroots_runtime_paths::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
    };
    use radroots_storage::event::SourceGeneration;
    use sha2::{Digest, Sha256};
    use sqlx::{ConnectOptions, Connection, sqlite::SqliteConnectOptions};

    use super::*;
    use crate::restore::finalize::{
        TEST_FINALIZE_BLOCK_PHASE, TEST_FINALIZE_PHASE, TEST_PHASE_AFTER_PREPARED,
        TEST_PHASE_BEFORE_PREPARED, TEST_PHASE_COMMIT_OWNED,
        reset_test_controls as reset_finalize_controls, test_finalize_with_failpoint,
        test_finalize_with_failure,
    };
    use crate::restore::{RestoreMarkerBinding, RestoreRecoveryMarker, RestoreRecoveryPhase};
    use crate::{
        BackupCreatedAtUnixMs, BackupMemberSha256, MigrationChecksum, MigrationDescriptor,
        SchemaObject, SchemaObjectKind, SchemaVersionCatalog, ServiceBackupManifest,
        ServiceDatabaseMetadata, ServiceSqliteApplicationId, ServiceSqliteConnectionOptions,
        ServiceSqliteHost, finalize_staged_restore, verify_backup_bundle,
    };

    static STAGE_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[test]
    fn restore_failure_inventory_is_complete_and_source_aware() {
        let cases = [
            (RestoreFailureKind::Layout, "restore layout is invalid"),
            (
                RestoreFailureKind::LiveState,
                "live state is not closed and canonical",
            ),
            (
                RestoreFailureKind::RecoveryEvidence,
                "prior restore evidence is present",
            ),
            (
                RestoreFailureKind::StageCollision,
                "restore staging destination exists",
            ),
            (
                RestoreFailureKind::CreateStage,
                "restore staging could not be created",
            ),
            (
                RestoreFailureKind::SourceChanged,
                "verified backup binding changed",
            ),
            (RestoreFailureKind::Copy, "verified backup copy failed"),
            (
                RestoreFailureKind::HashStaged,
                "restore staging hash failed",
            ),
            (
                RestoreFailureKind::StagedChanged,
                "restore staging binding changed",
            ),
            (
                RestoreFailureKind::OpenStaged,
                "restore staging could not be opened",
            ),
            (
                RestoreFailureKind::Policy,
                "restore verification policy failed",
            ),
            (
                RestoreFailureKind::SyncStaged,
                "restore staging sync failed",
            ),
            (
                RestoreFailureKind::SyncDirectory,
                "restore directory sync failed",
            ),
            (
                RestoreFailureKind::CloseStaged,
                "restore verification close failed",
            ),
            (
                RestoreFailureKind::Cleanup,
                "restore staging cleanup failed",
            ),
            (
                RestoreFailureKind::Cancelled,
                "restore staging was cancelled",
            ),
            (RestoreFailureKind::Join, "restore staging worker failed"),
        ];
        for (kind, message) in cases {
            let plain = RestoreFailure { kind, source: None };
            assert_eq!(plain.to_string(), message);
            assert!(plain.source().is_none());
            let sourced = RestoreFailure {
                kind,
                source: Some(Box::new(std::io::Error::other("private-cause"))),
            };
            assert_eq!(sourced.to_string(), message);
            assert!(sourced.source().is_some());
            let debug = format!("{sourced:?}");
            assert!(debug.contains("[redacted]"));
            assert!(!debug.contains("private-cause"));
            assert!(require_restore_condition(true, kind).is_ok());
            assert_eq!(
                require_restore_condition(false, kind)
                    .expect_err("false condition")
                    .kind(),
                ServiceSqliteErrorKind::Restore
            );
        }
    }

    #[test]
    fn read_only_policy_projection_rejects_each_independent_drift() {
        assert!(read_only_policy_matches(1, 0, 1, Some(0), Some("main")));
        assert!(!read_only_policy_matches(0, 0, 1, Some(0), Some("main")));
        assert!(!read_only_policy_matches(1, 1, 1, Some(0), Some("main")));
        assert!(!read_only_policy_matches(1, 0, 2, Some(0), Some("main")));
        assert!(!read_only_policy_matches(1, 0, 1, Some(1), Some("main")));
        assert!(!read_only_policy_matches(1, 0, 1, Some(0), Some("temp")));
        assert!(!read_only_policy_matches(1, 0, 0, None, None));
    }

    #[test]
    fn exact_copy_hash_and_cleanup_helpers_fail_closed() {
        let root = tempfile::tempdir().expect("root");
        let payload = b"restore-stage-payload";
        let digest: [u8; 32] = Sha256::digest(payload).into();
        let source_path = root.path().join("source.sqlite");
        fs::write(&source_path, payload).expect("source");
        let source = File::open(&source_path).expect("open source");

        let destination_path = root.path().join("destination.sqlite");
        let destination = std::fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&destination_path)
            .expect("open destination");
        let active = AtomicBool::new(false);
        copy_exact(
            &source,
            &destination,
            u64::try_from(payload.len()).expect("length"),
            digest,
            &active,
        )
        .expect("exact copy");
        assert_eq!(fs::read(&destination_path).expect("destination"), payload);
        assert_eq!(
            hash_exact(&destination, u64::try_from(payload.len()).expect("length"))
                .expect("exact hash"),
            digest
        );

        for (name, expected_length, expected_digest, cancelled) in [
            (
                "short.sqlite",
                u64::try_from(payload.len() + 1).expect("short length"),
                digest,
                false,
            ),
            (
                "long.sqlite",
                u64::try_from(payload.len() - 1).expect("long length"),
                digest,
                false,
            ),
            (
                "digest.sqlite",
                u64::try_from(payload.len()).expect("digest length"),
                [0; 32],
                false,
            ),
            (
                "cancelled.sqlite",
                u64::try_from(payload.len()).expect("cancelled length"),
                digest,
                true,
            ),
        ] {
            let path = root.path().join(name);
            let output = std::fs::OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(path)
                .expect("open negative destination");
            let cancellation = AtomicBool::new(cancelled);
            let error = copy_exact(
                &source,
                &output,
                expected_length,
                expected_digest,
                &cancellation,
            )
            .expect_err("copy must fail closed");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Restore);
        }

        assert_eq!(
            hash_exact(
                &destination,
                u64::try_from(payload.len() + 1).expect("short hash length")
            )
            .expect_err("short hash input")
            .kind(),
            ServiceSqliteErrorKind::Restore
        );
        assert_eq!(
            hash_exact(
                &destination,
                u64::try_from(payload.len() - 1).expect("long hash length")
            )
            .expect_err("long hash input")
            .kind(),
            ServiceSqliteErrorKind::Restore
        );

        let directory = File::open(root.path()).expect("open directory");
        let staged_path = root.path().join(STAGED_FILE_NAME);
        fs::write(&staged_path, b"owned-stage").expect("owned stage");
        let held = File::open(&staged_path).expect("open owned stage");
        let identity = status_identity(&fstat(&held).expect("owned status")).expect("identity");
        cleanup_exact_stage(&directory, &held, identity).expect("cleanup owned stage");
        assert!(!staged_path.exists());
        cleanup_exact_stage(&directory, &held, identity).expect("absent cleanup is idempotent");

        fs::write(&staged_path, b"original-stage").expect("original stage");
        let original = File::open(&staged_path).expect("open original stage");
        let original_identity =
            status_identity(&fstat(&original).expect("original status")).expect("identity");
        fs::rename(&staged_path, root.path().join("retained-original")).expect("retain original");
        fs::write(&staged_path, b"foreign-stage").expect("foreign replacement");
        cleanup_exact_stage(&directory, &original, original_identity)
            .expect("replacement is preserved");
        assert_eq!(
            fs::read(&staged_path).expect("replacement"),
            b"foreign-stage"
        );
    }

    struct Fixture {
        _root: tempfile::TempDir,
        paths: ServiceSqlitePaths,
        metadata: ServiceDatabaseMetadata,
        identity: ServiceDatabaseIdentity,
        migrations: MigrationCatalog,
        schema: SchemaCatalog,
        bundle: PathBuf,
        manifest: ServiceBackupManifest,
    }

    impl Fixture {
        async fn new() -> Self {
            let root = tempfile::tempdir().expect("root");
            let paths = paths(root.path());
            let state_directory = paths.state_database().parent().expect("state directory");
            fs::create_dir_all(state_directory).expect("create state directory");
            fs::set_permissions(state_directory, fs::Permissions::from_mode(0o700))
                .expect("state directory mode");
            let metadata = ServiceDatabaseMetadata::new(
                &paths,
                SourceGeneration::new([7; 32]).expect("generation"),
                NonZeroU32::new(1).expect("schema"),
                1_700_000_000_000,
                ServiceSqliteApplicationId::new(0x5244_5351).expect("application ID"),
            )
            .expect("metadata");
            let migrations = MigrationCatalog::new([]).expect("migration catalog");
            let digest = SchemaVersionCatalog::computed_digest(1, []).expect("schema digest");
            let version = SchemaVersionCatalog::new(1, [], digest).expect("schema version");
            let schema = SchemaCatalog::new(&migrations, [version]).expect("schema catalog");
            let mut authority = crate::initialize_database(
                &paths,
                OpenMode::Initialize,
                &metadata,
                &schema,
                |path| async move {
                    let options = SqliteConnectOptions::new()
                        .filename(path)
                        .create_if_missing(false)
                        .disable_statement_logging();
                    let connection = SqliteConnection::connect_with(&options).await?;
                    connection.close().await
                },
            )
            .await
            .expect("initialize");
            authority
                .release()
                .expect("release initialization authority");
            {
                let mut connection = SqliteConnection::connect_with(
                    &SqliteConnectOptions::new()
                        .filename(paths.state_database())
                        .create_if_missing(false)
                        .disable_statement_logging(),
                )
                .await
                .expect("open live database for WAL posture");
                sqlx::query("PRAGMA journal_mode = WAL")
                    .execute(&mut connection)
                    .await
                    .expect("set WAL posture");
                sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                    .execute(&mut connection)
                    .await
                    .expect("checkpoint WAL posture");
                connection.close().await.expect("close live database");
            }

            let bundle = root.path().join("verified-bundle");
            fs::create_dir(&bundle).expect("bundle");
            fs::set_permissions(&bundle, fs::Permissions::from_mode(0o700)).expect("bundle mode");
            let member = bundle.join(crate::BACKUP_STATE_MEMBER_NAME);
            fs::copy(paths.state_database(), &member).expect("copy bundle member");
            fs::set_permissions(&member, fs::Permissions::from_mode(0o600)).expect("member mode");
            let bytes = fs::read(&member).expect("member bytes");
            let manifest = ServiceBackupManifest::from_capture(
                &metadata,
                BackupCreatedAtUnixMs::new(1_700_000_000_123).expect("capture time"),
                u64::try_from(bytes.len()).expect("member length"),
                BackupMemberSha256::from_bytes(Sha256::digest(&bytes).into()),
            )
            .expect("manifest");
            let identity = metadata.identity();
            Self {
                _root: root,
                paths,
                metadata,
                identity,
                migrations,
                schema,
                bundle,
                manifest,
            }
        }

        fn proof(&self) -> VerifiedServiceBackup {
            verify_backup_bundle(
                self.manifest.canonical_bytes(),
                self.manifest.digest(),
                &self.bundle,
                &self.identity,
                NonZeroU64::new(16 * 1024 * 1024).expect("member limit"),
            )
            .expect("verified backup")
        }

        fn staged_path(&self) -> PathBuf {
            self.paths
                .state_database()
                .parent()
                .expect("state directory")
                .join(STAGED_FILE_NAME)
        }

        fn refresh_manifest(&mut self) {
            let bytes =
                fs::read(self.bundle.join(crate::BACKUP_STATE_MEMBER_NAME)).expect("member bytes");
            self.manifest = ServiceBackupManifest::from_capture(
                &self.metadata,
                BackupCreatedAtUnixMs::new(1_700_000_000_123).expect("capture time"),
                u64::try_from(bytes.len()).expect("member length"),
                BackupMemberSha256::from_bytes(Sha256::digest(&bytes).into()),
            )
            .expect("manifest");
        }
    }

    fn manifest_for(metadata: &ServiceDatabaseMetadata) -> ServiceBackupManifest {
        ServiceBackupManifest::from_capture(
            metadata,
            BackupCreatedAtUnixMs::new(1_700_000_000_123).expect("capture time"),
            4_096,
            BackupMemberSha256::from_bytes([9; 32]),
        )
        .expect("manifest")
    }

    #[tokio::test(flavor = "current_thread")]
    async fn staging_intent_rejects_each_independent_identity_and_catalog_drift() {
        let fixture = Fixture::new().await;
        assert!(stage_intent_matches(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            &fixture.metadata,
            &fixture.manifest,
        ));

        let alternate_paths = paths_for(fixture._root.path(), "rhi", "secondary");
        let alternate_identity = ServiceDatabaseIdentity::new(
            &alternate_paths,
            fixture.metadata.source_generation(),
            NonZeroU32::new(1).expect("schema"),
            fixture.metadata.application_id(),
        );
        assert!(!stage_intent_matches(
            &fixture.paths,
            &alternate_identity,
            &fixture.migrations,
            &fixture.schema,
            &fixture.metadata,
            &fixture.manifest,
        ));

        let migration = MigrationDescriptor::sql(
            2,
            "add_stage_probe",
            "SELECT 1",
            MigrationChecksum::for_sql("SELECT 1"),
        )
        .expect("migration");
        let migrations_v2 = MigrationCatalog::new([migration]).expect("v2 migrations");
        let v1_digest = SchemaVersionCatalog::computed_digest(1, []).expect("v1 digest");
        let v1 = SchemaVersionCatalog::new(1, [], v1_digest).expect("v1 schema");
        let v2_digest = SchemaVersionCatalog::computed_digest(2, []).expect("v2 digest");
        let v2 = SchemaVersionCatalog::new(2, [], v2_digest).expect("v2 schema");
        let schema_v2 = SchemaCatalog::new(&migrations_v2, [v1, v2]).expect("v2 catalog");
        let identity_v2 = ServiceDatabaseIdentity::new(
            &fixture.paths,
            fixture.metadata.source_generation(),
            NonZeroU32::new(2).expect("schema"),
            fixture.metadata.application_id(),
        );
        assert!(stage_intent_matches(
            &fixture.paths,
            &identity_v2,
            &migrations_v2,
            &schema_v2,
            &fixture.metadata,
            &fixture.manifest,
        ));
        assert!(!stage_intent_matches(
            &fixture.paths,
            &identity_v2,
            &fixture.migrations,
            &fixture.schema,
            &fixture.metadata,
            &fixture.manifest,
        ));
        assert!(!stage_intent_matches(
            &fixture.paths,
            &identity_v2,
            &migrations_v2,
            &fixture.schema,
            &fixture.metadata,
            &fixture.manifest,
        ));

        let generation = SourceGeneration::new([8; 32]).expect("alternate generation");
        let alternate_application =
            ServiceSqliteApplicationId::new(0x5244_5352).expect("alternate application ID");
        let identity_drifts = [
            ServiceDatabaseMetadata::from_verified_backup(
                fixture.metadata.service().clone(),
                fixture.metadata.instance().clone(),
                generation,
                fixture.metadata.state_schema_version(),
                fixture.metadata.created_at_unix_ms(),
                fixture.metadata.application_id(),
            )
            .expect("generation drift"),
            ServiceDatabaseMetadata::from_verified_backup(
                fixture.metadata.service().clone(),
                fixture.metadata.instance().clone(),
                fixture.metadata.source_generation(),
                fixture.metadata.state_schema_version(),
                fixture.metadata.created_at_unix_ms(),
                alternate_application,
            )
            .expect("application drift"),
            ServiceDatabaseMetadata::from_verified_backup(
                ServiceId::new("rhi").expect("service"),
                fixture.metadata.instance().clone(),
                fixture.metadata.source_generation(),
                fixture.metadata.state_schema_version(),
                fixture.metadata.created_at_unix_ms(),
                fixture.metadata.application_id(),
            )
            .expect("service drift"),
            ServiceDatabaseMetadata::from_verified_backup(
                fixture.metadata.service().clone(),
                InstanceId::new("secondary").expect("instance"),
                fixture.metadata.source_generation(),
                fixture.metadata.state_schema_version(),
                fixture.metadata.created_at_unix_ms(),
                fixture.metadata.application_id(),
            )
            .expect("instance drift"),
        ];
        for metadata in &identity_drifts {
            let manifest = manifest_for(metadata);
            assert!(!stage_intent_matches(
                &fixture.paths,
                &fixture.identity,
                &fixture.migrations,
                &fixture.schema,
                metadata,
                &manifest,
            ));
        }

        let metadata_v2 = ServiceDatabaseMetadata::from_verified_backup(
            fixture.metadata.service().clone(),
            fixture.metadata.instance().clone(),
            fixture.metadata.source_generation(),
            NonZeroU32::new(2).expect("schema"),
            fixture.metadata.created_at_unix_ms(),
            fixture.metadata.application_id(),
        )
        .expect("v2 metadata");
        let manifest_v2 = manifest_for(&metadata_v2);
        assert!(!stage_intent_matches(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            &metadata_v2,
            &manifest_v2,
        ));

        for manifest in [
            manifest_for(&identity_drifts[0]),
            manifest_for(&identity_drifts[2]),
            manifest_for(&identity_drifts[3]),
            manifest_v2,
        ] {
            assert!(!stage_intent_matches(
                &fixture.paths,
                &fixture.identity,
                &fixture.migrations,
                &fixture.schema,
                &fixture.metadata,
                &manifest,
            ));
        }
    }

    fn paths(root: &Path) -> ServiceSqlitePaths {
        paths_for(root, "myc", "primary")
    }

    fn paths_for(root: &Path, service: &str, instance: &str) -> ServiceSqlitePaths {
        let context = RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(root.to_path_buf()),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new(service).expect("service"),
            InstanceId::new(instance).expect("instance"),
        )
        .expect("runtime context");
        ServiceSqlitePaths::from_runtime_context(&context).expect("SQLite paths")
    }

    fn reset_test_controls() {
        TEST_STAGE_PHASE.store(0, Ordering::Release);
        TEST_STAGE_BLOCK_PHASE.store(0, Ordering::Release);
        TEST_STAGE_FAIL_SYNC.store(0, Ordering::Release);
        TEST_STAGE_PANIC_WORKER.store(false, Ordering::Release);
    }

    async fn wait_for_phase(phase: u8) {
        for _ in 0..100_000 {
            if TEST_STAGE_PHASE.load(Ordering::Acquire) == phase {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("restore staging did not reach phase {phase}");
    }

    async fn wait_for_cleanup(paths: &ServiceSqlitePaths, staged_path: &Path) {
        for _ in 0..100_000 {
            match WriterAuthority::acquire(paths, OpenMode::ReadWriteExisting) {
                Ok(Some(mut authority)) => {
                    authority.release().expect("release cleanup probe");
                    assert!(!staged_path.exists());
                    return;
                }
                Ok(None) | Err(_) => tokio::task::yield_now().await,
            }
        }
        panic!("restore staging cleanup did not release authority");
    }

    async fn wait_for_finalize_phase(phase: u8) {
        for _ in 0..100_000 {
            if TEST_FINALIZE_PHASE.load(Ordering::Acquire) == phase {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("restore finalization did not reach phase {phase}");
    }

    fn recovery_path(fixture: &Fixture, name: &str) -> PathBuf {
        fixture.staged_path().with_file_name(name)
    }

    fn marker_phase(fixture: &Fixture) -> RestoreRecoveryPhase {
        let bytes = fs::read(recovery_path(fixture, MARKER_FILE_NAME)).expect("marker bytes");
        RestoreRecoveryMarker::from_canonical_bytes(&bytes)
            .expect("canonical marker")
            .phase()
    }

    async fn wait_for_replacement_install(fixture: &Fixture) {
        for _ in 0..100_000 {
            if marker_phase_if_present(fixture) == Some(RestoreRecoveryPhase::ReplacementInstalled)
                && WriterAuthority::acquire(&fixture.paths, OpenMode::ReadWriteExisting).is_ok()
            {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("restore finalization worker did not complete");
    }

    fn marker_phase_if_present(fixture: &Fixture) -> Option<RestoreRecoveryPhase> {
        let bytes = fs::read(recovery_path(fixture, MARKER_FILE_NAME)).ok()?;
        RestoreRecoveryMarker::from_canonical_bytes(&bytes)
            .ok()
            .map(|marker| marker.phase())
    }

    fn spawn_stage(
        fixture: &Fixture,
    ) -> tokio::task::JoinHandle<Result<StagedServiceRestore, ServiceSqliteError>> {
        let paths = fixture.paths.clone();
        let identity = fixture.identity.clone();
        let migrations = fixture.migrations.clone();
        let schema = fixture.schema.clone();
        let proof = fixture.proof();
        tokio::spawn(async move {
            stage_verified_restore(&paths, &identity, &migrations, &schema, proof).await
        })
    }

    #[tokio::test(flavor = "current_thread")]
    async fn success_reverifies_exact_stage_preserves_live_and_drop_cleans() {
        let fixture = Fixture::new().await;
        let live_before = fs::metadata(fixture.paths.state_database()).expect("live metadata");
        let live_bytes = fs::read(fixture.paths.state_database()).expect("live bytes");
        let bundle_bytes =
            fs::read(fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME)).expect("bundle bytes");

        let staged = stage_verified_restore(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            fixture.proof(),
        )
        .await
        .unwrap_or_else(|error| {
            panic!(
                "stage restore: {:?} / {:?}",
                error.kind(),
                error.source().map(ToString::to_string)
            )
        });
        assert_eq!(format!("{staged:?}"), "StagedServiceRestore([redacted])");
        let native = staged.inner.as_ref().expect("native staged capability");
        assert_eq!(native.metadata(), &fixture.metadata);
        assert_eq!(
            native.artifact().byte_length(),
            u64::try_from(bundle_bytes.len()).expect("bundle length")
        );
        let expected_digest: [u8; 32] = Sha256::digest(&bundle_bytes).into();
        assert_eq!(native.artifact().sha256(), expected_digest);
        assert_eq!(
            fs::read(fixture.staged_path()).expect("stage bytes"),
            bundle_bytes
        );
        assert_eq!(
            fs::metadata(fixture.staged_path())
                .expect("stage metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::read(fixture.paths.state_database()).expect("live after"),
            live_bytes
        );
        let live_after = fs::metadata(fixture.paths.state_database()).expect("live metadata after");
        assert_eq!(
            (live_before.dev(), live_before.ino()),
            (live_after.dev(), live_after.ino())
        );
        assert_eq!(live_before.mode(), live_after.mode());
        assert_eq!(
            (live_before.mtime(), live_before.mtime_nsec()),
            (live_after.mtime(), live_after.mtime_nsec())
        );
        assert_eq!(
            fs::read(fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME))
                .expect("source remains readable"),
            bundle_bytes
        );
        assert!(WriterAuthority::acquire(&fixture.paths, OpenMode::ReadWriteExisting).is_err());
        for name in [BACKUP_FILE_NAME, MARKER_FILE_NAME, MARKER_NEXT_FILE_NAME] {
            assert!(!fixture.staged_path().with_file_name(name).exists());
        }
        drop(staged);
        assert!(!fixture.staged_path().exists());
        let mut reacquired = WriterAuthority::acquire(&fixture.paths, OpenMode::ReadWriteExisting)
            .expect("reacquire")
            .expect("authority");
        reacquired.release().expect("release");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn active_writer_and_every_recovery_collision_fail_closed() {
        let fixture = Fixture::new().await;
        let authority = WriterAuthority::acquire(&fixture.paths, OpenMode::ReadWriteExisting)
            .expect("authority")
            .expect("writer");
        let error = stage_verified_restore(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            fixture.proof(),
        )
        .await
        .expect_err("active writer rejection");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
        drop(authority);

        let inspection = ServiceSqliteHost::open_read_only_inspection(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            ServiceSqliteConnectionOptions::default(),
        )
        .await
        .expect("inspection host");
        let error = stage_verified_restore(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            fixture.proof(),
        )
        .await
        .expect_err("inspection rejection");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
        inspection.close().await.expect("close inspection");

        for name in [
            STAGED_FILE_NAME,
            BACKUP_FILE_NAME,
            MARKER_FILE_NAME,
            MARKER_NEXT_FILE_NAME,
            "state.sqlite-wal",
            "state.sqlite-shm",
            "state.sqlite-journal",
        ] {
            let collision = fixture.staged_path().with_file_name(name);
            fs::write(&collision, b"collision").expect("collision");
            fs::set_permissions(&collision, fs::Permissions::from_mode(0o600))
                .expect("collision mode");
            let error = stage_verified_restore(
                &fixture.paths,
                &fixture.identity,
                &fixture.migrations,
                &fixture.schema,
                fixture.proof(),
            )
            .await
            .expect_err("collision rejection");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Restore);
            assert_eq!(
                fs::read(&collision).expect("preserved collision"),
                b"collision"
            );
            fs::remove_file(collision).expect("remove collision");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn retained_source_tamper_and_wrong_intent_never_publish_a_stage() {
        let fixture = Fixture::new().await;
        let wrong = ServiceDatabaseIdentity::new(
            &fixture.paths,
            SourceGeneration::new([8; 32]).expect("generation"),
            NonZeroU32::new(1).expect("schema"),
            fixture.identity.application_id(),
        );
        let error = stage_verified_restore(
            &fixture.paths,
            &wrong,
            &fixture.migrations,
            &fixture.schema,
            fixture.proof(),
        )
        .await
        .expect_err("intent rejection");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Metadata);
        assert!(!fixture.staged_path().exists());

        let proof = fixture.proof();
        let member = fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME);
        let mut bytes = fs::read(&member).expect("member");
        bytes[100] ^= 0x01;
        fs::write(&member, bytes).expect("tamper in place");
        let error = stage_verified_restore(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            proof,
        )
        .await
        .expect_err("tamper rejection");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Restore);
        assert!(!fixture.staged_path().exists());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn stage_collision_types_are_never_followed_or_removed() {
        let fixture = Fixture::new().await;
        let staged_path = fixture.staged_path();

        fs::create_dir(&staged_path).expect("directory collision");
        assert!(
            stage_verified_restore(
                &fixture.paths,
                &fixture.identity,
                &fixture.migrations,
                &fixture.schema,
                fixture.proof(),
            )
            .await
            .is_err()
        );
        assert!(staged_path.is_dir());
        fs::remove_dir(&staged_path).expect("remove directory collision");

        symlink(fixture.paths.state_database(), &staged_path).expect("symlink collision");
        assert!(
            stage_verified_restore(
                &fixture.paths,
                &fixture.identity,
                &fixture.migrations,
                &fixture.schema,
                fixture.proof(),
            )
            .await
            .is_err()
        );
        assert!(
            fs::symlink_metadata(&staged_path)
                .expect("symlink")
                .file_type()
                .is_symlink()
        );
        fs::remove_file(&staged_path).expect("remove symlink collision");

        fs::hard_link(fixture.paths.state_database(), &staged_path).expect("hardlink collision");
        assert!(
            stage_verified_restore(
                &fixture.paths,
                &fixture.identity,
                &fixture.migrations,
                &fixture.schema,
                fixture.proof(),
            )
            .await
            .is_err()
        );
        assert_eq!(fs::metadata(&staged_path).expect("hardlink").nlink(), 2);
        fs::remove_file(&staged_path).expect("remove hardlink collision");

        assert!(
            Command::new("mkfifo")
                .arg(&staged_path)
                .status()
                .expect("mkfifo")
                .success()
        );
        assert!(
            stage_verified_restore(
                &fixture.paths,
                &fixture.identity,
                &fixture.migrations,
                &fixture.schema,
                fixture.proof(),
            )
            .await
            .is_err()
        );
        assert!(
            fs::symlink_metadata(&staged_path)
                .expect("FIFO")
                .file_type()
                .is_fifo()
        );
        fs::remove_file(&staged_path).expect("remove FIFO collision");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancellation_at_every_owned_phase_cleans_before_releasing_authority() {
        let _serial = STAGE_TEST_LOCK.lock().await;
        for phase in [
            TEST_PHASE_BEFORE_CREATE,
            TEST_PHASE_CREATED,
            TEST_PHASE_MID_COPY,
            TEST_PHASE_POST_COPY,
            TEST_PHASE_POLICY,
            TEST_PHASE_METADATA,
            TEST_PHASE_HISTORY,
            TEST_PHASE_INTEGRITY,
            TEST_PHASE_PRE_FINAL_SYNC,
        ] {
            reset_test_controls();
            let fixture = Fixture::new().await;
            TEST_STAGE_BLOCK_PHASE.store(phase, Ordering::Release);
            let caller = spawn_stage(&fixture);
            wait_for_phase(phase).await;
            caller.abort();
            let _ = caller.await;
            wait_for_cleanup(&fixture.paths, &fixture.staged_path()).await;
            reset_test_controls();
            let retry = stage_verified_restore(
                &fixture.paths,
                &fixture.identity,
                &fixture.migrations,
                &fixture.schema,
                fixture.proof(),
            )
            .await
            .expect("retry after cancellation");
            drop(retry);
        }
        reset_test_controls();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn storage_full_sync_and_join_failures_cleanup_and_allow_retry() {
        let _serial = STAGE_TEST_LOCK.lock().await;
        for failure in [1, 2, 3] {
            reset_test_controls();
            let fixture = Fixture::new().await;
            TEST_STAGE_FAIL_SYNC.store(failure, Ordering::Release);
            let error = stage_verified_restore(
                &fixture.paths,
                &fixture.identity,
                &fixture.migrations,
                &fixture.schema,
                fixture.proof(),
            )
            .await
            .expect_err("sync failure");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Restore);
            let storage = error
                .source()
                .and_then(Error::source)
                .and_then(|source| source.downcast_ref::<std::io::Error>())
                .expect("storage-full cause");
            assert_eq!(storage.kind(), std::io::ErrorKind::StorageFull);
            wait_for_cleanup(&fixture.paths, &fixture.staged_path()).await;
        }

        reset_test_controls();
        let fixture = Fixture::new().await;
        TEST_STAGE_PANIC_WORKER.store(true, Ordering::Release);
        let error = stage_verified_restore(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            fixture.proof(),
        )
        .await
        .expect_err("join failure");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Restore);
        reset_test_controls();
        wait_for_cleanup(&fixture.paths, &fixture.staged_path()).await;
        let retry = stage_verified_restore(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            fixture.proof(),
        )
        .await
        .expect("retry after worker failure");
        drop(retry);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn staged_replacement_is_rejected_and_foreign_inode_is_preserved() {
        let _serial = STAGE_TEST_LOCK.lock().await;
        reset_test_controls();
        let fixture = Fixture::new().await;
        TEST_STAGE_BLOCK_PHASE.store(TEST_PHASE_POST_COPY, Ordering::Release);
        let caller = spawn_stage(&fixture);
        wait_for_phase(TEST_PHASE_POST_COPY).await;
        let foreign = fixture.staged_path().with_file_name("foreign-stage");
        fs::write(&foreign, b"foreign").expect("foreign stage");
        fs::set_permissions(&foreign, fs::Permissions::from_mode(0o600)).expect("foreign mode");
        fs::rename(&foreign, fixture.staged_path()).expect("replace stage");
        TEST_STAGE_BLOCK_PHASE.store(0, Ordering::Release);
        let error = caller
            .await
            .expect("caller task")
            .expect_err("replacement rejection");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Restore);
        assert_eq!(
            fs::read(fixture.staged_path()).expect("preserved replacement"),
            b"foreign"
        );
        fs::remove_file(fixture.staged_path()).expect("remove replacement");
        reset_test_controls();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn writer_lock_replacement_has_authority_precedence_and_cleans_stage() {
        let _serial = STAGE_TEST_LOCK.lock().await;
        reset_test_controls();
        let fixture = Fixture::new().await;
        TEST_STAGE_BLOCK_PHASE.store(TEST_PHASE_POST_COPY, Ordering::Release);
        let caller = spawn_stage(&fixture);
        wait_for_phase(TEST_PHASE_POST_COPY).await;
        let old_lock = fixture.paths.state_lock().with_file_name("state.lock.old");
        fs::rename(fixture.paths.state_lock(), &old_lock).expect("retain old lock inode");
        fs::write(fixture.paths.state_lock(), b"").expect("replacement lock");
        fs::set_permissions(
            fixture.paths.state_lock(),
            fs::Permissions::from_mode(0o600),
        )
        .expect("replacement lock mode");
        TEST_STAGE_BLOCK_PHASE.store(0, Ordering::Release);
        let error = caller
            .await
            .expect("caller task")
            .expect_err("authority drift");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
        assert!(!fixture.staged_path().exists());
        assert!(fixture.paths.state_lock().exists());
        fs::remove_file(old_lock).expect("remove old lock");
        reset_test_controls();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn older_valid_prefix_is_accepted_without_migrating_the_stage() {
        const ADD_ALPHA: &str = "CREATE TABLE alpha (id INTEGER PRIMARY KEY)";
        let fixture = Fixture::new().await;
        let migration = MigrationDescriptor::sql(
            2,
            "add_alpha",
            ADD_ALPHA,
            MigrationChecksum::for_sql(ADD_ALPHA),
        )
        .expect("migration");
        let migrations = MigrationCatalog::new([migration]).expect("migrations");
        let v1_digest = SchemaVersionCatalog::computed_digest(1, []).expect("v1 digest");
        let v1 = SchemaVersionCatalog::new(1, [], v1_digest).expect("v1");
        let alpha = SchemaObject::new(
            SchemaObjectKind::Table,
            "alpha",
            "alpha",
            ADD_ALPHA,
            SchemaObject::computed_digest(SchemaObjectKind::Table, "alpha", "alpha", ADD_ALPHA)
                .expect("alpha digest"),
        )
        .expect("alpha object");
        let v2_digest =
            SchemaVersionCatalog::computed_digest(2, [alpha.clone()]).expect("v2 digest");
        let v2 = SchemaVersionCatalog::new(2, [alpha], v2_digest).expect("v2");
        let schema = SchemaCatalog::new(&migrations, [v1, v2]).expect("schema");
        let expected = ServiceDatabaseIdentity::new(
            &fixture.paths,
            fixture.identity.source_generation(),
            NonZeroU32::new(2).expect("supported schema"),
            fixture.identity.application_id(),
        );
        let proof = verify_backup_bundle(
            fixture.manifest.canonical_bytes(),
            fixture.manifest.digest(),
            &fixture.bundle,
            &expected,
            NonZeroU64::new(16 * 1024 * 1024).expect("limit"),
        )
        .expect("older proof");
        let staged = stage_verified_restore(&fixture.paths, &expected, &migrations, &schema, proof)
            .await
            .expect("older prefix stage");
        assert_eq!(
            staged
                .inner
                .as_ref()
                .expect("native")
                .metadata()
                .state_schema_version()
                .get(),
            1
        );
        drop(staged);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn schema_catalog_and_migration_ledger_drift_are_rejected() {
        let mut schema_drift = Fixture::new().await;
        {
            let mut connection = SqliteConnection::connect_with(
                &SqliteConnectOptions::new()
                    .filename(schema_drift.bundle.join(crate::BACKUP_STATE_MEMBER_NAME))
                    .create_if_missing(false)
                    .disable_statement_logging(),
            )
            .await
            .expect("open bundle");
            sqlx::query("CREATE TABLE unexpected (id INTEGER PRIMARY KEY)")
                .execute(&mut connection)
                .await
                .expect("add unexpected table");
            connection.close().await.expect("close bundle");
        }
        schema_drift.refresh_manifest();
        let error = stage_verified_restore(
            &schema_drift.paths,
            &schema_drift.identity,
            &schema_drift.migrations,
            &schema_drift.schema,
            schema_drift.proof(),
        )
        .await
        .expect_err("schema drift");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Integrity);
        assert!(!schema_drift.staged_path().exists());

        let mut ledger_drift = Fixture::new().await;
        {
            let mut connection = SqliteConnection::connect_with(
                &SqliteConnectOptions::new()
                    .filename(ledger_drift.bundle.join(crate::BACKUP_STATE_MEMBER_NAME))
                    .create_if_missing(false)
                    .disable_statement_logging(),
            )
            .await
            .expect("open bundle");
            sqlx::query(
                "INSERT INTO schema_migrations (
                        version, name, checksum, applied_at_unix_s,
                        service_version, service_commit, lib_revision, rust_version,
                        target, feature_profile, config_contract_version,
                        state_contract_version, admin_contract_version,
                        status_contract_version, provider_contract_version
                     ) VALUES (
                        3, 'unexpected', zeroblob(32), 1,
                        '0.1.0', '0123456789abcdef0123456789abcdef01234567',
                        '89abcdef0123456789abcdef0123456789abcdef', '1.97.1',
                        'x86_64-unknown-linux-gnu', 'test', 1, 1, 1, 1, 1
                     )",
            )
            .execute(&mut connection)
            .await
            .expect("insert ledger drift");
            connection.close().await.expect("close bundle");
        }
        ledger_drift.refresh_manifest();
        let error = stage_verified_restore(
            &ledger_drift.paths,
            &ledger_drift.identity,
            &ledger_drift.migrations,
            &ledger_drift.schema,
            ledger_drift.proof(),
        )
        .await
        .expect_err("ledger drift");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Migration);
        assert!(!ledger_drift.staged_path().exists());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn atomic_finalization_retains_old_live_installs_stage_and_requires_recovery_open() {
        let _serial = STAGE_TEST_LOCK.lock().await;
        reset_finalize_controls();
        let fixture = Fixture::new().await;
        let old_live = fs::read(fixture.paths.state_database()).expect("old live");
        let replacement =
            fs::read(fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME)).expect("replacement");
        let old_metadata = fs::metadata(fixture.paths.state_database()).expect("old metadata");
        let staged = stage_verified_restore(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            fixture.proof(),
        )
        .await
        .expect("stage");

        finalize_staged_restore(staged).await.expect("finalize");

        let backup = recovery_path(&fixture, BACKUP_FILE_NAME);
        assert_eq!(fs::read(&backup).expect("retained old live"), old_live);
        assert_eq!(
            fs::read(fixture.paths.state_database()).expect("installed live"),
            replacement
        );
        let backup_metadata = fs::metadata(&backup).expect("backup metadata");
        assert_eq!(
            (old_metadata.dev(), old_metadata.ino()),
            (backup_metadata.dev(), backup_metadata.ino())
        );
        assert!(!fixture.staged_path().exists());
        assert!(!recovery_path(&fixture, MARKER_NEXT_FILE_NAME).exists());

        let binding = RestoreMarkerBinding::load(&fixture.paths)
            .expect("load marker")
            .expect("marker present");
        assert_eq!(
            binding.marker().phase(),
            RestoreRecoveryPhase::ReplacementInstalled
        );
        assert_eq!(binding.marker().live(), binding.marker().backup());
        assert_ne!(
            (
                binding.marker().live().device(),
                binding.marker().live().inode()
            ),
            (
                binding.marker().staged().device(),
                binding.marker().staged().inode()
            )
        );

        let read_only = crate::open::open_existing_connection_pool(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            OpenMode::ReadOnlyInspection,
            ServiceSqliteConnectionOptions::reviewed(),
        )
        .await
        .err()
        .expect("read-only open must not recover");
        assert_eq!(read_only.kind(), ServiceSqliteErrorKind::Recovery);
        let initialize = crate::initialize_database(
            &fixture.paths,
            OpenMode::Initialize,
            &fixture.metadata,
            &fixture.schema,
            |_| async { Ok::<(), std::io::Error>(()) },
        )
        .await
        .expect_err("unresolved recovery must reject initialization");
        assert_eq!(initialize.kind(), ServiceSqliteErrorKind::Recovery);

        let writable = crate::open::open_existing_connection_pool(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            OpenMode::ReadWriteExisting,
            ServiceSqliteConnectionOptions::reviewed(),
        )
        .await
        .expect("writable open must recover exact finalization evidence");
        writable.close().await.expect("close recovered pool");
        assert_eq!(
            fs::read(fixture.paths.state_database()).expect("recovered live database"),
            replacement
        );
        for name in [
            BACKUP_FILE_NAME,
            MARKER_FILE_NAME,
            MARKER_NEXT_FILE_NAME,
            STAGED_FILE_NAME,
        ] {
            assert!(!recovery_path(&fixture, name).exists(), "unexpected {name}");
        }
        reset_finalize_controls();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn finalization_cancellation_is_clean_before_handoff_and_owned_after_handoff() {
        let _serial = STAGE_TEST_LOCK.lock().await;

        reset_finalize_controls();
        let before = Fixture::new().await;
        let old_live = fs::read(before.paths.state_database()).expect("old live");
        let staged = stage_verified_restore(
            &before.paths,
            &before.identity,
            &before.migrations,
            &before.schema,
            before.proof(),
        )
        .await
        .expect("stage");
        TEST_FINALIZE_BLOCK_PHASE.store(TEST_PHASE_BEFORE_PREPARED, Ordering::Release);
        let caller = tokio::spawn(finalize_staged_restore(staged));
        wait_for_finalize_phase(TEST_PHASE_BEFORE_PREPARED).await;
        caller.abort();
        let _ = caller.await;
        wait_for_cleanup(&before.paths, &before.staged_path()).await;
        assert_eq!(
            fs::read(before.paths.state_database()).expect("preserved live"),
            old_live
        );
        assert!(!recovery_path(&before, MARKER_FILE_NAME).exists());

        for phase in [TEST_PHASE_COMMIT_OWNED, TEST_PHASE_AFTER_PREPARED] {
            reset_finalize_controls();
            let after = Fixture::new().await;
            let staged = stage_verified_restore(
                &after.paths,
                &after.identity,
                &after.migrations,
                &after.schema,
                after.proof(),
            )
            .await
            .expect("stage");
            TEST_FINALIZE_BLOCK_PHASE.store(phase, Ordering::Release);
            let caller = tokio::spawn(finalize_staged_restore(staged));
            wait_for_finalize_phase(phase).await;
            if phase == TEST_PHASE_AFTER_PREPARED {
                assert_eq!(marker_phase(&after), RestoreRecoveryPhase::Prepared);
            } else {
                assert!(!recovery_path(&after, MARKER_FILE_NAME).exists());
            }
            caller.abort();
            let _ = caller.await;
            assert!(WriterAuthority::acquire(&after.paths, OpenMode::ReadWriteExisting).is_err());
            TEST_FINALIZE_BLOCK_PHASE.store(0, Ordering::Release);
            wait_for_replacement_install(&after).await;
            assert_eq!(
                marker_phase(&after),
                RestoreRecoveryPhase::ReplacementInstalled
            );
        }
        reset_finalize_controls();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn every_rename_sync_and_marker_advance_failure_leaves_exact_recovery_evidence() {
        let _serial = STAGE_TEST_LOCK.lock().await;
        for failure in 1..=8 {
            reset_finalize_controls();
            let fixture = Fixture::new().await;
            let staged = stage_verified_restore(
                &fixture.paths,
                &fixture.identity,
                &fixture.migrations,
                &fixture.schema,
                fixture.proof(),
            )
            .await
            .expect("stage");
            let error = test_finalize_with_failure(staged, failure)
                .await
                .expect_err("injected finalization failure");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Restore);
            assert!(recovery_path(&fixture, MARKER_FILE_NAME).exists());
            let live = fixture.paths.state_database().exists();
            let staged = fixture.staged_path().exists();
            let backup = recovery_path(&fixture, BACKUP_FILE_NAME).exists();
            match failure {
                1 => {
                    assert_eq!((live, staged, backup), (true, true, false));
                    assert_eq!(marker_phase(&fixture), RestoreRecoveryPhase::Prepared);
                }
                2..=4 => {
                    assert_eq!((live, staged, backup), (false, true, true));
                    assert_eq!(marker_phase(&fixture), RestoreRecoveryPhase::Prepared);
                }
                5 => {
                    assert_eq!((live, staged, backup), (false, true, true));
                    assert_eq!(marker_phase(&fixture), RestoreRecoveryPhase::LiveRetained);
                }
                6..=8 => {
                    assert_eq!((live, staged, backup), (true, false, true));
                    assert_eq!(marker_phase(&fixture), RestoreRecoveryPhase::LiveRetained);
                }
                _ => unreachable!("complete injected failure inventory"),
            }
            assert_eq!(
                recovery_path(&fixture, MARKER_NEXT_FILE_NAME).exists(),
                matches!(failure, 4 | 8)
            );
        }
        reset_finalize_controls();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn every_marker_and_restore_durability_edge_is_wired_once() {
        use crate::failpoint::{DurabilityFailpoint, DurabilityFailpoints};

        let _serial = STAGE_TEST_LOCK.lock().await;
        for point in [
            DurabilityFailpoint::MarkerBeforeCreate,
            DurabilityFailpoint::MarkerAfterCreate,
            DurabilityFailpoint::MarkerBeforeFileSync,
            DurabilityFailpoint::MarkerAfterFileSync,
            DurabilityFailpoint::MarkerBeforeDirectorySync,
            DurabilityFailpoint::MarkerAfterDirectorySync,
            DurabilityFailpoint::MarkerAdvanceBeforeWriteAndFileSync,
            DurabilityFailpoint::MarkerAdvanceAfterWriteAndFileSync,
            DurabilityFailpoint::MarkerAdvanceBeforeReplace,
            DurabilityFailpoint::MarkerAdvanceAfterReplace,
            DurabilityFailpoint::MarkerAdvanceBeforeDirectorySync,
            DurabilityFailpoint::MarkerAdvanceAfterDirectorySync,
            DurabilityFailpoint::RestoreBeforeRetainLiveRename,
            DurabilityFailpoint::RestoreAfterRetainLiveRename,
            DurabilityFailpoint::RestoreBeforeRetainLiveSync,
            DurabilityFailpoint::RestoreAfterRetainLiveSync,
            DurabilityFailpoint::RestoreBeforeInstallStageRename,
            DurabilityFailpoint::RestoreAfterInstallStageRename,
            DurabilityFailpoint::RestoreBeforeInstallStageSync,
            DurabilityFailpoint::RestoreAfterInstallStageSync,
        ] {
            reset_finalize_controls();
            let fixture = Fixture::new().await;
            let staged = stage_verified_restore(
                &fixture.paths,
                &fixture.identity,
                &fixture.migrations,
                &fixture.schema,
                fixture.proof(),
            )
            .await
            .expect("stage");
            let failpoints = DurabilityFailpoints::armed(point);
            let error = test_finalize_with_failpoint(staged, failpoints.clone())
                .await
                .expect_err("injected marker or restore edge");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Restore);
            assert!(failpoints.fired(), "edge was not reached: {point:?}");
            assert!(
                failpoints.reached().contains(&point),
                "named marker/rename edge must be observed"
            );
            assert!(
                fixture.paths.state_database().exists()
                    || recovery_path(&fixture, BACKUP_FILE_NAME).exists(),
                "live content remains represented by an exact governed artifact"
            );
            let topology = (
                fixture.paths.state_database().exists(),
                fixture.staged_path().exists(),
                recovery_path(&fixture, BACKUP_FILE_NAME).exists(),
                recovery_path(&fixture, MARKER_FILE_NAME).exists(),
            );
            match point {
                DurabilityFailpoint::MarkerBeforeCreate
                | DurabilityFailpoint::MarkerAfterCreate
                | DurabilityFailpoint::MarkerBeforeFileSync
                | DurabilityFailpoint::MarkerAfterFileSync
                | DurabilityFailpoint::MarkerBeforeDirectorySync => {
                    assert_eq!(topology, (true, false, false, false));
                }
                DurabilityFailpoint::MarkerAfterDirectorySync
                | DurabilityFailpoint::RestoreBeforeRetainLiveRename => {
                    assert_eq!(topology, (true, true, false, true));
                    assert_eq!(marker_phase(&fixture), RestoreRecoveryPhase::Prepared);
                }
                DurabilityFailpoint::RestoreAfterRetainLiveRename
                | DurabilityFailpoint::RestoreBeforeRetainLiveSync
                | DurabilityFailpoint::RestoreAfterRetainLiveSync
                | DurabilityFailpoint::MarkerAdvanceBeforeWriteAndFileSync
                | DurabilityFailpoint::MarkerAdvanceAfterWriteAndFileSync
                | DurabilityFailpoint::MarkerAdvanceBeforeReplace => {
                    assert_eq!(topology, (false, true, true, true));
                    assert_eq!(marker_phase(&fixture), RestoreRecoveryPhase::Prepared);
                }
                DurabilityFailpoint::MarkerAdvanceAfterReplace
                | DurabilityFailpoint::MarkerAdvanceBeforeDirectorySync
                | DurabilityFailpoint::MarkerAdvanceAfterDirectorySync
                | DurabilityFailpoint::RestoreBeforeInstallStageRename => {
                    assert_eq!(topology, (false, true, true, true));
                    assert_eq!(marker_phase(&fixture), RestoreRecoveryPhase::LiveRetained);
                }
                DurabilityFailpoint::RestoreAfterInstallStageRename
                | DurabilityFailpoint::RestoreBeforeInstallStageSync
                | DurabilityFailpoint::RestoreAfterInstallStageSync => {
                    assert_eq!(topology, (true, false, true, true));
                    assert_eq!(marker_phase(&fixture), RestoreRecoveryPhase::LiveRetained);
                }
                _ => unreachable!("complete marker/restore failpoint inventory"),
            }
        }
        reset_finalize_controls();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn finalization_rejects_live_stage_and_destination_replacement_without_clobbering() {
        let _serial = STAGE_TEST_LOCK.lock().await;

        let live_replaced = Fixture::new().await;
        let staged = stage_verified_restore(
            &live_replaced.paths,
            &live_replaced.identity,
            &live_replaced.migrations,
            &live_replaced.schema,
            live_replaced.proof(),
        )
        .await
        .expect("stage");
        let foreign = recovery_path(&live_replaced, "foreign-live");
        fs::write(&foreign, b"foreign-live").expect("foreign live");
        fs::set_permissions(&foreign, fs::Permissions::from_mode(0o600)).expect("foreign mode");
        fs::rename(&foreign, live_replaced.paths.state_database()).expect("replace live");
        let error = finalize_staged_restore(staged)
            .await
            .expect_err("live replacement");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Restore);
        assert_eq!(
            fs::read(live_replaced.paths.state_database()).expect("preserved foreign live"),
            b"foreign-live"
        );
        assert!(!recovery_path(&live_replaced, MARKER_FILE_NAME).exists());

        let stage_replaced = Fixture::new().await;
        let staged = stage_verified_restore(
            &stage_replaced.paths,
            &stage_replaced.identity,
            &stage_replaced.migrations,
            &stage_replaced.schema,
            stage_replaced.proof(),
        )
        .await
        .expect("stage");
        let foreign = recovery_path(&stage_replaced, "foreign-stage-finalize");
        fs::write(&foreign, b"foreign-stage").expect("foreign stage");
        fs::set_permissions(&foreign, fs::Permissions::from_mode(0o600)).expect("foreign mode");
        fs::rename(&foreign, stage_replaced.staged_path()).expect("replace stage");
        let error = finalize_staged_restore(staged)
            .await
            .expect_err("stage replacement");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Restore);
        assert_eq!(
            fs::read(stage_replaced.staged_path()).expect("preserved foreign stage"),
            b"foreign-stage"
        );
        assert!(!recovery_path(&stage_replaced, MARKER_FILE_NAME).exists());

        let collision = Fixture::new().await;
        let staged = stage_verified_restore(
            &collision.paths,
            &collision.identity,
            &collision.migrations,
            &collision.schema,
            collision.proof(),
        )
        .await
        .expect("stage");
        let backup = recovery_path(&collision, BACKUP_FILE_NAME);
        fs::write(&backup, b"foreign-backup").expect("backup collision");
        fs::set_permissions(&backup, fs::Permissions::from_mode(0o600)).expect("backup mode");
        let error = finalize_staged_restore(staged)
            .await
            .expect_err("backup collision");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Restore);
        assert_eq!(
            fs::read(backup).expect("preserved backup"),
            b"foreign-backup"
        );
        assert!(!recovery_path(&collision, MARKER_FILE_NAME).exists());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn finalization_preserves_authority_precedence_before_and_after_prepared() {
        let _serial = STAGE_TEST_LOCK.lock().await;
        for phase in [TEST_PHASE_BEFORE_PREPARED, TEST_PHASE_AFTER_PREPARED] {
            reset_finalize_controls();
            let fixture = Fixture::new().await;
            let staged = stage_verified_restore(
                &fixture.paths,
                &fixture.identity,
                &fixture.migrations,
                &fixture.schema,
                fixture.proof(),
            )
            .await
            .expect("stage");
            TEST_FINALIZE_BLOCK_PHASE.store(phase, Ordering::Release);
            let caller = tokio::spawn(finalize_staged_restore(staged));
            wait_for_finalize_phase(phase).await;
            let old_lock = recovery_path(&fixture, "state.lock.finalize-old");
            fs::rename(fixture.paths.state_lock(), &old_lock).expect("retain old lock");
            fs::write(fixture.paths.state_lock(), b"").expect("replacement lock");
            fs::set_permissions(
                fixture.paths.state_lock(),
                fs::Permissions::from_mode(0o600),
            )
            .expect("lock mode");
            TEST_FINALIZE_BLOCK_PHASE.store(0, Ordering::Release);
            let error = caller.await.expect("caller").expect_err("authority drift");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
            assert_eq!(
                recovery_path(&fixture, MARKER_FILE_NAME).exists(),
                phase == TEST_PHASE_AFTER_PREPARED
            );
            assert_eq!(
                fixture.staged_path().exists(),
                phase == TEST_PHASE_AFTER_PREPARED
            );
            fs::remove_file(old_lock).expect("remove old lock");
        }
        reset_finalize_controls();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn marker_sync_authority_drift_retains_durable_prepared_stage() {
        let _serial = STAGE_TEST_LOCK.lock().await;
        reset_finalize_controls();
        let fixture = Fixture::new().await;
        let staged = stage_verified_restore(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            fixture.proof(),
        )
        .await
        .expect("stage");

        let error = test_finalize_with_failure(staged, 9)
            .await
            .expect_err("authority drift after durable marker sync");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
        assert_eq!(marker_phase(&fixture), RestoreRecoveryPhase::Prepared);
        assert!(fixture.paths.state_database().exists());
        assert!(fixture.staged_path().exists());
        assert!(!recovery_path(&fixture, BACKUP_FILE_NAME).exists());
        fs::set_permissions(
            fixture
                .paths
                .state_database()
                .parent()
                .expect("state directory"),
            fs::Permissions::from_mode(0o700),
        )
        .expect("restore directory mode");
        reset_finalize_controls();
    }

    #[test]
    fn public_debug_and_errors_do_not_disclose_paths_or_digests() {
        let error = restore_error(RestoreFailureKind::StagedChanged);
        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains("state.restore"));
        assert!(!rendered.contains("/tmp"));
        assert!(!rendered.contains("sha256"));
    }
}
