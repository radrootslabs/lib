use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct PlotTag {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub plot_id: String,
    pub tag: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotTagFields {
    pub plot_id: String,
    pub tag: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotTagFieldsPartial {
    pub plot_id: Option<serde_json::Value>,
    pub tag: Option<serde_json::Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotTagFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub plot_id: Option<String>,
    pub tag: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PlotTagQueryBindValues {
    Id { id: String },
    PlotId { plot_id: String },
    Tag { tag: String },
}
impl PlotTagQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::PlotId { plot_id } => ("plot_id", Value::from(plot_id.clone())),
            Self::Tag { tag } => ("tag", Value::from(tag.clone())),
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
            Self::Tag { tag } => tag.clone(),
        }
    }
}

pub struct IPlotTagCreateTs;
pub type IPlotTagCreate = IPlotTagFields;
pub struct IPlotTagCreateResolveTs;
pub type IPlotTagCreateResolve = IResult<PlotTag>;
#[derive(Deserialize, Serialize)]
pub struct IPlotTagFindOneArgs {
    pub on: PlotTagQueryBindValues,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IPlotTagFindOne {
    On(IPlotTagFindOneArgs),
}

pub struct IPlotTagFindOneResolveTs;
pub type IPlotTagFindOneResolve = IResult<Option<PlotTag>>;
#[derive(Deserialize, Serialize)]
pub struct IPlotTagFindManyArgs {
    pub filter: Option<IPlotTagFieldsFilter>,
}
pub type IPlotTagFindMany = IPlotTagFindManyArgs;
pub struct IPlotTagFindManyResolveTs;
pub type IPlotTagFindManyResolve = IResultList<PlotTag>;
pub struct IPlotTagDeleteTs;
pub type IPlotTagDelete = IPlotTagFindOne;
pub struct IPlotTagDeleteResolveTs;
pub type IPlotTagDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct IPlotTagUpdateArgs {
    pub on: PlotTagQueryBindValues,
    pub fields: IPlotTagFieldsPartial,
}
pub type IPlotTagUpdate = IPlotTagUpdateArgs;
pub struct IPlotTagUpdateResolveTs;
pub type IPlotTagUpdateResolve = IResult<PlotTag>;
