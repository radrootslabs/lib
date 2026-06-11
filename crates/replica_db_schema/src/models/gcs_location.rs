use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Clone, Deserialize, Serialize)]
pub struct IGcsLocationFields {
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

#[derive(Clone, Deserialize, Serialize)]
pub struct IGcsLocationFieldsPartial {
    pub d_tag: Option<serde_json::Value>,
    pub lat: Option<serde_json::Value>,
    pub lng: Option<serde_json::Value>,
    pub geohash: Option<serde_json::Value>,
    pub point: Option<serde_json::Value>,
    pub polygon: Option<serde_json::Value>,
    pub accuracy: Option<serde_json::Value>,
    pub altitude: Option<serde_json::Value>,
    pub tag_0: Option<serde_json::Value>,
    pub label: Option<serde_json::Value>,
    pub area: Option<serde_json::Value>,
    pub elevation: Option<serde_json::Value>,
    pub soil: Option<serde_json::Value>,
    pub climate: Option<serde_json::Value>,
    pub gc_id: Option<serde_json::Value>,
    pub gc_name: Option<serde_json::Value>,
    pub gc_admin1_id: Option<serde_json::Value>,
    pub gc_admin1_name: Option<serde_json::Value>,
    pub gc_country_id: Option<serde_json::Value>,
    pub gc_country_name: Option<serde_json::Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IGcsLocationFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub d_tag: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub geohash: Option<String>,
    pub point: Option<String>,
    pub polygon: Option<String>,
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

#[derive(Clone, Deserialize, Serialize)]
pub struct GcsLocationTradeProductArgs {
    pub id: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct GcsLocationFarmArgs {
    pub id: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct GcsLocationPlotArgs {
    pub id: String,
}

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

pub struct IGcsLocationCreateTs;
pub type IGcsLocationCreate = IGcsLocationFields;
pub struct IGcsLocationCreateResolveTs;
pub type IGcsLocationCreateResolve = IResult<GcsLocation>;
#[derive(Deserialize, Serialize)]
pub struct IGcsLocationFindOneArgs {
    pub on: GcsLocationQueryBindValues,
}

#[derive(Deserialize, Serialize)]
pub struct IGcsLocationFindOneRelArgs {
    pub rel: GcsLocationFindManyRel,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IGcsLocationFindOne {
    On(IGcsLocationFindOneArgs),
    Rel(IGcsLocationFindOneRelArgs),
}

pub struct IGcsLocationFindOneResolveTs;
pub type IGcsLocationFindOneResolve = IResult<Option<GcsLocation>>;
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
pub struct IGcsLocationFindManyResolveTs;
pub type IGcsLocationFindManyResolve = IResultList<GcsLocation>;
pub struct IGcsLocationDeleteTs;
pub type IGcsLocationDelete = IGcsLocationFindOne;
pub struct IGcsLocationDeleteResolveTs;
pub type IGcsLocationDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct IGcsLocationUpdateArgs {
    pub on: GcsLocationQueryBindValues,
    pub fields: IGcsLocationFieldsPartial,
}
pub type IGcsLocationUpdate = IGcsLocationUpdateArgs;
pub struct IGcsLocationUpdateResolveTs;
pub type IGcsLocationUpdateResolve = IResult<GcsLocation>;
