//! Create-new initialization for one service-owned SQLite database.

use core::{fmt, future::Future};
use std::{
    error::Error,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use futures::{future::BoxFuture, stream::BoxStream};
use sqlx::{
    Either, Execute, Executor, SqlStr, Sqlite, SqliteConnection,
    sqlite::{SqliteQueryResult, SqliteRow, SqliteStatement, SqliteTypeInfo},
};

use crate::{
    OpenMode, SchemaCatalog, ServiceDatabaseMetadata, ServiceSqliteError, ServiceSqliteErrorKind,
    ServiceSqlitePaths, WriterAuthority,
};

/// A sealed initialization executor that never exposes its SQLite connection.
///
/// Service repositories may create their product schema with ordinary typed
/// SQLx queries through a mutable borrow. The service-SQLite runner exclusively
/// owns transaction begin, commit, rollback, shared metadata, and the migration
/// ledger.
///
/// ```
/// use radroots_service_sqlite::ServiceSqliteInitializer;
///
/// async fn create_product_schema(
///     initializer: &mut ServiceSqliteInitializer<'_>,
/// ) -> Result<(), sqlx::Error> {
///     sqlx::query(concat!("CREATE ", "TABLE product_items (id INTEGER PRIMARY KEY)"))
///         .execute(initializer)
///         .await?;
///     Ok(())
/// }
/// ```
///
/// Transaction control and the raw connection remain inaccessible:
///
/// ```compile_fail
/// use radroots_service_sqlite::ServiceSqliteInitializer;
///
/// async fn bypass(initializer: ServiceSqliteInitializer<'_>) {
///     initializer.commit().await.unwrap();
/// }
/// ```
pub struct ServiceSqliteInitializer<'connection> {
    connection: &'connection mut SqliteConnection,
    statement_control_rejected: Arc<AtomicBool>,
}

struct RestrictedInitializationExecute<Q> {
    query: Q,
    statement_control_rejected: Arc<AtomicBool>,
}

impl<'query, Q> Execute<'query, Sqlite> for RestrictedInitializationExecute<Q>
where
    Q: Execute<'query, Sqlite>,
{
    fn sql(self) -> SqlStr {
        restricted_initialization_sql(self.query.sql(), &self.statement_control_rejected)
    }

    fn statement(&self) -> Option<&SqliteStatement> {
        None
    }

    fn take_arguments(
        &mut self,
    ) -> Result<Option<<Sqlite as sqlx::Database>::Arguments>, sqlx::error::BoxDynError> {
        self.query.take_arguments()
    }

    fn persistent(&self) -> bool {
        self.query.persistent()
    }
}

fn restricted_initialization_sql(sql: SqlStr, statement_control_rejected: &AtomicBool) -> SqlStr {
    if crate::statement_policy::contains_forbidden_statement_control(sql.as_str()) {
        statement_control_rejected.store(true, Ordering::Release);
        SqlStr::from_static("RADROOTS_FORBIDDEN_INITIALIZATION_STATEMENT_CONTROL")
    } else {
        sql
    }
}

impl fmt::Debug for ServiceSqliteInitializer<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ServiceSqliteInitializer([redacted])")
    }
}

impl<'executor, 'connection> Executor<'executor>
    for &'executor mut ServiceSqliteInitializer<'connection>
where
    'connection: 'executor,
{
    type Database = Sqlite;

    fn fetch_many<'e, 'q: 'e, Q>(
        self,
        query: Q,
    ) -> BoxStream<'e, Result<Either<SqliteQueryResult, SqliteRow>, sqlx::Error>>
    where
        'executor: 'e,
        Q: 'q + Execute<'q, Self::Database>,
    {
        (&mut *self.connection).fetch_many(RestrictedInitializationExecute {
            query,
            statement_control_rejected: Arc::clone(&self.statement_control_rejected),
        })
    }

    fn fetch_optional<'e, 'q: 'e, Q>(
        self,
        query: Q,
    ) -> BoxFuture<'e, Result<Option<SqliteRow>, sqlx::Error>>
    where
        'executor: 'e,
        Q: 'q + Execute<'q, Self::Database>,
    {
        (&mut *self.connection).fetch_optional(RestrictedInitializationExecute {
            query,
            statement_control_rejected: Arc::clone(&self.statement_control_rejected),
        })
    }

    fn prepare_with<'e>(
        self,
        sql: SqlStr,
        parameters: &'e [SqliteTypeInfo],
    ) -> BoxFuture<'e, Result<SqliteStatement, sqlx::Error>>
    where
        'executor: 'e,
    {
        (&mut *self.connection).prepare_with(
            restricted_initialization_sql(sql, &self.statement_control_rejected),
            parameters,
        )
    }
}

/// Boxed callback future tied to the borrowed initialization executor.
pub type ServiceSqliteInitializerFuture<'a, E> =
    Pin<Box<dyn Future<Output = Result<(), E>> + Send + 'a>>;

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Debug)]
pub(crate) enum InitializeDatabaseOutcome {
    Initialized(WriterAuthority),
    Existing(WriterAuthority),
}

/// Creates and initializes a missing service database while holding sole writer authority.
///
/// The callback receives only a mutable borrow of the sealed initialization
/// executor. Product schema, shared metadata, the empty v1 migration ledger,
/// and catalog verification occur in one runner-owned transaction. Cancellation
/// or failure quarantines the one-shot connection and removes only the exact
/// inode reserved by this call.
pub async fn initialize_database<F, E>(
    paths: &ServiceSqlitePaths,
    mode: OpenMode,
    metadata: &ServiceDatabaseMetadata,
    schema_catalog: &SchemaCatalog,
    initialize_schema: F,
) -> Result<WriterAuthority, ServiceSqliteError>
where
    F: for<'a> FnOnce(
        &'a mut ServiceSqliteInitializer<'_>,
    ) -> ServiceSqliteInitializerFuture<'a, E>,
    E: Error + Send + Sync + 'static,
{
    if mode != OpenMode::Initialize {
        return Err(initialization_error(InitializationCause::new(
            InitializationFailureKind::UnsupportedMode,
        )));
    }
    if !metadata.matches_paths(paths) {
        return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Metadata));
    }

    let authority = WriterAuthority::acquire(paths, mode)?.ok_or_else(|| {
        initialization_error(InitializationCause::new(
            InitializationFailureKind::UnsupportedMode,
        ))
    })?;

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        authority.validate_for(paths)?;
        let recovery = crate::restore::refuse_unresolved_recovery(authority.directory());
        authority.validate_for(paths)?;
        recovery?;
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let failpoints = crate::failpoint::DurabilityFailpoints::default();
        match initialize_with_ops(
            paths,
            authority,
            metadata,
            schema_catalog,
            initialize_schema,
            &SystemInitializationOperations,
            &failpoints,
        )
        .await?
        {
            InitializeDatabaseOutcome::Initialized(authority) => Ok(authority),
            InitializeDatabaseOutcome::Existing(_authority) => Err(initialization_error(
                InitializationCause::new(InitializationFailureKind::StateAlreadyExists),
            )),
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        drop((authority, metadata, schema_catalog, initialize_schema));
        Err(initialization_error(InitializationCause::new(
            InitializationFailureKind::CreateUnavailable,
        )))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) async fn initialize_or_existing_database<F, E>(
    paths: &ServiceSqlitePaths,
    metadata: &ServiceDatabaseMetadata,
    schema_catalog: &SchemaCatalog,
    initialize_schema: F,
) -> Result<InitializeDatabaseOutcome, ServiceSqliteError>
where
    F: for<'a> FnOnce(
        &'a mut ServiceSqliteInitializer<'_>,
    ) -> ServiceSqliteInitializerFuture<'a, E>,
    E: Error + Send + Sync + 'static,
{
    if !metadata.matches_paths(paths) {
        return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Metadata));
    }
    let authority = WriterAuthority::acquire(paths, OpenMode::Initialize)?.ok_or_else(|| {
        initialization_error(InitializationCause::new(
            InitializationFailureKind::UnsupportedMode,
        ))
    })?;
    let failpoints = crate::failpoint::DurabilityFailpoints::default();
    initialize_with_ops(
        paths,
        authority,
        metadata,
        schema_catalog,
        initialize_schema,
        &SystemInitializationOperations,
        &failpoints,
    )
    .await
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InitializationFailureKind {
    UnsupportedMode,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    StateAlreadyExists,
    CreateUnavailable,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    InvalidDatabase,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    SchemaInitializationFailed,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    DatabaseSyncFailed,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    DatabaseReplaced,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    DirectorySyncFailed,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    CleanupFailed,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    InjectedFailure,
}

struct InitializationCause {
    kind: InitializationFailureKind,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl InitializationCause {
    const fn new(kind: InitializationFailureKind) -> Self {
        Self { kind, source: None }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn with_source(
        kind: InitializationFailureKind,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            source: Some(Box::new(source)),
        }
    }
}

impl fmt::Debug for InitializationCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InitializationCause")
            .field("kind", &self.kind)
            .field("source", &self.source.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

impl fmt::Display for InitializationCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            InitializationFailureKind::UnsupportedMode => {
                "SQLite initialization requires initialize mode"
            }
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            InitializationFailureKind::StateAlreadyExists => "SQLite state already exists",
            InitializationFailureKind::CreateUnavailable => "SQLite state could not be reserved",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            InitializationFailureKind::InvalidDatabase => "SQLite state file has invalid metadata",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            InitializationFailureKind::SchemaInitializationFailed => {
                "SQLite schema initialization failed"
            }
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            InitializationFailureKind::DatabaseSyncFailed => {
                "SQLite state could not be synchronized"
            }
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            InitializationFailureKind::DatabaseReplaced => {
                "SQLite state identity changed during initialization"
            }
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            InitializationFailureKind::DirectorySyncFailed => {
                "SQLite state directory could not be synchronized"
            }
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            InitializationFailureKind::CleanupFailed => "SQLite initialization cleanup failed",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            InitializationFailureKind::InjectedFailure => {
                "SQLite initialization durability boundary failed"
            }
        })
    }
}

impl Error for InitializationCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

fn initialization_error(cause: InitializationCause) -> ServiceSqliteError {
    ServiceSqliteError::with_source(ServiceSqliteErrorKind::Create, cause)
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
fn require_initialization_condition(
    condition: bool,
    kind: InitializationFailureKind,
) -> Result<(), InitializationCause> {
    condition
        .then_some(())
        .ok_or_else(|| InitializationCause::new(kind))
}

#[cfg(test)]
mod failure_tests {

    use super::*;

    #[test]
    fn initialization_failure_inventory_is_complete_and_source_aware() {
        let cases = [
            (
                InitializationFailureKind::UnsupportedMode,
                "SQLite initialization requires initialize mode",
            ),
            (
                InitializationFailureKind::CreateUnavailable,
                "SQLite state could not be reserved",
            ),
        ]
        .into_iter();
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let cases = cases.chain([
            (
                InitializationFailureKind::StateAlreadyExists,
                "SQLite state already exists",
            ),
            (
                InitializationFailureKind::InvalidDatabase,
                "SQLite state file has invalid metadata",
            ),
            (
                InitializationFailureKind::SchemaInitializationFailed,
                "SQLite schema initialization failed",
            ),
            (
                InitializationFailureKind::DatabaseSyncFailed,
                "SQLite state could not be synchronized",
            ),
            (
                InitializationFailureKind::DatabaseReplaced,
                "SQLite state identity changed during initialization",
            ),
            (
                InitializationFailureKind::DirectorySyncFailed,
                "SQLite state directory could not be synchronized",
            ),
            (
                InitializationFailureKind::CleanupFailed,
                "SQLite initialization cleanup failed",
            ),
            (
                InitializationFailureKind::InjectedFailure,
                "SQLite initialization durability boundary failed",
            ),
        ]);

        for (kind, message) in cases {
            let plain = InitializationCause::new(kind);
            assert_eq!(plain.to_string(), message);
            assert!(plain.source().is_none());
            assert!(format!("{plain:?}").contains("source: None"));
            assert!(require_initialization_condition(true, kind).is_ok());
            assert_eq!(
                require_initialization_condition(false, kind)
                    .expect_err("false condition")
                    .kind,
                kind
            );

            #[cfg(any(target_os = "linux", target_os = "macos"))]
            {
                let sourced =
                    InitializationCause::with_source(kind, std::io::Error::other("private-cause"));
                assert_eq!(sourced.to_string(), message);
                assert!(sourced.source().is_some());
                let debug = format!("{sourced:?}");
                assert!(debug.contains("[redacted]"));
                assert!(!debug.contains("private-cause"));
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod supported {
    use std::{fs::File, os::fd::AsRawFd};

    use rustix::{
        fs::{AtFlags, FileType, Mode, OFlags, fchmod, fstat, lstat, openat, statat, unlinkat},
        process::geteuid,
    };

    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FileIdentity {
        device: u64,
        inode: u64,
    }

    pub(super) trait InitializationOperations {
        fn sync_database(&self, database: &File) -> Result<(), InitializationCause>;
        fn sync_directory(&self, directory: &File) -> Result<(), InitializationCause>;
        fn unlink_database(&self, directory: &File) -> Result<(), InitializationCause>;
    }

    pub(super) struct SystemInitializationOperations;

    impl InitializationOperations for SystemInitializationOperations {
        #[cfg_attr(coverage_nightly, coverage(off))]
        fn sync_database(&self, database: &File) -> Result<(), InitializationCause> {
            database.sync_all().map_err(|_| {
                InitializationCause::new(InitializationFailureKind::DatabaseSyncFailed)
            })
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        fn sync_directory(&self, directory: &File) -> Result<(), InitializationCause> {
            directory.sync_all().map_err(|_| {
                InitializationCause::new(InitializationFailureKind::DirectorySyncFailed)
            })
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        fn unlink_database(&self, directory: &File) -> Result<(), InitializationCause> {
            unlinkat(
                directory,
                radroots_runtime_paths::SERVICE_STATE_DATABASE_FILE_NAME,
                AtFlags::empty(),
            )
            .map_err(|_| InitializationCause::new(InitializationFailureKind::CleanupFailed))
        }
    }

    struct PendingDatabase<'a, O: InitializationOperations> {
        directory: &'a File,
        database: File,
        identity: FileIdentity,
        operations: &'a O,
        committed: bool,
    }

    impl<'a, O: InitializationOperations> PendingDatabase<'a, O> {
        fn create(directory: &'a File, operations: &'a O) -> Result<Self, InitializationCause> {
            let descriptor = openat(
                directory,
                radroots_runtime_paths::SERVICE_STATE_DATABASE_FILE_NAME,
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|error| {
                if error == rustix::io::Errno::EXIST {
                    InitializationCause::new(InitializationFailureKind::StateAlreadyExists)
                } else {
                    InitializationCause::new(InitializationFailureKind::CreateUnavailable)
                }
            })?;
            let database = File::from(descriptor);
            let identity = descriptor_identity(&database)?;
            let pending = Self {
                directory,
                database,
                identity,
                operations,
                committed: false,
            };
            fchmod(&pending.database, Mode::RUSR | Mode::WUSR).map_err(|_| {
                InitializationCause::new(InitializationFailureKind::InvalidDatabase)
            })?;
            pending.validate()?;
            Ok(pending)
        }

        fn validate(&self) -> Result<(), InitializationCause> {
            let descriptor_identity = validate_descriptor(&self.database)?;
            require_initialization_condition(
                descriptor_identity == self.identity,
                InitializationFailureKind::InvalidDatabase,
            )?;
            self.validate_entry()
        }

        fn sqlite_descriptor_path(&self) -> String {
            let descriptor = self.database.as_raw_fd();
            #[cfg(target_os = "linux")]
            let path = format!("/proc/self/fd/{descriptor}");
            #[cfg(target_os = "macos")]
            let path = format!("/dev/fd/{descriptor}");
            path
        }

        fn validate_entry(&self) -> Result<(), InitializationCause> {
            let status = statat(
                self.directory,
                radroots_runtime_paths::SERVICE_STATE_DATABASE_FILE_NAME,
                AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|_| InitializationCause::new(InitializationFailureKind::DatabaseReplaced))?;
            let device = crate::native_metadata::device(status.st_dev).map_err(|_| {
                InitializationCause::new(InitializationFailureKind::InvalidDatabase)
            })?;
            let current = validate_status(
                FileType::from_raw_mode(status.st_mode).is_file(),
                crate::native_metadata::link_count(status.st_nlink),
                status.st_uid,
                crate::native_metadata::mode(status.st_mode),
                device,
                status.st_ino,
            )?;
            require_initialization_condition(
                current == self.identity,
                InitializationFailureKind::DatabaseReplaced,
            )?;
            Ok(())
        }

        fn current_entry_identity(&self) -> Result<FileIdentity, InitializationCause> {
            let status = statat(
                self.directory,
                radroots_runtime_paths::SERVICE_STATE_DATABASE_FILE_NAME,
                AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|_| InitializationCause::new(InitializationFailureKind::DatabaseReplaced))?;
            let device = crate::native_metadata::device(status.st_dev).map_err(|_| {
                InitializationCause::new(InitializationFailureKind::DatabaseReplaced)
            })?;
            Ok(FileIdentity {
                device,
                inode: status.st_ino,
            })
        }

        fn validate_canonical_path(
            &self,
            path: &std::path::Path,
        ) -> Result<(), InitializationCause> {
            let status = lstat(path).map_err(|_| {
                InitializationCause::new(InitializationFailureKind::DatabaseReplaced)
            })?;
            let device = crate::native_metadata::device(status.st_dev).map_err(|_| {
                InitializationCause::new(InitializationFailureKind::InvalidDatabase)
            })?;
            let current = validate_status(
                FileType::from_raw_mode(status.st_mode).is_file(),
                crate::native_metadata::link_count(status.st_nlink),
                status.st_uid,
                crate::native_metadata::mode(status.st_mode),
                device,
                status.st_ino,
            )?;
            require_initialization_condition(
                current == self.identity,
                InitializationFailureKind::DatabaseReplaced,
            )?;
            Ok(())
        }

        fn commit(
            &mut self,
            canonical_path: &std::path::Path,
            failpoints: &crate::failpoint::DurabilityFailpoints,
        ) -> Result<(), InitializationCause> {
            hit(
                failpoints,
                crate::failpoint::DurabilityFailpoint::InitializeBeforeFileSync,
            )?;
            self.operations.sync_database(&self.database)?;
            hit(
                failpoints,
                crate::failpoint::DurabilityFailpoint::InitializeAfterFileSync,
            )?;
            self.validate()?;
            self.validate_canonical_path(canonical_path)?;
            hit(
                failpoints,
                crate::failpoint::DurabilityFailpoint::InitializeBeforeCommitDirectorySync,
            )?;
            self.operations.sync_directory(self.directory)?;
            hit(
                failpoints,
                crate::failpoint::DurabilityFailpoint::InitializeAfterCommitDirectorySync,
            )?;
            self.committed = true;
            Ok(())
        }

        fn rollback(&mut self) -> Result<(), InitializationCause> {
            if self.committed {
                return Ok(());
            }
            require_initialization_condition(
                self.current_entry_identity()? == self.identity,
                InitializationFailureKind::DatabaseReplaced,
            )?;
            self.operations.unlink_database(self.directory)?;
            self.operations.sync_directory(self.directory)?;
            self.committed = true;
            Ok(())
        }
    }

    impl<O: InitializationOperations> Drop for PendingDatabase<'_, O> {
        fn drop(&mut self) {
            let _ = self.rollback();
        }
    }

    fn validate_descriptor(
        descriptor: &impl std::os::fd::AsFd,
    ) -> Result<FileIdentity, InitializationCause> {
        let status = fstat(descriptor)
            .map_err(|_| InitializationCause::new(InitializationFailureKind::InvalidDatabase))?;
        let device = crate::native_metadata::device(status.st_dev)
            .map_err(|_| InitializationCause::new(InitializationFailureKind::InvalidDatabase))?;
        validate_status(
            FileType::from_raw_mode(status.st_mode).is_file(),
            crate::native_metadata::link_count(status.st_nlink),
            status.st_uid,
            crate::native_metadata::mode(status.st_mode),
            device,
            status.st_ino,
        )
    }

    fn descriptor_identity(
        descriptor: &impl std::os::fd::AsFd,
    ) -> Result<FileIdentity, InitializationCause> {
        let status = fstat(descriptor)
            .map_err(|_| InitializationCause::new(InitializationFailureKind::InvalidDatabase))?;
        let device = crate::native_metadata::device(status.st_dev)
            .map_err(|_| InitializationCause::new(InitializationFailureKind::InvalidDatabase))?;
        Ok(FileIdentity {
            device,
            inode: status.st_ino,
        })
    }

    fn validate_status(
        is_regular_file: bool,
        link_count: u64,
        actual_uid: u32,
        mode: u32,
        device: u64,
        inode: u64,
    ) -> Result<FileIdentity, InitializationCause> {
        require_initialization_condition(
            crate::native_metadata::exact_regular_file(
                is_regular_file,
                link_count,
                actual_uid,
                geteuid().as_raw(),
                mode,
            ),
            InitializationFailureKind::InvalidDatabase,
        )?;
        Ok(FileIdentity { device, inode })
    }

    fn rollback_failure(
        primary: InitializationCause,
        cleanup: Result<(), InitializationCause>,
    ) -> ServiceSqliteError {
        match cleanup {
            Ok(()) => initialization_error(primary),
            Err(_cleanup) if primary.kind == InitializationFailureKind::DatabaseReplaced => {
                initialization_error(primary)
            }
            Err(cleanup) => {
                initialization_error(InitializationCause::with_source(cleanup.kind, primary))
            }
        }
    }

    async fn fail_with_rollback<O: InitializationOperations>(
        mut pending: PendingDatabase<'_, O>,
        primary: InitializationCause,
    ) -> Result<InitializeDatabaseOutcome, ServiceSqliteError> {
        Err(rollback_failure(primary, pending.rollback()))
    }

    async fn fail_metadata_with_rollback<O: InitializationOperations>(
        mut pending: PendingDatabase<'_, O>,
        primary: ServiceSqliteError,
    ) -> Result<InitializeDatabaseOutcome, ServiceSqliteError> {
        match pending.rollback() {
            Ok(()) => Err(primary),
            Err(cleanup) => Err(initialization_error(InitializationCause::with_source(
                cleanup.kind,
                primary,
            ))),
        }
    }

    pub(super) async fn initialize_with_ops<F, E, O>(
        paths: &ServiceSqlitePaths,
        authority: WriterAuthority,
        metadata: &ServiceDatabaseMetadata,
        schema_catalog: &SchemaCatalog,
        initialize_schema: F,
        operations: &O,
        failpoints: &crate::failpoint::DurabilityFailpoints,
    ) -> Result<InitializeDatabaseOutcome, ServiceSqliteError>
    where
        F: for<'a> FnOnce(
            &'a mut ServiceSqliteInitializer<'_>,
        ) -> ServiceSqliteInitializerFuture<'a, E>,
        E: Error + Send + Sync + 'static,
        O: InitializationOperations,
    {
        hit(
            failpoints,
            crate::failpoint::DurabilityFailpoint::InitializeBeforeCreate,
        )
        .map_err(initialization_error)?;
        let initialization_directory = authority.directory().try_clone().map_err(|source| {
            initialization_error(InitializationCause::with_source(
                InitializationFailureKind::CreateUnavailable,
                source,
            ))
        })?;
        let mut pending = match PendingDatabase::create(&initialization_directory, operations) {
            Ok(pending) => pending,
            Err(error) if error.kind == InitializationFailureKind::StateAlreadyExists => {
                return Ok(InitializeDatabaseOutcome::Existing(authority));
            }
            Err(error) => return Err(initialization_error(error)),
        };
        if let Err(error) = hit(
            failpoints,
            crate::failpoint::DurabilityFailpoint::InitializeAfterCreate,
        ) {
            return fail_with_rollback(pending, error).await;
        }
        if let Err(error) = hit(
            failpoints,
            crate::failpoint::DurabilityFailpoint::InitializeBeforeReservationDirectorySync,
        ) {
            return fail_with_rollback(pending, error).await;
        }
        if let Err(error) = operations.sync_directory(authority.directory()) {
            return fail_with_rollback(pending, error).await;
        }
        if let Err(error) = hit(
            failpoints,
            crate::failpoint::DurabilityFailpoint::InitializeAfterReservationDirectorySync,
        ) {
            return fail_with_rollback(pending, error).await;
        }
        authority.validate_for(paths)?;
        let recovery = crate::restore::refuse_unresolved_recovery(authority.directory());
        authority.validate_for(paths)?;
        if let Err(error) = recovery {
            return fail_metadata_with_rollback(pending, error).await;
        }
        if let Err(error) = pending.validate_canonical_path(paths.state_database()) {
            return fail_with_rollback(pending, error).await;
        }
        let metadata_result = initialize_transaction(
            paths,
            &authority,
            &pending,
            metadata,
            schema_catalog,
            initialize_schema,
        )
        .await;
        authority.validate_for(paths)?;
        if let Err(error) = pending.validate() {
            return fail_with_rollback(pending, error).await;
        }
        if let Err(error) = pending.validate_canonical_path(paths.state_database()) {
            return fail_with_rollback(pending, error).await;
        }
        if let Err(error) = metadata_result {
            return fail_metadata_with_rollback(pending, error).await;
        }
        if let Err(error) = pending.validate() {
            return fail_with_rollback(pending, error).await;
        }
        if let Err(error) = pending.validate_canonical_path(paths.state_database()) {
            return fail_with_rollback(pending, error).await;
        }
        if let Err(error) = pending.commit(paths.state_database(), failpoints) {
            return fail_with_rollback(pending, error).await;
        }
        drop(pending);
        Ok(InitializeDatabaseOutcome::Initialized(authority))
    }

    async fn initialize_transaction<F, E, O>(
        paths: &ServiceSqlitePaths,
        authority: &WriterAuthority,
        pending: &PendingDatabase<'_, O>,
        metadata: &ServiceDatabaseMetadata,
        schema_catalog: &SchemaCatalog,
        initialize_schema: F,
    ) -> Result<(), ServiceSqliteError>
    where
        F: for<'a> FnOnce(
            &'a mut ServiceSqliteInitializer<'_>,
        ) -> ServiceSqliteInitializerFuture<'a, E>,
        E: Error + Send + Sync + 'static,
        O: InitializationOperations,
    {
        use sqlx::{
            ConnectOptions, Connection,
            sqlite::{SqliteConnectOptions, SqliteJournalMode},
        };

        authority.validate_for(paths)?;
        pending
            .validate()
            .and_then(|()| pending.validate_canonical_path(paths.state_database()))
            .map_err(initialization_error)?;
        let options = SqliteConnectOptions::new()
            .filename(pending.sqlite_descriptor_path())
            .create_if_missing(false)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Memory)
            .pragma("trusted_schema", "OFF")
            .disable_statement_logging();
        let connected = SqliteConnection::connect_with(&options).await;
        authority.validate_for(paths)?;
        pending
            .validate()
            .and_then(|()| pending.validate_canonical_path(paths.state_database()))
            .map_err(initialization_error)?;
        let mut connection = connected.map_err(|source| {
            initialization_error(InitializationCause::with_source(
                InitializationFailureKind::SchemaInitializationFailed,
                source,
            ))
        })?;
        let installed =
            crate::transaction_control::TransactionControlGate::install(&mut connection).await;
        authority.validate_for(paths)?;
        pending
            .validate()
            .and_then(|()| pending.validate_canonical_path(paths.state_database()))
            .map_err(initialization_error)?;
        let gate = installed.map_err(|source| {
            initialization_error(InitializationCause::with_source(
                InitializationFailureKind::SchemaInitializationFailed,
                source,
            ))
        })?;
        let begun = connection.begin_with("BEGIN IMMEDIATE").await;
        authority.validate_for(paths)?;
        pending
            .validate()
            .and_then(|()| pending.validate_canonical_path(paths.state_database()))
            .map_err(initialization_error)?;
        let mut transaction = begun.map_err(|source| {
            initialization_error(InitializationCause::with_source(
                InitializationFailureKind::SchemaInitializationFailed,
                source,
            ))
        })?;

        let statement_control_rejected = Arc::new(AtomicBool::new(false));
        let callback = {
            let mut initializer = ServiceSqliteInitializer {
                connection: &mut transaction,
                statement_control_rejected: Arc::clone(&statement_control_rejected),
            };
            initialize_schema(&mut initializer).await
        };
        authority.validate_for(paths)?;
        pending
            .validate()
            .and_then(|()| pending.validate_canonical_path(paths.state_database()))
            .map_err(initialization_error)?;
        let governed_after_callback =
            crate::migration::assert_governed_transaction(&mut transaction)
                .await
                .is_ok();
        authority.validate_for(paths)?;

        let operation = match callback {
            Ok(())
                if !statement_control_rejected.load(Ordering::Acquire)
                    && !gate.control_violation_observed()
                    && governed_after_callback =>
            {
                crate::metadata::write_database_metadata_in_transaction(
                    &mut transaction,
                    metadata,
                    schema_catalog,
                )
                .await
            }
            Ok(()) => Err(initialization_error(InitializationCause::new(
                InitializationFailureKind::SchemaInitializationFailed,
            ))),
            Err(source) => Err(initialization_error(InitializationCause::with_source(
                InitializationFailureKind::SchemaInitializationFailed,
                source,
            ))),
        };
        authority.validate_for(paths)?;
        pending
            .validate()
            .and_then(|()| pending.validate_canonical_path(paths.state_database()))
            .map_err(initialization_error)?;
        let governed_before_commit =
            crate::migration::assert_governed_transaction(&mut transaction)
                .await
                .is_ok();
        authority.validate_for(paths)?;

        if let Err(primary) = operation {
            let permit = gate.permit_runner_rollback();
            let rollback = transaction.rollback().await;
            drop(permit);
            authority.validate_for(paths)?;
            pending
                .validate()
                .and_then(|()| pending.validate_canonical_path(paths.state_database()))
                .map_err(initialization_error)?;
            let remove = gate.remove(&mut connection).await;
            authority.validate_for(paths)?;
            let close = connection.close().await;
            authority.validate_for(paths)?;
            if let Err(source) = rollback.or(remove).or(close) {
                return Err(initialization_error(InitializationCause::with_source(
                    InitializationFailureKind::SchemaInitializationFailed,
                    source,
                )));
            }
            return Err(primary);
        }

        if gate.control_violation_observed() || !governed_before_commit {
            let permit = gate.permit_runner_rollback();
            let rollback = transaction.rollback().await;
            drop(permit);
            let remove = gate.remove(&mut connection).await;
            let close = connection.close().await;
            authority.validate_for(paths)?;
            rollback.or(remove).or(close).map_err(|source| {
                initialization_error(InitializationCause::with_source(
                    InitializationFailureKind::SchemaInitializationFailed,
                    source,
                ))
            })?;
            return Err(initialization_error(InitializationCause::new(
                InitializationFailureKind::SchemaInitializationFailed,
            )));
        }

        let permit = gate.permit_outer_commit();
        let committed = transaction.commit().await;
        drop(permit);
        authority.validate_for(paths)?;
        pending
            .validate()
            .and_then(|()| pending.validate_canonical_path(paths.state_database()))
            .map_err(initialization_error)?;
        let removed = gate.remove(&mut connection).await;
        authority.validate_for(paths)?;
        let closed = connection.close().await;
        authority.validate_for(paths)?;
        committed.or(removed).or(closed).map_err(|source| {
            initialization_error(InitializationCause::with_source(
                InitializationFailureKind::SchemaInitializationFailed,
                source,
            ))
        })
    }

    fn hit(
        failpoints: &crate::failpoint::DurabilityFailpoints,
        point: crate::failpoint::DurabilityFailpoint,
    ) -> Result<(), InitializationCause> {
        failpoints.hit(point).map_err(|source| {
            InitializationCause::with_source(InitializationFailureKind::InjectedFailure, source)
        })
    }

    #[cfg(test)]
    mod tests {
        use std::{
            cell::{Cell, RefCell},
            convert::Infallible,
            fs,
            future::{pending, ready},
            io,
            num::NonZeroU32,
            os::unix::fs::{MetadataExt, PermissionsExt, symlink},
            path::Path,
            sync::Arc,
        };

        use radroots_runtime_paths::{
            InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
            RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource,
            ServiceId,
        };
        use radroots_storage::event::SourceGeneration;
        use tokio::sync::Notify;

        use super::*;

        #[derive(Debug)]
        struct CallbackFailure;

        impl fmt::Display for CallbackFailure {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("secret callback path=/private/state.sqlite")
            }
        }

        impl Error for CallbackFailure {}

        #[derive(Default)]
        struct RecordingOperations {
            events: RefCell<Vec<&'static str>>,
            fail_database_sync: Cell<bool>,
            fail_directory_sync_on_call: Cell<Option<usize>>,
            directory_sync_calls: Cell<usize>,
            fail_unlink: Cell<bool>,
        }

        impl InitializationOperations for RecordingOperations {
            fn sync_database(&self, _database: &File) -> Result<(), InitializationCause> {
                self.events.borrow_mut().push("sync_database");
                if self.fail_database_sync.replace(false) {
                    Err(InitializationCause::new(
                        InitializationFailureKind::DatabaseSyncFailed,
                    ))
                } else {
                    Ok(())
                }
            }

            fn sync_directory(&self, _directory: &File) -> Result<(), InitializationCause> {
                self.events.borrow_mut().push("sync_directory");
                let call = self.directory_sync_calls.get() + 1;
                self.directory_sync_calls.set(call);
                if self.fail_directory_sync_on_call.get() == Some(call) {
                    Err(InitializationCause::new(
                        InitializationFailureKind::DirectorySyncFailed,
                    ))
                } else {
                    Ok(())
                }
            }

            fn unlink_database(&self, directory: &File) -> Result<(), InitializationCause> {
                self.events.borrow_mut().push("unlink_database");
                if self.fail_unlink.get() {
                    return Err(InitializationCause::new(
                        InitializationFailureKind::CleanupFailed,
                    ));
                }
                SystemInitializationOperations.unlink_database(directory)
            }
        }

        fn paths(root: &Path, instance: &str) -> ServiceSqlitePaths {
            let context = RuntimeContext::resolve(
                &RadrootsPathResolver::new(
                    RadrootsPlatform::Linux,
                    RadrootsHostEnvironment::default(),
                ),
                RuntimeContextBootstrap::new(
                    RadrootsPathProfile::RepoLocal,
                    Some(root.to_path_buf()),
                    RuntimeContextSource::BootstrapCli,
                    RuntimeContextSource::BootstrapCli,
                )
                .expect("bootstrap"),
                ServiceId::new("myc").expect("service"),
                InstanceId::new(instance).expect("instance"),
            )
            .expect("runtime context");
            ServiceSqlitePaths::from_runtime_context(&context).expect("SQLite paths")
        }

        fn prepare(paths: &ServiceSqlitePaths) {
            fs::create_dir_all(paths.state_database().parent().expect("state directory"))
                .expect("create state directory");
        }

        fn metadata(paths: &ServiceSqlitePaths) -> ServiceDatabaseMetadata {
            ServiceDatabaseMetadata::new(
                paths,
                SourceGeneration::new([7; 32]).expect("source generation"),
                NonZeroU32::new(1).expect("schema version"),
                1_700_000_000_000,
                crate::ServiceSqliteApplicationId::new(0x5244_5351).expect("application ID"),
            )
            .expect("database metadata")
        }

        fn schema_catalog(service_objects: Vec<crate::SchemaObject>) -> crate::SchemaCatalog {
            let migrations = crate::MigrationCatalog::new([]).expect("empty migrations");
            let digest =
                crate::SchemaVersionCatalog::computed_digest(1, service_objects.iter().cloned())
                    .expect("schema digest");
            let version = crate::SchemaVersionCatalog::new(1, service_objects, digest)
                .expect("schema version");
            crate::SchemaCatalog::new(&migrations, [version]).expect("schema catalog")
        }

        fn base_schema_catalog() -> crate::SchemaCatalog {
            schema_catalog(Vec::new())
        }

        fn service_schema_catalog() -> crate::SchemaCatalog {
            const SQL: &str = "CREATE TABLE service_schema (id INTEGER PRIMARY KEY)";
            let object = crate::SchemaObject::new(
                crate::SchemaObjectKind::Table,
                "service_schema",
                "service_schema",
                SQL,
                crate::SchemaObject::computed_digest(
                    crate::SchemaObjectKind::Table,
                    "service_schema",
                    "service_schema",
                    SQL,
                )
                .expect("service schema digest"),
            )
            .expect("service schema object");
            schema_catalog(vec![object])
        }

        #[tokio::test(flavor = "current_thread")]
        async fn successful_initialization_is_create_new_mode_0600_and_retains_authority() {
            let root = tempfile::tempdir().expect("root");
            let paths = paths(root.path(), "success");
            prepare(&paths);
            let metadata = metadata(&paths);
            let schema_catalog = service_schema_catalog();
            let mut authority = initialize_database(
                &paths,
                OpenMode::Initialize,
                &metadata,
                &schema_catalog,
                |initializer| {
                    Box::pin(async move {
                        assert_eq!(
                            format!("{initializer:?}"),
                            "ServiceSqliteInitializer([redacted])"
                        );
                        sqlx::query("CREATE TABLE service_schema (id INTEGER PRIMARY KEY)")
                            .execute(initializer)
                            .await?;
                        Ok::<(), sqlx::Error>(())
                    })
                },
            )
            .await
            .expect("initialize");

            let filesystem_metadata = fs::metadata(paths.state_database()).unwrap();
            assert_eq!(filesystem_metadata.permissions().mode() & 0o777, 0o600);
            assert_eq!(filesystem_metadata.nlink(), 1);
            assert!(authority.is_held());
            assert!(WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting).is_err());
            authority.release().expect("release");
            assert!(
                WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting)
                    .expect("reacquire")
                    .is_some()
            );
        }

        #[tokio::test(flavor = "current_thread")]
        async fn existing_regular_symlink_directory_and_hardlink_are_never_mutated() {
            for shape in ["regular", "symlink", "directory", "hardlink"] {
                let root = tempfile::tempdir().expect("root");
                let paths = paths(root.path(), shape);
                prepare(&paths);
                let target = root.path().join("existing-target");
                fs::write(&target, b"preserve-me").expect("target");
                match shape {
                    "regular" => fs::write(paths.state_database(), b"existing").unwrap(),
                    "symlink" => symlink(&target, paths.state_database()).unwrap(),
                    "directory" => fs::create_dir(paths.state_database()).unwrap(),
                    "hardlink" => fs::hard_link(&target, paths.state_database()).unwrap(),
                    _ => unreachable!(),
                }
                let called = Cell::new(false);
                let metadata = metadata(&paths);
                let schema_catalog = base_schema_catalog();
                let error = initialize_database(
                    &paths,
                    OpenMode::Initialize,
                    &metadata,
                    &schema_catalog,
                    |_| {
                        called.set(true);
                        Box::pin(ready(Ok::<(), CallbackFailure>(())))
                    },
                )
                .await
                .expect_err("existing state must fail");
                assert_eq!(error.kind(), ServiceSqliteErrorKind::Create);
                assert!(!called.get());
                assert_eq!(fs::read(&target).unwrap(), b"preserve-me");
            }
        }

        #[tokio::test(flavor = "current_thread")]
        async fn non_initialize_modes_have_zero_callback_and_filesystem_effects() {
            for mode in [OpenMode::ReadWriteExisting, OpenMode::ReadOnlyInspection] {
                let root = tempfile::tempdir().expect("root");
                let paths = paths(root.path(), "wrong-mode");
                let called = Cell::new(false);
                let metadata = metadata(&paths);
                let schema_catalog = base_schema_catalog();
                let error = initialize_database(&paths, mode, &metadata, &schema_catalog, |_| {
                    called.set(true);
                    Box::pin(ready(Ok::<(), CallbackFailure>(())))
                })
                .await
                .expect_err("mode must reject");
                assert_eq!(error.kind(), ServiceSqliteErrorKind::Create);
                assert!(!called.get());
                assert!(!paths.state_database().exists());
                assert!(!paths.state_lock().exists());
                assert!(!paths.state_database().parent().unwrap().exists());
            }
        }

        #[tokio::test(flavor = "current_thread")]
        async fn mismatched_metadata_paths_have_zero_callback_and_filesystem_effects() {
            let root = tempfile::tempdir().expect("root");
            let expected_paths = paths(root.path(), "expected");
            let other = paths(root.path(), "other");
            let called = Cell::new(false);
            let other_metadata = metadata(&other);
            let error = initialize_database(
                &expected_paths,
                OpenMode::Initialize,
                &other_metadata,
                &base_schema_catalog(),
                |_| {
                    called.set(true);
                    Box::pin(ready(Ok::<(), CallbackFailure>(())))
                },
            )
            .await
            .expect_err("metadata paths must reject");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Metadata);
            assert!(!called.get());
            assert!(!expected_paths.state_database().exists());
            assert!(!expected_paths.state_lock().exists());
        }

        #[tokio::test(flavor = "current_thread")]
        async fn callback_failure_cleans_up_releases_authority_and_preserves_trusted_cause() {
            let root = tempfile::tempdir().expect("root");
            let paths = paths(root.path(), "callback-failure");
            prepare(&paths);
            let metadata = metadata(&paths);
            let schema_catalog = base_schema_catalog();
            let error = initialize_database(
                &paths,
                OpenMode::Initialize,
                &metadata,
                &schema_catalog,
                |_| Box::pin(ready(Err::<(), _>(CallbackFailure))),
            )
            .await
            .expect_err("callback failure");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Create);
            assert!(!paths.state_database().exists());
            assert!(
                WriterAuthority::acquire(&paths, OpenMode::Initialize)
                    .unwrap()
                    .is_some()
            );

            let display = error.to_string();
            let debug = format!("{error:?}");
            assert!(!display.contains("secret"));
            assert!(!debug.contains("secret"));
            let first = error.source().expect("initialization cause");
            assert_eq!(first.to_string(), "SQLite schema initialization failed");
            assert_eq!(
                first.source().map(ToString::to_string).as_deref(),
                Some("secret callback path=/private/state.sqlite")
            );

            let retry = initialize_database(
                &paths,
                OpenMode::Initialize,
                &metadata,
                &schema_catalog,
                |_| Box::pin(ready(Ok::<(), CallbackFailure>(()))),
            )
            .await
            .expect("retry after cleanup");
            assert!(retry.is_held());
            assert!(paths.state_database().exists());
        }

        #[tokio::test(flavor = "current_thread")]
        async fn transaction_control_and_attachments_are_rejected_before_sqlite_compilation() {
            for scenario in ["commit", "rollback_begin", "attach_detach"] {
                let root = tempfile::tempdir().expect("root");
                let paths = paths(root.path(), scenario);
                prepare(&paths);
                let external = root.path().join("must-not-exist.sqlite");
                let external_for_callback = external.clone();
                let metadata = metadata(&paths);
                let schema_catalog = base_schema_catalog();
                let error = initialize_database(
                    &paths,
                    OpenMode::Initialize,
                    &metadata,
                    &schema_catalog,
                    move |initializer| {
                        Box::pin(async move {
                            let sql = match scenario {
                                "commit" => "COMMIT".to_owned(),
                                "rollback_begin" => {
                                    "ROLLBACK; BEGIN DEFERRED; CREATE TABLE escaped(value INTEGER)"
                                        .to_owned()
                                }
                                "attach_detach" => format!(
                                    "ATTACH DATABASE '{}' AS extra; DETACH DATABASE extra",
                                    external_for_callback.display()
                                ),
                                _ => unreachable!(),
                            };
                            let _ = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                                .execute(initializer)
                                .await;
                            Ok::<(), Infallible>(())
                        })
                    },
                )
                .await
                .expect_err("forbidden statement control must fail initialization");
                assert_eq!(error.kind(), ServiceSqliteErrorKind::Create);
                for rendered in [error.to_string(), format!("{error:?}")] {
                    assert!(!rendered.contains("must-not-exist"));
                    assert!(!rendered.contains("ATTACH"));
                    assert!(!rendered.contains("ROLLBACK"));
                }
                assert!(!paths.state_database().exists());
                assert!(!external.exists());
                assert!(
                    WriterAuthority::acquire(&paths, OpenMode::Initialize)
                        .expect("reacquire")
                        .is_some()
                );
            }
        }

        #[tokio::test(flavor = "current_thread")]
        async fn metadata_or_migration_ledger_conflict_cleans_the_exact_reserved_database() {
            let root = tempfile::tempdir().expect("root");
            for (instance, statement, expected_kind) in [
                (
                    "metadata-failure",
                    "CREATE TABLE radroots_service_metadata (value TEXT)",
                    ServiceSqliteErrorKind::Metadata,
                ),
                (
                    "migration-ledger-failure",
                    "CREATE TABLE schema_migrations (value TEXT)",
                    ServiceSqliteErrorKind::Migration,
                ),
            ] {
                let paths = paths(root.path(), instance);
                prepare(&paths);
                let metadata = metadata(&paths);
                let schema_catalog = base_schema_catalog();
                let error = initialize_database(
                    &paths,
                    OpenMode::Initialize,
                    &metadata,
                    &schema_catalog,
                    move |initializer| {
                        Box::pin(async move {
                            sqlx::query(statement).execute(initializer).await?;
                            Ok::<(), sqlx::Error>(())
                        })
                    },
                )
                .await
                .expect_err("conflicting governed table must fail");

                assert_eq!(error.kind(), expected_kind);
                assert!(!paths.state_database().exists());
                assert!(
                    WriterAuthority::acquire(&paths, OpenMode::Initialize)
                        .expect("reacquire")
                        .is_some()
                );
            }
        }

        #[tokio::test(flavor = "current_thread")]
        async fn schema_catalog_mismatch_cleans_the_exact_reserved_database() {
            let root = tempfile::tempdir().expect("root");
            let paths = paths(root.path(), "schema-mismatch");
            prepare(&paths);
            let metadata = metadata(&paths);
            let schema_catalog = base_schema_catalog();
            let error = initialize_database(
                &paths,
                OpenMode::Initialize,
                &metadata,
                &schema_catalog,
                |initializer| {
                    Box::pin(async move {
                        sqlx::query("CREATE TABLE unexpected (value INTEGER)")
                            .execute(initializer)
                            .await?;
                        Ok::<(), sqlx::Error>(())
                    })
                },
            )
            .await
            .expect_err("unlisted schema object must fail initialization");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Integrity);
            assert!(!paths.state_database().exists());
            assert!(
                WriterAuthority::acquire(&paths, OpenMode::Initialize)
                    .unwrap()
                    .is_some()
            );
        }

        #[tokio::test(flavor = "current_thread")]
        async fn cancellation_rolls_back_and_releases_authority() {
            let root = tempfile::tempdir().expect("root");
            let paths = paths(root.path(), "cancelled");
            prepare(&paths);
            let metadata = metadata(&paths);
            let schema_catalog = base_schema_catalog();
            let task_paths = paths.clone();
            let reached = Arc::new(Notify::new());
            let reached_from_callback = Arc::clone(&reached);
            let task = tokio::spawn(async move {
                initialize_database(
                    &task_paths,
                    OpenMode::Initialize,
                    &metadata,
                    &schema_catalog,
                    move |initializer| {
                        Box::pin(async move {
                            sqlx::query("CREATE TABLE cancelled_probe (value INTEGER)")
                                .execute(initializer)
                                .await?;
                            reached_from_callback.notify_one();
                            pending::<Result<(), sqlx::Error>>().await
                        })
                    },
                )
                .await
            });
            reached.notified().await;
            assert!(paths.state_database().exists());
            assert!(WriterAuthority::acquire(&paths, OpenMode::Initialize).is_err());
            task.abort();
            task.await.expect_err("initialization task is cancelled");
            assert!(!paths.state_database().exists());
            assert!(
                WriterAuthority::acquire(&paths, OpenMode::Initialize)
                    .unwrap()
                    .is_some()
            );
        }

        #[tokio::test(flavor = "current_thread")]
        async fn replacement_is_detected_and_never_deleted() {
            let root = tempfile::tempdir().expect("root");
            let paths = paths(root.path(), "replacement");
            prepare(&paths);
            let metadata = metadata(&paths);
            let schema_catalog = base_schema_catalog();
            let replacement_path = paths.state_database().to_path_buf();
            let error = initialize_database(
                &paths,
                OpenMode::Initialize,
                &metadata,
                &schema_catalog,
                move |_| {
                    Box::pin(async move {
                        fs::remove_file(&replacement_path)?;
                        fs::write(&replacement_path, b"replacement")?;
                        Ok::<(), io::Error>(())
                    })
                },
            )
            .await
            .expect_err("replacement must fail");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Create);
            assert_eq!(fs::read(paths.state_database()).unwrap(), b"replacement");
        }

        #[tokio::test(flavor = "current_thread")]
        async fn parent_directory_replacement_cannot_rebind_the_callback_path() {
            let root = tempfile::tempdir().expect("root");
            let paths = paths(root.path(), "parent-replacement");
            prepare(&paths);
            let metadata = metadata(&paths);
            let schema_catalog = base_schema_catalog();
            let state_directory = paths.state_database().parent().unwrap().to_path_buf();
            let displaced_directory = state_directory.with_file_name("parent-replacement-old");
            let displaced_for_callback = displaced_directory.clone();
            let replacement_path = paths.state_database().to_path_buf();

            let error = initialize_database(
                &paths,
                OpenMode::Initialize,
                &metadata,
                &schema_catalog,
                move |_| {
                    Box::pin(async move {
                        fs::rename(&state_directory, &displaced_for_callback)?;
                        fs::create_dir(&state_directory)?;
                        fs::write(&replacement_path, b"replacement")?;
                        fs::set_permissions(&replacement_path, fs::Permissions::from_mode(0o600))?;
                        Ok::<(), io::Error>(())
                    })
                },
            )
            .await
            .expect_err("canonical path replacement must fail");

            assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
            assert_eq!(fs::read(paths.state_database()).unwrap(), b"replacement");
            assert!(!displaced_directory.join("state.sqlite").exists());
        }

        #[tokio::test(flavor = "current_thread")]
        async fn injected_sync_and_cleanup_failures_preserve_exact_ordering() {
            let scenarios = [
                "reservation-sync",
                "database-sync",
                "directory-sync",
                "cleanup",
            ];
            for scenario in scenarios {
                let root = tempfile::tempdir().expect("root");
                let paths = paths(root.path(), scenario);
                prepare(&paths);
                let metadata = metadata(&paths);
                let schema_catalog = base_schema_catalog();
                let authority = WriterAuthority::acquire(&paths, OpenMode::Initialize)
                    .unwrap()
                    .unwrap();
                let operations = RecordingOperations::default();
                match scenario {
                    "reservation-sync" => {
                        operations.fail_directory_sync_on_call.set(Some(1));
                    }
                    "database-sync" => operations.fail_database_sync.set(true),
                    "directory-sync" => {
                        operations.fail_directory_sync_on_call.set(Some(2));
                    }
                    "cleanup" => operations.fail_unlink.set(true),
                    _ => unreachable!(),
                }
                let result = if scenario == "cleanup" {
                    initialize_with_ops(
                        &paths,
                        authority,
                        &metadata,
                        &schema_catalog,
                        |_| Box::pin(ready(Err::<(), _>(CallbackFailure))),
                        &operations,
                        &crate::failpoint::DurabilityFailpoints::default(),
                    )
                    .await
                } else {
                    initialize_with_ops(
                        &paths,
                        authority,
                        &metadata,
                        &schema_catalog,
                        |_| Box::pin(ready(Ok::<(), CallbackFailure>(()))),
                        &operations,
                        &crate::failpoint::DurabilityFailpoints::default(),
                    )
                    .await
                };
                assert_eq!(
                    result.expect_err("injected failure").kind(),
                    ServiceSqliteErrorKind::Create
                );
                let events = operations.events.borrow();
                match scenario {
                    "reservation-sync" => assert_eq!(
                        events.as_slice(),
                        ["sync_directory", "unlink_database", "sync_directory"]
                    ),
                    "database-sync" => assert_eq!(
                        events.as_slice(),
                        [
                            "sync_directory",
                            "sync_database",
                            "unlink_database",
                            "sync_directory"
                        ]
                    ),
                    "directory-sync" => assert_eq!(
                        events.as_slice(),
                        [
                            "sync_directory",
                            "sync_database",
                            "sync_directory",
                            "unlink_database",
                            "sync_directory"
                        ]
                    ),
                    "cleanup" => {
                        assert_eq!(
                            events.as_slice(),
                            ["sync_directory", "unlink_database", "unlink_database"]
                        )
                    }
                    _ => unreachable!(),
                }
            }
        }

        #[test]
        fn rollback_failure_classifier_preserves_primary_and_cleanup_precedence() {
            let ordinary =
                || InitializationCause::new(InitializationFailureKind::SchemaInitializationFailed);
            let replaced = || InitializationCause::new(InitializationFailureKind::DatabaseReplaced);
            let cleanup = || InitializationCause::new(InitializationFailureKind::CleanupFailed);

            let ordinary_success = rollback_failure(ordinary(), Ok(()));
            assert_eq!(ordinary_success.kind(), ServiceSqliteErrorKind::Create);
            assert_eq!(
                ordinary_success
                    .source()
                    .map(ToString::to_string)
                    .as_deref(),
                Some("SQLite schema initialization failed")
            );

            let replaced_failure = rollback_failure(replaced(), Err(cleanup()));
            assert_eq!(replaced_failure.kind(), ServiceSqliteErrorKind::Create);
            assert_eq!(
                replaced_failure
                    .source()
                    .map(ToString::to_string)
                    .as_deref(),
                Some("SQLite state identity changed during initialization")
            );

            let cleanup_failure = rollback_failure(ordinary(), Err(cleanup()));
            assert_eq!(cleanup_failure.kind(), ServiceSqliteErrorKind::Create);
            let outer = cleanup_failure.source().expect("cleanup source");
            assert_eq!(outer.to_string(), "SQLite initialization cleanup failed");
            assert_eq!(
                outer.source().map(ToString::to_string).as_deref(),
                Some("SQLite schema initialization failed")
            );
        }

        #[tokio::test(flavor = "current_thread")]
        async fn every_initialization_durability_edge_fails_once_and_rolls_back() {
            use crate::failpoint::{DurabilityFailpoint, DurabilityFailpoints};

            for (index, (point, expected_events)) in [
                (DurabilityFailpoint::InitializeBeforeCreate, &[][..]),
                (
                    DurabilityFailpoint::InitializeAfterCreate,
                    &["unlink_database", "sync_directory"][..],
                ),
                (
                    DurabilityFailpoint::InitializeBeforeReservationDirectorySync,
                    &["unlink_database", "sync_directory"][..],
                ),
                (
                    DurabilityFailpoint::InitializeAfterReservationDirectorySync,
                    &["sync_directory", "unlink_database", "sync_directory"][..],
                ),
                (
                    DurabilityFailpoint::InitializeBeforeFileSync,
                    &["sync_directory", "unlink_database", "sync_directory"][..],
                ),
                (
                    DurabilityFailpoint::InitializeAfterFileSync,
                    &[
                        "sync_directory",
                        "sync_database",
                        "unlink_database",
                        "sync_directory",
                    ][..],
                ),
                (
                    DurabilityFailpoint::InitializeBeforeCommitDirectorySync,
                    &[
                        "sync_directory",
                        "sync_database",
                        "unlink_database",
                        "sync_directory",
                    ][..],
                ),
                (
                    DurabilityFailpoint::InitializeAfterCommitDirectorySync,
                    &[
                        "sync_directory",
                        "sync_database",
                        "sync_directory",
                        "unlink_database",
                        "sync_directory",
                    ][..],
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let root = tempfile::tempdir().expect("root");
                let paths = paths(root.path(), &format!("failpoint-{index}"));
                prepare(&paths);
                let metadata = metadata(&paths);
                let schema_catalog = base_schema_catalog();
                let authority = WriterAuthority::acquire(&paths, OpenMode::Initialize)
                    .expect("authority acquisition")
                    .expect("initialize authority");
                let failpoints = DurabilityFailpoints::armed(point);
                let operations = RecordingOperations::default();
                let error = initialize_with_ops(
                    &paths,
                    authority,
                    &metadata,
                    &schema_catalog,
                    |_| Box::pin(ready(Ok::<(), CallbackFailure>(()))),
                    &operations,
                    &failpoints,
                )
                .await
                .expect_err("durability edge must fail");
                assert_eq!(error.kind(), ServiceSqliteErrorKind::Create);
                assert!(failpoints.fired());
                assert_eq!(
                    failpoints.reached().last(),
                    Some(&point),
                    "named edge must be the last reached boundary"
                );
                assert_eq!(
                    operations.events.borrow().as_slice(),
                    expected_events,
                    "named before/after edge must bracket the expected sync operation"
                );
                assert!(!paths.state_database().exists());
                let mut recovered = WriterAuthority::acquire(&paths, OpenMode::Initialize)
                    .expect("reacquire after rollback")
                    .expect("initialize authority");
                recovered.release().expect("release recovered authority");
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
use supported::{SystemInitializationOperations, initialize_with_ops};
