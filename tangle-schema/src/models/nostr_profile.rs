use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize, Deserialize)]
pub struct NostrProfile {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub public_key: String,
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrProfileFields {
    pub public_key: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub display_name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub about: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub website: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub picture: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub banner: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub nip05: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub lud06: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub lud16: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrProfileFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub public_key: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub name: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub display_name: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub about: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub website: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub picture: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub banner: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub nip05: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub lud06: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub lud16: Option<serde_json::Value>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrProfileFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub public_key: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub display_name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub about: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub website: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub picture: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub banner: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub nip05: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub lud06: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub lud16: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrProfileCreate",
        type = "INostrProfileFields"
    )
)]
pub struct INostrProfileCreateTs;
pub type INostrProfileCreate = INostrProfileFields;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrProfileCreateResolve",
        type = "IResult<NostrProfile>"
    )
)]
pub struct INostrProfileCreateResolveTs;
pub type INostrProfileCreateResolve = IResult<NostrProfile>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "INostrProfileFindOne")
)]
#[derive(Deserialize, Serialize)]
pub struct INostrProfileFindOneArgs {
    pub on: NostrProfileQueryBindValues,
}
pub type INostrProfileFindOne = INostrProfileFindOneArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrProfileFindOneResolve",
        type = "IResult<NostrProfile | undefined>"
    )
)]
pub struct INostrProfileFindOneResolveTs;
pub type INostrProfileFindOneResolve = IResult<Option<NostrProfile>>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "INostrProfileFindMany")
)]
#[derive(Deserialize, Serialize)]
pub struct INostrProfileFindManyArgs {
    pub filter: Option<INostrProfileFieldsFilter>,
}
pub type INostrProfileFindMany = INostrProfileFindManyArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrProfileFindManyResolve",
        type = "IResultList<NostrProfile>"
    )
)]
pub struct INostrProfileFindManyResolveTs;
pub type INostrProfileFindManyResolve = IResultList<NostrProfile>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrProfileDelete",
        type = "INostrProfileFindOne"
    )
)]
pub struct INostrProfileDeleteTs;
pub type INostrProfileDelete = INostrProfileFindOneArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrProfileDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct INostrProfileDeleteResolveTs;
pub type INostrProfileDeleteResolve = IResult<String>;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "INostrProfileUpdate")
)]
#[derive(Deserialize, Serialize)]
pub struct INostrProfileUpdateArgs {
    pub on: NostrProfileQueryBindValues,
    pub fields: INostrProfileFieldsPartial,
}
pub type INostrProfileUpdate = INostrProfileUpdateArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrProfileUpdateResolve",
        type = "IResult<NostrProfile>"
    )
)]
pub struct INostrProfileUpdateResolveTs;
pub type INostrProfileUpdateResolve = IResult<NostrProfile>;
