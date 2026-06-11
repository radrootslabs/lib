use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Serialize, Deserialize)]
pub struct LogError {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub error: String,
    pub message: String,
    pub stack_trace: Option<String>,
    pub cause: Option<String>,
    pub app_system: String,
    pub app_version: String,
    pub nostr_pubkey: String,
    pub data: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct ILogErrorFields {
    pub error: String,
    pub message: String,
    pub stack_trace: Option<String>,
    pub cause: Option<String>,
    pub app_system: String,
    pub app_version: String,
    pub nostr_pubkey: String,
    pub data: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct ILogErrorFieldsPartial {
    pub error: Option<serde_json::Value>,
    pub message: Option<serde_json::Value>,
    pub stack_trace: Option<serde_json::Value>,
    pub cause: Option<serde_json::Value>,
    pub app_system: Option<serde_json::Value>,
    pub app_version: Option<serde_json::Value>,
    pub nostr_pubkey: Option<serde_json::Value>,
    pub data: Option<serde_json::Value>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct ILogErrorFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub error: Option<String>,
    pub message: Option<String>,
    pub stack_trace: Option<String>,
    pub cause: Option<String>,
    pub app_system: Option<String>,
    pub app_version: Option<String>,
    pub nostr_pubkey: Option<String>,
    pub data: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum LogErrorQueryBindValues {
    Id { id: String },
    NostrPubkey { nostr_pubkey: String },
}
impl LogErrorQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::NostrPubkey { nostr_pubkey } => {
                ("nostr_pubkey", Value::from(nostr_pubkey.clone()))
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
            Self::NostrPubkey { nostr_pubkey } => nostr_pubkey.clone(),
        }
    }
}
pub struct ILogErrorCreateTs;
pub type ILogErrorCreate = ILogErrorFields;
pub struct ILogErrorCreateResolveTs;
pub type ILogErrorCreateResolve = IResult<LogError>;
#[derive(Deserialize, Serialize)]
pub struct ILogErrorFindOneArgs {
    pub on: LogErrorQueryBindValues,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum ILogErrorFindOne {
    On(ILogErrorFindOneArgs),
}

pub struct ILogErrorFindOneResolveTs;
pub type ILogErrorFindOneResolve = IResult<Option<LogError>>;
#[derive(Deserialize, Serialize)]
pub struct ILogErrorFindManyArgs {
    pub filter: Option<ILogErrorFieldsFilter>,
}
pub type ILogErrorFindMany = ILogErrorFindManyArgs;
pub struct ILogErrorFindManyResolveTs;
pub type ILogErrorFindManyResolve = IResultList<LogError>;
pub struct ILogErrorDeleteTs;
pub type ILogErrorDelete = ILogErrorFindOne;
pub struct ILogErrorDeleteResolveTs;
pub type ILogErrorDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct ILogErrorUpdateArgs {
    pub on: LogErrorQueryBindValues,
    pub fields: ILogErrorFieldsPartial,
}
pub type ILogErrorUpdate = ILogErrorUpdateArgs;
pub struct ILogErrorUpdateResolveTs;
pub type ILogErrorUpdateResolve = IResult<LogError>;
