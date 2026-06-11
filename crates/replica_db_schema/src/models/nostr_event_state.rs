use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct NostrEventState {
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
pub struct INostrEventStateFields {
    pub key: String,
    pub kind: u32,
    pub pubkey: String,
    pub d_tag: String,
    pub last_event_id: String,
    pub last_created_at: u32,
    pub content_hash: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct INostrEventStateFieldsPartial {
    pub key: Option<serde_json::Value>,
    pub kind: Option<serde_json::Value>,
    pub pubkey: Option<serde_json::Value>,
    pub d_tag: Option<serde_json::Value>,
    pub last_event_id: Option<serde_json::Value>,
    pub last_created_at: Option<serde_json::Value>,
    pub content_hash: Option<serde_json::Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct INostrEventStateFieldsFilter {
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
pub enum NostrEventStateQueryBindValues {
    Id { id: String },
    Key { key: String },
}
impl NostrEventStateQueryBindValues {
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

pub struct INostrEventStateCreateTs;
pub type INostrEventStateCreate = INostrEventStateFields;
pub struct INostrEventStateCreateResolveTs;
pub type INostrEventStateCreateResolve = IResult<NostrEventState>;
#[derive(Deserialize, Serialize)]
pub struct INostrEventStateFindOneArgs {
    pub on: NostrEventStateQueryBindValues,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum INostrEventStateFindOne {
    On(INostrEventStateFindOneArgs),
}

pub struct INostrEventStateFindOneResolveTs;
pub type INostrEventStateFindOneResolve = IResult<Option<NostrEventState>>;
#[derive(Deserialize, Serialize)]
pub struct INostrEventStateFindManyArgs {
    pub filter: Option<INostrEventStateFieldsFilter>,
}
pub type INostrEventStateFindMany = INostrEventStateFindManyArgs;
pub struct INostrEventStateFindManyResolveTs;
pub type INostrEventStateFindManyResolve = IResultList<NostrEventState>;
pub struct INostrEventStateDeleteTs;
pub type INostrEventStateDelete = INostrEventStateFindOne;
pub struct INostrEventStateDeleteResolveTs;
pub type INostrEventStateDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct INostrEventStateUpdateArgs {
    pub on: NostrEventStateQueryBindValues,
    pub fields: INostrEventStateFieldsPartial,
}
pub type INostrEventStateUpdate = INostrEventStateUpdateArgs;
pub struct INostrEventStateUpdateResolveTs;
pub type INostrEventStateUpdateResolve = IResult<NostrEventState>;
