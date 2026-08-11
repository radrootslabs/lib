//! Instance-bound SQLite paths and declarative open modes.

use core::fmt;
use std::{
    error::Error,
    path::{Path, PathBuf},
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::{
    fs::File,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use fs2::FileExt;
use radroots_runtime_paths::{
    InstanceId, RuntimeContext, ServiceId, default_service_instance_artifacts,
};
use serde::Serialize;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use sqlx::{
    ConnectOptions, Connection, Sqlite, SqliteConnection, SqlitePool,
    pool::PoolConnection,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::{
    MigrationAppliedAtUnixSeconds, MigrationBuildIdentity, MigrationCatalog,
    ServiceDatabaseIdentity, ServiceSqliteConnectionOptions, ServiceSqliteError,
    ServiceSqliteErrorKind, WriterAuthority,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
const STATEMENT_CACHE_CAPACITY: usize = 100;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const COMMAND_BUFFER_CAPACITY: usize = 50;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const ROW_BUFFER_CAPACITY: usize = 50;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const WAL_FILE_NAME: &str = "state.sqlite-wal";
#[cfg(any(target_os = "linux", target_os = "macos"))]
const SHARED_MEMORY_FILE_NAME: &str = "state.sqlite-shm";

/// Canonical database and writer-lock paths for one validated service instance.
///
/// Callers cannot forge paths or rebind the service and instance independently:
///
/// ```compile_fail
/// use std::path::PathBuf;
/// use radroots_runtime_paths::{InstanceId, ServiceId};
/// use radroots_service_sqlite::ServiceSqlitePaths;
///
/// let _ = ServiceSqlitePaths {
///     service: ServiceId::new("myc").unwrap(),
///     instance: InstanceId::new("primary").unwrap(),
///     state_database: PathBuf::from("/tmp/alternate.sqlite"),
///     state_lock: PathBuf::from("/tmp/alternate.lock"),
/// };
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct ServiceSqlitePaths {
    service: ServiceId,
    instance: InstanceId,
    state_database: PathBuf,
    state_lock: PathBuf,
}

impl ServiceSqlitePaths {
    /// Derives the fixed SQLite artifacts from one immutable runtime context.
    pub fn from_runtime_context(context: &RuntimeContext) -> Result<Self, ServiceSqlitePathError> {
        validate_state_directory(context.paths().state())?;
        let artifacts = default_service_instance_artifacts(context.paths());
        Ok(Self {
            service: context.service().clone(),
            instance: context.instance().clone(),
            state_database: artifacts.state_database().to_path_buf(),
            state_lock: artifacts.state_lock().to_path_buf(),
        })
    }

    /// Returns the validated service identity bound to these paths.
    #[must_use]
    pub fn service(&self) -> &ServiceId {
        &self.service
    }

    /// Returns the validated instance identity bound to these paths.
    #[must_use]
    pub fn instance(&self) -> &InstanceId {
        &self.instance
    }

    /// Returns the canonical `state.sqlite` path.
    #[must_use]
    pub fn state_database(&self) -> &Path {
        &self.state_database
    }

    /// Returns the canonical retained `state.lock` path.
    #[must_use]
    pub fn state_lock(&self) -> &Path {
        &self.state_lock
    }
}

impl fmt::Debug for ServiceSqlitePaths {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceSqlitePaths")
            .field("service", &self.service)
            .field("instance", &self.instance)
            .field("state_database", &"[redacted]")
            .field("state_lock", &"[redacted]")
            .finish()
    }
}

/// Path-shape failure detected before any filesystem or SQLite operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceSqlitePathError {
    RelativeStateDirectory,
    MissingStateDirectoryParent,
}

impl fmt::Display for ServiceSqlitePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RelativeStateDirectory => {
                formatter.write_str("SQLite state directory must be absolute")
            }
            Self::MissingStateDirectoryParent => {
                formatter.write_str("SQLite state directory must have a parent")
            }
        }
    }
}

impl Error for ServiceSqlitePathError {}

fn validate_state_directory(path: &Path) -> Result<(), ServiceSqlitePathError> {
    if !path.is_absolute() {
        return Err(ServiceSqlitePathError::RelativeStateDirectory);
    }
    if path.parent().is_none() {
        return Err(ServiceSqlitePathError::MissingStateDirectoryParent);
    }
    Ok(())
}

/// Declarative behavior for opening one service-owned SQLite database.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenMode {
    Initialize,
    ReadWriteExisting,
    ReadOnlyInspection,
}

impl OpenMode {
    /// Returns whether this mode permits creating missing state.
    #[must_use]
    pub const fn may_create(self) -> bool {
        matches!(self, Self::Initialize)
    }

    /// Returns whether state must already exist before opening.
    #[must_use]
    pub const fn requires_existing(self) -> bool {
        !matches!(self, Self::Initialize)
    }

    /// Returns whether exclusive writer authority is required.
    #[must_use]
    pub const fn requires_writer_authority(self) -> bool {
        !matches!(self, Self::ReadOnlyInspection)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    dead_code,
    reason = "Step 056 keeps the pool private until the Step 061 host boundary"
)]
struct PrivateConnectionPool {
    pool: SqlitePool,
    binding: DirectoryBinding,
    paths: ServiceSqlitePaths,
    catalog: MigrationCatalog,
    authority: Option<WriterAuthority>,
    inspection_guard: Option<ReadOnlyInspectionGuard>,
    authority_failure: Arc<AtomicBool>,
    metadata_failure: Arc<AtomicBool>,
    migration_failure: Arc<AtomicBool>,
    pragma_failure: Arc<AtomicBool>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    dead_code,
    reason = "Step 056 keeps pool lifecycle private until the Step 061 host boundary"
)]
impl PrivateConnectionPool {
    fn connection_failure_kind(&self) -> ServiceSqliteErrorKind {
        if self.authority_failure.load(Ordering::Acquire) {
            ServiceSqliteErrorKind::Authority
        } else if self.metadata_failure.load(Ordering::Acquire) {
            ServiceSqliteErrorKind::Metadata
        } else if self.migration_failure.load(Ordering::Acquire) {
            ServiceSqliteErrorKind::Migration
        } else if self.pragma_failure.load(Ordering::Acquire) {
            ServiceSqliteErrorKind::Pragma
        } else {
            ServiceSqliteErrorKind::Open
        }
    }

    async fn acquire(&self) -> Result<PoolConnection<Sqlite>, ServiceSqliteError> {
        self.binding.validate(&self.paths)?;
        let result = self.pool.acquire().await;
        self.binding.validate(&self.paths)?;
        let mut connection =
            result.map_err(|source| connection_source(self.connection_failure_kind(), source))?;
        let history =
            crate::migration::verify_migration_history(&mut connection, &self.catalog, true).await;
        self.binding.validate(&self.paths)?;
        history?;
        Ok(connection)
    }

    async fn apply_migrations(
        &self,
        applied_at: MigrationAppliedAtUnixSeconds,
        build: &MigrationBuildIdentity,
        callbacks: &[crate::migration::MigrationCallbackBinding],
    ) -> Result<crate::migration::MigrationApplicationOutcome, ServiceSqliteError> {
        let authority = self
            .authority
            .as_ref()
            .ok_or_else(|| ServiceSqliteError::new(ServiceSqliteErrorKind::Authority))?;
        authority.validate_for(&self.paths)?;
        self.binding.validate(&self.paths)?;
        let acquired = self.pool.acquire().await;
        authority.validate_for(&self.paths)?;
        self.binding.validate(&self.paths)?;
        let mut connection =
            acquired.map_err(|source| connection_source(self.connection_failure_kind(), source))?;
        // Migration execution installs connection-local fail-closed guards. Always
        // discard this one-time connection so cancellation cannot return a guarded
        // or callback-altered handle to the pool.
        connection.close_on_drop();
        let mut validate_authority = || {
            authority.validate_for(&self.paths)?;
            self.binding.validate(&self.paths)
        };
        let result = crate::migration::apply_governed_migrations(
            &mut connection,
            &self.catalog,
            applied_at,
            build,
            callbacks,
            &mut validate_authority,
        )
        .await;
        authority.validate_for(&self.paths)?;
        self.binding.validate(&self.paths)?;
        result
    }

    async fn close(mut self) -> Option<WriterAuthority> {
        self.pool.close().await;
        self.inspection_guard.take();
        self.authority.take()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    dead_code,
    reason = "Step 056 keeps pool opening private until the Step 061 host boundary"
)]
async fn open_existing_connection_pool(
    paths: &ServiceSqlitePaths,
    identity: &ServiceDatabaseIdentity,
    catalog: &MigrationCatalog,
    mode: OpenMode,
    policy: ServiceSqliteConnectionOptions,
) -> Result<PrivateConnectionPool, ServiceSqliteError> {
    if mode == OpenMode::Initialize {
        return Err(connection_error(
            ServiceSqliteErrorKind::Open,
            ConnectionFailureKind::UnsupportedMode,
        ));
    }
    let (authority, inspection_guard) = match mode {
        OpenMode::ReadWriteExisting => (WriterAuthority::acquire(paths, mode)?, None),
        OpenMode::ReadOnlyInspection => (None, Some(ReadOnlyInspectionGuard::acquire(paths)?)),
        OpenMode::Initialize => unreachable!("initialize mode returned above"),
    };
    open_connection_pool(
        paths,
        identity,
        catalog,
        mode,
        policy,
        authority,
        inspection_guard,
    )
    .await
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    dead_code,
    reason = "Step 056 keeps pool opening private until the Step 061 host boundary"
)]
async fn open_initialized_connection_pool(
    paths: &ServiceSqlitePaths,
    identity: &ServiceDatabaseIdentity,
    catalog: &MigrationCatalog,
    policy: ServiceSqliteConnectionOptions,
    authority: WriterAuthority,
) -> Result<PrivateConnectionPool, ServiceSqliteError> {
    authority.validate_for(paths)?;
    open_connection_pool(
        paths,
        identity,
        catalog,
        OpenMode::Initialize,
        policy,
        Some(authority),
        None,
    )
    .await
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    dead_code,
    reason = "Step 056 keeps pool construction private until the Step 061 host boundary"
)]
async fn open_connection_pool(
    paths: &ServiceSqlitePaths,
    identity: &ServiceDatabaseIdentity,
    catalog: &MigrationCatalog,
    mode: OpenMode,
    policy: ServiceSqliteConnectionOptions,
    authority: Option<WriterAuthority>,
    inspection_guard: Option<ReadOnlyInspectionGuard>,
) -> Result<PrivateConnectionPool, ServiceSqliteError> {
    if !identity.matches_paths(paths) {
        return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Metadata));
    }
    if identity.supported_state_schema_version().get() != catalog.current_version() {
        return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Migration));
    }
    let binding = match (mode, authority.as_ref(), inspection_guard.as_ref()) {
        (OpenMode::Initialize | OpenMode::ReadWriteExisting, Some(authority), None) => {
            authority.validate_for(paths)?;
            DirectoryBinding::capture(authority.directory(), paths)?
        }
        (OpenMode::ReadOnlyInspection, None, Some(inspection_guard)) => {
            DirectoryBinding::capture(&inspection_guard.directory, paths)?
        }
        _ => {
            return Err(connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            ));
        }
    };

    let connect_options = sqlite_connect_options(paths, mode, policy);
    binding.validate(paths)?;
    let preflight_result = SqliteConnection::connect_with(&connect_options).await;
    binding.validate(paths)?;
    let mut preflight = preflight_result
        .map_err(|source| connection_source(ServiceSqliteErrorKind::Open, source))?;
    let preflight_policy = verify_connection_policy(&mut preflight, mode, policy).await;
    binding.validate(paths)?;
    preflight_policy.map_err(|source| connection_source(ServiceSqliteErrorKind::Pragma, source))?;
    let preflight_metadata =
        crate::metadata::verify_database_metadata(&mut preflight, identity).await;
    binding.validate(paths)?;
    preflight_metadata?;
    let preflight_history = crate::migration::verify_migration_history(
        &mut preflight,
        catalog,
        mode == OpenMode::ReadOnlyInspection,
    )
    .await;
    binding.validate(paths)?;
    preflight_history?;
    let preflight_close = preflight.close().await;
    binding.validate(paths)?;
    preflight_close.map_err(|source| connection_source(ServiceSqliteErrorKind::Open, source))?;

    let after_policy = policy;
    let before_policy = policy;
    let after_mode = mode;
    let before_mode = mode;
    let retained_binding = binding.clone();
    let after_binding = binding.clone();
    let before_binding = binding.clone();
    let pool_binding = binding;
    let after_paths = paths.clone();
    let before_paths = paths.clone();
    let after_metadata = identity.clone();
    let before_metadata = identity.clone();
    let retained_catalog = catalog.clone();
    let after_catalog = catalog.clone();
    let before_catalog = catalog.clone();
    let authority_failure = Arc::new(AtomicBool::new(false));
    let metadata_failure = Arc::new(AtomicBool::new(false));
    let migration_failure = Arc::new(AtomicBool::new(false));
    let pragma_failure = Arc::new(AtomicBool::new(false));
    let after_authority_failure = Arc::clone(&authority_failure);
    let after_metadata_failure = Arc::clone(&metadata_failure);
    let after_migration_failure = Arc::clone(&migration_failure);
    let after_pragma_failure = Arc::clone(&pragma_failure);
    let before_authority_failure = Arc::clone(&authority_failure);
    let before_metadata_failure = Arc::clone(&metadata_failure);
    let before_migration_failure = Arc::clone(&migration_failure);
    let before_pragma_failure = Arc::clone(&pragma_failure);
    let pool_result = SqlitePoolOptions::new()
        .min_connections(1)
        .max_connections(policy.max_connections())
        .acquire_timeout(policy.busy_timeout())
        .idle_timeout(None)
        .max_lifetime(None)
        .test_before_acquire(true)
        .after_connect(move |connection, _metadata| {
            let binding = after_binding.clone();
            let paths = after_paths.clone();
            let metadata = after_metadata.clone();
            let catalog = after_catalog.clone();
            let authority_failure = Arc::clone(&after_authority_failure);
            let metadata_failure = Arc::clone(&after_metadata_failure);
            let migration_failure = Arc::clone(&after_migration_failure);
            let pragma_failure = Arc::clone(&after_pragma_failure);
            Box::pin(async move {
                if binding.validate(&paths).is_err() {
                    authority_failure.store(true, Ordering::Release);
                    return Err(sqlx::Error::Protocol(
                        "SQLite connection authority mismatch".to_owned(),
                    ));
                }
                let policy_result =
                    connection_policy_matches(connection, after_mode, after_policy).await;
                if binding.validate(&paths).is_err() {
                    authority_failure.store(true, Ordering::Release);
                    return Err(sqlx::Error::Protocol(
                        "SQLite connection authority mismatch".to_owned(),
                    ));
                }
                let matches = policy_result.inspect_err(|_| {
                    pragma_failure.store(true, Ordering::Release);
                })?;
                if !matches {
                    pragma_failure.store(true, Ordering::Release);
                    return Err(sqlx::Error::Protocol(
                        "SQLite connection policy mismatch".to_owned(),
                    ));
                }
                let metadata_result =
                    crate::metadata::verify_database_metadata(connection, &metadata).await;
                if binding.validate(&paths).is_err() {
                    authority_failure.store(true, Ordering::Release);
                    return Err(sqlx::Error::Protocol(
                        "SQLite connection authority mismatch".to_owned(),
                    ));
                }
                if metadata_result.is_err() {
                    metadata_failure.store(true, Ordering::Release);
                    return Err(sqlx::Error::Protocol(
                        "SQLite connection metadata mismatch".to_owned(),
                    ));
                }
                let migration_result = crate::migration::verify_migration_history(
                    connection,
                    &catalog,
                    after_mode == OpenMode::ReadOnlyInspection,
                )
                .await;
                if binding.validate(&paths).is_err() {
                    authority_failure.store(true, Ordering::Release);
                    return Err(sqlx::Error::Protocol(
                        "SQLite connection authority mismatch".to_owned(),
                    ));
                }
                if migration_result.is_err() {
                    migration_failure.store(true, Ordering::Release);
                    return Err(sqlx::Error::Protocol(
                        "SQLite migration history mismatch".to_owned(),
                    ));
                }
                Ok(())
            })
        })
        .before_acquire(move |connection, _metadata| {
            let binding = before_binding.clone();
            let paths = before_paths.clone();
            let metadata = before_metadata.clone();
            let catalog = before_catalog.clone();
            let authority_failure = Arc::clone(&before_authority_failure);
            let metadata_failure = Arc::clone(&before_metadata_failure);
            let migration_failure = Arc::clone(&before_migration_failure);
            let pragma_failure = Arc::clone(&before_pragma_failure);
            Box::pin(async move {
                if binding.validate(&paths).is_err() {
                    authority_failure.store(true, Ordering::Release);
                    return Err(sqlx::Error::Protocol(
                        "SQLite connection authority mismatch".to_owned(),
                    ));
                }
                let policy_result =
                    connection_policy_matches(connection, before_mode, before_policy).await;
                if binding.validate(&paths).is_err() {
                    authority_failure.store(true, Ordering::Release);
                    return Err(sqlx::Error::Protocol(
                        "SQLite connection authority mismatch".to_owned(),
                    ));
                }
                let matches = policy_result.inspect_err(|_| {
                    pragma_failure.store(true, Ordering::Release);
                })?;
                if !matches {
                    pragma_failure.store(true, Ordering::Release);
                    return Ok(false);
                }
                let metadata_result =
                    crate::metadata::verify_database_metadata(connection, &metadata).await;
                if binding.validate(&paths).is_err() {
                    authority_failure.store(true, Ordering::Release);
                    return Err(sqlx::Error::Protocol(
                        "SQLite connection authority mismatch".to_owned(),
                    ));
                }
                if metadata_result.is_err() {
                    metadata_failure.store(true, Ordering::Release);
                    return Err(sqlx::Error::Protocol(
                        "SQLite connection metadata mismatch".to_owned(),
                    ));
                }
                let migration_result = crate::migration::verify_migration_history(
                    connection,
                    &catalog,
                    before_mode == OpenMode::ReadOnlyInspection,
                )
                .await;
                if binding.validate(&paths).is_err() {
                    authority_failure.store(true, Ordering::Release);
                    return Err(sqlx::Error::Protocol(
                        "SQLite connection authority mismatch".to_owned(),
                    ));
                }
                if migration_result.is_err() {
                    migration_failure.store(true, Ordering::Release);
                    return Err(sqlx::Error::Protocol(
                        "SQLite migration history mismatch".to_owned(),
                    ));
                }
                Ok(true)
            })
        })
        .connect_with(connect_options)
        .await;
    pool_binding.validate(paths)?;
    let pool = pool_result.map_err(|source| {
        let kind = if authority_failure.load(Ordering::Acquire) {
            ServiceSqliteErrorKind::Authority
        } else if metadata_failure.load(Ordering::Acquire) {
            ServiceSqliteErrorKind::Metadata
        } else if migration_failure.load(Ordering::Acquire) {
            ServiceSqliteErrorKind::Migration
        } else if pragma_failure.load(Ordering::Acquire) {
            ServiceSqliteErrorKind::Pragma
        } else {
            ServiceSqliteErrorKind::Open
        };
        connection_source(kind, source)
    })?;

    Ok(PrivateConnectionPool {
        pool,
        binding: retained_binding,
        paths: paths.clone(),
        catalog: retained_catalog,
        authority,
        inspection_guard,
        authority_failure,
        metadata_failure,
        migration_failure,
        pragma_failure,
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    dead_code,
    reason = "Step 056 keeps SQLx options private until the Step 061 host boundary"
)]
fn sqlite_connect_options(
    paths: &ServiceSqlitePaths,
    mode: OpenMode,
    policy: ServiceSqliteConnectionOptions,
) -> SqliteConnectOptions {
    let mut options = SqliteConnectOptions::new()
        .filename(paths.state_database())
        .read_only(mode == OpenMode::ReadOnlyInspection)
        .create_if_missing(false)
        .foreign_keys(true)
        .busy_timeout(policy.busy_timeout())
        .synchronous(SqliteSynchronous::Full)
        .pragma("trusted_schema", "OFF")
        .pragma(
            "query_only",
            if mode == OpenMode::ReadOnlyInspection {
                "ON"
            } else {
                "OFF"
            },
        )
        .statement_cache_capacity(STATEMENT_CACHE_CAPACITY)
        .command_buffer_size(COMMAND_BUFFER_CAPACITY)
        .row_buffer_size(ROW_BUFFER_CAPACITY)
        .disable_statement_logging();
    if mode != OpenMode::ReadOnlyInspection {
        options = options.journal_mode(SqliteJournalMode::Wal);
    } else {
        options = options.immutable(true);
    }
    options
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    dead_code,
    reason = "Step 056 keeps pragma verification private until the Step 061 host boundary"
)]
async fn verify_connection_policy(
    connection: &mut SqliteConnection,
    mode: OpenMode,
    policy: ServiceSqliteConnectionOptions,
) -> Result<(), sqlx::Error> {
    if connection_policy_matches(connection, mode, policy).await? {
        Ok(())
    } else {
        Err(sqlx::Error::Protocol(
            "SQLite connection policy mismatch".to_owned(),
        ))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    dead_code,
    reason = "Step 056 keeps pragma verification private until the Step 061 host boundary"
)]
async fn connection_policy_matches(
    connection: &mut SqliteConnection,
    mode: OpenMode,
    policy: ServiceSqliteConnectionOptions,
) -> Result<bool, sqlx::Error> {
    let journal_mode = sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
        .fetch_one(&mut *connection)
        .await?;
    let synchronous = sqlx::query_scalar::<_, i64>("PRAGMA synchronous")
        .fetch_one(&mut *connection)
        .await?;
    let foreign_keys = sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
        .fetch_one(&mut *connection)
        .await?;
    let trusted_schema = sqlx::query_scalar::<_, i64>("PRAGMA trusted_schema")
        .fetch_one(&mut *connection)
        .await?;
    let busy_timeout = sqlx::query_scalar::<_, i64>("PRAGMA busy_timeout")
        .fetch_one(&mut *connection)
        .await?;
    let query_only = sqlx::query_scalar::<_, i64>("PRAGMA query_only")
        .fetch_one(&mut *connection)
        .await?;
    // SQLite reports `delete` for immutable handles; the inspection guard
    // independently verifies WAL read/write header bytes before this opens.
    let journal_mode_matches = if mode == OpenMode::ReadOnlyInspection {
        journal_mode.eq_ignore_ascii_case("delete")
    } else {
        journal_mode.eq_ignore_ascii_case("wal")
    };
    Ok(journal_mode_matches
        && synchronous == 2
        && foreign_keys == 1
        && trusted_schema == 0
        && busy_timeout == policy.busy_timeout_milliseconds()
        && query_only == i64::from(mode == OpenMode::ReadOnlyInspection))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    dead_code,
    reason = "Step 056 keeps connection failures private until the Step 061 host boundary"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConnectionFailureKind {
    UnsupportedMode,
    AuthorityMismatch,
    InspectionUnavailable,
    InspectionContended,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Display for ConnectionFailureKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedMode => "SQLite initialize mode requires reserved state",
            Self::AuthorityMismatch => "SQLite writer authority is missing or mismatched",
            Self::InspectionUnavailable => "SQLite inspection authority is unavailable",
            Self::InspectionContended => "SQLite inspection requires an offline writer",
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Error for ConnectionFailureKind {}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    dead_code,
    reason = "Step 056 keeps connection failures private until the Step 061 host boundary"
)]
fn connection_error(
    kind: ServiceSqliteErrorKind,
    cause: ConnectionFailureKind,
) -> ServiceSqliteError {
    ServiceSqliteError::with_source(kind, cause)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    dead_code,
    reason = "Step 056 keeps dependency causes private until the Step 061 host boundary"
)]
fn connection_source(kind: ServiceSqliteErrorKind, cause: sqlx::Error) -> ServiceSqliteError {
    ServiceSqliteError::with_source(kind, cause)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    dead_code,
    reason = "Step 056 keeps inspection authority private until the Step 061 host boundary"
)]
struct ReadOnlyInspectionGuard {
    lock: File,
    directory: File,
    _database: File,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    dead_code,
    reason = "Step 056 keeps inspection authority private until the Step 061 host boundary"
)]
impl ReadOnlyInspectionGuard {
    fn acquire(paths: &ServiceSqlitePaths) -> Result<Self, ServiceSqliteError> {
        use rustix::{
            fs::{AtFlags, FileType, Mode, OFlags, fstat, open, openat, statat},
            process::geteuid,
        };

        let directory = open(
            paths
                .state_lock()
                .parent()
                .ok_or_else(|| inspection_error(ConnectionFailureKind::InspectionUnavailable))?,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| inspection_error(ConnectionFailureKind::InspectionUnavailable))?;
        let directory_status = fstat(&directory)
            .map_err(|_| inspection_error(ConnectionFailureKind::InspectionUnavailable))?;
        if !FileType::from_raw_mode(directory_status.st_mode).is_dir()
            || directory_status.st_uid != geteuid().as_raw()
            || u32::from(directory_status.st_mode) & 0o022 != 0
        {
            return Err(inspection_error(
                ConnectionFailureKind::InspectionUnavailable,
            ));
        }
        let lock = openat(
            &directory,
            radroots_runtime_paths::SERVICE_STATE_LOCK_FILE_NAME,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|_| inspection_error(ConnectionFailureKind::InspectionUnavailable))?;
        let lock_status = fstat(&lock)
            .map_err(|_| inspection_error(ConnectionFailureKind::InspectionUnavailable))?;
        if !FileType::from_raw_mode(lock_status.st_mode).is_file()
            || u64::from(lock_status.st_nlink) != 1
            || lock_status.st_uid != geteuid().as_raw()
            || u32::from(lock_status.st_mode) & 0o777 != 0o600
        {
            return Err(inspection_error(
                ConnectionFailureKind::InspectionUnavailable,
            ));
        }
        let lock = File::from(lock);
        FileExt::try_lock_shared(&lock).map_err(|error| {
            if error.kind() == std::io::ErrorKind::WouldBlock {
                inspection_error(ConnectionFailureKind::InspectionContended)
            } else {
                inspection_error(ConnectionFailureKind::InspectionUnavailable)
            }
        })?;
        for sidecar in [WAL_FILE_NAME, SHARED_MEMORY_FILE_NAME] {
            match statat(&directory, sidecar, AtFlags::SYMLINK_NOFOLLOW) {
                Err(error) if error == rustix::io::Errno::NOENT => {}
                Ok(_) | Err(_) => {
                    return Err(inspection_error(
                        ConnectionFailureKind::InspectionUnavailable,
                    ));
                }
            }
        }
        let database = openat(
            &directory,
            radroots_runtime_paths::SERVICE_STATE_DATABASE_FILE_NAME,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| {
            connection_error(
                ServiceSqliteErrorKind::Open,
                ConnectionFailureKind::InspectionUnavailable,
            )
        })?;
        let database_status = fstat(&database).map_err(|_| {
            connection_error(
                ServiceSqliteErrorKind::Open,
                ConnectionFailureKind::InspectionUnavailable,
            )
        })?;
        if !FileType::from_raw_mode(database_status.st_mode).is_file()
            || u64::from(database_status.st_nlink) != 1
            || database_status.st_uid != geteuid().as_raw()
            || u32::from(database_status.st_mode) & 0o777 != 0o600
        {
            return Err(connection_error(
                ServiceSqliteErrorKind::Open,
                ConnectionFailureKind::InspectionUnavailable,
            ));
        }
        let database = File::from(database);
        let mut sqlite_header = [0_u8; 20];
        std::os::unix::fs::FileExt::read_exact_at(&database, &mut sqlite_header, 0).map_err(
            |_| {
                connection_error(
                    ServiceSqliteErrorKind::Open,
                    ConnectionFailureKind::InspectionUnavailable,
                )
            },
        )?;
        if &sqlite_header[..16] != b"SQLite format 3\0"
            || sqlite_header[18] != 2
            || sqlite_header[19] != 2
        {
            return Err(connection_error(
                ServiceSqliteErrorKind::Pragma,
                ConnectionFailureKind::InspectionUnavailable,
            ));
        }
        Ok(Self {
            lock,
            directory: File::from(directory),
            _database: database,
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for ReadOnlyInspectionGuard {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.lock);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn inspection_error(cause: ConnectionFailureKind) -> ServiceSqliteError {
    connection_error(ServiceSqliteErrorKind::Authority, cause)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone)]
struct DirectoryBinding {
    database_path: PathBuf,
    directory: Arc<File>,
    directory_device: u64,
    directory_inode: u64,
    database: Arc<File>,
    database_device: u64,
    database_inode: u64,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl DirectoryBinding {
    fn capture(directory: &File, paths: &ServiceSqlitePaths) -> Result<Self, ServiceSqliteError> {
        use rustix::{
            fs::{FileType, Mode, OFlags, fstat, openat},
            process::geteuid,
        };

        let directory_status = fstat(directory).map_err(|_| {
            connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            )
        })?;
        let database = openat(
            directory,
            radroots_runtime_paths::SERVICE_STATE_DATABASE_FILE_NAME,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            connection_error(
                if error == rustix::io::Errno::NOENT {
                    ServiceSqliteErrorKind::Open
                } else {
                    ServiceSqliteErrorKind::Authority
                },
                ConnectionFailureKind::AuthorityMismatch,
            )
        })?;
        let database_status = fstat(&database).map_err(|_| {
            connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            )
        })?;
        if !FileType::from_raw_mode(database_status.st_mode).is_file()
            || u64::from(database_status.st_nlink) != 1
            || database_status.st_uid != geteuid().as_raw()
            || u32::from(database_status.st_mode) & 0o777 != 0o600
        {
            return Err(connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            ));
        }
        Ok(Self {
            database_path: paths.state_database().to_path_buf(),
            directory: Arc::new(directory.try_clone().map_err(|_| {
                connection_error(
                    ServiceSqliteErrorKind::Authority,
                    ConnectionFailureKind::AuthorityMismatch,
                )
            })?),
            directory_device: u64::try_from(directory_status.st_dev).map_err(|_| {
                connection_error(
                    ServiceSqliteErrorKind::Authority,
                    ConnectionFailureKind::AuthorityMismatch,
                )
            })?,
            directory_inode: directory_status.st_ino,
            database: Arc::new(File::from(database)),
            database_device: u64::try_from(database_status.st_dev).map_err(|_| {
                connection_error(
                    ServiceSqliteErrorKind::Authority,
                    ConnectionFailureKind::AuthorityMismatch,
                )
            })?,
            database_inode: database_status.st_ino,
        })
    }

    fn validate(&self, paths: &ServiceSqlitePaths) -> Result<(), ServiceSqliteError> {
        use rustix::{
            fs::{FileType, Mode, OFlags, fstat, open, openat},
            process::geteuid,
        };

        if self.database_path != paths.state_database() {
            return Err(connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            ));
        }
        let directory = open(
            paths.state_database().parent().ok_or_else(|| {
                connection_error(
                    ServiceSqliteErrorKind::Authority,
                    ConnectionFailureKind::AuthorityMismatch,
                )
            })?,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| {
            connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            )
        })?;
        let held_directory_status = fstat(&*self.directory).map_err(|_| {
            connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            )
        })?;
        let directory_status = fstat(&directory).map_err(|_| {
            connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            )
        })?;
        let directory_device = u64::try_from(directory_status.st_dev).map_err(|_| {
            connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            )
        })?;
        let held_directory_device = u64::try_from(held_directory_status.st_dev).map_err(|_| {
            connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            )
        })?;
        if directory_device != self.directory_device
            || directory_status.st_ino != self.directory_inode
            || held_directory_device != self.directory_device
            || held_directory_status.st_ino != self.directory_inode
        {
            return Err(connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            ));
        }

        let database = openat(
            &directory,
            radroots_runtime_paths::SERVICE_STATE_DATABASE_FILE_NAME,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| {
            connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            )
        })?;
        let held_database_status = fstat(&*self.database).map_err(|_| {
            connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            )
        })?;
        let database_status = fstat(&database).map_err(|_| {
            connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            )
        })?;
        let database_device = u64::try_from(database_status.st_dev).map_err(|_| {
            connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            )
        })?;
        let held_database_device = u64::try_from(held_database_status.st_dev).map_err(|_| {
            connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            )
        })?;
        if !FileType::from_raw_mode(database_status.st_mode).is_file()
            || u64::from(database_status.st_nlink) != 1
            || database_status.st_uid != geteuid().as_raw()
            || u32::from(database_status.st_mode) & 0o777 != 0o600
            || !FileType::from_raw_mode(held_database_status.st_mode).is_file()
            || u64::from(held_database_status.st_nlink) != 1
            || held_database_status.st_uid != geteuid().as_raw()
            || u32::from(held_database_status.st_mode) & 0o777 != 0o600
            || database_device != self.database_device
            || database_status.st_ino != self.database_inode
            || held_database_device != self.database_device
            || held_database_status.st_ino != self.database_inode
        {
            return Err(connection_error(
                ServiceSqliteErrorKind::Authority,
                ConnectionFailureKind::AuthorityMismatch,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU32, path::PathBuf};

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use std::{
        collections::BTreeMap,
        convert::Infallible,
        fs,
        os::unix::fs::{PermissionsExt, symlink},
        sync::atomic::{AtomicUsize, Ordering as AtomicOrdering},
        time::{Duration, SystemTime},
    };

    use radroots_runtime_paths::{
        RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver, RadrootsPlatform,
        RuntimeContextBootstrap, RuntimeContextSource,
    };
    use radroots_storage::event::SourceGeneration;

    use crate::{ServiceDatabaseMetadata, ServiceSqliteApplicationId};

    use super::*;

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[derive(Debug, PartialEq, Eq)]
    struct FileSnapshot {
        bytes: Vec<u8>,
        length: u64,
        modified: SystemTime,
        mode: u32,
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn directory_snapshot(directory: &Path) -> BTreeMap<String, FileSnapshot> {
        fs::read_dir(directory)
            .expect("read state directory")
            .map(|entry| {
                let entry = entry.expect("state entry");
                let name = entry.file_name().into_string().expect("UTF-8 state entry");
                let metadata = entry.metadata().expect("state metadata");
                let snapshot = FileSnapshot {
                    bytes: fs::read(entry.path()).expect("state bytes"),
                    length: metadata.len(),
                    modified: metadata.modified().expect("modified time"),
                    mode: metadata.permissions().mode() & 0o777,
                };
                (name, snapshot)
            })
            .collect()
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn database_metadata(paths: &ServiceSqlitePaths) -> ServiceDatabaseMetadata {
        ServiceDatabaseMetadata::new(
            paths,
            SourceGeneration::new([7; 32]).expect("source generation"),
            NonZeroU32::new(1).expect("schema version"),
            1_700_000_000_000,
            ServiceSqliteApplicationId::new(0x5244_5351).expect("application ID"),
        )
        .expect("database metadata")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn base_catalog() -> MigrationCatalog {
        MigrationCatalog::new([]).expect("empty v1 catalog")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn migration_catalog() -> MigrationCatalog {
        const CREATE: &str = "CREATE TABLE migration_probe (value INTEGER NOT NULL);";
        const CALLBACK_DEFINITION: &[u8] = b"callback:migration_probe:v1";
        MigrationCatalog::new([
            crate::MigrationDescriptor::sql(
                2,
                "create_migration_probe",
                CREATE,
                crate::MigrationChecksum::for_sql(CREATE),
            )
            .expect("SQL migration"),
            crate::MigrationDescriptor::callback(
                3,
                "populate_migration_probe",
                CALLBACK_DEFINITION,
                crate::MigrationChecksum::for_callback(CALLBACK_DEFINITION),
            )
            .expect("callback migration"),
        ])
        .expect("migration catalog")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn migration_build() -> MigrationBuildIdentity {
        MigrationBuildIdentity::new(
            "0.1.0-alpha",
            "0123456789abcdef0123456789abcdef01234567",
            "89abcdef0123456789abcdef0123456789abcdef",
            "1.97.1",
            "x86_64-unknown-linux-gnu",
            "service-host",
            1,
            2,
            3,
            4,
            5,
        )
        .expect("migration build")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn migration_callback_binding() -> crate::migration::MigrationCallbackBinding {
        let catalog = migration_catalog();
        let descriptor = &catalog.descriptors()[1];
        crate::migration::MigrationCallbackBinding::new(
            descriptor.target_version(),
            descriptor.name(),
            descriptor.checksum(),
            migration_callback,
        )
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn migration_callback<'a>(
        executor: &'a mut crate::migration::MigrationTransactionExecutor<'_>,
    ) -> crate::migration::MigrationCallbackFuture<'a> {
        Box::pin(async move {
            executor
                .execute("INSERT INTO migration_probe (value) VALUES (41)")
                .await
        })
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn counted_migration_callback<'a>(
        executor: &'a mut crate::migration::MigrationTransactionExecutor<'_>,
    ) -> crate::migration::MigrationCallbackFuture<'a> {
        Box::pin(async move {
            CONCURRENT_CALLBACK_COUNT.fetch_add(1, AtomicOrdering::SeqCst);
            executor
                .execute("INSERT INTO migration_probe (value) VALUES (41)")
                .await
        })
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    static CONCURRENT_CALLBACK_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn yielding_migration_callback<'a>(
        executor: &'a mut crate::migration::MigrationTransactionExecutor<'_>,
    ) -> crate::migration::MigrationCallbackFuture<'a> {
        Box::pin(async move {
            AUTHORITY_CALLBACK_COUNT.fetch_add(1, AtomicOrdering::SeqCst);
            tokio::task::yield_now().await;
            executor
                .execute("INSERT INTO migration_probe (value) VALUES (41)")
                .await
        })
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    static AUTHORITY_CALLBACK_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn initialized_authority(
        root: &Path,
        instance: &str,
    ) -> (ServiceSqlitePaths, ServiceDatabaseIdentity, WriterAuthority) {
        let paths = ServiceSqlitePaths::from_runtime_context(&runtime_context(
            RadrootsPathProfile::RepoLocal,
            Some(root.to_path_buf()),
            "myc",
            instance,
        ))
        .expect("SQLite paths");
        fs::create_dir_all(paths.state_database().parent().expect("state directory"))
            .expect("create state directory");
        let metadata = database_metadata(&paths);
        let authority = crate::initialize_database(
            &paths,
            OpenMode::Initialize,
            &metadata,
            |database_path| async move {
                let options = SqliteConnectOptions::new()
                    .filename(database_path)
                    .create_if_missing(false);
                let connection = SqliteConnection::connect_with(&options)
                    .await
                    .expect("open reserved database");
                connection.close().await.expect("close reserved database");
                Ok::<_, Infallible>(())
            },
        )
        .await
        .expect("initialize database");
        let identity = metadata.identity();
        (paths, identity, authority)
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn initialized_pool(
        root: &Path,
        policy: ServiceSqliteConnectionOptions,
    ) -> (ServiceSqlitePaths, PrivateConnectionPool) {
        let (paths, identity, authority) = initialized_authority(root, "primary").await;
        let pool =
            open_initialized_connection_pool(&paths, &identity, &base_catalog(), policy, authority)
                .await
                .expect("open initialized pool");
        (paths, pool)
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn initialized_migration_pool(
        root: &Path,
        policy: ServiceSqliteConnectionOptions,
        catalog: &MigrationCatalog,
    ) -> (
        ServiceSqlitePaths,
        ServiceDatabaseIdentity,
        PrivateConnectionPool,
    ) {
        let (paths, base_identity, authority) = initialized_authority(root, "migrations").await;
        let identity = ServiceDatabaseIdentity::new(
            &paths,
            base_identity.source_generation(),
            NonZeroU32::new(catalog.current_version()).expect("catalog version"),
            base_identity.application_id(),
        );
        let pool = open_initialized_connection_pool(&paths, &identity, catalog, policy, authority)
            .await
            .expect("open migration pool");
        (paths, identity, pool)
    }

    fn runtime_context(
        profile: RadrootsPathProfile,
        repo_local_root: Option<PathBuf>,
        service: &str,
        instance: &str,
    ) -> RuntimeContext {
        let profile_source = if matches!(profile, RadrootsPathProfile::RepoLocal) {
            RuntimeContextSource::BootstrapCli
        } else {
            RuntimeContextSource::SafeDefault
        };
        RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                profile,
                repo_local_root,
                profile_source,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("valid bootstrap"),
            ServiceId::new(service).expect("valid service"),
            InstanceId::new(instance).expect("valid instance"),
        )
        .expect("valid runtime context")
    }

    #[test]
    fn paths_bind_exact_service_host_and_repo_local_artifacts() {
        let myc = ServiceSqlitePaths::from_runtime_context(&runtime_context(
            RadrootsPathProfile::ServiceHost,
            None,
            "myc",
            "primary",
        ))
        .expect("Myc paths");
        assert_eq!(myc.service().as_str(), "myc");
        assert_eq!(myc.instance().as_str(), "primary");
        assert_eq!(
            myc.state_database(),
            Path::new("/var/lib/radroots/services/myc/primary/state.sqlite")
        );
        assert_eq!(
            myc.state_lock(),
            Path::new("/var/lib/radroots/services/myc/primary/state.lock")
        );

        let rhi = ServiceSqlitePaths::from_runtime_context(&runtime_context(
            RadrootsPathProfile::RepoLocal,
            Some(PathBuf::from("/repo/.local/radroots")),
            "rhi",
            "north-01",
        ))
        .expect("RHI paths");
        assert_eq!(rhi.service().as_str(), "rhi");
        assert_eq!(rhi.instance().as_str(), "north-01");
        assert_eq!(
            rhi.state_database(),
            Path::new("/repo/.local/radroots/data/services/rhi/north-01/state.sqlite")
        );
        assert_eq!(
            rhi.state_lock(),
            Path::new("/repo/.local/radroots/data/services/rhi/north-01/state.lock")
        );

        let second = ServiceSqlitePaths::from_runtime_context(&runtime_context(
            RadrootsPathProfile::RepoLocal,
            Some(PathBuf::from("/repo/.local/radroots")),
            "rhi",
            "south-02",
        ))
        .expect("second RHI paths");
        assert_ne!(rhi, second);
        assert_ne!(rhi.state_database(), second.state_database());
        assert_ne!(rhi.state_lock(), second.state_lock());
    }

    #[test]
    fn path_shape_failures_are_typed_path_free_and_debug_is_redacted() {
        assert_eq!(
            validate_state_directory(Path::new("relative/state")),
            Err(ServiceSqlitePathError::RelativeStateDirectory)
        );
        assert_eq!(
            validate_state_directory(Path::new("/")),
            Err(ServiceSqlitePathError::MissingStateDirectoryParent)
        );

        let error = ServiceSqlitePathError::RelativeStateDirectory;
        assert_eq!(error.to_string(), "SQLite state directory must be absolute");
        assert_eq!(format!("{error:?}"), "RelativeStateDirectory");

        let paths = ServiceSqlitePaths::from_runtime_context(&runtime_context(
            RadrootsPathProfile::RepoLocal,
            Some(PathBuf::from("/sensitive/project-root")),
            "myc",
            "private-instance",
        ))
        .expect("redacted paths");
        let debug = format!("{paths:?}");
        assert!(debug.contains("service: ServiceId(\"myc\")"));
        assert!(debug.contains("instance: InstanceId(\"private-instance\")"));
        assert!(debug.contains("state_database: \"[redacted]\""));
        assert!(debug.contains("state_lock: \"[redacted]\""));
        assert!(!debug.contains("sensitive"));
        assert!(!debug.contains("project-root"));
        assert!(!debug.contains("state.sqlite"));
        assert!(!debug.contains("state.lock"));
    }

    #[test]
    fn open_mode_wire_inventory_and_semantics_are_exact() {
        let inventory = [
            (OpenMode::Initialize, "initialize", true, false, true),
            (
                OpenMode::ReadWriteExisting,
                "read_write_existing",
                false,
                true,
                true,
            ),
            (
                OpenMode::ReadOnlyInspection,
                "read_only_inspection",
                false,
                true,
                false,
            ),
        ];
        for (mode, wire, may_create, requires_existing, requires_writer) in inventory {
            assert_eq!(
                serde_json::to_string(&mode).unwrap(),
                format!(r#""{wire}""#)
            );
            assert_eq!(mode.may_create(), may_create);
            assert_eq!(mode.requires_existing(), requires_existing);
            assert_eq!(mode.requires_writer_authority(), requires_writer);
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn every_connection_uses_the_exact_reviewed_pragma_policy() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let policy = ServiceSqliteConnectionOptions::reviewed();
        let (_paths, pool) = initialized_pool(directory.path(), policy).await;

        let mut connections = Vec::with_capacity(8);
        for _ in 0..8 {
            connections.push(pool.acquire().await.expect("pooled connection"));
        }
        assert_eq!(pool.pool.size(), 8);
        for connection in &mut connections {
            assert!(
                connection_policy_matches(connection, OpenMode::Initialize, policy)
                    .await
                    .unwrap()
            );
        }
        drop(connections);
        assert!(pool.close().await.expect("writer authority").is_held());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn reused_connection_drift_is_rejected_before_checkout() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let policy = ServiceSqliteConnectionOptions::reviewed();
        let (_paths, pool) = initialized_pool(directory.path(), policy).await;

        let mut connection = pool.acquire().await.expect("pooled connection");
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await
            .expect("drift pragma");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
                .fetch_one(&mut *connection)
                .await
                .unwrap(),
            0
        );
        drop(connection);

        let mut replacement = pool.acquire().await.expect("replacement connection");
        assert!(
            connection_policy_matches(&mut replacement, OpenMode::Initialize, policy)
                .await
                .unwrap()
        );
        drop(replacement);
        let _authority = pool.close().await;
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn metadata_mismatch_fails_open_and_checkout_before_use() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let policy = ServiceSqliteConnectionOptions::new(Duration::from_millis(500), 1).unwrap();
        let (paths, pool) = initialized_pool(directory.path(), policy).await;

        let mut connection = pool.acquire().await.expect("pooled connection");
        sqlx::query("PRAGMA application_id = 1380209490")
            .execute(&mut *connection)
            .await
            .expect("drift application ID");
        drop(connection);
        let error = pool
            .acquire()
            .await
            .expect_err("metadata drift must prevent checkout");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Metadata);
        let authority = pool.close().await.expect("writer authority retained");
        drop(authority);

        let wrong = ServiceDatabaseIdentity::new(
            &paths,
            SourceGeneration::new([8; 32]).expect("wrong generation"),
            NonZeroU32::new(1).expect("schema version"),
            ServiceSqliteApplicationId::new(0x5244_5351).expect("application ID"),
        );
        let result = open_existing_connection_pool(
            &paths,
            &wrong,
            &base_catalog(),
            OpenMode::ReadWriteExisting,
            policy,
        )
        .await;
        let Err(error) = result else {
            panic!("wrong generation must fail open");
        };
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Metadata);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn migration_entry_preserves_post_open_ledger_drift_classification() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let policy = ServiceSqliteConnectionOptions::new(Duration::from_millis(500), 1).unwrap();
        let (_paths, pool) = initialized_pool(directory.path(), policy).await;
        let build = migration_build();
        let mut connection = pool.acquire().await.expect("pooled connection");
        sqlx::query(
            "INSERT INTO schema_migrations (
                version, name, checksum, applied_at_unix_s,
                service_version, service_commit, lib_revision, rust_version, target,
                feature_profile, config_contract_version, state_contract_version,
                admin_contract_version, status_contract_version, provider_contract_version
             ) VALUES (2, 'unexpected_row', zeroblob(32), 0, ?, ?, ?, ?, ?, ?, 1, 2, 3, 4, 5)",
        )
        .bind(build.service_version())
        .bind(build.service_commit())
        .bind(build.lib_revision())
        .bind(build.rust_version())
        .bind(build.target())
        .bind(build.feature_profile())
        .execute(&mut *connection)
        .await
        .expect("inject post-open ledger drift");
        drop(connection);

        let error = pool
            .apply_migrations(
                MigrationAppliedAtUnixSeconds::new(1_800_000_000).unwrap(),
                &build,
                &[],
            )
            .await
            .expect_err("migration entry must reject drift before execution");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Migration);
        let authority = pool.close().await.expect("writer authority retained");
        drop(authority);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn pending_history_is_not_exposed_and_read_only_requires_current_state() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let catalog = migration_catalog();
        let policy = ServiceSqliteConnectionOptions::new(Duration::from_millis(500), 2).unwrap();
        let (paths, identity, pending) =
            initialized_migration_pool(directory.path(), policy, &catalog).await;

        let error = pending
            .acquire()
            .await
            .expect_err("pending schema must not escape the private pool");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Migration);
        let authority = pending.close().await.expect("writer authority");
        drop(authority);

        let read_only = open_existing_connection_pool(
            &paths,
            &identity,
            &catalog,
            OpenMode::ReadOnlyInspection,
            policy,
        )
        .await;
        let Err(error) = read_only else {
            panic!("read-only pending state must fail closed");
        };
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Migration);

        let writable = open_existing_connection_pool(
            &paths,
            &identity,
            &catalog,
            OpenMode::ReadWriteExisting,
            policy,
        )
        .await
        .expect("reopen writable prefix");
        let outcome = writable
            .apply_migrations(
                MigrationAppliedAtUnixSeconds::new(1_800_000_000).unwrap(),
                &migration_build(),
                &[migration_callback_binding()],
            )
            .await
            .expect("apply pending migrations");
        assert_eq!(outcome.initial_version(), 1);
        assert_eq!(outcome.final_version(), 3);
        assert_eq!(outcome.applied_count(), 2);
        let connection = writable.acquire().await.expect("current checkout");
        drop(connection);
        let authority = writable.close().await.expect("writer authority");
        drop(authority);

        let current = open_existing_connection_pool(
            &paths,
            &identity,
            &catalog,
            OpenMode::ReadOnlyInspection,
            policy,
        )
        .await
        .expect("current read-only state");
        assert!(current.close().await.is_none());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn concurrent_migration_attempts_serialize_and_execute_callback_once() {
        CONCURRENT_CALLBACK_COUNT.store(0, AtomicOrdering::SeqCst);
        let directory = tempfile::tempdir().expect("temporary directory");
        let catalog = migration_catalog();
        let policy = ServiceSqliteConnectionOptions::new(Duration::from_secs(2), 2).unwrap();
        let (_paths, _identity, pool) =
            initialized_migration_pool(directory.path(), policy, &catalog).await;
        let applied_at = MigrationAppliedAtUnixSeconds::new(1_800_000_000).unwrap();
        let build = migration_build();
        let descriptor = &catalog.descriptors()[1];
        let callbacks = [crate::migration::MigrationCallbackBinding::new(
            descriptor.target_version(),
            descriptor.name(),
            descriptor.checksum(),
            counted_migration_callback,
        )];

        let (first, second) = tokio::join!(
            pool.apply_migrations(applied_at, &build, &callbacks),
            pool.apply_migrations(applied_at, &build, &callbacks),
        );
        let first = first.expect("first migration attempt");
        let second = second.expect("second migration attempt");
        assert_eq!(
            first.applied_count() + second.applied_count(),
            2,
            "exact catalog is committed only once"
        );
        assert_eq!(CONCURRENT_CALLBACK_COUNT.load(AtomicOrdering::SeqCst), 1);
        let mut connection = pool.acquire().await.expect("current connection");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM migration_probe")
                .fetch_one(&mut *connection)
                .await
                .unwrap(),
            1
        );
        drop(connection);
        let _authority = pool.close().await;
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn authority_replacement_wins_over_in_flight_migration_failure() {
        AUTHORITY_CALLBACK_COUNT.store(0, AtomicOrdering::SeqCst);
        let directory = tempfile::tempdir().expect("temporary directory");
        let catalog = migration_catalog();
        let policy = ServiceSqliteConnectionOptions::new(Duration::from_secs(2), 1).unwrap();
        let (paths, _identity, pool) =
            initialized_migration_pool(directory.path(), policy, &catalog).await;
        let descriptor = &catalog.descriptors()[1];
        let callbacks = [crate::migration::MigrationCallbackBinding::new(
            descriptor.target_version(),
            descriptor.name(),
            descriptor.checksum(),
            yielding_migration_callback,
        )];
        let state_directory = paths.state_database().parent().unwrap().to_path_buf();
        let displaced = directory.path().join("displaced-migration-state");
        let build = migration_build();

        let application = pool.apply_migrations(
            MigrationAppliedAtUnixSeconds::new(1_800_000_000).unwrap(),
            &build,
            &callbacks,
        );
        let replace = async {
            while AUTHORITY_CALLBACK_COUNT.load(AtomicOrdering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
            fs::rename(&state_directory, &displaced).expect("displace state directory");
            fs::create_dir_all(&state_directory).expect("replace state directory");
            fs::copy(displaced.join("state.sqlite"), paths.state_database())
                .expect("copy replacement database");
            fs::set_permissions(paths.state_database(), fs::Permissions::from_mode(0o600))
                .expect("secure replacement database");
        };
        let (result, ()) = tokio::join!(application, replace);
        assert_eq!(
            result.expect_err("authority replacement must fail").kind(),
            ServiceSqliteErrorKind::Authority
        );
        let authority = pool.close().await.expect("retained writer authority");
        assert!(authority.is_held());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn existing_modes_never_create_missing_state_and_enforce_authority() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let paths = ServiceSqlitePaths::from_runtime_context(&runtime_context(
            RadrootsPathProfile::RepoLocal,
            Some(directory.path().to_path_buf()),
            "rhi",
            "default",
        ))
        .expect("SQLite paths");
        fs::create_dir_all(paths.state_database().parent().expect("state directory"))
            .expect("create state directory");
        let metadata = database_metadata(&paths).identity();

        for mode in [OpenMode::ReadWriteExisting, OpenMode::ReadOnlyInspection] {
            let result = open_existing_connection_pool(
                &paths,
                &metadata,
                &base_catalog(),
                mode,
                ServiceSqliteConnectionOptions::reviewed(),
            )
            .await;
            let Err(error) = result else {
                panic!("missing database must fail");
            };
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Open);
            assert!(!paths.state_database().exists());
        }
        let result = open_existing_connection_pool(
            &paths,
            &metadata,
            &base_catalog(),
            OpenMode::Initialize,
            ServiceSqliteConnectionOptions::reviewed(),
        )
        .await;
        let Err(error) = result else {
            panic!("initialize needs reserved state");
        };
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Open);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn initialized_pool_rejects_mismatched_paths_and_rebound_directory() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let (_paths, metadata, authority) =
            initialized_authority(directory.path(), "primary").await;
        let other = ServiceSqlitePaths::from_runtime_context(&runtime_context(
            RadrootsPathProfile::RepoLocal,
            Some(directory.path().to_path_buf()),
            "myc",
            "secondary",
        ))
        .expect("other SQLite paths");
        fs::create_dir_all(
            other
                .state_database()
                .parent()
                .expect("other state directory"),
        )
        .expect("create other state directory");
        let result = open_initialized_connection_pool(
            &other,
            &metadata,
            &base_catalog(),
            ServiceSqliteConnectionOptions::reviewed(),
            authority,
        )
        .await;
        let Err(error) = result else {
            panic!("mismatched paths must fail");
        };
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);

        let (paths, metadata, authority) = initialized_authority(directory.path(), "rebound").await;
        let state_directory = paths.state_database().parent().expect("state directory");
        let displaced = directory.path().join("displaced-state");
        fs::rename(state_directory, &displaced).expect("displace state directory");
        fs::create_dir_all(state_directory).expect("replace state directory");
        fs::copy(displaced.join("state.sqlite"), paths.state_database())
            .expect("copy replacement database");
        let result = open_initialized_connection_pool(
            &paths,
            &metadata,
            &base_catalog(),
            ServiceSqliteConnectionOptions::reviewed(),
            authority,
        )
        .await;
        let Err(error) = result else {
            panic!("rebound state directory must fail");
        };
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn pool_checkout_rejects_state_directory_replacement_before_growth() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let policy = ServiceSqliteConnectionOptions::new(Duration::from_millis(500), 2).unwrap();
        let (paths, pool) = initialized_pool(directory.path(), policy).await;
        assert_eq!(pool.pool.size(), 1);

        let state_directory = paths.state_database().parent().expect("state directory");
        let displaced = directory.path().join("displaced-live-state");
        fs::rename(state_directory, &displaced).expect("displace live state directory");
        fs::create_dir_all(state_directory).expect("replace live state directory");
        fs::copy(displaced.join("state.sqlite"), paths.state_database())
            .expect("copy replacement database");

        let error = pool
            .acquire()
            .await
            .expect_err("rebound directory must prevent checkout and pool growth");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
        assert_eq!(pool.pool.size(), 1);
        let authority = pool.close().await.expect("writer authority retained");
        assert!(authority.is_held());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn writable_existing_rejects_database_symlink_and_hardlink() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let policy = ServiceSqliteConnectionOptions::reviewed();

        let (symlink_paths, symlink_metadata, symlink_authority) =
            initialized_authority(directory.path(), "symlink-database").await;
        drop(symlink_authority);
        let symlink_backing = symlink_paths
            .state_database()
            .parent()
            .expect("state directory")
            .join("backing.sqlite");
        fs::rename(symlink_paths.state_database(), &symlink_backing)
            .expect("displace symlink database");
        symlink(&symlink_backing, symlink_paths.state_database()).expect("database symlink");
        let symlink_result = open_existing_connection_pool(
            &symlink_paths,
            &symlink_metadata,
            &base_catalog(),
            OpenMode::ReadWriteExisting,
            policy,
        )
        .await;
        let Err(symlink_error) = symlink_result else {
            panic!("database symlink must fail");
        };
        assert_eq!(symlink_error.kind(), ServiceSqliteErrorKind::Authority);

        let (hardlink_paths, hardlink_metadata, hardlink_authority) =
            initialized_authority(directory.path(), "hardlink-database").await;
        drop(hardlink_authority);
        let hardlink_alias = hardlink_paths
            .state_database()
            .parent()
            .expect("state directory")
            .join("alias.sqlite");
        fs::hard_link(hardlink_paths.state_database(), hardlink_alias).expect("database hard link");
        let hardlink_result = open_existing_connection_pool(
            &hardlink_paths,
            &hardlink_metadata,
            &base_catalog(),
            OpenMode::ReadWriteExisting,
            policy,
        )
        .await;
        let Err(hardlink_error) = hardlink_result else {
            panic!("database hard link must fail");
        };
        assert_eq!(hardlink_error.kind(), ServiceSqliteErrorKind::Authority);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn lazy_pool_growth_rejects_same_directory_database_replacement() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let policy = ServiceSqliteConnectionOptions::new(Duration::from_millis(500), 2).unwrap();
        let (paths, pool) = initialized_pool(directory.path(), policy).await;
        let held = pool.acquire().await.expect("hold initial connection");

        let displaced = paths
            .state_database()
            .parent()
            .expect("state directory")
            .join("displaced.sqlite");
        fs::rename(paths.state_database(), &displaced).expect("displace live database");
        fs::copy(&displaced, paths.state_database()).expect("copy replacement database");
        fs::set_permissions(paths.state_database(), fs::Permissions::from_mode(0o600))
            .expect("secure replacement database");

        let error = pool
            .acquire()
            .await
            .expect_err("database replacement must prevent lazy pool growth");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
        drop(held);
        let authority = pool.close().await.expect("writer authority retained");
        assert!(authority.is_held());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn read_only_inspection_is_offline_query_only_and_side_effect_free() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let policy = ServiceSqliteConnectionOptions::reviewed();
        let (paths, writable) = initialized_pool(directory.path(), policy).await;
        let metadata = database_metadata(&paths).identity();
        let mut connection = writable.acquire().await.expect("writable connection");
        sqlx::query("CREATE TABLE inspection_fixture (value INTEGER NOT NULL)")
            .execute(&mut *connection)
            .await
            .expect("create fixture");
        sqlx::query("INSERT INTO inspection_fixture (value) VALUES (41)")
            .execute(&mut *connection)
            .await
            .expect("insert fixture");
        drop(connection);

        let contended = open_existing_connection_pool(
            &paths,
            &metadata,
            &base_catalog(),
            OpenMode::ReadOnlyInspection,
            policy,
        )
        .await;
        let Err(error) = contended else {
            panic!("inspection must reject an active writer");
        };
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);

        let authority = writable.close().await.expect("writer authority");
        drop(authority);
        let state_directory = paths.state_database().parent().expect("state directory");
        let stale_wal = state_directory.join(WAL_FILE_NAME);
        fs::write(&stale_wal, b"stale-wal-evidence").expect("write stale WAL evidence");
        let stale = open_existing_connection_pool(
            &paths,
            &metadata,
            &base_catalog(),
            OpenMode::ReadOnlyInspection,
            policy,
        )
        .await;
        let Err(error) = stale else {
            panic!("inspection must reject stale WAL state");
        };
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
        assert_eq!(fs::read(&stale_wal).unwrap(), b"stale-wal-evidence");
        fs::remove_file(stale_wal).expect("remove test WAL evidence");
        let before = directory_snapshot(state_directory);

        let read_only = open_existing_connection_pool(
            &paths,
            &metadata,
            &base_catalog(),
            OpenMode::ReadOnlyInspection,
            policy,
        )
        .await
        .expect("offline read-only inspection");
        let mut connection = read_only.acquire().await.expect("inspection connection");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT value FROM inspection_fixture")
                .fetch_one(&mut *connection)
                .await
                .expect("read fixture"),
            41
        );
        assert!(
            sqlx::query("INSERT INTO inspection_fixture (value) VALUES (42)")
                .execute(&mut *connection)
                .await
                .is_err()
        );
        assert!(
            connection_policy_matches(&mut connection, OpenMode::ReadOnlyInspection, policy)
                .await
                .unwrap()
        );
        drop(connection);
        assert!(read_only.close().await.is_none());

        let after = directory_snapshot(state_directory);
        assert_eq!(
            after.keys().collect::<Vec<_>>(),
            before.keys().collect::<Vec<_>>()
        );
        for (name, before_file) in &before {
            let after_file = after.get(name).expect("same state entry");
            assert!(
                after_file.bytes == before_file.bytes,
                "{name} bytes changed"
            );
            assert_eq!(
                after_file.length, before_file.length,
                "{name} length changed"
            );
            assert_eq!(
                after_file.modified, before_file.modified,
                "{name} mtime changed"
            );
            assert_eq!(after_file.mode, before_file.mode, "{name} mode changed");
        }
        assert!(!after.contains_key("state.sqlite-wal"));
        assert!(!after.contains_key("state.sqlite-shm"));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn pool_saturation_recovers_and_explicit_close_finishes() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let policy = ServiceSqliteConnectionOptions::new(Duration::from_millis(500), 1).unwrap();
        let (paths, pool) = initialized_pool(directory.path(), policy).await;
        let metadata = database_metadata(&paths).identity();
        let observer = pool.pool.clone();
        let held = pool.acquire().await.expect("only connection");
        let saturated = pool.pool.try_acquire();
        assert!(saturated.is_none());
        drop(held);
        let recovered = pool.acquire().await.expect("recovered connection");
        drop(recovered);

        let authority = pool.close().await.expect("writer authority retained");
        assert!(observer.is_closed());
        assert!(authority.is_held());
        drop(authority);

        let read_only = open_existing_connection_pool(
            &paths,
            &metadata,
            &base_catalog(),
            OpenMode::ReadOnlyInspection,
            ServiceSqliteConnectionOptions::reviewed(),
        )
        .await
        .expect("read-only inspection");
        assert!(read_only.authority.is_none());
        assert!(read_only.close().await.is_none());
    }
}
