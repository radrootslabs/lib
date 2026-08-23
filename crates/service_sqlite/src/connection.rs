//! Narrow service-store host and transaction execution boundary.

use core::fmt;
use std::{
    error::Error,
    future::Future,
    path::Path,
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
    ExistingServiceDatabaseIntent, MigrationApplicationOutcome, MigrationAppliedAtUnixSeconds,
    MigrationBuildIdentity, MigrationCallbackBinding, MigrationCatalog, OpenMode, SchemaCatalog,
    ServiceDatabaseIdentity, ServiceDatabaseMetadata, ServiceSqliteConnectionOptions,
    ServiceSqliteError, ServiceSqliteErrorKind, ServiceSqliteIntegrityReport, ServiceSqlitePaths,
    WriterAuthority,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use sqlx::{Connection, pool::PoolConnection};

/// One service-owned SQLite host whose raw pool remains inaccessible.
///
/// The host intentionally has no raw-pool accessor:
///
/// ```compile_fail
/// use radroots_service_sqlite::ServiceSqliteHost;
///
/// fn leak_pool(host: &ServiceSqliteHost) {
///     let _ = host.pool();
/// }
/// ```
pub struct ServiceSqliteHost {
    mode: OpenMode,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pool: crate::open::PrivateConnectionPool,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    closing: AtomicBool,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    close_state: tokio::sync::Mutex<ServiceSqliteHostCloseState>,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    backup_active: Arc<AtomicBool>,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    integrity_driver: tokio::sync::Mutex<IntegrityInspectionDriver>,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    failpoints: crate::failpoint::DurabilityFailpoints,
}

/// Existing database opened under retained authority with its verified metadata.
///
/// This result cannot be assembled independently from a host and metadata:
///
/// ```compile_fail
/// use radroots_service_sqlite::{OpenedExistingServiceDatabase, ServiceSqliteHost};
///
/// fn forge(host: ServiceSqliteHost) {
///     let _ = OpenedExistingServiceDatabase { host };
/// }
/// ```
pub struct OpenedExistingServiceDatabase {
    host: ServiceSqliteHost,
    metadata: ServiceDatabaseMetadata,
}

impl OpenedExistingServiceDatabase {
    fn new(host: ServiceSqliteHost, metadata: ServiceDatabaseMetadata) -> Self {
        Self { host, metadata }
    }

    /// Borrows the authority-retaining host.
    #[must_use]
    pub const fn host(&self) -> &ServiceSqliteHost {
        &self.host
    }

    /// Borrows the metadata discovered and verified by the retained open.
    #[must_use]
    pub const fn database_metadata(&self) -> &ServiceDatabaseMetadata {
        &self.metadata
    }

    /// Consumes the binding into the authority-retaining host and actual metadata.
    #[must_use]
    pub fn into_parts(self) -> (ServiceSqliteHost, ServiceDatabaseMetadata) {
        (self.host, self.metadata)
    }
}

impl fmt::Debug for OpenedExistingServiceDatabase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenedExistingServiceDatabase")
            .field("mode", &self.host.mode())
            .field("database_metadata", &"[redacted]")
            .finish()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
enum ServiceSqliteHostCloseState {
    Pending,
    Complete(Option<ServiceSqliteErrorKind>),
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
enum IntegrityInspectionDriver {
    Idle,
    Connected(QuarantinedConnection),
    Closing(BoxFuture<'static, Result<(), sqlx::Error>>),
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IntegrityInspectionDriverFailure {
    Invariant,
    ConnectionClose,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn integrity_driver_close_result(
    result: Result<(), sqlx::Error>,
    injected_failure: bool,
) -> Result<(), IntegrityInspectionDriverFailure> {
    if injected_failure {
        Err(IntegrityInspectionDriverFailure::ConnectionClose)
    } else {
        result.map_err(|_| IntegrityInspectionDriverFailure::ConnectionClose)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn final_connection_policy_matches(
    initial: &crate::migration::MigrationConnectionPolicy,
    final_policy: &crate::migration::MigrationConnectionPolicy,
) -> Result<(), ServiceSqliteError> {
    (final_policy == initial)
        .then_some(())
        .ok_or_else(|| ServiceSqliteError::new(ServiceSqliteErrorKind::Pragma))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn unconfirmed_rollback_error(
    rollback: Option<ServiceSqliteError>,
    rollback_was_confirmed: bool,
) -> Option<ServiceSqliteError> {
    rollback.filter(|_| !rollback_was_confirmed)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn precondition_rollback_failure(
    authority: Option<ServiceSqliteError>,
    rollback: Option<ServiceSqliteError>,
    rollback_was_confirmed: bool,
    hook_removal: Option<ServiceSqliteError>,
) -> Option<ServiceSqliteError> {
    authority
        .or_else(|| unconfirmed_rollback_error(rollback, rollback_was_confirmed))
        .or(hook_removal)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn authority_drift_rollback_failure(
    rollback: Option<ServiceSqliteError>,
    rollback_was_confirmed: bool,
    hook_removal: Option<ServiceSqliteError>,
) -> Option<ServiceSqliteError> {
    unconfirmed_rollback_error(rollback, rollback_was_confirmed).or(hook_removal)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn operation_rollback_failure(
    rollback: Option<ServiceSqliteError>,
    rollback_was_confirmed: bool,
    hook_removal: Option<ServiceSqliteError>,
    authority: Option<ServiceSqliteError>,
) -> Option<ServiceSqliteError> {
    unconfirmed_rollback_error(rollback, rollback_was_confirmed)
        .or(hook_removal)
        .or(authority)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl IntegrityInspectionDriver {
    async fn close_retained(&mut self) -> Result<(), IntegrityInspectionDriverFailure> {
        loop {
            match self {
                Self::Idle => return Ok(()),
                Self::Connected(_) => {
                    let Self::Connected(connection) = core::mem::replace(self, Self::Idle) else {
                        return Err(IntegrityInspectionDriverFailure::Invariant);
                    };
                    *self = Self::Closing(
                        connection
                            .into_close_future()
                            .ok_or(IntegrityInspectionDriverFailure::Invariant)?,
                    );
                }
                Self::Closing(close) => {
                    #[cfg(test)]
                    crate::integrity::integrity_test_seam::pause(
                        crate::integrity::integrity_test_seam::PHASE_CONNECTION_CLOSE_AWAITING,
                    )
                    .await;
                    let result = close.await;
                    *self = Self::Idle;
                    #[cfg(test)]
                    let injected_failure =
                        crate::integrity::integrity_test_seam::take_connection_close_failure();
                    #[cfg(not(test))]
                    let injected_failure = false;
                    return integrity_driver_close_result(result, injected_failure);
                }
            }
        }
    }

    fn connection_mut(&mut self) -> Result<&mut SqliteConnection, ServiceSqliteError> {
        match self {
            Self::Connected(connection) => Ok(connection),
            Self::Idle | Self::Closing(_) => {
                Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Integrity))
            }
        }
    }

    fn return_to_pool(&mut self) -> Result<(), ServiceSqliteError> {
        let Self::Connected(mut connection) = core::mem::replace(self, Self::Idle) else {
            return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Integrity));
        };
        connection.trust();
        drop(connection);
        Ok(())
    }

    #[cfg(test)]
    const fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }
}

impl ServiceSqliteHost {
    /// Opens existing writable state and finishes every pending governed migration.
    ///
    /// Before opening SQLite, this path holds exclusive writer authority and
    /// synchronously reconciles any exact interrupted-restore topology. The
    /// recovery sequence has no await point: cancellation cannot split one
    /// filesystem step from its authority check. If the surrounding open is
    /// cancelled later, a retry re-reads the already durable filesystem state.
    #[allow(clippy::too_many_arguments)]
    pub async fn open_read_write_existing(
        paths: &ServiceSqlitePaths,
        identity: &ServiceDatabaseIdentity,
        migrations: &MigrationCatalog,
        schema: &SchemaCatalog,
        options: ServiceSqliteConnectionOptions,
        applied_at: MigrationAppliedAtUnixSeconds,
        build: &MigrationBuildIdentity,
        callbacks: &[MigrationCallbackBinding],
    ) -> Result<(Self, MigrationApplicationOutcome), ServiceSqliteError> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let pool = crate::open::open_existing_connection_pool(
                paths,
                identity,
                migrations,
                schema,
                OpenMode::ReadWriteExisting,
                options,
            )
            .await?;
            match pool.apply_migrations(applied_at, build, callbacks).await {
                Ok(outcome) => Ok((Self::from_pool(OpenMode::ReadWriteExisting, pool), outcome)),
                Err(error) => {
                    drop(pool.close().await);
                    Err(error)
                }
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (
                paths, identity, migrations, schema, options, applied_at, build, callbacks,
            );
            Err(unsupported_host())
        }
    }

    /// Opens existing writable state and discovers its stored generation under authority.
    ///
    /// The intent binds service, instance, application ID, and the supported
    /// schema ceiling before filesystem or SQLite admission. The returned
    /// metadata is read from the same authority-retaining host after governed
    /// migrations finish, so callers never guess a source generation or reopen
    /// the database between discovery and use.
    #[allow(clippy::too_many_arguments)]
    pub async fn open_read_write_existing_with_intent(
        paths: &ServiceSqlitePaths,
        intent: &ExistingServiceDatabaseIntent,
        migrations: &MigrationCatalog,
        schema: &SchemaCatalog,
        options: ServiceSqliteConnectionOptions,
        applied_at: MigrationAppliedAtUnixSeconds,
        build: &MigrationBuildIdentity,
        callbacks: &[MigrationCallbackBinding],
    ) -> Result<(OpenedExistingServiceDatabase, MigrationApplicationOutcome), ServiceSqliteError>
    {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let pool = crate::open::open_existing_connection_pool_with_intent(
                paths,
                intent,
                migrations,
                schema,
                OpenMode::ReadWriteExisting,
                options,
            )
            .await?;
            let outcome = match pool.apply_migrations(applied_at, build, callbacks).await {
                Ok(outcome) => outcome,
                Err(error) => {
                    drop(pool.close().await);
                    return Err(error);
                }
            };
            let metadata = match pool.database_metadata().await {
                Ok(metadata) => metadata,
                Err(error) => {
                    drop(pool.close().await);
                    return Err(error);
                }
            };
            let host = Self::from_pool(OpenMode::ReadWriteExisting, pool);
            Ok((OpenedExistingServiceDatabase::new(host, metadata), outcome))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (
                paths, intent, migrations, schema, options, applied_at, build, callbacks,
            );
            Err(unsupported_host())
        }
    }

    /// Opens state created under a retained initialization writer authority.
    #[allow(clippy::too_many_arguments)]
    pub async fn open_initialized(
        paths: &ServiceSqlitePaths,
        identity: &ServiceDatabaseIdentity,
        migrations: &MigrationCatalog,
        schema: &SchemaCatalog,
        options: ServiceSqliteConnectionOptions,
        authority: WriterAuthority,
        applied_at: MigrationAppliedAtUnixSeconds,
        build: &MigrationBuildIdentity,
        callbacks: &[MigrationCallbackBinding],
    ) -> Result<(Self, MigrationApplicationOutcome), ServiceSqliteError> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let pool = crate::open::open_initialized_connection_pool(
                paths, identity, migrations, schema, options, authority,
            )
            .await?;
            match pool.apply_migrations(applied_at, build, callbacks).await {
                Ok(outcome) => Ok((Self::from_pool(OpenMode::Initialize, pool), outcome)),
                Err(error) => {
                    drop(pool.close().await);
                    Err(error)
                }
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (
                paths, identity, migrations, schema, options, authority, applied_at, build,
                callbacks,
            );
            Err(unsupported_host())
        }
    }

    /// Opens an immutable, current-schema inspection host without writer authority.
    pub async fn open_read_only_inspection(
        paths: &ServiceSqlitePaths,
        identity: &ServiceDatabaseIdentity,
        migrations: &MigrationCatalog,
        schema: &SchemaCatalog,
        options: ServiceSqliteConnectionOptions,
    ) -> Result<Self, ServiceSqliteError> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let pool = crate::open::open_existing_connection_pool(
                paths,
                identity,
                migrations,
                schema,
                OpenMode::ReadOnlyInspection,
                options,
            )
            .await?;
            Ok(Self::from_pool(OpenMode::ReadOnlyInspection, pool))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (paths, identity, migrations, schema, options);
            Err(unsupported_host())
        }
    }

    /// Opens existing state for immutable inspection and discovers its metadata.
    pub async fn open_read_only_inspection_with_intent(
        paths: &ServiceSqlitePaths,
        intent: &ExistingServiceDatabaseIntent,
        migrations: &MigrationCatalog,
        schema: &SchemaCatalog,
        options: ServiceSqliteConnectionOptions,
    ) -> Result<OpenedExistingServiceDatabase, ServiceSqliteError> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let pool = crate::open::open_existing_connection_pool_with_intent(
                paths,
                intent,
                migrations,
                schema,
                OpenMode::ReadOnlyInspection,
                options,
            )
            .await?;
            let metadata = match pool.database_metadata().await {
                Ok(metadata) => metadata,
                Err(error) => {
                    drop(pool.close().await);
                    return Err(error);
                }
            };
            let host = Self::from_pool(OpenMode::ReadOnlyInspection, pool);
            Ok(OpenedExistingServiceDatabase::new(host, metadata))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (paths, intent, migrations, schema, options);
            Err(unsupported_host())
        }
    }

    /// Returns the fixed mode selected when the host was opened.
    #[must_use]
    pub const fn mode(&self) -> OpenMode {
        self.mode
    }

    /// Closes all connections and explicitly releases retained instance authority.
    ///
    /// Close rejects new transactions as soon as it starts and waits for already
    /// admitted transactions to finish. Writable hosts then perform the fixed
    /// governed `TRUNCATE` WAL checkpoint before releasing writer authority;
    /// read-only inspection performs no checkpoint or filesystem mutation.
    ///
    /// Cancelling this future leaves the host permanently non-admitting and retains
    /// authority until a later call resumes close. A completed result is cached, so
    /// sequential or concurrent later calls return the same stable outer outcome.
    pub async fn close(&self) -> Result<(), ServiceSqliteError> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.closing.store(true, Ordering::Release);
            let mut state = self.close_state.lock().await;
            if let ServiceSqliteHostCloseState::Complete(kind) = *state {
                return kind.map_or(Ok(()), |kind| Err(ServiceSqliteError::new(kind)));
            }
            let (integrity_cleanup, integrity_validation) = {
                let mut driver = self.integrity_driver.lock().await;
                let cleanup = driver
                    .close_retained()
                    .await
                    .map_err(|_| ServiceSqliteError::new(ServiceSqliteErrorKind::Open));
                let validation = self.pool.validate();
                (cleanup, validation)
            };
            let close = self.pool.close_explicit(&self.failpoints).await;
            match close {
                Err(retryable) => Err(retryable),
                Ok(terminal) => {
                    let terminal = integrity_validation.and(terminal).and(integrity_cleanup);
                    *state = ServiceSqliteHostCloseState::Complete(
                        terminal.as_ref().err().map(ServiceSqliteError::kind),
                    );
                    terminal
                }
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Err(unsupported_host())
        }
    }

    /// Captures one point-in-time SQLite backup into a new staging directory.
    ///
    /// The caller supplies the exact new absolute staging-directory path and an
    /// injected creation time. Capture is available only on writable hosts and
    /// admits at most one active capture per host. The canonical manifest and a
    /// successful result are returned only after the visible staging directory's
    /// sole `state.sqlite` member has passed metadata, integrity, digest, and
    /// durability checks. The manifest remains in memory and is not written into
    /// the staging directory.
    ///
    /// Dropping this future requests cancellation. The admitted worker retains
    /// host authority until it has closed SQLite handles and either completed or
    /// cleaned the exact staging artifacts, so `close` drains that work before
    /// releasing writer authority.
    pub async fn capture_online_backup(
        &self,
        staging_directory: &Path,
        created_at_unix_ms: crate::BackupCreatedAtUnixMs,
    ) -> Result<crate::ServiceBackupManifest, ServiceSqliteError> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            crate::backup::capture_online_backup(
                &self.pool,
                &self.closing,
                &self.backup_active,
                staging_directory,
                created_at_unix_ms,
                &self.failpoints,
            )
            .await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (staging_directory, created_at_unix_ms);
            Err(unsupported_host())
        }
    }

    /// Runs one explicit bounded integrity inspection over a single read snapshot.
    ///
    /// The caller injects the wall-clock completion time and owns any monotonic
    /// deadline. The host admits at most one inspection at a time. Dropping this
    /// future before it returns publishes no report, persists no status, and
    /// leaves the checked-out connection in a host-owned explicit-close driver.
    /// Retry or host close finishes that close before another check or authority
    /// release; a retry must inject a new time.
    /// Completed SQLite and foreign-key failures are returned only as fixed safe
    /// diagnostic codes. An inability to execute or decode either check is an
    /// `Integrity` error.
    pub async fn inspect_integrity(
        &self,
        checked_at: crate::IntegrityCheckedAtUnixMs,
    ) -> Result<ServiceSqliteIntegrityReport, ServiceSqliteError> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.inspect_integrity_supported(checked_at).await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = checked_at;
            Err(unsupported_host())
        }
    }

    /// Executes one runner-owned transaction without exposing its connection or pool.
    ///
    /// Dropping this future before the runner enables its outer commit quarantines
    /// the connection and leaves no authoritative transaction effect. An operation
    /// error is returned as `OperationRolledBack` only after rollback is confirmed;
    /// an unconfirmed rollback is `RollbackFailed`. Once outer commit begins,
    /// cancelling the future yields no result and must be treated as an unknown
    /// commit outcome. Callers receiving `CommitOutcomeUnknown`, or cancelling after
    /// commit begins, must reread authoritative state before any idempotent retry.
    pub async fn transaction<T, E, F>(
        &self,
        operation: F,
    ) -> Result<T, ServiceSqliteTransactionError<E>>
    where
        T: Send + 'static,
        E: Send + 'static,
        F: for<'a> FnOnce(
                &'a mut ServiceSqliteTransaction<'_>,
            ) -> ServiceSqliteTransactionFuture<'a, T, E>
            + Send,
    {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.transaction_supported(operation).await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            drop(operation);
            Err(ServiceSqliteTransactionError::not_committed(
                unsupported_host(),
            ))
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn transaction_supported<T, E, F>(
        &self,
        operation: F,
    ) -> Result<T, ServiceSqliteTransactionError<E>>
    where
        T: Send + 'static,
        E: Send + 'static,
        F: for<'a> FnOnce(
                &'a mut ServiceSqliteTransaction<'_>,
            ) -> ServiceSqliteTransactionFuture<'a, T, E>
            + Send,
    {
        crate::require_condition(
            !self.closing.load(Ordering::Acquire),
            ServiceSqliteErrorKind::Open,
        )
        .map_err(ServiceSqliteTransactionError::not_committed)?;
        self.pool
            .validate()
            .map_err(ServiceSqliteTransactionError::not_committed)?;
        let connection = self
            .pool
            .acquire()
            .await
            .map_err(ServiceSqliteTransactionError::not_committed)?;
        let mut connection = QuarantinedConnection::new(connection);
        self.pool
            .validate()
            .map_err(ServiceSqliteTransactionError::not_committed)?;
        let initial_policy = crate::migration::read_connection_policy(&mut connection)
            .await
            .map_err(ServiceSqliteTransactionError::not_committed)?;
        self.pool
            .validate()
            .map_err(ServiceSqliteTransactionError::not_committed)?;
        let gate = crate::transaction_control::TransactionControlGate::install(&mut connection)
            .await
            .map_err(|source| {
                ServiceSqliteTransactionError::not_committed(sqlite_source(source))
            })?;
        self.pool
            .validate()
            .map_err(ServiceSqliteTransactionError::not_committed)?;
        let before_begin = self
            .failpoints
            .hit(crate::failpoint::DurabilityFailpoint::TransactionBeforeBegin)
            .map_err(|source| {
                ServiceSqliteError::with_source(ServiceSqliteErrorKind::Open, source)
            });
        self.pool
            .validate()
            .map_err(ServiceSqliteTransactionError::not_committed)?;
        before_begin.map_err(ServiceSqliteTransactionError::not_committed)?;
        let mut transaction = match match self.pool.mode() {
            OpenMode::Initialize | OpenMode::ReadWriteExisting => {
                connection.begin_with("BEGIN IMMEDIATE").await
            }
            OpenMode::ReadOnlyInspection => connection.begin().await,
        } {
            Ok(transaction) => transaction,
            Err(source) => {
                return Err(ServiceSqliteTransactionError::not_committed(sqlite_source(
                    source,
                )));
            }
        };
        let injected_after_begin = self
            .failpoints
            .hit(crate::failpoint::DurabilityFailpoint::TransactionAfterBegin)
            .map_err(|source| {
                ServiceSqliteError::with_source(ServiceSqliteErrorKind::Open, source)
            });
        let after_begin = self.pool.validate().and(injected_after_begin);
        if let Err(error) = after_begin {
            let permit = gate.permit_runner_rollback();
            let rollback = transaction.rollback().await.map_err(sqlite_source);
            drop(permit);
            let rollback_was_confirmed =
                gate.rejected_commit_rolled_back() && !connection.is_in_transaction();
            let remove = gate.remove(&mut connection).await.map_err(sqlite_source);
            let authority = self.pool.validate();
            if let Some(rollback_error) = precondition_rollback_failure(
                authority.err(),
                rollback.err(),
                rollback_was_confirmed,
                remove.err(),
            ) {
                return Err(ServiceSqliteTransactionError::rollback_failed(
                    None,
                    rollback_error,
                ));
            }
            return Err(ServiceSqliteTransactionError::not_committed(error));
        }

        let operation_result = {
            let statement_control_rejected = Arc::new(AtomicBool::new(false));
            let mut executor = ServiceSqliteTransaction {
                connection: &mut transaction,
                statement_control_rejected: Arc::clone(&statement_control_rejected),
            };
            (operation(&mut executor).await, statement_control_rejected)
        };
        let (operation_result, statement_control_rejected) = operation_result;
        if let Err(error) = self.pool.validate() {
            let operation_error = operation_result.err();
            let permit = gate.permit_runner_rollback();
            let rollback = transaction.rollback().await.map_err(sqlite_source);
            drop(permit);
            let rollback_was_confirmed =
                gate.rejected_commit_rolled_back() && !connection.is_in_transaction();
            let remove = gate.remove(&mut connection).await.map_err(sqlite_source);
            let rollback_error = authority_drift_rollback_failure(
                rollback.err(),
                rollback_was_confirmed,
                remove.err(),
            );
            return Err(match rollback_error {
                Some(rollback_error) => {
                    ServiceSqliteTransactionError::rollback_failed(operation_error, rollback_error)
                }
                None => ServiceSqliteTransactionError::not_committed_with_operation(
                    operation_error,
                    error,
                ),
            });
        }
        let value = match operation_result {
            Ok(value) => value,
            Err(operation_error) => {
                let permit = gate.permit_runner_rollback();
                let rollback = transaction.rollback().await.map_err(sqlite_source);
                drop(permit);
                let rollback_was_confirmed =
                    gate.rejected_commit_rolled_back() && !connection.is_in_transaction();
                let remove = gate.remove(&mut connection).await.map_err(sqlite_source);
                let authority = self.pool.validate();
                if let Some(error) = operation_rollback_failure(
                    rollback.err(),
                    rollback_was_confirmed,
                    remove.err(),
                    authority.err(),
                ) {
                    return Err(ServiceSqliteTransactionError::rollback_failed(
                        Some(operation_error),
                        error,
                    ));
                }
                return Err(ServiceSqliteTransactionError::operation_rolled_back(
                    operation_error,
                ));
            }
        };

        let precommit = self
            .verify_before_commit(
                &mut transaction,
                &gate,
                &initial_policy,
                &statement_control_rejected,
            )
            .await;
        let precommit = match precommit {
            Ok(()) => {
                let injected = self
                    .failpoints
                    .hit(crate::failpoint::DurabilityFailpoint::TransactionBeforeCommit)
                    .map_err(|source| {
                        ServiceSqliteError::with_source(ServiceSqliteErrorKind::Open, source)
                    });
                self.pool.validate().and(injected)
            }
            Err(error) => Err(error),
        };
        if let Err(error) = precommit {
            let permit = gate.permit_runner_rollback();
            let rollback = transaction.rollback().await.map_err(sqlite_source);
            drop(permit);
            let rollback_was_confirmed =
                gate.rejected_commit_rolled_back() && !connection.is_in_transaction();
            let remove = gate.remove(&mut connection).await.map_err(sqlite_source);
            let authority = self.pool.validate();
            if let Some(rollback_error) = precondition_rollback_failure(
                authority.err(),
                rollback.err(),
                rollback_was_confirmed,
                remove.err(),
            ) {
                return Err(ServiceSqliteTransactionError::rollback_failed(
                    None,
                    rollback_error,
                ));
            }
            return Err(ServiceSqliteTransactionError::not_committed(error));
        }

        let permit = gate.permit_outer_commit();
        let commit = transaction.commit().await.map_err(sqlite_source);
        drop(permit);
        let injected_after_commit = if commit.is_ok() {
            self.failpoints
                .hit(crate::failpoint::DurabilityFailpoint::TransactionAfterCommit)
                .map_err(|source| {
                    ServiceSqliteError::with_source(ServiceSqliteErrorKind::Open, source)
                })
        } else {
            Ok(())
        };
        let remove = gate.remove(&mut connection).await.map_err(sqlite_source);
        self.pool
            .validate()
            .map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        commit.map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        remove.map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        injected_after_commit.map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        let final_policy = crate::migration::read_connection_policy(&mut connection)
            .await
            .map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        self.pool
            .validate()
            .map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        final_connection_policy_matches(&initial_policy, &final_policy)
            .map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        crate::metadata::verify_database_metadata(&mut connection, self.pool.identity())
            .await
            .map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        self.pool
            .validate()
            .map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        crate::migration::verify_migration_history(
            &mut connection,
            self.pool.catalog(),
            self.pool.schema_catalog(),
            true,
        )
        .await
        .map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        self.pool
            .validate()
            .map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        connection.trust();
        Ok(value)
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn inspect_integrity_supported(
        &self,
        checked_at: crate::IntegrityCheckedAtUnixMs,
    ) -> Result<ServiceSqliteIntegrityReport, ServiceSqliteError> {
        crate::require_condition(
            !self.closing.load(Ordering::Acquire),
            ServiceSqliteErrorKind::Open,
        )?;
        let mut driver = self
            .integrity_driver
            .try_lock()
            .map_err(|_| ServiceSqliteError::new(ServiceSqliteErrorKind::Integrity))?;
        let cleanup = driver.close_retained().await;
        self.pool.validate()?;
        cleanup.map_err(|_| ServiceSqliteError::new(ServiceSqliteErrorKind::Integrity))?;
        crate::require_condition(
            !self.closing.load(Ordering::Acquire),
            ServiceSqliteErrorKind::Open,
        )?;
        self.pool.validate()?;
        let connection = self.pool.acquire().await;
        self.pool.validate()?;
        *driver = IntegrityInspectionDriver::Connected(QuarantinedConnection::new(connection?));
        crate::require_condition(
            !self.closing.load(Ordering::Acquire),
            ServiceSqliteErrorKind::Open,
        )?;
        let report = crate::integrity::inspect_database_integrity(
            driver.connection_mut()?,
            checked_at,
            || self.pool.validate(),
        )
        .await;
        let validation = self.pool.validate();
        match (validation, report) {
            (Err(error), _) => {
                let cleanup = driver.close_retained().await;
                let validation = self.pool.validate();
                if error.kind() == ServiceSqliteErrorKind::Authority {
                    return Err(error);
                }
                validation?;
                cleanup.map_err(|_| ServiceSqliteError::new(ServiceSqliteErrorKind::Integrity))?;
                Err(error)
            }
            (Ok(()), Err(error)) => {
                let cleanup = driver.close_retained().await;
                self.pool.validate()?;
                cleanup.map_err(|_| ServiceSqliteError::new(ServiceSqliteErrorKind::Integrity))?;
                Err(error)
            }
            (Ok(()), Ok(report)) => {
                driver.return_to_pool()?;
                Ok(report)
            }
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn from_pool(mode: OpenMode, pool: crate::open::PrivateConnectionPool) -> Self {
        Self {
            mode,
            pool,
            closing: AtomicBool::new(false),
            close_state: tokio::sync::Mutex::new(ServiceSqliteHostCloseState::Pending),
            backup_active: Arc::new(AtomicBool::new(false)),
            integrity_driver: tokio::sync::Mutex::new(IntegrityInspectionDriver::Idle),
            failpoints: crate::failpoint::DurabilityFailpoints::default(),
        }
    }

    #[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
    fn arm_durability_failpoint(&self, point: crate::failpoint::DurabilityFailpoint) {
        self.failpoints.arm(point);
    }

    fn lifecycle_state(&self) -> &'static str {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            if self.closing.load(Ordering::Acquire) {
                "closing_or_closed"
            } else {
                "open"
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            "closing_or_closed"
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn verify_before_commit(
        &self,
        connection: &mut SqliteConnection,
        gate: &crate::transaction_control::TransactionControlGate,
        initial_policy: &crate::migration::MigrationConnectionPolicy,
        statement_control_rejected: &AtomicBool,
    ) -> Result<(), ServiceSqliteError> {
        self.pool.validate()?;
        crate::require_condition(
            !gate.control_violation_observed()
                && !statement_control_rejected.load(Ordering::Acquire),
            ServiceSqliteErrorKind::Open,
        )?;
        crate::migration::assert_governed_transaction(connection).await?;
        self.pool.validate()?;
        crate::require_condition(
            &crate::migration::read_connection_policy(connection).await? == initial_policy,
            ServiceSqliteErrorKind::Pragma,
        )?;
        self.pool.validate()?;
        crate::metadata::verify_database_metadata(connection, self.pool.identity()).await?;
        self.pool.validate()?;
        crate::migration::verify_migration_history_snapshot(
            connection,
            self.pool.catalog(),
            self.pool.schema_catalog(),
            true,
        )
        .await?;
        self.pool.validate()?;
        crate::migration::assert_governed_transaction(connection).await?;
        crate::require_condition(
            !gate.control_violation_observed(),
            ServiceSqliteErrorKind::Open,
        )?;
        Ok(())
    }
}

impl fmt::Debug for ServiceSqliteHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceSqliteHost")
            .field("mode", &self.mode)
            .field("pool", &"[redacted]")
            .field("lifecycle", &self.lifecycle_state())
            .finish()
    }
}

/// A sealed transaction executor that never exposes its raw SQLite connection.
///
/// Service repositories may use ordinary typed SQLx queries through the
/// borrowed executor:
///
/// ```
/// use radroots_service_sqlite::ServiceSqliteTransaction;
///
/// async fn row_count(
///     transaction: &mut ServiceSqliteTransaction<'_>,
/// ) -> Result<i64, sqlx::Error> {
///     sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM service_items")
///         .fetch_one(transaction)
///         .await
/// }
/// ```
///
/// Transaction control remains runner-owned:
///
/// ```compile_fail
/// use radroots_service_sqlite::ServiceSqliteTransaction;
///
/// async fn bypass(transaction: ServiceSqliteTransaction<'_>) {
///     transaction.commit().await.unwrap();
/// }
/// ```
pub struct ServiceSqliteTransaction<'connection> {
    connection: &'connection mut SqliteConnection,
    statement_control_rejected: Arc<AtomicBool>,
}

struct RestrictedExecute<Q> {
    query: Q,
    statement_control_rejected: Arc<AtomicBool>,
}

impl<'query, Q> Execute<'query, Sqlite> for RestrictedExecute<Q>
where
    Q: Execute<'query, Sqlite>,
{
    fn sql(self) -> SqlStr {
        restricted_sql(self.query.sql(), &self.statement_control_rejected)
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

fn restricted_sql(sql: SqlStr, statement_control_rejected: &AtomicBool) -> SqlStr {
    if crate::statement_policy::contains_forbidden_statement_control(sql.as_str()) {
        statement_control_rejected.store(true, Ordering::Release);
        SqlStr::from_static("RADROOTS_FORBIDDEN_STATEMENT_CONTROL")
    } else {
        sql
    }
}

impl fmt::Debug for ServiceSqliteTransaction<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ServiceSqliteTransaction([redacted])")
    }
}

impl<'executor, 'connection> Executor<'executor>
    for &'executor mut ServiceSqliteTransaction<'connection>
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
        (&mut *self.connection).fetch_many(RestrictedExecute {
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
        (&mut *self.connection).fetch_optional(RestrictedExecute {
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
            restricted_sql(sql, &self.statement_control_rejected),
            parameters,
        )
    }
}

/// Boxed callback future tied to the borrowed transaction executor.
pub type ServiceSqliteTransactionFuture<'a, T, E> =
    Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>;

/// Stable transaction completion phases without expanding SQLite error kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceSqliteTransactionErrorKind {
    NotCommitted,
    OperationRolledBack,
    RollbackFailed,
    CommitOutcomeUnknown,
}

/// Transaction failure retaining trusted details behind redacted diagnostics.
pub struct ServiceSqliteTransactionError<E> {
    kind: ServiceSqliteTransactionErrorKind,
    operation_error: Option<E>,
    sqlite_error: Option<ServiceSqliteError>,
}

impl<E> ServiceSqliteTransactionError<E> {
    fn not_committed(error: ServiceSqliteError) -> Self {
        Self::not_committed_with_operation(None, error)
    }

    fn not_committed_with_operation(
        operation_error: Option<E>,
        sqlite_error: ServiceSqliteError,
    ) -> Self {
        Self {
            kind: ServiceSqliteTransactionErrorKind::NotCommitted,
            operation_error,
            sqlite_error: Some(sqlite_error),
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn operation_rolled_back(error: E) -> Self {
        Self {
            kind: ServiceSqliteTransactionErrorKind::OperationRolledBack,
            operation_error: Some(error),
            sqlite_error: None,
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn rollback_failed(operation_error: Option<E>, sqlite_error: ServiceSqliteError) -> Self {
        Self {
            kind: ServiceSqliteTransactionErrorKind::RollbackFailed,
            operation_error,
            sqlite_error: Some(sqlite_error),
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn commit_outcome_unknown(error: ServiceSqliteError) -> Self {
        Self {
            kind: ServiceSqliteTransactionErrorKind::CommitOutcomeUnknown,
            operation_error: None,
            sqlite_error: Some(error),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> ServiceSqliteTransactionErrorKind {
        self.kind
    }

    #[must_use]
    pub const fn operation_error(&self) -> Option<&E> {
        self.operation_error.as_ref()
    }

    #[must_use]
    pub const fn sqlite_error(&self) -> Option<&ServiceSqliteError> {
        self.sqlite_error.as_ref()
    }
}

impl<E> fmt::Debug for ServiceSqliteTransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceSqliteTransactionError")
            .field("kind", &self.kind)
            .field(
                "operation_error",
                &self.operation_error.as_ref().map(|_| "[redacted]"),
            )
            .field(
                "sqlite_error",
                &self.sqlite_error.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}

impl<E> fmt::Display for ServiceSqliteTransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            ServiceSqliteTransactionErrorKind::NotCommitted => {
                "SQLite transaction did not reach commit"
            }
            ServiceSqliteTransactionErrorKind::OperationRolledBack => {
                "SQLite transaction operation was rolled back"
            }
            ServiceSqliteTransactionErrorKind::RollbackFailed => {
                "SQLite transaction rollback could not be confirmed"
            }
            ServiceSqliteTransactionErrorKind::CommitOutcomeUnknown => {
                "SQLite transaction commit outcome is unknown"
            }
        })
    }
}

impl<E: Send + Sync + 'static> Error for ServiceSqliteTransactionError<E> {}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct QuarantinedConnection {
    connection: Option<PoolConnection<Sqlite>>,
    trusted: bool,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl QuarantinedConnection {
    fn new(connection: PoolConnection<Sqlite>) -> Self {
        Self {
            connection: Some(connection),
            trusted: false,
        }
    }

    fn trust(&mut self) {
        self.trusted = true;
    }

    fn into_close_future(mut self) -> Option<BoxFuture<'static, Result<(), sqlx::Error>>> {
        let connection = self.connection.take()?;
        Some(Box::pin(async move { connection.close().await }))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl core::ops::Deref for QuarantinedConnection {
    type Target = SqliteConnection;

    fn deref(&self) -> &Self::Target {
        self.connection.as_deref().expect("connection is retained")
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl core::ops::DerefMut for QuarantinedConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.connection
            .as_deref_mut()
            .expect("connection is retained")
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for QuarantinedConnection {
    fn drop(&mut self) {
        if !self.trusted
            && let Some(connection) = self.connection.as_mut()
        {
            connection.close_on_drop();
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn sqlite_source(source: sqlx::Error) -> ServiceSqliteError {
    ServiceSqliteError::with_source(ServiceSqliteErrorKind::Open, source)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn unsupported_host() -> ServiceSqliteError {
    ServiceSqliteError::new(ServiceSqliteErrorKind::Open)
}

#[cfg(test)]
mod tests {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use std::{
        collections::BTreeMap,
        convert::Infallible,
        fs,
        num::NonZeroU32,
        os::unix::fs::PermissionsExt,
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, SystemTime},
    };

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use radroots_runtime_paths::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
    };
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use radroots_storage::event::SourceGeneration;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use sqlx::{Connection, sqlite::SqliteConnectOptions};
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use tokio::sync::Notify;

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    static CAPTURE_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    use super::*;

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    const HOST_TABLE_SQL: &str = "CREATE TABLE host_probe (value INTEGER NOT NULL)";

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn runtime_context(root: &std::path::Path) -> RuntimeContext {
        RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(root.to_path_buf()),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("valid bootstrap"),
            ServiceId::new("myc").expect("service ID"),
            InstanceId::new("host-boundary").expect("instance ID"),
        )
        .expect("runtime context")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rollback_failure_selection_preserves_each_exact_precedence() {
        let error = || ServiceSqliteError::new(ServiceSqliteErrorKind::Open);
        for authority in [false, true] {
            for rollback in [false, true] {
                for confirmed in [false, true] {
                    for removal in [false, true] {
                        let expected_precondition = if authority {
                            1
                        } else if rollback && !confirmed {
                            2
                        } else if removal {
                            3
                        } else {
                            0
                        };
                        let precondition = precondition_rollback_failure(
                            authority.then(error),
                            rollback.then(error),
                            confirmed,
                            removal.then(error),
                        );
                        assert_eq!(
                            usize::from(precondition.is_some()),
                            usize::from(expected_precondition != 0)
                        );

                        let expected_drift = (rollback && !confirmed) || removal;
                        assert_eq!(
                            authority_drift_rollback_failure(
                                rollback.then(error),
                                confirmed,
                                removal.then(error),
                            )
                            .is_some(),
                            expected_drift
                        );

                        let expected_operation = (rollback && !confirmed) || removal || authority;
                        assert_eq!(
                            operation_rollback_failure(
                                rollback.then(error),
                                confirmed,
                                removal.then(error),
                                authority.then(error),
                            )
                            .is_some(),
                            expected_operation
                        );
                    }
                }
            }
        }
        assert!(unconfirmed_rollback_error(Some(error()), false).is_some());
        assert!(unconfirmed_rollback_error(Some(error()), true).is_none());
        assert!(unconfirmed_rollback_error(None, false).is_none());

        assert!(integrity_driver_close_result(Ok(()), false).is_ok());
        assert!(matches!(
            integrity_driver_close_result(Ok(()), true),
            Err(IntegrityInspectionDriverFailure::ConnectionClose)
        ));
        assert!(matches!(
            integrity_driver_close_result(Err(sqlx::Error::Protocol("close".to_owned())), false),
            Err(IntegrityInspectionDriverFailure::ConnectionClose)
        ));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn final_connection_policy_classifier_preserves_pragma_kind() {
        let mut first =
            SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(":memory:"))
                .await
                .expect("first connection");
        let mut second =
            SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(":memory:"))
                .await
                .expect("second connection");
        let initial = crate::migration::read_connection_policy(&mut first)
            .await
            .expect("initial policy");
        let same = crate::migration::read_connection_policy(&mut second)
            .await
            .expect("same policy");
        assert!(final_connection_policy_matches(&initial, &same).is_ok());
        sqlx::query("PRAGMA query_only = ON")
            .execute(&mut second)
            .await
            .expect("change policy");
        let changed = crate::migration::read_connection_policy(&mut second)
            .await
            .expect("changed policy");
        let error = final_connection_policy_matches(&initial, &changed).expect_err("policy drift");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Pragma);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn migration_catalog() -> MigrationCatalog {
        MigrationCatalog::new([]).expect("empty v1 migration catalog")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn schema_catalog(migrations: &MigrationCatalog) -> SchemaCatalog {
        let table = crate::SchemaObject::new(
            crate::SchemaObjectKind::Table,
            "host_probe",
            "host_probe",
            HOST_TABLE_SQL,
            crate::SchemaObject::computed_digest(
                crate::SchemaObjectKind::Table,
                "host_probe",
                "host_probe",
                HOST_TABLE_SQL,
            )
            .expect("table digest"),
        )
        .expect("table descriptor");
        let version_digest = crate::SchemaVersionCatalog::computed_digest(1, [table.clone()])
            .expect("version digest");
        let version =
            crate::SchemaVersionCatalog::new(1, [table], version_digest).expect("schema version");
        SchemaCatalog::new(migrations, [version]).expect("schema catalog")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn build_identity() -> MigrationBuildIdentity {
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
        .expect("build identity")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn initialized_host() -> (
        tempfile::TempDir,
        ServiceSqlitePaths,
        ServiceDatabaseIdentity,
        MigrationCatalog,
        SchemaCatalog,
        ServiceSqliteHost,
    ) {
        initialized_host_with_options(ServiceSqliteConnectionOptions::reviewed()).await
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn initialized_host_with_options(
        options: ServiceSqliteConnectionOptions,
    ) -> (
        tempfile::TempDir,
        ServiceSqlitePaths,
        ServiceDatabaseIdentity,
        MigrationCatalog,
        SchemaCatalog,
        ServiceSqliteHost,
    ) {
        let root = tempfile::tempdir().expect("temporary root");
        let paths = ServiceSqlitePaths::from_runtime_context(&runtime_context(root.path()))
            .expect("SQLite paths");
        fs::create_dir_all(paths.state_database().parent().expect("state directory"))
            .expect("create state directory");
        let metadata = crate::ServiceDatabaseMetadata::new(
            &paths,
            SourceGeneration::new([9; 32]).expect("source generation"),
            NonZeroU32::new(1).expect("schema version"),
            1_700_000_000_000,
            crate::ServiceSqliteApplicationId::new(0x5244_5351).expect("application ID"),
        )
        .expect("database metadata");
        let migrations = migration_catalog();
        let schema = schema_catalog(&migrations);
        let authority = crate::initialize_database(
            &paths,
            OpenMode::Initialize,
            &metadata,
            &schema,
            |database_path| async move {
                let options = SqliteConnectOptions::new()
                    .filename(database_path)
                    .create_if_missing(false);
                let mut connection = SqliteConnection::connect_with(&options)
                    .await
                    .expect("open reserved database");
                sqlx::query(HOST_TABLE_SQL)
                    .execute(&mut connection)
                    .await
                    .expect("create host table");
                connection.close().await.expect("close reserved database");
                Ok::<_, Infallible>(())
            },
        )
        .await
        .expect("initialize database");
        let identity = metadata.identity();
        let (host, outcome) = ServiceSqliteHost::open_initialized(
            &paths,
            &identity,
            &migrations,
            &schema,
            options,
            authority,
            MigrationAppliedAtUnixSeconds::new(1_700_000_000).expect("migration time"),
            &build_identity(),
            &[],
        )
        .await
        .expect("open initialized host");
        assert_eq!(outcome.initial_version(), 1);
        assert_eq!(outcome.final_version(), 1);
        assert_eq!(outcome.applied_count(), 0);
        (root, paths, identity, migrations, schema, host)
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn row_count(host: &ServiceSqliteHost) -> i64 {
        host.transaction(|transaction| {
            Box::pin(async move {
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM host_probe")
                    .fetch_one(&mut *transaction)
                    .await
            })
        })
        .await
        .expect("count rows")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn integrity_checked_at(value: u64) -> crate::IntegrityCheckedAtUnixMs {
        crate::IntegrityCheckedAtUnixMs::new(value).expect("integrity inspection time")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn existing_intent_returns_actual_metadata_without_releasing_authority() {
        let (_root, paths, identity, migrations, schema, initialized) = initialized_host().await;
        initialized.close().await.expect("close initialized host");
        let intent = ExistingServiceDatabaseIntent::new(
            &paths,
            identity.supported_state_schema_version(),
            identity.application_id(),
        );

        let wrong_application = ExistingServiceDatabaseIntent::new(
            &paths,
            identity.supported_state_schema_version(),
            crate::ServiceSqliteApplicationId::new(7).expect("other application"),
        );
        let error = ServiceSqliteHost::open_read_write_existing_with_intent(
            &paths,
            &wrong_application,
            &migrations,
            &schema,
            ServiceSqliteConnectionOptions::reviewed(),
            MigrationAppliedAtUnixSeconds::new(1_700_000_001).expect("migration time"),
            &build_identity(),
            &[],
        )
        .await
        .expect_err("application mismatch");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Metadata);

        let (opened, outcome) = ServiceSqliteHost::open_read_write_existing_with_intent(
            &paths,
            &intent,
            &migrations,
            &schema,
            ServiceSqliteConnectionOptions::reviewed(),
            MigrationAppliedAtUnixSeconds::new(1_700_000_001).expect("migration time"),
            &build_identity(),
            &[],
        )
        .await
        .expect("open writable from intent");
        assert_eq!(outcome.applied_count(), 0);
        assert_eq!(
            opened.database_metadata().source_generation(),
            identity.source_generation()
        );
        assert_eq!(opened.host().mode(), OpenMode::ReadWriteExisting);
        let debug = format!("{opened:?}");
        assert!(debug.contains("OpenedExistingServiceDatabase"));
        assert!(!debug.contains("09090909"));
        assert!(WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting).is_err());
        let (writable, actual) = opened.into_parts();
        assert_eq!(actual.source_generation(), identity.source_generation());
        assert_eq!(row_count(&writable).await, 0);
        writable.close().await.expect("close writable host");

        let inspected = ServiceSqliteHost::open_read_only_inspection_with_intent(
            &paths,
            &intent,
            &migrations,
            &schema,
            ServiceSqliteConnectionOptions::reviewed(),
        )
        .await
        .expect("open inspection from intent");
        assert_eq!(inspected.host().mode(), OpenMode::ReadOnlyInspection);
        assert_eq!(
            inspected.database_metadata().source_generation(),
            identity.source_generation()
        );
        assert!(WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting).is_err());
        let (inspection, actual) = inspected.into_parts();
        assert_eq!(actual.source_generation(), identity.source_generation());
        inspection.close().await.expect("close inspection host");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn integrity_inspection_is_explicit_safe_and_available_in_every_host_mode() {
        let _serial = crate::integrity::integrity_test_seam::LOCK.lock().await;
        crate::integrity::integrity_test_seam::release();
        let (_root, paths, identity, migrations, schema, initialized) = initialized_host().await;

        let initialized_report = initialized
            .inspect_integrity(integrity_checked_at(1_700_000_000_500))
            .await
            .expect("inspect initialized host");
        assert_eq!(initialized.mode(), OpenMode::Initialize);
        assert_eq!(
            initialized_report.sqlite(),
            crate::IntegrityCheckOutcome::Verified
        );
        assert_eq!(
            initialized_report.foreign_keys(),
            crate::IntegrityCheckOutcome::Verified
        );
        assert!(initialized_report.diagnostics().is_empty());
        assert_eq!(
            initialized_report.storage_integrity(),
            crate::StorageIntegrity::Verified
        );
        initialized.close().await.expect("close initialized host");

        let (writable, outcome) = ServiceSqliteHost::open_read_write_existing(
            &paths,
            &identity,
            &migrations,
            &schema,
            ServiceSqliteConnectionOptions::reviewed(),
            MigrationAppliedAtUnixSeconds::new(1_700_000_001).expect("migration time"),
            &build_identity(),
            &[],
        )
        .await
        .expect("open writable host");
        assert_eq!(outcome.applied_count(), 0);
        let writable_report = writable
            .inspect_integrity(integrity_checked_at(1_700_000_000_501))
            .await
            .expect("inspect writable host");
        assert_eq!(writable.mode(), OpenMode::ReadWriteExisting);
        assert_eq!(
            writable_report.storage_integrity(),
            crate::StorageIntegrity::Verified
        );
        writable.close().await.expect("close writable host");

        let read_only = ServiceSqliteHost::open_read_only_inspection(
            &paths,
            &identity,
            &migrations,
            &schema,
            ServiceSqliteConnectionOptions::reviewed(),
        )
        .await
        .expect("open read-only host");
        let read_only_report = read_only
            .inspect_integrity(integrity_checked_at(1_700_000_000_502))
            .await
            .expect("inspect read-only host");
        assert_eq!(read_only.mode(), OpenMode::ReadOnlyInspection);
        assert_eq!(
            read_only_report.storage_integrity(),
            crate::StorageIntegrity::Verified
        );
        read_only.close().await.expect("close read-only host");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn integrity_inspection_is_single_admission_cancel_safe_and_close_drained() {
        let _serial = crate::integrity::integrity_test_seam::LOCK.lock().await;
        crate::integrity::integrity_test_seam::release();
        let (_root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let host = Arc::new(host);

        crate::integrity::integrity_test_seam::block(
            crate::integrity::integrity_test_seam::PHASE_BEFORE_SQLITE,
        );
        let first = tokio::spawn({
            let host = Arc::clone(&host);
            async move {
                host.inspect_integrity(integrity_checked_at(1_700_000_000_510))
                    .await
            }
        });
        while crate::integrity::integrity_test_seam::reached()
            != crate::integrity::integrity_test_seam::PHASE_BEFORE_SQLITE
        {
            tokio::task::yield_now().await;
        }
        let concurrent = host
            .inspect_integrity(integrity_checked_at(1_700_000_000_511))
            .await
            .expect_err("second integrity inspection is rejected");
        assert_eq!(concurrent.kind(), ServiceSqliteErrorKind::Integrity);
        first.abort();
        assert!(
            first
                .await
                .expect_err("inspection is cancelled")
                .is_cancelled()
        );
        crate::integrity::integrity_test_seam::release();
        assert!(
            !host.integrity_driver.lock().await.is_idle(),
            "cancelled SQLx connection remains host-owned until explicit cleanup"
        );

        let recovered = host
            .inspect_integrity(integrity_checked_at(1_700_000_000_512))
            .await
            .expect("inspection recovers after cancellation");
        assert_eq!(
            recovered.storage_integrity(),
            crate::StorageIntegrity::Verified
        );

        crate::integrity::integrity_test_seam::block(
            crate::integrity::integrity_test_seam::PHASE_BEFORE_FOREIGN_KEYS,
        );
        let timed_out = tokio::time::timeout(
            Duration::from_millis(20),
            host.inspect_integrity(integrity_checked_at(1_700_000_000_513)),
        )
        .await;
        assert!(timed_out.is_err(), "caller deadline cancels inspection");
        crate::integrity::integrity_test_seam::release();
        assert!(!host.integrity_driver.lock().await.is_idle());

        crate::integrity::integrity_test_seam::block(
            crate::integrity::integrity_test_seam::PHASE_BEFORE_ROLLBACK,
        );
        let pre_rollback = tokio::spawn({
            let host = Arc::clone(&host);
            async move {
                host.inspect_integrity(integrity_checked_at(1_700_000_000_514))
                    .await
            }
        });
        while crate::integrity::integrity_test_seam::reached()
            != crate::integrity::integrity_test_seam::PHASE_BEFORE_ROLLBACK
        {
            tokio::task::yield_now().await;
        }
        pre_rollback.abort();
        assert!(
            pre_rollback
                .await
                .expect_err("pre-rollback inspection is cancelled")
                .is_cancelled()
        );
        crate::integrity::integrity_test_seam::release();
        assert!(!host.integrity_driver.lock().await.is_idle());
        host.inspect_integrity(integrity_checked_at(1_700_000_000_515))
            .await
            .expect("inspection recovers after pre-rollback cancellation");

        crate::integrity::integrity_test_seam::block(
            crate::integrity::integrity_test_seam::PHASE_BEFORE_ROLLBACK,
        );
        let admitted = tokio::spawn({
            let host = Arc::clone(&host);
            async move {
                host.inspect_integrity(integrity_checked_at(1_700_000_000_516))
                    .await
            }
        });
        while crate::integrity::integrity_test_seam::reached()
            != crate::integrity::integrity_test_seam::PHASE_BEFORE_ROLLBACK
        {
            tokio::task::yield_now().await;
        }
        let close = tokio::spawn({
            let host = Arc::clone(&host);
            async move { host.close().await }
        });
        while !host.closing.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        let rejected = host
            .inspect_integrity(integrity_checked_at(1_700_000_000_517))
            .await
            .expect_err("closing host rejects inspection");
        assert_eq!(rejected.kind(), ServiceSqliteErrorKind::Open);
        assert!(!close.is_finished());
        crate::integrity::integrity_test_seam::release();
        admitted
            .await
            .expect("admitted inspection task joins")
            .expect("admitted inspection completes");
        close
            .await
            .expect("close task joins")
            .expect("close drains admitted inspection");
        let closed = host
            .inspect_integrity(integrity_checked_at(1_700_000_000_518))
            .await
            .expect_err("closed host rejects inspection");
        assert_eq!(closed.kind(), ServiceSqliteErrorKind::Open);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn integrity_inspection_preserves_authority_precedence_after_await() {
        let _serial = crate::integrity::integrity_test_seam::LOCK.lock().await;
        crate::integrity::integrity_test_seam::release();
        let (_root, paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let host = Arc::new(host);
        crate::integrity::integrity_test_seam::block(
            crate::integrity::integrity_test_seam::PHASE_BEFORE_FOREIGN_KEYS,
        );
        let inspection = tokio::spawn({
            let host = Arc::clone(&host);
            async move {
                host.inspect_integrity(integrity_checked_at(1_700_000_000_520))
                    .await
            }
        });
        while crate::integrity::integrity_test_seam::reached()
            != crate::integrity::integrity_test_seam::PHASE_BEFORE_FOREIGN_KEYS
        {
            tokio::task::yield_now().await;
        }
        let retired_lock = paths
            .state_lock()
            .parent()
            .expect("state directory")
            .join("retired-integrity-state.lock");
        fs::rename(paths.state_lock(), &retired_lock).expect("retire writer lock");
        crate::integrity::integrity_test_seam::release();
        let error = inspection
            .await
            .expect("inspection task joins")
            .expect_err("authority drift rejects inspection");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
        fs::rename(&retired_lock, paths.state_lock()).expect("restore writer lock");
        host.close().await.expect("close host after restored lock");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn observed_authority_drift_precedes_transient_restore_and_close_failure() {
        let _serial = crate::integrity::integrity_test_seam::LOCK.lock().await;
        crate::integrity::integrity_test_seam::release();
        crate::integrity::integrity_test_seam::inject_connection_close_failure(false);
        let (_root, paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let host = Arc::new(host);

        crate::integrity::integrity_test_seam::block(
            crate::integrity::integrity_test_seam::PHASE_BEFORE_FOREIGN_KEYS,
        );
        let inspection = tokio::spawn({
            let host = Arc::clone(&host);
            async move {
                host.inspect_integrity(integrity_checked_at(1_700_000_000_525))
                    .await
            }
        });
        while crate::integrity::integrity_test_seam::reached()
            != crate::integrity::integrity_test_seam::PHASE_BEFORE_FOREIGN_KEYS
        {
            tokio::task::yield_now().await;
        }

        let retired_lock = paths
            .state_lock()
            .parent()
            .expect("state directory")
            .join("transient-integrity-state.lock");
        fs::rename(paths.state_lock(), &retired_lock).expect("retire writer lock");
        crate::integrity::integrity_test_seam::block(
            crate::integrity::integrity_test_seam::PHASE_CONNECTION_CLOSE_AWAITING,
        );
        while crate::integrity::integrity_test_seam::reached()
            != crate::integrity::integrity_test_seam::PHASE_CONNECTION_CLOSE_AWAITING
        {
            tokio::task::yield_now().await;
        }

        fs::rename(&retired_lock, paths.state_lock()).expect("restore writer lock");
        crate::integrity::integrity_test_seam::inject_connection_close_failure(true);
        crate::integrity::integrity_test_seam::release();
        let error = inspection
            .await
            .expect("inspection task joins")
            .expect_err("observed authority drift remains terminal");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
        assert!(host.integrity_driver.lock().await.is_idle());
        host.close().await.expect("close host after restored lock");
        crate::integrity::integrity_test_seam::inject_connection_close_failure(false);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn cancelled_real_sqlite_work_is_explicitly_closed_before_retry_or_host_close() {
        let _serial = crate::integrity::integrity_test_seam::LOCK.lock().await;
        crate::integrity::integrity_test_seam::release();
        let (_root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let host = Arc::new(host);

        crate::integrity::integrity_test_seam::enable_real_sqlite_probe(true);
        let cancelled = tokio::spawn({
            let host = Arc::clone(&host);
            async move {
                host.inspect_integrity(integrity_checked_at(1_700_000_000_530))
                    .await
            }
        });
        while crate::integrity::integrity_test_seam::reached()
            != crate::integrity::integrity_test_seam::PHASE_SQLITE_EXECUTION_AWAITING
        {
            tokio::task::yield_now().await;
        }
        assert!(!cancelled.is_finished(), "SQLite probe remains in flight");
        cancelled.abort();
        assert!(
            cancelled
                .await
                .expect_err("real SQLite inspection is cancelled")
                .is_cancelled()
        );
        crate::integrity::integrity_test_seam::enable_real_sqlite_probe(false);
        assert!(!host.integrity_driver.lock().await.is_idle());

        let cancelled_cleanup = tokio::spawn({
            let host = Arc::clone(&host);
            async move {
                host.inspect_integrity(integrity_checked_at(1_700_000_000_531))
                    .await
            }
        });
        while crate::integrity::integrity_test_seam::reached()
            != crate::integrity::integrity_test_seam::PHASE_CONNECTION_CLOSE_AWAITING
        {
            tokio::task::yield_now().await;
        }
        cancelled_cleanup.abort();
        assert!(
            cancelled_cleanup
                .await
                .expect_err("retained close retry is cancelled")
                .is_cancelled()
        );
        assert!(!host.integrity_driver.lock().await.is_idle());

        let recovered = host
            .inspect_integrity(integrity_checked_at(1_700_000_000_532))
            .await
            .expect("retry explicitly closes prior SQLite worker before inspecting");
        assert_eq!(
            recovered.storage_integrity(),
            crate::StorageIntegrity::Verified
        );
        assert!(host.integrity_driver.lock().await.is_idle());

        crate::integrity::integrity_test_seam::enable_real_sqlite_probe(true);
        let cancelled = tokio::spawn({
            let host = Arc::clone(&host);
            async move {
                host.inspect_integrity(integrity_checked_at(1_700_000_000_533))
                    .await
            }
        });
        while crate::integrity::integrity_test_seam::reached()
            != crate::integrity::integrity_test_seam::PHASE_SQLITE_EXECUTION_AWAITING
        {
            tokio::task::yield_now().await;
        }
        cancelled.abort();
        assert!(
            cancelled
                .await
                .expect_err("second real SQLite inspection is cancelled")
                .is_cancelled()
        );
        crate::integrity::integrity_test_seam::enable_real_sqlite_probe(false);
        assert!(!host.integrity_driver.lock().await.is_idle());
        host.close()
            .await
            .expect("host close explicitly terminates retained SQLite worker");
        assert!(host.integrity_driver.lock().await.is_idle());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn retained_integrity_close_releases_authority_and_caches_concurrent_lock_drift() {
        let _serial = crate::integrity::integrity_test_seam::LOCK.lock().await;
        crate::integrity::integrity_test_seam::release();
        let (_root, paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let host = Arc::new(host);

        crate::integrity::integrity_test_seam::enable_real_sqlite_probe(true);
        let inspection = tokio::spawn({
            let host = Arc::clone(&host);
            async move {
                host.inspect_integrity(integrity_checked_at(1_700_000_000_540))
                    .await
            }
        });
        while crate::integrity::integrity_test_seam::reached()
            != crate::integrity::integrity_test_seam::PHASE_SQLITE_EXECUTION_AWAITING
        {
            tokio::task::yield_now().await;
        }
        inspection.abort();
        assert!(
            inspection
                .await
                .expect_err("real integrity work is cancelled")
                .is_cancelled()
        );
        crate::integrity::integrity_test_seam::enable_real_sqlite_probe(false);

        let close = tokio::spawn({
            let host = Arc::clone(&host);
            async move { host.close().await }
        });
        while crate::integrity::integrity_test_seam::reached()
            != crate::integrity::integrity_test_seam::PHASE_CONNECTION_CLOSE_AWAITING
        {
            tokio::task::yield_now().await;
        }
        let retired_lock = paths
            .state_lock()
            .parent()
            .expect("state directory")
            .join("retired-integrity-close-state.lock");
        fs::rename(paths.state_lock(), &retired_lock).expect("retire held writer lock");
        fs::write(paths.state_lock(), b"").expect("create replacement writer lock");
        fs::set_permissions(paths.state_lock(), fs::Permissions::from_mode(0o600))
            .expect("replacement writer lock mode");

        let error = close
            .await
            .expect("close task joins")
            .expect_err("authority drift is terminal");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
        let repeated = host.close().await.expect_err("terminal result is cached");
        assert_eq!(repeated.kind(), ServiceSqliteErrorKind::Authority);
        assert!(host.integrity_driver.lock().await.is_idle());

        fs::remove_file(paths.state_lock()).expect("remove replacement writer lock");
        fs::rename(&retired_lock, paths.state_lock()).expect("restore original writer lock");
        let mut authority = WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting)
            .expect("authority reacquisition")
            .expect("writer authority");
        authority.release().expect("release reacquired authority");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn retained_integrity_close_failure_is_cached_as_open_and_releases_authority() {
        let _serial = crate::integrity::integrity_test_seam::LOCK.lock().await;
        crate::integrity::integrity_test_seam::release();
        crate::integrity::integrity_test_seam::inject_connection_close_failure(false);
        let (_root, paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let host = Arc::new(host);

        crate::integrity::integrity_test_seam::enable_real_sqlite_probe(true);
        let inspection = tokio::spawn({
            let host = Arc::clone(&host);
            async move {
                host.inspect_integrity(integrity_checked_at(1_700_000_000_550))
                    .await
            }
        });
        while crate::integrity::integrity_test_seam::reached()
            != crate::integrity::integrity_test_seam::PHASE_SQLITE_EXECUTION_AWAITING
        {
            tokio::task::yield_now().await;
        }
        inspection.abort();
        assert!(
            inspection
                .await
                .expect_err("real integrity work is cancelled")
                .is_cancelled()
        );
        crate::integrity::integrity_test_seam::enable_real_sqlite_probe(false);
        assert!(!host.integrity_driver.lock().await.is_idle());

        crate::integrity::integrity_test_seam::inject_connection_close_failure(true);
        let error = host
            .close()
            .await
            .expect_err("retained connection close failure is terminal");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Open);
        let repeated = host.close().await.expect_err("terminal result is cached");
        assert_eq!(repeated.kind(), ServiceSqliteErrorKind::Open);
        assert!(host.integrity_driver.lock().await.is_idle());

        let mut authority = WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting)
            .expect("authority reacquisition")
            .expect("writer authority");
        authority.release().expect("release reacquired authority");
        crate::integrity::integrity_test_seam::inject_connection_close_failure(false);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn online_backup_captures_exact_member_manifest_and_preserves_source() {
        use sha2::Digest;

        let _serial = CAPTURE_TEST_LOCK.lock().await;
        crate::backup::test_capture_reset();
        let (root, paths, identity, migrations, schema, host) = initialized_host().await;
        host.transaction(|transaction| {
            Box::pin(async move {
                sqlx::query("INSERT INTO host_probe (value) VALUES (41), (42)")
                    .execute(&mut *transaction)
                    .await
                    .map(|_| ())
            })
        })
        .await
        .expect("seed live WAL state");

        let output = root.path().join("backup-output");
        fs::create_dir(&output).expect("backup parent");
        fs::set_permissions(&output, fs::Permissions::from_mode(0o700))
            .expect("backup parent mode");
        let collision = output.join("collision");
        fs::create_dir(&collision).expect("preexisting collision");
        fs::write(collision.join("foreign"), b"preserve").expect("foreign collision member");
        let collision_error = host
            .capture_online_backup(
                &collision,
                crate::BackupCreatedAtUnixMs::new(1_700_000_000_100).expect("backup creation time"),
            )
            .await
            .expect_err("existing destination is rejected");
        assert_eq!(collision_error.kind(), ServiceSqliteErrorKind::Backup);
        assert_eq!(
            fs::read(collision.join("foreign")).expect("preserved collision"),
            b"preserve"
        );

        let source_database_before = fs::read(paths.state_database()).expect("source bytes");
        let source_inventory_before =
            fs::read_dir(paths.state_database().parent().expect("source directory"))
                .expect("source inventory")
                .map(|entry| entry.expect("source entry").file_name())
                .collect::<std::collections::BTreeSet<_>>();
        let stage = output.join("successful");
        let created_at =
            crate::BackupCreatedAtUnixMs::new(1_700_000_000_101).expect("backup creation time");
        let manifest = host
            .capture_online_backup(&stage, created_at)
            .await
            .expect("online backup");

        let verified = crate::verify_backup_bundle(
            manifest.canonical_bytes(),
            manifest.digest(),
            &stage,
            &identity,
            std::num::NonZeroU64::new(manifest.members()[0].byte_length())
                .expect("positive captured member length"),
        )
        .expect("independently verify captured bundle");
        assert_eq!(verified.manifest(), &manifest);
        assert_eq!(verified.database_metadata().service(), identity.service());
        assert_eq!(verified.database_metadata().instance(), identity.instance());

        assert_eq!(manifest.service(), paths.service());
        assert_eq!(manifest.instance(), paths.instance());
        assert_eq!(manifest.source_generation(), identity.source_generation());
        assert_eq!(
            manifest.state_schema_version(),
            identity.supported_state_schema_version()
        );
        assert_eq!(manifest.created_at_unix_ms(), created_at);
        assert_eq!(manifest.members().len(), 1);
        assert!(!manifest.protected_material_included());
        assert_eq!(manifest.integrity().sqlite(), "ok");
        assert_eq!(manifest.integrity().foreign_keys(), "ok");

        let state = stage.join("state.sqlite");
        let bytes = fs::read(&state).expect("captured state bytes");
        let digest: [u8; 32] = sha2::Sha256::digest(&bytes).into();
        assert_eq!(manifest.members()[0].byte_length(), bytes.len() as u64);
        assert_eq!(manifest.members()[0].sha256().as_bytes(), &digest);
        assert_eq!(
            fs::metadata(&stage)
                .expect("stage metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&state)
                .expect("state metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::read_dir(&stage)
                .expect("stage inventory")
                .map(|entry| entry.expect("stage entry").file_name())
                .collect::<Vec<_>>(),
            vec![std::ffi::OsString::from("state.sqlite")]
        );

        let mut backup = SqliteConnection::connect_with(
            &SqliteConnectOptions::new().filename(&state).read_only(true),
        )
        .await
        .expect("open captured database");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM host_probe")
                .fetch_one(&mut backup)
                .await
                .expect("captured row count"),
            2
        );
        backup.close().await.expect("close captured database");

        assert_eq!(
            fs::read(paths.state_database()).expect("source bytes after capture"),
            source_database_before
        );
        assert_eq!(
            fs::read_dir(paths.state_database().parent().expect("source directory"))
                .expect("source inventory after")
                .map(|entry| entry.expect("source entry").file_name())
                .collect::<std::collections::BTreeSet<_>>(),
            source_inventory_before
        );

        host.close().await.expect("close writable host");
        let inspection = ServiceSqliteHost::open_read_only_inspection(
            &paths,
            &identity,
            &migrations,
            &schema,
            ServiceSqliteConnectionOptions::reviewed(),
        )
        .await
        .expect("open inspection");
        let forbidden = output.join("read-only-forbidden");
        let error = inspection
            .capture_online_backup(&forbidden, created_at)
            .await
            .expect_err("read-only capture is unavailable");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Open);
        assert!(!forbidden.exists());
        inspection.close().await.expect("close inspection");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn cancelled_capture_cleans_up_close_drains_and_next_capture_recovers() {
        let _serial = CAPTURE_TEST_LOCK.lock().await;
        crate::backup::test_capture_reset();
        let (root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let host = Arc::new(host);
        let output = root.path().join("cancel-output");
        fs::create_dir(&output).expect("backup parent");
        fs::set_permissions(&output, fs::Permissions::from_mode(0o700))
            .expect("backup parent mode");
        let cancelled_stage = output.join("cancelled");
        crate::backup::test_capture_block_phase(crate::backup::TEST_CAPTURE_PHASE_STAGING_CREATED);
        let capture = tokio::spawn({
            let host = Arc::clone(&host);
            let stage = cancelled_stage.clone();
            async move {
                host.capture_online_backup(
                    &stage,
                    crate::BackupCreatedAtUnixMs::new(1_700_000_000_200)
                        .expect("backup creation time"),
                )
                .await
            }
        });
        while crate::backup::test_capture_phase()
            != crate::backup::TEST_CAPTURE_PHASE_STAGING_CREATED
        {
            tokio::task::yield_now().await;
        }

        let concurrent_stage = output.join("concurrent");
        let concurrent = host
            .capture_online_backup(
                &concurrent_stage,
                crate::BackupCreatedAtUnixMs::new(1_700_000_000_201).expect("backup creation time"),
            )
            .await
            .expect_err("second capture is rejected");
        assert_eq!(concurrent.kind(), ServiceSqliteErrorKind::Backup);
        assert!(!concurrent_stage.exists());

        capture.abort();
        assert!(
            capture
                .await
                .expect_err("capture task is cancelled")
                .is_cancelled()
        );
        while host.backup_active.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        crate::backup::test_capture_reset();
        assert!(!cancelled_stage.exists());

        let recovered_stage = output.join("recovered");
        host.capture_online_backup(
            &recovered_stage,
            crate::BackupCreatedAtUnixMs::new(1_700_000_000_202).expect("backup creation time"),
        )
        .await
        .expect("capture recovers after cancellation");
        assert!(recovered_stage.join("state.sqlite").exists());

        crate::backup::test_capture_reset();
        crate::backup::test_capture_block_phase(crate::backup::TEST_CAPTURE_PHASE_STAGING_CREATED);
        let close_drained_stage = output.join("close-drained");
        let capture = tokio::spawn({
            let host = Arc::clone(&host);
            let stage = close_drained_stage.clone();
            async move {
                host.capture_online_backup(
                    &stage,
                    crate::BackupCreatedAtUnixMs::new(1_700_000_000_203)
                        .expect("backup creation time"),
                )
                .await
            }
        });
        while crate::backup::test_capture_phase()
            != crate::backup::TEST_CAPTURE_PHASE_STAGING_CREATED
        {
            tokio::task::yield_now().await;
        }
        capture.abort();
        assert!(
            capture
                .await
                .expect_err("capture task is cancelled")
                .is_cancelled()
        );
        host.close()
            .await
            .expect("close drains every admitted capture");
        crate::backup::test_capture_reset();
        assert!(!close_drained_stage.exists());
        assert!(!host.backup_active.load(Ordering::Acquire));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn capture_cancellation_is_cleanup_safe_at_every_governed_phase() {
        let _serial = CAPTURE_TEST_LOCK.lock().await;

        for (index, phase) in [
            crate::backup::TEST_CAPTURE_PHASE_BEFORE_CREATE,
            crate::backup::TEST_CAPTURE_PHASE_BACKUP_STEPPED,
            crate::backup::TEST_CAPTURE_PHASE_POST_COPY,
            crate::backup::TEST_CAPTURE_PHASE_PRE_FINAL_SYNC,
        ]
        .into_iter()
        .enumerate()
        {
            crate::backup::test_capture_reset();
            let (root, paths, _identity, _migrations, _schema, host) = initialized_host().await;
            let host = Arc::new(host);
            if phase == crate::backup::TEST_CAPTURE_PHASE_BACKUP_STEPPED {
                host.transaction(|transaction| {
                    Box::pin(async move {
                        sqlx::raw_sql(
                            "WITH RECURSIVE sequence(value) AS (
                                 VALUES(1)
                                 UNION ALL
                                 SELECT value + 1 FROM sequence WHERE value < 100000
                             )
                             INSERT INTO host_probe(value) SELECT 0 FROM sequence",
                        )
                        .execute(&mut *transaction)
                        .await
                        .map(|_| ())
                    })
                })
                .await
                .expect("seed multi-batch cancellation fixture");
            }

            let output = root.path().join(format!("phase-cancel-output-{index}"));
            fs::create_dir(&output).expect("backup parent");
            fs::set_permissions(&output, fs::Permissions::from_mode(0o700))
                .expect("backup parent mode");
            let stage = output.join("cancelled");
            crate::backup::test_capture_block_phase(phase);
            let capture = tokio::spawn({
                let host = Arc::clone(&host);
                let stage = stage.clone();
                async move {
                    host.capture_online_backup(
                        &stage,
                        crate::BackupCreatedAtUnixMs::new(1_700_000_000_220 + index as u64)
                            .expect("backup creation time"),
                    )
                    .await
                }
            });
            while crate::backup::test_capture_phase() != phase {
                tokio::task::yield_now().await;
            }

            capture.abort();
            assert!(
                capture
                    .await
                    .expect_err("capture task is cancelled")
                    .is_cancelled()
            );
            host.close()
                .await
                .expect("close drains phase-cancelled capture cleanup");
            crate::backup::test_capture_reset();

            assert!(!stage.exists(), "phase {phase} must leave no staging tree");
            assert!(!host.backup_active.load(Ordering::Acquire));
            let mut authority = WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting)
                .expect("writer authority can be reacquired")
                .expect("writable mode returns authority");
            authority.release().expect("release reacquired authority");
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn online_backup_remains_consistent_with_a_concurrent_wal_writer() {
        let _serial = CAPTURE_TEST_LOCK.lock().await;
        crate::backup::test_capture_reset();
        let (root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let host = Arc::new(host);
        host.transaction(|transaction| {
            Box::pin(async move {
                sqlx::raw_sql(
                    "WITH RECURSIVE sequence(value) AS (
                         VALUES(1) UNION ALL SELECT value + 1 FROM sequence WHERE value < 100000
                     )
                     INSERT INTO host_probe(value) SELECT 0 FROM sequence",
                )
                .execute(&mut *transaction)
                .await
                .map(|_| ())
            })
        })
        .await
        .expect("seed a multi-batch database");

        let output = root.path().join("concurrent-output");
        fs::create_dir(&output).expect("backup parent");
        fs::set_permissions(&output, fs::Permissions::from_mode(0o700))
            .expect("backup parent mode");
        let stage = output.join("consistent");
        crate::backup::test_capture_block_phase(crate::backup::TEST_CAPTURE_PHASE_BACKUP_STEPPED);
        let capture = tokio::spawn({
            let host = Arc::clone(&host);
            let stage = stage.clone();
            async move {
                host.capture_online_backup(
                    &stage,
                    crate::BackupCreatedAtUnixMs::new(1_700_000_000_300)
                        .expect("backup creation time"),
                )
                .await
            }
        });
        while crate::backup::test_capture_phase()
            != crate::backup::TEST_CAPTURE_PHASE_BACKUP_STEPPED
        {
            tokio::task::yield_now().await;
        }
        host.transaction(|transaction| {
            Box::pin(async move {
                sqlx::query("UPDATE host_probe SET value = 1 WHERE rowid IN (1, 2)")
                    .execute(&mut *transaction)
                    .await
                    .map(|_| ())
            })
        })
        .await
        .expect("commit concurrent WAL transaction");
        crate::backup::test_capture_block_phase(0);
        capture
            .await
            .expect("capture joins")
            .expect("capture remains consistent");
        crate::backup::test_capture_reset();

        let mut backup = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(stage.join("state.sqlite"))
                .read_only(true),
        )
        .await
        .expect("open backup");
        let updated =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM host_probe WHERE value = 1")
                .fetch_one(&mut backup)
                .await
                .expect("count transaction projection");
        assert!(
            updated == 0 || updated == 2,
            "backup must not tear a transaction"
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("PRAGMA integrity_check(1)")
                .fetch_one(&mut backup)
                .await
                .expect("backup integrity"),
            "ok"
        );
        backup.close().await.expect("close backup");
        host.close().await.expect("close writer host");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn backup_sync_failures_cleanup_and_leave_host_recoverable() {
        let _serial = CAPTURE_TEST_LOCK.lock().await;
        crate::backup::test_capture_reset();
        let (root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let output = root.path().join("sync-failure-output");
        fs::create_dir(&output).expect("backup parent");
        fs::set_permissions(&output, fs::Permissions::from_mode(0o700))
            .expect("backup parent mode");

        for (index, failure) in [
            crate::backup::TestCaptureSyncFailure::State,
            crate::backup::TestCaptureSyncFailure::Staging,
            crate::backup::TestCaptureSyncFailure::FinalParent,
        ]
        .into_iter()
        .enumerate()
        {
            let stage = output.join(format!("failure-{index}"));
            let error = crate::backup::test_capture_online_backup_with_sync_failure(
                &host.pool,
                &host.closing,
                &host.backup_active,
                &stage,
                crate::BackupCreatedAtUnixMs::new(1_700_000_000_400 + index as u64)
                    .expect("backup creation time"),
                failure,
            )
            .await
            .expect_err("injected synchronization failure must reject capture");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Backup);
            assert!(!stage.exists());
            assert!(!host.backup_active.load(Ordering::Acquire));
            assert_eq!(row_count(&host).await, 0);
        }

        let recovered = output.join("recovered");
        host.capture_online_backup(
            &recovered,
            crate::BackupCreatedAtUnixMs::new(1_700_000_000_410).expect("backup creation time"),
        )
        .await
        .expect("host remains usable after sync failures");
        assert!(recovered.join("state.sqlite").exists());
        host.close().await.expect("close host");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn backup_await_boundaries_preserve_authority_precedence() {
        let _serial = CAPTURE_TEST_LOCK.lock().await;
        crate::backup::test_capture_reset();
        let (root, paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let host = Arc::new(host);
        let output = root.path().join("precedence-output");
        fs::create_dir(&output).expect("backup parent");
        fs::set_permissions(&output, fs::Permissions::from_mode(0o700))
            .expect("backup parent mode");

        crate::backup::test_capture_inject_metadata_failure(true);
        crate::backup::test_capture_block_phase(crate::backup::TEST_CAPTURE_PHASE_METADATA_AWAITED);
        let metadata_stage = output.join("metadata");
        let capture = tokio::spawn({
            let host = Arc::clone(&host);
            let stage = metadata_stage.clone();
            async move {
                host.capture_online_backup(
                    &stage,
                    crate::BackupCreatedAtUnixMs::new(1_700_000_000_420)
                        .expect("backup creation time"),
                )
                .await
            }
        });
        while crate::backup::test_capture_phase()
            != crate::backup::TEST_CAPTURE_PHASE_METADATA_AWAITED
        {
            tokio::task::yield_now().await;
        }
        let retired_lock = paths
            .state_lock()
            .parent()
            .expect("state directory")
            .join("retired-backup-precedence.lock");
        fs::rename(paths.state_lock(), &retired_lock).expect("retire writer lock");
        crate::backup::test_capture_block_phase(0);
        let metadata_error = capture
            .await
            .expect("metadata capture joins")
            .expect_err("authority overrides metadata failure");
        assert_eq!(metadata_error.kind(), ServiceSqliteErrorKind::Authority);
        fs::rename(&retired_lock, paths.state_lock()).expect("restore writer lock");
        crate::backup::test_capture_reset();
        assert!(!metadata_stage.exists());
        assert_eq!(row_count(&host).await, 0);

        crate::backup::test_capture_panic_worker(true);
        crate::backup::test_capture_block_phase(crate::backup::TEST_CAPTURE_PHASE_JOIN_AWAITED);
        let join_stage = output.join("join");
        let capture = tokio::spawn({
            let host = Arc::clone(&host);
            let stage = join_stage.clone();
            async move {
                host.capture_online_backup(
                    &stage,
                    crate::BackupCreatedAtUnixMs::new(1_700_000_000_421)
                        .expect("backup creation time"),
                )
                .await
            }
        });
        while crate::backup::test_capture_phase() != crate::backup::TEST_CAPTURE_PHASE_JOIN_AWAITED
        {
            tokio::task::yield_now().await;
        }
        fs::rename(paths.state_lock(), &retired_lock).expect("retire writer lock again");
        crate::backup::test_capture_block_phase(0);
        let join_error = capture
            .await
            .expect("join-failure capture joins")
            .expect_err("authority overrides worker join failure");
        assert_eq!(join_error.kind(), ServiceSqliteErrorKind::Authority);
        fs::rename(&retired_lock, paths.state_lock()).expect("restore writer lock again");
        crate::backup::test_capture_reset();
        assert!(!join_stage.exists());
        assert_eq!(row_count(&host).await, 0);
        host.close().await.expect("close host");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[derive(Debug, PartialEq, Eq)]
    struct StateFileSnapshot {
        bytes: Vec<u8>,
        length: u64,
        modified: SystemTime,
        mode: u32,
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn state_directory_snapshot(directory: &Path) -> BTreeMap<String, StateFileSnapshot> {
        fs::read_dir(directory)
            .expect("read state directory")
            .map(|entry| {
                let entry = entry.expect("state entry");
                let name = entry.file_name().into_string().expect("UTF-8 state entry");
                let metadata = entry.metadata().expect("state entry metadata");
                (
                    name,
                    StateFileSnapshot {
                        bytes: fs::read(entry.path()).expect("state entry bytes"),
                        length: metadata.len(),
                        modified: metadata.modified().expect("state entry mtime"),
                        mode: metadata.permissions().mode() & 0o777,
                    },
                )
            })
            .collect()
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn close_drains_admitted_work_rejects_new_work_and_is_idempotent() {
        let (_root, paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let host = Arc::new(host);
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let transaction = tokio::spawn({
            let host = Arc::clone(&host);
            let entered = Arc::clone(&entered);
            let release = Arc::clone(&release);
            async move {
                host.transaction::<i64, Infallible, _>(|transaction| {
                    Box::pin(async move {
                        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM host_probe")
                            .fetch_one(&mut *transaction)
                            .await
                            .expect("read in admitted transaction");
                        entered.notify_one();
                        release.notified().await;
                        Ok(count)
                    })
                })
                .await
            }
        });
        entered.notified().await;

        let first_close = tokio::spawn({
            let host = Arc::clone(&host);
            async move { host.close().await }
        });
        while !host.closing.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        let second_close = tokio::spawn({
            let host = Arc::clone(&host);
            async move { host.close().await }
        });
        let rejected = host
            .transaction(|_| Box::pin(async { Ok::<_, Infallible>(()) }))
            .await
            .expect_err("close admission must reject new work");
        assert_eq!(
            rejected.kind(),
            ServiceSqliteTransactionErrorKind::NotCommitted
        );
        assert_eq!(
            rejected.sqlite_error().map(ServiceSqliteError::kind),
            Some(ServiceSqliteErrorKind::Open)
        );
        let contended = WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting);
        assert!(matches!(
            contended,
            Err(ref error) if error.kind() == ServiceSqliteErrorKind::Authority
        ));

        release.notify_one();
        assert_eq!(
            transaction
                .await
                .expect("transaction task joins")
                .expect("admitted transaction"),
            0
        );
        first_close
            .await
            .expect("first close task joins")
            .expect("first close succeeds");
        second_close
            .await
            .expect("second close task joins")
            .expect("concurrent close is idempotent");
        host.close().await.expect("sequential close is idempotent");

        let mut next = WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting)
            .expect("authority can be reacquired")
            .expect("writer mode yields authority");
        next.release().expect("release reacquired authority");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn cancelled_close_retains_authority_and_retry_finishes() {
        let (_root, paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let host = Arc::new(host);
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let transaction = tokio::spawn({
            let host = Arc::clone(&host);
            let entered = Arc::clone(&entered);
            let release = Arc::clone(&release);
            async move {
                host.transaction::<(), Infallible, _>(|transaction| {
                    Box::pin(async move {
                        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM host_probe")
                            .fetch_one(&mut *transaction)
                            .await
                            .expect("admitted read");
                        entered.notify_one();
                        release.notified().await;
                        Ok(())
                    })
                })
                .await
            }
        });
        entered.notified().await;
        let close_task = tokio::spawn({
            let host = Arc::clone(&host);
            async move { host.close().await }
        });
        while !host.closing.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        close_task.abort();
        assert!(
            close_task
                .await
                .expect_err("close task is cancelled")
                .is_cancelled()
        );
        let retained = WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting);
        assert!(matches!(
            retained,
            Err(ref error) if error.kind() == ServiceSqliteErrorKind::Authority
        ));
        let rejected = host
            .transaction(|_| Box::pin(async { Ok::<_, Infallible>(()) }))
            .await
            .expect_err("cancelled close remains non-admitting");
        assert_eq!(
            rejected.kind(),
            ServiceSqliteTransactionErrorKind::NotCommitted
        );

        release.notify_one();
        transaction
            .await
            .expect("transaction task joins")
            .expect("admitted transaction finishes");
        host.close().await.expect("close retry succeeds");
        assert!(
            WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting)
                .expect("authority reacquisition after retry")
                .is_some()
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn writable_close_checkpoints_and_read_only_close_is_side_effect_free() {
        let (_root, paths, identity, migrations, schema, host) = initialized_host().await;
        host.transaction(|transaction| {
            Box::pin(async move {
                sqlx::query("INSERT INTO host_probe (value) VALUES (41)")
                    .execute(&mut *transaction)
                    .await
                    .map(|_| ())
            })
        })
        .await
        .expect("write WAL frame");
        host.close().await.expect("writable close checkpoints");
        let state_directory = paths.state_database().parent().expect("state directory");
        assert!(!state_directory.join("state.sqlite-wal").exists());
        assert!(!state_directory.join("state.sqlite-shm").exists());
        let before = state_directory_snapshot(state_directory);

        let inspection = ServiceSqliteHost::open_read_only_inspection(
            &paths,
            &identity,
            &migrations,
            &schema,
            ServiceSqliteConnectionOptions::reviewed(),
        )
        .await
        .expect("open read-only inspection");
        assert_eq!(row_count(&inspection).await, 1);
        inspection
            .close()
            .await
            .expect("close read-only inspection");
        assert_eq!(state_directory_snapshot(state_directory), before);

        let (reopened, outcome) = ServiceSqliteHost::open_read_write_existing(
            &paths,
            &identity,
            &migrations,
            &schema,
            ServiceSqliteConnectionOptions::reviewed(),
            MigrationAppliedAtUnixSeconds::new(1_700_000_001).expect("migration time"),
            &build_identity(),
            &[],
        )
        .await
        .expect("reopen writer after read-only close");
        assert_eq!(outcome.applied_count(), 0);
        assert_eq!(row_count(&reopened).await, 1);
        reopened.close().await.expect("close reopened writer");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn read_only_close_revalidates_after_drain_before_releasing_stale_authority() {
        let (_root, paths, identity, migrations, schema, writer) = initialized_host().await;
        writer.close().await.expect("close writer host");
        let inspection = Arc::new(
            ServiceSqliteHost::open_read_only_inspection(
                &paths,
                &identity,
                &migrations,
                &schema,
                ServiceSqliteConnectionOptions::reviewed(),
            )
            .await
            .expect("open read-only inspection"),
        );
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let transaction = tokio::spawn({
            let inspection = Arc::clone(&inspection);
            let entered = Arc::clone(&entered);
            let release = Arc::clone(&release);
            async move {
                inspection
                    .transaction::<(), Infallible, _>(|transaction| {
                        Box::pin(async move {
                            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM host_probe")
                                .fetch_one(&mut *transaction)
                                .await
                                .expect("read through retained inspection");
                            entered.notify_one();
                            release.notified().await;
                            Ok(())
                        })
                    })
                    .await
            }
        });
        entered.notified().await;
        let close_task = tokio::spawn({
            let inspection = Arc::clone(&inspection);
            async move { inspection.close().await }
        });
        while !inspection.closing.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }

        let retired_lock = paths
            .state_lock()
            .parent()
            .expect("state directory")
            .join("retired-inspection-close.lock");
        fs::rename(paths.state_lock(), retired_lock).expect("replace inspection lock");
        let mut replacement = WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting)
            .expect("replacement acquisition")
            .expect("replacement authority");
        release.notify_one();
        let transaction_error = transaction
            .await
            .expect("inspection transaction task joins")
            .expect_err("binding drift revokes admitted inspection");
        assert_eq!(
            transaction_error.kind(),
            ServiceSqliteTransactionErrorKind::NotCommitted
        );
        let close_error = close_task
            .await
            .expect("inspection close task joins")
            .expect_err("close must report stale inspection authority");
        assert_eq!(close_error.kind(), ServiceSqliteErrorKind::Authority);
        assert_eq!(
            inspection
                .close()
                .await
                .expect_err("terminal authority result is cached")
                .kind(),
            ServiceSqliteErrorKind::Authority
        );
        replacement
            .release()
            .expect("release replacement authority");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn cancelled_checkpoint_resumes_to_terminal_error_and_releases_authority() {
        let options = ServiceSqliteConnectionOptions::new(Duration::from_secs(1), 1)
            .expect("short reviewed limits");
        let (_root, paths, _identity, _migrations, _schema, host) =
            initialized_host_with_options(options).await;
        let host = Arc::new(host);
        host.transaction(|transaction| {
            Box::pin(async move {
                sqlx::query("INSERT INTO host_probe (value) VALUES (1)")
                    .execute(&mut *transaction)
                    .await
                    .map(|_| ())
            })
        })
        .await
        .expect("seed reader snapshot");

        let mut reader = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(paths.state_database())
                .read_only(true)
                .create_if_missing(false),
        )
        .await
        .expect("open external reader");
        let mut reader_transaction = reader.begin().await.expect("begin external read");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM host_probe")
                .fetch_one(&mut *reader_transaction)
                .await
                .expect("establish reader snapshot"),
            1
        );
        host.transaction(|transaction| {
            Box::pin(async move {
                sqlx::query("INSERT INTO host_probe (value) VALUES (2)")
                    .execute(&mut *transaction)
                    .await
                    .map(|_| ())
            })
        })
        .await
        .expect("append frame after reader snapshot");

        let close_task = tokio::spawn({
            let host = Arc::clone(&host);
            async move { host.close().await }
        });
        while host.pool.close_phase() != crate::open::TEST_CLOSE_PHASE_CHECKPOINT {
            tokio::task::yield_now().await;
        }
        close_task.abort();
        assert!(
            close_task
                .await
                .expect_err("checkpoint close task is cancelled")
                .is_cancelled()
        );
        let retained = WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting);
        assert!(matches!(
            retained,
            Err(ref error) if error.kind() == ServiceSqliteErrorKind::Authority
        ));

        let first = host
            .close()
            .await
            .expect_err("active reader prevents TRUNCATE");
        assert_eq!(first.kind(), ServiceSqliteErrorKind::Pragma);
        assert!(!first.to_string().contains("state.sqlite"));
        let repeated = host.close().await.expect_err("terminal result is cached");
        assert_eq!(repeated.kind(), ServiceSqliteErrorKind::Pragma);
        let mut replacement = WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting)
            .expect("close releases authority despite checkpoint failure")
            .expect("writer mode yields authority");
        replacement
            .release()
            .expect("release replacement authority");

        reader_transaction
            .rollback()
            .await
            .expect("release external snapshot");
        reader.close().await.expect("close external reader");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn close_authority_drift_is_cached_and_releases_the_stale_lock() {
        let (_root, paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let retired_lock = paths
            .state_lock()
            .parent()
            .expect("state directory")
            .join("retired-close-state.lock");
        fs::rename(paths.state_lock(), retired_lock).expect("replace canonical lock");
        let mut replacement = WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting)
            .expect("replacement acquisition")
            .expect("replacement authority");

        let first = host
            .close()
            .await
            .expect_err("close detects authority drift");
        assert_eq!(first.kind(), ServiceSqliteErrorKind::Authority);
        let repeated = host.close().await.expect_err("authority result is cached");
        assert_eq!(repeated.kind(), ServiceSqliteErrorKind::Authority);
        assert!(!format!("{host:?}").contains(paths.state_database().to_string_lossy().as_ref()));
        replacement
            .release()
            .expect("release replacement authority");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn typed_execution_commits_and_operation_failure_rolls_back() {
        let (_root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
        assert_eq!(host.mode(), OpenMode::Initialize);

        let inserted = host
            .transaction(|transaction| {
                Box::pin(async move {
                    sqlx::query("INSERT INTO host_probe (value) VALUES (?)")
                        .bind(41_i64)
                        .execute(&mut *transaction)
                        .await?;
                    sqlx::query_scalar::<_, i64>("SELECT value FROM host_probe")
                        .fetch_one(&mut *transaction)
                        .await
                })
            })
            .await
            .expect("commit typed operation");
        assert_eq!(inserted, 41);

        let error = host
            .transaction(|transaction| {
                Box::pin(async move {
                    sqlx::query("INSERT INTO host_probe (value) VALUES (99)")
                        .execute(&mut *transaction)
                        .await
                        .map_err(|_| "query-failure-secret")?;
                    Err::<(), _>("operation-secret")
                })
            })
            .await
            .expect_err("operation rejection must roll back");
        assert_eq!(
            error.kind(),
            ServiceSqliteTransactionErrorKind::OperationRolledBack
        );
        assert_eq!(error.operation_error(), Some(&"operation-secret"));
        assert!(error.sqlite_error().is_none());
        assert!(!format!("{error:?}").contains("operation-secret"));
        assert!(!error.to_string().contains("operation-secret"));
        assert_eq!(row_count(&host).await, 1);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn transaction_durability_edges_preserve_exact_commit_semantics() {
        use crate::failpoint::DurabilityFailpoint;

        for (point, expected_reached) in [
            (
                DurabilityFailpoint::TransactionBeforeBegin,
                &[DurabilityFailpoint::TransactionBeforeBegin][..],
            ),
            (
                DurabilityFailpoint::TransactionAfterBegin,
                &[
                    DurabilityFailpoint::TransactionBeforeBegin,
                    DurabilityFailpoint::TransactionAfterBegin,
                ][..],
            ),
            (
                DurabilityFailpoint::TransactionBeforeCommit,
                &[
                    DurabilityFailpoint::TransactionBeforeBegin,
                    DurabilityFailpoint::TransactionAfterBegin,
                    DurabilityFailpoint::TransactionBeforeCommit,
                ][..],
            ),
            (
                DurabilityFailpoint::TransactionAfterCommit,
                &[
                    DurabilityFailpoint::TransactionBeforeBegin,
                    DurabilityFailpoint::TransactionAfterBegin,
                    DurabilityFailpoint::TransactionBeforeCommit,
                    DurabilityFailpoint::TransactionAfterCommit,
                ][..],
            ),
        ] {
            let (_root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
            host.arm_durability_failpoint(point);
            let error = host
                .transaction(|transaction| {
                    Box::pin(async move {
                        sqlx::query("INSERT INTO host_probe (value) VALUES (72)")
                            .execute(&mut *transaction)
                            .await
                            .map(|_| ())
                    })
                })
                .await
                .expect_err("injected transaction edge");
            let reached = host.failpoints.reached();
            if point == DurabilityFailpoint::TransactionAfterCommit {
                assert_eq!(
                    error.kind(),
                    ServiceSqliteTransactionErrorKind::CommitOutcomeUnknown
                );
                assert_eq!(row_count(&host).await, 1);
            } else {
                assert_eq!(
                    error.kind(),
                    ServiceSqliteTransactionErrorKind::NotCommitted
                );
                assert_eq!(row_count(&host).await, 0);
            }
            assert!(host.failpoints.fired());
            assert_eq!(reached, expected_reached);
            host.close()
                .await
                .expect("close host after transaction edge");
        }

        let (_root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
        host.arm_durability_failpoint(DurabilityFailpoint::TransactionBeforeCommit);
        let error = host
            .transaction(|transaction| {
                Box::pin(async move {
                    let _ = sqlx::raw_sql(
                        "PRAGMA trusted_schema=ON; INSERT INTO host_probe (value) VALUES (73)",
                    )
                    .execute(&mut *transaction)
                    .await;
                    Ok::<_, Infallible>(())
                })
            })
            .await
            .expect_err("precommit policy rejection must precede the commit-edge hook");
        assert_eq!(
            error.kind(),
            ServiceSqliteTransactionErrorKind::NotCommitted
        );
        assert!(!host.failpoints.fired());
        assert_eq!(
            host.failpoints.reached(),
            [
                DurabilityFailpoint::TransactionBeforeBegin,
                DurabilityFailpoint::TransactionAfterBegin,
            ]
        );
        host.failpoints.disarm();
        assert_eq!(row_count(&host).await, 0);
        host.close().await.expect("close policy-drift host");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn backup_durability_edges_fail_once_clean_exact_stage_and_recover() {
        use crate::failpoint::DurabilityFailpoint;

        let _serial = CAPTURE_TEST_LOCK.lock().await;
        for (index, (point, expected_phase)) in [
            (DurabilityFailpoint::BackupBeforeCreate, 1),
            (DurabilityFailpoint::BackupAfterCreate, 1),
            (DurabilityFailpoint::BackupBeforeCopy, 2),
            (DurabilityFailpoint::BackupAfterCopy, 3),
            (DurabilityFailpoint::BackupBeforeFileSync, 4),
            (DurabilityFailpoint::BackupAfterFileSync, 4),
            (DurabilityFailpoint::BackupBeforeDirectorySync, 5),
            (DurabilityFailpoint::BackupAfterDirectorySync, 5),
        ]
        .into_iter()
        .enumerate()
        {
            let (root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
            let stage = root.path().join(format!("failpoint-backup-{index}"));
            host.arm_durability_failpoint(point);
            let error = host
                .capture_online_backup(
                    &stage,
                    crate::BackupCreatedAtUnixMs::new(1_700_000_072_000).expect("capture time"),
                )
                .await
                .expect_err("injected backup edge");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Backup);
            assert!(host.failpoints.fired());
            assert_eq!(host.failpoints.reached().last(), Some(&point));
            assert_eq!(
                host.failpoints.observation(point),
                Some(expected_phase),
                "backup failpoint must fire in its named lifecycle phase"
            );
            assert!(!stage.exists(), "owned failed stage is cleaned");
            let recovery = root.path().join(format!("recovered-backup-{index}"));
            host.capture_online_backup(
                &recovery,
                crate::BackupCreatedAtUnixMs::new(1_700_000_072_001).expect("capture time"),
            )
            .await
            .expect("one-shot failpoint permits retry");
            assert!(recovery.join(crate::BACKUP_STATE_MEMBER_NAME).is_file());
            host.close().await.expect("close backup host");
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn close_durability_edges_are_once_only_retryable_or_terminal() {
        use crate::failpoint::DurabilityFailpoint;

        let all_close_edges = [
            DurabilityFailpoint::CloseBeforeDrain,
            DurabilityFailpoint::CloseAfterDrain,
            DurabilityFailpoint::CloseBeforeCheckpoint,
            DurabilityFailpoint::CloseAfterCheckpoint,
            DurabilityFailpoint::CloseBeforeConnectionClose,
            DurabilityFailpoint::CloseAfterConnectionClose,
            DurabilityFailpoint::CloseBeforeAuthorityRelease,
            DurabilityFailpoint::CloseAfterAuthorityRelease,
        ];
        for (point, expected, retryable, expected_phase, expected_reached) in [
            (
                DurabilityFailpoint::CloseBeforeDrain,
                ServiceSqliteErrorKind::Open,
                true,
                0,
                &all_close_edges[..1],
            ),
            (
                DurabilityFailpoint::CloseAfterDrain,
                ServiceSqliteErrorKind::Open,
                true,
                0,
                &all_close_edges[..2],
            ),
            (
                DurabilityFailpoint::CloseBeforeCheckpoint,
                ServiceSqliteErrorKind::Pragma,
                false,
                6,
                &[
                    DurabilityFailpoint::CloseBeforeDrain,
                    DurabilityFailpoint::CloseAfterDrain,
                    DurabilityFailpoint::CloseBeforeCheckpoint,
                    DurabilityFailpoint::CloseBeforeConnectionClose,
                    DurabilityFailpoint::CloseAfterConnectionClose,
                    DurabilityFailpoint::CloseBeforeAuthorityRelease,
                    DurabilityFailpoint::CloseAfterAuthorityRelease,
                ][..],
            ),
            (
                DurabilityFailpoint::CloseAfterCheckpoint,
                ServiceSqliteErrorKind::Pragma,
                false,
                6,
                &all_close_edges[..],
            ),
            (
                DurabilityFailpoint::CloseBeforeConnectionClose,
                ServiceSqliteErrorKind::Open,
                false,
                6,
                &all_close_edges[..],
            ),
            (
                DurabilityFailpoint::CloseAfterConnectionClose,
                ServiceSqliteErrorKind::Open,
                false,
                6,
                &all_close_edges[..],
            ),
            (
                DurabilityFailpoint::CloseBeforeAuthorityRelease,
                ServiceSqliteErrorKind::Authority,
                true,
                6,
                &all_close_edges[..7],
            ),
            (
                DurabilityFailpoint::CloseAfterAuthorityRelease,
                ServiceSqliteErrorKind::Authority,
                false,
                6,
                &all_close_edges[..],
            ),
        ] {
            let (_root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
            host.arm_durability_failpoint(point);
            let error = host.close().await.expect_err("injected close edge");
            assert_eq!(error.kind(), expected);
            assert!(host.failpoints.fired());
            assert_eq!(host.failpoints.reached(), expected_reached);
            assert_eq!(
                host.pool.close_phase(),
                expected_phase,
                "close failpoint must fire in its named driver phase"
            );
            if retryable {
                host.close().await.expect("one-shot close edge resumes");
            } else {
                assert_eq!(
                    host.close()
                        .await
                        .expect_err("terminal close result is cached")
                        .kind(),
                    expected
                );
            }
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn cancellation_quarantines_connection_and_pool_recovers() {
        let (_root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let host = Arc::new(host);
        let entered = Arc::new(AtomicBool::new(false));
        let task = tokio::spawn({
            let host = Arc::clone(&host);
            let entered = Arc::clone(&entered);
            async move {
                host.transaction::<(), Infallible, _>(|transaction| {
                    Box::pin(async move {
                        sqlx::query("INSERT INTO host_probe (value) VALUES (77)")
                            .execute(&mut *transaction)
                            .await
                            .expect("tentative insert");
                        entered.store(true, Ordering::Release);
                        std::future::pending::<()>().await;
                        Ok(())
                    })
                })
                .await
            }
        });
        while !entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        task.abort();
        assert!(
            task.await
                .expect_err("task must be cancelled")
                .is_cancelled()
        );
        assert_eq!(row_count(&host).await, 0);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn complete_statement_control_inventory_is_sticky_and_fails_closed() {
        for statement in [
            "INSERT INTO host_probe (value) VALUES (0); /* policy */ PrAgMa\ntrusted_schema=ON",
            "INSERT INTO host_probe (value) VALUES (1); ATTACH DATABASE ':memory:' AS extra",
            "INSERT INTO host_probe (value) VALUES (2); DETACH DATABASE extra",
            "INSERT INTO host_probe (value) VALUES (3); BEGIN DEFERRED",
            "INSERT INTO host_probe (value) VALUES (4); COMMIT",
            "INSERT INTO host_probe (value) VALUES (5); END",
            "INSERT INTO host_probe (value) VALUES (6); ROLLBACK",
            "INSERT INTO host_probe (value) VALUES (7); SAVEPOINT escaped",
            "INSERT INTO host_probe (value) VALUES (8); RELEASE SAVEPOINT escaped",
        ] {
            let (_root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
            let error = host
                .transaction(|transaction| {
                    Box::pin(async move {
                        let _ = sqlx::raw_sql(statement).execute(&mut *transaction).await;
                        Ok::<_, Infallible>(())
                    })
                })
                .await
                .expect_err("escape attempt must not commit");
            assert_eq!(
                error.kind(),
                ServiceSqliteTransactionErrorKind::NotCommitted
            );
            assert!(error.sqlite_error().is_some());
            assert_eq!(row_count(&host).await, 0);
        }

        let (_root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let error = host
            .transaction(|transaction| {
                Box::pin(async move {
                    let _ = sqlx::raw_sql("INSERT INTO host_probe (value) VALUES (4); COMMIT")
                        .execute(&mut *transaction)
                        .await;
                    let _ =
                        sqlx::raw_sql("BEGIN DEFERRED; INSERT INTO host_probe (value) VALUES (5)")
                            .execute(&mut *transaction)
                            .await;
                    Ok::<_, Infallible>(())
                })
            })
            .await
            .expect_err("replacement transaction after denied COMMIT must not escape");
        assert_eq!(
            error.kind(),
            ServiceSqliteTransactionErrorKind::NotCommitted
        );
        assert_eq!(row_count(&host).await, 0);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn prepared_query_policy_rejection_is_sticky_and_rolls_back_prior_work() {
        let (_root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let error = host
            .transaction(|transaction| {
                Box::pin(async move {
                    sqlx::query("INSERT INTO host_probe (value) VALUES (11)")
                        .execute(&mut *transaction)
                        .await
                        .expect("ordinary service statement");
                    let _ = (&mut *transaction)
                        .prepare(SqlStr::from_static(
                            "SELECT 1; /* ignored */ PRAGMA trusted_schema=ON",
                        ))
                        .await;
                    Ok::<_, Infallible>(())
                })
            })
            .await
            .expect_err("ignored prepared-query rejection must block commit");
        assert_eq!(
            error.kind(),
            ServiceSqliteTransactionErrorKind::NotCommitted
        );
        assert_eq!(row_count(&host).await, 0);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn control_words_in_values_and_case_expressions_remain_available() {
        let (_root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let value = host
            .transaction(|transaction| {
                Box::pin(async move {
                    let value = sqlx::query_scalar::<_, String>(
                        "SELECT CASE WHEN 1 = 1 THEN 'commit' ELSE 'end' END",
                    )
                    .fetch_one(&mut *transaction)
                    .await?;
                    sqlx::query("INSERT INTO host_probe (value) VALUES (12)")
                        .execute(&mut *transaction)
                        .await?;
                    Ok::<_, sqlx::Error>(value)
                })
            })
            .await
            .expect("ordinary expression commits");
        assert_eq!(value, "commit");
        assert_eq!(row_count(&host).await, 1);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn attach_detach_is_rejected_before_it_can_create_external_state() {
        let (root, _paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let external_database = root.path().join("forbidden-attachment.sqlite");
        let statement = format!(
            "ATTACH DATABASE '{}' AS extra; DETACH DATABASE extra",
            external_database.display()
        );
        let error = host
            .transaction(|transaction| {
                Box::pin(async move {
                    sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
                        .execute(&mut *transaction)
                        .await
                        .map(|_| ())
                })
            })
            .await
            .expect_err("ATTACH and DETACH must be rejected before SQLite compilation");
        assert_eq!(
            error.kind(),
            ServiceSqliteTransactionErrorKind::OperationRolledBack
        );
        assert!(!external_database.exists());
        assert_eq!(row_count(&host).await, 0);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn writer_lock_replacement_and_insecure_directory_revoke_live_host() {
        let (_root, paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let retired_lock = paths
            .state_lock()
            .parent()
            .expect("state directory")
            .join("retired-state.lock");
        fs::rename(paths.state_lock(), &retired_lock).expect("replace canonical lock name");
        let replacement_authority = WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting)
            .expect("replacement authority acquisition")
            .expect("new writer authority");
        let replaced = host
            .transaction(|_| Box::pin(async { Ok::<_, Infallible>(()) }))
            .await
            .expect_err("old writer authority must reject the replacement lock");
        assert_eq!(
            replaced.kind(),
            ServiceSqliteTransactionErrorKind::NotCommitted
        );
        assert_eq!(
            replaced.sqlite_error().map(ServiceSqliteError::kind),
            Some(ServiceSqliteErrorKind::Authority)
        );
        drop(replacement_authority);
        drop(host);

        let (_root, paths, _identity, _migrations, _schema, host) = initialized_host().await;
        let directory = paths.state_database().parent().expect("state directory");
        let original_mode = fs::metadata(directory)
            .expect("state directory metadata")
            .permissions()
            .mode()
            & 0o777;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o770))
            .expect("make directory insecure");
        let insecure = host
            .transaction(|_| Box::pin(async { Ok::<_, Infallible>(()) }))
            .await
            .expect_err("insecure live directory must revoke authority");
        assert_eq!(
            insecure.kind(),
            ServiceSqliteTransactionErrorKind::NotCommitted
        );
        assert_eq!(
            insecure.sqlite_error().map(ServiceSqliteError::kind),
            Some(ServiceSqliteErrorKind::Authority)
        );
        fs::set_permissions(directory, fs::Permissions::from_mode(original_mode))
            .expect("restore directory mode");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn inspection_lock_replacement_and_new_writer_revoke_live_inspection() {
        let (_root, paths, identity, migrations, schema, host) = initialized_host().await;
        host.close().await.expect("close writer host");
        drop(host);
        let inspection = ServiceSqliteHost::open_read_only_inspection(
            &paths,
            &identity,
            &migrations,
            &schema,
            ServiceSqliteConnectionOptions::reviewed(),
        )
        .await
        .expect("open inspection host");
        let retired_lock = paths
            .state_lock()
            .parent()
            .expect("state directory")
            .join("inspection-state.lock");
        fs::rename(paths.state_lock(), retired_lock).expect("replace inspection lock name");
        let (writer, _outcome) = ServiceSqliteHost::open_read_write_existing(
            &paths,
            &identity,
            &migrations,
            &schema,
            ServiceSqliteConnectionOptions::reviewed(),
            MigrationAppliedAtUnixSeconds::new(1_700_000_001).expect("migration time"),
            &build_identity(),
            &[],
        )
        .await
        .expect("open replacement writer");
        writer
            .transaction(|transaction| {
                Box::pin(async move {
                    sqlx::query("INSERT INTO host_probe (value) VALUES (88)")
                        .execute(&mut *transaction)
                        .await
                        .map(|_| ())
                })
            })
            .await
            .expect("replacement writer commits");
        let stale = inspection
            .transaction(|transaction| {
                Box::pin(async move {
                    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM host_probe")
                        .fetch_one(&mut *transaction)
                        .await
                })
            })
            .await
            .expect_err("stale inspection authority must refuse work");
        assert_eq!(
            stale.kind(),
            ServiceSqliteTransactionErrorKind::NotCommitted
        );
        assert_eq!(
            stale.sqlite_error().map(ServiceSqliteError::kind),
            Some(ServiceSqliteErrorKind::Authority)
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn read_only_writes_roll_back_and_internal_pool_close_refuses_new_work() {
        let (root, paths, identity, migrations, schema, host) = initialized_host().await;
        host.close().await.expect("close writer host");
        let closed = host
            .transaction(|_| Box::pin(async { Ok::<_, Infallible>(()) }))
            .await
            .expect_err("closed pool must refuse work");
        assert_eq!(
            closed.kind(),
            ServiceSqliteTransactionErrorKind::NotCommitted
        );
        drop(host);

        let inspection = ServiceSqliteHost::open_read_only_inspection(
            &paths,
            &identity,
            &migrations,
            &schema,
            ServiceSqliteConnectionOptions::reviewed(),
        )
        .await
        .expect("open read-only host");
        let error = inspection
            .transaction(|transaction| {
                Box::pin(async move {
                    sqlx::query("INSERT INTO host_probe (value) VALUES (5)")
                        .execute(&mut *transaction)
                        .await
                        .map(|_| ())
                })
            })
            .await
            .expect_err("read-only write must fail");
        assert_eq!(
            error.kind(),
            ServiceSqliteTransactionErrorKind::OperationRolledBack
        );
        assert_eq!(row_count(&inspection).await, 0);
        drop(inspection);
        drop(root);
    }

    #[test]
    fn host_and_transaction_errors_are_redacted_and_source_free() {
        let error = ServiceSqliteTransactionError::not_committed_with_operation(
            Some("operation-secret"),
            ServiceSqliteError::new(ServiceSqliteErrorKind::Open),
        );
        let debug = format!("{error:?}");
        assert!(!debug.contains("operation-secret"));
        assert!(!debug.contains("state.sqlite"));
        assert!(error.source().is_none());
        assert_eq!(error.operation_error(), Some(&"operation-secret"));
        assert_eq!(
            error.sqlite_error().map(ServiceSqliteError::kind),
            Some(ServiceSqliteErrorKind::Open)
        );
    }
}
