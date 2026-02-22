use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrEventStateFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub key: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub kind: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub pubkey: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub d_tag: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub last_event_id: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub last_created_at: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub content_hash: Option<serde_json::Value>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrEventStateFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub key: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub kind: Option<u32>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub d_tag: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub last_event_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub last_created_at: Option<u32>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub content_hash: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrEventStateCreate",
        type = "INostrEventStateFields"
    )
)]
pub struct INostrEventStateCreateTs;
pub type INostrEventStateCreate = INostrEventStateFields;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrEventStateCreateResolve",
        type = "IResult<NostrEventState>"
    )
)]
pub struct INostrEventStateCreateResolveTs;
pub type INostrEventStateCreateResolve = IResult<NostrEventState>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct INostrEventStateFindOneArgs {
    pub on: NostrEventStateQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "INostrEventStateFindOne")
)]
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum INostrEventStateFindOne {
    On(INostrEventStateFindOneArgs),
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrEventStateFindOneResolve",
        type = "IResult<NostrEventState>"
    )
)]
pub struct INostrEventStateFindOneResolveTs;
pub type INostrEventStateFindOneResolve = IResult<Option<NostrEventState>>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "INostrEventStateFindMany")
)]
#[derive(Deserialize, Serialize)]
pub struct INostrEventStateFindManyArgs {
    pub filter: Option<INostrEventStateFieldsFilter>,
}
pub type INostrEventStateFindMany = INostrEventStateFindManyArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrEventStateFindManyResolve",
        type = "IResultList<NostrEventState>"
    )
)]
pub struct INostrEventStateFindManyResolveTs;
pub type INostrEventStateFindManyResolve = IResultList<NostrEventState>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrEventStateDelete",
        type = "INostrEventStateFindOne"
    )
)]
pub struct INostrEventStateDeleteTs;
pub type INostrEventStateDelete = INostrEventStateFindOne;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrEventStateDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct INostrEventStateDeleteResolveTs;
pub type INostrEventStateDeleteResolve = IResult<String>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "INostrEventStateUpdate")
)]
#[derive(Deserialize, Serialize)]
pub struct INostrEventStateUpdateArgs {
    pub on: NostrEventStateQueryBindValues,
    pub fields: INostrEventStateFieldsPartial,
}
pub type INostrEventStateUpdate = INostrEventStateUpdateArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrEventStateUpdateResolve",
        type = "IResult<NostrEventState>"
    )
)]
pub struct INostrEventStateUpdateResolveTs;
pub type INostrEventStateUpdateResolve = IResult<NostrEventState>;
