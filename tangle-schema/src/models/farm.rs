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
    pub name: String,
    pub area: Option<String>,
    pub area_unit: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmFields {
    pub name: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub area: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub area_unit: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub title: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub description: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub name: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub area: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub area_unit: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub title: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub description: Option<serde_json::Value>,
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
    pub name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub area: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub area_unit: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub title: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub description: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FarmQueryBindValues {
    Id { id: String },
}

impl FarmQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
        }
    }

    pub fn primary_key(&self) -> Option<String> {
        match self {
            Self::Id { id } => Some(id.clone()),
        }
    }

    pub fn lookup_key(&self) -> String {
        match self {
            Self::Id { id } => id.clone(),
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
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IFarmFindOne")
)]
#[derive(Deserialize, Serialize)]
pub struct IFarmFindOneArgs {
    pub on: FarmQueryBindValues,
}
pub type IFarmFindOne = IFarmFindOneArgs;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmFindOneResolve",
        type = "IResult<Farm | undefined>"
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
pub type IFarmDelete = IFarmFindOneArgs;

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
