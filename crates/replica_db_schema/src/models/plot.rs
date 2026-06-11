use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct Plot {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub d_tag: String,
    pub farm_id: String,
    pub name: String,
    pub about: Option<String>,
    pub location_primary: Option<String>,
    pub location_city: Option<String>,
    pub location_region: Option<String>,
    pub location_country: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotFields {
    pub d_tag: String,
    pub farm_id: String,
    pub name: String,
    pub about: Option<String>,
    pub location_primary: Option<String>,
    pub location_city: Option<String>,
    pub location_region: Option<String>,
    pub location_country: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotFieldsPartial {
    pub d_tag: Option<serde_json::Value>,
    pub farm_id: Option<serde_json::Value>,
    pub name: Option<serde_json::Value>,
    pub about: Option<serde_json::Value>,
    pub location_primary: Option<serde_json::Value>,
    pub location_city: Option<serde_json::Value>,
    pub location_region: Option<serde_json::Value>,
    pub location_country: Option<serde_json::Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub d_tag: Option<String>,
    pub farm_id: Option<String>,
    pub name: Option<String>,
    pub about: Option<String>,
    pub location_primary: Option<String>,
    pub location_city: Option<String>,
    pub location_region: Option<String>,
    pub location_country: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PlotQueryBindValues {
    Id { id: String },
    DTag { d_tag: String },
    FarmId { farm_id: String },
}
impl PlotQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::DTag { d_tag } => ("d_tag", Value::from(d_tag.clone())),
            Self::FarmId { farm_id } => ("farm_id", Value::from(farm_id.clone())),
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
            Self::FarmId { farm_id } => farm_id.clone(),
        }
    }
}

pub struct IPlotCreateTs;
pub type IPlotCreate = IPlotFields;
pub struct IPlotCreateResolveTs;
pub type IPlotCreateResolve = IResult<Plot>;
#[derive(Deserialize, Serialize)]
pub struct IPlotFindOneArgs {
    pub on: PlotQueryBindValues,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IPlotFindOne {
    On(IPlotFindOneArgs),
}

pub struct IPlotFindOneResolveTs;
pub type IPlotFindOneResolve = IResult<Option<Plot>>;
#[derive(Deserialize, Serialize)]
pub struct IPlotFindManyArgs {
    pub filter: Option<IPlotFieldsFilter>,
}
pub type IPlotFindMany = IPlotFindManyArgs;
pub struct IPlotFindManyResolveTs;
pub type IPlotFindManyResolve = IResultList<Plot>;
pub struct IPlotDeleteTs;
pub type IPlotDelete = IPlotFindOne;
pub struct IPlotDeleteResolveTs;
pub type IPlotDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct IPlotUpdateArgs {
    pub on: PlotQueryBindValues,
    pub fields: IPlotFieldsPartial,
}
pub type IPlotUpdate = IPlotUpdateArgs;
pub struct IPlotUpdateResolveTs;
pub type IPlotUpdateResolve = IResult<Plot>;
