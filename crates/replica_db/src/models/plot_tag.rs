use radroots_replica_db_schema::plot_tag::{
    IPlotTagCreate, IPlotTagCreateResolve, IPlotTagDelete, IPlotTagDeleteResolve,
    IPlotTagFieldsFilter, IPlotTagFindMany, IPlotTagFindManyResolve, IPlotTagFindOne,
    IPlotTagFindOneResolve, IPlotTagUpdate, IPlotTagUpdateResolve, PlotTag, PlotTagQueryBindValues,
};
use radroots_replica_db_schema::{
    ReplicaSchemaError, ReplicaSchemaResult, ReplicaSchemaResultList,
};
use radroots_sql_core::error::SqlError;
use radroots_sql_core::{SqlExecutor, utils};
use serde_json::Value;

const TABLE_NAME: &str = "plot_tag";

pub fn create(
    exec: &dyn SqlExecutor,
    opts: &IPlotTagCreate,
) -> Result<IPlotTagCreateResolve, ReplicaSchemaError<SqlError>> {
    let field_map = utils::to_object_map(opts).expect("serialize object map");
    let id = utils::uuidv4();
    let now = utils::time_created_on();
    let meta: [(&str, Value); 3] = [
        ("id", Value::from(id.clone())),
        ("created_at", Value::from(now.clone())),
        ("updated_at", Value::from(now.clone())),
    ];
    let (sql, bind_values) = utils::build_insert_query_with_meta(TABLE_NAME, &meta, &field_map);
    let params_json = utils::to_params_json(bind_values).expect("serialize bind params");
    let _ = exec.exec(&sql, &params_json)?;
    let on = PlotTagQueryBindValues::Id { id: id.clone() };
    let result = find_one_by_on(exec, &on)?
        .ok_or(ReplicaSchemaError::from(SqlError::NotFound(id.clone())))?;
    Ok(ReplicaSchemaResult { result })
}

pub fn find_one(
    exec: &dyn SqlExecutor,
    opts: &IPlotTagFindOne,
) -> Result<IPlotTagFindOneResolve, ReplicaSchemaError<SqlError>> {
    let result = match opts {
        IPlotTagFindOne::On(args) => find_one_by_on(exec, &args.on)?,
    };
    Ok(ReplicaSchemaResult { result })
}

pub fn find_many(
    exec: &dyn SqlExecutor,
    opts: &IPlotTagFindMany,
) -> Result<IPlotTagFindManyResolve, ReplicaSchemaError<SqlError>> {
    let results = find_many_filter(exec, &opts.filter)?;
    Ok(ReplicaSchemaResultList { results })
}

fn find_many_filter(
    exec: &dyn SqlExecutor,
    filter: &Option<IPlotTagFieldsFilter>,
) -> Result<Vec<PlotTag>, ReplicaSchemaError<SqlError>> {
    let (sql, bind_values) = utils::build_select_query_with_meta(TABLE_NAME, filter.as_ref());
    let params_json = utils::to_params_json(bind_values).expect("serialize bind params");
    let json = exec.query_raw(&sql, &params_json)?;
    let rows: Vec<PlotTag> = utils::parse_json(&json)?;
    Ok(rows)
}

fn find_one_by_on(
    exec: &dyn SqlExecutor,
    on: &PlotTagQueryBindValues,
) -> Result<Option<PlotTag>, ReplicaSchemaError<SqlError>> {
    let (column, value) = on.to_filter_param();
    let sql = format!("SELECT * FROM {TABLE_NAME} WHERE {column} = ? LIMIT 1;");
    let params_json = utils::to_params_json(vec![value]).expect("serialize bind params");
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<PlotTag> = utils::parse_json(&json)?;
    Ok(rows.pop())
}

fn select_by_id(exec: &dyn SqlExecutor, id: &str) -> Result<PlotTag, ReplicaSchemaError<SqlError>> {
    let params_json =
        utils::to_params_json(vec![Value::from(id.to_owned())]).expect("serialize bind params");
    let sql = format!("SELECT * FROM {TABLE_NAME} WHERE id = ?;");
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<PlotTag> = utils::parse_json(&json)?;
    rows.pop()
        .ok_or(ReplicaSchemaError::from(SqlError::NotFound(id.to_owned())))
}

pub fn update(
    exec: &dyn SqlExecutor,
    opts: &IPlotTagUpdate,
) -> Result<IPlotTagUpdateResolve, ReplicaSchemaError<SqlError>> {
    let mut updates =
        utils::to_partial_object_map(&opts.fields).expect("serialize partial object map");
    if updates.is_empty() {
        return Err(ReplicaSchemaError::from(SqlError::InvalidArgument(
            String::from("no fields to update"),
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
    let id_for_lookup = match opts.on.primary_key() {
        Some(id) => id,
        None => {
            let found = find_one_by_on(exec, &opts.on)?;
            let model = found.ok_or(ReplicaSchemaError::from(SqlError::NotFound(
                opts.on.lookup_key(),
            )))?;
            model.id
        }
    };
    bind_values.push(Value::from(id_for_lookup.clone()));
    let sql = format!(
        "UPDATE {TABLE_NAME} SET {} WHERE id = ?;",
        set_parts.join(", ")
    );
    let params_json = utils::to_params_json(bind_values).expect("serialize bind params");
    let _ = exec.exec(&sql, &params_json)?;
    let updated = select_by_id(exec, &id_for_lookup)?;
    Ok(ReplicaSchemaResult { result: updated })
}

pub fn delete(
    exec: &dyn SqlExecutor,
    opts: &IPlotTagDelete,
) -> Result<IPlotTagDeleteResolve, ReplicaSchemaError<SqlError>> {
    let id_for_lookup = match opts {
        IPlotTagDelete::On(args) => match args.on.primary_key() {
            Some(id) => id,
            None => {
                let found = find_one_by_on(exec, &args.on)?;
                let model = found.ok_or(ReplicaSchemaError::from(SqlError::NotFound(
                    args.on.lookup_key(),
                )))?;
                model.id
            }
        },
    };
    let params_json = utils::to_params_json(vec![Value::from(id_for_lookup.clone())])
        .expect("serialize bind params");
    let sql = format!("DELETE FROM {TABLE_NAME} WHERE id = ?;");
    let outcome = exec.exec(&sql, &params_json)?;
    if outcome.changes == 0 {
        return Err(ReplicaSchemaError::from(SqlError::NotFound(
            id_for_lookup.clone(),
        )));
    }
    Ok(ReplicaSchemaResult {
        result: id_for_lookup,
    })
}
