use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize, Deserialize)]
pub struct FarmTag {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub farm_id: String,
    pub tag: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmTagFields {
    pub farm_id: String,
    pub tag: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmTagFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub farm_id: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub tag: Option<serde_json::Value>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmTagFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub farm_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub tag: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FarmTagQueryBindValues {
    Id { id: String },
    FarmId { farm_id: String },
    Tag { tag: String },
}
impl FarmTagQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::FarmId { farm_id } => ("farm_id", Value::from(farm_id.clone())),
            Self::Tag { tag } => ("tag", Value::from(tag.clone())),
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
            Self::FarmId { farm_id } => farm_id.clone(),
            Self::Tag { tag } => tag.clone(),
        }
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub enum FarmTagFindManyRel {}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmTagCreate",
        type = "IFarmTagFields"
    )
)]
pub struct IFarmTagCreateTs;
pub type IFarmTagCreate = IFarmTagFields;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmTagCreateResolve",
        type = "IResult<FarmTag>"
    )
)]
pub struct IFarmTagCreateResolveTs;
pub type IFarmTagCreateResolve = IResult<FarmTag>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IFarmTagFindOneArgs {
    pub on: FarmTagQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IFarmTagFindOneRelArgs {
    pub rel: FarmTagFindManyRel,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IFarmTagFindOne")
)]
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IFarmTagFindOne {
    On(IFarmTagFindOneArgs),
    Rel(IFarmTagFindOneRelArgs),
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmTagFindOneResolve",
        type = "IResult<FarmTag>"
    )
)]
pub struct IFarmTagFindOneResolveTs;
pub type IFarmTagFindOneResolve = IResult<Option<FarmTag>>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IFarmTagFindMany")
)]
#[derive(Deserialize, Serialize)]
pub struct IFarmTagFindManyArgs {
    pub filter: Option<IFarmTagFieldsFilter>,
}
pub type IFarmTagFindMany = IFarmTagFindManyArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmTagFindManyResolve",
        type = "IResultList<FarmTag>"
    )
)]
pub struct IFarmTagFindManyResolveTs;
pub type IFarmTagFindManyResolve = IResultList<FarmTag>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmTagDelete",
        type = "IFarmTagFindOne"
    )
)]
pub struct IFarmTagDeleteTs;
pub type IFarmTagDelete = IFarmTagFindOne;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmTagDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct IFarmTagDeleteResolveTs;
pub type IFarmTagDeleteResolve = IResult<String>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IFarmTagUpdate")
)]
#[derive(Deserialize, Serialize)]
pub struct IFarmTagUpdateArgs {
    pub on: FarmTagQueryBindValues,
    pub fields: IFarmTagFieldsPartial,
}
pub type IFarmTagUpdate = IFarmTagUpdateArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmTagUpdateResolve",
        type = "IResult<FarmTag>"
    )
)]
pub struct IFarmTagUpdateResolveTs;
pub type IFarmTagUpdateResolve = IResult<FarmTag>;
