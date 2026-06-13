use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct NostrEventHead {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub key: String,
    pub kind: u32,
    pub pubkey: String,
    pub d_tag: String,
    pub last_event_id: String,
    pub last_created_at: u32,
    pub content_hash: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct INostrEventHeadFields {
    pub key: String,
    pub kind: u32,
    pub pubkey: String,
    pub d_tag: String,
    pub last_event_id: String,
    pub last_created_at: u32,
    pub content_hash: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct INostrEventHeadFieldsPartial {
    pub key: Option<serde_json::Value>,
    pub kind: Option<serde_json::Value>,
    pub pubkey: Option<serde_json::Value>,
    pub d_tag: Option<serde_json::Value>,
    pub last_event_id: Option<serde_json::Value>,
    pub last_created_at: Option<serde_json::Value>,
    pub content_hash: Option<serde_json::Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct INostrEventHeadFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub key: Option<String>,
    pub kind: Option<u32>,
    pub pubkey: Option<String>,
    pub d_tag: Option<String>,
    pub last_event_id: Option<String>,
    pub last_created_at: Option<u32>,
    pub content_hash: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum NostrEventHeadQueryBindValues {
    Id { id: String },
    Key { key: String },
}
impl NostrEventHeadQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::Key { key } => ("key", Value::from(key.clone())),
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
            Self::Key { key } => key.clone(),
        }
    }
}

pub struct INostrEventHeadCreateTs;
pub type INostrEventHeadCreate = INostrEventHeadFields;
pub struct INostrEventHeadCreateResolveTs;
pub type INostrEventHeadCreateResolve = IResult<NostrEventHead>;
#[derive(Deserialize, Serialize)]
pub struct INostrEventHeadFindOneArgs {
    pub on: NostrEventHeadQueryBindValues,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum INostrEventHeadFindOne {
    On(INostrEventHeadFindOneArgs),
}

pub struct INostrEventHeadFindOneResolveTs;
pub type INostrEventHeadFindOneResolve = IResult<Option<NostrEventHead>>;
#[derive(Deserialize, Serialize)]
pub struct INostrEventHeadFindManyArgs {
    pub filter: Option<INostrEventHeadFieldsFilter>,
}
pub type INostrEventHeadFindMany = INostrEventHeadFindManyArgs;
pub struct INostrEventHeadFindManyResolveTs;
pub type INostrEventHeadFindManyResolve = IResultList<NostrEventHead>;
pub struct INostrEventHeadDeleteTs;
pub type INostrEventHeadDelete = INostrEventHeadFindOne;
pub struct INostrEventHeadDeleteResolveTs;
pub type INostrEventHeadDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct INostrEventHeadUpdateArgs {
    pub on: NostrEventHeadQueryBindValues,
    pub fields: INostrEventHeadFieldsPartial,
}
pub type INostrEventHeadUpdate = INostrEventHeadUpdateArgs;
pub struct INostrEventHeadUpdateResolveTs;
pub type INostrEventHeadUpdateResolve = IResult<NostrEventHead>;
