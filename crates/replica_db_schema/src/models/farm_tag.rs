use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct FarmTag {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub farm_id: String,
    pub tag: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmTagFields {
    pub farm_id: String,
    pub tag: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmTagFieldsPartial {
    pub farm_id: Option<serde_json::Value>,
    pub tag: Option<serde_json::Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmTagFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub farm_id: Option<String>,
    pub tag: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FarmTagQueryBindValues {
    Id { id: String },
    FarmId { farm_id: String },
    Tag { tag: String },
}
impl FarmTagQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::FarmId { farm_id } => ("farm_id", Value::from(farm_id.clone())),
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
            Self::FarmId { farm_id } => farm_id.clone(),
            Self::Tag { tag } => tag.clone(),
        }
    }
}

pub struct IFarmTagCreateTs;
pub type IFarmTagCreate = IFarmTagFields;
pub struct IFarmTagCreateResolveTs;
pub type IFarmTagCreateResolve = IResult<FarmTag>;
#[derive(Deserialize, Serialize)]
pub struct IFarmTagFindOneArgs {
    pub on: FarmTagQueryBindValues,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IFarmTagFindOne {
    On(IFarmTagFindOneArgs),
}

pub struct IFarmTagFindOneResolveTs;
pub type IFarmTagFindOneResolve = IResult<Option<FarmTag>>;
#[derive(Deserialize, Serialize)]
pub struct IFarmTagFindManyArgs {
    pub filter: Option<IFarmTagFieldsFilter>,
}
pub type IFarmTagFindMany = IFarmTagFindManyArgs;
pub struct IFarmTagFindManyResolveTs;
pub type IFarmTagFindManyResolve = IResultList<FarmTag>;
pub struct IFarmTagDeleteTs;
pub type IFarmTagDelete = IFarmTagFindOne;
pub struct IFarmTagDeleteResolveTs;
pub type IFarmTagDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct IFarmTagUpdateArgs {
    pub on: FarmTagQueryBindValues,
    pub fields: IFarmTagFieldsPartial,
}
pub type IFarmTagUpdate = IFarmTagUpdateArgs;
pub struct IFarmTagUpdateResolveTs;
pub type IFarmTagUpdateResolve = IResult<FarmTag>;
