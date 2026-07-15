#![forbid(unsafe_code)]

use crate::error::SqlError;
use serde_json::{Map, Value};
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::{Column, Row, TypeInfo, ValueRef};

#[derive(Debug)]
pub enum SqliteBindValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
}

pub fn parse_params(params_json: &str) -> Result<Vec<SqliteBindValue>, SqlError> {
    let vals: Vec<Value> = serde_json::from_str(params_json)
        .map_err(|e| SqlError::SerializationError(e.to_string()))?;
    vals.into_iter()
        .map(|v| match v {
            Value::Null => Ok(SqliteBindValue::Null),
            Value::Bool(b) => Ok(SqliteBindValue::Integer(i64::from(b))),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(SqliteBindValue::Integer(i))
                } else if let Some(u) = n.as_u64() {
                    let value = i64::try_from(u).map_err(|_| {
                        SqlError::InvalidArgument("integer bind exceeds i64".to_string())
                    })?;
                    Ok(SqliteBindValue::Integer(value))
                } else if let Some(f) = n.as_f64() {
                    Ok(SqliteBindValue::Real(f))
                } else {
                    Err(SqlError::InvalidArgument("unsupported number".to_string()))
                }
            }
            Value::String(s) => Ok(SqliteBindValue::Text(s)),
            other => Err(SqlError::InvalidArgument(format!(
                "unsupported bind value: {}",
                other
            ))),
        })
        .collect()
}

pub fn bind_params<'q>(
    query: sqlx::query::Query<'q, sqlx::Sqlite, SqliteArguments>,
    params: Vec<SqliteBindValue>,
) -> Result<sqlx::query::Query<'q, sqlx::Sqlite, SqliteArguments>, SqlError> {
    let mut query = query;
    for param in params {
        query = match param {
            SqliteBindValue::Null => query.bind(Option::<String>::None),
            SqliteBindValue::Integer(value) => query.bind(value),
            SqliteBindValue::Real(value) => query.bind(value),
            SqliteBindValue::Text(value) => query.bind(value),
        };
    }
    Ok(query)
}

pub fn row_to_json(row: &SqliteRow) -> Result<Value, SqlError> {
    let mut obj = Map::new();
    for (index, column) in row.columns().iter().enumerate() {
        let raw = row.try_get_raw(index)?;
        let value = if raw.is_null() {
            Value::Null
        } else {
            match raw.type_info().name() {
                "INTEGER" | "BOOLEAN" => Value::from(row.try_get::<i64, _>(index)?),
                "REAL" => Value::from(row.try_get::<f64, _>(index)?),
                "TEXT" | "DATE" | "TIME" | "DATETIME" => {
                    Value::from(row.try_get::<String, _>(index)?)
                }
                "BLOB" => Value::Null,
                _ => return Err(SqlError::InvalidQuery(raw.type_info().name().to_string())),
            }
        };
        obj.insert(column.name().to_string(), value);
    }
    Ok(Value::Object(obj))
}
