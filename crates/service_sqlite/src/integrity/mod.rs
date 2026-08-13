//! Exact schema-object catalog verification.

pub(crate) mod catalog;
mod inspection;

pub use catalog::{
    SchemaCatalog, SchemaCatalogContractError, SchemaDigest, SchemaObject, SchemaObjectKind,
    SchemaVersionCatalog,
};
pub use inspection::{
    IntegrityCheckOutcome, IntegrityCheckedAtUnixMs, IntegrityDiagnosticCode,
    ServiceSqliteIntegrityReport,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) use inspection::inspect_database_integrity;

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(crate) use inspection::test_seam as integrity_test_seam;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use core::fmt;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::error::Error;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use sqlx::{Row, SqliteConnection};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::{ServiceSqliteError, ServiceSqliteErrorKind};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use self::catalog::{
    MAX_SCHEMA_CATALOG_UTF8_BYTES, MAX_SCHEMA_OBJECT_COUNT, MAX_SCHEMA_SQL_UTF8_BYTES, ObjectRef,
    object_digest, snapshot_digest,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
const READ_SCHEMA_CATALOG_SQL: &str = r#"
WITH raw_catalog AS (
    SELECT type, name, tbl_name, sql
    FROM main.sqlite_schema
    WHERE NOT (typeof(name) = 'text' AND substr(name, 1, 7) = 'sqlite_')
), bounded_catalog AS (
    SELECT
        type,
        name,
        tbl_name,
        sql,
        COUNT(*) OVER () AS object_count,
        COALESCE(SUM(length(CAST(sql AS BLOB))) OVER (), 0) AS total_sql_bytes
    FROM raw_catalog
)
SELECT
    object_count,
    total_sql_bytes,
    CASE
        WHEN object_count <= 4096
         AND typeof(type) = 'text'
         AND length(CAST(type AS BLOB)) BETWEEN 1 AND 16
        THEN type
    END AS bounded_type,
    CASE
        WHEN object_count <= 4096
         AND typeof(name) = 'text'
         AND length(CAST(name AS BLOB)) BETWEEN 1 AND 128
        THEN name
    END AS bounded_name,
    CASE
        WHEN object_count <= 4096
         AND typeof(tbl_name) = 'text'
         AND length(CAST(tbl_name AS BLOB)) BETWEEN 1 AND 128
        THEN tbl_name
    END AS bounded_table_name,
    CASE
        WHEN object_count <= 4096
         AND total_sql_bytes <= 16777216
         AND typeof(sql) = 'text'
         AND length(CAST(sql AS BLOB)) BETWEEN 1 AND 1048576
        THEN sql
    END AS bounded_sql
FROM bounded_catalog
LIMIT 4097
"#;

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SchemaVerificationReport {
    version: u32,
    expected_count: u32,
    actual_count: u32,
    expected_digest: SchemaDigest,
    actual_digest: SchemaDigest,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SchemaIntegrityFailureKind {
    CatalogMismatch,
    CatalogCorrupt,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct SchemaIntegrityFailure {
    kind: SchemaIntegrityFailureKind,
    report: Option<SchemaVerificationReport>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Debug for SchemaIntegrityFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SchemaIntegrityFailure")
            .field("kind", &self.kind)
            .field("report", &self.report)
            .finish()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Display for SchemaIntegrityFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            SchemaIntegrityFailureKind::CatalogMismatch => {
                "SQLite schema object catalog does not match"
            }
            SchemaIntegrityFailureKind::CatalogCorrupt => "SQLite schema object catalog is invalid",
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Error for SchemaIntegrityFailure {}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn integrity_error(kind: SchemaIntegrityFailureKind) -> ServiceSqliteError {
    ServiceSqliteError::with_source(
        ServiceSqliteErrorKind::Integrity,
        SchemaIntegrityFailure { kind, report: None },
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn mismatch_error(report: SchemaVerificationReport) -> ServiceSqliteError {
    ServiceSqliteError::with_source(
        ServiceSqliteErrorKind::Integrity,
        SchemaIntegrityFailure {
            kind: SchemaIntegrityFailureKind::CatalogMismatch,
            report: Some(report),
        },
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct RuntimeSchemaObject {
    kind: SchemaObjectKind,
    name: String,
    table_name: String,
    sql: String,
    digest: SchemaDigest,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) async fn verify_schema_catalog(
    connection: &mut SqliteConnection,
    catalog: &SchemaCatalog,
    version: u32,
) -> Result<SchemaVerificationReport, ServiceSqliteError> {
    let expected = catalog
        .version(version)
        .ok_or_else(|| integrity_error(SchemaIntegrityFailureKind::CatalogMismatch))?;
    let rows = sqlx::query(READ_SCHEMA_CATALOG_SQL)
        .fetch_all(connection)
        .await
        .map_err(|_| integrity_error(SchemaIntegrityFailureKind::CatalogCorrupt))?;
    require_schema_row_limit(rows.len())?;
    let mut objects = Vec::with_capacity(rows.len());
    let mut reported_count = None;
    let mut reported_total = None;
    for row in rows {
        let count = row
            .try_get::<i64, _>("object_count")
            .ok()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| integrity_error(SchemaIntegrityFailureKind::CatalogCorrupt))?;
        let total = row
            .try_get::<i64, _>("total_sql_bytes")
            .ok()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| integrity_error(SchemaIntegrityFailureKind::CatalogCorrupt))?;
        require_catalog_summary_projection([
            count <= u32::try_from(MAX_SCHEMA_OBJECT_COUNT).unwrap_or(u32::MAX),
            total <= MAX_SCHEMA_CATALOG_UTF8_BYTES,
            reported_count.is_none_or(|observed| observed == count),
            reported_total.is_none_or(|observed| observed == total),
        ])?;
        reported_count = Some(count);
        reported_total = Some(total);
        let object_type = row
            .try_get::<String, _>("bounded_type")
            .map_err(|_| integrity_error(SchemaIntegrityFailureKind::CatalogCorrupt))?;
        let name = row
            .try_get::<String, _>("bounded_name")
            .map_err(|_| integrity_error(SchemaIntegrityFailureKind::CatalogCorrupt))?;
        let table_name = row
            .try_get::<String, _>("bounded_table_name")
            .map_err(|_| integrity_error(SchemaIntegrityFailureKind::CatalogCorrupt))?;
        let sql = row
            .try_get::<String, _>("bounded_sql")
            .map_err(|_| integrity_error(SchemaIntegrityFailureKind::CatalogCorrupt))?;
        require_schema_sql_limit(sql.len())?;
        let kind = SchemaObjectKind::from_sqlite(&object_type)
            .ok_or_else(|| integrity_error(SchemaIntegrityFailureKind::CatalogCorrupt))?;
        require_runtime_object_projection(kind, &name, &table_name)?;
        let digest = object_digest(kind, &name, &table_name, &sql);
        objects.push(RuntimeSchemaObject {
            kind,
            name,
            table_name,
            sql,
            digest,
        });
    }
    let actual_count = u32::try_from(objects.len())
        .map_err(|_| integrity_error(SchemaIntegrityFailureKind::CatalogCorrupt))?;
    require_reported_object_count(reported_count, actual_count)?;
    let mut identities = std::collections::BTreeSet::new();
    require_unique_runtime_objects(
        !objects
            .iter()
            .any(|object| !identities.insert((object.kind, object.name.as_str()))),
    )?;
    let refs = objects
        .iter()
        .map(|object| ObjectRef {
            kind: object.kind,
            name: &object.name,
            table_name: &object.table_name,
            sql: &object.sql,
            digest: object.digest,
        })
        .collect::<Vec<_>>();
    let actual_digest = snapshot_digest(version, &refs);
    let report = SchemaVerificationReport {
        version,
        expected_count: expected.object_count(),
        actual_count,
        expected_digest: expected.digest(),
        actual_digest,
    };
    require_schema_report_projection(report)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_catalog_summary_projection(matches: [bool; 4]) -> Result<(), ServiceSqliteError> {
    crate::all_constraints(matches)
        .then_some(())
        .ok_or_else(|| integrity_error(SchemaIntegrityFailureKind::CatalogCorrupt))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_runtime_object_projection(
    kind: SchemaObjectKind,
    name: &str,
    table_name: &str,
) -> Result<(), ServiceSqliteError> {
    crate::all_constraints([
        runtime_name_is_valid(name),
        runtime_name_is_valid(table_name),
        (kind == SchemaObjectKind::Table) == (name == table_name),
    ])
    .then_some(())
    .ok_or_else(|| integrity_error(SchemaIntegrityFailureKind::CatalogCorrupt))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_schema_report_projection(
    report: SchemaVerificationReport,
) -> Result<SchemaVerificationReport, ServiceSqliteError> {
    let matches = crate::all_constraints([
        report.expected_count == report.actual_count,
        report.expected_digest == report.actual_digest,
    ]);
    if matches {
        Ok(report)
    } else {
        Err(mismatch_error(report))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) async fn verify_database_integrity(
    connection: &mut SqliteConnection,
) -> Result<(), ServiceSqliteError> {
    let rows = sqlx::query("PRAGMA integrity_check(1)")
        .fetch_all(&mut *connection)
        .await
        .map_err(|_| integrity_error(SchemaIntegrityFailureKind::CatalogCorrupt))?;
    let value = rows.first().and_then(|row| row.try_get::<&str, _>(0).ok());
    catalog_corrupt_unless(integrity_projection_matches(rows.len(), value))?;
    let foreign_key_violation =
        sqlx::query_scalar::<_, i64>("SELECT 1 FROM pragma_foreign_key_check LIMIT 1")
            .fetch_optional(connection)
            .await
            .map_err(|_| integrity_error(SchemaIntegrityFailureKind::CatalogCorrupt))?;
    require_no_foreign_key_violation(foreign_key_violation.is_some())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn catalog_corrupt_unless(condition: bool) -> Result<(), ServiceSqliteError> {
    condition
        .then_some(())
        .ok_or_else(|| integrity_error(SchemaIntegrityFailureKind::CatalogCorrupt))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_schema_row_limit(row_count: usize) -> Result<(), ServiceSqliteError> {
    catalog_corrupt_unless(row_count <= MAX_SCHEMA_OBJECT_COUNT)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_schema_sql_limit(sql_bytes: usize) -> Result<(), ServiceSqliteError> {
    catalog_corrupt_unless(sql_bytes <= MAX_SCHEMA_SQL_UTF8_BYTES)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_reported_object_count(
    reported: Option<u32>,
    actual: u32,
) -> Result<(), ServiceSqliteError> {
    catalog_corrupt_unless(reported.unwrap_or(0) == actual)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_unique_runtime_objects(unique: bool) -> Result<(), ServiceSqliteError> {
    catalog_corrupt_unless(unique)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_no_foreign_key_violation(present: bool) -> Result<(), ServiceSqliteError> {
    catalog_corrupt_unless(!present)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn integrity_projection_matches(row_count: usize, value: Option<&str>) -> bool {
    let present = value.is_some();
    let bounded = value.is_some_and(|value| value.len() <= 64);
    let exact = value == Some("ok");
    crate::all_constraints([row_count == 1, present, bounded, exact])
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn runtime_name_is_valid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    crate::all_constraints([
        bytes.len() <= 128,
        bytes[0].is_ascii_lowercase(),
        bytes[bytes.len() - 1].is_ascii_alphanumeric(),
        !bytes.windows(2).any(|pair| pair == b"__"),
        bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_'),
    ])
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {

    use super::*;
    use sqlx::{Connection, Executor, sqlite::SqliteConnectOptions};

    const TABLE_SQL: &str =
        "CREATE TABLE alpha (id INTEGER PRIMARY KEY, value INTEGER NOT NULL) STRICT";
    const INDEX_SQL: &str = "CREATE INDEX alpha_value_idx ON alpha(value)";
    const TRIGGER_SQL: &str = "CREATE TRIGGER alpha_guard BEFORE UPDATE ON alpha BEGIN SELECT RAISE(ABORT, 'blocked'); END";

    #[test]
    fn schema_integrity_failure_inventory_is_complete_and_source_free() {
        for (kind, message) in [
            (
                SchemaIntegrityFailureKind::CatalogMismatch,
                "SQLite schema object catalog does not match",
            ),
            (
                SchemaIntegrityFailureKind::CatalogCorrupt,
                "SQLite schema object catalog is invalid",
            ),
        ] {
            let failure = SchemaIntegrityFailure { kind, report: None };
            assert_eq!(failure.to_string(), message);
            assert!(failure.source().is_none());
            assert!(format!("{failure:?}").contains(&format!("{kind:?}")));
            let error = integrity_error(kind);
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Integrity);
            assert!(error.source().is_some());
        }
    }

    #[test]
    fn database_integrity_projection_rejects_each_independent_drift() {
        assert!(integrity_projection_matches(1, Some("ok")));
        assert!(!integrity_projection_matches(0, Some("ok")));
        assert!(!integrity_projection_matches(2, Some("ok")));
        assert!(!integrity_projection_matches(1, None));
        assert!(!integrity_projection_matches(1, Some("not-ok")));
        let oversized = "x".repeat(65);
        assert!(!integrity_projection_matches(1, Some(&oversized)));
    }

    #[test]
    fn catalog_projection_helpers_reject_every_independent_drift() {
        assert!(require_catalog_summary_projection([true; 4]).is_ok());
        for changed in 0..4 {
            let mut matches = [true; 4];
            matches[changed] = false;
            assert!(require_catalog_summary_projection(matches).is_err());
        }

        assert!(
            require_runtime_object_projection(SchemaObjectKind::Table, "alpha", "alpha").is_ok()
        );
        assert!(
            require_runtime_object_projection(SchemaObjectKind::Index, "alpha_idx", "alpha")
                .is_ok()
        );
        for (kind, name, table) in [
            (SchemaObjectKind::Table, "Bad", "Bad"),
            (SchemaObjectKind::Table, "alpha", "Bad"),
            (SchemaObjectKind::Table, "alpha", "other"),
            (SchemaObjectKind::Index, "alpha", "alpha"),
        ] {
            assert!(require_runtime_object_projection(kind, name, table).is_err());
        }

        let digest = SchemaDigest::from_bytes([7; 32]);
        let mut report = SchemaVerificationReport {
            version: 1,
            expected_count: 1,
            actual_count: 1,
            expected_digest: digest,
            actual_digest: digest,
        };
        assert!(require_schema_report_projection(report.clone()).is_ok());
        report.actual_count = 2;
        assert!(require_schema_report_projection(report.clone()).is_err());
        report.actual_count = 1;
        report.actual_digest = SchemaDigest::from_bytes([8; 32]);
        assert!(require_schema_report_projection(report).is_err());

        assert!(catalog_corrupt_unless(true).is_ok());
        assert!(catalog_corrupt_unless(false).is_err());
        assert!(require_schema_row_limit(MAX_SCHEMA_OBJECT_COUNT).is_ok());
        assert!(require_schema_row_limit(MAX_SCHEMA_OBJECT_COUNT + 1).is_err());
        assert!(require_schema_sql_limit(MAX_SCHEMA_SQL_UTF8_BYTES).is_ok());
        assert!(require_schema_sql_limit(MAX_SCHEMA_SQL_UTF8_BYTES + 1).is_err());
        assert!(require_reported_object_count(Some(1), 1).is_ok());
        assert!(require_reported_object_count(None, 1).is_err());
        assert!(require_reported_object_count(Some(2), 1).is_err());
        assert!(require_unique_runtime_objects(true).is_ok());
        assert!(require_unique_runtime_objects(false).is_err());
        assert!(require_no_foreign_key_violation(false).is_ok());
        assert!(require_no_foreign_key_violation(true).is_err());
    }

    fn object(
        kind: SchemaObjectKind,
        name: &'static str,
        table_name: &'static str,
        sql: &'static str,
    ) -> SchemaObject {
        SchemaObject::new(
            kind,
            name,
            table_name,
            sql,
            SchemaObject::computed_digest(kind, name, table_name, sql).unwrap(),
        )
        .unwrap()
    }

    fn expected(objects: Vec<SchemaObject>) -> SchemaCatalog {
        let migrations = crate::MigrationCatalog::new([]).unwrap();
        let digest = SchemaVersionCatalog::computed_digest(1, objects.iter().cloned()).unwrap();
        let version = SchemaVersionCatalog::new(1, objects, digest).unwrap();
        SchemaCatalog::new(&migrations, [version]).unwrap()
    }

    fn full_objects() -> Vec<SchemaObject> {
        vec![
            object(
                SchemaObjectKind::Trigger,
                "alpha_guard",
                "alpha",
                TRIGGER_SQL,
            ),
            object(
                SchemaObjectKind::Index,
                "alpha_value_idx",
                "alpha",
                INDEX_SQL,
            ),
            object(SchemaObjectKind::Table, "alpha", "alpha", TABLE_SQL),
        ]
    }

    async fn shared_database() -> SqliteConnection {
        let mut connection =
            SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(":memory:"))
                .await
                .unwrap();
        for statement in catalog::METADATA_SCHEMA_SQL
            .into_iter()
            .chain(catalog::MIGRATION_LEDGER_SCHEMA_SQL)
        {
            connection.execute(statement).await.unwrap();
        }
        connection
    }

    async fn full_database() -> SqliteConnection {
        let mut connection = shared_database().await;
        for statement in [TABLE_SQL, INDEX_SQL, TRIGGER_SQL] {
            connection.execute(statement).await.unwrap();
        }
        connection
    }

    #[tokio::test(flavor = "current_thread")]
    async fn exact_catalog_is_order_independent_and_report_is_bounded() {
        let mut connection = full_database().await;
        let catalog = expected(full_objects());
        let first = verify_schema_catalog(&mut connection, &catalog, 1)
            .await
            .unwrap();
        let second = verify_schema_catalog(&mut connection, &catalog, 1)
            .await
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.version, 1);
        assert_eq!(first.expected_count, 9);
        assert_eq!(first.actual_count, 9);
        assert_eq!(first.expected_digest, first.actual_digest);
        assert!(!format!("{first:?}").contains(TABLE_SQL));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn missing_extra_replaced_index_trigger_column_and_view_fail_closed() {
        let cases = [
            "DROP INDEX alpha_value_idx",
            "CREATE TABLE extra (value INTEGER)",
            "DROP INDEX alpha_value_idx; CREATE INDEX alpha_value_idx ON alpha(value DESC)",
            "DROP TRIGGER alpha_guard; CREATE TRIGGER alpha_guard BEFORE UPDATE ON alpha BEGIN SELECT RAISE(ABORT, 'changed'); END",
            "ALTER TABLE alpha ADD COLUMN changed TEXT",
            "CREATE VIEW alpha_view AS SELECT value FROM alpha",
        ];
        for mutation in cases {
            let mut connection = full_database().await;
            sqlx::raw_sql(mutation)
                .execute(&mut connection)
                .await
                .unwrap();
            let error = verify_schema_catalog(&mut connection, &expected(full_objects()), 1)
                .await
                .expect_err("schema drift must fail");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Integrity);
            assert!(!error.to_string().contains("alpha"));
            assert!(!format!("{error:?}").contains("alpha"));
        }

        let mut missing = shared_database().await;
        assert_eq!(
            verify_schema_catalog(&mut missing, &expected(full_objects()), 1)
                .await
                .expect_err("missing table")
                .kind(),
            ServiceSqliteErrorKind::Integrity
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn oversized_persisted_sql_is_rejected_before_rust_decode() {
        let mut connection = shared_database().await;
        let oversized = "x".repeat(catalog::MAX_SCHEMA_SQL_UTF8_BYTES + 1);
        let statement = format!("CREATE TABLE oversized (value TEXT DEFAULT '{oversized}')");
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .execute(&mut connection)
            .await
            .unwrap();
        let error = verify_schema_catalog(&mut connection, &expected(Vec::new()), 1)
            .await
            .expect_err("oversized SQL must fail");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Integrity);
        assert!(!error.to_string().contains(&oversized));
        assert!(!format!("{error:?}").contains(&oversized));
    }

    #[test]
    fn runtime_names_bind_every_grammar_constraint() {
        assert!(runtime_name_is_valid("a"));
        assert!(runtime_name_is_valid("alpha_2"));
        assert!(!runtime_name_is_valid(""));
        assert!(!runtime_name_is_valid("Alpha"));
        assert!(!runtime_name_is_valid("2alpha"));
        assert!(!runtime_name_is_valid("alpha_"));
        assert!(!runtime_name_is_valid("alpha__beta"));
        assert!(!runtime_name_is_valid("alpha-beta"));
        assert!(runtime_name_is_valid(&"a".repeat(128)));
        assert!(!runtime_name_is_valid(&"a".repeat(129)));
    }
}
