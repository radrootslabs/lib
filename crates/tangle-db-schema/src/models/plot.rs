use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize, Deserialize)]
pub struct Plot {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub d_tag: String,
    pub farm_id: String,
    pub name: String,
    pub about: Option<String>,
    pub location_primary: Option<String>,
    pub location_city: Option<String>,
    pub location_region: Option<String>,
    pub location_country: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotFields {
    pub d_tag: String,
    pub farm_id: String,
    pub name: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub about: Option<String>,
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
pub struct IPlotFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub d_tag: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub farm_id: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub name: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub about: Option<serde_json::Value>,
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
pub struct IPlotFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub d_tag: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub farm_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub about: Option<String>,
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
pub enum PlotQueryBindValues {
    Id { id: String },
    DTag { d_tag: String },
    FarmId { farm_id: String },
}
impl PlotQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::DTag { d_tag } => ("d_tag", Value::from(d_tag.clone())),
            Self::FarmId { farm_id } => ("farm_id", Value::from(farm_id.clone())),
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
            Self::FarmId { farm_id } => farm_id.clone(),
        }
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub enum PlotFindManyRel {}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotCreate",
        type = "IPlotFields"
    )
)]
pub struct IPlotCreateTs;
pub type IPlotCreate = IPlotFields;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotCreateResolve",
        type = "IResult<Plot>"
    )
)]
pub struct IPlotCreateResolveTs;
pub type IPlotCreateResolve = IResult<Plot>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IPlotFindOneArgs {
    pub on: PlotQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IPlotFindOneRelArgs {
    pub rel: PlotFindManyRel,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IPlotFindOne")
)]
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IPlotFindOne {
    On(IPlotFindOneArgs),
    Rel(IPlotFindOneRelArgs),
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotFindOneResolve",
        type = "IResult<Plot>"
    )
)]
pub struct IPlotFindOneResolveTs;
pub type IPlotFindOneResolve = IResult<Option<Plot>>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IPlotFindMany")
)]
#[derive(Deserialize, Serialize)]
pub struct IPlotFindManyArgs {
    pub filter: Option<IPlotFieldsFilter>,
}
pub type IPlotFindMany = IPlotFindManyArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotFindManyResolve",
        type = "IResultList<Plot>"
    )
)]
pub struct IPlotFindManyResolveTs;
pub type IPlotFindManyResolve = IResultList<Plot>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotDelete",
        type = "IPlotFindOne"
    )
)]
pub struct IPlotDeleteTs;
pub type IPlotDelete = IPlotFindOne;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct IPlotDeleteResolveTs;
pub type IPlotDeleteResolve = IResult<String>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IPlotUpdate")
)]
#[derive(Deserialize, Serialize)]
pub struct IPlotUpdateArgs {
    pub on: PlotQueryBindValues,
    pub fields: IPlotFieldsPartial,
}
pub type IPlotUpdate = IPlotUpdateArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotUpdateResolve",
        type = "IResult<Plot>"
    )
)]
pub struct IPlotUpdateResolveTs;
pub type IPlotUpdateResolve = IResult<Plot>;
