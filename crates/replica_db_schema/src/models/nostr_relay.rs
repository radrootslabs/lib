use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Serialize, Deserialize)]
pub struct NostrRelay {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
    pub relay_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub pubkey: Option<String>,
    pub contact: Option<String>,
    pub supported_nips: Option<String>,
    pub software: Option<String>,
    pub version: Option<String>,
    pub data: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrRelayFields {
    pub url: String,
    pub relay_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub pubkey: Option<String>,
    pub contact: Option<String>,
    pub supported_nips: Option<String>,
    pub software: Option<String>,
    pub version: Option<String>,
    pub data: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrRelayFieldsPartial {
    pub url: Option<serde_json::Value>,
    pub relay_id: Option<serde_json::Value>,
    pub name: Option<serde_json::Value>,
    pub description: Option<serde_json::Value>,
    pub pubkey: Option<serde_json::Value>,
    pub contact: Option<serde_json::Value>,
    pub supported_nips: Option<serde_json::Value>,
    pub software: Option<serde_json::Value>,
    pub version: Option<serde_json::Value>,
    pub data: Option<serde_json::Value>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrRelayFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub url: Option<String>,
    pub relay_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub pubkey: Option<String>,
    pub contact: Option<String>,
    pub supported_nips: Option<String>,
    pub software: Option<String>,
    pub version: Option<String>,
    pub data: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum NostrRelayQueryBindValues {
    Id { id: String },
    Url { url: String },
}
impl NostrRelayQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::Url { url } => ("url", Value::from(url.clone())),
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
            Self::Url { url } => url.clone(),
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
pub struct NostrRelayProfileArgs {
    pub public_key: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum NostrRelayFindManyRel {
    #[serde(rename = "on_profile")]
    OnProfile(NostrRelayProfileArgs),
    #[serde(rename = "off_profile")]
    OffProfile(NostrRelayProfileArgs),
}

pub struct INostrRelayCreateTs;
pub type INostrRelayCreate = INostrRelayFields;
pub struct INostrRelayCreateResolveTs;
pub type INostrRelayCreateResolve = IResult<NostrRelay>;
#[derive(Deserialize, Serialize)]
pub struct INostrRelayFindOneArgs {
    pub on: NostrRelayQueryBindValues,
}

#[derive(Deserialize, Serialize)]
pub struct INostrRelayFindOneRelArgs {
    pub rel: NostrRelayFindManyRel,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum INostrRelayFindOne {
    On(INostrRelayFindOneArgs),
    Rel(INostrRelayFindOneRelArgs),
}

pub struct INostrRelayFindOneResolveTs;
pub type INostrRelayFindOneResolve = IResult<Option<NostrRelay>>;
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum INostrRelayFindMany {
    Filter {
        filter: Option<INostrRelayFieldsFilter>,
    },
    Rel {
        rel: NostrRelayFindManyRel,
    },
}
pub struct INostrRelayFindManyResolveTs;
pub type INostrRelayFindManyResolve = IResultList<NostrRelay>;
pub struct INostrRelayDeleteTs;
pub type INostrRelayDelete = INostrRelayFindOne;
pub struct INostrRelayDeleteResolveTs;
pub type INostrRelayDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct INostrRelayUpdateArgs {
    pub on: NostrRelayQueryBindValues,
    pub fields: INostrRelayFieldsPartial,
}
pub type INostrRelayUpdate = INostrRelayUpdateArgs;
pub struct INostrRelayUpdateResolveTs;
pub type INostrRelayUpdateResolve = IResult<NostrRelay>;
