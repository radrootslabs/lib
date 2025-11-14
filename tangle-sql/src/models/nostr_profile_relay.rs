use radroots_sql_core::error::SqlError;
use radroots_sql_core::{SqlExecutor, utils};
use radroots_tangle_schema::nostr_profile_relay::{
    INostrProfileRelayRelation,
    INostrProfileRelayResolve,
};
use radroots_types::types::{IError, IResultPass};
use serde_json::Value;

const TABLE_NAME: &str = "nostr_profile_relay";

pub fn set<E: SqlExecutor>(
    exec: &E,
    opts: &INostrProfileRelayRelation,
) -> Result<INostrProfileRelayResolve, IError<SqlError>> {
    let mut query_vals: Vec<Value> = Vec::new();
    let (nostr_profile_column, nostr_profile_value) = opts.nostr_profile.to_filter_param();
    query_vals.push(nostr_profile_value);
    let (nostr_relay_column, nostr_relay_value) = opts.nostr_relay.to_filter_param();
    query_vals.push(nostr_relay_value);
    let query = format!("INSERT INTO {} (tb_pr, tb_rl) VALUES ((SELECT id FROM nostr_profile WHERE {} = ?), (SELECT id FROM nostr_relay WHERE {} = ?));", TABLE_NAME, nostr_profile_column, nostr_relay_column);
    let params_json = utils::to_params_json(query_vals)?;
    let _ = exec.exec(&query, &params_json)?;
    Ok(IResultPass { pass: true })
}

pub fn unset<E: SqlExecutor>(
    exec: &E,
    opts: &INostrProfileRelayRelation,
) -> Result<INostrProfileRelayResolve, IError<SqlError>> {
    let mut query_vals: Vec<Value> = Vec::new();
    let (nostr_profile_column, nostr_profile_value) = opts.nostr_profile.to_filter_param();
    query_vals.push(nostr_profile_value);
    let (nostr_relay_column, nostr_relay_value) = opts.nostr_relay.to_filter_param();
    query_vals.push(nostr_relay_value);
    let query = format!("DELETE FROM {} WHERE tb_pr = (SELECT id FROM nostr_profile WHERE {} = ?) AND tb_rl = (SELECT id FROM nostr_relay WHERE {} = ?);", TABLE_NAME, nostr_profile_column, nostr_relay_column);
    let params_json = utils::to_params_json(query_vals)?;
    let _ = exec.exec(&query, &params_json)?;
    Ok(IResultPass { pass: true })
}
