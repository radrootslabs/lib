use radroots_replica_db_schema::farm::{
    Farm, FarmQueryBindValues, IFarmCreate, IFarmCreateResolve, IFarmDelete, IFarmDeleteResolve,
    IFarmFieldsFilter, IFarmFindMany, IFarmFindManyResolve, IFarmFindOne, IFarmFindOneResolve,
    IFarmUpdate, IFarmUpdateResolve,
};
use radroots_sql_core::error::SqlError;
use radroots_sql_core::{SqlExecutor, utils};
use radroots_types::types::{IError, IResult, IResultList};
use serde_json::Value;

const TABLE_NAME: &str = "farm";

pub fn create(
    exec: &dyn SqlExecutor,
    opts: &IFarmCreate,
) -> Result<IFarmCreateResolve, IError<SqlError>> {
    let field_map = utils::to_object_map(opts).expect("farm create fields serialize");
    let id = utils::uuidv4();
    let now = utils::time_created_on();
    let meta: [(&str, Value); 3] = [
        ("id", Value::from(id.clone())),
        ("created_at", Value::from(now.clone())),
        ("updated_at", Value::from(now.clone())),
    ];
    let (sql, bind_values) = utils::build_insert_query_with_meta(TABLE_NAME, &meta, &field_map);
    let params_json =
        utils::to_params_json(bind_values).expect("farm create bind params serialize");
    let _ = exec.exec(&sql, &params_json)?;
    let on = FarmQueryBindValues::Id { id: id.clone() };
    let result = find_one_by_on(exec, &on)?.ok_or(IError::from(SqlError::NotFound(id.clone())))?;
    Ok(IResult { result })
}

pub fn find_one(
    exec: &dyn SqlExecutor,
    opts: &IFarmFindOne,
) -> Result<IFarmFindOneResolve, IError<SqlError>> {
    let result = match opts {
        IFarmFindOne::On(args) => find_one_by_on(exec, &args.on)?,
    };
    Ok(IResult { result })
}

pub fn find_many(
    exec: &dyn SqlExecutor,
    opts: &IFarmFindMany,
) -> Result<IFarmFindManyResolve, IError<SqlError>> {
    let results = find_many_filter(exec, &opts.filter)?;
    Ok(IResultList { results })
}

fn find_many_filter(
    exec: &dyn SqlExecutor,
    filter: &Option<IFarmFieldsFilter>,
) -> Result<Vec<Farm>, IError<SqlError>> {
    let (sql, bind_values) = utils::build_select_query_with_meta(TABLE_NAME, filter.as_ref());
    let params_json =
        utils::to_params_json(bind_values).expect("farm find_many bind params serialize");
    let json = exec.query_raw(&sql, &params_json)?;
    let rows: Vec<Farm> = utils::parse_json(&json)?;
    Ok(rows)
}

fn find_one_by_on(
    exec: &dyn SqlExecutor,
    on: &FarmQueryBindValues,
) -> Result<Option<Farm>, IError<SqlError>> {
    let (column, value) = on.to_filter_param();
    let sql = format!("SELECT * FROM {TABLE_NAME} WHERE {column} = ? LIMIT 1;");
    let params_json =
        utils::to_params_json(vec![value]).expect("farm find_one bind params serialize");
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<Farm> = utils::parse_json(&json)?;
    Ok(rows.pop())
}

fn select_by_id(exec: &dyn SqlExecutor, id: &str) -> Result<Farm, IError<SqlError>> {
    let params_json = utils::to_params_json(vec![Value::from(id.to_owned())])
        .expect("farm select_by_id bind params serialize");
    let sql = format!("SELECT * FROM {TABLE_NAME} WHERE id = ?;");
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<Farm> = utils::parse_json(&json)?;
    rows.pop()
        .ok_or(IError::from(SqlError::NotFound(id.to_owned())))
}

pub fn update(
    exec: &dyn SqlExecutor,
    opts: &IFarmUpdate,
) -> Result<IFarmUpdateResolve, IError<SqlError>> {
    let mut updates =
        utils::to_partial_object_map(&opts.fields).expect("farm update fields serialize");
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
    let id_for_lookup = match opts.on.primary_key() {
        Some(id) => id,
        None => {
            let found = find_one_by_on(exec, &opts.on)?;
            let model = found.ok_or(IError::from(SqlError::NotFound(opts.on.lookup_key())))?;
            model.id
        }
    };
    bind_values.push(Value::from(id_for_lookup.clone()));
    let sql = format!(
        "UPDATE {TABLE_NAME} SET {} WHERE id = ?;",
        set_parts.join(", ")
    );
    let params_json =
        utils::to_params_json(bind_values).expect("farm update bind params serialize");
    let _ = exec.exec(&sql, &params_json)?;
    let updated = select_by_id(exec, &id_for_lookup)?;
    Ok(IResult { result: updated })
}

pub fn delete(
    exec: &dyn SqlExecutor,
    opts: &IFarmDelete,
) -> Result<IFarmDeleteResolve, IError<SqlError>> {
    let id_for_lookup = match opts {
        IFarmDelete::On(args) => match args.on.primary_key() {
            Some(id) => id,
            None => {
                let found = find_one_by_on(exec, &args.on)?;
                let model = found.ok_or(IError::from(SqlError::NotFound(args.on.lookup_key())))?;
                model.id
            }
        },
    };
    let params_json = utils::to_params_json(vec![Value::from(id_for_lookup.clone())])
        .expect("farm delete bind params serialize");
    let sql = format!("DELETE FROM {TABLE_NAME} WHERE id = ?;");
    let outcome = exec.exec(&sql, &params_json)?;
    if outcome.changes == 0 {
        return Err(IError::from(SqlError::NotFound(id_for_lookup.clone())));
    }
    Ok(IResult {
        result: id_for_lookup,
    })
}
