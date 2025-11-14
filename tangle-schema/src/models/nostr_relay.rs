use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrRelayFields {
    pub url: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub relay_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub description: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub contact: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub supported_nips: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub software: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub version: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub data: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrRelayFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub url: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub relay_id: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub name: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub description: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub pubkey: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub contact: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub supported_nips: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub software: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub version: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub data: Option<serde_json::Value>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrRelayFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub url: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub relay_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub description: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub contact: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub supported_nips: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub software: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub version: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub data: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrRelayCreate",
        type = "INostrRelayFields"
    )
)]
pub struct INostrRelayCreateTs;
pub type INostrRelayCreate = INostrRelayFields;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrRelayCreateResolve",
        type = "IResult<NostrRelay>"
    )
)]
pub struct INostrRelayCreateResolveTs;
pub type INostrRelayCreateResolve = IResult<NostrRelay>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "INostrRelayFindOne")
)]
#[derive(Deserialize, Serialize)]
pub struct INostrRelayFindOneArgs {
    pub on: NostrRelayQueryBindValues,
}
pub type INostrRelayFindOne = INostrRelayFindOneArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrRelayFindOneResolve",
        type = "IResult<NostrRelay | undefined>"
    )
)]
pub struct INostrRelayFindOneResolveTs;
pub type INostrRelayFindOneResolve = IResult<Option<NostrRelay>>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "INostrRelayFindMany")
)]
#[derive(Deserialize, Serialize)]
pub struct INostrRelayFindManyArgs {
    pub filter: Option<INostrRelayFieldsFilter>,
}
pub type INostrRelayFindMany = INostrRelayFindManyArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrRelayFindManyResolve",
        type = "IResultList<NostrRelay>"
    )
)]
pub struct INostrRelayFindManyResolveTs;
pub type INostrRelayFindManyResolve = IResultList<NostrRelay>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrRelayDelete",
        type = "INostrRelayFindOne"
    )
)]
pub struct INostrRelayDeleteTs;
pub type INostrRelayDelete = INostrRelayFindOneArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrRelayDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct INostrRelayDeleteResolveTs;
pub type INostrRelayDeleteResolve = IResult<String>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "INostrRelayUpdate")
)]
#[derive(Deserialize, Serialize)]
pub struct INostrRelayUpdateArgs {
    pub on: NostrRelayQueryBindValues,
    pub fields: INostrRelayFieldsPartial,
}
pub type INostrRelayUpdate = INostrRelayUpdateArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrRelayUpdateResolve",
        type = "IResult<NostrRelay>"
    )
)]
pub struct INostrRelayUpdateResolveTs;
pub type INostrRelayUpdateResolve = IResult<NostrRelay>;
