use radroots_replica_schema::nostr_relay::{
    INostrRelayCreate, INostrRelayCreateResolve, INostrRelayDelete, INostrRelayDeleteResolve,
    INostrRelayFieldsFilter, INostrRelayFindMany, INostrRelayFindManyResolve, INostrRelayFindOne,
    INostrRelayFindOneResolve, INostrRelayUpdate, INostrRelayUpdateResolve, NostrRelay,
    NostrRelayFindManyRel, NostrRelayQueryBindValues,
};
use radroots_replica_schema::{ReplicaSchemaError, ReplicaSchemaResult, ReplicaSchemaResultList};
use radroots_sql_core::error::SqlError;
use radroots_sql_core::{SqlExecutor, utils};
use serde_json::Value;

const TABLE_NAME: &str = "nostr_relay";

pub fn create(
    exec: &dyn SqlExecutor,
    opts: &INostrRelayCreate,
) -> Result<INostrRelayCreateResolve, ReplicaSchemaError<SqlError>> {
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
    let on = NostrRelayQueryBindValues::Id { id: id.clone() };
    let result = find_one_by_on(exec, &on)?
        .ok_or(ReplicaSchemaError::from(SqlError::NotFound(id.clone())))?;
    Ok(ReplicaSchemaResult { result })
}

pub fn find_one(
    exec: &dyn SqlExecutor,
    opts: &INostrRelayFindOne,
) -> Result<INostrRelayFindOneResolve, ReplicaSchemaError<SqlError>> {
    let result = match opts {
        INostrRelayFindOne::On(args) => find_one_by_on(exec, &args.on)?,
        INostrRelayFindOne::Rel(args) => find_one_by_rel(exec, &args.rel)?,
    };
    Ok(ReplicaSchemaResult { result })
}

pub fn find_many(
    exec: &dyn SqlExecutor,
    opts: &INostrRelayFindMany,
) -> Result<INostrRelayFindManyResolve, ReplicaSchemaError<SqlError>> {
    let results = match opts {
        INostrRelayFindMany::Filter { filter } => find_many_filter(exec, filter)?,
        INostrRelayFindMany::Rel { rel } => find_many_by_rel(exec, rel)?,
    };
    Ok(ReplicaSchemaResultList { results })
}

fn find_many_filter(
    exec: &dyn SqlExecutor,
    filter: &Option<INostrRelayFieldsFilter>,
) -> Result<Vec<NostrRelay>, ReplicaSchemaError<SqlError>> {
    let (sql, bind_values) = utils::build_select_query_with_meta(TABLE_NAME, filter.as_ref());
    let params_json = utils::to_params_json(bind_values).expect("serialize bind params");
    let json = exec.query_raw(&sql, &params_json)?;
    let rows: Vec<NostrRelay> = utils::parse_json(&json)?;
    Ok(rows)
}

fn find_one_by_on(
    exec: &dyn SqlExecutor,
    on: &NostrRelayQueryBindValues,
) -> Result<Option<NostrRelay>, ReplicaSchemaError<SqlError>> {
    let (column, value) = on.to_filter_param();
    let sql = format!("SELECT * FROM {TABLE_NAME} WHERE {column} = ? LIMIT 1;");
    let params_json = utils::to_params_json(vec![value]).expect("serialize bind params");
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<NostrRelay> = utils::parse_json(&json)?;
    Ok(rows.pop())
}

fn rel_query(rel: &NostrRelayFindManyRel) -> (&'static str, Vec<Value>) {
    match rel {
        NostrRelayFindManyRel::OnProfile(args) => (
            "SELECT rl.* FROM nostr_relay rl JOIN nostr_profile_relay pr_rl ON rl.id = pr_rl.tb_rl JOIN nostr_profile pr ON pr.id = pr_rl.tb_pr WHERE pr.public_key = ?",
            vec![Value::from(args.public_key.clone())],
        ),
        NostrRelayFindManyRel::OffProfile(args) => (
            "SELECT rl.* FROM nostr_relay rl LEFT JOIN nostr_profile_relay pr_rl ON rl.id = pr_rl.tb_rl LEFT JOIN nostr_profile pr ON pr.id = pr_rl.tb_pr WHERE pr.public_key <> ?",
            vec![Value::from(args.public_key.clone())],
        ),
    }
}

fn find_one_by_rel(
    exec: &dyn SqlExecutor,
    rel: &NostrRelayFindManyRel,
) -> Result<Option<NostrRelay>, ReplicaSchemaError<SqlError>> {
    let (sql, bind_values) = rel_query(rel);
    let params_json = utils::to_params_json(bind_values).expect("serialize bind params");
    let sql = format!("{sql} LIMIT 1;");
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<NostrRelay> = utils::parse_json(&json)?;
    Ok(rows.pop())
}

fn find_many_by_rel(
    exec: &dyn SqlExecutor,
    rel: &NostrRelayFindManyRel,
) -> Result<Vec<NostrRelay>, ReplicaSchemaError<SqlError>> {
    let (sql, bind_values) = rel_query(rel);
    let params_json = utils::to_params_json(bind_values).expect("serialize bind params");
    let sql = format!("{sql};");
    let json = exec.query_raw(&sql, &params_json)?;
    let rows: Vec<NostrRelay> = utils::parse_json(&json)?;
    Ok(rows)
}

fn select_by_id(
    exec: &dyn SqlExecutor,
    id: &str,
) -> Result<NostrRelay, ReplicaSchemaError<SqlError>> {
    let params_json =
        utils::to_params_json(vec![Value::from(id.to_owned())]).expect("serialize bind params");
    let sql = format!("SELECT * FROM {TABLE_NAME} WHERE id = ?;");
    let json = exec.query_raw(&sql, &params_json)?;
    let mut rows: Vec<NostrRelay> = utils::parse_json(&json)?;
    rows.pop()
        .ok_or(ReplicaSchemaError::from(SqlError::NotFound(id.to_owned())))
}

pub fn update(
    exec: &dyn SqlExecutor,
    opts: &INostrRelayUpdate,
) -> Result<INostrRelayUpdateResolve, ReplicaSchemaError<SqlError>> {
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
    opts: &INostrRelayDelete,
) -> Result<INostrRelayDeleteResolve, ReplicaSchemaError<SqlError>> {
    let id_for_lookup = match opts {
        INostrRelayDelete::On(args) => match args.on.primary_key() {
            Some(id) => id,
            None => {
                let found = find_one_by_on(exec, &args.on)?;
                let model = found.ok_or(ReplicaSchemaError::from(SqlError::NotFound(
                    args.on.lookup_key(),
                )))?;
                model.id
            }
        },
        INostrRelayDelete::Rel(args) => {
            let found = find_one_by_rel(exec, &args.rel)?;
            let model = found.ok_or(ReplicaSchemaError::from(SqlError::NotFound(
                rel_lookup_key(&args.rel),
            )))?;
            model.id
        }
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

fn rel_lookup_key(rel: &NostrRelayFindManyRel) -> String {
    match rel {
        NostrRelayFindManyRel::OnProfile(args) => {
            format!("on_profile:{}", args.public_key.as_str())
        }
        NostrRelayFindManyRel::OffProfile(args) => {
            format!("off_profile:{}", args.public_key.as_str())
        }
    }
}
