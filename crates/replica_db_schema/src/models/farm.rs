use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct Farm {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub d_tag: String,
    pub pubkey: String,
    pub name: String,
    pub about: Option<String>,
    pub website: Option<String>,
    pub picture: Option<String>,
    pub banner: Option<String>,
    pub location_primary: Option<String>,
    pub location_city: Option<String>,
    pub location_region: Option<String>,
    pub location_country: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmFields {
    pub d_tag: String,
    pub pubkey: String,
    pub name: String,
    pub about: Option<String>,
    pub website: Option<String>,
    pub picture: Option<String>,
    pub banner: Option<String>,
    pub location_primary: Option<String>,
    pub location_city: Option<String>,
    pub location_region: Option<String>,
    pub location_country: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmFieldsPartial {
    pub d_tag: Option<serde_json::Value>,
    pub pubkey: Option<serde_json::Value>,
    pub name: Option<serde_json::Value>,
    pub about: Option<serde_json::Value>,
    pub website: Option<serde_json::Value>,
    pub picture: Option<serde_json::Value>,
    pub banner: Option<serde_json::Value>,
    pub location_primary: Option<serde_json::Value>,
    pub location_city: Option<serde_json::Value>,
    pub location_region: Option<serde_json::Value>,
    pub location_country: Option<serde_json::Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub d_tag: Option<String>,
    pub pubkey: Option<String>,
    pub name: Option<String>,
    pub about: Option<String>,
    pub website: Option<String>,
    pub picture: Option<String>,
    pub banner: Option<String>,
    pub location_primary: Option<String>,
    pub location_city: Option<String>,
    pub location_region: Option<String>,
    pub location_country: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FarmQueryBindValues {
    Id { id: String },
    DTag { d_tag: String },
    Pubkey { pubkey: String },
}
impl FarmQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::DTag { d_tag } => ("d_tag", Value::from(d_tag.clone())),
            Self::Pubkey { pubkey } => ("pubkey", Value::from(pubkey.clone())),
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
            Self::Pubkey { pubkey } => pubkey.clone(),
        }
    }
}

pub struct IFarmCreateTs;
pub type IFarmCreate = IFarmFields;
pub struct IFarmCreateResolveTs;
pub type IFarmCreateResolve = IResult<Farm>;
#[derive(Deserialize, Serialize)]
pub struct IFarmFindOneArgs {
    pub on: FarmQueryBindValues,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IFarmFindOne {
    On(IFarmFindOneArgs),
}

pub struct IFarmFindOneResolveTs;
pub type IFarmFindOneResolve = IResult<Option<Farm>>;
#[derive(Deserialize, Serialize)]
pub struct IFarmFindManyArgs {
    pub filter: Option<IFarmFieldsFilter>,
}
pub type IFarmFindMany = IFarmFindManyArgs;
pub struct IFarmFindManyResolveTs;
pub type IFarmFindManyResolve = IResultList<Farm>;
pub struct IFarmDeleteTs;
pub type IFarmDelete = IFarmFindOne;
pub struct IFarmDeleteResolveTs;
pub type IFarmDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct IFarmUpdateArgs {
    pub on: FarmQueryBindValues,
    pub fields: IFarmFieldsPartial,
}
pub type IFarmUpdate = IFarmUpdateArgs;
pub struct IFarmUpdateResolveTs;
pub type IFarmUpdateResolve = IResult<Farm>;
