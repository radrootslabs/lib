use radroots_sql_core::error::SqlError;
use radroots_sql_core::{SqlExecutor, utils};
use radroots_tangle_db_schema::media_image::{
    IMediaImageCreate,
    IMediaImageCreateResolve,
    IMediaImageDelete,
    IMediaImageDeleteResolve,
    IMediaImageFieldsFilter,
    IMediaImageFindMany,
    IMediaImageFindManyResolve,
    IMediaImageFindOne,
    IMediaImageFindOneResolve,
    IMediaImageUpdate,
    IMediaImageUpdateResolve,
    MediaImage,
    MediaImageFindManyRel,
    MediaImageQueryBindValues,
};
use radroots_types::types::{IError, IResult, IResultList};
use serde_json::Value;

const TABLE_NAME: &str = "media_image";

pub fn create<E: SqlExecutor>(
    exec: &E,
    opts: &IMediaImageCreate,
) -> Result<IMediaImageCreateResolve, IError<SqlError>> {
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
    let on = MediaImageQueryBindValues::Id { id: id.clone() };
    let result = find_one_by_on(exec, &on)?
        .ok_or_else(|| IError::from(SqlError::NotFound(id.clone())))?;
    Ok(IResult { result })
}

pub fn find_one<E: SqlExecutor>(
    exec: &E,
    opts: &IMediaImageFindOne,
) -> Result<IMediaImageFindOneResolve, IError<SqlError>> {
    let result = match opts {
        IMediaImageFindOne::On(args) => find_one_by_on(exec, &args.on)?,
        IMediaImageFindOne::Rel(args) => find_one_by_rel(exec, &args.rel)?,
    };
    Ok(IResult { result })
}

pub fn find_many<E: SqlExecutor>(
    exec: &E,
    opts: &IMediaImageFindMany,
) -> Result<IMediaImageFindManyResolve, IError<SqlError>> {
    let results = match opts {
        IMediaImageFindMany::Filter { filter } => find_many_filter(exec, filter)?,
        IMediaImageFindMany::Rel { rel } => find_many_by_rel(exec, rel)?,
    };
    Ok(IResultList { results })
}

fn find_many_filter<E: SqlExecutor>(
    exec: &E,
    filter: &Option<IMediaImageFieldsFilter>,
) -> Result<Vec<MediaImage>, IError<SqlError>> {
    let (sql, bind_values) = utils::build_select_query_with_meta(TABLE_NAME, filter.as_ref());
    let params_json = utils::to_params_json(bind_values)?;
    let json = exec.query_raw(&sql, &params_json)?;
    let rows: Vec<MediaImage> = utils::parse_json(&json)?;
    Ok(rows)
}

fn find_one_by_on<E: SqlExecutor>(
    exec: &E,
    on: &MediaImageQueryBindValues,
) -> Result<Option<MediaImage>, IError<SqlError>> {
    let (column, value) = on.to_filter_param();
    let sql = format!("SELECT * FROM {TABLE_NAME} WHERE {column} = ? LIMIT 1;");
    let params_json = utils::to_params_json(vec![value])?;
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<MediaImage> = utils::parse_json(&json)?;
    Ok(rows.pop())
}

fn rel_query(rel: &MediaImageFindManyRel) -> (&'static str, Vec<Value>) {
    match rel {
        MediaImageFindManyRel::OnTradeProduct(args) => (
            "SELECT mu.* FROM media_image mu JOIN trade_product_media tp_lg ON mu.id = tp_lg.tb_mu WHERE tp_lg.tb_tp = ?",
            vec![Value::from(args.id.clone())],
        ),
        MediaImageFindManyRel::OffTradeProduct(args) => (
            "SELECT mu.* FROM media_image mu WHERE NOT EXISTS (SELECT 1 FROM trade_product_media tp_lg WHERE tp_lg.tb_mu = mu.id AND tp_lg.tb_tp = ?)",
            vec![Value::from(args.id.clone())],
        ),
    }
}

fn find_one_by_rel<E: SqlExecutor>(
    exec: &E,
    rel: &MediaImageFindManyRel,
) -> Result<Option<MediaImage>, IError<SqlError>> {
    let (sql, bind_values) = rel_query(rel);
    let params_json = utils::to_params_json(bind_values)?;
    let sql = format!("{sql} LIMIT 1;");
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<MediaImage> = utils::parse_json(&json)?;
    Ok(rows.pop())
}

fn find_many_by_rel<E: SqlExecutor>(
    exec: &E,
    rel: &MediaImageFindManyRel,
) -> Result<Vec<MediaImage>, IError<SqlError>> {
    let (sql, bind_values) = rel_query(rel);
    let params_json = utils::to_params_json(bind_values)?;
    let sql = format!("{sql};");
    let json = exec.query_raw(&sql, &params_json)?;
    let rows: Vec<MediaImage> = utils::parse_json(&json)?;
    Ok(rows)
}

fn select_by_id<E: SqlExecutor>(exec: &E, id: &str) -> Result<MediaImage, IError<SqlError>> {
    let params_json = utils::to_params_json(vec![Value::from(id.to_owned())])?;
    let sql = format!("SELECT * FROM {TABLE_NAME} WHERE id = ?;");
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<MediaImage> = utils::parse_json(&json)?;
    rows.pop()
        .ok_or_else(|| IError::from(SqlError::NotFound(id.to_owned())))
}

pub fn update<E: SqlExecutor>(
    exec: &E,
    opts: &IMediaImageUpdate,
) -> Result<IMediaImageUpdateResolve, IError<SqlError>> {
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
            let found = find_one_by_on(exec, &opts.on)?;
            let model = found.ok_or_else(|| IError::from(SqlError::NotFound(opts.on.lookup_key())))?;
            model.id
        }
    };
    bind_values.push(Value::from(id_for_lookup.clone()));
    let sql = format!("UPDATE {TABLE_NAME} SET {} WHERE id = ?;", set_parts.join(", "));
    let params_json = utils::to_params_json(bind_values)?;
    let _ = exec.exec(&sql, &params_json)?;
    let updated = select_by_id(exec, &id_for_lookup)?;
    Ok(IResult { result: updated })
}

pub fn delete<E: SqlExecutor>(
    exec: &E,
    opts: &IMediaImageDelete,
) -> Result<IMediaImageDeleteResolve, IError<SqlError>> {
    let id_for_lookup = match opts {
        IMediaImageDelete::On(args) => match args.on.primary_key() {
            Some(id) => id,
            None => {
                let found = find_one_by_on(exec, &args.on)?;
                let model = found.ok_or_else(|| IError::from(SqlError::NotFound(args.on.lookup_key())))?;
                model.id
            }
        },
        IMediaImageDelete::Rel(args) => {
            let found = find_one_by_rel(exec, &args.rel)?;
            let model = found.ok_or_else(|| IError::from(SqlError::NotFound(rel_lookup_key(&args.rel))))?;
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

fn rel_lookup_key(rel: &MediaImageFindManyRel) -> String {
    match rel {
        MediaImageFindManyRel::OnTradeProduct(args) => format!("on_trade_product:{}", args.id.as_str()),
        MediaImageFindManyRel::OffTradeProduct(args) => format!("off_trade_product:{}", args.id.as_str()),
    }
}
