use radroots_sql_core::error::SqlError;
use radroots_sql_core::utils;
use radroots_sql_core::{ExecOutcome, SqlExecutor};
use radroots_tangle_schema::log_error::{
    ILogErrorFields, ILogErrorFieldsFilter, ILogErrorFieldsPartial, LogError,
    LogErrorQueryBindValues,
};
use serde_json::Value;

const TABLE_NAME: &str = "log_error";

pub fn insert<E: SqlExecutor>(exec: &E, fields: ILogErrorFields) -> Result<LogError, SqlError> {
    let field_map = utils::to_object_map(&fields)?;
    let id = utils::uuidv4();
    let now = utils::time_created_on();
    let meta: [(&str, Value); 3] = [
        ("id", Value::from(id.clone())),
        ("created_at", Value::from(now.clone())),
        ("updated_at", Value::from(now.clone())),
    ];
    let (sql, bind_values) = utils::build_insert_query_with_meta(TABLE_NAME, &meta, &field_map);
    let params_json = utils::to_params_json(bind_values)?;
    let _ = exec.exec(&sql, &params_json)?;
    let log_error = LogError {
        id,
        created_at: now.clone(),
        updated_at: now,
        error: fields.error,
        message: fields.message,
        stack_trace: fields.stack_trace,
        cause: fields.cause,
        app_system: fields.app_system,
        app_version: fields.app_version,
        nostr_pubkey: fields.nostr_pubkey,
        data: fields.data,
    };
    Ok(log_error)
}

pub fn find_many<E: SqlExecutor>(
    exec: &E,
    filter: Option<&ILogErrorFieldsFilter>,
) -> Result<Vec<LogError>, SqlError> {
    let (sql, bind_values) = utils::build_select_query_with_meta(TABLE_NAME, filter);
    let params_json = utils::to_params_json(bind_values)?;
    let json = exec.query_raw(&sql, &params_json)?;
    let rows: Vec<LogError> = utils::parse_json(&json)?;
    Ok(rows)
}

pub fn find_one<E: SqlExecutor>(
    exec: &E,
    bind: &LogErrorQueryBindValues,
) -> Result<Option<LogError>, SqlError> {
    let (sql, bind_values) = utils::build_select_query_with_meta(TABLE_NAME, Some(bind));
    let params_json = utils::to_params_json(bind_values)?;
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<LogError> = utils::parse_json(&json)?;
    Ok(rows.pop())
}

pub fn update<E: SqlExecutor>(
    exec: &E,
    id: &str,
    fields: ILogErrorFieldsPartial,
) -> Result<ExecOutcome, SqlError> {
    let mut updates = utils::to_partial_object_map(fields)?;
    if updates.is_empty() {
        return Err(SqlError::InvalidArgument(String::from(
            "no fields to update",
        )));
    }
    updates.insert(
        String::from("updated_at"),
        Value::from(utils::time_created_on()),
    );
    let mut set_parts = Vec::with_capacity(updates.len());
    let mut bind_values = Vec::with_capacity(updates.len() + 1);
    for (column, value) in updates {
        set_parts.push(format!("{column} = ?"));
        bind_values.push(utils::to_db_bind_value(&value));
    }
    bind_values.push(Value::from(String::from(id)));
    let sql = format!(
        "UPDATE {TABLE_NAME} SET {} WHERE id = ?;",
        set_parts.join(", ")
    );
    let params_json = utils::to_params_json(bind_values)?;
    exec.exec(&sql, &params_json)
}

pub fn delete<E: SqlExecutor>(
    exec: &E,
    bind: &LogErrorQueryBindValues,
) -> Result<ExecOutcome, SqlError> {
    let (where_clause, bind_values) = utils::build_where_clause_eq(bind)?;
    if where_clause.is_empty() {
        return Err(SqlError::InvalidArgument(String::from(
            "delete requires at least one filter field",
        )));
    }
    let sql = format!("DELETE FROM {TABLE_NAME}{where_clause};");
    let params_json = utils::to_params_json(bind_values)?;
    exec.exec(&sql, &params_json)
}
