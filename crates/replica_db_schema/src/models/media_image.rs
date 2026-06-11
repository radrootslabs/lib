use radroots_types::types::{IResult, IResultList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
#[derive(Clone, Deserialize, Serialize)]
pub struct IMediaImageFields {
    pub file_path: String,
    pub mime_type: String,
    pub res_base: String,
    pub res_path: String,
    pub label: Option<String>,
    pub description: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct IMediaImageFieldsPartial {
    pub file_path: Option<serde_json::Value>,
    pub mime_type: Option<serde_json::Value>,
    pub res_base: Option<serde_json::Value>,
    pub res_path: Option<serde_json::Value>,
    pub label: Option<serde_json::Value>,
    pub description: Option<serde_json::Value>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct IMediaImageFieldsFilter {
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub file_path: Option<String>,
    pub mime_type: Option<String>,
    pub res_base: Option<String>,
    pub res_path: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
}
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
#[derive(Clone, Deserialize, Serialize)]
pub struct MediaImageTradeProductArgs {
    pub id: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum MediaImageFindManyRel {
    #[serde(rename = "on_trade_product")]
    OnTradeProduct(MediaImageTradeProductArgs),
    #[serde(rename = "off_trade_product")]
    OffTradeProduct(MediaImageTradeProductArgs),
}

pub struct IMediaImageCreateTs;
pub type IMediaImageCreate = IMediaImageFields;
pub struct IMediaImageCreateResolveTs;
pub type IMediaImageCreateResolve = IResult<MediaImage>;
#[derive(Deserialize, Serialize)]
pub struct IMediaImageFindOneArgs {
    pub on: MediaImageQueryBindValues,
}

#[derive(Deserialize, Serialize)]
pub struct IMediaImageFindOneRelArgs {
    pub rel: MediaImageFindManyRel,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum IMediaImageFindOne {
    On(IMediaImageFindOneArgs),
    Rel(IMediaImageFindOneRelArgs),
}

pub struct IMediaImageFindOneResolveTs;
pub type IMediaImageFindOneResolve = IResult<Option<MediaImage>>;
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
pub struct IMediaImageFindManyResolveTs;
pub type IMediaImageFindManyResolve = IResultList<MediaImage>;
pub struct IMediaImageDeleteTs;
pub type IMediaImageDelete = IMediaImageFindOne;
pub struct IMediaImageDeleteResolveTs;
pub type IMediaImageDeleteResolve = IResult<String>;
#[derive(Deserialize, Serialize)]
pub struct IMediaImageUpdateArgs {
    pub on: MediaImageQueryBindValues,
    pub fields: IMediaImageFieldsPartial,
}
pub type IMediaImageUpdate = IMediaImageUpdateArgs;
pub struct IMediaImageUpdateResolveTs;
pub type IMediaImageUpdateResolve = IResult<MediaImage>;
