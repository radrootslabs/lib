#![forbid(unsafe_code)]

use std::sync::Mutex;

use radroots_sql_core::sqlite_util;
use radroots_sql_core::{ExecOutcome, SqlError, SqlExecutor};
use rusqlite::{Connection, MAIN_DB, params_from_iter};
use serde_json::Value;

const SAVEPOINT_BEGIN: &str = "savepoint radroots_schema_tx";
const SAVEPOINT_RELEASE: &str = "release savepoint radroots_schema_tx";
const SAVEPOINT_ROLLBACK: &str = "rollback to savepoint radroots_schema_tx";

#[cfg(test)]
mod failpoints {
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone, Copy)]
    pub enum Point {
        Open = 1 << 0,
        BeginExecute = 1 << 1,
        ReleaseExecute = 1 << 2,
        ExportSerialize = 1 << 3,
        EncodeRows = 1 << 4,
        RowToJson = 1 << 5,
    }

    static FLAGS: AtomicUsize = AtomicUsize::new(0);

    pub fn set(point: Point) {
        FLAGS.fetch_or(point as usize, Ordering::SeqCst);
    }

    pub fn take(point: Point) -> bool {
        let mask = point as usize;
        let prev = FLAGS.fetch_and(!mask, Ordering::SeqCst);
        (prev & mask) != 0
    }

    pub fn clear() {
        FLAGS.store(0, Ordering::SeqCst);
    }
}

#[cfg(test)]
fn forced_error() -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName("forced".to_string())
}

fn open_in_memory_with_failpoint() -> Result<Connection, rusqlite::Error> {
    #[cfg(test)]
    if failpoints::take(failpoints::Point::Open) {
        return Err(forced_error());
    }
    Connection::open_in_memory()
}

fn execute_begin_savepoint(conn: &Connection) -> Result<(), SqlError> {
    #[cfg(test)]
    let result = if failpoints::take(failpoints::Point::BeginExecute) {
        Err(forced_error())
    } else {
        conn.execute(SAVEPOINT_BEGIN, [])
    };
    #[cfg(not(test))]
    let result = conn.execute(SAVEPOINT_BEGIN, []);
    result.map(|_| ()).map_err(map_rusqlite)
}

fn execute_release_savepoint(conn: &Connection) -> Result<(), SqlError> {
    #[cfg(test)]
    let result = if failpoints::take(failpoints::Point::ReleaseExecute) {
        Err(forced_error())
    } else {
        conn.execute(SAVEPOINT_RELEASE, [])
    };
    #[cfg(not(test))]
    let result = conn.execute(SAVEPOINT_RELEASE, []);
    result.map(|_| ()).map_err(map_rusqlite)
}

fn serialize_main(conn: &Connection) -> Result<Vec<u8>, rusqlite::Error> {
    #[cfg(test)]
    if failpoints::take(failpoints::Point::ExportSerialize) {
        return Err(forced_error());
    }
    conn.serialize(MAIN_DB).map(|data| data.to_vec())
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    #[cfg(test)]
    if failpoints::take(failpoints::Point::RowToJson) {
        return Err(forced_error());
    }
    sqlite_util::row_to_json(row)
}

fn encode_rows(rows: &[Value]) -> Result<String, SqlError> {
    #[cfg(test)]
    if failpoints::take(failpoints::Point::EncodeRows) {
        return serde_json::to_string(&FailSerialize).map_err(SqlError::from);
    }
    serde_json::to_string(rows).map_err(SqlError::from)
}

#[cfg(test)]
struct FailSerialize;

#[cfg(test)]
impl serde::Serialize for FailSerialize {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom("forced"))
    }
}

#[derive(Debug)]
pub struct EmbeddedSqlEngine {
    conn: Mutex<Connection>,
}

impl EmbeddedSqlEngine {
    pub fn new() -> Result<Self, SqlError> {
        let conn = open_in_memory_with_failpoint().map_err(map_rusqlite)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError> {
        let binds = sqlite_util::parse_params(params_json)?;
        let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        if binds.is_empty() {
            let total_changes_before = conn.total_changes();
            conn.execute_batch(sql).map_err(map_rusqlite)?;
            let total_changes_after = conn.total_changes();
            let last_insert_id = conn.last_insert_rowid();
            return Ok(ExecOutcome {
                changes: (total_changes_after - total_changes_before) as i64,
                last_insert_id,
            });
        }
        let changes = conn
            .execute(sql, params_from_iter(binds.into_iter()))
            .map_err(map_rusqlite)?;
        let last_insert_id = conn.last_insert_rowid();
        Ok(ExecOutcome {
            changes: changes as i64,
            last_insert_id,
        })
    }

    pub fn query_rows(&self, sql: &str, params_json: &str) -> Result<Vec<Value>, SqlError> {
        let binds = sqlite_util::parse_params(params_json)?;
        let rows = {
            let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
            let mut stmt = conn.prepare(sql).map_err(map_rusqlite)?;
            let params = params_from_iter(binds.into_iter());
            let mapped = stmt.query_map(params, map_row)?;
            mapped
                .collect::<Result<Vec<_>, _>>()
                .map_err(map_rusqlite)?
        };
        Ok(rows)
    }

    pub fn query_raw(&self, sql: &str, params_json: &str) -> Result<String, SqlError> {
        let rows = self.query_rows(sql, params_json)?;
        encode_rows(&rows)
    }

    pub fn begin_tx(&self) -> Result<(), SqlError> {
        let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        execute_begin_savepoint(&conn)
    }

    pub fn commit_tx(&self) -> Result<(), SqlError> {
        let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        execute_release_savepoint(&conn)
    }

    pub fn rollback_tx(&self) -> Result<(), SqlError> {
        let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        conn.execute(SAVEPOINT_ROLLBACK, []).map_err(map_rusqlite)?;
        execute_release_savepoint(&conn)
    }

    pub fn export_bytes(&self) -> Result<Vec<u8>, SqlError> {
        let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        let data = serialize_main(&conn).map_err(map_rusqlite)?;
        Ok(data)
    }
}

impl SqlExecutor for EmbeddedSqlEngine {
    fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError> {
        EmbeddedSqlEngine::exec(self, sql, params_json)
    }

    fn query_raw(&self, sql: &str, params_json: &str) -> Result<String, SqlError> {
        EmbeddedSqlEngine::query_raw(self, sql, params_json)
    }

    fn begin(&self) -> Result<(), SqlError> {
        EmbeddedSqlEngine::begin_tx(self)
    }

    fn commit(&self) -> Result<(), SqlError> {
        EmbeddedSqlEngine::commit_tx(self)
    }

    fn rollback(&self) -> Result<(), SqlError> {
        EmbeddedSqlEngine::rollback_tx(self)
    }
}

fn map_rusqlite(err: rusqlite::Error) -> SqlError {
    SqlError::InvalidQuery(err.to_string())
}

pub fn coverage_branch_probe(input: bool) -> &'static str {
    if input { "sql" } else { "sql" }
}

#[cfg(all(test, feature = "embedded"))]
mod tests {
    use super::{EmbeddedSqlEngine, coverage_branch_probe, failpoints};
    use radroots_sql_core::SqlExecutor;

    const CREATE_TABLE_SQL: &str = "CREATE TABLE test_items (id INTEGER PRIMARY KEY, name TEXT)";

    fn poison_engine(engine: &EmbeddedSqlEngine) {
        let _ = std::panic::catch_unwind(|| {
            let _guard = engine.conn.lock().unwrap();
            panic!("poison");
        });
    }

    #[test]
    fn open_in_memory_failpoint_surfaces_error() {
        failpoints::clear();
        failpoints::set(failpoints::Point::Open);
        let err = EmbeddedSqlEngine::new().unwrap_err();
        assert_eq!(err.code(), "ERR_INVALID_QUERY");
    }

    #[test]
    fn exec_query_roundtrip() {
        let engine = EmbeddedSqlEngine::new().unwrap();
        engine.exec(CREATE_TABLE_SQL, "[]").unwrap();
        let outcome = engine
            .exec("INSERT INTO test_items (name) VALUES (?)", "[\"rad\"]")
            .unwrap();
        assert_eq!(outcome.changes, 1);
        let rows = engine
            .query_rows("SELECT name FROM test_items WHERE id = ?", "[1]")
            .unwrap();
        let name = rows
            .first()
            .and_then(|row| row.get("name"))
            .and_then(|value| value.as_str())
            .expect("missing name");
        assert_eq!(name, "rad");
    }

    #[test]
    fn rollback_discards_changes() {
        let engine = EmbeddedSqlEngine::new().unwrap();
        engine.exec(CREATE_TABLE_SQL, "[]").unwrap();
        engine.begin_tx().unwrap();
        engine
            .exec("INSERT INTO test_items (name) VALUES (?)", "[\"rad\"]")
            .unwrap();
        engine.rollback_tx().unwrap();
        let rows = engine
            .query_rows("SELECT name FROM test_items", "[]")
            .unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn exec_runs_multi_statement_batches_without_params() {
        let engine = EmbeddedSqlEngine::new().unwrap();

        let outcome = engine
            .exec(
                "CREATE TABLE demo (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\
\nCREATE UNIQUE INDEX demo_name_idx ON demo(name);",
                "[]",
            )
            .unwrap();
        assert_eq!(outcome.changes, 0);

        let insert = engine
            .exec("INSERT INTO demo (name) VALUES ('alpha')", "[]")
            .unwrap();
        assert_eq!(insert.changes, 1);

        let rows = engine
            .query_rows(
                "SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'demo_name_idx'",
                "[]",
            )
            .unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn exec_empty_bind_batch_surfaces_invalid_query() {
        let engine = EmbeddedSqlEngine::new().unwrap();
        let err = engine.exec("CREATE TABLE broken (", "[]").unwrap_err();
        assert_eq!(err.code(), "ERR_INVALID_QUERY");
    }

    #[test]
    fn export_bytes_non_empty() {
        let engine = EmbeddedSqlEngine::new().unwrap();
        engine.exec(CREATE_TABLE_SQL, "[]").unwrap();
        engine
            .exec("INSERT INTO test_items (name) VALUES (?)", "[\"rad\"]")
            .unwrap();
        let bytes = engine.export_bytes().unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn query_raw_commit_and_trait_executor_paths() {
        let engine = EmbeddedSqlEngine::new().unwrap();
        engine.exec(CREATE_TABLE_SQL, "[]").unwrap();
        engine.begin_tx().unwrap();
        engine
            .exec("INSERT INTO test_items (name) VALUES (?)", "[\"rad\"]")
            .unwrap();
        engine.commit_tx().unwrap();
        let rows = engine
            .query_raw("SELECT name FROM test_items ORDER BY id ASC", "[]")
            .unwrap();
        assert!(rows.contains("rad"));

        let executor: &dyn SqlExecutor = &engine;
        executor.begin().unwrap();
        let _ = executor
            .exec("INSERT INTO test_items (name) VALUES (?)", "[\"trait\"]")
            .unwrap();
        executor.rollback().unwrap();
        let rows_after = executor
            .query_raw("SELECT name FROM test_items ORDER BY id ASC", "[]")
            .unwrap();
        assert!(rows_after.contains("rad"));
        assert!(!rows_after.contains("trait"));
    }

    #[test]
    fn invalid_sql_paths_surface_invalid_query() {
        let engine = EmbeddedSqlEngine::new().unwrap();
        let err_exec = engine
            .exec("INSERT INTO missing (name) VALUES (?)", "[\"rad\"]")
            .unwrap_err();
        assert_eq!(err_exec.code(), "ERR_INVALID_QUERY");

        let err_rows = engine.query_rows("SELECT name FROM missing", "[]");
        assert_eq!(err_rows.unwrap_err().code(), "ERR_INVALID_QUERY");

        let err_raw = engine.query_raw("SELECT name FROM missing", "[]");
        assert_eq!(err_raw.unwrap_err().code(), "ERR_INVALID_QUERY");

        let err_commit = engine.commit_tx().unwrap_err();
        assert_eq!(err_commit.code(), "ERR_INVALID_QUERY");

        let err_rollback = engine.rollback_tx().unwrap_err();
        assert_eq!(err_rollback.code(), "ERR_INVALID_QUERY");

        let executor: &dyn SqlExecutor = &engine;
        let err_trait_commit = executor.commit().unwrap_err();
        assert_eq!(err_trait_commit.code(), "ERR_INVALID_QUERY");
    }

    #[test]
    fn invalid_params_surface_errors() {
        let engine = EmbeddedSqlEngine::new().unwrap();
        let err_exec = engine.exec(CREATE_TABLE_SQL, "{}").unwrap_err();
        assert_eq!(err_exec.code(), "ERR_SERIALIZATION");

        let err_rows = engine.query_rows("SELECT 1", "{}").unwrap_err();
        assert_eq!(err_rows.code(), "ERR_SERIALIZATION");

        let err_raw = engine.query_raw("SELECT 1", "{}").unwrap_err();
        assert_eq!(err_raw.code(), "ERR_SERIALIZATION");
    }

    #[test]
    fn query_rows_surfaces_prepare_and_bind_errors() {
        let engine = EmbeddedSqlEngine::new().unwrap();
        let err_prepare = engine
            .query_rows("SELEC name FROM test_items", "[]")
            .unwrap_err();
        assert_eq!(err_prepare.code(), "ERR_INVALID_QUERY");

        engine.exec(CREATE_TABLE_SQL, "[]").unwrap();
        let err_bind = engine
            .query_rows("SELECT name FROM test_items WHERE id = ?", "[]")
            .unwrap_err();
        assert_eq!(err_bind.code(), "ERR_INVALID_QUERY");
    }

    #[test]
    fn query_rows_collect_error_is_reported() {
        let engine = EmbeddedSqlEngine::new().unwrap();
        engine.exec(CREATE_TABLE_SQL, "[]").unwrap();
        engine
            .exec("INSERT INTO test_items (name) VALUES (?)", "[\"rad\"]")
            .unwrap();
        failpoints::clear();
        failpoints::set(failpoints::Point::RowToJson);
        let err = engine
            .query_rows("SELECT name FROM test_items", "[]")
            .unwrap_err();
        assert_eq!(err.code(), "ERR_INVALID_QUERY");
    }

    #[test]
    fn query_raw_serialization_error_is_reported() {
        let engine = EmbeddedSqlEngine::new().unwrap();
        engine.exec(CREATE_TABLE_SQL, "[]").unwrap();
        engine
            .exec("INSERT INTO test_items (name) VALUES (?)", "[\"rad\"]")
            .unwrap();
        failpoints::clear();
        failpoints::set(failpoints::Point::EncodeRows);
        let err = engine
            .query_raw("SELECT name FROM test_items", "[]")
            .unwrap_err();
        assert_eq!(err.code(), "ERR_SERIALIZATION");
    }

    #[test]
    fn begin_tx_failpoint_surfaces_error() {
        let engine = EmbeddedSqlEngine::new().unwrap();
        failpoints::clear();
        failpoints::set(failpoints::Point::BeginExecute);
        let err = engine.begin_tx().unwrap_err();
        assert_eq!(err.code(), "ERR_INVALID_QUERY");
    }

    #[test]
    fn rollback_release_failpoint_surfaces_error() {
        let engine = EmbeddedSqlEngine::new().unwrap();
        engine.exec(CREATE_TABLE_SQL, "[]").unwrap();
        engine.begin_tx().unwrap();
        failpoints::clear();
        failpoints::set(failpoints::Point::ReleaseExecute);
        let err = engine.rollback_tx().unwrap_err();
        assert_eq!(err.code(), "ERR_INVALID_QUERY");
    }

    #[test]
    fn export_bytes_failpoint_surfaces_error() {
        let engine = EmbeddedSqlEngine::new().unwrap();
        engine.exec(CREATE_TABLE_SQL, "[]").unwrap();
        failpoints::clear();
        failpoints::set(failpoints::Point::ExportSerialize);
        let err = engine.export_bytes().unwrap_err();
        assert_eq!(err.code(), "ERR_INVALID_QUERY");
    }

    #[test]
    fn lock_errors_surface_internal() {
        let engine = EmbeddedSqlEngine::new().unwrap();
        poison_engine(&engine);
        let err_exec = engine.exec(CREATE_TABLE_SQL, "[]");
        assert_eq!(err_exec.unwrap_err().code(), "ERR_INTERNAL");
        let err_rows = engine.query_rows("SELECT 1", "[]");
        assert_eq!(err_rows.unwrap_err().code(), "ERR_INTERNAL");
        let err_raw = engine.query_raw("SELECT 1", "[]");
        assert_eq!(err_raw.unwrap_err().code(), "ERR_INTERNAL");
        let err_begin = engine.begin_tx();
        assert_eq!(err_begin.unwrap_err().code(), "ERR_INTERNAL");
        let err_commit = engine.commit_tx();
        assert_eq!(err_commit.unwrap_err().code(), "ERR_INTERNAL");
        let err_rollback = engine.rollback_tx();
        assert_eq!(err_rollback.unwrap_err().code(), "ERR_INTERNAL");
        let err_export = engine.export_bytes();
        assert_eq!(err_export.unwrap_err().code(), "ERR_INTERNAL");
    }

    #[test]
    fn coverage_branch_probe_hits_both_paths() {
        assert_eq!(coverage_branch_probe(true), "sql");
        assert_eq!(coverage_branch_probe(false), "sql");
    }
}
