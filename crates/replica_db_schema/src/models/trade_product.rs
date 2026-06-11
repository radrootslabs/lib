use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Serialize, Deserialize)]
pub struct TradeProduct {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub key: String,
    pub category: String,
    pub title: String,
    pub summary: String,
    pub process: String,
    pub lot: String,
    pub profile: String,
    pub year: i64,
    pub qty_amt: f64,
    pub qty_amt_exact: Option<String>,
    pub qty_unit: String,
    pub qty_label: Option<String>,
    pub qty_avail: Option<i64>,
    pub price_amt: f64,
    pub price_amt_exact: Option<String>,
    pub price_currency: String,
    pub price_qty_amt: f64,
    pub price_qty_amt_exact: Option<String>,
    pub price_qty_unit: String,
    pub listing_addr: Option<String>,
    pub primary_bin_id: Option<String>,
    pub verified_primary_bin_id: Option<String>,
    pub notes: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct ITradeProductFields {
    pub key: String,
    pub category: String,
    pub title: String,
    pub summary: String,
    pub process: String,
    pub lot: String,
    pub profile: String,
    pub year: i64,
    pub qty_amt: f64,
    pub qty_amt_exact: String,
    pub qty_unit: String,
    pub qty_label: Option<String>,
    pub qty_avail: Option<i64>,
    pub price_amt: f64,
    pub price_amt_exact: String,
    pub price_currency: String,
    pub price_qty_amt: f64,
    pub price_qty_amt_exact: String,
    pub price_qty_unit: String,
    pub listing_addr: Option<String>,
    pub primary_bin_id: Option<String>,
    pub verified_primary_bin_id: Option<String>,
    pub notes: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct ITradeProductFieldsPartial {
    pub key: Option<serde_json::Value>,
    pub category: Option<serde_json::Value>,
    pub title: Option<serde_json::Value>,
    pub summary: Option<serde_json::Value>,
    pub process: Option<serde_json::Value>,
    pub lot: Option<serde_json::Value>,
    pub profile: Option<serde_json::Value>,
    pub year: Option<serde_json::Value>,
    pub qty_amt: Option<serde_json::Value>,
    pub qty_amt_exact: Option<serde_json::Value>,
    pub qty_unit: Option<serde_json::Value>,
    pub qty_label: Option<serde_json::Value>,
    pub qty_avail: Option<serde_json::Value>,
    pub price_amt: Option<serde_json::Value>,
    pub price_amt_exact: Option<serde_json::Value>,
    pub price_currency: Option<serde_json::Value>,
    pub price_qty_amt: Option<serde_json::Value>,
    pub price_qty_amt_exact: Option<serde_json::Value>,
    pub price_qty_unit: Option<serde_json::Value>,
    pub listing_addr: Option<serde_json::Value>,
    pub primary_bin_id: Option<serde_json::Value>,
    pub verified_primary_bin_id: Option<serde_json::Value>,
    pub notes: Option<serde_json::Value>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct ITradeProductFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub key: Option<String>,
    pub category: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub process: Option<String>,
    pub lot: Option<String>,
    pub profile: Option<String>,
    pub year: Option<i64>,
    pub qty_amt: Option<f64>,
    pub qty_amt_exact: Option<String>,
    pub qty_unit: Option<String>,
    pub qty_label: Option<String>,
    pub qty_avail: Option<i64>,
    pub price_amt: Option<f64>,
    pub price_amt_exact: Option<String>,
    pub price_currency: Option<String>,
    pub price_qty_amt: Option<f64>,
    pub price_qty_amt_exact: Option<String>,
    pub price_qty_unit: Option<String>,
    pub listing_addr: Option<String>,
    pub primary_bin_id: Option<String>,
    pub verified_primary_bin_id: Option<String>,
    pub notes: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TradeProductQueryBindValues {
    Id { id: String },
}
impl TradeProductQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
        }
    }

    pub fn primary_key(&self) -> Option<String> {
        match self {
            Self::Id { id } => Some(id.clone()),
        }
    }

    pub fn lookup_key(&self) -> String {
        match self {
            Self::Id { id } => id.clone(),
        }
    }
}
pub struct ITradeProductCreateTs;
pub type ITradeProductCreate = ITradeProductFields;
pub struct ITradeProductCreateResolveTs;
pub type ITradeProductCreateResolve = IResult<TradeProduct>;
#[derive(Deserialize, Serialize)]
pub struct ITradeProductFindOneArgs {
    pub on: TradeProductQueryBindValues,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum ITradeProductFindOne {
    On(ITradeProductFindOneArgs),
}

pub struct ITradeProductFindOneResolveTs;
pub type ITradeProductFindOneResolve = IResult<Option<TradeProduct>>;
#[derive(Deserialize, Serialize)]
pub struct ITradeProductFindManyArgs {
    pub filter: Option<ITradeProductFieldsFilter>,
}
pub type ITradeProductFindMany = ITradeProductFindManyArgs;
pub struct ITradeProductFindManyResolveTs;
pub type ITradeProductFindManyResolve = IResultList<TradeProduct>;
pub struct ITradeProductDeleteTs;
pub type ITradeProductDelete = ITradeProductFindOne;
pub struct ITradeProductDeleteResolveTs;
pub type ITradeProductDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct ITradeProductUpdateArgs {
    pub on: TradeProductQueryBindValues,
    pub fields: ITradeProductFieldsPartial,
}
pub type ITradeProductUpdate = ITradeProductUpdateArgs;
pub struct ITradeProductUpdateResolveTs;
pub type ITradeProductUpdateResolve = IResult<TradeProduct>;
