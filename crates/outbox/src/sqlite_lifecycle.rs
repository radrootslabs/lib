#![forbid(unsafe_code)]

use crate::RadrootsOutboxError;
use crate::migrations::{
    RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT, RADROOTS_OUTBOX_SCHEMA_VERSION_MIN,
};
use crate::schema::{
    RadrootsOutboxSchemaStatus, inspect_outbox_schema_on_connection,
    migrate_outbox_schema_on_connection, rollback_outbox_schema_on_connection,
    validate_full_database_integrity, validate_outbox_owned_integrity,
};
#[cfg(test)]
use crate::schema::{SQLITE_IDENTIFIER_BYTES_MAX, SQLITE_LEDGER_NAME_BYTES_MAX};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{SqliteConnection, SqlitePool};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// Maximum public error diagnostic size in UTF-8 bytes.
pub const RADROOTS_OUTBOX_DIAGNOSTIC_BYTES_MAX: usize = 4_096;
/// Maximum accepted encoded database path size in bytes.
pub const RADROOTS_OUTBOX_FILE_PATH_BYTES_MAX: usize = 4_096;
/// Governed maximum number of connections in a file-backed outbox pool.
pub const RADROOTS_OUTBOX_FILE_CONNECTION_LIMIT: u32 = 4;
/// Total open and offline-maintenance deadline in milliseconds.
pub const RADROOTS_OUTBOX_OPEN_DEADLINE_MILLIS: u64 = 5_000;

const MEMORY_CONNECTION_LIMIT: u32 = 1;
const OPEN_DEADLINE: Duration = Duration::from_millis(RADROOTS_OUTBOX_OPEN_DEADLINE_MILLIS);

/// Explicit acknowledgement required by destructive offline rollback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxRollbackConfirmation(());

impl RadrootsOutboxRollbackConfirmation {
    /// Acknowledges that migrations above the exact target may lose data.
    pub const fn acknowledge_data_loss() -> Self {
        Self(())
    }
}

pub(crate) struct OpenedOutboxDatabase {
    pub(crate) pool: SqlitePool,
    pub(crate) file_lease: Option<Arc<OutboxFileLease>>,
}

#[derive(Debug)]
pub(crate) struct OutboxFileLease {
    path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileLifecycleState {
    Open(usize),
    OfflineRollback,
}

static FILE_LIFECYCLES: OnceLock<Mutex<BTreeMap<PathBuf, FileLifecycleState>>> = OnceLock::new();

fn file_lifecycles() -> MutexGuard<'static, BTreeMap<PathBuf, FileLifecycleState>> {
    FILE_LIFECYCLES
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl OutboxFileLease {
    fn acquire(path: PathBuf) -> Result<Arc<Self>, RadrootsOutboxError> {
        let mut lifecycles = file_lifecycles();
        match lifecycles.get_mut(&path) {
            Some(FileLifecycleState::Open(count)) => {
                *count =
                    count
                        .checked_add(1)
                        .ok_or(RadrootsOutboxError::SqliteLifecycleFailure {
                            stage: "file lifecycle lease count",
                        })?;
            }
            Some(FileLifecycleState::OfflineRollback) => {
                return Err(RadrootsOutboxError::SqliteOfflineRollbackInProgress);
            }
            None => {
                lifecycles.insert(path.clone(), FileLifecycleState::Open(1));
            }
        }
        drop(lifecycles);
        Ok(Arc::new(Self { path }))
    }
}

impl Drop for OutboxFileLease {
    fn drop(&mut self) {
        let mut lifecycles = file_lifecycles();
        match lifecycles.get_mut(&self.path) {
            Some(FileLifecycleState::Open(1)) => {
                lifecycles.remove(&self.path);
            }
            Some(FileLifecycleState::Open(count)) => *count -= 1,
            Some(FileLifecycleState::OfflineRollback) | None => {}
        }
    }
}

struct OfflineRollbackLease {
    path: PathBuf,
}

impl OfflineRollbackLease {
    fn acquire(path: PathBuf) -> Result<Self, RadrootsOutboxError> {
        let mut lifecycles = file_lifecycles();
        match lifecycles.get(&path) {
            Some(FileLifecycleState::Open(_)) => {
                return Err(RadrootsOutboxError::SqliteOfflineRollbackHasLiveHandles);
            }
            Some(FileLifecycleState::OfflineRollback) => {
                return Err(RadrootsOutboxError::SqliteOfflineRollbackInProgress);
            }
            None => {
                lifecycles.insert(path.clone(), FileLifecycleState::OfflineRollback);
            }
        }
        drop(lifecycles);
        Ok(Self { path })
    }
}

impl Drop for OfflineRollbackLease {
    fn drop(&mut self) {
        let mut lifecycles = file_lifecycles();
        if matches!(
            lifecycles.get(&self.path),
            Some(FileLifecycleState::OfflineRollback)
        ) {
            lifecycles.remove(&self.path);
        }
    }
}

#[derive(Clone, Copy)]
enum OpenFailureInjection {
    None,
    #[cfg(test)]
    AfterAuthority,
}

struct OpenDeadline {
    started: Instant,
    limit: Duration,
}

impl OpenDeadline {
    fn new(limit: Duration) -> Self {
        Self {
            started: Instant::now(),
            limit,
        }
    }

    fn exceeded(&self, stage: &'static str) -> RadrootsOutboxError {
        RadrootsOutboxError::SqliteOpenDeadlineExceeded {
            stage,
            limit_ms: u64::try_from(self.limit.as_millis()).unwrap_or(u64::MAX),
        }
    }

    fn remaining(&self, stage: &'static str) -> Result<Duration, RadrootsOutboxError> {
        self.limit
            .checked_sub(self.started.elapsed())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| self.exceeded(stage))
    }

    async fn run<T, F>(&self, stage: &'static str, operation: F) -> Result<T, RadrootsOutboxError>
    where
        F: Future<Output = Result<T, RadrootsOutboxError>>,
    {
        let _remaining = self.remaining(stage)?;
        #[cfg(feature = "runtime-tokio")]
        {
            tokio::time::timeout(_remaining, operation)
                .await
                .map_err(|_| self.exceeded(stage))?
        }
        #[cfg(not(feature = "runtime-tokio"))]
        {
            let result = operation.await;
            if self.started.elapsed() >= self.limit {
                return Err(self.exceeded(stage));
            }
            result
        }
    }

    async fn wait_for_lock_retry(&self) -> Result<(), RadrootsOutboxError> {
        const RETRY_INTERVAL: Duration = Duration::from_millis(10);
        let remaining = self.remaining("SQLite lock contention")?;
        let delay = remaining.min(RETRY_INTERVAL);
        #[cfg(feature = "runtime-tokio")]
        tokio::time::sleep(delay).await;
        #[cfg(not(feature = "runtime-tokio"))]
        std::thread::sleep(delay);
        Ok(())
    }
}

pub(crate) async fn open_memory() -> Result<OpenedOutboxDatabase, RadrootsOutboxError> {
    let deadline = OpenDeadline::new(OPEN_DEADLINE);
    let options = SqliteConnectOptions::from_str("sqlite::memory:")?
        .foreign_keys(true)
        .busy_timeout(OPEN_DEADLINE);
    open_owned_pool(
        options,
        false,
        MEMORY_CONNECTION_LIMIT,
        deadline,
        None,
        OpenFailureInjection::None,
    )
    .await
}

pub(crate) async fn open_file(path: &Path) -> Result<OpenedOutboxDatabase, RadrootsOutboxError> {
    open_file_with(path, OPEN_DEADLINE, OpenFailureInjection::None).await
}

async fn open_file_with(
    path: &Path,
    limit: Duration,
    injection: OpenFailureInjection,
) -> Result<OpenedOutboxDatabase, RadrootsOutboxError> {
    let deadline = OpenDeadline::new(limit);
    let lifecycle_path = canonical_lifecycle_path(path)?;
    deadline.remaining("file path resolution")?;
    let file_lease = OutboxFileLease::acquire(lifecycle_path)?;
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(limit);
    open_owned_pool(
        options,
        true,
        RADROOTS_OUTBOX_FILE_CONNECTION_LIMIT,
        deadline,
        Some(file_lease),
        injection,
    )
    .await
}

async fn open_owned_pool(
    options: SqliteConnectOptions,
    file_backed: bool,
    connection_limit: u32,
    deadline: OpenDeadline,
    file_lease: Option<Arc<OutboxFileLease>>,
    injection: OpenFailureInjection,
) -> Result<OpenedOutboxDatabase, RadrootsOutboxError> {
    let pool = SqlitePoolOptions::new()
        .min_connections(0)
        .max_connections(connection_limit)
        .acquire_timeout(deadline.limit)
        .connect_lazy_with(options.clone());
    let result = loop {
        match authenticate_owned_pool(&pool, options.clone(), file_backed, &deadline, injection)
            .await
        {
            Err(error) if is_sqlite_lock_contention(&error) => {
                if let Err(deadline_error) = deadline.wait_for_lock_retry().await {
                    break Err(deadline_error);
                }
            }
            result => break result,
        }
    };
    if let Err(error) = result {
        pool.close().await;
        return Err(error);
    }
    Ok(OpenedOutboxDatabase { pool, file_lease })
}

fn is_sqlite_lock_contention(error: &RadrootsOutboxError) -> bool {
    let RadrootsOutboxError::Sqlx(sqlx::Error::Database(database)) = error else {
        return false;
    };
    database
        .code()
        .and_then(|code| code.parse::<u32>().ok())
        .is_some_and(|code| matches!(code & 0xff, 5 | 6))
}

async fn authenticate_owned_pool(
    pool: &SqlitePool,
    options: SqliteConnectOptions,
    file_backed: bool,
    deadline: &OpenDeadline,
    injection: OpenFailureInjection,
) -> Result<(), RadrootsOutboxError> {
    let mut connection = deadline
        .run("connection", async {
            pool.acquire().await.map_err(RadrootsOutboxError::from)
        })
        .await?;
    deadline
        .run("connection-local safety settings", async {
            validate_connection_local_authority(&mut connection).await
        })
        .await?;
    deadline
        .run("schema authentication", async {
            inspect_outbox_schema_on_connection(&mut connection)
                .await
                .map(|_| ())
        })
        .await?;
    #[cfg(test)]
    if matches!(injection, OpenFailureInjection::AfterAuthority) {
        return Err(RadrootsOutboxError::SqliteLifecycleFailure {
            stage: "injected post-authority failure",
        });
    }
    #[cfg(not(test))]
    let _ = injection;
    deadline
        .run("schema migration", async {
            migrate_outbox_schema_on_connection(&mut connection).await
        })
        .await?;
    deadline
        .run("managed schema authentication", async {
            match inspect_outbox_schema_on_connection(&mut connection).await? {
                RadrootsOutboxSchemaStatus::Managed { version }
                    if version == RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT =>
                {
                    Ok(())
                }
                _ => Err(RadrootsOutboxError::SqliteLifecycleFailure {
                    stage: "managed schema postcondition",
                }),
            }
        })
        .await?;
    deadline
        .run("owned integrity", async {
            validate_outbox_owned_integrity(&mut connection).await
        })
        .await?;
    if file_backed {
        deadline
            .run("file journal mode", async {
                configure_file_journal_mode(&mut connection).await
            })
            .await?;
        pool.set_connect_options(options.journal_mode(SqliteJournalMode::Wal));
    }
    Ok(())
}

async fn validate_connection_local_authority(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsOutboxError> {
    let encoding: String = sqlx::query_scalar("PRAGMA main.encoding")
        .fetch_one(&mut *connection)
        .await?;
    if encoding != "UTF-8" {
        return Err(RadrootsOutboxError::SqliteMainDatabaseEncodingNotUtf8 { actual: encoding });
    }
    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&mut *connection)
        .await?;
    if foreign_keys != 1 {
        return Err(RadrootsOutboxError::SqliteForeignKeysNotEnabled {
            actual: foreign_keys,
        });
    }
    Ok(())
}

async fn configure_file_journal_mode(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsOutboxError> {
    let actual: String = sqlx::query_scalar("PRAGMA main.journal_mode = WAL")
        .fetch_one(&mut *connection)
        .await?;
    if actual != "wal" {
        return Err(RadrootsOutboxError::SqliteFileJournalModeNotWal { actual });
    }
    Ok(())
}

pub(crate) async fn verify_full_integrity(pool: &SqlitePool) -> Result<(), RadrootsOutboxError> {
    let mut connection = pool.acquire().await?;
    validate_full_database_integrity(&mut connection).await
}

pub(crate) async fn rollback_file_offline(
    path: &Path,
    target: u32,
    confirmation: RadrootsOutboxRollbackConfirmation,
) -> Result<(), RadrootsOutboxError> {
    let deadline = OpenDeadline::new(OPEN_DEADLINE);
    if target < RADROOTS_OUTBOX_SCHEMA_VERSION_MIN {
        return Err(RadrootsOutboxError::RollbackBelowVersionFloor {
            floor: RADROOTS_OUTBOX_SCHEMA_VERSION_MIN,
            target,
        });
    }
    let lifecycle_path = canonical_lifecycle_path(path)?;
    deadline.remaining("offline rollback path resolution")?;
    let _rollback_lease = OfflineRollbackLease::acquire(lifecycle_path)?;
    let _confirmation = confirmation;
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .foreign_keys(true)
        .busy_timeout(OPEN_DEADLINE);
    let pool = SqlitePoolOptions::new()
        .min_connections(0)
        .max_connections(1)
        .acquire_timeout(OPEN_DEADLINE)
        .connect_lazy_with(options);
    let result = rollback_owned_pool(&pool, target, &deadline).await;
    pool.close().await;
    result
}

async fn rollback_owned_pool(
    pool: &SqlitePool,
    target: u32,
    deadline: &OpenDeadline,
) -> Result<(), RadrootsOutboxError> {
    let mut connection = deadline
        .run("offline rollback connection", async {
            pool.acquire().await.map_err(RadrootsOutboxError::from)
        })
        .await?;
    deadline
        .run("offline rollback connection authority", async {
            validate_connection_local_authority(&mut connection).await
        })
        .await?;
    let current = deadline
        .run("offline rollback preflight", async {
            match inspect_outbox_schema_on_connection(&mut connection).await? {
                RadrootsOutboxSchemaStatus::Managed { version } => Ok(version),
                _ => Err(RadrootsOutboxError::RollbackUnmanaged),
            }
        })
        .await?;
    if target > current {
        return Err(RadrootsOutboxError::RollbackAhead { current, target });
    }
    deadline
        .run("offline rollback owned integrity preflight", async {
            validate_outbox_owned_integrity(&mut connection).await
        })
        .await?;
    deadline
        .run("offline rollback transaction", async {
            rollback_outbox_schema_on_connection(&mut connection, target).await
        })
        .await?;
    deadline
        .run("offline rollback postcondition", async {
            match inspect_outbox_schema_on_connection(&mut connection).await? {
                RadrootsOutboxSchemaStatus::Managed { version } if version == target => {
                    validate_outbox_owned_integrity(&mut connection).await
                }
                _ => Err(RadrootsOutboxError::SqliteLifecycleFailure {
                    stage: "offline rollback target postcondition",
                }),
            }
        })
        .await
}

fn canonical_lifecycle_path(path: &Path) -> Result<PathBuf, RadrootsOutboxError> {
    validate_path_bytes(path)?;
    let filename = path
        .file_name()
        .filter(|filename| !filename.is_empty())
        .ok_or(RadrootsOutboxError::SqliteFilePathInvalid)?;
    let resolved = if path.exists() {
        std::fs::canonicalize(path)
    } else {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty());
        std::fs::canonicalize(parent.unwrap_or_else(|| Path::new(".")))
            .map(|parent| parent.join(filename))
    }
    .map_err(|source| RadrootsOutboxError::SqliteFilePathResolutionFailed { source })?;
    validate_path_bytes(&resolved)?;
    Ok(resolved)
}

fn validate_path_bytes(path: &Path) -> Result<(), RadrootsOutboxError> {
    let actual = path.as_os_str().as_encoded_bytes().len();
    if actual > RADROOTS_OUTBOX_FILE_PATH_BYTES_MAX {
        return Err(RadrootsOutboxError::SqliteFilePathTooLong {
            max: RADROOTS_OUTBOX_FILE_PATH_BYTES_MAX,
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RadrootsOutbox, RadrootsOutboxSchemaStatus};
    use sqlx::Connection;

    async fn direct_connection(path: &Path) -> SqliteConnection {
        SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .expect("direct connection")
    }

    async fn journal_mode(path: &Path) -> String {
        let mut connection = direct_connection(path).await;
        sqlx::query_scalar("PRAGMA main.journal_mode")
            .fetch_one(&mut connection)
            .await
            .expect("journal mode")
    }

    #[tokio::test]
    async fn sqlite_lifecycle_owned_pool_is_bounded_lazy_and_reopen_is_authenticated() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("owned.sqlite");
        let store = RadrootsOutbox::open_file(&path).await.expect("open");
        assert_eq!(
            store.pool().options().get_max_connections(),
            RADROOTS_OUTBOX_FILE_CONNECTION_LIMIT
        );
        assert_eq!(store.pool().size(), 1);
        assert_eq!(store.pragma_foreign_keys().await.expect("foreign keys"), 1);
        assert_eq!(
            store.pragma_busy_timeout().await.expect("busy timeout"),
            i64::try_from(RADROOTS_OUTBOX_OPEN_DEADLINE_MILLIS).expect("deadline")
        );
        assert_eq!(store.pragma_journal_mode().await.expect("journal"), "wal");
        assert_eq!(
            store.schema_status().await.expect("schema"),
            RadrootsOutboxSchemaStatus::Managed {
                version: RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT,
            }
        );
        store
            .verify_full_database_integrity()
            .await
            .expect("explicit full integrity");
        store.close().await;

        let reopened = RadrootsOutbox::open_file(&path).await.expect("reopen");
        assert_eq!(reopened.pool().size(), 1);
        reopened.close().await;
    }

    #[tokio::test]
    async fn sqlite_lifecycle_rejects_hostile_schema_before_persistent_mutation() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("hostile.sqlite");
        let mut connection = direct_connection(&path).await;
        sqlx::raw_sql(
            "CREATE TABLE caller_state(value TEXT NOT NULL);
             INSERT INTO caller_state(value) VALUES ('preserved');
             CREATE TABLE outbox_counterfeit(value TEXT NOT NULL);",
        )
        .execute(&mut connection)
        .await
        .expect("hostile schema");
        connection.close().await.expect("close");
        assert_eq!(journal_mode(&path).await, "delete");

        assert!(matches!(
            RadrootsOutbox::open_file(&path).await,
            Err(RadrootsOutboxError::UnmanagedSchema { .. })
        ));
        assert_eq!(journal_mode(&path).await, "delete");
        let mut connection = direct_connection(&path).await;
        let caller: String = sqlx::query_scalar("SELECT value FROM caller_state")
            .fetch_one(&mut connection)
            .await
            .expect("caller state");
        assert_eq!(caller, "preserved");
        let ledger: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'radroots_outbox_schema_migrations'",
        )
        .fetch_one(&mut connection)
        .await
        .expect("ledger count");
        assert_eq!(ledger, 0);
    }

    #[tokio::test]
    async fn sqlite_lifecycle_bounds_catalog_ledger_path_and_diagnostics() {
        let directory = tempfile::tempdir().expect("tempdir");
        let oversized_path = directory
            .path()
            .join("x".repeat(RADROOTS_OUTBOX_FILE_PATH_BYTES_MAX + 1));
        assert!(matches!(
            RadrootsOutbox::open_file(&oversized_path).await,
            Err(RadrootsOutboxError::SqliteFilePathTooLong { .. })
        ));
        assert!(!oversized_path.exists());

        let catalog_path = directory.path().join("catalog.sqlite");
        let mut connection = direct_connection(&catalog_path).await;
        let oversized_name = format!("outbox_{}", "n".repeat(SQLITE_IDENTIFIER_BYTES_MAX));
        let sql = format!("CREATE TABLE \"{oversized_name}\"(value TEXT)");
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(&mut connection)
            .await
            .expect("oversized catalog identifier");
        connection.close().await.expect("close catalog");
        assert!(matches!(
            RadrootsOutbox::open_file(&catalog_path).await,
            Err(RadrootsOutboxError::SqliteTextLimitExceeded {
                field: "catalog object name",
                ..
            })
        ));

        let ledger_path = directory.path().join("ledger.sqlite");
        let store = RadrootsOutbox::open_file(&ledger_path)
            .await
            .expect("ledger store");
        store.close().await;
        let mut connection = direct_connection(&ledger_path).await;
        sqlx::query("UPDATE radroots_outbox_schema_migrations SET name = ? WHERE version = 1")
            .bind("n".repeat(SQLITE_LEDGER_NAME_BYTES_MAX + 1))
            .execute(&mut connection)
            .await
            .expect("oversized ledger name");
        connection.close().await.expect("close ledger");
        assert!(matches!(
            RadrootsOutbox::open_file(&ledger_path).await,
            Err(RadrootsOutboxError::SqliteTextLimitExceeded {
                field: "migration ledger name",
                ..
            })
        ));

        let diagnostic = RadrootsOutboxError::IntegrityCheckFailed {
            detail: "é".repeat(RADROOTS_OUTBOX_DIAGNOSTIC_BYTES_MAX),
        }
        .public_diagnostic();
        assert!(diagnostic.len() <= RADROOTS_OUTBOX_DIAGNOSTIC_BYTES_MAX);
        assert!(diagnostic.is_char_boundary(diagnostic.len()));
    }

    #[tokio::test]
    async fn sqlite_lifecycle_rejects_non_utf8_and_owned_foreign_key_drift() {
        let directory = tempfile::tempdir().expect("tempdir");
        let encoding_path = directory.path().join("utf16.sqlite");
        let mut encoding = direct_connection(&encoding_path).await;
        sqlx::query("PRAGMA main.encoding = 'UTF-16le'")
            .execute(&mut encoding)
            .await
            .expect("UTF-16 encoding");
        sqlx::query("CREATE TABLE encoding_anchor(value TEXT)")
            .execute(&mut encoding)
            .await
            .expect("encoding anchor");
        encoding.close().await.expect("close encoding");
        assert!(matches!(
            RadrootsOutbox::open_file(&encoding_path).await,
            Err(RadrootsOutboxError::SqliteMainDatabaseEncodingNotUtf8 { .. })
        ));
        assert_eq!(journal_mode(&encoding_path).await, "delete");

        let foreign_key_path = directory.path().join("foreign-key.sqlite");
        let store = RadrootsOutbox::open_file(&foreign_key_path)
            .await
            .expect("foreign-key store");
        store.close().await;
        let mut connection = direct_connection(&foreign_key_path).await;
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut connection)
            .await
            .expect("disable foreign keys");
        sqlx::query(
            "INSERT INTO outbox_event(outbox_event_id, operation_id, event_id, expected_pubkey, draft_json, signed_event_json, raw_event_json, state, attempt_count, claim_token, claim_owner, claim_expires_at_ms, active_delivery_plan_id, next_attempt_after_ms, last_error, event_store_ingested, event_store_inserted, event_store_ingested_at_ms, created_at_ms, updated_at_ms)
             VALUES (1, 999, 'event', 'author', '{}', NULL, NULL, 'draft_queued', 0, NULL, NULL, NULL, NULL, 0, NULL, 0, 0, NULL, 0, 0)",
        )
        .execute(&mut connection)
        .await
        .expect("foreign-key drift");
        connection.close().await.expect("close foreign-key fixture");
        assert!(matches!(
            RadrootsOutbox::open_file(&foreign_key_path).await,
            Err(RadrootsOutboxError::ForeignKeyViolation {
                table,
                ..
            }) if table == "outbox_event"
        ));
    }

    #[tokio::test]
    async fn sqlite_lifecycle_deadline_concurrency_and_failure_injection_are_transactional() {
        let directory = tempfile::tempdir().expect("tempdir");
        let concurrent_path = directory.path().join("concurrent.sqlite");
        let (left, right) = tokio::join!(
            RadrootsOutbox::open_file(&concurrent_path),
            RadrootsOutbox::open_file(&concurrent_path),
        );
        left.expect("left concurrent open").close().await;
        right.expect("right concurrent open").close().await;

        let locked_path = directory.path().join("locked.sqlite");
        let store = RadrootsOutbox::open_file(&locked_path)
            .await
            .expect("locked store");
        store.close().await;
        let mut locker = direct_connection(&locked_path).await;
        let transaction = locker
            .begin_with("BEGIN EXCLUSIVE")
            .await
            .expect("exclusive lock");
        let lock_error = match open_file_with(
            &locked_path,
            Duration::from_millis(50),
            OpenFailureInjection::None,
        )
        .await
        {
            Ok(_) => panic!("locked open must exhaust its deadline"),
            Err(error) => error,
        };
        assert!(
            matches!(
                lock_error,
                RadrootsOutboxError::SqliteOpenDeadlineExceeded {
                    stage: "schema migration" | "SQLite lock contention",
                    limit_ms: 50,
                }
            ),
            "{lock_error:?}"
        );
        transaction.rollback().await.expect("release lock");

        let injected_path = directory.path().join("injected.sqlite");
        let mut connection = direct_connection(&injected_path).await;
        sqlx::raw_sql(
            "CREATE TABLE caller_state(value TEXT NOT NULL);
             INSERT INTO caller_state(value) VALUES ('preserved');",
        )
        .execute(&mut connection)
        .await
        .expect("caller fixture");
        connection.close().await.expect("close fixture");
        assert!(matches!(
            open_file_with(
                &injected_path,
                OPEN_DEADLINE,
                OpenFailureInjection::AfterAuthority,
            )
            .await,
            Err(RadrootsOutboxError::SqliteLifecycleFailure {
                stage: "injected post-authority failure",
            })
        ));
        assert_eq!(journal_mode(&injected_path).await, "delete");
        let mut connection = direct_connection(&injected_path).await;
        let outbox_objects: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name LIKE 'outbox_%' OR name = 'radroots_outbox_schema_migrations'",
        )
        .fetch_one(&mut connection)
        .await
        .expect("outbox object count");
        assert_eq!(outbox_objects, 0);
        let caller: String = sqlx::query_scalar("SELECT value FROM caller_state")
            .fetch_one(&mut connection)
            .await
            .expect("caller state");
        assert_eq!(caller, "preserved");
    }

    #[tokio::test]
    async fn sqlite_lifecycle_offline_rollback_rejects_live_handles_and_exactly_targets() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("rollback.sqlite");
        let store = RadrootsOutbox::open_file(&path).await.expect("store");
        assert!(matches!(
            RadrootsOutbox::rollback_file_schema_offline(
                &path,
                RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT,
                RadrootsOutboxRollbackConfirmation::acknowledge_data_loss(),
            )
            .await,
            Err(RadrootsOutboxError::SqliteOfflineRollbackHasLiveHandles)
        ));
        store.close().await;
        RadrootsOutbox::rollback_file_schema_offline(
            &path,
            RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT,
            RadrootsOutboxRollbackConfirmation::acknowledge_data_loss(),
        )
        .await
        .expect("exact target no-op");
        assert!(matches!(
            RadrootsOutbox::rollback_file_schema_offline(
                &path,
                RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT + 1,
                RadrootsOutboxRollbackConfirmation::acknowledge_data_loss(),
            )
            .await,
            Err(RadrootsOutboxError::RollbackAhead { .. })
        ));
        assert!(matches!(
            RadrootsOutbox::rollback_file_schema_offline(
                &path,
                RADROOTS_OUTBOX_SCHEMA_VERSION_MIN - 1,
                RadrootsOutboxRollbackConfirmation::acknowledge_data_loss(),
            )
            .await,
            Err(RadrootsOutboxError::RollbackBelowVersionFloor { .. })
        ));
    }
}
