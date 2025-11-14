use radroots_sql_core::error::SqlError;
use radroots_sql_core::{SqlExecutor, utils};
use radroots_tangle_schema::farm_location::{
    IFarmLocationRelation,
    IFarmLocationResolve,
};
use radroots_types::types::{IError, IResultPass};
use serde_json::Value;

const TABLE_NAME: &str = "farm_location";

pub fn set<E: SqlExecutor>(
    exec: &E,
    opts: &IFarmLocationRelation,
) -> Result<IFarmLocationResolve, IError<SqlError>> {
    let mut query_vals: Vec<Value> = Vec::new();
    let (farm_column, farm_value) = opts.farm.to_filter_param();
    query_vals.push(farm_value);
    let (location_gcs_column, location_gcs_value) = opts.location_gcs.to_filter_param();
    query_vals.push(location_gcs_value);
    let query = format!("INSERT INTO {} (tb_farm, tb_lg) VALUES ((SELECT id FROM farm WHERE {} = ?), (SELECT id FROM location_gcs WHERE {} = ?));", TABLE_NAME, farm_column, location_gcs_column);
    let params_json = utils::to_params_json(query_vals)?;
    let _ = exec.exec(&query, &params_json)?;
    Ok(IResultPass { pass: true })
}

pub fn unset<E: SqlExecutor>(
    exec: &E,
    opts: &IFarmLocationRelation,
) -> Result<IFarmLocationResolve, IError<SqlError>> {
    let mut query_vals: Vec<Value> = Vec::new();
    let (farm_column, farm_value) = opts.farm.to_filter_param();
    query_vals.push(farm_value);
    let (location_gcs_column, location_gcs_value) = opts.location_gcs.to_filter_param();
    query_vals.push(location_gcs_value);
    let query = format!("DELETE FROM {} WHERE tb_farm = (SELECT id FROM farm WHERE {} = ?) AND tb_lg = (SELECT id FROM location_gcs WHERE {} = ?);", TABLE_NAME, farm_column, location_gcs_column);
    let params_json = utils::to_params_json(query_vals)?;
    let _ = exec.exec(&query, &params_json)?;
    Ok(IResultPass { pass: true })
}
