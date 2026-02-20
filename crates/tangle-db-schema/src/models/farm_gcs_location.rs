use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize, Deserialize)]
pub struct FarmGcsLocation {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub farm_id: String,
    pub gcs_location_id: String,
    pub role: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmGcsLocationFields {
    pub farm_id: String,
    pub gcs_location_id: String,
    pub role: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmGcsLocationFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub farm_id: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gcs_location_id: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub role: Option<serde_json::Value>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmGcsLocationFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub farm_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub gcs_location_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub role: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FarmGcsLocationQueryBindValues {
    Id { id: String },
    FarmId { farm_id: String },
    GcsLocationId { gcs_location_id: String },
}
impl FarmGcsLocationQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::FarmId { farm_id } => ("farm_id", Value::from(farm_id.clone())),
            Self::GcsLocationId { gcs_location_id } => {
                ("gcs_location_id", Value::from(gcs_location_id.clone()))
            }
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
            Self::FarmId { farm_id } => farm_id.clone(),
            Self::GcsLocationId { gcs_location_id } => gcs_location_id.clone(),
        }
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub enum FarmGcsLocationFindManyRel {}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmGcsLocationCreate",
        type = "IFarmGcsLocationFields"
    )
)]
pub struct IFarmGcsLocationCreateTs;
pub type IFarmGcsLocationCreate = IFarmGcsLocationFields;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmGcsLocationCreateResolve",
        type = "IResult<FarmGcsLocation>"
    )
)]
pub struct IFarmGcsLocationCreateResolveTs;
pub type IFarmGcsLocationCreateResolve = IResult<FarmGcsLocation>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IFarmGcsLocationFindOneArgs {
    pub on: FarmGcsLocationQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IFarmGcsLocationFindOneRelArgs {
    pub rel: FarmGcsLocationFindManyRel,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IFarmGcsLocationFindOne")
)]
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IFarmGcsLocationFindOne {
    On(IFarmGcsLocationFindOneArgs),
    Rel(IFarmGcsLocationFindOneRelArgs),
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmGcsLocationFindOneResolve",
        type = "IResult<FarmGcsLocation>"
    )
)]
pub struct IFarmGcsLocationFindOneResolveTs;
pub type IFarmGcsLocationFindOneResolve = IResult<Option<FarmGcsLocation>>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IFarmGcsLocationFindMany")
)]
#[derive(Deserialize, Serialize)]
pub struct IFarmGcsLocationFindManyArgs {
    pub filter: Option<IFarmGcsLocationFieldsFilter>,
}
pub type IFarmGcsLocationFindMany = IFarmGcsLocationFindManyArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmGcsLocationFindManyResolve",
        type = "IResultList<FarmGcsLocation>"
    )
)]
pub struct IFarmGcsLocationFindManyResolveTs;
pub type IFarmGcsLocationFindManyResolve = IResultList<FarmGcsLocation>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmGcsLocationDelete",
        type = "IFarmGcsLocationFindOne"
    )
)]
pub struct IFarmGcsLocationDeleteTs;
pub type IFarmGcsLocationDelete = IFarmGcsLocationFindOne;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmGcsLocationDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct IFarmGcsLocationDeleteResolveTs;
pub type IFarmGcsLocationDeleteResolve = IResult<String>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IFarmGcsLocationUpdate")
)]
#[derive(Deserialize, Serialize)]
pub struct IFarmGcsLocationUpdateArgs {
    pub on: FarmGcsLocationQueryBindValues,
    pub fields: IFarmGcsLocationFieldsPartial,
}
pub type IFarmGcsLocationUpdate = IFarmGcsLocationUpdateArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmGcsLocationUpdateResolve",
        type = "IResult<FarmGcsLocation>"
    )
)]
pub struct IFarmGcsLocationUpdateResolveTs;
pub type IFarmGcsLocationUpdateResolve = IResult<FarmGcsLocation>;
