use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize, Deserialize)]
pub struct PlotTag {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub plot_id: String,
    pub tag: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotTagFields {
    pub plot_id: String,
    pub tag: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotTagFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub plot_id: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub tag: Option<serde_json::Value>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotTagFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub plot_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub tag: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PlotTagQueryBindValues {
    Id { id: String },
    PlotId { plot_id: String },
    Tag { tag: String },
}
impl PlotTagQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::PlotId { plot_id } => ("plot_id", Value::from(plot_id.clone())),
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
            Self::PlotId { plot_id } => plot_id.clone(),
            Self::Tag { tag } => tag.clone(),
        }
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub enum PlotTagFindManyRel {

}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotTagCreate",
        type = "IPlotTagFields"
    )
)]
pub struct IPlotTagCreateTs;
pub type IPlotTagCreate = IPlotTagFields;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotTagCreateResolve",
        type = "IResult<PlotTag>"
    )
)]
pub struct IPlotTagCreateResolveTs;
pub type IPlotTagCreateResolve = IResult<PlotTag>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IPlotTagFindOneArgs {
    pub on: PlotTagQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IPlotTagFindOneRelArgs {
    pub rel: PlotTagFindManyRel,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IPlotTagFindOne")
)]
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IPlotTagFindOne {
    On(IPlotTagFindOneArgs),
    Rel(IPlotTagFindOneRelArgs),
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotTagFindOneResolve",
        type = "IResult<PlotTag>"
    )
)]
pub struct IPlotTagFindOneResolveTs;
pub type IPlotTagFindOneResolve = IResult<Option<PlotTag>>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IPlotTagFindMany")
)]
#[derive(Deserialize, Serialize)]
pub struct IPlotTagFindManyArgs {
    pub filter: Option<IPlotTagFieldsFilter>,
}
pub type IPlotTagFindMany = IPlotTagFindManyArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotTagFindManyResolve",
        type = "IResultList<PlotTag>"
    )
)]
pub struct IPlotTagFindManyResolveTs;
pub type IPlotTagFindManyResolve = IResultList<PlotTag>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotTagDelete",
        type = "IPlotTagFindOne"
    )
)]
pub struct IPlotTagDeleteTs;
pub type IPlotTagDelete = IPlotTagFindOne;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotTagDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct IPlotTagDeleteResolveTs;
pub type IPlotTagDeleteResolve = IResult<String>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts", rename = "IPlotTagUpdate"))]
#[derive(Deserialize, Serialize)]
pub struct IPlotTagUpdateArgs {
    pub on: PlotTagQueryBindValues,
    pub fields: IPlotTagFieldsPartial,
}
pub type IPlotTagUpdate = IPlotTagUpdateArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotTagUpdateResolve",
        type = "IResult<PlotTag>"
    )
)]
pub struct IPlotTagUpdateResolveTs;
pub type IPlotTagUpdateResolve = IResult<PlotTag>;
