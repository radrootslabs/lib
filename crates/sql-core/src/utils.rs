use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::error::SqlError;

pub fn parse_json<T: for<'de> Deserialize<'de>>(s: &str) -> Result<T, SqlError> {
    serde_json::from_str::<T>(s).map_err(SqlError::from)
}

pub fn uuidv4() -> String {
    Uuid::new_v4().to_string()
}

pub fn time_created_on() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn to_object_map<T: Serialize>(opts: T) -> Result<Map<String, Value>, SqlError> {
    let v = serde_json::to_value(opts).map_err(SqlError::from)?;
    let obj = v
        .as_object()
        .ok_or_else(|| SqlError::SerializationError(String::from("Expected an object")))?;
    Ok(obj.clone())
}

pub fn to_partial_object_map<T: Serialize>(opts: T) -> Result<Map<String, Value>, SqlError> {
    let v = serde_json::to_value(opts).map_err(SqlError::from)?;
    let obj = v
        .as_object()
        .ok_or_else(|| SqlError::SerializationError(String::from("Expected an object")))?;
    let mut filtered = Map::new();
    for (k, v) in obj.iter() {
        if !v.is_null() {
            filtered.insert(k.clone(), v.clone());
        }
    }
    Ok(filtered)
}

pub fn to_db_bind_value(value: &Value) -> Value {
    match value {
        Value::Bool(b) => Value::from(i64::from(*b)),
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                Value::from(f)
            } else if let Some(i) = n.as_i64() {
                Value::from(i)
            } else if let Some(u) = n.as_u64() {
                if u <= u32::MAX as u64 {
                    Value::from(u as u32)
                } else {
                    Value::from(u)
                }
            } else {
                Value::Null
            }
        }
        Value::String(s) => Value::from(s.clone()),
        _ => Value::Null,
    }
}

pub fn build_where_clause_eq<T: Serialize>(filter: &T) -> Result<(String, Vec<Value>), SqlError> {
    let obj = to_partial_object_map(filter)?;
    if obj.is_empty() {
        return Ok((String::new(), Vec::new()));
    }
    let mut clauses = Vec::with_capacity(obj.len());
    let mut binds = Vec::with_capacity(obj.len());
    for (k, v) in obj {
        clauses.push(format!("{k} = ?"));
        binds.push(to_db_bind_value(&v));
    }
    Ok((format!(" WHERE {}", clauses.join(" AND ")), binds))
}

pub fn build_insert_query_with_meta(
    table: &str,
    meta: &[(&str, Value)],
    fields: &Map<String, Value>,
) -> (String, Vec<Value>) {
    let mut cols: Vec<String> = meta.iter().map(|(k, _)| k.to_string()).collect();
    cols.extend(fields.keys().cloned());
    let meta_binds: Vec<Value> = meta.iter().map(|(_, v)| to_db_bind_value(v)).collect();
    let field_binds: Vec<Value> = fields.values().map(to_db_bind_value).collect();
    let placeholders = (0..cols.len())
        .map(|_| "?")
        .collect::<Vec<&str>>()
        .join(",");
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({});",
        cols.join(","),
        placeholders
    );
    let mut binds = Vec::with_capacity(cols.len());
    binds.extend(meta_binds);
    binds.extend(field_binds);
    (sql, binds)
}

pub fn build_select_query_with_meta<T: Serialize>(
    table: &str,
    filter: Option<&T>,
) -> (String, Vec<Value>) {
    let (where_clause, binds) = match filter {
        Some(f) => match build_where_clause_eq(f) {
            Ok(t) => t,
            Err(_) => (String::new(), Vec::new()),
        },
        None => (String::new(), Vec::new()),
    };
    let sql = format!("SELECT * FROM {table}{where_clause};");
    (sql, binds)
}

pub fn parse_query_value(v: &Value) -> Result<Value, SqlError> {
    Ok(match v {
        Value::Bool(b) => {
            if *b {
                serde_json::json!(1)
            } else {
                serde_json::json!(0)
            }
        }
        Value::Null => Value::Null,
        Value::Number(_) | Value::String(_) => v.clone(),
        other => {
            return Err(SqlError::InvalidArgument(other.to_string()));
        }
    })
}

pub fn to_params_json<T: Serialize>(v: T) -> Result<String, SqlError> {
    serde_json::to_string(&v).map_err(SqlError::from)
}

pub fn with_transaction<E, F, T>(exec: &E, f: F) -> Result<T, SqlError>
where
    E: crate::SqlExecutor,
    F: FnOnce() -> Result<T, SqlError>,
{
    exec.begin()?;
    match f() {
        Ok(v) => {
            exec.commit()?;
            Ok(v)
        }
        Err(e) => {
            let _ = exec.rollback();
            Err(e)
        }
    }
}
