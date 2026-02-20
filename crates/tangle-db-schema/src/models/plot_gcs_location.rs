use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize, Deserialize)]
pub struct PlotGcsLocation {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub plot_id: String,
    pub gcs_location_id: String,
    pub role: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotGcsLocationFields {
    pub plot_id: String,
    pub gcs_location_id: String,
    pub role: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotGcsLocationFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub plot_id: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gcs_location_id: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub role: Option<serde_json::Value>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IPlotGcsLocationFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub plot_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub gcs_location_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub role: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PlotGcsLocationQueryBindValues {
    Id { id: String },
    PlotId { plot_id: String },
    GcsLocationId { gcs_location_id: String },
}
impl PlotGcsLocationQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::PlotId { plot_id } => ("plot_id", Value::from(plot_id.clone())),
            Self::GcsLocationId { gcs_location_id } => {
                ("gcs_location_id", Value::from(gcs_location_id.clone()))
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
            Self::PlotId { plot_id } => plot_id.clone(),
            Self::GcsLocationId { gcs_location_id } => gcs_location_id.clone(),
        }
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub enum PlotGcsLocationFindManyRel {}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotGcsLocationCreate",
        type = "IPlotGcsLocationFields"
    )
)]
pub struct IPlotGcsLocationCreateTs;
pub type IPlotGcsLocationCreate = IPlotGcsLocationFields;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotGcsLocationCreateResolve",
        type = "IResult<PlotGcsLocation>"
    )
)]
pub struct IPlotGcsLocationCreateResolveTs;
pub type IPlotGcsLocationCreateResolve = IResult<PlotGcsLocation>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IPlotGcsLocationFindOneArgs {
    pub on: PlotGcsLocationQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IPlotGcsLocationFindOneRelArgs {
    pub rel: PlotGcsLocationFindManyRel,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IPlotGcsLocationFindOne")
)]
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IPlotGcsLocationFindOne {
    On(IPlotGcsLocationFindOneArgs),
    Rel(IPlotGcsLocationFindOneRelArgs),
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotGcsLocationFindOneResolve",
        type = "IResult<PlotGcsLocation>"
    )
)]
pub struct IPlotGcsLocationFindOneResolveTs;
pub type IPlotGcsLocationFindOneResolve = IResult<Option<PlotGcsLocation>>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IPlotGcsLocationFindMany")
)]
#[derive(Deserialize, Serialize)]
pub struct IPlotGcsLocationFindManyArgs {
    pub filter: Option<IPlotGcsLocationFieldsFilter>,
}
pub type IPlotGcsLocationFindMany = IPlotGcsLocationFindManyArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotGcsLocationFindManyResolve",
        type = "IResultList<PlotGcsLocation>"
    )
)]
pub struct IPlotGcsLocationFindManyResolveTs;
pub type IPlotGcsLocationFindManyResolve = IResultList<PlotGcsLocation>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotGcsLocationDelete",
        type = "IPlotGcsLocationFindOne"
    )
)]
pub struct IPlotGcsLocationDeleteTs;
pub type IPlotGcsLocationDelete = IPlotGcsLocationFindOne;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotGcsLocationDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct IPlotGcsLocationDeleteResolveTs;
pub type IPlotGcsLocationDeleteResolve = IResult<String>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IPlotGcsLocationUpdate")
)]
#[derive(Deserialize, Serialize)]
pub struct IPlotGcsLocationUpdateArgs {
    pub on: PlotGcsLocationQueryBindValues,
    pub fields: IPlotGcsLocationFieldsPartial,
}
pub type IPlotGcsLocationUpdate = IPlotGcsLocationUpdateArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IPlotGcsLocationUpdateResolve",
        type = "IResult<PlotGcsLocation>"
    )
)]
pub struct IPlotGcsLocationUpdateResolveTs;
pub type IPlotGcsLocationUpdateResolve = IResult<PlotGcsLocation>;
