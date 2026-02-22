use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmFields {
    pub d_tag: String,
    pub pubkey: String,
    pub name: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub about: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub website: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub picture: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub banner: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub location_primary: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub location_city: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub location_region: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub location_country: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub d_tag: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub pubkey: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub name: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub about: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub website: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub picture: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub banner: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub location_primary: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub location_city: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub location_region: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub location_country: Option<serde_json::Value>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub d_tag: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub about: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub website: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub picture: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub banner: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub location_primary: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub location_city: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub location_region: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub location_country: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmCreate",
        type = "IFarmFields"
    )
)]
pub struct IFarmCreateTs;
pub type IFarmCreate = IFarmFields;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmCreateResolve",
        type = "IResult<Farm>"
    )
)]
pub struct IFarmCreateResolveTs;
pub type IFarmCreateResolve = IResult<Farm>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IFarmFindOneArgs {
    pub on: FarmQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IFarmFindOne")
)]
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IFarmFindOne {
    On(IFarmFindOneArgs),
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmFindOneResolve",
        type = "IResult<Farm>"
    )
)]
pub struct IFarmFindOneResolveTs;
pub type IFarmFindOneResolve = IResult<Option<Farm>>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IFarmFindMany")
)]
#[derive(Deserialize, Serialize)]
pub struct IFarmFindManyArgs {
    pub filter: Option<IFarmFieldsFilter>,
}
pub type IFarmFindMany = IFarmFindManyArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmFindManyResolve",
        type = "IResultList<Farm>"
    )
)]
pub struct IFarmFindManyResolveTs;
pub type IFarmFindManyResolve = IResultList<Farm>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmDelete",
        type = "IFarmFindOne"
    )
)]
pub struct IFarmDeleteTs;
pub type IFarmDelete = IFarmFindOne;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct IFarmDeleteResolveTs;
pub type IFarmDeleteResolve = IResult<String>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IFarmUpdate")
)]
#[derive(Deserialize, Serialize)]
pub struct IFarmUpdateArgs {
    pub on: FarmQueryBindValues,
    pub fields: IFarmFieldsPartial,
}
pub type IFarmUpdate = IFarmUpdateArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmUpdateResolve",
        type = "IResult<Farm>"
    )
)]
pub struct IFarmUpdateResolveTs;
pub type IFarmUpdateResolve = IResult<Farm>;
