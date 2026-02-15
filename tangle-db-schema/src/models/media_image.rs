use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize, Deserialize)]
pub struct MediaImage {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub file_path: String,
    pub mime_type: String,
    pub res_base: String,
    pub res_path: String,
    pub label: Option<String>,
    pub description: Option<String>,
}
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IMediaImageFields {
    pub file_path: String,
    pub mime_type: String,
    pub res_base: String,
    pub res_path: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub label: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub description: Option<String>,
}
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IMediaImageFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub file_path: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub mime_type: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub res_base: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub res_path: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub label: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub description: Option<serde_json::Value>,
}
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IMediaImageFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub file_path: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub mime_type: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub res_base: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub res_path: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub label: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub description: Option<String>,
}
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MediaImageQueryBindValues {
    Id { id: String },
    FilePath { file_path: String },
}
impl MediaImageQueryBindValues {
    pub fn to_filter_param(&self) -> (&'static str, Value) {
        match self {
            Self::Id { id } => ("id", Value::from(id.clone())),
            Self::FilePath { file_path } => ("file_path", Value::from(file_path.clone())),
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
            Self::FilePath { file_path } => file_path.clone(),
        }
    }
}
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct MediaImageTradeProductArgs {
    pub id: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub enum MediaImageFindManyRel {
    #[serde(rename = "on_trade_product")]
    OnTradeProduct(MediaImageTradeProductArgs),
    #[serde(rename = "off_trade_product")]
    OffTradeProduct(MediaImageTradeProductArgs),
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IMediaImageCreate",
        type = "IMediaImageFields"
    )
)]
pub struct IMediaImageCreateTs;
pub type IMediaImageCreate = IMediaImageFields;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IMediaImageCreateResolve",
        type = "IResult<MediaImage>"
    )
)]
pub struct IMediaImageCreateResolveTs;
pub type IMediaImageCreateResolve = IResult<MediaImage>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IMediaImageFindOneArgs {
    pub on: MediaImageQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Deserialize, Serialize)]
pub struct IMediaImageFindOneRelArgs {
    pub rel: MediaImageFindManyRel,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IMediaImageFindOne")
)]
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IMediaImageFindOne {
    On(IMediaImageFindOneArgs),
    Rel(IMediaImageFindOneRelArgs),
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IMediaImageFindOneResolve",
        type = "IResult<MediaImage>"
    )
)]
pub struct IMediaImageFindOneResolveTs;
pub type IMediaImageFindOneResolve = IResult<Option<MediaImage>>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IMediaImageFindMany")
)]
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IMediaImageFindMany {
    Filter {
        filter: Option<IMediaImageFieldsFilter>,
    },
    Rel {
        rel: MediaImageFindManyRel,
    },
}
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IMediaImageFindManyResolve",
        type = "IResultList<MediaImage>"
    )
)]
pub struct IMediaImageFindManyResolveTs;
pub type IMediaImageFindManyResolve = IResultList<MediaImage>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IMediaImageDelete",
        type = "IMediaImageFindOne"
    )
)]
pub struct IMediaImageDeleteTs;
pub type IMediaImageDelete = IMediaImageFindOne;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IMediaImageDeleteResolve",
        type = "IResult<string>"
    )
)]
pub struct IMediaImageDeleteResolveTs;
pub type IMediaImageDeleteResolve = IResult<String>;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename = "IMediaImageUpdate")
)]
#[derive(Deserialize, Serialize)]
pub struct IMediaImageUpdateArgs {
    pub on: MediaImageQueryBindValues,
    pub fields: IMediaImageFieldsPartial,
}
pub type IMediaImageUpdate = IMediaImageUpdateArgs;
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IMediaImageUpdateResolve",
        type = "IResult<MediaImage>"
    )
)]
pub struct IMediaImageUpdateResolveTs;
pub type IMediaImageUpdateResolve = IResult<MediaImage>;
