use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Serialize, Deserialize)]
pub struct NostrProfile {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub public_key: String,
    pub profile_type: String,
    pub name: String,
    pub display_name: Option<String>,
    pub about: Option<String>,
    pub website: Option<String>,
    pub picture: Option<String>,
    pub banner: Option<String>,
    pub nip05: Option<String>,
    pub lud06: Option<String>,
    pub lud16: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrProfileFields {
    pub public_key: String,
    pub profile_type: String,
    pub name: String,
    pub display_name: Option<String>,
    pub about: Option<String>,
    pub website: Option<String>,
    pub picture: Option<String>,
    pub banner: Option<String>,
    pub nip05: Option<String>,
    pub lud06: Option<String>,
    pub lud16: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrProfileFieldsPartial {
    pub public_key: Option<serde_json::Value>,
    pub profile_type: Option<serde_json::Value>,
    pub name: Option<serde_json::Value>,
    pub display_name: Option<serde_json::Value>,
    pub about: Option<serde_json::Value>,
    pub website: Option<serde_json::Value>,
    pub picture: Option<serde_json::Value>,
    pub banner: Option<serde_json::Value>,
    pub nip05: Option<serde_json::Value>,
    pub lud06: Option<serde_json::Value>,
    pub lud16: Option<serde_json::Value>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrProfileFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub public_key: Option<String>,
    pub profile_type: Option<String>,
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub about: Option<String>,
    pub website: Option<String>,
    pub picture: Option<String>,
    pub banner: Option<String>,
    pub nip05: Option<String>,
    pub lud06: Option<String>,
    pub lud16: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum NostrProfileQueryBindValues {
    Id { id: String },
    PublicKey { public_key: String },
}
impl NostrProfileQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::PublicKey { public_key } => ("public_key", Value::from(public_key.clone())),
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
            Self::PublicKey { public_key } => public_key.clone(),
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
pub struct NostrProfileRelayArgs {
    pub id: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum NostrProfileFindManyRel {
    #[serde(rename = "on_relay")]
    OnRelay(NostrProfileRelayArgs),
    #[serde(rename = "off_relay")]
    OffRelay(NostrProfileRelayArgs),
}

pub struct INostrProfileCreateTs;
pub type INostrProfileCreate = INostrProfileFields;
pub struct INostrProfileCreateResolveTs;
pub type INostrProfileCreateResolve = IResult<NostrProfile>;
#[derive(Deserialize, Serialize)]
pub struct INostrProfileFindOneArgs {
    pub on: NostrProfileQueryBindValues,
}

#[derive(Deserialize, Serialize)]
pub struct INostrProfileFindOneRelArgs {
    pub rel: NostrProfileFindManyRel,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum INostrProfileFindOne {
    On(INostrProfileFindOneArgs),
    Rel(INostrProfileFindOneRelArgs),
}

pub struct INostrProfileFindOneResolveTs;
pub type INostrProfileFindOneResolve = IResult<Option<NostrProfile>>;
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum INostrProfileFindMany {
    Filter {
        filter: Option<INostrProfileFieldsFilter>,
    },
    Rel {
        rel: NostrProfileFindManyRel,
    },
}
pub struct INostrProfileFindManyResolveTs;
pub type INostrProfileFindManyResolve = IResultList<NostrProfile>;
pub struct INostrProfileDeleteTs;
pub type INostrProfileDelete = INostrProfileFindOne;
pub struct INostrProfileDeleteResolveTs;
pub type INostrProfileDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct INostrProfileUpdateArgs {
    pub on: NostrProfileQueryBindValues,
    pub fields: INostrProfileFieldsPartial,
}
pub type INostrProfileUpdate = INostrProfileUpdateArgs;
pub struct INostrProfileUpdateResolveTs;
pub type INostrProfileUpdateResolve = IResult<NostrProfile>;
