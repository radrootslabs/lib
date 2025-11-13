use radroots_sql_core::error::SqlError;
use radroots_sql_core::{SqlExecutor, utils};
use radroots_tangle_schema::log_error::{
    ILogErrorCreate, ILogErrorCreateResolve, ILogErrorDelete, ILogErrorDeleteResolve,
    ILogErrorFindMany, ILogErrorFindManyResolve, ILogErrorFindOne, ILogErrorFindOneResolve,
    ILogErrorUpdate, ILogErrorUpdateResolve, LogError, LogErrorQueryBindValues,
};
use radroots_types::types::{IError, IResult, IResultList};
use serde_json::Value;

const TABLE_NAME: &str = "log_error";

pub fn create<E: SqlExecutor>(
    exec: &E,
    opts: &ILogErrorCreate,
) -> Result<ILogErrorCreateResolve, IError<SqlError>> {
    let field_map = utils::to_object_map(opts)?;
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
    let result = LogError {
        id,
        created_at: now.clone(),
        updated_at: now,
        error: opts.error.clone(),
        message: opts.message.clone(),
        stack_trace: opts.stack_trace.clone(),
        cause: opts.cause.clone(),
        app_system: opts.app_system.clone(),
        app_version: opts.app_version.clone(),
        nostr_pubkey: opts.nostr_pubkey.clone(),
        data: opts.data.clone(),
    };
    Ok(IResult { result })
}

pub fn find_one<E: SqlExecutor>(
    exec: &E,
    opts: &ILogErrorFindOne,
) -> Result<ILogErrorFindOneResolve, IError<SqlError>> {
    let (column, value) = match &opts.on {
        LogErrorQueryBindValues::Id { id } => ("id", Value::from(id.clone())),
        LogErrorQueryBindValues::NostrPubkey { nostr_pubkey } => {
            ("nostr_pubkey", Value::from(nostr_pubkey.clone()))
        }
    };
    let sql = format!("SELECT * FROM {TABLE_NAME} WHERE {column} = ? LIMIT 1;");
    let params_json = utils::to_params_json(vec![value])?;
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<LogError> = utils::parse_json(&json)?;
    let result = rows.pop();
    Ok(IResult { result })
}

pub fn find_many<E: SqlExecutor>(
    exec: &E,
    opts: &ILogErrorFindMany,
) -> Result<ILogErrorFindManyResolve, IError<SqlError>> {
    let (sql, bind_values) = utils::build_select_query_with_meta(TABLE_NAME, opts.filter.as_ref());
    let params_json = utils::to_params_json(bind_values)?;
    let json = exec.query_raw(&sql, &params_json)?;
    let results: Vec<LogError> = utils::parse_json(&json)?;
    Ok(IResultList { results })
}

fn select_by_id<E: SqlExecutor>(exec: &E, id: &str) -> Result<LogError, IError<SqlError>> {
    let params_json = utils::to_params_json(vec![Value::from(id.to_owned())])?;
    let sql = format!("SELECT * FROM {TABLE_NAME} WHERE id = ?;");
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<LogError> = utils::parse_json(&json)?;
    rows.pop()
        .ok_or_else(|| IError::from(SqlError::NotFound(id.to_owned())))
}

fn resolve_on_bind<E: SqlExecutor>(
    exec: &E,
    on: &LogErrorQueryBindValues,
) -> Result<(&'static str, Value, String), IError<SqlError>> {
    match on {
        LogErrorQueryBindValues::Id { id } => Ok(("id", Value::from(id.clone()), id.clone())),
        LogErrorQueryBindValues::NostrPubkey { nostr_pubkey } => {
            let args = ILogErrorFindOne {
                on: LogErrorQueryBindValues::NostrPubkey {
                    nostr_pubkey: nostr_pubkey.clone(),
                },
            };
            let found = find_one(exec, &args)?;
            let model = found
                .result
                .ok_or_else(|| IError::from(SqlError::NotFound(nostr_pubkey.clone())))?;
            Ok(("nostr_pubkey", Value::from(nostr_pubkey.clone()), model.id))
        }
    }
}

pub fn update<E: SqlExecutor>(
    exec: &E,
    opts: &ILogErrorUpdate,
) -> Result<ILogErrorUpdateResolve, IError<SqlError>> {
    let mut updates = utils::to_partial_object_map(&opts.fields)?;
    if updates.is_empty() {
        return Err(IError::from(SqlError::InvalidArgument(String::from(
            "no fields to update",
        ))));
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
    let (where_column, where_value, id_for_lookup) = resolve_on_bind(exec, &opts.on)?;
    bind_values.push(where_value);
    let sql = format!(
        "UPDATE {TABLE_NAME} SET {} WHERE {where_column} = ?;",
        set_parts.join(", ")
    );
    let params_json = utils::to_params_json(bind_values)?;
    let outcome = exec.exec(&sql, &params_json)?;
    if outcome.changes == 0 {
        return Err(IError::from(SqlError::NotFound(id_for_lookup.clone())));
    }
    let updated = select_by_id(exec, &id_for_lookup)?;
    Ok(IResult { result: updated })
}

pub fn delete<E: SqlExecutor>(
    exec: &E,
    opts: &ILogErrorDelete,
) -> Result<ILogErrorDeleteResolve, IError<SqlError>> {
    let (_, _, id) = resolve_on_bind(exec, &opts.on)?;
    let params_json = utils::to_params_json(vec![Value::from(id.clone())])?;
    let sql = format!("DELETE FROM {TABLE_NAME} WHERE id = ?;");
    let outcome = exec.exec(&sql, &params_json)?;
    if outcome.changes == 0 {
        return Err(IError::from(SqlError::NotFound(id)));
    }
    Ok(IResult { result: id })
}
