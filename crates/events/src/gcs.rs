#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsGeoJsonPoint {
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub r#type: String,
    pub coordinates: [f64; 2],
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsGeoJsonPolygon {
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub r#type: String,
    pub coordinates: Vec<Vec<[f64; 2]>>,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsGcsLocation {
    pub lat: f64,
    pub lng: f64,
    pub geohash: String,
    pub point: RadrootsGeoJsonPoint,
    pub polygon: RadrootsGeoJsonPolygon,
    pub accuracy: Option<f64>,
    pub altitude: Option<f64>,
    pub tag_0: Option<String>,
    pub label: Option<String>,
    pub area: Option<f64>,
    pub elevation: Option<u32>,
    pub soil: Option<String>,
    pub climate: Option<String>,
    pub gc_id: Option<String>,
    pub gc_name: Option<String>,
    pub gc_admin1_id: Option<String>,
    pub gc_admin1_name: Option<String>,
    pub gc_country_id: Option<String>,
    pub gc_country_name: Option<String>,
}
