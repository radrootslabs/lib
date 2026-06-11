use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct PlotGcsLocation {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub plot_id: String,
    pub gcs_location_id: String,
    pub role: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotGcsLocationFields {
    pub plot_id: String,
    pub gcs_location_id: String,
    pub role: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotGcsLocationFieldsPartial {
    pub plot_id: Option<serde_json::Value>,
    pub gcs_location_id: Option<serde_json::Value>,
    pub role: Option<serde_json::Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotGcsLocationFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub plot_id: Option<String>,
    pub gcs_location_id: Option<String>,
    pub role: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PlotGcsLocationQueryBindValues {
    Id { id: String },
    PlotId { plot_id: String },
    GcsLocationId { gcs_location_id: String },
}
impl PlotGcsLocationQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::PlotId { plot_id } => ("plot_id", Value::from(plot_id.clone())),
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
            Self::PlotId { plot_id } => plot_id.clone(),
            Self::GcsLocationId { gcs_location_id } => gcs_location_id.clone(),
        }
    }
}

pub struct IPlotGcsLocationCreateTs;
pub type IPlotGcsLocationCreate = IPlotGcsLocationFields;
pub struct IPlotGcsLocationCreateResolveTs;
pub type IPlotGcsLocationCreateResolve = IResult<PlotGcsLocation>;
#[derive(Deserialize, Serialize)]
pub struct IPlotGcsLocationFindOneArgs {
    pub on: PlotGcsLocationQueryBindValues,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IPlotGcsLocationFindOne {
    On(IPlotGcsLocationFindOneArgs),
}

pub struct IPlotGcsLocationFindOneResolveTs;
pub type IPlotGcsLocationFindOneResolve = IResult<Option<PlotGcsLocation>>;
#[derive(Deserialize, Serialize)]
pub struct IPlotGcsLocationFindManyArgs {
    pub filter: Option<IPlotGcsLocationFieldsFilter>,
}
pub type IPlotGcsLocationFindMany = IPlotGcsLocationFindManyArgs;
pub struct IPlotGcsLocationFindManyResolveTs;
pub type IPlotGcsLocationFindManyResolve = IResultList<PlotGcsLocation>;
pub struct IPlotGcsLocationDeleteTs;
pub type IPlotGcsLocationDelete = IPlotGcsLocationFindOne;
pub struct IPlotGcsLocationDeleteResolveTs;
pub type IPlotGcsLocationDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct IPlotGcsLocationUpdateArgs {
    pub on: PlotGcsLocationQueryBindValues,
    pub fields: IPlotGcsLocationFieldsPartial,
}
pub type IPlotGcsLocationUpdate = IPlotGcsLocationUpdateArgs;
pub struct IPlotGcsLocationUpdateResolveTs;
pub type IPlotGcsLocationUpdateResolve = IResult<PlotGcsLocation>;
