use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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
    pub qty_amt: i64,
    pub qty_unit: String,
    pub qty_label: Option<String>,
    pub qty_avail: Option<i64>,
    pub price_amt: f64,
    pub price_currency: String,
    pub price_qty_amt: u32,
    pub price_qty_unit: String,
    pub notes: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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
    pub qty_amt: i64,
    pub qty_unit: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub qty_label: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub qty_avail: Option<i64>,
    pub price_amt: f64,
    pub price_currency: String,
    pub price_qty_amt: u32,
    pub price_qty_unit: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub notes: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct ITradeProductFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub key: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub category: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub title: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub summary: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub process: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub lot: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub profile: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub year: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub qty_amt: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub qty_unit: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub qty_label: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub qty_avail: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub price_amt: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub price_currency: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub price_qty_amt: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub price_qty_unit: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub notes: Option<serde_json::Value>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct ITradeProductFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub key: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub category: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub title: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub summary: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub process: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub lot: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub profile: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub year: Option<i64>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub qty_amt: Option<i64>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub qty_unit: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub qty_label: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub qty_avail: Option<i64>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub price_amt: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub price_currency: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub price_qty_amt: Option<u32>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub price_qty_unit: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub notes: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ITradeProductCreate",
        type = "ITradeProductFields"
    )
)]
pub struct ITradeProductCreateTs;
pub type ITradeProductCreate = ITradeProductFields;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ITradeProductCreateResolve",
        type = "IResult<TradeProduct>"
    )
)]
pub struct ITradeProductCreateResolveTs;
pub type ITradeProductCreateResolve = IResult<TradeProduct>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "ITradeProductFindOne")
)]
#[derive(Deserialize, Serialize)]
pub struct ITradeProductFindOneArgs {
    pub on: TradeProductQueryBindValues,
}
pub type ITradeProductFindOne = ITradeProductFindOneArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ITradeProductFindOneResolve",
        type = "IResult<TradeProduct | undefined>"
    )
)]
pub struct ITradeProductFindOneResolveTs;
pub type ITradeProductFindOneResolve = IResult<Option<TradeProduct>>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "ITradeProductFindMany")
)]
#[derive(Deserialize, Serialize)]
pub struct ITradeProductFindManyArgs {
    pub filter: Option<ITradeProductFieldsFilter>,
}
pub type ITradeProductFindMany = ITradeProductFindManyArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ITradeProductFindManyResolve",
        type = "IResultList<TradeProduct>"
    )
)]
pub struct ITradeProductFindManyResolveTs;
pub type ITradeProductFindManyResolve = IResultList<TradeProduct>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ITradeProductDelete",
        type = "ITradeProductFindOne"
    )
)]
pub struct ITradeProductDeleteTs;
pub type ITradeProductDelete = ITradeProductFindOneArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ITradeProductDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct ITradeProductDeleteResolveTs;
pub type ITradeProductDeleteResolve = IResult<String>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "ITradeProductUpdate")
)]
#[derive(Deserialize, Serialize)]
pub struct ITradeProductUpdateArgs {
    pub on: TradeProductQueryBindValues,
    pub fields: ITradeProductFieldsPartial,
}
pub type ITradeProductUpdate = ITradeProductUpdateArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ITradeProductUpdateResolve",
        type = "IResult<TradeProduct>"
    )
)]
pub struct ITradeProductUpdateResolveTs;
pub type ITradeProductUpdateResolve = IResult<TradeProduct>;
