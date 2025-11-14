use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct ILogErrorFields {
    pub error: String,
    pub message: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub stack_trace: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub cause: Option<String>,
    pub app_system: String,
    pub app_version: String,
    pub nostr_pubkey: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub data: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct ILogErrorFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub error: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub message: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub stack_trace: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub cause: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub app_system: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub app_version: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub nostr_pubkey: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub data: Option<serde_json::Value>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct ILogErrorFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub error: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub message: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub stack_trace: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub cause: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub app_system: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub app_version: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub nostr_pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub data: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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
            Self::NostrPubkey { nostr_pubkey } => ("nostr_pubkey", Value::from(nostr_pubkey.clone())),
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ILogErrorCreate",
        type = "ILogErrorFields"
    )
)]
pub struct ILogErrorCreateTs;
pub type ILogErrorCreate = ILogErrorFields;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ILogErrorCreateResolve",
        type = "IResult<LogError>"
    )
)]
pub struct ILogErrorCreateResolveTs;
pub type ILogErrorCreateResolve = IResult<LogError>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "ILogErrorFindOne")
)]
#[derive(Deserialize, Serialize)]
pub struct ILogErrorFindOneArgs {
    pub on: LogErrorQueryBindValues,
}
pub type ILogErrorFindOne = ILogErrorFindOneArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ILogErrorFindOneResolve",
        type = "IResult<LogError | undefined>"
    )
)]
pub struct ILogErrorFindOneResolveTs;
pub type ILogErrorFindOneResolve = IResult<Option<LogError>>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "ILogErrorFindMany")
)]
#[derive(Deserialize, Serialize)]
pub struct ILogErrorFindManyArgs {
    pub filter: Option<ILogErrorFieldsFilter>,
}
pub type ILogErrorFindMany = ILogErrorFindManyArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ILogErrorFindManyResolve",
        type = "IResultList<LogError>"
    )
)]
pub struct ILogErrorFindManyResolveTs;
pub type ILogErrorFindManyResolve = IResultList<LogError>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ILogErrorDelete",
        type = "ILogErrorFindOne"
    )
)]
pub struct ILogErrorDeleteTs;
pub type ILogErrorDelete = ILogErrorFindOneArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ILogErrorDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct ILogErrorDeleteResolveTs;
pub type ILogErrorDeleteResolve = IResult<String>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "ILogErrorUpdate")
)]
#[derive(Deserialize, Serialize)]
pub struct ILogErrorUpdateArgs {
    pub on: LogErrorQueryBindValues,
    pub fields: ILogErrorFieldsPartial,
}
pub type ILogErrorUpdate = ILogErrorUpdateArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ILogErrorUpdateResolve",
        type = "IResult<LogError>"
    )
)]
pub struct ILogErrorUpdateResolveTs;
pub type ILogErrorUpdateResolve = IResult<LogError>;
