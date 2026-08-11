//! Create-new initialization for one service-owned SQLite database.

use core::{fmt, future::Future};
use std::{error::Error, path::PathBuf};

use crate::{
    OpenMode, SchemaCatalog, ServiceDatabaseMetadata, ServiceSqliteError, ServiceSqliteErrorKind,
    ServiceSqlitePaths, WriterAuthority,
};

/// Creates and initializes a missing service database while holding sole writer authority.
///
/// The callback receives the already-reserved canonical database path. It must
/// open that file without create or replacement flags, initialize its schema,
/// close every database handle, and only then resolve its future. The supplied
/// metadata must derive from the same paths; it is written and verified after
/// the callback but before the database becomes durable. Cancellation or
/// failure removes only the exact inode reserved by this call.
pub async fn initialize_database<F, Fut, E>(
    paths: &ServiceSqlitePaths,
    mode: OpenMode,
    metadata: &ServiceDatabaseMetadata,
    schema_catalog: &SchemaCatalog,
    initialize_schema: F,
) -> Result<WriterAuthority, ServiceSqliteError>
where
    F: FnOnce(PathBuf) -> Fut,
    Fut: Future<Output = Result<(), E>>,
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
        initialize_with_ops(
            paths,
            authority,
            metadata,
            schema_catalog,
            initialize_schema,
            &SystemInitializationOperations,
        )
        .await
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        drop((authority, metadata, schema_catalog, initialize_schema));
        Err(initialization_error(InitializationCause::new(
            InitializationFailureKind::CreateUnavailable,
        )))
    }
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod supported {
    use std::fs::File;

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
        fn sync_database(&self, database: &File) -> Result<(), InitializationCause> {
            database.sync_all().map_err(|_| {
                InitializationCause::new(InitializationFailureKind::DatabaseSyncFailed)
            })
        }

        fn sync_directory(&self, directory: &File) -> Result<(), InitializationCause> {
            directory.sync_all().map_err(|_| {
                InitializationCause::new(InitializationFailureKind::DirectorySyncFailed)
            })
        }

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
            if descriptor_identity != self.identity {
                return Err(InitializationCause::new(
                    InitializationFailureKind::InvalidDatabase,
                ));
            }
            self.validate_entry()
        }

        fn validate_entry(&self) -> Result<(), InitializationCause> {
            let status = statat(
                self.directory,
                radroots_runtime_paths::SERVICE_STATE_DATABASE_FILE_NAME,
                AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|_| InitializationCause::new(InitializationFailureKind::DatabaseReplaced))?;
            let device = u64::try_from(status.st_dev).map_err(|_| {
                InitializationCause::new(InitializationFailureKind::InvalidDatabase)
            })?;
            let current = validate_status(
                FileType::from_raw_mode(status.st_mode).is_file(),
                u64::from(status.st_nlink),
                status.st_uid,
                u32::from(status.st_mode),
                device,
                status.st_ino,
            )?;
            if current != self.identity {
                return Err(InitializationCause::new(
                    InitializationFailureKind::DatabaseReplaced,
                ));
            }
            Ok(())
        }

        fn current_entry_identity(&self) -> Result<FileIdentity, InitializationCause> {
            let status = statat(
                self.directory,
                radroots_runtime_paths::SERVICE_STATE_DATABASE_FILE_NAME,
                AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|_| InitializationCause::new(InitializationFailureKind::DatabaseReplaced))?;
            let device = u64::try_from(status.st_dev).map_err(|_| {
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
            let device = u64::try_from(status.st_dev).map_err(|_| {
                InitializationCause::new(InitializationFailureKind::InvalidDatabase)
            })?;
            let current = validate_status(
                FileType::from_raw_mode(status.st_mode).is_file(),
                u64::from(status.st_nlink),
                status.st_uid,
                u32::from(status.st_mode),
                device,
                status.st_ino,
            )?;
            if current != self.identity {
                return Err(InitializationCause::new(
                    InitializationFailureKind::DatabaseReplaced,
                ));
            }
            Ok(())
        }

        fn commit(&mut self, canonical_path: &std::path::Path) -> Result<(), InitializationCause> {
            self.operations.sync_database(&self.database)?;
            self.validate()?;
            self.validate_canonical_path(canonical_path)?;
            self.operations.sync_directory(self.directory)?;
            self.committed = true;
            Ok(())
        }

        fn rollback(&mut self) -> Result<(), InitializationCause> {
            if self.committed {
                return Ok(());
            }
            if self.current_entry_identity()? != self.identity {
                return Err(InitializationCause::new(
                    InitializationFailureKind::DatabaseReplaced,
                ));
            }
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
        let device = u64::try_from(status.st_dev)
            .map_err(|_| InitializationCause::new(InitializationFailureKind::InvalidDatabase))?;
        validate_status(
            FileType::from_raw_mode(status.st_mode).is_file(),
            u64::from(status.st_nlink),
            status.st_uid,
            u32::from(status.st_mode),
            device,
            status.st_ino,
        )
    }

    fn descriptor_identity(
        descriptor: &impl std::os::fd::AsFd,
    ) -> Result<FileIdentity, InitializationCause> {
        let status = fstat(descriptor)
            .map_err(|_| InitializationCause::new(InitializationFailureKind::InvalidDatabase))?;
        let device = u64::try_from(status.st_dev)
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
        if !is_regular_file
            || link_count != 1
            || actual_uid != geteuid().as_raw()
            || mode & 0o777 != 0o600
        {
            return Err(InitializationCause::new(
                InitializationFailureKind::InvalidDatabase,
            ));
        }
        Ok(FileIdentity { device, inode })
    }

    async fn fail_with_rollback<O: InitializationOperations>(
        mut pending: PendingDatabase<'_, O>,
        primary: InitializationCause,
    ) -> Result<WriterAuthority, ServiceSqliteError> {
        match pending.rollback() {
            Ok(()) => Err(initialization_error(primary)),
            Err(_cleanup) if primary.kind == InitializationFailureKind::DatabaseReplaced => {
                Err(initialization_error(primary))
            }
            Err(cleanup) => Err(initialization_error(InitializationCause::with_source(
                cleanup.kind,
                primary,
            ))),
        }
    }

    async fn fail_metadata_with_rollback<O: InitializationOperations>(
        mut pending: PendingDatabase<'_, O>,
        primary: ServiceSqliteError,
    ) -> Result<WriterAuthority, ServiceSqliteError> {
        match pending.rollback() {
            Ok(()) => Err(primary),
            Err(cleanup) => Err(initialization_error(InitializationCause::with_source(
                cleanup.kind,
                primary,
            ))),
        }
    }

    pub(super) async fn initialize_with_ops<F, Fut, E, O>(
        paths: &ServiceSqlitePaths,
        authority: WriterAuthority,
        metadata: &ServiceDatabaseMetadata,
        schema_catalog: &SchemaCatalog,
        initialize_schema: F,
        operations: &O,
    ) -> Result<WriterAuthority, ServiceSqliteError>
    where
        F: FnOnce(PathBuf) -> Fut,
        Fut: Future<Output = Result<(), E>>,
        E: Error + Send + Sync + 'static,
        O: InitializationOperations,
    {
        let mut pending = PendingDatabase::create(authority.directory(), operations)
            .map_err(initialization_error)?;
        if let Err(error) = operations.sync_directory(authority.directory()) {
            return fail_with_rollback(pending, error).await;
        }
        if let Err(error) = pending.validate_canonical_path(paths.state_database()) {
            return fail_with_rollback(pending, error).await;
        }
        let callback_result = initialize_schema(paths.state_database().to_path_buf()).await;
        if let Err(error) = callback_result {
            return fail_with_rollback(
                pending,
                InitializationCause::with_source(
                    InitializationFailureKind::SchemaInitializationFailed,
                    error,
                ),
            )
            .await;
        }
        if let Err(error) = pending.validate() {
            return fail_with_rollback(pending, error).await;
        }
        if let Err(error) = pending.validate_canonical_path(paths.state_database()) {
            return fail_with_rollback(pending, error).await;
        }
        let metadata_result = async {
            use sqlx::{ConnectOptions, Connection, sqlite::SqliteConnectOptions};

            let options = SqliteConnectOptions::new()
                .filename(paths.state_database())
                .create_if_missing(false)
                .disable_statement_logging();
            let mut connection = sqlx::SqliteConnection::connect_with(&options)
                .await
                .map_err(|source| {
                    ServiceSqliteError::with_source(ServiceSqliteErrorKind::Metadata, source)
                })?;
            let write_result =
                crate::metadata::write_database_metadata(&mut connection, metadata, schema_catalog)
                    .await;
            let close_result = connection.close().await.map_err(|source| {
                ServiceSqliteError::with_source(ServiceSqliteErrorKind::Metadata, source)
            });
            write_result.and(close_result)
        }
        .await;
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
        if let Err(error) = pending.commit(paths.state_database()) {
            return fail_with_rollback(pending, error).await;
        }
        drop(pending);
        Ok(authority)
    }

    #[cfg(test)]
    mod tests {
        use std::{
            cell::{Cell, RefCell},
            fs,
            future::{Future, pending, ready},
            io,
            num::NonZeroU32,
            os::unix::fs::{MetadataExt, PermissionsExt, symlink},
            path::Path,
            pin::Pin,
            task::{Context, Poll, Waker},
        };

        use radroots_runtime_paths::{
            InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
            RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource,
            ServiceId,
        };
        use radroots_storage::event::SourceGeneration;

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

        fn poll_once<F: Future>(future: F) -> (Poll<F::Output>, Pin<Box<F>>) {
            let mut future = Box::pin(future);
            let mut context = Context::from_waker(Waker::noop());
            let result = future.as_mut().poll(&mut context);
            (result, future)
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
            let expected_path = paths.state_database().to_path_buf();
            let metadata = metadata(&paths);
            let schema_catalog = service_schema_catalog();
            let mut authority = initialize_database(
                &paths,
                OpenMode::Initialize,
                &metadata,
                &schema_catalog,
                |path| async move {
                    assert_eq!(path, expected_path);
                    use sqlx::{ConnectOptions, Connection, sqlite::SqliteConnectOptions};

                    let options = SqliteConnectOptions::new()
                        .filename(path)
                        .create_if_missing(false)
                        .disable_statement_logging();
                    let mut connection = sqlx::SqliteConnection::connect_with(&options).await?;
                    sqlx::query("CREATE TABLE service_schema (id INTEGER PRIMARY KEY)")
                        .execute(&mut connection)
                        .await?;
                    connection.close().await?;
                    Ok::<(), sqlx::Error>(())
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
                        ready(Ok::<(), CallbackFailure>(()))
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
                    ready(Ok::<(), CallbackFailure>(()))
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
                |_| ready(Err::<(), _>(CallbackFailure)),
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
                |_| ready(Ok::<(), CallbackFailure>(())),
            )
            .await
            .expect("retry after cleanup");
            assert!(retry.is_held());
            assert!(paths.state_database().exists());
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
                    move |path| async move {
                        use sqlx::{ConnectOptions, Connection, sqlite::SqliteConnectOptions};

                        let options = SqliteConnectOptions::new()
                            .filename(path)
                            .create_if_missing(false)
                            .disable_statement_logging();
                        let mut connection = sqlx::SqliteConnection::connect_with(&options).await?;
                        sqlx::query(statement).execute(&mut connection).await?;
                        connection.close().await?;
                        Ok::<(), sqlx::Error>(())
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
                |path| async move {
                    use sqlx::{ConnectOptions, Connection, sqlite::SqliteConnectOptions};

                    let options = SqliteConnectOptions::new()
                        .filename(path)
                        .create_if_missing(false)
                        .disable_statement_logging();
                    let mut connection = sqlx::SqliteConnection::connect_with(&options).await?;
                    sqlx::query("CREATE TABLE unexpected (value INTEGER)")
                        .execute(&mut connection)
                        .await?;
                    connection.close().await?;
                    Ok::<(), sqlx::Error>(())
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
            let (poll, future) = poll_once(initialize_database(
                &paths,
                OpenMode::Initialize,
                &metadata,
                &schema_catalog,
                |_| pending::<Result<(), CallbackFailure>>(),
            ));
            assert!(poll.is_pending());
            assert!(paths.state_database().exists());
            assert!(WriterAuthority::acquire(&paths, OpenMode::Initialize).is_err());
            drop(future);
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
                move |path| async move {
                    fs::remove_file(&path)?;
                    fs::write(&replacement_path, b"replacement")?;
                    Ok::<(), io::Error>(())
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
                move |_| async move {
                    fs::rename(&state_directory, &displaced_for_callback)?;
                    fs::create_dir(&state_directory)?;
                    fs::write(&replacement_path, b"replacement")?;
                    fs::set_permissions(&replacement_path, fs::Permissions::from_mode(0o600))?;
                    Ok::<(), io::Error>(())
                },
            )
            .await
            .expect_err("canonical path replacement must fail");

            assert_eq!(error.kind(), ServiceSqliteErrorKind::Create);
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
                        |_| ready(Err::<(), _>(CallbackFailure)),
                        &operations,
                    )
                    .await
                } else {
                    initialize_with_ops(
                        &paths,
                        authority,
                        &metadata,
                        &schema_catalog,
                        |_| ready(Ok::<(), CallbackFailure>(())),
                        &operations,
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
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
use supported::{SystemInitializationOperations, initialize_with_ops};
