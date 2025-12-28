use radroots_sql_core::error::SqlError;
use radroots_sql_core::{SqlExecutor, utils};
use radroots_tangle_db_schema::nostr_profile::{
    INostrProfileCreate,
    INostrProfileCreateResolve,
    INostrProfileDelete,
    INostrProfileDeleteResolve,
    INostrProfileFieldsFilter,
    INostrProfileFindMany,
    INostrProfileFindManyResolve,
    INostrProfileFindOne,
    INostrProfileFindOneResolve,
    INostrProfileUpdate,
    INostrProfileUpdateResolve,
    NostrProfile,
    NostrProfileFindManyRel,
    NostrProfileQueryBindValues,
};
use radroots_types::types::{IError, IResult, IResultList};
use serde_json::Value;

const TABLE_NAME: &str = "nostr_profile";

pub fn create<E: SqlExecutor>(
    exec: &E,
    opts: &INostrProfileCreate,
) -> Result<INostrProfileCreateResolve, IError<SqlError>> {
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
    let on = NostrProfileQueryBindValues::Id { id: id.clone() };
    let result = find_one_by_on(exec, &on)?
        .ok_or_else(|| IError::from(SqlError::NotFound(id.clone())))?;
    Ok(IResult { result })
}

pub fn find_one<E: SqlExecutor>(
    exec: &E,
    opts: &INostrProfileFindOne,
) -> Result<INostrProfileFindOneResolve, IError<SqlError>> {
    let result = match opts {
        INostrProfileFindOne::On(args) => find_one_by_on(exec, &args.on)?,
        INostrProfileFindOne::Rel(args) => find_one_by_rel(exec, &args.rel)?,
    };
    Ok(IResult { result })
}

pub fn find_many<E: SqlExecutor>(
    exec: &E,
    opts: &INostrProfileFindMany,
) -> Result<INostrProfileFindManyResolve, IError<SqlError>> {
    let results = match opts {
        INostrProfileFindMany::Filter { filter } => find_many_filter(exec, filter)?,
        INostrProfileFindMany::Rel { rel } => find_many_by_rel(exec, rel)?,
    };
    Ok(IResultList { results })
}

fn find_many_filter<E: SqlExecutor>(
    exec: &E,
    filter: &Option<INostrProfileFieldsFilter>,
) -> Result<Vec<NostrProfile>, IError<SqlError>> {
    let (sql, bind_values) = utils::build_select_query_with_meta(TABLE_NAME, filter.as_ref());
    let params_json = utils::to_params_json(bind_values)?;
    let json = exec.query_raw(&sql, &params_json)?;
    let rows: Vec<NostrProfile> = utils::parse_json(&json)?;
    Ok(rows)
}

fn find_one_by_on<E: SqlExecutor>(
    exec: &E,
    on: &NostrProfileQueryBindValues,
) -> Result<Option<NostrProfile>, IError<SqlError>> {
    let (column, value) = on.to_filter_param();
    let sql = format!("SELECT * FROM {TABLE_NAME} WHERE {column} = ? LIMIT 1;");
    let params_json = utils::to_params_json(vec![value])?;
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<NostrProfile> = utils::parse_json(&json)?;
    Ok(rows.pop())
}

fn rel_query(rel: &NostrProfileFindManyRel) -> (&'static str, Vec<Value>) {
    match rel {
        NostrProfileFindManyRel::OnRelay(args) => (
            "SELECT pr.* FROM nostr_profile pr JOIN nostr_profile_relay pr_rl ON pr.id = pr_rl.tb_pr WHERE pr_rl.tb_rl = ?",
            vec![Value::from(args.id.clone())],
        ),
        NostrProfileFindManyRel::OffRelay(args) => (
            "SELECT pr.* FROM nostr_profile pr WHERE NOT EXISTS (SELECT 1 FROM nostr_profile_relay pr_rl WHERE pr_rl.tb_pr = pr.id AND pr_rl.tb_rl = ?)",
            vec![Value::from(args.id.clone())],
        ),
    }
}

fn find_one_by_rel<E: SqlExecutor>(
    exec: &E,
    rel: &NostrProfileFindManyRel,
) -> Result<Option<NostrProfile>, IError<SqlError>> {
    let (sql, bind_values) = rel_query(rel);
    let params_json = utils::to_params_json(bind_values)?;
    let sql = format!("{sql} LIMIT 1;");
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<NostrProfile> = utils::parse_json(&json)?;
    Ok(rows.pop())
}

fn find_many_by_rel<E: SqlExecutor>(
    exec: &E,
    rel: &NostrProfileFindManyRel,
) -> Result<Vec<NostrProfile>, IError<SqlError>> {
    let (sql, bind_values) = rel_query(rel);
    let params_json = utils::to_params_json(bind_values)?;
    let sql = format!("{sql};");
    let json = exec.query_raw(&sql, &params_json)?;
    let rows: Vec<NostrProfile> = utils::parse_json(&json)?;
    Ok(rows)
}

fn select_by_id<E: SqlExecutor>(exec: &E, id: &str) -> Result<NostrProfile, IError<SqlError>> {
    let params_json = utils::to_params_json(vec![Value::from(id.to_owned())])?;
    let sql = format!("SELECT * FROM {TABLE_NAME} WHERE id = ?;");
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<NostrProfile> = utils::parse_json(&json)?;
    rows.pop()
        .ok_or_else(|| IError::from(SqlError::NotFound(id.to_owned())))
}

pub fn update<E: SqlExecutor>(
    exec: &E,
    opts: &INostrProfileUpdate,
) -> Result<INostrProfileUpdateResolve, IError<SqlError>> {
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
    opts: &INostrProfileDelete,
) -> Result<INostrProfileDeleteResolve, IError<SqlError>> {
    let id_for_lookup = match opts {
        INostrProfileDelete::On(args) => match args.on.primary_key() {
            Some(id) => id,
            None => {
                let found = find_one_by_on(exec, &args.on)?;
                let model = found.ok_or_else(|| IError::from(SqlError::NotFound(args.on.lookup_key())))?;
                model.id
            }
        },
        INostrProfileDelete::Rel(args) => {
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

fn rel_lookup_key(rel: &NostrProfileFindManyRel) -> String {
    match rel {
        NostrProfileFindManyRel::OnRelay(args) => format!("on_relay:{}", args.id.as_str()),
        NostrProfileFindManyRel::OffRelay(args) => format!("off_relay:{}", args.id.as_str()),
    }
}
