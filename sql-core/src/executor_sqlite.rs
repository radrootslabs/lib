use crate::{ExecOutcome, SqlExecutor, error::SqlError};
use crate::sqlite_util;
use rusqlite::{Connection, params_from_iter};
use serde_json::Value;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct SqliteExecutor {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteExecutor {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SqlError> {
        let conn = Connection::open(path).map_err(SqlError::from)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_memory() -> Result<Self, SqlError> {
        let conn = Connection::open_in_memory().map_err(SqlError::from)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

}

impl SqlExecutor for SqliteExecutor {
    fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError> {
        let binds = sqlite_util::parse_params(params_json)?;
        let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        let n = conn
            .execute(sql, params_from_iter(binds.into_iter()))
            .map_err(SqlError::from)?;
        let last_insert_id = conn.last_insert_rowid();
        Ok(ExecOutcome {
            changes: n as i64,
            last_insert_id,
        })
    }

    fn query_raw(&self, sql: &str, params_json: &str) -> Result<String, SqlError> {
        let binds = sqlite_util::parse_params(params_json)?;
        let rows = {
            let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
            let mut stmt = conn.prepare(sql).map_err(SqlError::from)?;
            let mapped = stmt.query_map(params_from_iter(binds.into_iter()), |row| {
                sqlite_util::row_to_json(row)
            })?;
            let collected = mapped.collect::<Result<Vec<_>, _>>()?;
            collected
        };
        Ok(Value::from(rows).to_string())
    }

    fn begin(&self) -> Result<(), SqlError> {
        let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        conn.execute("BEGIN", []).map_err(SqlError::from)?;
        Ok(())
    }

    fn commit(&self) -> Result<(), SqlError> {
        let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        conn.execute("COMMIT", []).map_err(SqlError::from)?;
        Ok(())
    }

    fn rollback(&self) -> Result<(), SqlError> {
        let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        conn.execute("ROLLBACK", []).map_err(SqlError::from)?;
        Ok(())
    }
}
