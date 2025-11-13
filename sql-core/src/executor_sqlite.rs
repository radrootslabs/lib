use crate::{ExecOutcome, SqlExecutor, error::SqlError};
use rusqlite::{Connection, Row, params_from_iter};
use serde_json::{Map, Value};
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

    fn parse_params(&self, params_json: &str) -> Result<Vec<rusqlite::types::Value>, SqlError> {
        let vals: Vec<Value> = serde_json::from_str(params_json)
            .map_err(|e| SqlError::SerializationError(e.to_string()))?;
        vals.into_iter()
            .map(|v| match v {
                Value::Null => Ok(rusqlite::types::Value::Null),
                Value::Bool(b) => Ok(rusqlite::types::Value::from(if b { 1 } else { 0 })),
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        Ok(rusqlite::types::Value::from(i))
                    } else if let Some(u) = n.as_u64() {
                        Ok(rusqlite::types::Value::from(u as i64))
                    } else if let Some(f) = n.as_f64() {
                        Ok(rusqlite::types::Value::from(f))
                    } else {
                        Err(SqlError::InvalidArgument("unsupported number".to_string()))
                    }
                }
                Value::String(s) => Ok(rusqlite::types::Value::from(s)),
                other => Err(SqlError::InvalidArgument(format!(
                    "unsupported bind value: {}",
                    other
                ))),
            })
            .collect()
    }

    fn row_to_json(row: &Row) -> rusqlite::Result<Value> {
        let stmt = row.as_ref();
        let mut obj = Map::new();
        for i in 0..stmt.column_count() {
            let name = stmt.column_name(i).unwrap_or("").to_string();
            let v = row.get_ref(i)?;
            let j = match v {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(i) => Value::from(i),
                rusqlite::types::ValueRef::Real(f) => Value::from(f),
                rusqlite::types::ValueRef::Text(s) => {
                    let s = std::str::from_utf8(s).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            i,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                    Value::from(s.to_string())
                }
                rusqlite::types::ValueRef::Blob(_) => Value::Null,
            };
            obj.insert(name, j);
        }
        Ok(Value::Object(obj))
    }
}

impl SqlExecutor for SqliteExecutor {
    fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError> {
        let binds = self.parse_params(params_json)?;
        let mut conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
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
        let binds = self.parse_params(params_json)?;
        let rows = {
            let conn = self.conn.lock().map_err(|_| SqlError::Internal)?;
            let mut stmt = conn.prepare(sql).map_err(SqlError::from)?;
            let mapped = stmt.query_map(params_from_iter(binds.into_iter()), |row| {
                Self::row_to_json(row)
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
