#![forbid(unsafe_code)]

use crate::error::SqlError;
use rusqlite::{Row, types::Value as SqlValue};
use serde_json::{Map, Value};

pub fn parse_params(params_json: &str) -> Result<Vec<SqlValue>, SqlError> {
    let vals: Vec<Value> = serde_json::from_str(params_json)
        .map_err(|e| SqlError::SerializationError(e.to_string()))?;
    vals.into_iter()
        .map(|v| match v {
            Value::Null => Ok(SqlValue::Null),
            Value::Bool(b) => Ok(SqlValue::from(if b { 1 } else { 0 })),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(SqlValue::from(i))
                } else if let Some(u) = n.as_u64() {
                    Ok(SqlValue::from(u as i64))
                } else if let Some(f) = n.as_f64() {
                    Ok(SqlValue::from(f))
                } else {
                    Err(SqlError::InvalidArgument("unsupported number".to_string()))
                }
            }
            Value::String(s) => Ok(SqlValue::from(s)),
            other => Err(SqlError::InvalidArgument(format!(
                "unsupported bind value: {}",
                other
            ))),
        })
        .collect()
}

pub fn row_to_json(row: &Row) -> rusqlite::Result<Value> {
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
