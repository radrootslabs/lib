use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize, Deserialize)]
pub struct LocationGcs {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub lat: f64,
    pub lng: f64,
    pub geohash: String,
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
pub struct ILocationGcsFields {
    pub lat: f64,
    pub lng: f64,
    pub geohash: String,
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
pub struct ILocationGcsFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub lat: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub lng: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub geohash: Option<serde_json::Value>,
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
pub struct ILocationGcsFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub lat: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub lng: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub geohash: Option<String>,
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
pub enum LocationGcsQueryBindValues {
    Id { id: String },
    Geohash { geohash: String },
}

impl LocationGcsQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
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
            Self::Geohash { geohash } => geohash.clone(),
        }
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ILocationGcsCreate",
        type = "ILocationGcsFields"
    )
)]
pub struct ILocationGcsCreateTs;
pub type ILocationGcsCreate = ILocationGcsFields;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ILocationGcsCreateResolve",
        type = "IResult<LocationGcs>"
    )
)]
pub struct ILocationGcsCreateResolveTs;
pub type ILocationGcsCreateResolve = IResult<LocationGcs>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "ILocationGcsFindOne")
)]
#[derive(Deserialize, Serialize)]
pub struct ILocationGcsFindOneArgs {
    pub on: LocationGcsQueryBindValues,
}
pub type ILocationGcsFindOne = ILocationGcsFindOneArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ILocationGcsFindOneResolve",
        type = "IResult<LocationGcs | undefined>"
    )
)]
pub struct ILocationGcsFindOneResolveTs;
pub type ILocationGcsFindOneResolve = IResult<Option<LocationGcs>>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "ILocationGcsFindMany")
)]
#[derive(Deserialize, Serialize)]
pub struct ILocationGcsFindManyArgs {
    pub filter: Option<ILocationGcsFieldsFilter>,
}
pub type ILocationGcsFindMany = ILocationGcsFindManyArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ILocationGcsFindManyResolve",
        type = "IResultList<LocationGcs>"
    )
)]
pub struct ILocationGcsFindManyResolveTs;
pub type ILocationGcsFindManyResolve = IResultList<LocationGcs>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ILocationGcsDelete",
        type = "ILocationGcsFindOne"
    )
)]
pub struct ILocationGcsDeleteTs;
pub type ILocationGcsDelete = ILocationGcsFindOneArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ILocationGcsDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct ILocationGcsDeleteResolveTs;
pub type ILocationGcsDeleteResolve = IResult<String>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "ILocationGcsUpdate")
)]
#[derive(Deserialize, Serialize)]
pub struct ILocationGcsUpdateArgs {
    pub on: LocationGcsQueryBindValues,
    pub fields: ILocationGcsFieldsPartial,
}
pub type ILocationGcsUpdate = ILocationGcsUpdateArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ILocationGcsUpdateResolve",
        type = "IResult<LocationGcs>"
    )
)]
pub struct ILocationGcsUpdateResolveTs;
pub type ILocationGcsUpdateResolve = IResult<LocationGcs>;
