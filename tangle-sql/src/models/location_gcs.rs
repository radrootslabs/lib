use radroots_sql_core::error::SqlError;
use radroots_sql_core::{SqlExecutor, utils};
use radroots_tangle_schema::location_gcs::{
    ILocationGcsCreate,
    ILocationGcsCreateResolve,
    ILocationGcsDelete,
    ILocationGcsDeleteResolve,
    ILocationGcsFieldsFilter,
    ILocationGcsFindMany,
    ILocationGcsFindManyResolve,
    ILocationGcsFindOne,
    ILocationGcsFindOneResolve,
    ILocationGcsUpdate,
    ILocationGcsUpdateResolve,
    LocationGcs,
    LocationGcsFindManyRel,
    LocationGcsQueryBindValues,
};
use radroots_types::types::{IError, IResult, IResultList};
use serde_json::Value;

const TABLE_NAME: &str = "location_gcs";

pub fn create<E: SqlExecutor>(
    exec: &E,
    opts: &ILocationGcsCreate,
) -> Result<ILocationGcsCreateResolve, IError<SqlError>> {
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
    let args = ILocationGcsFindOne {
        on: LocationGcsQueryBindValues::Id { id: id.clone() },
    };
    let found = find_one(exec, &args)?;
    let result = found
        .result
        .ok_or_else(|| IError::from(SqlError::NotFound(id.clone())))?;
    Ok(IResult { result })
}

pub fn find_one<E: SqlExecutor>(
    exec: &E,
    opts: &ILocationGcsFindOne,
) -> Result<ILocationGcsFindOneResolve, IError<SqlError>> {
    let (column, value) = opts.on.to_filter_param();
    let sql = format!("SELECT * FROM {TABLE_NAME} WHERE {column} = ? LIMIT 1;");
    let params_json = utils::to_params_json(vec![value])?;
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<LocationGcs> = utils::parse_json(&json)?;
    let result = rows.pop();
    Ok(IResult { result })
}

pub fn find_many<E: SqlExecutor>(
    exec: &E,
    opts: &ILocationGcsFindMany,
) -> Result<ILocationGcsFindManyResolve, IError<SqlError>> {
    let results = match opts {
        ILocationGcsFindMany::Filter { filter } => find_many_filter(exec, filter)?,
        ILocationGcsFindMany::Rel { rel } => find_many_by_rel(exec, rel)?,
    };
    Ok(IResultList { results })
}

fn find_many_filter<E: SqlExecutor>(
    exec: &E,
    filter: &Option<ILocationGcsFieldsFilter>,
) -> Result<Vec<LocationGcs>, IError<SqlError>> {
    let (sql, bind_values) = utils::build_select_query_with_meta(TABLE_NAME, filter.as_ref());
    let params_json = utils::to_params_json(bind_values)?;
    let json = exec.query_raw(&sql, &params_json)?;
    let rows: Vec<LocationGcs> = utils::parse_json(&json)?;
    Ok(rows)
}

fn find_many_by_rel<E: SqlExecutor>(
    exec: &E,
    rel: &LocationGcsFindManyRel,
) -> Result<Vec<LocationGcs>, IError<SqlError>> {
    let (sql, bind_values): (String, Vec<Value>) = match rel {
        LocationGcsFindManyRel::OnTradeProduct(args) => {
            let sql = String::from("SELECT lg.* FROM location_gcs lg JOIN trade_product_location tp_lg ON lg.id = tp_lg.tb_lg WHERE tp_lg.tb_tp = ?;");
            let binds = vec![Value::from(args.id.clone())];
            (sql, binds)
        }
        LocationGcsFindManyRel::OffTradeProduct(args) => {
            let sql = String::from("SELECT lg.* FROM location_gcs lg WHERE NOT EXISTS (SELECT 1 FROM trade_product_location tp_lg WHERE tp_lg.tb_lg = lg.id AND tp_lg.tb_tp = ?);");
            let binds = vec![Value::from(args.id.clone())];
            (sql, binds)
        }
        LocationGcsFindManyRel::OnFarm(args) => {
            let sql = String::from("SELECT lg.* FROM location_gcs lg JOIN farm_location farm_lg ON lg.id = farm_lg.tb_lg WHERE farm_lg.tb_farm = ?;");
            let binds = vec![Value::from(args.id.clone())];
            (sql, binds)
        }
        LocationGcsFindManyRel::OffFarm(args) => {
            let sql = String::from("SELECT lg.* FROM location_gcs lg WHERE NOT EXISTS (SELECT 1 FROM farm_location farm_lg WHERE farm_lg.tb_lg = lg.id AND farm_lg.tb_farm = ?);");
            let binds = vec![Value::from(args.id.clone())];
            (sql, binds)
        }
    };
    let params_json = utils::to_params_json(bind_values)?;
    let json = exec.query_raw(&sql, &params_json)?;
    let rows: Vec<LocationGcs> = utils::parse_json(&json)?;
    Ok(rows)
}

fn select_by_id<E: SqlExecutor>(exec: &E, id: &str) -> Result<LocationGcs, IError<SqlError>> {
    let params_json = utils::to_params_json(vec![Value::from(id.to_owned())])?;
    let sql = format!("SELECT * FROM {TABLE_NAME} WHERE id = ?;");
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<LocationGcs> = utils::parse_json(&json)?;
    rows.pop()
        .ok_or_else(|| IError::from(SqlError::NotFound(id.to_owned())))
}

pub fn update<E: SqlExecutor>(
    exec: &E,
    opts: &ILocationGcsUpdate,
) -> Result<ILocationGcsUpdateResolve, IError<SqlError>> {
    let mut updates = utils::to_partial_object_map(&opts.fields)?;
    if updates.is_empty() {
        return Err(IError::from(SqlError::InvalidArgument(String::from("no fields to update"))));
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
            let find_opts = ILocationGcsFindOne {
                on: opts.on.clone(),
            };
            let found = find_one(exec, &find_opts)?;
            let model = found.result.ok_or_else(|| IError::from(SqlError::NotFound(opts.on.lookup_key())))?;
            model.id
        }
    };
    bind_values.push(Value::from(id_for_lookup.clone()));
    let sql = format!("UPDATE {TABLE_NAME} SET {} WHERE id = ?;", set_parts.join(", "));
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
    opts: &ILocationGcsDelete,
) -> Result<ILocationGcsDeleteResolve, IError<SqlError>> {
    let id_for_lookup = match opts.on.primary_key() {
        Some(id) => id,
        None => {
            let find_opts = ILocationGcsFindOne {
                on: opts.on.clone(),
            };
            let found = find_one(exec, &find_opts)?;
            let model = found.result.ok_or_else(|| IError::from(SqlError::NotFound(opts.on.lookup_key())))?;
            model.id
        }
    };
    let params_json = utils::to_params_json(vec![Value::from(id_for_lookup.clone())])?;
    let sql = format!("DELETE FROM {TABLE_NAME} WHERE id = ?;");
    let outcome = exec.exec(&sql, &params_json)?;
    if outcome.changes == 0 {
        return Err(IError::from(SqlError::NotFound(id_for_lookup.clone())));
    }
    Ok(IResult { result: id_for_lookup })
}
