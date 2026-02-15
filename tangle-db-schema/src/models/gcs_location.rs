use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize, Deserialize)]
pub struct GcsLocation {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub d_tag: String,
    pub lat: f64,
    pub lng: f64,
    pub geohash: String,
    pub point: String,
    pub polygon: String,
    pub accuracy: Option<f64>,
    pub altitude: Option<f64>,
    pub tag_0: Option<String>,
    pub label: Option<String>,
    pub area: Option<f64>,
    pub elevation: Option<u32>,
    pub soil: Option<String>,
    pub climate: Option<String>,
    pub gc_id: Option<String>,
    pub gc_name: Option<String>,
    pub gc_admin1_id: Option<String>,
    pub gc_admin1_name: Option<String>,
    pub gc_country_id: Option<String>,
    pub gc_country_name: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IGcsLocationFields {
    pub d_tag: String,
    pub lat: f64,
    pub lng: f64,
    pub geohash: String,
    pub point: String,
    pub polygon: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub accuracy: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub altitude: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub tag_0: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub label: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub area: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub elevation: Option<u32>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub soil: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub climate: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_admin1_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_admin1_name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_country_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_country_name: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IGcsLocationFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub d_tag: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub lat: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub lng: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub geohash: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub point: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub polygon: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub accuracy: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub altitude: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub tag_0: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub label: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub area: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub elevation: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub soil: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub climate: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_id: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_name: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_admin1_id: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_admin1_name: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_country_id: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_country_name: Option<serde_json::Value>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IGcsLocationFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub d_tag: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub lat: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub lng: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub geohash: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub point: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub polygon: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub accuracy: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub altitude: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub tag_0: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub label: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub area: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub elevation: Option<u32>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub soil: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub climate: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub gc_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub gc_name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub gc_admin1_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub gc_admin1_name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub gc_country_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub gc_country_name: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GcsLocationQueryBindValues {
    Id { id: String },
    DTag { d_tag: String },
    Geohash { geohash: String },
}
impl GcsLocationQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::DTag { d_tag } => ("d_tag", Value::from(d_tag.clone())),
            Self::Geohash { geohash } => ("geohash", Value::from(geohash.clone())),
        }
    }

    pub fn primary_key(&self) -> Option<String> {
        match self {
            Self::Id { id } => Some(id.clone()),
            _ => None,
        }
    }

    pub fn lookup_key(&self) -> String {
        match self {
            Self::Id { id } => id.clone(),
            Self::DTag { d_tag } => d_tag.clone(),
            Self::Geohash { geohash } => geohash.clone(),
        }
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct GcsLocationTradeProductArgs {
    pub id: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct GcsLocationFarmArgs {
    pub id: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct GcsLocationPlotArgs {
    pub id: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub enum GcsLocationFindManyRel {
    #[serde(rename = "on_trade_product")]
    OnTradeProduct(GcsLocationTradeProductArgs),
    #[serde(rename = "off_trade_product")]
    OffTradeProduct(GcsLocationTradeProductArgs),
    #[serde(rename = "on_farm")]
    OnFarm(GcsLocationFarmArgs),
    #[serde(rename = "off_farm")]
    OffFarm(GcsLocationFarmArgs),
    #[serde(rename = "on_plot")]
    OnPlot(GcsLocationPlotArgs),
    #[serde(rename = "off_plot")]
    OffPlot(GcsLocationPlotArgs),
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IGcsLocationCreate",
        type = "IGcsLocationFields"
    )
)]
pub struct IGcsLocationCreateTs;
pub type IGcsLocationCreate = IGcsLocationFields;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IGcsLocationCreateResolve",
        type = "IResult<GcsLocation>"
    )
)]
pub struct IGcsLocationCreateResolveTs;
pub type IGcsLocationCreateResolve = IResult<GcsLocation>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IGcsLocationFindOneArgs {
    pub on: GcsLocationQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IGcsLocationFindOneRelArgs {
    pub rel: GcsLocationFindManyRel,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IGcsLocationFindOne")
)]
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IGcsLocationFindOne {
    On(IGcsLocationFindOneArgs),
    Rel(IGcsLocationFindOneRelArgs),
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IGcsLocationFindOneResolve",
        type = "IResult<GcsLocation>"
    )
)]
pub struct IGcsLocationFindOneResolveTs;
pub type IGcsLocationFindOneResolve = IResult<Option<GcsLocation>>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IGcsLocationFindMany")
)]
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IGcsLocationFindMany {
    Filter {
        filter: Option<IGcsLocationFieldsFilter>,
    },
    Rel {
        rel: GcsLocationFindManyRel,
    },
}
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IGcsLocationFindManyResolve",
        type = "IResultList<GcsLocation>"
    )
)]
pub struct IGcsLocationFindManyResolveTs;
pub type IGcsLocationFindManyResolve = IResultList<GcsLocation>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IGcsLocationDelete",
        type = "IGcsLocationFindOne"
    )
)]
pub struct IGcsLocationDeleteTs;
pub type IGcsLocationDelete = IGcsLocationFindOne;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IGcsLocationDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct IGcsLocationDeleteResolveTs;
pub type IGcsLocationDeleteResolve = IResult<String>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IGcsLocationUpdate")
)]
#[derive(Deserialize, Serialize)]
pub struct IGcsLocationUpdateArgs {
    pub on: GcsLocationQueryBindValues,
    pub fields: IGcsLocationFieldsPartial,
}
pub type IGcsLocationUpdate = IGcsLocationUpdateArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IGcsLocationUpdateResolve",
        type = "IResult<GcsLocation>"
    )
)]
pub struct IGcsLocationUpdateResolveTs;
pub type IGcsLocationUpdateResolve = IResult<GcsLocation>;
