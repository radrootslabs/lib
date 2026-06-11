use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct FarmGcsLocation {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub farm_id: String,
    pub gcs_location_id: String,
    pub role: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmGcsLocationFields {
    pub farm_id: String,
    pub gcs_location_id: String,
    pub role: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmGcsLocationFieldsPartial {
    pub farm_id: Option<serde_json::Value>,
    pub gcs_location_id: Option<serde_json::Value>,
    pub role: Option<serde_json::Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmGcsLocationFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub farm_id: Option<String>,
    pub gcs_location_id: Option<String>,
    pub role: Option<String>,
}

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

pub struct IFarmGcsLocationCreateTs;
pub type IFarmGcsLocationCreate = IFarmGcsLocationFields;
pub struct IFarmGcsLocationCreateResolveTs;
pub type IFarmGcsLocationCreateResolve = IResult<FarmGcsLocation>;
#[derive(Deserialize, Serialize)]
pub struct IFarmGcsLocationFindOneArgs {
    pub on: FarmGcsLocationQueryBindValues,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IFarmGcsLocationFindOne {
    On(IFarmGcsLocationFindOneArgs),
}

pub struct IFarmGcsLocationFindOneResolveTs;
pub type IFarmGcsLocationFindOneResolve = IResult<Option<FarmGcsLocation>>;
#[derive(Deserialize, Serialize)]
pub struct IFarmGcsLocationFindManyArgs {
    pub filter: Option<IFarmGcsLocationFieldsFilter>,
}
pub type IFarmGcsLocationFindMany = IFarmGcsLocationFindManyArgs;
pub struct IFarmGcsLocationFindManyResolveTs;
pub type IFarmGcsLocationFindManyResolve = IResultList<FarmGcsLocation>;
pub struct IFarmGcsLocationDeleteTs;
pub type IFarmGcsLocationDelete = IFarmGcsLocationFindOne;
pub struct IFarmGcsLocationDeleteResolveTs;
pub type IFarmGcsLocationDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct IFarmGcsLocationUpdateArgs {
    pub on: FarmGcsLocationQueryBindValues,
    pub fields: IFarmGcsLocationFieldsPartial,
}
pub type IFarmGcsLocationUpdate = IFarmGcsLocationUpdateArgs;
pub struct IFarmGcsLocationUpdateResolveTs;
pub type IFarmGcsLocationUpdateResolve = IResult<FarmGcsLocation>;
