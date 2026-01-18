#![forbid(unsafe_code)]

use std::sync::Mutex;

use radroots_sql_core::sqlite_util;
use radroots_sql_core::{ExecOutcome, SqlError, SqlExecutor};
use rusqlite::{Connection, DatabaseName, params_from_iter};
use serde_json::Value;

const SAVEPOINT_BEGIN: &str = "savepoint radroots_schema_tx";
const SAVEPOINT_RELEASE: &str = "release savepoint radroots_schema_tx";
const SAVEPOINT_ROLLBACK: &str = "rollback to savepoint radroots_schema_tx";

#[derive(Debug)]
pub struct EmbeddedSqlEngine {
    conn: Mutex<Connection>,
}

impl EmbeddedSqlEngine {
    pub fn new() -> Result<Self, SqlError> {
        let conn = Connection::open_in_memory().map_err(map_rusqlite)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError> {
        let binds = sqlite_util::parse_params(params_json)?;
        let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
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
            let mapped = stmt.query_map(params_from_iter(binds.into_iter()), sqlite_util::row_to_json)?;
            mapped
                .collect::<Result<Vec<_>, _>>()
                .map_err(map_rusqlite)?
        };
        Ok(rows)
    }

    pub fn query_raw(&self, sql: &str, params_json: &str) -> Result<String, SqlError> {
        let rows = self.query_rows(sql, params_json)?;
        serde_json::to_string(&rows).map_err(SqlError::from)
    }

    pub fn begin_tx(&self) -> Result<(), SqlError> {
        let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        conn.execute(SAVEPOINT_BEGIN, []).map_err(map_rusqlite)?;
        Ok(())
    }

    pub fn commit_tx(&self) -> Result<(), SqlError> {
        let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        conn.execute(SAVEPOINT_RELEASE, []).map_err(map_rusqlite)?;
        Ok(())
    }

    pub fn rollback_tx(&self) -> Result<(), SqlError> {
        let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        conn.execute(SAVEPOINT_ROLLBACK, []).map_err(map_rusqlite)?;
        conn.execute(SAVEPOINT_RELEASE, []).map_err(map_rusqlite)?;
        Ok(())
    }

    pub fn export_bytes(&self) -> Result<Vec<u8>, SqlError> {
        let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        let data = conn.serialize(DatabaseName::Main).map_err(map_rusqlite)?;
        Ok(data.to_vec())
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

#[cfg(all(test, feature = "embedded"))]
mod tests {
    use super::EmbeddedSqlEngine;
    use radroots_sql_core::SqlError;

    #[test]
    fn exec_query_roundtrip() -> Result<(), SqlError> {
        let engine = EmbeddedSqlEngine::new()?;
        engine.exec(
            "CREATE TABLE test_items (id INTEGER PRIMARY KEY, name TEXT)",
            "[]",
        )?;
        let outcome = engine.exec(
            "INSERT INTO test_items (name) VALUES (?)",
            "[\"rad\"]",
        )?;
        assert_eq!(outcome.changes, 1);
        let rows = engine.query_rows(
            "SELECT name FROM test_items WHERE id = ?",
            "[1]",
        )?;
        let name = rows
            .first()
            .and_then(|row| row.get("name"))
            .and_then(|value| value.as_str())
            .ok_or_else(|| SqlError::InvalidArgument("missing name".to_string()))?;
        assert_eq!(name, "rad");
        Ok(())
    }

    #[test]
    fn rollback_discards_changes() -> Result<(), SqlError> {
        let engine = EmbeddedSqlEngine::new()?;
        engine.exec(
            "CREATE TABLE test_items (id INTEGER PRIMARY KEY, name TEXT)",
            "[]",
        )?;
        engine.begin_tx()?;
        engine.exec(
            "INSERT INTO test_items (name) VALUES (?)",
            "[\"rad\"]",
        )?;
        engine.rollback_tx()?;
        let rows = engine.query_rows("SELECT name FROM test_items", "[]")?;
        assert!(rows.is_empty());
        Ok(())
    }

    #[test]
    fn export_bytes_non_empty() -> Result<(), SqlError> {
        let engine = EmbeddedSqlEngine::new()?;
        engine.exec(
            "CREATE TABLE test_items (id INTEGER PRIMARY KEY, name TEXT)",
            "[]",
        )?;
        engine.exec(
            "INSERT INTO test_items (name) VALUES (?)",
            "[\"rad\"]",
        )?;
        let bytes = engine.export_bytes()?;
        assert!(!bytes.is_empty());
        Ok(())
    }
}
