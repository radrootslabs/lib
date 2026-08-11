//! Narrow service-store host and transaction execution boundary.

use core::fmt;
use std::{
    error::Error,
    future::Future,
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
    MigrationApplicationOutcome, MigrationAppliedAtUnixSeconds, MigrationBuildIdentity,
    MigrationCallbackBinding, MigrationCatalog, OpenMode, SchemaCatalog, ServiceDatabaseIdentity,
    ServiceSqliteConnectionOptions, ServiceSqliteError, ServiceSqliteErrorKind, ServiceSqlitePaths,
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
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
enum ServiceSqliteHostCloseState {
    Pending,
    Complete(Option<ServiceSqliteErrorKind>),
}

impl ServiceSqliteHost {
    /// Opens existing writable state and finishes every pending governed migration.
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
            let close = self.pool.close_explicit().await;
            match close {
                Err(retryable) => Err(retryable),
                Ok(terminal) => {
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
        if self.closing.load(Ordering::Acquire) {
            return Err(ServiceSqliteTransactionError::not_committed(
                ServiceSqliteError::new(ServiceSqliteErrorKind::Open),
            ));
        }
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
        self.pool
            .validate()
            .map_err(ServiceSqliteTransactionError::not_committed)?;

        let operation_result = {
            let database_control_rejected = Arc::new(AtomicBool::new(false));
            let mut executor = ServiceSqliteTransaction {
                connection: &mut transaction,
                database_control_rejected: Arc::clone(&database_control_rejected),
            };
            (operation(&mut executor).await, database_control_rejected)
        };
        let (operation_result, database_control_rejected) = operation_result;
        if let Err(error) = self.pool.validate() {
            let operation_error = operation_result.err();
            let permit = gate.permit_runner_rollback();
            let rollback = transaction.rollback().await.map_err(sqlite_source);
            drop(permit);
            let rollback_was_confirmed =
                gate.rejected_commit_rolled_back() && !connection.is_in_transaction();
            let remove = gate.remove(&mut connection).await.map_err(sqlite_source);
            let rollback_error = rollback
                .err()
                .filter(|_| !rollback_was_confirmed)
                .or_else(|| remove.err());
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
                if let Some(error) = rollback
                    .err()
                    .filter(|_| !rollback_was_confirmed)
                    .or_else(|| remove.err())
                    .or_else(|| authority.err())
                {
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
                &database_control_rejected,
            )
            .await;
        if let Err(error) = precommit {
            let permit = gate.permit_runner_rollback();
            let rollback = transaction.rollback().await.map_err(sqlite_source);
            drop(permit);
            let rollback_was_confirmed =
                gate.rejected_commit_rolled_back() && !connection.is_in_transaction();
            let remove = gate.remove(&mut connection).await.map_err(sqlite_source);
            let authority = self.pool.validate();
            if let Some(rollback_error) = rollback
                .err()
                .filter(|_| !rollback_was_confirmed)
                .or_else(|| remove.err())
                .or_else(|| authority.err())
            {
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
        let commit = match commit {
            Ok(()) => Ok(()),
            Err(error) => Err(ServiceSqliteTransactionError::commit_outcome_unknown(error)),
        };
        let remove = gate.remove(&mut connection).await.map_err(sqlite_source);
        commit?;
        remove.map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        self.pool
            .validate()
            .map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        let final_policy = crate::migration::read_connection_policy(&mut connection)
            .await
            .map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        self.pool
            .validate()
            .map_err(ServiceSqliteTransactionError::commit_outcome_unknown)?;
        if final_policy != initial_policy {
            return Err(ServiceSqliteTransactionError::commit_outcome_unknown(
                ServiceSqliteError::new(ServiceSqliteErrorKind::Pragma),
            ));
        }
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
    fn from_pool(mode: OpenMode, pool: crate::open::PrivateConnectionPool) -> Self {
        Self {
            mode,
            pool,
            closing: AtomicBool::new(false),
            close_state: tokio::sync::Mutex::new(ServiceSqliteHostCloseState::Pending),
        }
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
        database_control_rejected: &AtomicBool,
    ) -> Result<(), ServiceSqliteError> {
        self.pool.validate()?;
        if gate.control_violation_observed() || database_control_rejected.load(Ordering::Acquire) {
            return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Open));
        }
        crate::migration::assert_governed_transaction(connection).await?;
        self.pool.validate()?;
        if &crate::migration::read_connection_policy(connection).await? != initial_policy {
            return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Pragma));
        }
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
        if gate.control_violation_observed() {
            return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Open));
        }
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
    database_control_rejected: Arc<AtomicBool>,
}

struct RestrictedExecute<Q> {
    query: Q,
    database_control_rejected: Arc<AtomicBool>,
}

impl<'query, Q> Execute<'query, Sqlite> for RestrictedExecute<Q>
where
    Q: Execute<'query, Sqlite>,
{
    fn sql(self) -> SqlStr {
        restricted_sql(self.query.sql(), &self.database_control_rejected)
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

fn restricted_sql(sql: SqlStr, database_control_rejected: &AtomicBool) -> SqlStr {
    if contains_database_control(sql.as_str()) {
        database_control_rejected.store(true, Ordering::Release);
        SqlStr::from_static("RADROOTS_FORBIDDEN_DATABASE_CONTROL")
    } else {
        sql
    }
}

pub(crate) fn contains_database_control(sql: &str) -> bool {
    sql.as_bytes()
        .split(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
        .any(|token| token.eq_ignore_ascii_case(b"attach") || token.eq_ignore_ascii_case(b"detach"))
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
            database_control_rejected: Arc::clone(&self.database_control_rejected),
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
            database_control_rejected: Arc::clone(&self.database_control_rejected),
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
            restricted_sql(sql, &self.database_control_rejected),
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
    async fn transaction_control_policy_and_attachment_escapes_fail_closed() {
        for (statement, expected_kind) in [
            (
                "INSERT INTO host_probe (value) VALUES (0); COMMIT",
                ServiceSqliteTransactionErrorKind::RollbackFailed,
            ),
            (
                "ROLLBACK; BEGIN DEFERRED; INSERT INTO host_probe (value) VALUES (1)",
                ServiceSqliteTransactionErrorKind::NotCommitted,
            ),
            (
                "PRAGMA trusted_schema=ON; INSERT INTO host_probe (value) VALUES (2)",
                ServiceSqliteTransactionErrorKind::NotCommitted,
            ),
            (
                "ATTACH DATABASE ':memory:' AS extra; INSERT INTO host_probe (value) VALUES (3)",
                ServiceSqliteTransactionErrorKind::NotCommitted,
            ),
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
            assert_eq!(error.kind(), expected_kind);
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

    #[test]
    fn database_control_token_screen_is_closed_and_case_insensitive() {
        for forbidden in [
            "ATTACH DATABASE 'x' AS extra",
            "detach database extra",
            "SELECT 1; /* ignored */ AtTaCh ':memory:' AS x",
            "SELECT 'attach'",
        ] {
            assert!(contains_database_control(forbidden));
        }
        for allowed in [
            "SELECT attachment FROM items",
            "SELECT detached FROM items",
            "SELECT COUNT(*) FROM host_probe",
        ] {
            assert!(!contains_database_control(allowed));
        }
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
        let error = ServiceSqliteTransactionError::rollback_failed(
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
