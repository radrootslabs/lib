use std::path::Path;
use std::sync::{Arc, Mutex};

use sqlx::Connection;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};

use crate::sqlx_sqlite_util;
use crate::{ExecOutcome, SqlExecutor, error::SqlError};

pub struct SqlxSqliteExecutor {
    conn: Arc<Mutex<SqliteConnection>>,
}

impl SqlxSqliteExecutor {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SqlError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        Self::connect(options)
    }

    pub fn open_memory() -> Result<Self, SqlError> {
        Self::connect(SqliteConnectOptions::new().in_memory(true))
    }

    fn connect(options: SqliteConnectOptions) -> Result<Self, SqlError> {
        let conn = futures_executor::block_on(SqliteConnection::connect_with(&options))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

impl SqlExecutor for SqlxSqliteExecutor {
    fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError> {
        let binds = sqlx_sqlite_util::parse_params(params_json)?;
        let mut conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        if binds.is_empty() {
            let result = futures_executor::block_on(
                sqlx::raw_sql(sqlx::AssertSqlSafe(sql)).execute(&mut *conn),
            )?;
            return Ok(ExecOutcome {
                changes: i64::try_from(result.rows_affected()).map_err(|_| SqlError::Internal)?,
                last_insert_id: result.last_insert_rowid(),
            });
        }
        let query = sqlx_sqlite_util::bind_params(sqlx::query(sqlx::AssertSqlSafe(sql)), binds)?;
        let result = futures_executor::block_on(query.execute(&mut *conn))?;
        Ok(ExecOutcome {
            changes: i64::try_from(result.rows_affected()).map_err(|_| SqlError::Internal)?,
            last_insert_id: result.last_insert_rowid(),
        })
    }

    fn query_raw(&self, sql: &str, params_json: &str) -> Result<String, SqlError> {
        let binds = sqlx_sqlite_util::parse_params(params_json)?;
        let query = sqlx_sqlite_util::bind_params(sqlx::query(sqlx::AssertSqlSafe(sql)), binds)?;
        let rows = {
            let mut conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
            futures_executor::block_on(query.fetch_all(&mut *conn))?
        };
        let rows = rows
            .iter()
            .map(sqlx_sqlite_util::row_to_json)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(serde_json::Value::from(rows).to_string())
    }

    fn begin(&self) -> Result<(), SqlError> {
        let mut conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        futures_executor::block_on(sqlx::query("BEGIN").execute(&mut *conn))?;
        Ok(())
    }

    fn commit(&self) -> Result<(), SqlError> {
        let mut conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        futures_executor::block_on(sqlx::query("COMMIT").execute(&mut *conn))?;
        Ok(())
    }

    fn rollback(&self) -> Result<(), SqlError> {
        let mut conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
        futures_executor::block_on(sqlx::query("ROLLBACK").execute(&mut *conn))?;
        Ok(())
    }
}
