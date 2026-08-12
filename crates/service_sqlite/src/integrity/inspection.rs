//! Explicit, bounded integrity inspection over one governed SQLite snapshot.

use serde::Serialize;

use crate::StorageIntegrity;

/// Caller-injected wall-clock time for one completed integrity inspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct IntegrityCheckedAtUnixMs(u64);

impl IntegrityCheckedAtUnixMs {
    /// Constructs a positive timestamp that SQLite can represent exactly.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        if value > 0 && value <= i64::MAX as u64 {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Returns the validated Unix timestamp in milliseconds.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Closed result of one completed bounded database check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityCheckOutcome {
    Verified,
    Failed,
}

/// Stable, content-free diagnostic code for a completed failed check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityDiagnosticCode {
    SqliteIntegrityFailed,
    ForeignKeyViolation,
}

/// Safe bounded result of an explicit host integrity inspection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ServiceSqliteIntegrityReport {
    checked_at_unix_ms: IntegrityCheckedAtUnixMs,
    sqlite: IntegrityCheckOutcome,
    foreign_keys: IntegrityCheckOutcome,
    diagnostics: Box<[IntegrityDiagnosticCode]>,
}

impl ServiceSqliteIntegrityReport {
    #[cfg(any(test, target_os = "linux", target_os = "macos"))]
    pub(crate) fn new(
        checked_at_unix_ms: IntegrityCheckedAtUnixMs,
        sqlite: IntegrityCheckOutcome,
        foreign_keys: IntegrityCheckOutcome,
    ) -> Self {
        let diagnostics: Box<[IntegrityDiagnosticCode]> = match (sqlite, foreign_keys) {
            (IntegrityCheckOutcome::Verified, IntegrityCheckOutcome::Verified) => Box::new([]),
            (IntegrityCheckOutcome::Failed, IntegrityCheckOutcome::Verified) => {
                Box::new([IntegrityDiagnosticCode::SqliteIntegrityFailed])
            }
            (IntegrityCheckOutcome::Verified, IntegrityCheckOutcome::Failed) => {
                Box::new([IntegrityDiagnosticCode::ForeignKeyViolation])
            }
            (IntegrityCheckOutcome::Failed, IntegrityCheckOutcome::Failed) => Box::new([
                IntegrityDiagnosticCode::SqliteIntegrityFailed,
                IntegrityDiagnosticCode::ForeignKeyViolation,
            ]),
        };
        Self {
            checked_at_unix_ms,
            sqlite,
            foreign_keys,
            diagnostics,
        }
    }

    /// Returns the caller-injected completion time.
    #[must_use]
    pub const fn checked_at_unix_ms(&self) -> IntegrityCheckedAtUnixMs {
        self.checked_at_unix_ms
    }

    /// Returns the completed SQLite integrity-check outcome.
    #[must_use]
    pub const fn sqlite(&self) -> IntegrityCheckOutcome {
        self.sqlite
    }

    /// Returns the completed foreign-key-check outcome.
    #[must_use]
    pub const fn foreign_keys(&self) -> IntegrityCheckOutcome {
        self.foreign_keys
    }

    /// Returns zero to two stable diagnostic codes in canonical order.
    #[must_use]
    pub fn diagnostics(&self) -> &[IntegrityDiagnosticCode] {
        &self.diagnostics
    }

    /// Projects this active result into the passive storage-status vocabulary.
    #[must_use]
    pub const fn storage_integrity(&self) -> StorageIntegrity {
        if matches!(self.sqlite, IntegrityCheckOutcome::Verified)
            && matches!(self.foreign_keys, IntegrityCheckOutcome::Verified)
        {
            StorageIntegrity::Verified
        } else {
            StorageIntegrity::Failed
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod native {
    use sqlx::{Connection, Row, SqliteConnection};

    use super::{IntegrityCheckOutcome, IntegrityCheckedAtUnixMs, ServiceSqliteIntegrityReport};
    use crate::{ServiceSqliteError, ServiceSqliteErrorKind};

    const SQLITE_INTEGRITY_SQL: &str = "PRAGMA integrity_check(1)";
    const FOREIGN_KEY_SQL: &str = "SELECT 1 FROM pragma_foreign_key_check LIMIT 1";

    pub(crate) async fn inspect_database_integrity(
        connection: &mut SqliteConnection,
        checked_at: IntegrityCheckedAtUnixMs,
        mut validate: impl FnMut() -> Result<(), ServiceSqliteError>,
    ) -> Result<ServiceSqliteIntegrityReport, ServiceSqliteError> {
        validate()?;
        let transaction = connection.begin().await;
        validate()?;
        let mut transaction = transaction.map_err(|_| integrity_error())?;

        #[cfg(test)]
        if super::test_seam::real_sqlite_probe_enabled() {
            super::test_seam::observe(super::test_seam::PHASE_SQLITE_EXECUTION_AWAITING);
            let probe = sqlx::query_scalar::<_, i64>(
                "WITH RECURSIVE counter(value) AS (
                     VALUES(0) UNION ALL SELECT value + 1 FROM counter WHERE value < 5000000
                 ) SELECT sum(value) FROM counter",
            )
            .fetch_one(&mut *transaction)
            .await;
            validate()?;
            if probe.is_err() {
                return rollback_error(transaction, &mut validate).await;
            }
        }

        #[cfg(test)]
        super::test_seam::pause(super::test_seam::PHASE_BEFORE_SQLITE).await;
        let sqlite_rows = sqlx::query(SQLITE_INTEGRITY_SQL)
            .fetch_all(&mut *transaction)
            .await;
        validate()?;
        let sqlite = match sqlite_rows {
            Ok(rows) if rows.len() == 1 => match rows[0].try_get::<&str, _>(0) {
                Ok(value) => classify_integrity_value(value),
                Err(_) => return rollback_error(transaction, &mut validate).await,
            },
            Ok(_) | Err(_) => return rollback_error(transaction, &mut validate).await,
        };

        #[cfg(test)]
        super::test_seam::pause(super::test_seam::PHASE_BEFORE_FOREIGN_KEYS).await;
        let foreign_key_row = sqlx::query_scalar::<_, i64>(FOREIGN_KEY_SQL)
            .fetch_optional(&mut *transaction)
            .await;
        validate()?;
        let foreign_keys = match foreign_key_row {
            Ok(None) => IntegrityCheckOutcome::Verified,
            Ok(Some(1)) => IntegrityCheckOutcome::Failed,
            Ok(Some(_)) | Err(_) => return rollback_error(transaction, &mut validate).await,
        };

        #[cfg(test)]
        super::test_seam::pause(super::test_seam::PHASE_BEFORE_ROLLBACK).await;
        let rollback = transaction.rollback().await;
        validate()?;
        rollback.map_err(|_| integrity_error())?;
        Ok(ServiceSqliteIntegrityReport::new(
            checked_at,
            sqlite,
            foreign_keys,
        ))
    }

    async fn rollback_error(
        transaction: sqlx::Transaction<'_, sqlx::Sqlite>,
        validate: &mut impl FnMut() -> Result<(), ServiceSqliteError>,
    ) -> Result<ServiceSqliteIntegrityReport, ServiceSqliteError> {
        let rollback = transaction.rollback().await;
        validate()?;
        rollback.map_err(|_| integrity_error())?;
        Err(integrity_error())
    }

    fn integrity_error() -> ServiceSqliteError {
        ServiceSqliteError::new(ServiceSqliteErrorKind::Integrity)
    }

    fn classify_integrity_value(value: &str) -> IntegrityCheckOutcome {
        if value == "ok" {
            IntegrityCheckOutcome::Verified
        } else {
            IntegrityCheckOutcome::Failed
        }
    }

    #[cfg(test)]
    pub(super) fn classify_test_value(value: &str) -> IntegrityCheckOutcome {
        classify_integrity_value(value)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) use native::inspect_database_integrity;

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(crate) mod test_seam {
    use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

    pub(crate) const PHASE_BEFORE_SQLITE: u8 = 1;
    pub(crate) const PHASE_BEFORE_FOREIGN_KEYS: u8 = 2;
    pub(crate) const PHASE_BEFORE_ROLLBACK: u8 = 3;
    pub(crate) const PHASE_SQLITE_EXECUTION_AWAITING: u8 = 4;
    pub(crate) const PHASE_CONNECTION_CLOSE_AWAITING: u8 = 5;

    static BLOCKED: AtomicU8 = AtomicU8::new(0);
    static REACHED: AtomicU8 = AtomicU8::new(0);
    static RELEASED: AtomicBool = AtomicBool::new(true);
    static REAL_SQLITE_PROBE: AtomicBool = AtomicBool::new(false);
    static CONNECTION_CLOSE_FAILURE: AtomicBool = AtomicBool::new(false);
    pub(crate) static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    pub(crate) async fn pause(phase: u8) {
        REACHED.store(phase, Ordering::Release);
        while BLOCKED.load(Ordering::Acquire) == phase && !RELEASED.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    }

    pub(crate) fn block(phase: u8) {
        REACHED.store(0, Ordering::Release);
        BLOCKED.store(phase, Ordering::Release);
        RELEASED.store(false, Ordering::Release);
    }

    pub(crate) fn observe(phase: u8) {
        REACHED.store(phase, Ordering::Release);
    }

    pub(crate) fn enable_real_sqlite_probe(enabled: bool) {
        REACHED.store(0, Ordering::Release);
        REAL_SQLITE_PROBE.store(enabled, Ordering::Release);
    }

    pub(crate) fn real_sqlite_probe_enabled() -> bool {
        REAL_SQLITE_PROBE.load(Ordering::Acquire)
    }

    pub(crate) fn inject_connection_close_failure(enabled: bool) {
        CONNECTION_CLOSE_FAILURE.store(enabled, Ordering::Release);
    }

    pub(crate) fn take_connection_close_failure() -> bool {
        CONNECTION_CLOSE_FAILURE.swap(false, Ordering::AcqRel)
    }

    pub(crate) fn reached() -> u8 {
        REACHED.load(Ordering::Acquire)
    }

    pub(crate) fn release() {
        RELEASED.store(true, Ordering::Release);
        BLOCKED.store(0, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use std::{
        fs::OpenOptions,
        io::{Seek, SeekFrom, Write},
    };

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

    #[test]
    fn timestamp_bounds_and_report_wire_vocabulary_are_exact() {
        assert!(IntegrityCheckedAtUnixMs::new(0).is_none());
        let maximum = IntegrityCheckedAtUnixMs::new(i64::MAX as u64).expect("maximum timestamp");
        assert_eq!(maximum.get(), i64::MAX as u64);
        assert!(IntegrityCheckedAtUnixMs::new(i64::MAX as u64 + 1).is_none());

        let checked_at = IntegrityCheckedAtUnixMs::new(1_700_000_000_000).unwrap();
        let verified = ServiceSqliteIntegrityReport::new(
            checked_at,
            IntegrityCheckOutcome::Verified,
            IntegrityCheckOutcome::Verified,
        );
        assert!(verified.diagnostics().is_empty());
        assert_eq!(verified.storage_integrity(), StorageIntegrity::Verified);
        assert_eq!(
            serde_json::to_string(&verified).unwrap(),
            r#"{"checked_at_unix_ms":1700000000000,"sqlite":"verified","foreign_keys":"verified","diagnostics":[]}"#
        );

        let failed = ServiceSqliteIntegrityReport::new(
            checked_at,
            IntegrityCheckOutcome::Failed,
            IntegrityCheckOutcome::Failed,
        );
        assert_eq!(
            failed.diagnostics(),
            [
                IntegrityDiagnosticCode::SqliteIntegrityFailed,
                IntegrityDiagnosticCode::ForeignKeyViolation,
            ]
        );
        assert_eq!(failed.storage_integrity(), StorageIntegrity::Failed);
        assert_eq!(
            serde_json::to_string(&failed).unwrap(),
            r#"{"checked_at_unix_ms":1700000000000,"sqlite":"failed","foreign_keys":"failed","diagnostics":["sqlite_integrity_failed","foreign_key_violation"]}"#
        );
        assert!(!format!("{failed:?}").contains("sqlite_schema"));
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        assert_eq!(
            native::classify_test_value(
                "a completed SQLite diagnostic that is intentionally much longer than sixty-four bytes"
            ),
            IntegrityCheckOutcome::Failed
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn in_memory_database() -> SqliteConnection {
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(":memory:"))
            .await
            .expect("in-memory database")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn native_inspection_reports_healthy_and_foreign_key_failure() {
        let _serial = test_seam::LOCK.lock().await;
        test_seam::release();
        let checked_at = IntegrityCheckedAtUnixMs::new(1).unwrap();
        let mut healthy = in_memory_database().await;
        let healthy = inspect_database_integrity(&mut healthy, checked_at, || Ok(()))
            .await
            .expect("healthy inspection");
        assert_eq!(healthy.sqlite(), IntegrityCheckOutcome::Verified);
        assert_eq!(healthy.foreign_keys(), IntegrityCheckOutcome::Verified);
        assert!(healthy.diagnostics().is_empty());

        let mut foreign_keys = in_memory_database().await;
        sqlx::raw_sql(
            "PRAGMA foreign_keys=OFF;
                 CREATE TABLE parent (id INTEGER PRIMARY KEY) STRICT;
                 CREATE TABLE child (parent_id INTEGER REFERENCES parent(id)) STRICT;
                 INSERT INTO child(parent_id) VALUES (99);",
        )
        .execute(&mut foreign_keys)
        .await
        .expect("seed foreign-key violation");
        let failed = inspect_database_integrity(&mut foreign_keys, checked_at, || Ok(()))
            .await
            .expect("completed foreign-key inspection");
        assert_eq!(failed.sqlite(), IntegrityCheckOutcome::Verified);
        assert_eq!(failed.foreign_keys(), IntegrityCheckOutcome::Failed);
        assert_eq!(
            failed.diagnostics(),
            [IntegrityDiagnosticCode::ForeignKeyViolation]
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn native_inspection_keeps_completed_failure_diagnostics_bounded() {
        let _serial = test_seam::LOCK.lock().await;
        test_seam::release();
        let mut corrupt = in_memory_database().await;
        sqlx::raw_sql(
            "PRAGMA foreign_keys=OFF;
                 CREATE TABLE parent (id INTEGER PRIMARY KEY) STRICT;
                 CREATE TABLE child (parent_id INTEGER REFERENCES parent(id)) STRICT;
                 CREATE INDEX parent_index ON parent(id);
                 INSERT INTO child(parent_id) VALUES (99);
                 PRAGMA writable_schema=ON;
                 UPDATE sqlite_schema SET rootpage=0 WHERE name='parent_index';
                 PRAGMA writable_schema=OFF;
                 PRAGMA schema_version=99;",
        )
        .execute(&mut corrupt)
        .await
        .expect("seed bounded corruption");
        let report = inspect_database_integrity(
            &mut corrupt,
            IntegrityCheckedAtUnixMs::new(2).unwrap(),
            || Ok(()),
        )
        .await
        .expect("completed corruption inspection");
        assert_eq!(report.sqlite(), IntegrityCheckOutcome::Failed);
        assert_eq!(report.foreign_keys(), IntegrityCheckOutcome::Failed);
        assert_eq!(report.diagnostics().len(), 2);
        let rendered = format!("{report:?}");
        assert!(!rendered.contains("parent"));
        assert!(!rendered.contains("rootpage"));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn physical_corruption_and_query_failure_remain_redacted_and_typed() {
        let _serial = test_seam::LOCK.lock().await;
        test_seam::release();
        let directory = tempfile::tempdir().expect("temporary database directory");
        let path = directory.path().join("sensitive-state-name.sqlite");
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let mut connection = SqliteConnection::connect_with(&options)
            .await
            .expect("create database");
        sqlx::query("CREATE TABLE integrity_probe (value BLOB NOT NULL) STRICT")
            .execute(&mut connection)
            .await
            .expect("create probe table");
        sqlx::query("INSERT INTO integrity_probe(value) VALUES (zeroblob(4096))")
            .execute(&mut connection)
            .await
            .expect("allocate probe page");
        let page_size = sqlx::query_scalar::<_, i64>("PRAGMA page_size")
            .fetch_one(&mut connection)
            .await
            .expect("page size");
        let root_page = sqlx::query_scalar::<_, i64>(
            "SELECT rootpage FROM sqlite_schema WHERE name='integrity_probe'",
        )
        .fetch_one(&mut connection)
        .await
        .expect("probe root page");
        connection.close().await.expect("close database");
        let offset = u64::try_from(root_page - 1)
            .ok()
            .and_then(|page| page.checked_mul(u64::try_from(page_size).ok()?))
            .expect("corrupt page offset");
        let mut file = OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("open database bytes");
        file.seek(SeekFrom::Start(offset)).expect("seek root page");
        file.write_all(&[0xff]).expect("corrupt page type");
        file.sync_all().expect("sync corrupt database");
        drop(file);

        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(false);
        let mut corrupt = SqliteConnection::connect_with(&options)
            .await
            .expect("open corrupt database shell");
        let report = inspect_database_integrity(
            &mut corrupt,
            IntegrityCheckedAtUnixMs::new(3).unwrap(),
            || Ok(()),
        )
        .await
        .expect("completed physical-corruption result");
        assert_eq!(report.sqlite(), IntegrityCheckOutcome::Failed);
        assert_eq!(
            report.diagnostics(),
            [IntegrityDiagnosticCode::SqliteIntegrityFailed]
        );
        let rendered = format!("{report:?}");
        assert!(!rendered.contains("sensitive-state-name"));
        assert!(!rendered.contains("integrity_probe"));
        assert!(!rendered.contains("database disk image"));

        let mut malformed = in_memory_database().await;
        sqlx::raw_sql(
            "CREATE TABLE secret_schema_name (value INTEGER) STRICT;
             PRAGMA writable_schema=ON;
             UPDATE sqlite_schema SET sql='CREATE TABLE secret_schema_name('
             WHERE name='secret_schema_name';
             PRAGMA writable_schema=OFF;
             PRAGMA schema_version=99;",
        )
        .execute(&mut malformed)
        .await
        .expect("seed malformed schema");
        let error = inspect_database_integrity(
            &mut malformed,
            IntegrityCheckedAtUnixMs::new(4).unwrap(),
            || Ok(()),
        )
        .await
        .expect_err("query failure is not a completed report");
        assert_eq!(error.kind(), crate::ServiceSqliteErrorKind::Integrity);
        let rendered = format!("{error:?} {}", error);
        assert!(!rendered.contains("secret_schema_name"));
        assert!(!rendered.contains("incomplete input"));
    }
}
