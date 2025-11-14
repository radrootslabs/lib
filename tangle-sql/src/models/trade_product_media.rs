use radroots_sql_core::error::SqlError;
use radroots_sql_core::{SqlExecutor, utils};
use radroots_tangle_schema::trade_product_media::{
    ITradeProductMediaRelation,
    ITradeProductMediaResolve,
};
use radroots_types::types::{IError, IResultPass};
use serde_json::Value;

const TABLE_NAME: &str = "trade_product_media";

pub fn set<E: SqlExecutor>(
    exec: &E,
    opts: &ITradeProductMediaRelation,
) -> Result<ITradeProductMediaResolve, IError<SqlError>> {
    let mut query_vals: Vec<Value> = Vec::new();
    let (trade_product_column, trade_product_value) = opts.trade_product.to_filter_param();
    query_vals.push(trade_product_value);
    let (media_image_column, media_image_value) = opts.media_image.to_filter_param();
    query_vals.push(media_image_value);
    let query = format!("INSERT INTO {} (tb_tp, tb_mu) VALUES ((SELECT id FROM trade_product WHERE {} = ?), (SELECT id FROM media_image WHERE {} = ?));", TABLE_NAME, trade_product_column, media_image_column);
    let params_json = utils::to_params_json(query_vals)?;
    let _ = exec.exec(&query, &params_json)?;
    Ok(IResultPass { pass: true })
}

pub fn unset<E: SqlExecutor>(
    exec: &E,
    opts: &ITradeProductMediaRelation,
) -> Result<ITradeProductMediaResolve, IError<SqlError>> {
    let mut query_vals: Vec<Value> = Vec::new();
    let (trade_product_column, trade_product_value) = opts.trade_product.to_filter_param();
    query_vals.push(trade_product_value);
    let (media_image_column, media_image_value) = opts.media_image.to_filter_param();
    query_vals.push(media_image_value);
    let query = format!("DELETE FROM {} WHERE tb_tp = (SELECT id FROM trade_product WHERE {} = ?) AND tb_mu = (SELECT id FROM media_image WHERE {} = ?);", TABLE_NAME, trade_product_column, media_image_column);
    let params_json = utils::to_params_json(query_vals)?;
    let _ = exec.exec(&query, &params_json)?;
    Ok(IResultPass { pass: true })
}
